use super::*;
use crate::ProgressFill;

pub(super) fn paint_progress(paint: &mut PaintEntityContext<'_>) {
    let Some(progress) = paint.world.get::<Progress>(paint.entity).cloned() else {
        return;
    };
    if !progress.value.is_finite() || !progress.phase.is_finite() {
        return;
    }
    let rect = ClipRect::from(paint.rect);
    if !rect.width.is_finite()
        || !rect.height.is_finite()
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        return;
    }
    let radius = progress
        .style
        .corners
        .radius(rect.width, rect.height, paint.theme.corner_radius);
    let properties_progress = paint
        .world
        .get::<UiRole>(paint.entity)
        .is_some_and(|role| role.0 == EditorRole::PROGRESS);
    let default_track = if properties_progress {
        paint.base_theme.color(ThemeColorToken::EditorProgressTrack)
    } else {
        paint
            .base_theme
            .background
            .mix(paint.base_theme.surface, 0.4)
    };
    let track = progress
        .style
        .track
        .map_or(default_track, |color| color.resolve(paint.theme));
    paint.output.push(item(
        paint.clip,
        paint.z,
        Primitive::RoundedRect {
            rect,
            radius,
            color: track,
        },
    ));

    let (start, end) = if progress.indeterminate {
        let width = rect.width * 0.3;
        let phase = progress.phase.rem_euclid(1.0);
        let travel = 1.0 - (2.0 * phase - 1.0).abs();
        let x = rect.x + travel * (rect.width - width);
        (x, x + width)
    } else {
        (rect.x, rect.x + rect.width * progress.value.clamp(0.0, 1.0))
    };
    if end <= start {
        return;
    }
    let fill_rect = ClipRect {
        x: start,
        width: end - start,
        ..rect
    };
    let gradient_colors = match &progress.style.fill {
        ProgressFill::Gradient(colors) if colors.len() >= 2 => Some(
            colors
                .iter()
                .map(|color| color.resolve(paint.theme))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    };

    let mut pieces = Vec::new();
    match progress.style.segments {
        Some(requested) if requested >= 2 => {
            if let Some((count, gap, width)) = segment_layout(rect.width, rect.height, requested) {
                for index in 0..count {
                    let segment_start = rect.x + index as f32 * (width + gap);
                    let left = segment_start.max(start);
                    let right = (segment_start + width).min(end);
                    if right > left {
                        pieces.push(ClipRect {
                            x: left,
                            width: right - left,
                            ..rect
                        });
                    }
                }
            } else {
                pieces.push(fill_rect);
            }
        }
        _ => pieces.push(fill_rect),
    }
    for &piece in &pieces {
        let radius =
            progress
                .style
                .corners
                .radius(piece.width, piece.height, paint.theme.corner_radius);
        if let Some(glow) = &progress.style.fill_glow {
            push_effect_glow(
                paint.output,
                paint.layout.clip.map(ClipRect::from),
                paint.z.saturating_add(1),
                piece,
                radius,
                glow,
                paint.theme,
            );
        }
    }
    for &piece in &pieces {
        paint_fill_piece(
            paint,
            &progress,
            piece,
            fill_rect,
            gradient_colors.as_deref(),
        );
    }
    if let Some(border) = &progress.style.fill_border {
        for &piece in &pieces {
            let mut border = border.clone();
            border.radius =
                progress
                    .style
                    .corners
                    .radius(piece.width, piece.height, paint.theme.corner_radius);
            push_effect_border(
                paint.output,
                paint.clip,
                paint.z.saturating_add(3),
                piece,
                &border,
                paint.theme,
            );
        }
    }
}

fn segment_layout(width: f32, height: f32, requested: usize) -> Option<(usize, f32, f32)> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let gap = (height * 0.2).clamp(1.0, 3.0);
    let max_count = ((width + gap) / (gap + 2.0)).floor() as usize;
    let count = requested.min(128).min(max_count);
    if count < 2 {
        return None;
    }
    let segment_width = (width - gap * (count - 1) as f32) / count as f32;
    (segment_width > 0.0).then_some((count, gap, segment_width))
}

fn paint_fill_piece(
    paint: &mut PaintEntityContext<'_>,
    progress: &Progress,
    piece: ClipRect,
    entire_fill: ClipRect,
    gradient_colors: Option<&[Rgba]>,
) {
    let radius =
        progress
            .style
            .corners
            .radius(piece.width, piece.height, paint.theme.corner_radius);
    let primitive = match (&progress.style.fill, gradient_colors) {
        (ProgressFill::ThemeAccent, _) => Primitive::RoundedRect {
            rect: piece,
            radius,
            color: paint.theme.accent,
        },
        (ProgressFill::Solid(color), _) => Primitive::RoundedRect {
            rect: piece,
            radius,
            color: color.resolve(paint.theme),
        },
        (ProgressFill::Gradient(_), Some(colors)) => {
            let start = ((piece.x - entire_fill.x) / entire_fill.width).clamp(0.0, 1.0);
            let end = ((piece.x + piece.width - entire_fill.x) / entire_fill.width).clamp(0.0, 1.0);
            let stops = palette_stops(colors, start, end, progress.palette_phase);
            Primitive::RoundedLinearGradient {
                rect: piece,
                radius,
                stops,
            }
        }
        (ProgressFill::Gradient(colors), None) => Primitive::RoundedRect {
            rect: piece,
            radius,
            color: colors
                .first()
                .map_or(paint.theme.accent, |color| color.resolve(paint.theme)),
        },
    };
    paint
        .output
        .push(item(paint.clip, paint.z.saturating_add(2), primitive));
}

fn palette_color(colors: &[Rgba], coordinate: f32) -> Rgba {
    let span = colors.len() as f32;
    let coordinate = coordinate.rem_euclid(span);
    let index = coordinate.floor() as usize;
    let next = (index + 1) % colors.len();
    colors[index].mix(colors[next], coordinate.fract())
}

fn palette_stops(colors: &[Rgba], start: f32, end: f32, phase: f32) -> Vec<(f32, Rgba)> {
    let span = colors.len() as f32;
    let phase = if phase.is_finite() {
        phase.rem_euclid(1.0)
    } else {
        0.0
    };
    let from = start * (span - 1.0) + phase * span;
    let to = end * (span - 1.0) + phase * span;
    let mut stops = vec![(0.0, palette_color(colors, from))];
    let mut boundary = from.floor() + 1.0;
    while boundary < to {
        let position = (boundary - from) / (to - from);
        stops.push((position, palette_color(colors, boundary)));
        boundary += 1.0;
    }
    stops.push((1.0, palette_color(colors, to)));
    stops
}

#[cfg(test)]
mod tests {
    use super::{palette_stops, segment_layout};
    use bbui_style::Rgba;

    #[test]
    fn dense_segments_fit_or_fall_back_to_continuous_fill() {
        let (count, _, width) = segment_layout(300.0, 24.0, 128).unwrap();
        assert!(count < 128 && width >= 2.0);
        assert!(segment_layout(4.0, 24.0, 128).is_none());
    }

    #[test]
    fn palette_stops_follow_color_anchors_without_sampling_steps() {
        let colors = [
            Rgba(1.0, 0.0, 0.0, 1.0),
            Rgba(0.0, 1.0, 0.0, 1.0),
            Rgba(0.0, 0.0, 1.0, 1.0),
            Rgba(1.0, 0.0, 1.0, 1.0),
        ];
        let stops = palette_stops(&colors, 0.0, 1.0, 0.125);
        assert_eq!(stops.len(), 5);
        assert_eq!(stops[1], (1.0 / 6.0, colors[1]));
        assert_eq!(stops[2], (0.5, colors[2]));
        assert_eq!(stops[3], (5.0 / 6.0, colors[3]));
        assert_eq!(stops[0].1, colors[0].mix(colors[1], 0.5));
        assert_eq!(stops[4].1, colors[3].mix(colors[0], 0.5));
        assert_eq!(
            palette_stops(&colors, 0.0, 1.0, 1.0),
            palette_stops(&colors, 0.0, 1.0, 0.0)
        );
    }
}

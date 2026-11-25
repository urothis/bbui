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
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let radius = paint.theme.corner_radius.min(rect.height * 0.5);
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
    if let Some(glow) = &progress.style.fill_glow {
        push_effect_glow(
            paint.output,
            paint.layout.clip.map(ClipRect::from),
            paint.z.saturating_add(1),
            fill_rect,
            radius,
            glow,
            paint.theme,
        );
    }

    let gradient_colors = match &progress.style.fill {
        ProgressFill::Gradient(colors) if colors.len() >= 2 => Some(
            colors
                .iter()
                .map(|color| color.resolve(paint.theme))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    };

    match progress.style.segments {
        Some(count) if count >= 2 => {
            let count = count.min(128);
            let gap = (rect.height * 0.2).clamp(1.0, 3.0);
            let width = (rect.width - gap * (count - 1) as f32) / count as f32;
            if width > 0.0 {
                for index in 0..count {
                    let segment_start = rect.x + index as f32 * (width + gap);
                    let left = segment_start.max(start);
                    let right = (segment_start + width).min(end);
                    if right > left {
                        paint_fill_piece(
                            paint,
                            &progress,
                            ClipRect {
                                x: left,
                                width: right - left,
                                ..rect
                            },
                            fill_rect,
                            gradient_colors.as_deref(),
                        );
                    }
                }
            }
        }
        _ => paint_fill_piece(
            paint,
            &progress,
            fill_rect,
            fill_rect,
            gradient_colors.as_deref(),
        ),
    }
    if let Some(border) = &progress.style.fill_border {
        push_effect_border(
            paint.output,
            paint.clip,
            paint.z.saturating_add(3),
            fill_rect,
            border,
            paint.theme,
        );
    }
}

fn paint_fill_piece(
    paint: &mut PaintEntityContext<'_>,
    progress: &Progress,
    piece: ClipRect,
    entire_fill: ClipRect,
    gradient_colors: Option<&[Rgba]>,
) {
    let radius = paint.theme.corner_radius.min(piece.height * 0.5);
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
            let steps = if progress.style.segments.is_some() {
                8
            } else {
                32
            };
            let stops = (0..=steps)
                .map(|index| {
                    let x = piece.x + piece.width * index as f32 / steps as f32;
                    let position = ((x - entire_fill.x) / entire_fill.width).clamp(0.0, 1.0);
                    (
                        index as f32 / steps as f32,
                        palette_color(colors, position, progress.palette_phase),
                    )
                })
                .collect();
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

fn palette_color(colors: &[Rgba], position: f32, phase: f32) -> Rgba {
    let span = colors.len() as f32;
    let phase = if phase.is_finite() { phase } else { 0.0 };
    let coordinate = ((position * (span - 1.0)) + phase * span).rem_euclid(span);
    let index = coordinate.floor() as usize;
    let next = (index + 1) % colors.len();
    colors[index].mix(colors[next], coordinate.fract())
}

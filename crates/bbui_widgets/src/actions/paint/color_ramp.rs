use super::*;

pub(in crate::actions) struct ColorRampPaint<'a> {
    pub(in crate::actions) ramp: &'a ColorRamp,
    pub(in crate::actions) state: ColorRampEditorState,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) theme: &'a Theme,
}

pub(in crate::actions) fn paint_color_ramp(paint: ColorRampPaint<'_>, output: &mut Vec<DrawItem>) {
    let ColorRampPaint {
        ramp,
        state,
        label,
        rect,
        clip,
        z,
        theme,
    } = paint;
    let spacing = theme.spacing.color_ramp;
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    let mode = match ramp.interpolation {
        Interpolation::Constant => "Constant",
        Interpolation::Linear => "Linear",
        Interpolation::Smooth => "Smooth",
    };
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.title_x, rect.y + spacing.title_y],
            format!("{label} · {mode}"),
            theme.text,
            theme,
            false,
        ),
    ));
    for (index, (title, color)) in [
        ("+", theme.accent),
        ("−", theme.surface_hover),
        (mode, theme.surface_hover),
    ]
    .into_iter()
    .enumerate()
    {
        let width = spacing.button_width;
        let button_rect = ClipRect {
            x: rect.x + rect.width - spacing.button_right_inset
                + index as f32 * spacing.button_step,
            y: rect.y + spacing.button_top_inset,
            width,
            height: spacing.button_height,
        };
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: button_rect,
                radius: 3.0,
                color,
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    button_rect.x + spacing.button_label_x,
                    button_rect.y + spacing.button_label_y,
                ],
                title.to_owned(),
                theme.text,
                theme,
                false,
            ),
        ));
    }

    let track = color_ramp_track_rect(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect::from(track),
            radius: 2.0,
            color: theme.background,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::LinearGradient {
            rect: ClipRect::from(track),
            stops: color_ramp_gradient_stops(ramp),
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(3),
        Primitive::Line {
            from: [track.x, track.y],
            to: [track.x + track.width, track.y],
            width: 1.0,
            color: theme.border,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(3),
        Primitive::Line {
            from: [track.x, track.y + track.height],
            to: [track.x + track.width, track.y + track.height],
            width: 1.0,
            color: theme.border,
        },
    ));
    for stop in &ramp.stops {
        let x = track.x + stop.position * track.width;
        output.push(item(
            clip,
            z.saturating_add(4),
            Primitive::Line {
                from: [x, track.y + track.height],
                to: [x, track.y + track.height + spacing.marker_line_extension],
                width: if ramp.selected == Some(stop.id) {
                    2.0
                } else {
                    1.0
                },
                color: if ramp.selected == Some(stop.id) {
                    theme.accent
                } else {
                    theme.border
                },
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(5),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: x - spacing.marker_handle_radius,
                    y: track.y + track.height + spacing.marker_handle_top_inset,
                    width: spacing.marker_handle_radius * 2.0,
                    height: spacing.marker_handle_radius * 2.0,
                },
                radius: spacing.marker_handle_radius * 0.4,
                color: if ramp.selected == Some(stop.id) {
                    theme.accent
                } else {
                    stop.color
                },
            },
        ));
    }

    let Some(selected) = ramp.selected_stop() else {
        return;
    };
    let channels = [
        selected.color.0,
        selected.color.1,
        selected.color.2,
        selected.color.3,
    ];
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.details_x, rect.y + spacing.details_y],
            format!(
                "Stop {:.3}   RGBA {:.2}, {:.2}, {:.2}, {:.2}",
                selected.position, channels[0], channels[1], channels[2], channels[3]
            ),
            theme.text_muted,
            theme,
            false,
        ),
    ));
    for (index, value) in channels.into_iter().enumerate() {
        let channel = ColorChannel::from_index(index);
        let row = color_ramp_channel_rect(rect, index, spacing);
        output.push(item(
            clip,
            z.saturating_add(3),
            styled_text_primitive(
                clip,
                [
                    rect.x + spacing.channel_label_x,
                    row.y + spacing.channel_label_y,
                ],
                channel.label().to_owned(),
                if state.active_channel == channel {
                    theme.text
                } else {
                    theme.text_muted
                },
                theme,
                false,
            ),
        ));
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::LinearGradient {
                rect: ClipRect::from(row),
                stops: vec![
                    (0.0, color_ramp_channel_color(selected.color, channel, 0.0)),
                    (1.0, color_ramp_channel_color(selected.color, channel, 1.0)),
                ],
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(4),
            Primitive::Line {
                from: [
                    row.x + row.width * value,
                    row.y - spacing.channel_label_y - 1.0,
                ],
                to: [
                    row.x + row.width * value,
                    row.y + row.height + spacing.channel_label_y + 1.0,
                ],
                width: if state.active_channel == channel {
                    2.0
                } else {
                    1.0
                },
                color: if state.active_channel == channel {
                    theme.accent
                } else {
                    theme.text
                },
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(3),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width - spacing.channel_value_right_inset,
                    row.y + spacing.channel_label_y,
                ],
                format!("{value:.2}"),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    }
    if state.editing_color {
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width - spacing.editing_right_inset,
                    rect.y + rect.height - spacing.editing_bottom_inset,
                ],
                "Editing color · arrows adjust".to_owned(),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    }
}

fn color_ramp_gradient_stops(ramp: &ColorRamp) -> Vec<(f32, Rgba)> {
    let mut stops = Vec::new();
    let Some(first) = ramp.stops.first() else {
        return vec![
            (0.0, Rgba(0.0, 0.0, 0.0, 1.0)),
            (1.0, Rgba(0.0, 0.0, 0.0, 1.0)),
        ];
    };
    stops.push((0.0, first.color));
    for pair in ramp.stops.windows(2) {
        let left = pair[0];
        let right = pair[1];
        match ramp.interpolation {
            Interpolation::Linear => stops.push((right.position, right.color)),
            Interpolation::Constant => {
                stops.push((right.position, left.color));
                stops.push((right.position, right.color));
            }
            Interpolation::Smooth => {
                for step in 1..=16 {
                    let position =
                        left.position + (right.position - left.position) * step as f32 / 16.0;
                    stops.push((position, ramp.sample(position)));
                }
            }
        }
    }
    if let Some(last) = ramp.stops.last() {
        stops.push((1.0, last.color));
    }
    stops
}

pub(in crate::actions) fn color_ramp_channel_color(
    color: Rgba,
    channel: ColorChannel,
    value: f32,
) -> Rgba {
    match channel {
        ColorChannel::Red => Rgba(value, color.1, color.2, color.3),
        ColorChannel::Green => Rgba(color.0, value, color.2, color.3),
        ColorChannel::Blue => Rgba(color.0, color.1, value, color.3),
        ColorChannel::Alpha => Rgba(color.0, color.1, color.2, value),
    }
}

use super::*;

pub(in crate::actions) struct SliderPaint<'a> {
    pub(in crate::actions) slider: &'a Slider,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) enabled: bool,
    pub(in crate::actions) focused: bool,
    pub(in crate::actions) dragging: bool,
    pub(in crate::actions) filled: bool,
    pub(in crate::actions) value_width: Option<f32>,
    pub(in crate::actions) theme: &'a Theme,
}

pub(in crate::actions) fn paint_slider(paint: SliderPaint<'_>, output: &mut Vec<DrawItem>) {
    let SliderPaint {
        slider,
        label,
        rect,
        clip,
        z,
        enabled,
        focused,
        dragging,
        filled,
        value_width,
        theme,
    } = paint;
    if filled {
        let track = ClipRect::from(rect);
        output.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: track,
                radius: 2.0,
                color: theme.surface.mix(theme.text, 0.12),
            },
        ));
        let fill_width = rect.width * slider.normalized_value() as f32;
        if fill_width > 0.0 {
            output.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        width: fill_width,
                        ..track
                    },
                    radius: 2.0,
                    color: theme.accent,
                },
            ));
        }
        let value = format!("{:.3}", slider.value);
        let value_width = value_width.unwrap_or(0.0);
        let value_text = styled_text_primitive(
            clip,
            [
                rect.x + (rect.width - value_width).max(0.0) * 0.5,
                rect.y + theme.spacing.control.padding_y,
            ],
            value,
            theme.text,
            theme,
            false,
        );
        output.push(item(clip, z.saturating_add(2), value_text));
        return;
    }
    let active = focused || dragging;
    let radius = theme.spacing.editor.handle_radius;
    let radius = if radius.is_finite() {
        radius.clamp(3.0, 9.0)
    } else {
        6.0
    };
    let (left, right) = slider_track_x(rect, theme);
    let center_y = rect.y + rect.height - theme.spacing.control.padding_y.max(0.0) - radius;
    let position = slider.normalized_value() as f32;
    let handle_x = left + (right - left) * position;
    let track_height = theme.spacing.control.slider_track_height.max(2.0);
    let track_rect = ClipRect {
        x: left,
        y: center_y - track_height * 0.5,
        width: (right - left).max(0.0),
        height: track_height,
    };
    let background = if !enabled {
        theme.color(ThemeColorToken::ControlDisabledSurface)
    } else if active {
        theme.color(ThemeColorToken::EditorActiveFieldSurface)
    } else {
        theme.surface
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: background,
        },
    ));
    let text = if label.is_empty() {
        slider.display()
    } else {
        format!("{label}: {}", slider.display())
    };
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.control.padding_x,
                rect.y + theme.spacing.control.padding_y,
            ],
            text,
            if enabled {
                theme.text
            } else {
                theme.color(ThemeColorToken::ControlDisabledText)
            },
            theme,
            false,
        ),
    ));
    let track_color = if enabled {
        theme.border
    } else {
        theme.color(ThemeColorToken::ControlDisabledMark)
    };
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: track_rect,
            radius: track_height * 0.5,
            color: track_color,
        },
    ));
    let fill_width = (handle_x - left).max(0.0);
    if fill_width > 0.0 {
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    width: fill_width,
                    ..track_rect
                },
                radius: track_height * 0.5,
                color: if enabled {
                    theme.accent
                } else {
                    theme.color(ThemeColorToken::ControlDisabledMark)
                },
            },
        ));
    }
    if active && enabled {
        output.push(item(
            clip,
            z.saturating_add(3),
            Primitive::Ellipse {
                rect: ClipRect {
                    x: handle_x - radius - 2.0,
                    y: center_y - radius - 2.0,
                    width: (radius + 2.0) * 2.0,
                    height: (radius + 2.0) * 2.0,
                },
                color: theme.focus,
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(4),
        Primitive::Ellipse {
            rect: ClipRect {
                x: handle_x - radius,
                y: center_y - radius,
                width: radius * 2.0,
                height: radius * 2.0,
            },
            color: if enabled {
                theme.accent
            } else {
                theme.color(ThemeColorToken::ControlDisabledMark)
            },
        },
    ));
}

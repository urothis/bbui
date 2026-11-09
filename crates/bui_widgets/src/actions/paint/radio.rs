use super::*;

pub(in crate::actions) struct RadioGroupPaint<'a> {
    pub(in crate::actions) group: &'a RadioGroup,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) enabled: bool,
    pub(in crate::actions) theme: &'a Theme,
}

pub(in crate::actions) fn paint_radio_group(
    paint: RadioGroupPaint<'_>,
    output: &mut Vec<DrawItem>,
) {
    let RadioGroupPaint {
        group,
        label,
        rect,
        clip,
        z,
        enabled,
        theme,
    } = paint;
    let control_height = theme.control_height.max(1.0);
    let label_y = rect.y + (control_height - theme.font_size * theme.line_height).max(0.0) * 0.5;
    output.push(item(
        clip,
        z,
        styled_text_primitive(
            clip,
            [rect.x + theme.spacing.control.padding_x, label_y],
            label.to_owned(),
            if enabled && group.enabled {
                theme.text
            } else {
                theme.color(ThemeColorToken::ControlDisabledText)
            },
            theme,
            false,
        ),
    ));
    for (index, option) in group.options.iter().enumerate() {
        let row_y = rect.y + control_height * (index as f32 + 1.0);
        if row_y >= rect.y + rect.height {
            break;
        }
        let option_enabled = enabled && group.enabled && option.enabled;
        let diameter = (control_height - 8.0).clamp(8.0, 16.0);
        let circle = Rect {
            x: rect.x + theme.spacing.control.padding_x,
            y: row_y + (control_height - diameter) * 0.5,
            width: diameter,
            height: diameter,
        };
        let color = if option_enabled {
            theme.border
        } else {
            theme.color(ThemeColorToken::ControlDisabledMark)
        };
        output.push(item(
            clip,
            z.saturating_add(1 + index as i32 * 2),
            Primitive::Ellipse {
                rect: ClipRect::from(circle),
                color,
            },
        ));
        if group.selected == Some(index) {
            let inset = (diameter * 0.28).max(2.0);
            output.push(item(
                clip,
                z.saturating_add(2 + index as i32 * 2),
                Primitive::Ellipse {
                    rect: ClipRect::from(circle.inset(inset, inset, inset, inset)),
                    color: if option_enabled {
                        theme.accent
                    } else {
                        theme.color(ThemeColorToken::ControlDisabledMark)
                    },
                },
            ));
        }
        output.push(item(
            clip,
            z.saturating_add(2 + index as i32 * 2),
            styled_text_primitive(
                clip,
                [
                    circle.x + circle.width + theme.spacing.control.checkbox_label_gap,
                    row_y + (control_height - theme.font_size * theme.line_height).max(0.0) * 0.5,
                ],
                option.label.clone(),
                if option_enabled {
                    theme.text
                } else {
                    theme.color(ThemeColorToken::ControlDisabledText)
                },
                theme,
                false,
            ),
        ));
    }
}

use super::*;

pub(in crate::actions) struct HsvColorPaint<'a> {
    pub(in crate::actions) editor: HsvColorEditor,
    pub(in crate::actions) state: HsvColorEditorState,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) theme: &'a Theme,
}

pub(in crate::actions) fn paint_hsv_color_editor(
    paint: HsvColorPaint<'_>,
    output: &mut Vec<DrawItem>,
) {
    let HsvColorPaint {
        editor,
        state,
        label,
        rect,
        clip,
        z,
        theme,
    } = paint;
    let spacing = theme.spacing.hsv;
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.editor.padding_x,
                rect.y + theme.spacing.editor.padding_y,
            ],
            label.to_owned(),
            theme.text,
            theme,
            false,
        ),
    ));
    let preview = ClipRect {
        x: rect.x + rect.width - spacing.preview_right_inset,
        y: rect.y + spacing.preview_top_inset,
        width: spacing.preview_width,
        height: spacing.preview_height,
    };
    for index in 0..4 {
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: preview.x + index as f32 * 9.5,
                    y: preview.y,
                    width: 10.0,
                    height: preview.height,
                },
                radius: 1.0,
                color: if index % 2 == 0 {
                    theme.background
                } else {
                    theme.surface_hover
                },
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::RoundedRect {
            rect: preview,
            radius: 2.0,
            color: editor.color(),
        },
    ));

    let (center, outer_radius, _) = hsv_wheel_geometry(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::HsvWheel {
            rect: ClipRect {
                x: center[0] - outer_radius,
                y: center[1] - outer_radius,
                width: outer_radius * 2.0,
                height: outer_radius * 2.0,
            },
            hue_degrees: editor.hue_degrees,
            saturation: editor.saturation,
            value: editor.value,
        },
    ));

    let value_rect = hsv_value_rect(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.controls_x, rect.y + spacing.value_label_y],
            "Value".to_owned(),
            theme.text_muted,
            theme,
            false,
        ),
    ));
    const CONTROL_SEGMENTS: usize = 32;
    for segment in 0..CONTROL_SEGMENTS {
        let value = (segment as f32 + 0.5) / CONTROL_SEGMENTS as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: value_rect.x + value_rect.width * segment as f32 / CONTROL_SEGMENTS as f32,
                    y: value_rect.y,
                    width: value_rect.width / CONTROL_SEGMENTS as f32 + 0.6,
                    height: value_rect.height,
                },
                radius: if segment == 0 || segment + 1 == CONTROL_SEGMENTS {
                    3.0
                } else {
                    0.0
                },
                color: crate::hsv_to_rgb([editor.hue_degrees, editor.saturation, value], 1.0),
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(4),
        Primitive::Line {
            from: [
                value_rect.x + value_rect.width * editor.value,
                value_rect.y - 2.0,
            ],
            to: [
                value_rect.x + value_rect.width * editor.value,
                value_rect.y + value_rect.height + 2.0,
            ],
            width: 2.0,
            color: theme.text,
        },
    ));
    let alpha_rect = hsv_alpha_rect(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.controls_x, rect.y + spacing.alpha_label_y],
            "Alpha".to_owned(),
            theme.text_muted,
            theme,
            false,
        ),
    ));
    for cell in 0..8 {
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: alpha_rect.x + alpha_rect.width * cell as f32 / 8.0,
                    y: alpha_rect.y,
                    width: alpha_rect.width / 8.0 + 0.5,
                    height: alpha_rect.height,
                },
                radius: 0.0,
                color: if cell % 2 == 0 {
                    theme.background
                } else {
                    theme.surface_hover
                },
            },
        ));
    }
    let color = editor.color();
    for segment in 0..CONTROL_SEGMENTS {
        let alpha = (segment as f32 + 0.5) / CONTROL_SEGMENTS as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: alpha_rect.x + alpha_rect.width * segment as f32 / CONTROL_SEGMENTS as f32,
                    y: alpha_rect.y,
                    width: alpha_rect.width / CONTROL_SEGMENTS as f32 + 0.6,
                    height: alpha_rect.height,
                },
                radius: 0.0,
                color: Rgba(color.0, color.1, color.2, alpha),
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(4),
        Primitive::Line {
            from: [
                alpha_rect.x + alpha_rect.width * editor.alpha,
                alpha_rect.y - 2.0,
            ],
            to: [
                alpha_rect.x + alpha_rect.width * editor.alpha,
                alpha_rect.y + alpha_rect.height + 2.0,
            ],
            width: 2.0,
            color: theme.text,
        },
    ));

    for field in HsvColorField::ALL {
        let field_rect = hsv_field_rect(rect, field, spacing);
        let active = state.active_field == field;
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect::from(field_rect),
                radius: 3.0,
                color: if active {
                    theme.color(ThemeColorToken::EditorActiveFieldSurface)
                } else {
                    theme.background
                },
            },
        ));
        for (from, to) in [
            (
                [field_rect.x, field_rect.y],
                [field_rect.x + field_rect.width, field_rect.y],
            ),
            (
                [field_rect.x, field_rect.y + field_rect.height],
                [
                    field_rect.x + field_rect.width,
                    field_rect.y + field_rect.height,
                ],
            ),
        ] {
            output.push(item(
                clip,
                z.saturating_add(2),
                Primitive::Line {
                    from,
                    to,
                    width: if active { 1.5 } else { 0.75 },
                    color: if active { theme.accent } else { theme.border },
                },
            ));
        }
        let text = if active && state.editing {
            if field == HsvColorField::Hex {
                state.edit_buffer.clone()
            } else {
                format!("{}: {}", field.label(), state.edit_buffer)
            }
        } else {
            hsv_field_text(editor, field)
        };
        output.push(item(
            clip,
            z.saturating_add(3),
            styled_text_primitive(
                clip,
                [
                    field_rect.x + spacing.field_text_inset_x,
                    field_rect.y + spacing.field_text_inset_y,
                ],
                text,
                if active { theme.text } else { theme.text_muted },
                theme,
                false,
            ),
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.controls_x, rect.y + spacing.help_y],
            "Enter edit · arrows adjust · Home/End set limits".to_owned(),
            theme.text_muted,
            theme,
            false,
        ),
    ));
}

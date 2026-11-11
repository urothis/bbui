use super::*;

pub(in crate::actions) struct CurveEditorPaint<'a> {
    pub(in crate::actions) editor: &'a CurveEditor,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) theme: &'a Theme,
}

pub(in crate::actions) fn paint_curve_editor(
    paint: CurveEditorPaint<'_>,
    output: &mut Vec<DrawItem>,
) {
    let CurveEditorPaint {
        editor,
        label,
        rect,
        clip,
        z,
        theme,
    } = paint;
    let spacing = theme.spacing.editor;
    let curve_spacing = theme.spacing.curve;
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
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [rect.x + spacing.padding_x, rect.y + spacing.padding_y],
            label.to_owned(),
            theme.text,
            theme,
            false,
        ),
    ));
    if let Some(point) = editor
        .selected
        .and_then(|id| editor.points.iter().find(|point| point.id == id))
    {
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width - curve_spacing.selected_status_width - spacing.padding_x,
                    rect.y + spacing.padding_y,
                ],
                format!(
                    "x {:.3}   y {:.3}   {:?}",
                    point.position[0], point.position[1], editor.interpolation
                ),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    } else {
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width
                        - curve_spacing.interpolation_status_width
                        - spacing.padding_x,
                    rect.y + spacing.padding_y,
                ],
                format!("{:?}", editor.interpolation),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    }
    let plot = curve_plot_rect(rect, theme);
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect::from(plot),
            radius: 2.0,
            color: theme.color(ThemeColorToken::CurvePlotSurface),
        },
    ));
    let x_grid_steps = (plot.width / spacing.grid_spacing.max(1.0))
        .ceil()
        .clamp(2.0, 8.0) as usize;
    let y_grid_steps = (plot.height / spacing.grid_spacing.max(1.0))
        .ceil()
        .clamp(2.0, 8.0) as usize;
    let grid = theme.color(ThemeColorToken::CurveGridLine);
    for index in 0..=x_grid_steps {
        let x = plot.x + plot.width * index as f32 / x_grid_steps as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [x, plot.y],
                to: [x, plot.y + plot.height],
                width: 1.0,
                color: grid,
            },
        ));
    }
    for index in 0..=y_grid_steps {
        let y = plot.y + plot.height * index as f32 / y_grid_steps as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [plot.x, y],
                to: [plot.x + plot.width, y],
                width: 1.0,
                color: grid,
            },
        ));
    }
    let axis = theme.color(ThemeColorToken::CurveAxisLine);
    for pair in editor.points.windows(2) {
        let left = pair[0];
        let right = pair[1];
        let p0 = curve_screen_position(editor, plot, left.position);
        let p3 = curve_screen_position(editor, plot, right.position);
        match editor.interpolation {
            Interpolation::Constant => {
                let corner =
                    curve_screen_position(editor, plot, [right.position[0], left.position[1]]);
                output.push(item(
                    clip,
                    z.saturating_add(3),
                    Primitive::Line {
                        from: p0,
                        to: corner,
                        width: 2.0,
                        color: theme.accent,
                    },
                ));
                output.push(item(
                    clip,
                    z.saturating_add(3),
                    Primitive::Line {
                        from: corner,
                        to: p3,
                        width: 2.0,
                        color: theme.accent,
                    },
                ));
            }
            Interpolation::Linear => output.push(item(
                clip,
                z.saturating_add(3),
                Primitive::Line {
                    from: p0,
                    to: p3,
                    width: 2.0,
                    color: theme.accent,
                },
            )),
            Interpolation::Smooth => {
                let c1 = curve_screen_position(
                    editor,
                    plot,
                    [
                        left.position[0] + left.right_handle[0],
                        left.position[1] + left.right_handle[1],
                    ],
                );
                let c2 = curve_screen_position(
                    editor,
                    plot,
                    [
                        right.position[0] + right.left_handle[0],
                        right.position[1] + right.left_handle[1],
                    ],
                );
                output.push(item(
                    clip,
                    z.saturating_add(3),
                    Primitive::Bezier {
                        points: [p0, c1, c2, p3],
                        width: 2.0,
                        color: theme.accent,
                    },
                ));
            }
        }
    }
    for point in &editor.points {
        let center = curve_screen_position(editor, plot, point.position);
        let selected = editor.selected == Some(point.id);
        if selected && editor.interpolation == Interpolation::Smooth {
            for handle in [point.left_handle, point.right_handle] {
                let end = curve_screen_position(
                    editor,
                    plot,
                    [point.position[0] + handle[0], point.position[1] + handle[1]],
                );
                output.push(item(
                    clip,
                    z.saturating_add(4),
                    Primitive::Line {
                        from: center,
                        to: end,
                        width: 1.0,
                        color: theme.text_muted,
                    },
                ));
                output.push(item(
                    clip,
                    z.saturating_add(5),
                    Primitive::RoundedRect {
                        rect: ClipRect {
                            x: end[0] - spacing.handle_radius,
                            y: end[1] - spacing.handle_radius,
                            width: spacing.handle_radius * 2.0,
                            height: spacing.handle_radius * 2.0,
                        },
                        radius: spacing.handle_radius,
                        color: theme.text_muted,
                    },
                ));
            }
        }
        let radius = spacing.handle_radius * if selected { 1.5 } else { 1.0 };
        output.push(item(
            clip,
            z.saturating_add(6),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: center[0] - radius,
                    y: center[1] - radius,
                    width: radius * 2.0,
                    height: radius * 2.0,
                },
                radius,
                color: if selected { theme.accent } else { theme.text },
            },
        ));
    }
    // The plot border is drawn last to distinguish the editable viewport edge from its grid.
    output.push(item(
        clip,
        z.saturating_add(7),
        Primitive::Line {
            from: [plot.x, plot.y],
            to: [plot.x + plot.width, plot.y],
            width: 1.0,
            color: axis,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(7),
        Primitive::Line {
            from: [plot.x, plot.y + plot.height],
            to: [plot.x + plot.width, plot.y + plot.height],
            width: 1.0,
            color: axis,
        },
    ));
}

use super::*;

pub(super) fn hsv_spacing(world: &World) -> bbui_style::ThemeHsvSpacing {
    world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().spacing.hsv, |theme| theme.spacing.hsv)
}

pub(super) fn hsv_wheel_geometry(
    rect: Rect,
    spacing: bbui_style::ThemeHsvSpacing,
) -> ([f32; 2], f32, f32) {
    (
        [
            rect.x + spacing.wheel_center_x,
            rect.y + spacing.wheel_center_y,
        ],
        spacing.wheel_outer_radius,
        spacing.wheel_inner_radius,
    )
}

pub(super) fn hsv_value_rect(rect: Rect, spacing: bbui_style::ThemeHsvSpacing) -> Rect {
    Rect {
        x: rect.x + spacing.controls_x,
        y: rect.y + spacing.value_track_y,
        width: (rect.width - spacing.value_track_width_inset).max(1.0),
        height: spacing.value_track_height,
    }
}

pub(super) fn hsv_alpha_rect(rect: Rect, spacing: bbui_style::ThemeHsvSpacing) -> Rect {
    Rect {
        x: rect.x + spacing.controls_x,
        y: rect.y + spacing.alpha_track_y,
        width: (rect.width - spacing.alpha_track_width_inset).max(1.0),
        height: spacing.value_track_height,
    }
}

pub(super) fn hsv_field_rect(
    rect: Rect,
    field: HsvColorField,
    spacing: bbui_style::ThemeHsvSpacing,
) -> Rect {
    let origin_x = rect.x + spacing.controls_x;
    let cell_width = spacing.field_cell_width;
    let gap = spacing.field_gap;
    match field {
        HsvColorField::Hue => Rect {
            x: origin_x,
            y: rect.y + spacing.field_first_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Saturation => Rect {
            x: origin_x + cell_width + gap,
            y: rect.y + spacing.field_first_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Value => Rect {
            x: origin_x + (cell_width + gap) * 2.0,
            y: rect.y + spacing.field_first_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Red => Rect {
            x: origin_x,
            y: rect.y + spacing.field_second_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Green => Rect {
            x: origin_x + cell_width + gap,
            y: rect.y + spacing.field_second_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Blue => Rect {
            x: origin_x + (cell_width + gap) * 2.0,
            y: rect.y + spacing.field_second_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Alpha => Rect {
            x: rect.x + rect.width - spacing.alpha_field_right_inset,
            y: rect.y + spacing.alpha_field_y,
            width: spacing.alpha_field_width,
            height: spacing.field_height,
        },
        HsvColorField::Hex => Rect {
            x: origin_x,
            y: rect.y + spacing.field_hex_y,
            width: (rect.width - spacing.field_hex_width_inset).max(1.0),
            height: spacing.field_height,
        },
    }
}

pub(super) fn hsv_field_text(editor: HsvColorEditor, field: HsvColorField) -> String {
    match field {
        HsvColorField::Hue => format!("H: {}°", hsv_field_input_text(editor, field)),
        HsvColorField::Saturation => format!("S: {}%", hsv_field_input_text(editor, field)),
        HsvColorField::Value => format!("V: {}%", hsv_field_input_text(editor, field)),
        HsvColorField::Red => format!("R: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Green => format!("G: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Blue => format!("B: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Alpha => format!("A: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Hex => hsv_field_input_text(editor, field),
    }
}

pub(super) fn hsv_field_input_text(editor: HsvColorEditor, field: HsvColorField) -> String {
    match field {
        HsvColorField::Hue => format!("{:.0}", editor.hue_degrees),
        HsvColorField::Saturation => format!("{:.0}", editor.saturation * 100.0),
        HsvColorField::Value => format!("{:.0}", editor.value * 100.0),
        HsvColorField::Red => format!("{:.0}", editor.color().0 * 255.0),
        HsvColorField::Green => format!("{:.0}", editor.color().1 * 255.0),
        HsvColorField::Blue => format!("{:.0}", editor.color().2 * 255.0),
        HsvColorField::Alpha => format!("{:.0}", editor.alpha * 255.0),
        HsvColorField::Hex => editor.hex(),
    }
}

pub(super) fn hsv_numeric_field_value(editor: HsvColorEditor, field: HsvColorField) -> Option<f32> {
    let color = editor.color();
    match field {
        HsvColorField::Hue => Some(editor.hue_degrees),
        HsvColorField::Saturation => Some(editor.saturation * 100.0),
        HsvColorField::Value => Some(editor.value * 100.0),
        HsvColorField::Red => Some(color.0 * 255.0),
        HsvColorField::Green => Some(color.1 * 255.0),
        HsvColorField::Blue => Some(color.2 * 255.0),
        HsvColorField::Alpha => Some(editor.alpha * 255.0),
        HsvColorField::Hex => None,
    }
}

pub(super) fn set_hsv_numeric_field(
    editor: &mut HsvColorEditor,
    field: HsvColorField,
    text: &str,
) -> bool {
    if field == HsvColorField::Hex {
        return editor.set_hex(text).unwrap_or(false);
    }
    let Ok(value) = text
        .trim()
        .trim_end_matches('%')
        .trim_end_matches('°')
        .parse::<f32>()
    else {
        return false;
    };
    match field {
        HsvColorField::Hue => editor.set_hsv(field, value),
        HsvColorField::Saturation | HsvColorField::Value => editor.set_hsv(field, value / 100.0),
        HsvColorField::Red | HsvColorField::Green | HsvColorField::Blue => {
            editor.set_rgb(field, value / 255.0)
        }
        HsvColorField::Alpha => editor.set_hsv(field, value / 255.0),
        HsvColorField::Hex => false,
    }
}

pub(super) fn edit_hsv_color(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut HsvColorEditor) -> bool,
) {
    let Some(before_editor) = world.get::<HsvColorEditor>(target).copied() else {
        return;
    };
    let Ok(before) = before_editor.to_json() else {
        return;
    };
    let changed = world
        .get_mut::<HsvColorEditor>(target)
        .is_some_and(|mut editor| edit(&mut editor));
    if !changed {
        return;
    }
    if let Some(editor) = world.get::<HsvColorEditor>(target)
        && let Ok(after) = editor.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::HsvColorCommitted {
            target,
            before,
            after,
        });
    }
}

pub(super) fn begin_hsv_color_pointer(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let (Some(editor), Some(layout)) = (
        world.get::<HsvColorEditor>(target).copied(),
        world.get::<UiLayoutResult>(target).copied(),
    ) else {
        return;
    };
    let rect = layout.rect;
    let spacing = hsv_spacing(world);
    let (center, outer_radius, _) = hsv_wheel_geometry(rect, spacing);
    let dx = position[0] - center[0];
    let dy = position[1] - center[1];
    if dx.hypot(dy) <= outer_radius + 2.0 {
        world
            .resource_mut::<WidgetInteractionState>()
            .hsv_color_drags
            .insert(
                window,
                HsvColorDrag {
                    target,
                    before: editor,
                    kind: HsvColorDragKind::Wheel,
                },
            );
        update_hsv_color_drag(world, window, position);
        return;
    }
    let value_rect = hsv_value_rect(rect, spacing);
    if value_rect.contains(position[0], position[1]) {
        world
            .resource_mut::<WidgetInteractionState>()
            .hsv_color_drags
            .insert(
                window,
                HsvColorDrag {
                    target,
                    before: editor,
                    kind: HsvColorDragKind::Value,
                },
            );
        update_hsv_color_drag(world, window, position);
        return;
    }
    let alpha_rect = hsv_alpha_rect(rect, spacing);
    if alpha_rect.contains(position[0], position[1]) {
        world
            .resource_mut::<WidgetInteractionState>()
            .hsv_color_drags
            .insert(
                window,
                HsvColorDrag {
                    target,
                    before: editor,
                    kind: HsvColorDragKind::Alpha,
                },
            );
        update_hsv_color_drag(world, window, position);
        return;
    }
    for field in HsvColorField::ALL {
        if hsv_field_rect(rect, field, spacing).contains(position[0], position[1])
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.active_field = field;
            state.editing = false;
            state.edit_buffer.clear();
            return;
        }
    }
}

pub(super) fn update_hsv_color_drag(world: &mut World, window: Entity, position: [f32; 2]) {
    let Some((target, kind)) = world
        .resource::<WidgetInteractionState>()
        .hsv_color_drags
        .get(&window)
        .map(|drag| (drag.target, drag.kind))
    else {
        return;
    };
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = hsv_spacing(world);
    if let Some(mut editor) = world.get_mut::<HsvColorEditor>(target) {
        match kind {
            HsvColorDragKind::Wheel => {
                let (center, _, inner_radius) = hsv_wheel_geometry(rect, spacing);
                let dx = position[0] - center[0];
                let dy = position[1] - center[1];
                let distance = dx.hypot(dy).min(inner_radius);
                let hue = dy.atan2(dx).to_degrees().rem_euclid(360.0);
                editor.set_hsv(HsvColorField::Hue, hue);
                if distance <= inner_radius {
                    editor.set_hsv(HsvColorField::Saturation, distance / inner_radius);
                }
            }
            HsvColorDragKind::Value => {
                editor.set_hsv(
                    HsvColorField::Value,
                    normalized_x(hsv_value_rect(rect, spacing), position[0]),
                );
            }
            HsvColorDragKind::Alpha => {
                editor.set_hsv(
                    HsvColorField::Alpha,
                    normalized_x(hsv_alpha_rect(rect, spacing), position[0]),
                );
            }
        }
    }
}

pub(super) fn finish_hsv_color_drag(
    world: &mut World,
    drag: HsvColorDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Some(mut editor) = world.get_mut::<HsvColorEditor>(drag.target) {
            *editor = drag.before;
        }
        return;
    }
    let (Ok(before), Some(editor)) = (
        drag.before.to_json(),
        world.get::<HsvColorEditor>(drag.target),
    ) else {
        return;
    };
    if let Ok(after) = editor.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::HsvColorCommitted {
            target: drag.target,
            before,
            after,
        });
    }
}

use super::*;

pub(super) fn handle_curve_editor_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    if (modifiers.control || modifiers.super_key)
        && matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
    {
        if let Some(mut editor) = world.get_mut::<CurveEditor>(target)
            && !editor.points.is_empty()
        {
            let current = editor
                .selected
                .and_then(|id| editor.points.iter().position(|point| point.id == id));
            let index = match (current, key) {
                (Some(index), UiKeyCode::ArrowRight) => (index + 1) % editor.points.len(),
                (Some(index), UiKeyCode::ArrowLeft) => {
                    (index + editor.points.len() - 1) % editor.points.len()
                }
                (None, UiKeyCode::ArrowRight) => 0,
                (None, UiKeyCode::ArrowLeft) => editor.points.len() - 1,
                _ => return,
            };
            editor.selected = Some(editor.points[index].id);
        }
        return;
    }
    match key {
        UiKeyCode::A => edit_curve_editor(world, target, emitted, add_curve_point_near_selection),
        UiKeyCode::Delete | UiKeyCode::Backspace => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.selected.is_some_and(|id| editor.remove_point(id))
            });
        }
        UiKeyCode::C => edit_curve_editor(world, target, emitted, |editor| {
            editor.interpolation = match editor.interpolation {
                Interpolation::Constant => Interpolation::Linear,
                Interpolation::Linear => Interpolation::Smooth,
                Interpolation::Smooth => Interpolation::Constant,
            };
            true
        }),
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowDown
        | UiKeyCode::Home
        | UiKeyCode::End => {
            let step = if modifiers.control || modifiers.super_key {
                0.001
            } else if modifiers.shift {
                0.1
            } else {
                0.01
            };
            edit_curve_editor(world, target, emitted, |editor| {
                let Some(point) = editor
                    .selected
                    .and_then(|id| editor.points.iter().find(|point| point.id == id))
                    .or_else(|| editor.points.first())
                    .copied()
                else {
                    return false;
                };
                editor.selected = Some(point.id);
                let mut position = point.position;
                match key {
                    UiKeyCode::ArrowLeft => position[0] -= step,
                    UiKeyCode::ArrowRight => position[0] += step,
                    UiKeyCode::ArrowUp => position[1] += step,
                    UiKeyCode::ArrowDown => position[1] -= step,
                    UiKeyCode::Home => position[0] = 0.0,
                    UiKeyCode::End => position[0] = 1.0,
                    _ => return false,
                }
                editor.move_point(point.id, position)
            });
        }
        _ => {}
    }
}

pub(super) fn add_curve_point_near_selection(editor: &mut CurveEditor) -> bool {
    let pair = editor
        .selected
        .and_then(|id| editor.points.iter().position(|point| point.id == id))
        .and_then(|index| {
            if index + 1 < editor.points.len() {
                Some((editor.points[index], editor.points[index + 1]))
            } else if index > 0 {
                Some((editor.points[index - 1], editor.points[index]))
            } else {
                None
            }
        })
        .or_else(|| {
            editor
                .points
                .first()
                .zip(editor.points.last())
                .map(|(a, b)| (*a, *b))
        });
    let Some((left, right)) = pair else {
        return false;
    };
    let x = (left.position[0] + right.position[0]) * 0.5;
    editor.add_point(x, editor.evaluate(x)).is_some()
}

pub(super) fn handle_hsv_color_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    let state = world
        .get::<HsvColorEditorState>(target)
        .cloned()
        .unwrap_or_default();
    if state.editing {
        match key {
            UiKeyCode::Backspace => {
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.edit_buffer.pop();
                }
            }
            UiKeyCode::Delete => {
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.edit_buffer.clear();
                }
            }
            UiKeyCode::Enter => {
                let field = state.active_field;
                let buffer = state.edit_buffer;
                edit_hsv_color(world, target, emitted, |editor| {
                    set_hsv_numeric_field(editor, field, &buffer)
                });
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.editing = false;
                    state.edit_buffer.clear();
                }
            }
            UiKeyCode::Escape => {
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.editing = false;
                    state.edit_buffer.clear();
                }
            }
            _ => {}
        }
        return;
    }
    if matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown) {
        if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
            let index = HsvColorField::ALL
                .iter()
                .position(|field| *field == state.active_field)
                .unwrap_or(0);
            let delta = if key == UiKeyCode::ArrowDown { 1 } else { -1 };
            state.active_field = HsvColorField::ALL
                [(index as i32 + delta).rem_euclid(HsvColorField::ALL.len() as i32) as usize];
        }
        return;
    }
    if key == UiKeyCode::Enter || key == UiKeyCode::Space {
        let editor = world.get::<HsvColorEditor>(target).copied();
        if let Some(editor) = editor
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.editing = true;
            state.edit_buffer = hsv_field_input_text(editor, state.active_field);
        }
        return;
    }
    if matches!(key, UiKeyCode::Backspace | UiKeyCode::Delete) {
        let editor = world.get::<HsvColorEditor>(target).copied();
        if let Some(editor) = editor
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.editing = true;
            state.edit_buffer = hsv_field_input_text(editor, state.active_field);
            state.edit_buffer.pop();
        }
        return;
    }
    let field = state.active_field;
    let Some(current) = world
        .get::<HsvColorEditor>(target)
        .and_then(|editor| hsv_numeric_field_value(*editor, field))
    else {
        return;
    };
    let step = if modifiers.control || modifiers.super_key {
        10.0
    } else if modifiers.shift {
        0.1
    } else {
        1.0
    };
    let next = match key {
        UiKeyCode::ArrowRight | UiKeyCode::ArrowUp => Some(current + step),
        UiKeyCode::ArrowLeft | UiKeyCode::ArrowDown => Some(current - step),
        UiKeyCode::Home => Some(0.0),
        UiKeyCode::End => Some(match field {
            HsvColorField::Hue => 359.0,
            HsvColorField::Saturation | HsvColorField::Value => 100.0,
            HsvColorField::Red
            | HsvColorField::Green
            | HsvColorField::Blue
            | HsvColorField::Alpha => 255.0,
            HsvColorField::Hex => return,
        }),
        _ => None,
    };
    if let Some(next) = next {
        edit_hsv_color(world, target, emitted, |editor| {
            set_hsv_numeric_field(editor, field, &next.to_string())
        });
    }
}

pub(super) fn handle_color_ramp_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    let state = world
        .get::<ColorRampEditorState>(target)
        .copied()
        .unwrap_or_default();
    if key == UiKeyCode::C || key == UiKeyCode::Space {
        if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
            state.editing_color = !state.editing_color;
        }
        return;
    }
    if key == UiKeyCode::Enter && state.editing_color {
        if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
            state.editing_color = false;
        }
        return;
    }
    if key == UiKeyCode::A {
        edit_color_ramp(world, target, emitted, |ramp| {
            let position = suggested_stop_position(ramp);
            ramp.add_stop(position, ramp.sample(position)).is_some()
        });
        return;
    }
    if matches!(key, UiKeyCode::Delete | UiKeyCode::Backspace) {
        edit_color_ramp(world, target, emitted, |ramp| {
            ramp.selected.is_some_and(|id| ramp.remove_stop(id))
        });
        return;
    }
    if !state.editing_color && key == UiKeyCode::Enter {
        edit_color_ramp(world, target, emitted, |ramp| {
            let next = match ramp.interpolation {
                Interpolation::Constant => Interpolation::Linear,
                Interpolation::Linear => Interpolation::Smooth,
                Interpolation::Smooth => Interpolation::Constant,
            };
            ramp.set_interpolation(next)
        });
        return;
    }
    if state.editing_color {
        if matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown) {
            if let Some(mut editor) = world.get_mut::<ColorRampEditorState>(target) {
                let delta = if key == UiKeyCode::ArrowDown { 1 } else { -1 };
                editor.active_channel = ColorChannel::from_index(
                    (editor.active_channel.index() as i32 + delta).rem_euclid(4) as usize,
                );
            }
            return;
        }
        let step = if modifiers.control || modifiers.super_key {
            0.1
        } else if modifiers.shift {
            0.001
        } else {
            0.01
        };
        let direction = match key {
            UiKeyCode::ArrowRight | UiKeyCode::ArrowUp => Some(1.0),
            UiKeyCode::ArrowLeft | UiKeyCode::ArrowDown => Some(-1.0),
            _ => None,
        };
        if let Some(direction) = direction {
            let channel = state.active_channel;
            edit_color_ramp(world, target, emitted, |ramp| {
                let selected = ramp.selected_stop().map(|stop| (stop.id, stop.color));
                selected.is_some_and(|(stop_id, color)| {
                    let current = match channel {
                        ColorChannel::Red => color.0,
                        ColorChannel::Green => color.1,
                        ColorChannel::Blue => color.2,
                        ColorChannel::Alpha => color.3,
                    };
                    ramp.set_color_channel(
                        stop_id,
                        channel,
                        (current + direction * step).clamp(0.0, 1.0),
                    )
                })
            });
        } else if matches!(key, UiKeyCode::Home | UiKeyCode::End) {
            let channel = state.active_channel;
            let value = if key == UiKeyCode::Home { 0.0 } else { 1.0 };
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected
                    .is_some_and(|id| ramp.set_color_channel(id, channel, value))
            });
        }
        return;
    }
    match key {
        UiKeyCode::ArrowUp => {
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_relative(-1);
            }
        }
        UiKeyCode::ArrowDown => {
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_relative(1);
            }
        }
        UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight => {
            let step = if modifiers.control || modifiers.super_key {
                0.1
            } else if modifiers.shift {
                0.001
            } else {
                0.01
            };
            let delta = if key == UiKeyCode::ArrowLeft {
                -step
            } else {
                step
            };
            edit_color_ramp(world, target, emitted, |ramp| ramp.move_selected_by(delta));
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let position = if key == UiKeyCode::Home { 0.0 } else { 1.0 };
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected.is_some_and(|id| ramp.move_stop(id, position))
            });
        }
        _ => {}
    }
}

pub(super) fn handle_text_key(
    field: &mut TextField,
    key: UiKeyCode,
    modifiers: UiModifiers,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let before = field.text.clone();
    match key {
        UiKeyCode::ArrowLeft => field.move_caret(-1, modifiers.shift),
        UiKeyCode::ArrowRight => field.move_caret(1, modifiers.shift),
        UiKeyCode::Home => field.select(0..0),
        UiKeyCode::End => {
            let end = field.text.chars().count();
            field.select(end..end);
        }
        UiKeyCode::A if modifiers.control => field.select_all(),
        UiKeyCode::Backspace => {
            field.backspace();
        }
        UiKeyCode::Delete => {
            if let Some(range) = field.selection.clone() {
                field.select(range);
                field.backspace();
            } else if field.caret < field.text.chars().count() {
                field.select(field.caret..field.caret + 1);
                field.backspace();
            }
        }
        _ => return,
    }
    if before != field.text {
        emitted.push(WidgetEvent::TextChanged {
            target,
            text: field.text.clone(),
        });
    }
}

pub(super) fn insert_text(
    world: &mut World,
    target: Entity,
    text: &str,
    emitted: &mut Vec<WidgetEvent>,
) {
    if world.get::<TextBuffer>(target).is_some() {
        if world.get::<UiReadOnly>(target).is_some() {
            return;
        }
        if let Some(mut buffer) = world.get_mut::<TextBuffer>(target)
            && buffer.insert_text(text)
        {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: buffer.as_str().to_owned(),
            });
        }
    } else if let Some(mut field) = world.get_mut::<TextField>(target) {
        if field.insert(text) {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: field.text.clone(),
            });
        }
    } else if let Some(mut field) = world.get_mut::<TextArea>(target) {
        if field.insert_text(text) {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: field.field.text.clone(),
            });
        }
    } else if let Some(mut field) = world.get_mut::<NumericField>(target) {
        field.edit_insert(text);
    }
}

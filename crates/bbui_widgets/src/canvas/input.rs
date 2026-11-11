use super::*;

#[derive(Debug)]
pub(super) struct CanvasDrag {
    target: Entity,
    button: UiPointerButton,
    last: [f32; 2],
    before: String,
    moving_objects: bool,
}

#[derive(Resource, Debug, Default)]
pub struct CanvasInputState(pub(super) HashMap<Entity, CanvasDrag>);

/// Applies routed canvas gestures before the generic widget dispatcher. Screen
/// positions are converted to canvas-local coordinates using the same layout
/// result that clips painting.
pub fn process_canvas_input(world: &mut World) {
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|queue| queue.events.clone())
        .unwrap_or_default();
    let annotated_downs: HashSet<_> = events
        .iter()
        .filter_map(|event| {
            if let UiRoutedEvent::PointerDownWithModifiers {
                window,
                target,
                button,
                ..
            } = event
            {
                Some((*window, *target, *button))
            } else {
                None
            }
        })
        .collect();
    for event in events {
        match event {
            UiRoutedEvent::PointerDown {
                window,
                target,
                button,
                position,
            } if !annotated_downs.contains(&(window, target, button)) => {
                begin_canvas_drag(
                    world,
                    window,
                    target,
                    button,
                    position,
                    UiModifiers::default(),
                );
            }
            UiRoutedEvent::PointerDownWithModifiers {
                window,
                target,
                button,
                position,
                modifiers,
            } => {
                begin_canvas_drag(world, window, target, button, position, modifiers);
            }
            UiRoutedEvent::PointerMoved {
                window, position, ..
            } => {
                let Some(mut drag) = world.resource_mut::<CanvasInputState>().0.remove(&window)
                else {
                    continue;
                };
                if let Some((local, _)) = local_point(world, drag.target, position) {
                    let delta = [local[0] - drag.last[0], local[1] - drag.last[1]];
                    if drag.button == UiPointerButton::Middle {
                        ensure_canvas_spatial_index(world, drag.target);
                        let changed = world
                            .get_mut::<Canvas>(drag.target)
                            .is_some_and(|mut canvas| canvas.pan_by(delta));
                        if changed {
                            note_canvas_metadata_edit(world, drag.target);
                        }
                    } else if drag.moving_objects {
                        let zoom = world
                            .get::<Canvas>(drag.target)
                            .map_or(1.0, |canvas| canvas.zoom);
                        move_selected_objects(
                            world,
                            drag.target,
                            [delta[0] / zoom, delta[1] / zoom],
                        );
                    }
                    drag.last = local;
                }
                world
                    .resource_mut::<CanvasInputState>()
                    .0
                    .insert(window, drag);
            }
            UiRoutedEvent::PointerUp {
                window,
                button,
                cancelled,
                ..
            } => {
                let Some(drag) = world.resource_mut::<CanvasInputState>().0.remove(&window) else {
                    continue;
                };
                if drag.button != button {
                    world
                        .resource_mut::<CanvasInputState>()
                        .0
                        .insert(window, drag);
                    continue;
                }
                finish_canvas_drag(world, drag, cancelled);
            }
            UiRoutedEvent::PointerCancel { window, .. } => {
                if let Some(drag) = world.resource_mut::<CanvasInputState>().0.remove(&window) {
                    finish_canvas_drag(world, drag, true);
                }
            }
            UiRoutedEvent::Wheel {
                target,
                position,
                delta,
                unit,
                ..
            } => {
                let Some(target) = canvas_owner(world, target) else {
                    continue;
                };
                let Some((local, _)) = local_point(world, target, position) else {
                    continue;
                };
                let amount = match unit {
                    UiWheelUnit::Lines => delta[1],
                    UiWheelUnit::Pixels => delta[1] / 48.0,
                };
                edit_canvas(world, target, |canvas| {
                    canvas.zoom_at(local, 1.1_f32.powf(amount))
                });
            }
            UiRoutedEvent::Key {
                target,
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let Some(target) = canvas_owner(world, target) else {
                    continue;
                };
                if key == UiKeyCode::Delete || key == UiKeyCode::Backspace {
                    edit_canvas_structure(world, target, |canvas| canvas.remove_selected() > 0);
                } else if matches!(
                    key,
                    UiKeyCode::ArrowLeft
                        | UiKeyCode::ArrowRight
                        | UiKeyCode::ArrowUp
                        | UiKeyCode::ArrowDown
                ) {
                    let step = if modifiers.shift {
                        10.0
                    } else if modifiers.control || modifiers.super_key {
                        0.1
                    } else {
                        1.0
                    };
                    let delta = match key {
                        UiKeyCode::ArrowLeft => [-step, 0.0],
                        UiKeyCode::ArrowRight => [step, 0.0],
                        UiKeyCode::ArrowUp => [0.0, -step],
                        _ => [0.0, step],
                    };
                    edit_canvas_move_selected(world, target, delta);
                } else if matches!(key, UiKeyCode::PageUp | UiKeyCode::PageDown) {
                    if let Some(layout) = world.get::<UiLayoutResult>(target) {
                        let center = [layout.rect.width * 0.5, layout.rect.height * 0.5];
                        let factor = if key == UiKeyCode::PageUp {
                            1.2
                        } else {
                            1.0 / 1.2
                        };
                        edit_canvas(world, target, |canvas| canvas.zoom_at(center, factor));
                    }
                } else if key == UiKeyCode::Home {
                    edit_canvas(world, target, |canvas| {
                        let changed = canvas.pan != [0.0, 0.0] || canvas.zoom != 1.0;
                        canvas.pan = [0.0, 0.0];
                        canvas.zoom = 1.0;
                        changed
                    });
                } else if key == UiKeyCode::A && (modifiers.control || modifiers.super_key) {
                    edit_canvas(world, target, |canvas| {
                        let before = canvas.selected.len();
                        canvas.selected.extend(
                            canvas
                                .objects
                                .iter()
                                .filter(|object| !object.locked)
                                .map(|object| object.id),
                        );
                        canvas.selected.len() != before
                    });
                } else if key == UiKeyCode::Escape {
                    let windows: Vec<_> = world
                        .resource::<CanvasInputState>()
                        .0
                        .iter()
                        .filter_map(|(window, drag)| (drag.target == target).then_some(*window))
                        .collect();
                    for window in windows {
                        if let Some(drag) =
                            world.resource_mut::<CanvasInputState>().0.remove(&window)
                        {
                            finish_canvas_drag(world, drag, true);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub(super) fn begin_canvas_drag(
    world: &mut World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
    modifiers: UiModifiers,
) {
    if !matches!(button, UiPointerButton::Primary | UiPointerButton::Middle) {
        return;
    }
    let Some(target) = canvas_owner(world, target) else {
        return;
    };
    let Some((local, viewport)) = local_point(world, target, position) else {
        return;
    };
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let moving_objects = if button == UiPointerButton::Primary {
        let hit = indexed_canvas_hit_test(world, target, local, viewport);
        let additive = modifiers.control || modifiers.super_key || modifiers.shift;
        let (selected, selection_changed) = {
            let Some(mut canvas) = world.get_mut::<Canvas>(target) else {
                return;
            };
            let selection_changed =
                if additive || !hit.is_some_and(|id| canvas.selected.contains(&id)) {
                    canvas.select_hit(hit, additive)
                } else {
                    false
                };
            (
                hit.is_some_and(|id| canvas.selected.contains(&id)),
                selection_changed,
            )
        };
        if selection_changed {
            note_canvas_metadata_edit(world, target);
        }
        selected
    } else {
        false
    };
    world.resource_mut::<CanvasInputState>().0.insert(
        window,
        CanvasDrag {
            target,
            button,
            last: local,
            before,
            moving_objects,
        },
    );
}

pub(super) fn canvas_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<Canvas>(target).is_some() {
        Some(target)
    } else {
        world
            .get::<CanvasObjectControl>(target)
            .map(|control| control.owner)
    }
}

pub(super) fn local_point(
    world: &World,
    target: Entity,
    screen: [f32; 2],
) -> Option<([f32; 2], UiRect)> {
    world.get::<Canvas>(target)?;
    let layout = world.get::<UiLayoutResult>(target)?;
    let viewport = UiRect::new(0.0, 0.0, layout.rect.width, layout.rect.height);
    Some((
        [screen[0] - layout.rect.x, screen[1] - layout.rect.y],
        viewport,
    ))
}

pub(super) fn edit_canvas(
    world: &mut World,
    target: Entity,
    edit: impl FnOnce(&mut Canvas) -> bool,
) {
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<Canvas>(target)
        .is_some_and(|mut canvas| edit(&mut canvas));
    if changed {
        note_canvas_metadata_edit(world, target);
        emit_canvas_edit(world, target, before);
    }
}

pub(super) fn edit_canvas_structure(
    world: &mut World,
    target: Entity,
    edit: impl FnOnce(&mut Canvas) -> bool,
) {
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<Canvas>(target)
        .is_some_and(|mut canvas| edit(&mut canvas));
    if changed {
        emit_canvas_edit(world, target, before);
    }
}

pub(super) fn edit_canvas_move_selected(world: &mut World, target: Entity, delta: [f32; 2]) {
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = move_selected_objects(world, target, delta);
    if changed {
        emit_canvas_edit(world, target, before);
    }
}

pub(super) fn selected_canvas_object_positions(
    world: &World,
    target: Entity,
) -> Option<Vec<usize>> {
    let canvas = world.get::<Canvas>(target)?;
    let indexes = world.get_resource::<CanvasSpatialIndexes>()?;
    let mut positions = Vec::with_capacity(canvas.selected.len());
    for id in &canvas.selected {
        positions.push(indexes.position(target, *id)?);
    }
    Some(positions)
}

pub(super) fn move_selected_objects(world: &mut World, target: Entity, delta: [f32; 2]) -> bool {
    ensure_canvas_spatial_index(world, target);
    let Some(positions) = selected_canvas_object_positions(world, target) else {
        let changed = world
            .get_mut::<Canvas>(target)
            .is_some_and(|mut canvas| canvas.move_selected(delta));
        return changed;
    };
    let changed = world
        .get_mut::<Canvas>(target)
        .is_some_and(|mut canvas| canvas.move_selected_at_indices(&positions, delta));
    if changed {
        note_canvas_object_edits(world, target, &positions);
    }
    changed
}

pub(super) fn indexed_canvas_hit_test(
    world: &World,
    target: Entity,
    position: [f32; 2],
    viewport: UiRect,
) -> Option<u64> {
    let canvas = world.get::<Canvas>(target)?;
    let indexes = world.get_resource::<CanvasSpatialIndexes>()?;
    let index = indexes.0.get(&target)?;
    Some(canvas.hit_test_indexed(index, position, viewport)).flatten()
}

pub(super) fn finish_canvas_drag(world: &mut World, drag: CanvasDrag, cancelled: bool) {
    if cancelled {
        if let Ok(previous) = Canvas::from_json(&drag.before)
            && let Some(mut canvas) = world.get_mut::<Canvas>(drag.target)
        {
            *canvas = previous;
        }
    } else {
        emit_canvas_edit(world, drag.target, drag.before);
    }
}

pub(super) fn emit_canvas_edit(world: &mut World, target: Entity, before: String) {
    let Some(after) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    if before != after {
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::CanvasCommitted {
                target,
                before,
                after,
            });
    }
}

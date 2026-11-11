use super::*;

const NUMERIC_SCRUB_LOCK_DISTANCE: f64 = 3.0;

pub(super) fn numeric_target(field: &NumericField) -> NumericTarget {
    NumericTarget {
        start: field.value,
        min: field.min.unwrap_or(-f64::MAX),
        max: field.max.unwrap_or(f64::MAX),
        step: field.step,
    }
}

pub(super) fn slider_track_x(rect: Rect, theme: &Theme) -> (f32, f32) {
    let padding = theme.spacing.control.padding_x.max(0.0);
    let handle_radius = theme.spacing.editor.handle_radius;
    let handle_radius = if handle_radius.is_finite() {
        handle_radius.clamp(3.0, 9.0)
    } else {
        6.0
    };
    let left = rect.x + padding + handle_radius;
    let right = rect.x + rect.width - padding - handle_radius;
    if left <= right {
        (left, right)
    } else {
        let center = rect.x + rect.width * 0.5;
        (center, center)
    }
}

pub(super) fn slider_value_at_x(x: f32, left: f32, right: f32) -> f64 {
    if right <= left {
        0.5
    } else {
        f64::from(((x - left) / (right - left)).clamp(0.0, 1.0))
    }
}

pub(super) fn begin_slider_drag(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let Some(slider) = world.get::<Slider>(target).cloned() else {
        return;
    };
    if !slider.enabled || slider.read_only {
        return;
    }
    let (Some(min), Some(max)) = (slider.min, slider.max) else {
        return;
    };
    if !min.is_finite() || !max.is_finite() || !(max - min).is_finite() {
        return;
    }
    let rect = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
        .or_else(|| {
            world
                .get::<UiHitBox>(target)
                .map(|hit| Rect::new(hit.rect.x, hit.rect.y, hit.rect.width, hit.rect.height))
        });
    let Some(rect) = rect else {
        return;
    };
    let base_theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let theme = styled_theme(world, target, &base_theme);
    let (track_left, track_right) = slider_track_x(rect, &theme);
    let before = slider.value;
    if let Some(mut slider) = world.get_mut::<Slider>(target) {
        let _ =
            slider.set_normalized_value(slider_value_at_x(position[0], track_left, track_right));
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .slider_drags
        .insert(
            window,
            SliderDrag {
                target,
                before,
                min,
                max,
                track_left,
                track_right,
            },
        );
}

pub(super) fn update_slider_drag(world: &mut World, window: Entity, x: f32) {
    let Some(drag) = world
        .resource::<WidgetInteractionState>()
        .slider_drags
        .get(&window)
        .copied()
    else {
        return;
    };
    if let Some(mut slider) = world.get_mut::<Slider>(drag.target)
        && slider.enabled
        && !slider.read_only
        && slider.min == Some(drag.min)
        && slider.max == Some(drag.max)
    {
        let _ =
            slider.set_normalized_value(slider_value_at_x(x, drag.track_left, drag.track_right));
    }
}

pub(super) fn finish_slider_drag(
    world: &mut World,
    drag: SliderDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(mut slider) = world.get_mut::<Slider>(drag.target) else {
        return;
    };
    if cancelled {
        let _ = slider.restore_value(drag.before);
        emitted.push(WidgetEvent::NumericCancelled {
            target: drag.target,
            restored: vec![drag.before],
        });
        return;
    }
    let after = slider.value;
    if drag.before != after {
        emitted.push(WidgetEvent::NumericCommitted {
            target: drag.target,
            before: vec![drag.before],
            after: vec![after],
        });
    }
}

pub(super) fn cancel_slider_drag(
    world: &mut World,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    let drag = world
        .resource_mut::<WidgetInteractionState>()
        .slider_drags
        .remove(&window);
    if let Some(drag) = drag {
        finish_slider_drag(world, drag, true, emitted);
        true
    } else {
        false
    }
}

pub(super) fn begin_numeric_scrub(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let Some(field) = world.get::<NumericField>(target).cloned() else {
        return;
    };
    if !field.enabled || field.read_only {
        return;
    }
    let target_data = numeric_target(&field);
    let Ok(gesture) = NumericGesture::begin(vec![target_data]) else {
        return;
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .numeric_scrubs
        .insert(
            window,
            NumericScrub {
                entity: target,
                targets: vec![target],
                target_specs: vec![target_data],
                starts: vec![field.value],
                last_values: vec![field.value],
                gesture,
                phase: NumericScrubPhase::Selecting,
                last_position: position,
                pending_x: 0.0,
                pending_moves: Vec::new(),
                kind: field.kind,
                unit: field.unit,
            },
        );
    world
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::Begin {
            window,
            target,
            hide_cursor: true,
            continuous: true,
        });
}

pub(super) fn numeric_targets_crossed(
    world: &mut World,
    window: Entity,
    from: [f32; 2],
    to: [f32; 2],
    kind: NumericFieldKind,
    unit: Option<&str>,
) -> Vec<(Entity, NumericTarget)> {
    let delta_y = to[1] - from[1];
    if delta_y.abs() < f32::EPSILON {
        return Vec::new();
    }
    let min_y = from[1].min(to[1]);
    let max_y = from[1].max(to[1]);
    let mut query = world.query::<(Entity, &NumericField, &UiHitBox)>();
    let mut crossed: Vec<_> = query
        .iter(world)
        .filter_map(|(entity, field, hit)| {
            if field.kind != kind
                || field.unit.as_deref() != unit
                || !field.enabled
                || field.read_only
                || hit.window.is_some_and(|hit_window| hit_window != window)
                || !is_interactable(world, entity)
            {
                return None;
            }
            let center_y = hit.rect.y + hit.rect.height * 0.5;
            if center_y < min_y || center_y > max_y {
                return None;
            }
            let t = (center_y - from[1]) / delta_y;
            let x = from[0] + (to[0] - from[0]) * t;
            let point = [x, center_y];
            if !hit.contains(point)
                || hit
                    .clip
                    .is_some_and(|clip| !clip.contains(point[0], point[1]))
            {
                return None;
            }
            Some((t, hit.z_index, entity, numeric_target(field)))
        })
        .collect();
    crossed.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.index().cmp(&b.2.index()))
    });
    crossed
        .into_iter()
        .map(|(_, _, entity, target)| (entity, target))
        .collect()
}

pub(super) fn update_numeric_scrub(
    world: &mut World,
    window: Entity,
    position: [f32; 2],
    delta_x: f32,
    modifiers: UiModifiers,
) {
    let scrub_info = world
        .resource::<WidgetInteractionState>()
        .numeric_scrubs
        .get(&window)
        .map(|scrub| {
            (
                scrub.phase,
                scrub.last_position,
                scrub.kind,
                scrub.unit.clone(),
            )
        });
    let Some((phase, last_position, kind, unit)) = scrub_info else {
        return;
    };
    let additions = if phase == NumericScrubPhase::Selecting {
        numeric_targets_crossed(
            world,
            window,
            last_position,
            position,
            kind,
            unit.as_deref(),
        )
    } else {
        Vec::new()
    };
    let modifiers = NumericModifiers {
        precision: modifiers.shift,
        snap: modifiers.control,
    };
    let updates = {
        let mut state = world.resource_mut::<WidgetInteractionState>();
        let Some(scrub) = state.numeric_scrubs.get_mut(&window) else {
            return;
        };
        if scrub.phase == NumericScrubPhase::Selecting {
            for (entity, target) in additions {
                if !scrub.targets.contains(&entity) {
                    scrub.targets.push(entity);
                    scrub.target_specs.push(target);
                    scrub.starts.push(target.start);
                    scrub.last_values.push(target.start);
                    if let Ok(gesture) = NumericGesture::begin(scrub.target_specs.clone()) {
                        scrub.gesture = gesture;
                    } else {
                        scrub.targets.pop();
                        scrub.target_specs.pop();
                        scrub.starts.pop();
                        scrub.last_values.pop();
                    }
                }
            }
        }
        scrub.last_position = position;
        if scrub.phase == NumericScrubPhase::Selecting {
            let dx = f64::from(delta_x);
            scrub.pending_x += dx;
            scrub.pending_moves.push((dx, modifiers));
            if scrub.pending_x.abs() >= NUMERIC_SCRUB_LOCK_DISTANCE {
                scrub.phase = NumericScrubPhase::Scrubbing;
                let mut values = scrub.last_values.clone();
                for (movement, segment_modifiers) in std::mem::take(&mut scrub.pending_moves) {
                    values = scrub.gesture.update(movement, segment_modifiers).to_vec();
                }
                Some(
                    scrub
                        .targets
                        .iter()
                        .copied()
                        .zip(values)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        } else {
            let values = scrub.gesture.update(f64::from(delta_x), modifiers).to_vec();
            Some(
                scrub
                    .targets
                    .iter()
                    .copied()
                    .zip(values)
                    .collect::<Vec<_>>(),
            )
        }
    };
    let Some(updates) = updates else {
        return;
    };
    let actual_values: Vec<_> = updates
        .into_iter()
        .map(|(entity, value)| {
            let actual = if let Some(mut field) = world.get_mut::<NumericField>(entity) {
                if field.enabled && !field.read_only {
                    let _ = field.set_value(value);
                }
                field.value
            } else {
                value
            };
            (entity, actual)
        })
        .collect();
    let mut state = world.resource_mut::<WidgetInteractionState>();
    if let Some(scrub) = state.numeric_scrubs.get_mut(&window) {
        for (index, entity) in scrub.targets.iter().enumerate() {
            if let Some((_, value)) = actual_values.iter().find(|(target, _)| target == entity) {
                scrub.last_values[index] = *value;
            }
        }
    }
}

pub(super) fn finish_numeric_scrub(
    world: &mut World,
    window: Entity,
    scrub: NumericScrub,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    world
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::End {
            window,
            target: scrub.entity,
            restore_position: true,
        });
    if cancelled {
        for (entity, start) in scrub
            .targets
            .iter()
            .copied()
            .zip(scrub.starts.iter().copied())
        {
            if let Some(mut field) = world.get_mut::<NumericField>(entity) {
                field.value = start;
            }
        }
        emitted.push(WidgetEvent::NumericCancelled {
            target: scrub.entity,
            restored: scrub.starts,
        });
        return;
    }
    let changes: Vec<_> = scrub
        .targets
        .iter()
        .copied()
        .zip(scrub.starts.iter().copied())
        .filter_map(|(entity, before)| {
            let after = world.get::<NumericField>(entity)?.value;
            (before != after).then_some((entity, before, after))
        })
        .collect();
    match changes.as_slice() {
        [] => {}
        [(target, before, after)] => emitted.push(WidgetEvent::NumericCommitted {
            target: *target,
            before: vec![*before],
            after: vec![*after],
        }),
        _ => emitted.push(WidgetEvent::NumericGroupCommitted { changes }),
    }
}

pub(super) fn cancel_numeric_scrub(
    world: &mut World,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    let scrub = world
        .resource_mut::<WidgetInteractionState>()
        .numeric_scrubs
        .remove(&window);
    if let Some(scrub) = scrub {
        finish_numeric_scrub(world, window, scrub, true, emitted);
        true
    } else {
        false
    }
}

pub(super) fn apply_checkbox_paint_at(world: &mut World, window: Entity, position: [f32; 2]) {
    let mut query = world.query::<(Entity, &UiHitBox, &Checkbox)>();
    let hits: Vec<_> = query
        .iter(world)
        .filter(|(entity, hit, checkbox)| {
            checkbox.enabled
                && !checkbox.read_only
                && hit.window.is_none_or(|hit_window| hit_window == window)
                && is_interactable(world, *entity)
                && hit.contains(position)
        })
        .map(|(entity, hit, _)| (hit.z_index, entity))
        .collect();
    let Some((_, target)) = hits
        .into_iter()
        .max_by_key(|(z, entity)| (*z, entity.index()))
    else {
        return;
    };
    let target_value = {
        let state = world.resource::<WidgetInteractionState>();
        let Some(transaction) = state.checkbox_paints.get(&window) else {
            return;
        };
        transaction.target
    };
    let should_visit = {
        let mut state = world.resource_mut::<WidgetInteractionState>();
        let Some(transaction) = state.checkbox_paints.get_mut(&window) else {
            return;
        };
        transaction.visited.insert(target)
    };
    if !should_visit {
        return;
    }
    let change = {
        let Some(mut checkbox) = world.get_mut::<Checkbox>(target) else {
            return;
        };
        if !checkbox.enabled || checkbox.read_only {
            return;
        }
        let before = checkbox.value;
        let after = if target_value {
            CheckboxValue::On
        } else {
            CheckboxValue::Off
        };
        checkbox.value = after;
        (before != after).then_some(CheckboxPaintChange {
            entity: target,
            before,
            after,
        })
    };
    if let Some(change) = change
        && let Some(transaction) = world
            .resource_mut::<WidgetInteractionState>()
            .checkbox_paints
            .get_mut(&window)
    {
        transaction.changes.push(change);
    }
}

pub(super) fn restore_checkboxes(world: &mut World, changes: &[CheckboxPaintChange]) {
    for change in changes {
        if let Some(mut checkbox) = world.get_mut::<Checkbox>(change.entity) {
            checkbox.value = change.before;
        }
    }
}

pub(super) fn cancel_gestures(world: &mut World, window: Entity, emitted: &mut Vec<WidgetEvent>) {
    let paint = {
        world
            .resource_mut::<WidgetInteractionState>()
            .checkbox_paints
            .remove(&window)
    };
    if let Some(paint) = paint {
        restore_checkboxes(world, &paint.changes);
        emitted.push(WidgetEvent::CheckboxPaintCancelled {
            changes: paint.changes,
        });
    }
    if let Some(scrub) = world
        .resource_mut::<WidgetInteractionState>()
        .numeric_scrubs
        .remove(&window)
    {
        finish_numeric_scrub(world, window, scrub, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .slider_drags
        .remove(&window)
    {
        finish_slider_drag(world, drag, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .color_ramp_drags
        .remove(&window)
    {
        finish_color_ramp_drag(world, drag, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .hsv_color_drags
        .remove(&window)
    {
        finish_hsv_color_drag(world, drag, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .curve_editor_drags
        .remove(&window)
    {
        finish_curve_editor_drag(world, drag, true, emitted);
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .collection_drags
        .remove(&window);
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .data_table_resize_drags
        .remove(&window)
    {
        finish_data_table_resize(world, drag, true, emitted);
    }
    if let Some(gesture) = world
        .resource_mut::<WidgetInteractionState>()
        .panel_reorders
        .remove(&window)
    {
        finish_panel_reorder(world, gesture, true, emitted);
    }
}

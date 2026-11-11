use super::*;

/// Route queued events to the topmost eligible node and maintain focus/capture.
///
/// Captured pointer motion stays with the pressed node until release. If that
/// node is removed, disabled, hidden, or loses its bounds during a gesture, the
/// capture is cancelled and focus references are cleared.
pub fn process_ui_input(world: &mut World) {
    world.init_resource::<crate::UiDirty>();
    world.init_resource::<UiHoverValidation>();
    if !std::mem::take(&mut world.resource_mut::<UiHoverValidation>().prepared) {
        world
            .resource_mut::<UiDirty>()
            .mark_all(UiDirtyDomain::HitTest);
    }
    let events: Vec<_> = world
        .get_resource_mut::<UiInputQueue>()
        .map(|mut queue| queue.events.drain(..).collect())
        .unwrap_or_default();
    let lifecycle_events = world
        .get_resource_mut::<UiLifecycleEvents>()
        .map(|mut events| std::mem::take(&mut events.0))
        .unwrap_or_default();
    if let Some(mut routed) = world.get_resource_mut::<UiRoutedEvents>() {
        routed.events.clear();
        routed.events.extend(lifecycle_events);
    } else {
        return;
    }

    validate_states(world);

    for event in events {
        match event {
            UiInputEvent::PointerMove {
                window,
                position,
                time_seconds: _,
            } if position.iter().all(|value| value.is_finite()) => {
                route_pointer_move(world, window, position);
            }
            UiInputEvent::PointerButton {
                window,
                button,
                pressed,
                position,
                time_seconds,
            } if position.iter().all(|value| value.is_finite()) && time_seconds.is_finite() => {
                route_pointer_button(world, window, button, pressed, position, time_seconds, None);
            }
            UiInputEvent::PointerButtonWithModifiers {
                window,
                button,
                pressed,
                position,
                time_seconds,
                modifiers,
            } if position.iter().all(|value| value.is_finite()) && time_seconds.is_finite() => {
                route_pointer_button(
                    world,
                    window,
                    button,
                    pressed,
                    position,
                    time_seconds,
                    Some(modifiers),
                );
            }
            UiInputEvent::PointerCancel { window } => cancel_pointer(world, window),
            UiInputEvent::PointerLeave { window } => pointer_leave(world, window),
            UiInputEvent::WindowBlur { window } => window_blur(world, window),
            UiInputEvent::Key {
                window,
                key,
                pressed,
                repeat,
                modifiers,
            } => route_key(world, window, key, pressed, repeat, modifiers),
            UiInputEvent::Text { window, text } if !text.is_empty() => {
                let target = focused(world, window);
                if let Some(target) =
                    target.filter(|target| is_interactable_in_window(world, *target, window))
                {
                    emit(
                        world,
                        UiRoutedEvent::Text {
                            window,
                            target,
                            text,
                        },
                    );
                }
            }
            UiInputEvent::ImePreedit {
                window,
                text,
                cursor_range,
            } => {
                if let Some(target) = focused(world, window)
                    .filter(|target| is_interactable_in_window(world, *target, window))
                {
                    emit(
                        world,
                        UiRoutedEvent::ImePreedit {
                            window,
                            target,
                            text,
                            cursor_range,
                        },
                    );
                }
            }
            UiInputEvent::ImeCommit { window, text } if !text.is_empty() => {
                if let Some(target) = focused(world, window)
                    .filter(|target| is_interactable_in_window(world, *target, window))
                {
                    emit(
                        world,
                        UiRoutedEvent::ImeCommit {
                            window,
                            target,
                            text,
                        },
                    );
                }
            }
            UiInputEvent::ImeDisabled { window } => {
                if let Some(target) = focused(world, window)
                    .filter(|target| is_interactable_in_window(world, *target, window))
                {
                    emit(world, UiRoutedEvent::ImeDisabled { window, target });
                }
            }
            UiInputEvent::Wheel {
                window,
                position,
                delta,
                unit,
                modifiers,
            } if position.iter().all(|value| value.is_finite())
                && delta.iter().all(|value| value.is_finite()) =>
            {
                let target = hit_test(world, window, position).or_else(|| focused(world, window));
                if let Some(target) =
                    target.filter(|target| is_interactable_in_window(world, *target, window))
                {
                    emit(
                        world,
                        UiRoutedEvent::Wheel {
                            window,
                            target,
                            position,
                            delta,
                            unit,
                            modifiers,
                        },
                    );
                }
            }
            _ => {}
        }
    }
    validate_states(world);
}

/// Recompute hover from current hitboxes for windows with a known pointer
/// position. Layout can call this after moving or clipping controls so a
/// stationary cursor receives the appropriate leave/enter events immediately.
/// Returns true when at least one window's hovered target changed.
pub fn refresh_ui_hover(world: &mut World) -> bool {
    let pointers: Vec<_> = world
        .get_resource::<UiFocus>()
        .map(|focus| {
            focus
                .windows
                .iter()
                .filter_map(|(window, state)| {
                    state
                        .pointer_position
                        .map(|position| (*window, position, state.hovered))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut changed = false;
    for (window, position, previous) in pointers {
        let target = hit_test(world, window, position);
        if target != previous {
            update_hover(world, window, target);
            changed = true;
        }
    }
    changed
}

use super::*;

pub(super) fn route_pointer_move(world: &mut World, window: Entity, position: [f32; 2]) {
    let (previous_position, capture, press_position) = {
        let focus = world.resource::<UiFocus>();
        let state = focus.state(window);
        state.map_or((None, None, None), |state| {
            (state.pointer_position, state.captured, state.press_position)
        })
    };
    let target = hit_test(world, window, position);
    let route_target = capture
        .filter(|entity| is_pointer_capture_valid(world, window, *entity))
        .or(target);
    let delta = previous_position.map_or([0.0, 0.0], |previous| {
        [position[0] - previous[0], position[1] - previous[1]]
    });
    {
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        state.pointer_position = Some(position);
        focus.set_active(window);
    }
    update_hover(world, window, target);
    if let Some(target) = route_target {
        let dragging = capture.is_some_and(|_| {
            press_position.is_some_and(|origin| distance_squared(origin, position) >= 16.0)
        });
        emit(
            world,
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position,
                delta,
                dragging,
            },
        );
    }
}

pub(super) fn route_pointer_button(
    world: &mut World,
    window: Entity,
    button: UiPointerButton,
    pressed: bool,
    position: [f32; 2],
    time_seconds: f64,
    modifiers: Option<UiModifiers>,
) {
    let target = hit_test(world, window, position);
    {
        let mut focus = world.resource_mut::<UiFocus>();
        focus.state_mut(window).pointer_position = Some(position);
        focus.set_active(window);
    }
    update_hover(world, window, target);
    if pressed {
        if world
            .resource::<UiFocus>()
            .state(window)
            .is_some_and(|state| state.captured.is_some())
        {
            cancel_pointer(world, window);
        }
        let previous_focus = {
            let focus = world.resource::<UiFocus>();
            focus.state(window).and_then(|state| state.focused)
        };
        let target = target.filter(|target| is_interactable_in_window(world, *target, window));
        let focus_target = target.filter(|entity| world.get::<UiFocusable>(*entity).is_some());
        let current_focus = focus_target
            .or(previous_focus.filter(|entity| is_interactable_in_window(world, *entity, window)));
        {
            let mut focus = world.resource_mut::<UiFocus>();
            let state = focus.state_mut(window);
            state.pointer_position = Some(position);
            state.press_position = target.map(|_| position);
            state.captured = target;
            state.captured_button = target.map(|_| button);
            if target.is_some() {
                state.focused = current_focus;
            }
            focus.set_active(window);
        }
        if previous_focus != current_focus && target.is_some() {
            if let Some(previous) = previous_focus {
                emit(
                    world,
                    UiRoutedEvent::ImeDisabled {
                        window,
                        target: previous,
                    },
                );
            }
            emit(
                world,
                UiRoutedEvent::FocusChanged {
                    window,
                    previous: previous_focus,
                    current: current_focus,
                },
            );
        }
        if let Some(target) = target {
            emit(
                world,
                UiRoutedEvent::PointerDown {
                    window,
                    target,
                    button,
                    position,
                },
            );
            if let Some(modifiers) = modifiers {
                emit(
                    world,
                    UiRoutedEvent::PointerDownWithModifiers {
                        window,
                        target,
                        button,
                        position,
                        modifiers,
                    },
                );
            }
        }
    } else {
        let (capture, captured_button) = {
            let focus = world.resource::<UiFocus>();
            let state = focus.state(window);
            state.map_or((None, None), |state| {
                (state.captured, state.captured_button)
            })
        };
        if let Some(captured) = capture {
            if captured_button != Some(button) {
                return;
            }
            if !is_pointer_capture_valid(world, window, captured) {
                cancel_pointer(world, window);
                return;
            }
            let same_hit = target == Some(captured);
            emit(
                world,
                UiRoutedEvent::PointerUp {
                    window,
                    target: captured,
                    button,
                    position,
                    cancelled: false,
                },
            );
            if let Some(modifiers) = modifiers {
                emit(
                    world,
                    UiRoutedEvent::PointerUpWithModifiers {
                        window,
                        target: captured,
                        button,
                        position,
                        modifiers,
                    },
                );
            }
            if same_hit {
                let previous = world
                    .resource::<UiFocus>()
                    .state(window)
                    .and_then(|state| state.last_click);
                let click_count = previous
                    .filter(|last| {
                        last.target == captured
                            && time_seconds >= last.at_seconds
                            && time_seconds - last.at_seconds <= DOUBLE_CLICK_SECONDS
                            && distance_squared(last.position, position)
                                <= DOUBLE_CLICK_DISTANCE * DOUBLE_CLICK_DISTANCE
                    })
                    .map_or(1, |last| last.count.saturating_add(1));
                world.resource_mut::<UiFocus>().state_mut(window).last_click =
                    Some(UiClickRecord {
                        target: captured,
                        at_seconds: time_seconds,
                        position,
                        count: click_count,
                    });
                emit(
                    world,
                    UiRoutedEvent::Click {
                        window,
                        target: captured,
                        button,
                        click_count,
                        position,
                    },
                );
                if let Some(modifiers) = modifiers {
                    emit(
                        world,
                        UiRoutedEvent::ClickWithModifiers {
                            window,
                            target: captured,
                            button,
                            click_count,
                            position,
                            modifiers,
                        },
                    );
                }
            }
        }
        {
            let mut focus = world.resource_mut::<UiFocus>();
            let state = focus.state_mut(window);
            state.pointer_position = Some(position);
            state.press_position = None;
            state.captured = None;
            state.captured_button = None;
            focus.set_active(window);
        }
    }
}

pub(super) fn cancel_pointer(world: &mut World, window: Entity) {
    let capture = {
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        let target = state.captured.take();
        let button = state.captured_button.take();
        let position = state.pointer_position.unwrap_or([0.0, 0.0]);
        state.press_position = None;
        focus.set_active(window);
        target.map(|target| (target, button.unwrap_or(UiPointerButton::Primary), position))
    };
    if let Some((target, button, position)) = capture {
        emit(world, UiRoutedEvent::PointerCancel { window, target });
        emit(
            world,
            UiRoutedEvent::PointerUp {
                window,
                target,
                button,
                position,
                cancelled: true,
            },
        );
    }
}

pub(super) fn pointer_leave(world: &mut World, window: Entity) {
    let old_hover = {
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        state.pointer_position = None;
        let old_hover = state.hovered.take();
        focus.set_active(window);
        old_hover
    };
    if let Some(target) = old_hover {
        emit(world, UiRoutedEvent::PointerLeave { window, target });
    }
}

pub(super) fn window_blur(world: &mut World, window: Entity) {
    let (previous, capture, hover, position) = {
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        let previous = state.focused.take();
        let capture = state
            .captured
            .take()
            .map(|target| (target, state.captured_button.take()));
        let hover = state.hovered.take();
        let position = state.pointer_position.unwrap_or([0.0, 0.0]);
        state.pointer_position = None;
        state.press_position = None;
        state.captured_button = None;
        state.selection = None;
        state.last_click = None;
        (previous, capture, hover, position)
    };
    if let Some((target, button)) = capture {
        emit(world, UiRoutedEvent::PointerCancel { window, target });
        emit(
            world,
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: button.unwrap_or(UiPointerButton::Primary),
                position,
                cancelled: true,
            },
        );
    }
    if let Some(target) = hover {
        emit(world, UiRoutedEvent::PointerLeave { window, target });
    }
    if let Some(target) = previous {
        emit(world, UiRoutedEvent::ImeDisabled { window, target });
        emit(
            world,
            UiRoutedEvent::FocusChanged {
                window,
                previous: Some(target),
                current: None,
            },
        );
    }
    world.resource_mut::<UiFocus>().set_active(window);
}

pub(super) fn update_hover(world: &mut World, window: Entity, target: Option<Entity>) {
    let previous = {
        let mut focus = world.resource_mut::<UiFocus>();
        let previous = {
            let state = focus.windows.entry(window).or_default();
            let previous = state.hovered;
            state.hovered = target;
            previous
        };
        if focus.active_window == Some(window) {
            focus.sync_active();
        }
        previous
    };
    if previous == target {
        return;
    }
    if let Some(previous) = previous {
        emit(
            world,
            UiRoutedEvent::PointerLeave {
                window,
                target: previous,
            },
        );
    }
    if let Some(target) = target {
        emit(world, UiRoutedEvent::PointerEnter { window, target });
    }
}

pub(super) fn is_pointer_capture_valid(world: &World, window: Entity, target: Entity) -> bool {
    world.get::<UiHitBox>(target).is_some() && is_interactable_in_window(world, target, window)
}

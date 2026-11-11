use super::*;

pub(super) fn validate_states(world: &mut World) {
    let hit_changes = world
        .resource_mut::<crate::UiDirty>()
        .take(crate::UiDirtyDomain::HitTest);
    let hit_generation = hit_changes.generation;
    let mut windows: Vec<Entity> = world
        .resource::<UiFocus>()
        .windows
        .keys()
        .copied()
        .collect();
    windows.sort_unstable();
    for window in windows {
        let (focused, hovered, captured, pointer_position, last_click) = world
            .resource::<UiFocus>()
            .state(window)
            .map_or((None, None, None, None, None), |state| {
                (
                    state.focused,
                    state.hovered,
                    state.captured,
                    state.pointer_position,
                    state.last_click,
                )
            });
        let focus_invalid = focused.is_some_and(|entity| {
            !is_interactable_in_window(world, entity, window)
                || !has_visible_focus_geometry(world, entity)
        });
        let capture_invalid =
            captured.is_some_and(|entity| !is_pointer_capture_valid(world, window, entity));
        let click_invalid =
            last_click.is_some_and(|last| !is_interactable_in_window(world, last.target, window));
        let hover_invalid =
            hovered.is_some_and(|entity| !is_interactable_in_window(world, entity, window));
        let needs_hover_refresh = pointer_position.is_some_and(|position| {
            let affected = hit_changes.all
                || hit_changes.entities.iter().any(|&entity| {
                    hovered == Some(entity)
                        || world.get::<UiHitBox>(entity).is_some_and(|bounds| {
                            bounds.window.is_none_or(|owner| owner == window)
                                && bounds.contains(position)
                        })
                });
            world
                .resource::<UiHoverValidation>()
                .windows
                .get(&window)
                .is_none_or(|&(previous, generation)| {
                    previous != position || (generation != hit_generation && affected)
                })
        });
        let refreshed_hover = match pointer_position {
            Some(position) if needs_hover_refresh => {
                world
                    .resource_mut::<UiHoverValidation>()
                    .windows
                    .insert(window, (position, hit_generation));
                Some(hit_test(world, window, position))
            }
            Some(_) => None,
            None if hover_invalid => Some(None),
            None => None,
        };
        let capture_cancel = if capture_invalid {
            let mut focus = world.resource_mut::<UiFocus>();
            let Some(state) = focus.windows.get_mut(&window) else {
                continue;
            };
            let target = state.captured.take();
            let button = state.captured_button.take();
            let position = state.pointer_position.unwrap_or([0.0, 0.0]);
            state.press_position = None;
            if focus.active_window == Some(window) {
                focus.sync_active();
            }
            target.map(|target| (target, button, position))
        } else {
            None
        };
        let invalid_focus_target = focus_invalid
            .then(|| take_invalid_focus(world, window))
            .flatten();
        if click_invalid
            && let Some(state) = world.resource_mut::<UiFocus>().windows.get_mut(&window)
        {
            state.last_click = None;
        }
        if let Some((target, button, position)) = capture_cancel {
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
        if let Some(target) = invalid_focus_target {
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
        if let Some(target) = refreshed_hover {
            update_hover(world, window, target);
        }
    }
}

fn take_invalid_focus(world: &mut World, window: Entity) -> Option<Entity> {
    let mut focus = world.resource_mut::<UiFocus>();
    let state = focus.windows.get_mut(&window)?;
    let target = state.focused.take();
    state.selection = None;
    if focus.active_window == Some(window) {
        focus.sync_active();
    }
    target
}

/// Drop focus after a widget update hides its hit region, before the
/// accessibility focus mirror runs later in the same frame.
pub fn clear_invalid_ui_focus(world: &mut World) {
    let windows: Vec<_> = world
        .get_resource::<UiFocus>()
        .map(|focus| focus.windows.keys().copied().collect())
        .unwrap_or_default();
    for window in windows {
        let target = world
            .resource::<UiFocus>()
            .state(window)
            .and_then(|state| state.focused);
        if target.is_some_and(|entity| {
            !is_interactable_in_window(world, entity, window)
                || !has_visible_focus_geometry(world, entity)
        }) && let Some(target) = take_invalid_focus(world, window)
        {
            world.init_resource::<UiLifecycleEvents>();
            world.resource_mut::<UiLifecycleEvents>().0.extend([
                UiRoutedEvent::ImeDisabled { window, target },
                UiRoutedEvent::FocusChanged {
                    window,
                    previous: Some(target),
                    current: None,
                },
            ]);
        }
    }
}

pub(super) fn emit(world: &mut World, event: UiRoutedEvent) {
    if let Some(mut events) = world.get_resource_mut::<UiRoutedEvents>() {
        events.events.push(event);
    }
}

pub(super) fn distance_squared(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

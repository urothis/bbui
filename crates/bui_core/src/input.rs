//! Platform-neutral pointer and keyboard routing for BUI controls.

use std::collections::{HashMap, VecDeque};

use bevy_ecs::prelude::*;

type HitTestChanged<'w, 's> = Query<
    'w,
    's,
    Entity,
    Or<(
        Changed<UiNode>,
        Changed<UiHitBox>,
        Changed<UiEnabled>,
        Changed<UiVisible>,
        Changed<UiParent>,
        Changed<UiChildren>,
    )>,
>;

use crate::{
    UiChildren, UiClickRecord, UiDirty, UiDirtyDomain, UiEnabled, UiFocus, UiFocusable, UiHitBox,
    UiLifecycleEvents, UiNode, UiParent, UiRect, UiTabIndex, UiVisible, has_window_assignment,
    is_interactable_in_window,
};

const DOUBLE_CLICK_SECONDS: f64 = 0.35;
const DOUBLE_CLICK_DISTANCE: f32 = 5.0;

#[derive(Resource, Default)]
pub(crate) struct UiHoverValidation {
    windows: HashMap<Entity, ([f32; 2], u64)>,
    prepared: bool,
    #[cfg(test)]
    hit_tests: u64,
}

pub(crate) fn collect_hit_test_changes(
    changed: HitTestChanged,
    mut validation: ResMut<UiHoverValidation>,
    mut dirty: ResMut<UiDirty>,
) {
    if changed.iter().next().is_some() {
        dirty.mark_all(UiDirtyDomain::HitTest);
    }
    validation.prepared = true;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiPointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiWheelUnit {
    Lines,
    Pixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiKeyCode {
    Tab,
    Escape,
    Enter,
    Space,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    F2,
    A,
    C,
    N,
    V,
    X,
    Y,
    Z,
    Other(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiInputEvent {
    PointerMove {
        window: Entity,
        position: [f32; 2],
        time_seconds: f64,
    },
    PointerButton {
        window: Entity,
        button: UiPointerButton,
        pressed: bool,
        position: [f32; 2],
        time_seconds: f64,
    },
    /// Modifier-bearing pointer edge for hosts that can query keyboard state
    /// while reading mouse button messages. Routing also emits the legacy
    /// pointer event variant so existing consumers keep working.
    PointerButtonWithModifiers {
        window: Entity,
        button: UiPointerButton,
        pressed: bool,
        position: [f32; 2],
        time_seconds: f64,
        modifiers: UiModifiers,
    },
    PointerCancel {
        window: Entity,
    },
    PointerLeave {
        window: Entity,
    },
    WindowBlur {
        window: Entity,
    },
    Key {
        window: Entity,
        key: UiKeyCode,
        pressed: bool,
        repeat: bool,
        modifiers: UiModifiers,
    },
    Text {
        window: Entity,
        text: String,
    },
    ImePreedit {
        window: Entity,
        text: String,
        cursor_range: Option<[usize; 2]>,
    },
    ImeCommit {
        window: Entity,
        text: String,
    },
    ImeDisabled {
        window: Entity,
    },
    Wheel {
        window: Entity,
        position: [f32; 2],
        delta: [f32; 2],
        unit: UiWheelUnit,
        modifiers: UiModifiers,
    },
}

#[derive(Resource, Debug, Default)]
pub struct UiInputQueue {
    events: VecDeque<UiInputEvent>,
}

impl UiInputQueue {
    pub fn push(&mut self, event: UiInputEvent) {
        self.events.push_back(event);
    }

    pub fn extend(&mut self, events: impl IntoIterator<Item = UiInputEvent>) {
        self.events.extend(events);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiRoutedEvent {
    PointerEnter {
        window: Entity,
        target: Entity,
    },
    PointerLeave {
        window: Entity,
        target: Entity,
    },
    PointerMoved {
        window: Entity,
        target: Entity,
        position: [f32; 2],
        delta: [f32; 2],
        dragging: bool,
    },
    PointerDown {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
    },
    /// Modifier annotation for the matching legacy [`Self::PointerDown`].
    /// Consumers should handle only one of the two variants per pointer edge.
    PointerDownWithModifiers {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
        modifiers: UiModifiers,
    },
    PointerUp {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
        cancelled: bool,
    },
    /// Modifier annotation for a non-cancelled [`Self::PointerUp`].
    PointerUpWithModifiers {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
        modifiers: UiModifiers,
    },
    Click {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        click_count: u32,
        position: [f32; 2],
    },
    /// Modifier annotation for the matching legacy [`Self::Click`].
    /// Consumers should handle only one of the two variants per click.
    ClickWithModifiers {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        click_count: u32,
        position: [f32; 2],
        modifiers: UiModifiers,
    },
    PointerCancel {
        window: Entity,
        target: Entity,
    },
    Wheel {
        window: Entity,
        target: Entity,
        position: [f32; 2],
        delta: [f32; 2],
        unit: UiWheelUnit,
        modifiers: UiModifiers,
    },
    Key {
        window: Entity,
        target: Entity,
        key: UiKeyCode,
        pressed: bool,
        repeat: bool,
        modifiers: UiModifiers,
    },
    Text {
        window: Entity,
        target: Entity,
        text: String,
    },
    ImePreedit {
        window: Entity,
        target: Entity,
        text: String,
        cursor_range: Option<[usize; 2]>,
    },
    ImeCommit {
        window: Entity,
        target: Entity,
        text: String,
    },
    ImeDisabled {
        window: Entity,
        target: Entity,
    },
    FocusChanged {
        window: Entity,
        previous: Option<Entity>,
        current: Option<Entity>,
    },
}

#[derive(Resource, Debug, Default)]
pub struct UiRoutedEvents {
    /// Events are replaced on every routing pass; consumers should drain or
    /// copy them before the next update.
    pub events: Vec<UiRoutedEvent>,
}

impl UiRoutedEvents {
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

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

fn route_pointer_move(world: &mut World, window: Entity, position: [f32; 2]) {
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

fn route_pointer_button(
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

fn cancel_pointer(world: &mut World, window: Entity) {
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

fn pointer_leave(world: &mut World, window: Entity) {
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

fn window_blur(world: &mut World, window: Entity) {
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

fn update_hover(world: &mut World, window: Entity, target: Option<Entity>) {
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

fn is_pointer_capture_valid(world: &World, window: Entity, target: Entity) -> bool {
    world.get::<UiHitBox>(target).is_some() && is_interactable_in_window(world, target, window)
}

fn route_key(
    world: &mut World,
    window: Entity,
    key: UiKeyCode,
    pressed: bool,
    repeat: bool,
    modifiers: UiModifiers,
) {
    if pressed && key == UiKeyCode::Tab {
        let previous = focused(world, window);
        let next = traverse_focus(world, window, previous, modifiers.shift);
        set_focus(world, window, next);
        if previous != next {
            if let Some(target) = previous {
                emit(world, UiRoutedEvent::ImeDisabled { window, target });
            }
            emit(
                world,
                UiRoutedEvent::FocusChanged {
                    window,
                    previous,
                    current: next,
                },
            );
        }
        return;
    }
    let target =
        focused(world, window).filter(|target| is_interactable_in_window(world, *target, window));
    if let Some(target) = target {
        emit(
            world,
            UiRoutedEvent::Key {
                window,
                target,
                key,
                pressed,
                repeat,
                modifiers,
            },
        );
    }
}

fn traverse_focus(
    world: &mut World,
    window: Entity,
    current: Option<Entity>,
    backwards: bool,
) -> Option<Entity> {
    let mut query = world.query::<(Entity, &UiFocusable)>();
    let mut entries: Vec<(i32, usize, Entity)> = query
        .iter(world)
        .filter_map(|(entity, _)| {
            if !has_window_assignment(world, entity, window) {
                return None;
            }
            let tab = world.get::<UiTabIndex>(entity).map_or(0, |index| index.0);
            if tab < 0 {
                return None;
            }
            let order = hierarchy_order(world, entity);
            Some((tab, order, entity))
        })
        .collect();
    entries.sort_by_key(|(tab, order, _)| (*tab, *order));
    if entries.is_empty() {
        return None;
    }
    let position = entries
        .iter()
        .position(|(_, _, entity)| Some(*entity) == current);
    let next = match (position, backwards) {
        (None, false) => 0,
        (None, true) => entries.len() - 1,
        (Some(at), false) => (at + 1) % entries.len(),
        (Some(0), true) => entries.len() - 1,
        (Some(at), true) => at - 1,
    };
    Some(entries[next].2)
}

fn hierarchy_order(world: &World, entity: Entity) -> usize {
    let mut path = Vec::new();
    let mut current = Some(entity);
    while let Some(at) = current {
        path.push(at);
        current = world.get::<crate::UiParent>(at).map(|parent| parent.0);
    }
    path.reverse();
    let mut order = 0usize;
    for (depth, item) in path.iter().enumerate() {
        if depth == 0 {
            order = order
                .wrapping_mul(65_537)
                .wrapping_add(item.to_bits() as usize);
        } else {
            let parent = path[depth - 1];
            let child_index = world
                .get::<crate::UiChildren>(parent)
                .and_then(|children| children.0.iter().position(|child| child == item))
                .unwrap_or(item.to_bits() as usize);
            order = order.wrapping_mul(65_537).wrapping_add(child_index);
        }
    }
    order
}

fn set_focus(world: &mut World, window: Entity, target: Option<Entity>) {
    let mut focus = world.resource_mut::<UiFocus>();
    let state = focus.state_mut(window);
    if state.focused != target {
        state.selection = None;
    }
    state.focused = target;
    focus.set_active(window);
}

fn focused(world: &World, window: Entity) -> Option<Entity> {
    world
        .resource::<UiFocus>()
        .state(window)
        .and_then(|state| state.focused)
}

fn hit_test(world: &mut World, window: Entity, position: [f32; 2]) -> Option<Entity> {
    #[cfg(test)]
    if let Some(mut validation) = world.get_resource_mut::<UiHoverValidation>() {
        validation.hit_tests += 1;
    }
    let mut query = world.query::<(Entity, &UiHitBox)>();
    let mut candidates: Vec<(i32, usize, Entity)> = query
        .iter(world)
        .filter(|&(entity, bounds)| {
            bounds.window.is_none_or(|owner| owner == window)
                && is_interactable_in_window(world, entity, window)
                && bounds.contains(position)
        })
        .map(|(entity, bounds)| (bounds.z_index, hierarchy_order(world, entity), entity))
        .collect();
    candidates.sort_by_key(|(z, order, entity)| (*z, *order, entity.to_bits()));
    candidates.last().map(|(_, _, entity)| *entity)
}

fn validate_states(world: &mut World) {
    let hit_generation = world
        .resource_mut::<crate::UiDirty>()
        .take(crate::UiDirtyDomain::HitTest)
        .generation;
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
        let focus_invalid =
            focused.is_some_and(|entity| !is_interactable_in_window(world, entity, window));
        let capture_invalid =
            captured.is_some_and(|entity| !is_pointer_capture_valid(world, window, entity));
        let click_invalid =
            last_click.is_some_and(|last| !is_interactable_in_window(world, last.target, window));
        let hover_invalid =
            hovered.is_some_and(|entity| !is_interactable_in_window(world, entity, window));
        let needs_hover_refresh = pointer_position.is_some_and(|position| {
            world
                .resource::<UiHoverValidation>()
                .windows
                .get(&window)
                .is_none_or(|&(previous, generation)| {
                    previous != position || generation != hit_generation
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
        let invalid_focus_target = if focus_invalid {
            let mut focus = world.resource_mut::<UiFocus>();
            let Some(state) = focus.windows.get_mut(&window) else {
                continue;
            };
            let target = state.focused.take();
            state.selection = None;
            if focus.active_window == Some(window) {
                focus.sync_active();
            }
            target
        } else {
            None
        };
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

fn emit(world: &mut World, event: UiRoutedEvent) {
    if let Some(mut events) = world.get_resource_mut::<UiRoutedEvents>() {
        events.events.push(event);
    }
}

fn distance_squared(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

// Keep the rectangle type available as part of this module's routing contract.
const _: Option<UiRect> = None;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UiChildren, UiNode, UiParent, UiVisible, cleanup_removed_node, set_parent};

    fn setup() -> (World, Entity, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<UiFocus>();
        world.init_resource::<UiInputQueue>();
        world.init_resource::<UiRoutedEvents>();
        let window = world.spawn_empty().id();
        let root = world
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id();
        let button = world
            .spawn((
                UiNode,
                UiFocusable,
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(10.0, 10.0, 20.0, 20.0),
                    clip: Some(UiRect::new(0.0, 0.0, 25.0, 25.0)),
                    z_index: 1,
                },
            ))
            .id();
        set_parent(&mut world, button, root).unwrap();
        (world, window, root, button)
    }

    #[test]
    fn stationary_pointer_retests_only_after_hit_geometry_changes() {
        let mut app = bevy_app::App::new();
        app.add_plugins(crate::CorePlugin);
        let window = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerMove {
                window,
                position: [20.0, 20.0],
                time_seconds: 0.0,
            });
        app.update();
        let first = app.world().resource::<UiHoverValidation>().hit_tests;
        assert_eq!(app.world().resource::<UiFocus>().hovered, Some(target));

        app.update();
        assert_eq!(app.world().resource::<UiHoverValidation>().hit_tests, first);

        app.world_mut().get_mut::<UiHitBox>(target).unwrap().rect =
            UiRect::new(200.0, 200.0, 100.0, 100.0);
        app.update();
        assert!(app.world().resource::<UiHoverValidation>().hit_tests > first);
        assert_eq!(app.world().resource::<UiFocus>().hovered, None);
    }

    #[test]
    fn pointer_capture_focus_and_click_route_to_the_topmost_target() {
        let (mut world, window, _root, button) = setup();
        world.resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 0.0,
            },
            UiInputEvent::PointerMove {
                window,
                position: [70.0, 70.0],
                time_seconds: 0.1,
            },
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position: [70.0, 70.0],
                time_seconds: 0.2,
            },
        ]);
        process_ui_input(&mut world);
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerDown {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0]
        }));
        assert!(events.iter().any(|event| matches!(event, UiRoutedEvent::PointerMoved { target, dragging: true, .. } if *target == button)));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, UiRoutedEvent::Click { .. }))
        );
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().focused,
            Some(button)
        );
    }

    #[test]
    fn modifier_pointer_events_preserve_legacy_delivery_and_annotate_edges() {
        let (mut world, window, _root, button) = setup();
        let modifiers = UiModifiers {
            shift: true,
            control: true,
            alt: false,
            super_key: false,
        };
        world.resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButtonWithModifiers {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 0.0,
                modifiers,
            },
            UiInputEvent::PointerButtonWithModifiers {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position: [15.0, 15.0],
                time_seconds: 0.1,
                modifiers,
            },
        ]);

        process_ui_input(&mut world);

        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerDown {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
        }));
        assert!(events.contains(&UiRoutedEvent::PointerDownWithModifiers {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
            modifiers,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
            cancelled: false,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUpWithModifiers {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
            modifiers,
        }));
        assert!(events.contains(&UiRoutedEvent::Click {
            window,
            target: button,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [15.0, 15.0],
        }));
        assert!(events.contains(&UiRoutedEvent::ClickWithModifiers {
            window,
            target: button,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [15.0, 15.0],
            modifiers,
        }));
    }

    #[test]
    fn tab_skips_hidden_and_disabled_controls_and_wraps_within_its_window() {
        let (mut world, window, _root, first) = setup();
        let second = world
            .spawn((
                UiNode,
                UiFocusable,
                UiTabIndex(1),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(40.0, 10.0, 10.0, 10.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id();
        let hidden = world.spawn((UiNode, UiFocusable, UiVisible(false))).id();
        set_parent(&mut world, second, _root).unwrap();
        set_parent(&mut world, hidden, _root).unwrap();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(first);
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            });
        process_ui_input(&mut world);
        assert_eq!(focused(&world, window), Some(second));
    }

    #[test]
    fn tab_traversal_never_focuses_a_control_from_another_window() {
        let (mut world, window_a, root_a, first) = setup();
        let window_b = world.spawn_empty().id();
        let root_b = world
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window_b),
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id();
        let next_a = world
            .spawn((
                UiNode,
                UiFocusable,
                UiTabIndex(1),
                UiHitBox {
                    window: Some(window_a),
                    rect: UiRect::new(40.0, 10.0, 10.0, 10.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        let other_window = world
            .spawn((
                UiNode,
                UiFocusable,
                UiTabIndex(1),
                UiHitBox {
                    window: Some(window_b),
                    rect: UiRect::new(10.0, 10.0, 10.0, 10.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        set_parent(&mut world, next_a, root_a).unwrap();
        set_parent(&mut world, other_window, root_b).unwrap();
        world.resource_mut::<UiFocus>().state_mut(window_a).focused = Some(first);
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window: window_a,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            });

        process_ui_input(&mut world);

        assert_eq!(focused(&world, window_a), Some(next_a));
        assert_eq!(focused(&world, window_b), None);
        assert!(world.resource::<UiRoutedEvents>().events.iter().any(
            |event| matches!(event, UiRoutedEvent::FocusChanged { window, current: Some(target), .. } if *window == window_a && *target == next_a)
        ));
    }

    #[test]
    fn tab_traversal_skips_focusables_without_surface_geometry() {
        let (mut world, window, root, first) = setup();
        let unassigned = world.spawn((UiNode, UiFocusable, UiTabIndex(1))).id();
        let assigned = world
            .spawn((
                UiNode,
                UiFocusable,
                UiTabIndex(2),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(40.0, 10.0, 10.0, 10.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        set_parent(&mut world, assigned, root).unwrap();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(first);
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            });

        process_ui_input(&mut world);

        assert_eq!(focused(&world, window), Some(assigned));
        assert!(is_interactable_in_window(&world, unassigned, window));
        assert!(!crate::has_window_assignment(&world, unassigned, window));
    }

    #[test]
    fn overlapping_surfaces_are_hit_tested_independently() {
        let (mut world, window, _root, button) = setup();
        let other_window = world.spawn_empty().id();
        let overlay = world
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(other_window),
                    rect: UiRect::new(10.0, 10.0, 20.0, 20.0),
                    clip: None,
                    z_index: 100,
                },
            ))
            .id();
        assert_eq!(hit_test(&mut world, window, [15.0, 15.0]), Some(button));
        assert_eq!(
            hit_test(&mut world, other_window, [15.0, 15.0]),
            Some(overlay)
        );
    }

    #[test]
    fn removed_or_disabled_capture_is_cancelled_and_children_detach_on_cleanup() {
        let (mut world, window, root, button) = setup();
        world.resource_mut::<UiFocus>().state_mut(window).captured = Some(button);
        world.entity_mut(button).insert(crate::UiEnabled(false));
        process_ui_input(&mut world);
        assert!(world.resource::<UiRoutedEvents>().events.iter().any(|event| matches!(event, UiRoutedEvent::PointerCancel { target, .. } if *target == button)));
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().captured,
            None
        );
        cleanup_removed_node(&mut world, button);
        assert!(world.get::<UiChildren>(root).unwrap().0.is_empty());
        assert!(world.get::<UiParent>(button).is_none());
    }

    #[test]
    fn disabled_ancestor_cancels_capture_focus_and_stationary_hover_completely() {
        let (mut world, window, root, button) = setup();
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Secondary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 1.0,
            });
        process_ui_input(&mut world);
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().captured,
            Some(button)
        );
        assert_eq!(
            world
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .captured_button,
            Some(UiPointerButton::Secondary)
        );
        world.resource_mut::<UiRoutedEvents>().clear();

        world.entity_mut(root).insert(crate::UiEnabled(false));
        process_ui_input(&mut world);

        let state = world.resource::<UiFocus>().state(window).unwrap();
        assert_eq!(state.focused, None);
        assert_eq!(state.hovered, None);
        assert_eq!(state.captured, None);
        assert_eq!(state.selection, None);
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerCancel {
            window,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window,
            target: button,
            button: UiPointerButton::Secondary,
            position: [15.0, 15.0],
            cancelled: true,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerLeave {
            window,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::ImeDisabled {
            window,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::FocusChanged {
            window,
            previous: Some(button),
            current: None,
        }));
    }

    #[test]
    fn hidden_target_cancels_focus_capture_and_hover_on_stationary_pointer() {
        let (mut world, window, root, button) = setup();
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 1.0,
            });
        process_ui_input(&mut world);
        world.resource_mut::<UiRoutedEvents>().clear();

        world.entity_mut(button).insert(UiVisible(false));
        process_ui_input(&mut world);

        let state = world.resource::<UiFocus>().state(window).unwrap();
        assert_eq!(state.focused, None);
        assert_eq!(state.hovered, Some(root));
        assert_eq!(state.captured, None);
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerCancel {
            window,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
            cancelled: true,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerLeave {
            window,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerEnter {
            window,
            target: root,
        }));
        assert!(events.contains(&UiRoutedEvent::FocusChanged {
            window,
            previous: Some(button),
            current: None,
        }));
    }

    #[test]
    fn direct_despawn_cancels_state_even_without_cleanup_helper() {
        let (mut world, window, root, button) = setup();
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 1.5,
            });
        process_ui_input(&mut world);
        world.resource_mut::<UiRoutedEvents>().clear();

        world.despawn(button);
        process_ui_input(&mut world);

        let state = world.resource::<UiFocus>().state(window).unwrap();
        assert_eq!(state.focused, None);
        assert_eq!(state.captured, None);
        assert_eq!(state.hovered, Some(root));
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerCancel {
            window,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window,
            target: button,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
            cancelled: true,
        }));
        assert!(events.contains(&UiRoutedEvent::FocusChanged {
            window,
            previous: Some(button),
            current: None,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerEnter {
            window,
            target: root,
        }));
    }

    #[test]
    fn moving_a_captured_control_to_another_window_cancels_old_surface_state() {
        let (mut world, window_a, root_a, button) = setup();
        let window_b = world.spawn_empty().id();
        let root_b = world
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window_b),
                    rect: UiRect::new(200.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id();
        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerButton {
                window: window_a,
                button: UiPointerButton::Middle,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 2.0,
            });
        process_ui_input(&mut world);
        world.resource_mut::<UiRoutedEvents>().clear();

        set_parent(&mut world, button, root_b).unwrap();
        world.entity_mut(button).insert(UiHitBox {
            window: Some(window_b),
            rect: UiRect::new(210.0, 10.0, 20.0, 20.0),
            clip: None,
            z_index: 1,
        });
        process_ui_input(&mut world);

        assert_eq!(
            world.resource::<UiFocus>().state(window_a).unwrap().focused,
            None
        );
        assert_eq!(
            world.resource::<UiFocus>().state(window_a).unwrap().hovered,
            Some(root_a)
        );
        assert_eq!(
            world
                .resource::<UiFocus>()
                .state(window_a)
                .unwrap()
                .captured,
            None
        );
        assert!(is_interactable_in_window(&world, button, window_b));
        assert!(!is_interactable_in_window(&world, button, window_a));
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerCancel {
            window: window_a,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window: window_a,
            target: button,
            button: UiPointerButton::Middle,
            position: [15.0, 15.0],
            cancelled: true,
        }));
        assert!(events.contains(&UiRoutedEvent::FocusChanged {
            window: window_a,
            previous: Some(button),
            current: None,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerLeave {
            window: window_a,
            target: button,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerEnter {
            window: window_a,
            target: root_a,
        }));
        assert!(!events.iter().any(
            |event| matches!(event, UiRoutedEvent::PointerCancel { window, .. } if *window == window_b)
        ));
    }

    #[test]
    fn window_blur_cancels_only_that_windows_capture_and_focus() {
        let (mut world, window_a, _root_a, button_a) = setup();
        let window_b = world.spawn_empty().id();
        let button_b = world
            .spawn((
                UiNode,
                UiFocusable,
                UiHitBox {
                    window: Some(window_b),
                    rect: UiRect::new(10.0, 10.0, 20.0, 20.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        world.resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButton {
                window: window_a,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 3.0,
            },
            UiInputEvent::PointerButton {
                window: window_b,
                button: UiPointerButton::Secondary,
                pressed: true,
                position: [15.0, 15.0],
                time_seconds: 3.0,
            },
        ]);
        process_ui_input(&mut world);
        world.resource_mut::<UiRoutedEvents>().clear();

        world
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::WindowBlur { window: window_a });
        process_ui_input(&mut world);

        assert_eq!(
            world
                .resource::<UiFocus>()
                .state(window_a)
                .unwrap()
                .captured,
            None
        );
        assert_eq!(
            world.resource::<UiFocus>().state(window_a).unwrap().focused,
            None
        );
        assert_eq!(
            world
                .resource::<UiFocus>()
                .state(window_b)
                .unwrap()
                .captured,
            Some(button_b)
        );
        assert_eq!(
            world.resource::<UiFocus>().state(window_b).unwrap().focused,
            Some(button_b)
        );
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerCancel {
            window: window_a,
            target: button_a,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window: window_a,
            target: button_a,
            button: UiPointerButton::Primary,
            position: [15.0, 15.0],
            cancelled: true,
        }));
        assert!(!events.iter().any(
            |event| matches!(event, UiRoutedEvent::PointerCancel { window, .. } if *window == window_b)
        ));
    }

    #[test]
    fn cleanup_cancellation_events_survive_until_the_next_routing_pass() {
        let (mut world, window, root, button) = setup();
        {
            let mut focus = world.resource_mut::<UiFocus>();
            let state = focus.state_mut(window);
            state.focused = Some(button);
            state.hovered = Some(button);
            state.captured = Some(button);
            state.captured_button = Some(UiPointerButton::Middle);
            state.pointer_position = Some([18.0, 17.0]);
            focus.set_active(window);
        }
        cleanup_removed_node(&mut world, button);
        assert!(world.resource::<UiRoutedEvents>().events.is_empty());
        process_ui_input(&mut world);
        assert!(world.get::<UiChildren>(root).unwrap().0.is_empty());
        let events = &world.resource::<UiRoutedEvents>().events;
        assert!(events.contains(&UiRoutedEvent::PointerCancel {
            window,
            target: button
        }));
        assert!(events.contains(&UiRoutedEvent::PointerUp {
            window,
            target: button,
            button: UiPointerButton::Middle,
            position: [18.0, 17.0],
            cancelled: true,
        }));
        assert!(events.contains(&UiRoutedEvent::PointerLeave {
            window,
            target: button
        }));
        assert!(events.contains(&UiRoutedEvent::FocusChanged {
            window,
            previous: Some(button),
            current: None,
        }));
    }
}

//! Convert AppKit's positioned gestures and recover missed pointer edges.

use bbui_core::{
    UiGesturePhase, UiInputEvent, UiInputQueue, UiPointerButton, UiTrackpadGesture, UiTrackpadKind,
};
use bbui_macos_input::{MacInputKind, MacInputSample, TrackpadMonitor, TrackpadSample};
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButton, MouseButtonInput};
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow, RawHandleWrapper, WindowEvent};
use std::time::{Duration, Instant};

use crate::cursor_bridge::CursorCaptureState;
use crate::world_input::WorldPointerState;

#[derive(Resource, Default)]
pub(crate) struct MacInputFrame {
    pub(crate) available: bool,
    pub(crate) recovered_drag_window: Option<Entity>,
    recovered_release_window: Option<Entity>,
    recovered_releases: [bool; 3],
}

impl MacInputFrame {
    pub(crate) fn recovered_release(&self, window: Entity, button: MouseButton) -> bool {
        self.recovered_release_window == Some(window)
            && button_index(button).is_some_and(|index| self.recovered_releases[index])
    }
}

#[derive(Resource, Default)]
pub(crate) struct MacPointerState {
    down: [bool; 3],
    dragged: [bool; 3],
    pressed_at: [Option<Instant>; 3],
    pending_up: [bool; 3],
    last_cursor_position: Option<Vec2>,
}

fn button_index(button: MouseButton) -> Option<usize> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Right => Some(1),
        MouseButton::Middle => Some(2),
        _ => None,
    }
}

fn pointer_button(index: usize) -> UiPointerButton {
    match index {
        0 => UiPointerButton::Primary,
        1 => UiPointerButton::Secondary,
        _ => UiPointerButton::Middle,
    }
}

fn bevy_button(index: usize) -> MouseButton {
    [MouseButton::Left, MouseButton::Right, MouseButton::Middle][index]
}

/// Defer an observed native release by one frame if a drag moved in that frame.
fn missing_releases(
    down: &mut [bool; 3],
    pending: &mut [bool; 3],
    observed: [bool; 3],
    fallback: [bool; 3],
    pointer_moved: bool,
) -> Vec<usize> {
    let mut recovered = Vec::new();
    for index in 0..3 {
        pending[index] |= observed[index];
        if !down[index] {
            pending[index] = false;
        } else if (pending[index] && (!observed[index] || !pointer_moved)) || fallback[index] {
            down[index] = false;
            pending[index] = false;
            recovered.push(index);
        }
    }
    recovered
}

type NativePrimaryWindow<'w, 's> =
    Query<'w, 's, (Entity, &'static Window, &'static RawHandleWrapper), With<PrimaryWindow>>;

#[derive(SystemParam)]
pub(crate) struct NativePointerMessages<'w, 's> {
    mouse_events: MessageReader<'w, 's, MouseButtonInput>,
    cursor_events: MessageReader<'w, 's, CursorMoved>,
    mouse_buttons: Option<ResMut<'w, ButtonInput<MouseButton>>>,
    window_events: Option<ResMut<'w, Messages<WindowEvent>>>,
    capture: Res<'w, CursorCaptureState>,
    world_pointers: Res<'w, WorldPointerState>,
    time: Option<Res<'w, Time>>,
}

fn logical_position(position: [f64; 2], window_height: f32) -> Option<[f32; 2]> {
    let [x, y] = position;
    (x.is_finite() && y.is_finite() && window_height.is_finite())
        .then_some([x as f32, window_height - y as f32])
}

fn convert(sample: TrackpadSample, window: Entity, height: f32) -> Option<UiInputEvent> {
    match sample {
        TrackpadSample::Scroll {
            position,
            delta,
            began,
            momentum_ended,
            phased,
            command,
            ..
        } => {
            let position = logical_position(position, height)?;
            if !delta.iter().all(|value| value.is_finite()) {
                return None;
            }
            let phase = if momentum_ended {
                UiGesturePhase::Ended
            } else if began {
                UiGesturePhase::Started
            } else {
                UiGesturePhase::Updated
            };
            // Command/Control scroll zooms around the event's native position.
            let scale = if command {
                ((delta[0] + delta[1]) * 0.01).exp() as f32
            } else {
                1.0
            };
            Some(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
                window,
                position,
                delta: if command {
                    [0.0, 0.0]
                } else {
                    [delta[0] as f32, delta[1] as f32]
                },
                scale,
                kind: UiTrackpadKind::Scroll,
                phase,
                phased,
            }))
        }
        TrackpadSample::Magnify {
            position,
            delta,
            began,
            ended,
            cancelled,
            phased,
        } => {
            let position = logical_position(position, height)?;
            if !delta.is_finite() || delta <= -1.0 {
                return None;
            }
            let phase = if cancelled {
                UiGesturePhase::Cancelled
            } else if ended {
                UiGesturePhase::Ended
            } else if began {
                UiGesturePhase::Started
            } else {
                UiGesturePhase::Updated
            };
            Some(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
                window,
                position,
                delta: [0.0, 0.0],
                scale: (1.0 + delta) as f32,
                kind: UiTrackpadKind::Magnify,
                phase,
                phased,
            }))
        }
    }
}

pub(crate) fn bridge_native_macos_input(
    primary: NativePrimaryWindow,
    mut monitor: NonSendMut<TrackpadMonitor>,
    mut frame: ResMut<MacInputFrame>,
    mut pointer: ResMut<MacPointerState>,
    mut messages: NativePointerMessages,
    mut queue: ResMut<UiInputQueue>,
) {
    frame.recovered_drag_window = None;
    frame.recovered_release_window = None;
    frame.recovered_releases = [false; 3];
    let Ok((window, data, raw_handle)) = primary.single() else {
        frame.available = false;
        return;
    };
    let primary_window_number = TrackpadMonitor::window_number(raw_handle.get_window_handle());
    let now = Instant::now();
    let time_seconds = messages.time.as_deref().map_or(0.0, Time::elapsed_secs_f64);
    let mut pressed_this_frame = [false; 3];
    for event in messages
        .mouse_events
        .read()
        .filter(|event| event.window == window)
    {
        let Some(index) = button_index(event.button) else {
            continue;
        };
        if event.state == ButtonState::Pressed {
            pointer.down[index] = true;
            pointer.dragged[index] = false;
            pointer.pressed_at[index] = Some(now);
            pointer.pending_up[index] = false;
            pressed_this_frame[index] = true;
        } else {
            pointer.down[index] = false;
            pointer.dragged[index] = false;
            pointer.pressed_at[index] = None;
            pointer.pending_up[index] = false;
        }
    }
    let mut pointer_moved = false;
    for event in messages
        .cursor_events
        .read()
        .filter(|event| event.window == window)
    {
        pointer_moved |= event
            .delta
            .is_some_and(|delta| delta.length_squared() > 0.25)
            || pointer
                .last_cursor_position
                .is_some_and(|last| (event.position - last).length_squared() > 0.25);
        pointer.last_cursor_position = Some(event.position);
    }
    if pointer_moved {
        for (index, pressed) in pressed_this_frame.iter().enumerate() {
            if pointer.down[index] && !pressed {
                pointer.dragged[index] = true;
            }
        }
    }
    let samples = monitor.drain();
    frame.available = monitor.available() && primary_window_number.is_some();
    let height = data.resolution.height();
    let mut observed_up = [false; 3];
    let mut raw_drag_position = None;
    for MacInputSample {
        window_number,
        kind,
    } in samples
    {
        if Some(window_number) != primary_window_number {
            continue;
        }
        match kind {
            MacInputKind::Trackpad(sample) if data.focused => {
                if let Some(event) = convert(sample, window, height) {
                    queue.push(event);
                }
            }
            MacInputKind::Trackpad(_) => {}
            MacInputKind::MouseUp(index) => observed_up[usize::from(index)] = true,
            MacInputKind::MouseDrag { button, position } if pointer.down[usize::from(button)] => {
                raw_drag_position = logical_position(position, height).map(Vec2::from_array);
            }
            MacInputKind::MouseDrag { .. } => {}
        }
    }
    // A Bevy cursor update from this frame is authoritative. Replaying an
    // older AppKit drag sample after it would move the pointer backwards.
    if let Some(position) = raw_drag_position.filter(|_| !pointer_moved) {
        let moved = pointer
            .last_cursor_position
            .is_none_or(|last| (position - last).length_squared() > 0.25);
        if moved {
            pointer.last_cursor_position = Some(position);
            pointer_moved = true;
            if messages.capture.active.is_none()
                && messages.world_pointers.hover_target(window).is_none()
            {
                frame.recovered_drag_window = Some(window);
                queue.push(UiInputEvent::PointerMove {
                    window,
                    position: position.to_array(),
                    time_seconds,
                });
            }
        }
    }
    let native_buttons = (!monitor.available()).then(TrackpadMonitor::pressed_mouse_buttons);
    let mut fallback = [false; 3];
    for (index, allowed) in fallback.iter_mut().enumerate() {
        *allowed = !data.focused
            || (native_buttons.is_some()
                && !pointer.dragged[index]
                && !pressed_this_frame[index]
                && !pointer_moved
                && pointer.pressed_at[index]
                    .is_some_and(|at| now.duration_since(at) >= Duration::from_millis(50))
                && native_buttons.is_some_and(|buttons| buttons & (1 << index) == 0));
    }
    let mut down = pointer.down;
    let mut pending = pointer.pending_up;
    let recovered = missing_releases(
        &mut down,
        &mut pending,
        observed_up,
        fallback,
        pointer_moved,
    );
    pointer.down = down;
    pointer.pending_up = pending;
    for index in recovered {
        frame.recovered_release_window = Some(window);
        frame.recovered_releases[index] = true;
        if let Some(buttons) = messages.mouse_buttons.as_deref_mut() {
            buttons.release(bevy_button(index));
        }
        if let Some(events) = messages.window_events.as_deref_mut() {
            events.write(WindowEvent::MouseButtonInput(MouseButtonInput {
                button: bevy_button(index),
                state: ButtonState::Released,
                window,
            }));
        }
        if messages.world_pointers.hover_target(window).is_none() {
            queue.push(UiInputEvent::PointerButton {
                window,
                button: pointer_button(index),
                pressed: false,
                position: pointer
                    .last_cursor_position
                    .or_else(|| data.cursor_position())
                    .unwrap_or_default()
                    .to_array(),
                time_seconds,
            });
        }
    }
}

/// Fan out recovered releases through the same Bevy message read by app code.
/// This runs after native sampling and before world-space and BBUI input routing.
pub(crate) fn publish_recovered_mouse_releases(
    frame: Res<MacInputFrame>,
    mut events: MessageWriter<MouseButtonInput>,
) {
    let Some(window) = frame.recovered_release_window else {
        return;
    };
    for (index, recovered) in frame.recovered_releases.iter().enumerate() {
        if *recovered {
            events.write(MouseButtonInput {
                button: bevy_button(index),
                state: ButtonState::Released,
                window,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_magnify_keeps_position_and_incremental_scale() {
        let window = Entity::PLACEHOLDER;
        let event = convert(
            TrackpadSample::Magnify {
                position: [88.0, 26.0],
                delta: 0.125,
                began: true,
                ended: false,
                cancelled: false,
                phased: true,
            },
            window,
            200.0,
        );
        assert!(
            matches!(event, Some(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
            position: [88.0, 174.0], scale, phase: UiGesturePhase::Started,
            kind: UiTrackpadKind::Magnify, ..
        })) if (scale - 1.125).abs() < 0.0001)
        );
    }

    #[test]
    fn native_release_defers_past_drag_and_recovers_on_next_frame() {
        let mut down = [true, false, false];
        let mut pending = [false; 3];
        assert!(
            missing_releases(
                &mut down,
                &mut pending,
                [true, false, false],
                [false; 3],
                true,
            )
            .is_empty()
        );
        assert_eq!(
            missing_releases(&mut down, &mut pending, [false; 3], [false; 3], false),
            vec![0]
        );
        assert!(!down[0]);
    }

    #[test]
    fn native_release_does_not_drop_other_held_buttons() {
        let mut down = [true, true, false];
        let mut pending = [false; 3];
        assert_eq!(
            missing_releases(
                &mut down,
                &mut pending,
                [true, false, false],
                [false; 3],
                false,
            ),
            vec![0]
        );
        assert_eq!(down, [false, true, false]);
    }

    #[test]
    fn snapshot_fallback_never_releases_a_still_held_button() {
        let mut down = [true, false, false];
        let mut pending = [false; 3];
        assert!(
            missing_releases(&mut down, &mut pending, [false; 3], [false; 3], false,).is_empty()
        );
        assert_eq!(
            missing_releases(
                &mut down,
                &mut pending,
                [false; 3],
                [true, false, false],
                false,
            ),
            vec![0]
        );
    }
}

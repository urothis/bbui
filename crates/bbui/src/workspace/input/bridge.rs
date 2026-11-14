//! Keep a tab drag alive when routing switches to another native window.

use super::*;
use bbui_core::{UiInputEvent, UiInputQueue};

fn push_release(
    gesture: &mut WorkspaceGesture,
    window: Entity,
    position: [f32; 2],
    modifiers: Option<bbui_core::UiModifiers>,
) {
    gesture.cross_window_events.push(UiRoutedEvent::PointerUp {
        window,
        target: window,
        button: UiPointerButton::Primary,
        position,
        cancelled: false,
    });
    if let Some(modifiers) = modifiers {
        gesture
            .cross_window_events
            .push(UiRoutedEvent::PointerUpWithModifiers {
                window,
                target: window,
                button: UiPointerButton::Primary,
                position,
                modifiers,
            });
    }
}

pub(crate) fn bridge_cross_window_input(
    queue: Res<UiInputQueue>,
    windows: Query<&Window>,
    mut gesture: ResMut<WorkspaceGesture>,
) {
    let Some(Gesture::Tab { window: source, .. }) = &gesture.active else {
        return;
    };
    let source = *source;
    for event in queue.iter() {
        match event {
            UiInputEvent::PointerMove {
                window, position, ..
            } if *window != source => {
                gesture.cross_window_hover = Some(*window);
                gesture
                    .cross_window_events
                    .push(UiRoutedEvent::PointerMoved {
                        window: *window,
                        target: *window,
                        position: *position,
                        delta: [0.0, 0.0],
                        dragging: true,
                    });
            }
            UiInputEvent::PointerLeave { window } if *window != source => {
                if gesture.cross_window_hover == Some(*window) {
                    gesture.cross_window_hover = None;
                }
            }
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position,
                ..
            } => {
                if *window != source {
                    push_release(&mut gesture, *window, *position, None);
                } else if *position == [-1.0, -1.0]
                    && let Some(destination) = gesture.cross_window_hover
                    && let Some(position) = windows
                        .get(destination)
                        .ok()
                        .and_then(Window::cursor_position)
                {
                    push_release(&mut gesture, destination, position.to_array(), None);
                }
            }
            UiInputEvent::PointerButtonWithModifiers {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position,
                modifiers,
                ..
            } => {
                if *window != source {
                    push_release(&mut gesture, *window, *position, Some(*modifiers));
                } else if *position == [-1.0, -1.0]
                    && let Some(destination) = gesture.cross_window_hover
                    && let Some(position) = windows
                        .get(destination)
                        .ok()
                        .and_then(Window::cursor_position)
                {
                    push_release(
                        &mut gesture,
                        destination,
                        position.to_array(),
                        Some(*modifiers),
                    );
                }
            }
            _ => {}
        }
    }
}

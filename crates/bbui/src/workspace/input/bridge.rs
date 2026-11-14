//! Keep a tab drag alive when routing switches to another native window.

use super::*;
use bbui_core::{UiInputEvent, UiInputQueue};

pub(crate) fn bridge_cross_window_input(
    queue: Res<UiInputQueue>,
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
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position,
                ..
            } if *window != source => {
                gesture.cross_window_events.push(UiRoutedEvent::PointerUp {
                    window: *window,
                    target: *window,
                    button: UiPointerButton::Primary,
                    position: *position,
                    cancelled: false,
                });
            }
            UiInputEvent::PointerButtonWithModifiers {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position,
                modifiers,
                ..
            } if *window != source => {
                gesture
                    .cross_window_events
                    .push(UiRoutedEvent::PointerUpWithModifiers {
                        window: *window,
                        target: *window,
                        button: UiPointerButton::Primary,
                        position: *position,
                        modifiers: *modifiers,
                    });
            }
            _ => {}
        }
    }
}

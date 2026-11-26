//! Translate Bevy window messages into BBUI's platform-neutral input queue.

use crate::cursor_bridge::{CursorCaptureState, logical_pointer_motion};
use crate::world_input::WorldPointerState;
use bbui_core::{
    UiInputEvent, UiInputQueue, UiKeyCode, UiModifiers, UiPointerButton, UiTouchPhase, UiWheelUnit,
};
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::prelude::*;
use bevy::window::{CursorLeft, CursorMoved, Ime, WindowFocused};
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

fn other_key_code(key: KeyCode) -> UiKeyCode {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    UiKeyCode::Other(hasher.finish() as u32)
}

#[derive(Resource, Default)]
pub struct ImeBridgeState {
    active: HashSet<Entity>,
}

#[derive(Resource, Default)]
pub struct TouchBridgeState {
    active: HashMap<Entity, u64>,
}

#[derive(SystemParam)]
pub struct InputMessages<'w, 's> {
    world_pointers: Res<'w, WorldPointerState>,
    moves: MessageReader<'w, 's, CursorMoved>,
    motions: MessageReader<'w, 's, MouseMotion>,
    exits: MessageReader<'w, 's, CursorLeft>,
    buttons: MessageReader<'w, 's, MouseButtonInput>,
    wheels: MessageReader<'w, 's, MouseWheel>,
    keys: MessageReader<'w, 's, KeyboardInput>,
    ime: MessageReader<'w, 's, Ime>,
    focus: MessageReader<'w, 's, WindowFocused>,
    touches: MessageReader<'w, 's, TouchInput>,
    touch_state: ResMut<'w, TouchBridgeState>,
}

pub fn bridge_bevy_input(
    mut messages: InputMessages,
    windows: Query<&Window>,
    key_state: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut ime_state: ResMut<ImeBridgeState>,
    mut capture: ResMut<CursorCaptureState>,
    mut queue: ResMut<UiInputQueue>,
) {
    let now = time.elapsed_secs_f64();
    for event in messages.moves.read() {
        if messages.world_pointers.hover_target(event.window).is_some() {
            continue;
        }
        if capture
            .active
            .as_ref()
            .is_some_and(|active| active.physical_window == event.window)
        {
            continue;
        }
        queue.push(UiInputEvent::PointerMove {
            window: event.window,
            position: event.position.to_array(),
            time_seconds: now,
        });
    }
    if let Some(active) = capture
        .active
        .as_mut()
        .filter(|active| active.physical_window == active.window)
    {
        let scale_factor = windows
            .get(active.physical_window)
            .map_or(1.0, Window::scale_factor);
        for event in messages.motions.read() {
            active.virtual_position += logical_pointer_motion(event.delta, scale_factor);
            queue.push(UiInputEvent::PointerMove {
                window: active.window,
                position: active.virtual_position.to_array(),
                time_seconds: now,
            });
        }
    } else {
        // Advance the reader while no scrub is active so old motion cannot replay later.
        for _ in messages.motions.read() {}
    }
    for event in messages.exits.read() {
        queue.push(UiInputEvent::PointerLeave {
            window: event.window,
        });
    }
    let modifiers = UiModifiers {
        shift: key_state.pressed(KeyCode::ShiftLeft) || key_state.pressed(KeyCode::ShiftRight),
        control: key_state.pressed(KeyCode::ControlLeft)
            || key_state.pressed(KeyCode::ControlRight),
        alt: key_state.pressed(KeyCode::AltLeft) || key_state.pressed(KeyCode::AltRight),
        super_key: key_state.pressed(KeyCode::SuperLeft) || key_state.pressed(KeyCode::SuperRight),
    };
    for event in messages.touches.read() {
        let position = event.position.to_array();
        match event.phase {
            TouchPhase::Started if !messages.touch_state.active.contains_key(&event.window) => {
                messages.touch_state.active.insert(event.window, event.id);
                queue.push(UiInputEvent::PointerMove {
                    window: event.window,
                    position,
                    time_seconds: now,
                });
                queue.push(UiInputEvent::PointerButtonWithModifiers {
                    window: event.window,
                    button: UiPointerButton::Primary,
                    pressed: true,
                    position,
                    time_seconds: now,
                    modifiers: UiModifiers::default(),
                });
            }
            TouchPhase::Moved
                if messages.touch_state.active.get(&event.window) == Some(&event.id) =>
            {
                queue.push(UiInputEvent::PointerMove {
                    window: event.window,
                    position,
                    time_seconds: now,
                });
            }
            TouchPhase::Ended
                if messages.touch_state.active.get(&event.window) == Some(&event.id) =>
            {
                queue.push(UiInputEvent::PointerButtonWithModifiers {
                    window: event.window,
                    button: UiPointerButton::Primary,
                    pressed: false,
                    position,
                    time_seconds: now,
                    modifiers: UiModifiers::default(),
                });
                messages.touch_state.active.remove(&event.window);
            }
            TouchPhase::Canceled
                if messages.touch_state.active.get(&event.window) == Some(&event.id) =>
            {
                queue.push(UiInputEvent::PointerCancel {
                    window: event.window,
                });
                messages.touch_state.active.remove(&event.window);
            }
            _ => {}
        }
        queue.push(UiInputEvent::Touch {
            window: event.window,
            id: event.id,
            position,
            phase: match event.phase {
                TouchPhase::Started => UiTouchPhase::Started,
                TouchPhase::Moved => UiTouchPhase::Moved,
                TouchPhase::Ended => UiTouchPhase::Ended,
                TouchPhase::Canceled => UiTouchPhase::Cancelled,
            },
        });
    }
    for event in messages.buttons.read() {
        if messages.touch_state.active.contains_key(&event.window) {
            continue;
        }
        if messages.world_pointers.hover_target(event.window).is_some()
            || capture.active.as_ref().is_some_and(|active| {
                active.physical_window == event.window && active.window != event.window
            })
        {
            continue;
        }
        let Ok(window) = windows.get(event.window) else {
            continue;
        };
        let Some(position) = window.cursor_position() else {
            continue;
        };
        let button = match event.button {
            MouseButton::Left => UiPointerButton::Primary,
            MouseButton::Right => UiPointerButton::Secondary,
            MouseButton::Middle => UiPointerButton::Middle,
            MouseButton::Back => UiPointerButton::Other(4),
            MouseButton::Forward => UiPointerButton::Other(5),
            MouseButton::Other(number) => UiPointerButton::Other(number.min(u8::MAX as u16) as u8),
        };
        queue.push(UiInputEvent::PointerButtonWithModifiers {
            window: event.window,
            button,
            pressed: event.state == ButtonState::Pressed,
            position: position.to_array(),
            time_seconds: now,
            modifiers,
        });
    }
    for event in messages.wheels.read() {
        let Ok(window) = windows.get(event.window) else {
            continue;
        };
        let Some(position) = window.cursor_position() else {
            continue;
        };
        let (target, position) = messages
            .world_pointers
            .hover_target(event.window)
            .unwrap_or((event.window, position.to_array()));
        queue.push(UiInputEvent::Wheel {
            window: target,
            position,
            delta: [event.x, event.y],
            unit: match event.unit {
                MouseScrollUnit::Line => UiWheelUnit::Lines,
                MouseScrollUnit::Pixel => UiWheelUnit::Pixels,
            },
            modifiers,
        });
    }
    let mut composed_this_frame = HashSet::new();
    for event in messages.ime.read() {
        match event {
            Ime::Enabled { window } => {
                ime_state.active.insert(*window);
                composed_this_frame.insert(*window);
            }
            Ime::Disabled { window } => {
                ime_state.active.remove(window);
                composed_this_frame.insert(*window);
                queue.push(UiInputEvent::ImeDisabled {
                    window: messages
                        .world_pointers
                        .keyboard_target(*window)
                        .unwrap_or(*window),
                });
            }
            Ime::Preedit {
                window,
                value,
                cursor,
            } => {
                composed_this_frame.insert(*window);
                queue.push(UiInputEvent::ImePreedit {
                    window: messages
                        .world_pointers
                        .keyboard_target(*window)
                        .unwrap_or(*window),
                    text: value.clone(),
                    cursor_range: cursor.map(|(start, end)| [start, end]),
                });
            }
            Ime::Commit { window, value } => {
                composed_this_frame.insert(*window);
                queue.push(UiInputEvent::ImeCommit {
                    window: messages
                        .world_pointers
                        .keyboard_target(*window)
                        .unwrap_or(*window),
                    text: value.clone(),
                });
            }
        }
    }
    for event in messages.keys.read() {
        let key = match event.key_code {
            KeyCode::Tab => UiKeyCode::Tab,
            KeyCode::Escape => UiKeyCode::Escape,
            KeyCode::Enter | KeyCode::NumpadEnter => UiKeyCode::Enter,
            KeyCode::Space => UiKeyCode::Space,
            KeyCode::Backspace => UiKeyCode::Backspace,
            KeyCode::Delete => UiKeyCode::Delete,
            KeyCode::ArrowLeft => UiKeyCode::ArrowLeft,
            KeyCode::ArrowRight => UiKeyCode::ArrowRight,
            KeyCode::ArrowUp => UiKeyCode::ArrowUp,
            KeyCode::ArrowDown => UiKeyCode::ArrowDown,
            KeyCode::PageUp => UiKeyCode::PageUp,
            KeyCode::PageDown => UiKeyCode::PageDown,
            KeyCode::Home => UiKeyCode::Home,
            KeyCode::End => UiKeyCode::End,
            KeyCode::F2 => UiKeyCode::F2,
            KeyCode::KeyA => UiKeyCode::A,
            KeyCode::KeyC => UiKeyCode::C,
            KeyCode::KeyN => UiKeyCode::N,
            KeyCode::KeyV => UiKeyCode::V,
            KeyCode::KeyX => UiKeyCode::X,
            KeyCode::KeyY => UiKeyCode::Y,
            KeyCode::KeyZ => UiKeyCode::Z,
            _ => other_key_code(event.key_code),
        };
        let pressed = event.state == ButtonState::Pressed;
        let routed_window = messages
            .world_pointers
            .keyboard_target(event.window)
            .unwrap_or(event.window);
        queue.push(UiInputEvent::Key {
            window: routed_window,
            key,
            pressed,
            repeat: event.repeat,
            modifiers,
        });
        if pressed
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.super_key
            && !ime_state.active.contains(&event.window)
            && !composed_this_frame.contains(&event.window)
            && let Some(text) = &event.text
            // Some platforms provide control characters as text for editing
            // keys (for example, Backspace supplies U+0008). The key event
            // already performs the edit; inserting that text leaves a glyph.
            && !text.chars().any(char::is_control)
        {
            queue.push(UiInputEvent::Text {
                window: routed_window,
                text: text.to_string(),
            });
        }
    }
    for event in messages.focus.read() {
        if !event.focused {
            ime_state.active.remove(&event.window);
            messages.touch_state.active.remove(&event.window);
            queue.push(UiInputEvent::WindowBlur {
                window: event.window,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor_bridge::ActiveCapture;
    use bbui_core::{
        CorePlugin, UiHitBox, UiNode, UiRect, UiRoutedEvent, UiRoutedEvents, UiSystems,
    };
    use bevy::window::CursorGrabMode;

    #[test]
    fn captured_raw_motion_uses_logical_pixels_when_dpi_changes() {
        let mut app = App::new();
        app.add_plugins(CorePlugin)
            .add_message::<CursorMoved>()
            .add_message::<MouseMotion>()
            .add_message::<CursorLeft>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseWheel>()
            .add_message::<KeyboardInput>()
            .add_message::<Ime>()
            .add_message::<WindowFocused>()
            .add_message::<TouchInput>()
            .init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<WorldPointerState>()
            .init_resource::<ImeBridgeState>()
            .init_resource::<TouchBridgeState>()
            .init_resource::<CursorCaptureState>()
            .add_systems(
                Update,
                bridge_bevy_input
                    .in_set(UiSystems::Input)
                    .before(bbui_core::process_ui_input),
            );
        let window = app.world_mut().spawn(Window::default()).id();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_scale_factor_override(Some(2.0));
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
            .push(UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [10.0, 10.0],
                time_seconds: 1.0,
            });
        app.update();
        app.world_mut().resource_mut::<CursorCaptureState>().active = Some(ActiveCapture {
            window,
            physical_window: window,
            target,
            original_position: Some(Vec2::new(10.0, 10.0)),
            virtual_position: Vec2::new(10.0, 10.0),
            original_visible: true,
            original_grab_mode: CursorGrabMode::None,
        });

        // The third motion crosses the 100-pixel surface edge. Capture must
        // still route it to the original target, then continue after a DPI
        // change and a return across the opposite edge.
        for (dpi, raw_x, expected_x, delta_x) in [
            (2.0, 12.0, 16.0, 6.0),
            (1.0, 12.0, 28.0, 12.0),
            (1.0, 120.0, 148.0, 120.0),
            (2.0, -300.0, -2.0, -150.0),
        ] {
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .resolution
                .set_scale_factor_override(Some(dpi));
            app.world_mut().write_message(MouseMotion {
                delta: Vec2::new(raw_x, 0.0),
            });
            app.update();
            assert!(
                app.world()
                    .resource::<UiRoutedEvents>()
                    .events
                    .iter()
                    .any(|event| {
                        matches!(event, UiRoutedEvent::PointerMoved {
                    window: routed_window,
                    target: routed_target,
                    position,
                    delta,
                    ..
                } if *routed_window == window
                    && *routed_target == target
                    && *position == [expected_x, 10.0]
                    && *delta == [delta_x, 0.0])
                    })
            );
        }
        // The OS cursor remains at its original position while virtual motion
        // crosses logical window bounds; release restores the stored position.
        assert_eq!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .as_ref()
                .unwrap()
                .original_position,
            Some(Vec2::new(10.0, 10.0))
        );
    }

    #[test]
    fn primary_touch_routes_click_and_ignores_second_finger() {
        let mut app = App::new();
        app.add_plugins(CorePlugin)
            .add_message::<CursorMoved>()
            .add_message::<MouseMotion>()
            .add_message::<CursorLeft>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseWheel>()
            .add_message::<KeyboardInput>()
            .add_message::<Ime>()
            .add_message::<WindowFocused>()
            .add_message::<TouchInput>()
            .init_resource::<Time>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<WorldPointerState>()
            .init_resource::<ImeBridgeState>()
            .init_resource::<TouchBridgeState>()
            .init_resource::<CursorCaptureState>()
            .add_systems(
                Update,
                bridge_bevy_input
                    .in_set(UiSystems::Input)
                    .before(bbui_core::process_ui_input),
            );
        let window = app.world_mut().spawn(Window::default()).id();
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
        for id in [1, 2] {
            app.world_mut().write_message(TouchInput {
                phase: TouchPhase::Started,
                position: Vec2::new(20.0, 20.0),
                window,
                force: None,
                id,
            });
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<TouchBridgeState>()
                .active
                .get(&window),
            Some(&1)
        );
        app.world_mut().write_message(TouchInput {
            phase: TouchPhase::Ended,
            position: Vec2::new(20.0, 20.0),
            window,
            force: None,
            id: 1,
        });
        app.update();
        assert!(app.world().resource::<TouchBridgeState>().active.is_empty());
        assert!(app.world().resource::<UiRoutedEvents>().events.iter().any(|event| {
            matches!(event, UiRoutedEvent::Click { target: clicked, .. } if *clicked == target)
        }));
    }
}

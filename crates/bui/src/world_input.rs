//! Project pointer input from a Bevy window onto world-space UI planes.

use std::collections::HashMap;

use crate::cursor_bridge::{CursorCaptureState, logical_pointer_motion};
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion};
use bevy::prelude::*;
use bevy::window::{CursorLeft, CursorMoved, PrimaryWindow, WindowFocused, WindowRef};
use bui_core::{UiInputEvent, UiInputQueue, UiPointerButton};
use bui_render::UiWorldSurface;

#[derive(Resource, Default)]
pub struct WorldPointerState {
    /// Physical window -> offscreen camera and last local UI position.
    active: HashMap<Entity, (Entity, [f32; 2])>,
    /// Physical window -> offscreen camera whose BUI control owns keyboard focus.
    keyboard: HashMap<Entity, Entity>,
    last_keyboard_window: Option<Entity>,
}

impl WorldPointerState {
    pub fn hover_target(&self, window: Entity) -> Option<(Entity, [f32; 2])> {
        self.active.get(&window).copied()
    }

    pub fn keyboard_target(&self, window: Entity) -> Option<Entity> {
        self.keyboard.get(&window).copied()
    }

    pub fn physical_window_for(&self, surface_camera: Entity) -> Option<Entity> {
        self.last_keyboard_window
            .filter(|window| self.keyboard.get(window) == Some(&surface_camera))
            .or_else(|| {
                self.keyboard
                    .iter()
                    .find_map(|(window, camera)| (*camera == surface_camera).then_some(*window))
            })
            .or_else(|| {
                self.active.iter().find_map(|(window, (camera, _))| {
                    (*camera == surface_camera).then_some(*window)
                })
            })
    }
}

#[derive(SystemParam)]
pub struct WorldPointerMessages<'w, 's> {
    moves: MessageReader<'w, 's, CursorMoved>,
    exits: MessageReader<'w, 's, CursorLeft>,
    motions: MessageReader<'w, 's, MouseMotion>,
    buttons: MessageReader<'w, 's, MouseButtonInput>,
    focus: MessageReader<'w, 's, WindowFocused>,
}

#[derive(SystemParam)]
pub struct WorldPointerScene<'w, 's> {
    primary: Query<'w, 's, Entity, With<PrimaryWindow>>,
    windows: Query<'w, 's, &'static Window>,
    cameras: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static RenderTarget,
            &'static GlobalTransform,
        ),
    >,
    surfaces: Query<'w, 's, (&'static UiWorldSurface, &'static GlobalTransform)>,
}

fn camera_targets_window(target: &RenderTarget, window: Entity, primary: Option<Entity>) -> bool {
    match target {
        RenderTarget::Window(WindowRef::Primary) => primary == Some(window),
        RenderTarget::Window(WindowRef::Entity(entity)) => *entity == window,
        _ => false,
    }
}

fn pick_surface(
    window: Entity,
    pointer: Vec2,
    primary: Option<Entity>,
    cameras: &Query<(&Camera, &RenderTarget, &GlobalTransform)>,
    surfaces: &Query<(&UiWorldSurface, &GlobalTransform)>,
) -> Option<(Entity, [f32; 2])> {
    let mut closest: Option<(f32, Entity, [f32; 2])> = None;
    for (camera, target, camera_transform) in cameras {
        if !camera.is_active || !camera_targets_window(target, window, primary) {
            continue;
        }
        let Ok(ray) = camera.viewport_to_world(camera_transform, pointer) else {
            continue;
        };
        for (surface, transform) in surfaces {
            let inverse = transform.affine().inverse();
            let origin = inverse.transform_point3(ray.origin);
            let direction = inverse.transform_vector3(ray.direction.as_vec3());
            if direction.z.abs() <= f32::EPSILON {
                continue;
            }
            let t = -origin.z / direction.z;
            if t < 0.0 || !t.is_finite() {
                continue;
            }
            let local = origin + direction * t;
            let Some(ui) = surface.local_to_ui(local.truncate()) else {
                continue;
            };
            let world_hit = transform.transform_point(local);
            let distance = world_hit.distance(ray.origin);
            if closest.as_ref().is_none_or(|best| distance < best.0) {
                closest = Some((distance, surface.target_camera, ui));
            }
        }
    }
    closest.map(|(_, target, ui)| (target, ui))
}

pub fn bridge_world_pointer(
    mut messages: WorldPointerMessages,
    scene: WorldPointerScene,
    time: Res<Time>,
    mut state: ResMut<WorldPointerState>,
    mut capture: ResMut<CursorCaptureState>,
    mut queue: ResMut<UiInputQueue>,
) {
    let primary = scene.primary.iter().next();
    let now = time.elapsed_secs_f64();
    for event in messages.moves.read() {
        if capture
            .active
            .as_ref()
            .is_some_and(|active| active.physical_window == event.window)
        {
            continue;
        }
        let hit = pick_surface(
            event.window,
            event.position,
            primary,
            &scene.cameras,
            &scene.surfaces,
        );
        let previous = state.active.get(&event.window).copied();
        if previous.is_none() && hit.is_some() {
            queue.push(UiInputEvent::PointerLeave {
                window: event.window,
            });
        }
        if previous.map(|(target, _)| target) != hit.map(|(target, _)| target)
            && let Some((target, _)) = previous
        {
            queue.push(UiInputEvent::PointerLeave { window: target });
        }
        if let Some((target, position)) = hit {
            queue.push(UiInputEvent::PointerMove {
                window: target,
                position,
                time_seconds: now,
            });
            state.active.insert(event.window, (target, position));
        } else {
            state.active.remove(&event.window);
        }
    }
    for event in messages.exits.read() {
        if capture
            .active
            .as_ref()
            .is_some_and(|active| active.physical_window == event.window)
        {
            continue;
        }
        if let Some((target, _)) = state.active.remove(&event.window) {
            queue.push(UiInputEvent::PointerLeave { window: target });
        }
    }
    if let Some(active) = capture.active.as_mut()
        && active.physical_window != active.window
    {
        let scale_factor = scene
            .windows
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
        for _ in messages.motions.read() {}
    }
    for event in messages.buttons.read() {
        let hit = scene
            .windows
            .get(event.window)
            .ok()
            .and_then(Window::cursor_position)
            .and_then(|position| {
                pick_surface(
                    event.window,
                    position,
                    primary,
                    &scene.cameras,
                    &scene.surfaces,
                )
            });
        if event.button == MouseButton::Left && event.state == ButtonState::Pressed {
            let next = hit.map(|(target, _)| target);
            let previous = state.keyboard.get(&event.window).copied();
            if previous != next {
                if let Some(previous) = previous {
                    queue.push(UiInputEvent::WindowBlur { window: previous });
                }
                if let Some(next) = next {
                    state.keyboard.insert(event.window, next);
                } else {
                    state.keyboard.remove(&event.window);
                    if state.last_keyboard_window == Some(event.window) {
                        state.last_keyboard_window = None;
                    }
                }
            }
            if next.is_some() {
                state.last_keyboard_window = Some(event.window);
            }
        }
        let active = capture
            .active
            .as_ref()
            .filter(|active| active.physical_window == event.window)
            .map(|active| (active.window, active.virtual_position.to_array()))
            .or(hit)
            .or_else(|| state.active.get(&event.window).copied());
        let Some((target, position)) = active else {
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
        queue.push(UiInputEvent::PointerButton {
            window: target,
            button,
            pressed: event.state == ButtonState::Pressed,
            position,
            time_seconds: now,
        });
    }
    for event in messages.focus.read() {
        if !event.focused {
            if let Some((target, _)) = state.active.remove(&event.window) {
                queue.push(UiInputEvent::PointerLeave { window: target });
            }
            if let Some(target) = state.keyboard.remove(&event.window) {
                queue.push(UiInputEvent::WindowBlur { window: target });
            }
            if state.last_keyboard_window == Some(event.window) {
                state.last_keyboard_window = None;
            }
        }
    }
}

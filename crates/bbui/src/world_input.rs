//! Project pointer input from a Bevy window onto world-space UI planes.

use std::collections::{HashMap, HashSet};

use crate::cursor_bridge::{CursorCaptureState, logical_pointer_motion};
use bbui_core::{UiInputEvent, UiInputQueue, UiPointerButton};
use bbui_render::UiWorldSurface;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion};
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility};
use bevy::prelude::*;
use bevy::window::{CursorLeft, CursorMoved, PrimaryWindow, WindowFocused, WindowRef};

/// Use the center of this window camera as a world-space BBUI pointer.
/// The nearest visible mesh blocks the ray, and a held drag stays captured
/// by its original UI surface even when the crosshair passes an edge.
#[derive(Component, Debug, Clone, Copy)]
pub struct UiCrosshairPointer {
    pub max_distance: f32,
}

impl Default for UiCrosshairPointer {
    fn default() -> Self {
        Self { max_distance: 6.0 }
    }
}

#[derive(Clone, Copy)]
struct WorldDrag {
    surface: Entity,
    target: Entity,
    position: [f32; 2],
}

#[derive(Resource, Default)]
pub struct WorldPointerState {
    /// Physical window -> offscreen camera and last local UI position.
    active: HashMap<Entity, (Entity, [f32; 2])>,
    active_surface: HashMap<Entity, Entity>,
    dragging: HashMap<Entity, WorldDrag>,
    /// Physical window -> offscreen camera whose BBUI control owns keyboard focus.
    keyboard: HashMap<Entity, Entity>,
    last_keyboard_window: Option<Entity>,
    gaze_windows: HashSet<Entity>,
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
            Option<&'static UiCrosshairPointer>,
        ),
    >,
    surfaces: Query<'w, 's, (&'static UiWorldSurface, &'static GlobalTransform)>,
}

fn target_window(target: &RenderTarget, primary: Option<Entity>) -> Option<Entity> {
    match target {
        RenderTarget::Window(WindowRef::Primary) => primary,
        RenderTarget::Window(WindowRef::Entity(entity)) => Some(*entity),
        _ => None,
    }
}

fn camera_targets_window(target: &RenderTarget, window: Entity, primary: Option<Entity>) -> bool {
    target_window(target, primary) == Some(window)
}

fn pick_surface(
    window: Entity,
    pointer: Vec2,
    primary: Option<Entity>,
    cameras: &Query<(
        &Camera,
        &RenderTarget,
        &GlobalTransform,
        Option<&UiCrosshairPointer>,
    )>,
    surfaces: &Query<(&UiWorldSurface, &GlobalTransform)>,
) -> Option<(Entity, [f32; 2])> {
    let mut closest: Option<(f32, Entity, [f32; 2])> = None;
    for (camera, target, camera_transform, crosshair) in cameras {
        if !camera.is_active
            || crosshair.is_some()
            || !camera_targets_window(target, window, primary)
        {
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

fn project_drag(
    ray: Ray3d,
    surface_entity: Entity,
    surfaces: &Query<(&UiWorldSurface, &GlobalTransform)>,
) -> Option<(Entity, [f32; 2])> {
    let (surface, transform) = surfaces.get(surface_entity).ok()?;
    let inverse = transform.affine().inverse();
    let origin = inverse.transform_point3(ray.origin);
    let direction = inverse.transform_vector3(ray.direction.as_vec3());
    if direction.z.abs() <= f32::EPSILON {
        return None;
    }
    let t = -origin.z / direction.z;
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    let local = origin + direction * t;
    Some((
        surface.target_camera,
        surface.local_to_ui_unbounded(local.truncate())?,
    ))
}

fn pick_crosshair(
    ray: Ray3d,
    max_distance: f32,
    ray_cast: &mut MeshRayCast,
    surfaces: &Query<(&UiWorldSurface, &GlobalTransform)>,
) -> Option<(Entity, Entity, [f32; 2])> {
    let settings = MeshRayCastSettings::default().with_visibility(RayCastVisibility::Visible);
    let (entity, hit) = ray_cast.cast_ray(ray, &settings).first()?;
    if !max_distance.is_finite() || hit.distance > max_distance || hit.distance < 0.0 {
        return None;
    }
    let (surface, transform) = surfaces.get(*entity).ok()?;
    let local = transform.affine().inverse().transform_point3(hit.point);
    Some((
        *entity,
        surface.target_camera,
        surface.local_to_ui(local.truncate())?,
    ))
}

pub fn bridge_world_pointer(
    mut messages: WorldPointerMessages,
    scene: WorldPointerScene,
    time: Res<Time>,
    mut state: ResMut<WorldPointerState>,
    mut capture: ResMut<CursorCaptureState>,
    mut queue: ResMut<UiInputQueue>,
    mut ray_cast: MeshRayCast,
) {
    let primary = scene.primary.iter().next();
    let now = time.elapsed_secs_f64();
    let mut gaze_rays = HashMap::new();
    for (camera, target, transform, crosshair) in &scene.cameras {
        let Some(crosshair) = crosshair else { continue };
        let Some(window) = target_window(target, primary) else {
            continue;
        };
        if !camera.is_active {
            continue;
        }
        let Some(viewport) = camera.logical_viewport_rect() else {
            continue;
        };
        let Ok(ray) = camera.viewport_to_world(transform, viewport.center()) else {
            continue;
        };
        gaze_rays.insert(window, ray);
        if capture
            .active
            .as_ref()
            .is_some_and(|active| active.physical_window == window)
        {
            continue;
        }
        let previous = state.active.get(&window).copied();
        let drag = state.dragging.get(&window).copied();
        let hit = if let Some(drag) = drag {
            project_drag(ray, drag.surface, &scene.surfaces)
                .map(|(target, position)| (drag.surface, target, position))
        } else {
            pick_crosshair(ray, crosshair.max_distance, &mut ray_cast, &scene.surfaces)
        };
        if previous.is_none() && hit.is_some() {
            queue.push(UiInputEvent::PointerLeave { window });
        }
        if previous.map(|(target, _)| target) != hit.map(|(_, target, _)| target)
            && let Some((target, _)) = previous
        {
            queue.push(UiInputEvent::PointerLeave { window: target });
        }
        if let Some((surface, target, position)) = hit {
            if previous != Some((target, position)) {
                queue.push(UiInputEvent::PointerMove {
                    window: target,
                    position,
                    time_seconds: now,
                });
                state.active.insert(window, (target, position));
            }
            if state.active_surface.get(&window) != Some(&surface) {
                state.active_surface.insert(window, surface);
            }
            if let Some(drag) = state.dragging.get_mut(&window)
                && drag.position != position
            {
                drag.position = position;
            }
        } else {
            if previous.is_some() {
                state.active.remove(&window);
            }
            if state.active_surface.contains_key(&window) {
                state.active_surface.remove(&window);
            }
            if let Some(drag) = state.dragging.remove(&window) {
                queue.push(UiInputEvent::PointerCancel {
                    window: drag.target,
                });
            }
        }
    }
    let current_gaze_windows: HashSet<_> = gaze_rays.keys().copied().collect();
    for window in state
        .gaze_windows
        .difference(&current_gaze_windows)
        .copied()
        .collect::<Vec<_>>()
    {
        if let Some((target, _)) = state.active.remove(&window) {
            queue.push(UiInputEvent::PointerLeave { window: target });
        }
        state.active_surface.remove(&window);
        if let Some(drag) = state.dragging.remove(&window) {
            queue.push(UiInputEvent::PointerCancel {
                window: drag.target,
            });
        }
        if let Some(target) = state.keyboard.remove(&window) {
            queue.push(UiInputEvent::WindowBlur { window: target });
        }
        if state.last_keyboard_window == Some(window) {
            state.last_keyboard_window = None;
        }
    }
    state.gaze_windows = current_gaze_windows;
    let surface_available = |window: Entity, target: Entity| {
        scene
            .surfaces
            .iter()
            .any(|(surface, _)| surface.target_camera == target)
            && scene.cameras.iter().any(|(camera, render_target, _, _)| {
                camera.is_active && camera_targets_window(render_target, window, primary)
            })
    };
    let stale_hover: Vec<_> = state
        .active
        .iter()
        .filter_map(|(window, (target, _))| {
            (!gaze_rays.contains_key(window) && !surface_available(*window, *target))
                .then_some((*window, *target))
        })
        .collect();
    for (window, target) in stale_hover {
        state.active.remove(&window);
        state.active_surface.remove(&window);
        queue.push(UiInputEvent::PointerLeave { window: target });
        if let Some(drag) = state.dragging.remove(&window) {
            queue.push(UiInputEvent::PointerCancel {
                window: drag.target,
            });
        }
    }
    let stale_keyboard: Vec<_> = state
        .keyboard
        .iter()
        .filter_map(|(window, target)| {
            (!surface_available(*window, *target)).then_some((*window, *target))
        })
        .collect();
    for (window, target) in stale_keyboard {
        state.keyboard.remove(&window);
        queue.push(UiInputEvent::WindowBlur { window: target });
        if state.last_keyboard_window == Some(window) {
            state.last_keyboard_window = None;
        }
    }
    for event in messages.moves.read() {
        if gaze_rays.contains_key(&event.window) {
            continue;
        }
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
        if gaze_rays.contains_key(&event.window) {
            continue;
        }
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
        let hit = if gaze_rays.contains_key(&event.window) {
            state.active.get(&event.window).copied()
        } else {
            scene
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
                })
        };
        if gaze_rays.contains_key(&event.window)
            && event.button == MouseButton::Left
            && event.state == ButtonState::Pressed
            && let Some((target, position)) = hit
            && let Some(surface) = state.active_surface.get(&event.window).copied()
        {
            state.dragging.insert(
                event.window,
                WorldDrag {
                    surface,
                    target,
                    position,
                },
            );
        }
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
        if event.button == MouseButton::Left && event.state == ButtonState::Released {
            state.dragging.remove(&event.window);
        }
    }
    for event in messages.focus.read() {
        if !event.focused {
            if let Some((target, _)) = state.active.remove(&event.window) {
                queue.push(UiInputEvent::PointerLeave { window: target });
            }
            state.active_surface.remove(&event.window);
            state.dragging.remove(&event.window);
            if let Some(target) = state.keyboard.remove(&event.window) {
                queue.push(UiInputEvent::WindowBlur { window: target });
            }
            if state.last_keyboard_window == Some(event.window) {
                state.last_keyboard_window = None;
            }
        }
    }
}

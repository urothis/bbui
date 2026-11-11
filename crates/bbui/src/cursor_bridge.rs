//! Apply widget scrub capture requests to Bevy windows.

use crate::world_input::WorldPointerState;
use bbui_core::{UiEnabled, UiParent, UiVisible};
use bbui_widgets::actions::{CursorCaptureRequest, CursorCaptureRequestQueue};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

#[derive(Resource, Default)]
pub struct CursorCaptureState {
    pub active: Option<ActiveCapture>,
}

pub struct ActiveCapture {
    /// Logical BBUI surface used for routed pointer deltas.
    pub window: Entity,
    /// Actual OS window whose cursor is hidden and locked.
    pub physical_window: Entity,
    pub target: Entity,
    pub original_position: Option<Vec2>,
    pub virtual_position: Vec2,
    pub original_visible: bool,
    pub original_grab_mode: CursorGrabMode,
}

/// Bevy's raw `MouseMotion` uses physical pixels; BBUI input and cursor
/// positions use logical pixels. Read the current scale on every motion so a
/// scrub remains continuous when a window changes monitor DPI.
pub(crate) fn logical_pointer_motion(delta: Vec2, scale_factor: f32) -> Vec2 {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    delta / scale
}

type CaptureNodeState<'a> = (
    Entity,
    Option<&'a UiEnabled>,
    Option<&'a UiVisible>,
    Option<&'a UiParent>,
);

pub fn process_cursor_capture_requests(
    mut requests: ResMut<CursorCaptureRequestQueue>,
    mut state: ResMut<CursorCaptureState>,
    mut windows: Query<(&mut Window, &mut CursorOptions)>,
    nodes: Query<CaptureNodeState<'_>>,
    world_pointers: Res<WorldPointerState>,
) {
    if state.active.as_ref().is_some_and(|active| {
        nodes.get(active.target).is_err()
            || !windows
                .get(active.physical_window)
                .is_ok_and(|(window, _)| window.focused)
            || !capture_target_interactable(active.target, &nodes)
    }) && let Some(active) = state.active.take()
    {
        restore(&mut windows, active, true);
    }
    for request in requests.drain() {
        match request {
            CursorCaptureRequest::Begin {
                window,
                target,
                hide_cursor,
                continuous,
            } => {
                if !capture_target_interactable(target, &nodes) {
                    continue;
                }
                if let Some(active) = state.active.take() {
                    restore(&mut windows, active, true);
                }
                let physical_window = if windows.get(window).is_ok() {
                    window
                } else if let Some(physical) = world_pointers.physical_window_for(window) {
                    physical
                } else {
                    continue;
                };
                let Ok((window_component, mut cursor)) = windows.get_mut(physical_window) else {
                    continue;
                };
                if !window_component.focused {
                    continue;
                }
                let position = window_component.cursor_position();
                let virtual_position = if physical_window == window {
                    position.unwrap_or_default()
                } else {
                    world_pointers
                        .hover_target(physical_window)
                        .map(|(_, position)| Vec2::from_array(position))
                        .unwrap_or_default()
                };
                state.active = Some(ActiveCapture {
                    window,
                    physical_window,
                    target,
                    original_position: position,
                    virtual_position,
                    original_visible: cursor.visible,
                    original_grab_mode: cursor.grab_mode,
                });
                if hide_cursor {
                    cursor.visible = false;
                }
                if continuous {
                    cursor.grab_mode = CursorGrabMode::Locked;
                }
            }
            CursorCaptureRequest::End {
                window,
                target,
                restore_position,
            } => {
                if state
                    .active
                    .as_ref()
                    .is_some_and(|active| active.window == window && active.target == target)
                    && let Some(active) = state.active.take()
                {
                    restore(&mut windows, active, restore_position);
                }
            }
        }
    }
}

fn capture_target_interactable(target: Entity, nodes: &Query<CaptureNodeState<'_>>) -> bool {
    let mut current = Some(target);
    for _ in 0..1024 {
        let Some(at) = current else {
            return true;
        };
        let Ok((_, enabled, visible, parent)) = nodes.get(at) else {
            return false;
        };
        if enabled.is_some_and(|enabled| !enabled.0) || visible.is_some_and(|visible| !visible.0) {
            return false;
        }
        current = parent.map(|parent| parent.0);
    }
    false
}

fn restore(
    windows: &mut Query<(&mut Window, &mut CursorOptions)>,
    active: ActiveCapture,
    restore_position: bool,
) {
    if let Ok((mut window, mut cursor)) = windows.get_mut(active.physical_window) {
        cursor.visible = active.original_visible;
        cursor.grab_mode = active.original_grab_mode;
        if restore_position {
            window.set_cursor_position(active.original_position);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_capture_motion_tracks_current_dpi() {
        let physical = Vec2::new(12.0, -8.0);
        assert_eq!(logical_pointer_motion(physical, 1.0), physical);
        assert_eq!(logical_pointer_motion(physical, 2.0), Vec2::new(6.0, -4.0));
        assert_eq!(
            logical_pointer_motion(physical, 1.5),
            Vec2::new(8.0, -8.0 / 1.5)
        );

        let mut window = Window::default();
        window.resolution.set_scale_factor_override(Some(2.0));
        assert_eq!(
            logical_pointer_motion(physical, window.scale_factor()),
            Vec2::new(6.0, -4.0)
        );
        window.resolution.set_scale_factor_override(Some(1.0));
        assert_eq!(
            logical_pointer_motion(physical, window.scale_factor()),
            physical
        );
    }

    #[test]
    fn scrub_capture_hides_and_restores_cursor() {
        let mut app = App::new();
        app.init_resource::<CursorCaptureRequestQueue>()
            .init_resource::<CursorCaptureState>()
            .init_resource::<WorldPointerState>()
            .add_systems(Update, process_cursor_capture_requests);
        let window = app
            .world_mut()
            .spawn((Window::default(), CursorOptions::default()))
            .id();
        let original = Vec2::new(25.0, 30.0);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(original));
        let target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<CursorCaptureRequestQueue>()
            .0
            .push_back(CursorCaptureRequest::Begin {
                window,
                target,
                hide_cursor: true,
                continuous: true,
            });
        app.update();
        let options = app.world().get::<CursorOptions>(window).unwrap();
        assert!(!options.visible);
        assert_eq!(options.grab_mode, CursorGrabMode::Locked);

        app.world_mut()
            .resource_mut::<CursorCaptureRequestQueue>()
            .0
            .push_back(CursorCaptureRequest::End {
                window,
                target,
                restore_position: true,
            });
        app.update();
        let options = app.world().get::<CursorOptions>(window).unwrap();
        assert!(options.visible);
        assert_eq!(options.grab_mode, CursorGrabMode::None);
        assert_eq!(
            app.world().get::<Window>(window).unwrap().cursor_position(),
            Some(original)
        );
    }

    #[test]
    fn scrub_capture_restores_cursor_when_physical_window_blurs() {
        let mut app = App::new();
        app.init_resource::<CursorCaptureRequestQueue>()
            .init_resource::<CursorCaptureState>()
            .init_resource::<WorldPointerState>()
            .add_systems(Update, process_cursor_capture_requests);
        let window = app
            .world_mut()
            .spawn((Window::default(), CursorOptions::default()))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<CursorCaptureRequestQueue>()
            .0
            .push_back(CursorCaptureRequest::Begin {
                window,
                target,
                hide_cursor: true,
                continuous: true,
            });
        app.update();
        assert!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .is_some()
        );
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .is_none()
        );
        let cursor = app.world().get::<CursorOptions>(window).unwrap();
        assert!(cursor.visible);
        assert_eq!(cursor.grab_mode, CursorGrabMode::None);
    }

    #[test]
    fn scrub_capture_releases_when_target_or_ancestor_becomes_invalid() {
        let mut app = App::new();
        app.init_resource::<CursorCaptureRequestQueue>()
            .init_resource::<CursorCaptureState>()
            .init_resource::<WorldPointerState>()
            .add_systems(Update, process_cursor_capture_requests);
        let window = app
            .world_mut()
            .spawn((Window::default(), CursorOptions::default()))
            .id();
        let ancestor = app.world_mut().spawn(UiVisible(true)).id();
        let target = app
            .world_mut()
            .spawn((UiEnabled(true), UiParent(ancestor)))
            .id();
        let begin = |app: &mut App| {
            app.world_mut()
                .resource_mut::<CursorCaptureRequestQueue>()
                .0
                .push_back(CursorCaptureRequest::Begin {
                    window,
                    target,
                    hide_cursor: true,
                    continuous: true,
                });
            app.update();
            assert_eq!(
                app.world().get::<CursorOptions>(window).unwrap().grab_mode,
                CursorGrabMode::Locked
            );
        };
        begin(&mut app);
        app.world_mut().get_mut::<UiVisible>(ancestor).unwrap().0 = false;
        app.update();
        assert!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .is_none()
        );
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );

        app.world_mut().get_mut::<UiVisible>(ancestor).unwrap().0 = true;
        begin(&mut app);
        app.world_mut().get_mut::<UiEnabled>(target).unwrap().0 = false;
        app.update();
        assert!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .is_none()
        );

        app.world_mut().get_mut::<UiEnabled>(target).unwrap().0 = true;
        begin(&mut app);
        app.world_mut().despawn(target);
        app.update();
        assert!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .is_none()
        );
        let options = app.world().get::<CursorOptions>(window).unwrap();
        assert!(options.visible);
        assert_eq!(options.grab_mode, CursorGrabMode::None);
        app.world_mut()
            .resource_mut::<CursorCaptureRequestQueue>()
            .0
            .push_back(CursorCaptureRequest::Begin {
                window,
                target,
                hide_cursor: true,
                continuous: true,
            });
        app.update();
        assert!(
            app.world()
                .resource::<CursorCaptureState>()
                .active
                .is_none()
        );
        assert_eq!(
            app.world().get::<CursorOptions>(window).unwrap().grab_mode,
            CursorGrabMode::None
        );
    }
}

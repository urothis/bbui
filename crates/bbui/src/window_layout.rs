//! Keep layout roots tied to Bevy windows current through resize and DPI changes.

use bbui_layout::{Rect, UiLayoutRoot};
use bevy::prelude::*;

pub fn sync_window_layout_roots(
    windows: Query<(Entity, &Window)>,
    mut roots: Query<&mut UiLayoutRoot>,
) {
    for mut root in &mut roots {
        let Ok((_, window)) = windows.get(root.window) else {
            continue;
        };
        let viewport = Rect {
            x: 0.0,
            y: 0.0,
            width: window.resolution.width(),
            height: window.resolution.height(),
        };
        let dpi_scale = window.scale_factor();
        if root.viewport != viewport || root.dpi_scale != dpi_scale {
            root.viewport = viewport;
            root.dpi_scale = dpi_scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_updates_layout_root_from_window_resolution() {
        let mut app = App::new();
        app.add_systems(Update, sync_window_layout_roots);
        let window = app.world_mut().spawn(Window::default()).id();
        let root = app
            .world_mut()
            .spawn(UiLayoutRoot::new(
                window,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                1.0,
            ))
            .id();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(720.0, 480.0);
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set_scale_factor_override(Some(2.0));
        app.update();
        let layout = app.world().get::<UiLayoutRoot>(root).unwrap();
        assert_eq!(layout.viewport.width, 360.0);
        assert_eq!(layout.viewport.height, 240.0);
        assert_eq!(layout.dpi_scale, 2.0);
    }
}

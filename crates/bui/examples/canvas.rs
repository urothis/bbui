//! Run with `cargo run -p bui --example canvas` from the workspace root.
//! Left-click to select, Ctrl/Shift-click to add to selection, drag to move,
//! middle-drag to pan, and wheel to zoom. Escape cancels a drag; Delete removes
//! selected objects. Arrow keys nudge; Page Up/Down zoom; Home resets the view.
//! Ctrl+Z and Ctrl+Shift+Z undo and redo canvas edits.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{CanvasShape, WidgetNode};
use bui_layout::Rect;

fn sample_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    canvas
        .add(
            [36.0, 58.0],
            CanvasShape::Rectangle {
                size: [190.0, 130.0],
            },
            [0.21, 0.44, 0.75, 1.0],
        )
        .expect("valid rectangle");
    canvas
        .add(
            [162.0, 130.0],
            CanvasShape::Ellipse {
                size: [200.0, 140.0],
            },
            [0.86, 0.36, 0.22, 0.9],
        )
        .expect("valid ellipse");
    canvas
        .add(
            [76.0, 345.0],
            CanvasShape::Line {
                end: [420.0, 290.0],
            },
            [0.29, 0.76, 0.52, 1.0],
        )
        .expect("valid line");
    canvas
        .add(
            [525.0, 95.0],
            CanvasShape::Image {
                size: [210.0, 210.0],
                asset: "canvas/checker.svg".to_owned(),
            },
            [1.0; 4],
        )
        .expect("valid image");
    canvas
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("canvas example requires a window");
    let canvas =
        WidgetTree::new(WidgetNode::canvas("Canvas editor", sample_canvas()).with_key("canvas"))
            .spawn(world, None)
            .expect("canvas widget is valid");
    {
        let mut layout = world.get_mut::<UiLayoutNode>(canvas).unwrap();
        layout.size.width = UiLength::Px(900.0);
        layout.size.height = UiLength::Px(620.0);
        layout.clip_contents = true;
    }
    world.entity_mut(canvas).insert(UiLayoutRoot::new(
        window,
        Rect::new(24.0, 24.0, 900.0, 620.0),
        1.0,
    ));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI Canvas".into(),
                resolution: WindowResolution::new(960, 680),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_has_all_drawable_canvas_shapes() {
        let canvas = sample_canvas();
        assert_eq!(canvas.objects.len(), 4);
        assert!(matches!(
            canvas.objects[0].shape,
            CanvasShape::Rectangle { .. }
        ));
        assert!(matches!(
            canvas.objects[1].shape,
            CanvasShape::Ellipse { .. }
        ));
        assert!(matches!(canvas.objects[2].shape, CanvasShape::Line { .. }));
        assert!(matches!(canvas.objects[3].shape, CanvasShape::Image { .. }));
        assert!(include_str!("../../../assets/canvas/checker.svg").contains("<svg"));
    }
}

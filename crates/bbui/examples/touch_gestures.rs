//! A touch-ready canvas beside a scroll surface.
//!
//! Use one finger to select and drag a shape. Two fingers pan and pinch the
//! canvas; on the right, two fingers pan the scroll surface.

use bbui::prelude::*;
use bbui::widgets::{CanvasShape, WidgetNode};
use bbui_layout::{Rect, UiEdges};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "touch_gestures/capture.rs"]
mod capture;

fn sample_canvas() -> Option<Canvas> {
    let mut canvas = Canvas::default();
    canvas.add(
        [68.0, 72.0],
        CanvasShape::Rectangle {
            size: [230.0, 154.0],
        },
        [0.19, 0.47, 0.76, 1.0],
    )?;
    canvas.add(
        [320.0, 116.0],
        CanvasShape::Ellipse {
            size: [200.0, 200.0],
        },
        [0.85, 0.42, 0.27, 1.0],
    )?;
    canvas.add(
        [170.0, 342.0],
        CanvasShape::Rectangle {
            size: [262.0, 116.0],
        },
        [0.36, 0.67, 0.51, 1.0],
    )?;
    Some(canvas)
}

fn install_ui(world: &mut World) {
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("touch example requires a primary window");
        return;
    };
    let Some(canvas_model) = sample_canvas() else {
        eprintln!("touch example geometry is invalid");
        return;
    };
    let canvas = WidgetNode::canvas("Gesture canvas", canvas_model)
        .with_key("gesture-canvas")
        .with_width(UiLength::Px(576.0))
        .with_height(UiLength::Px(500.0));
    let scroll = WidgetNode::scroll([
        WidgetNode::panel(
            "One finger",
            true,
            [
                WidgetNode::label("Tap to select a shape"),
                WidgetNode::label("Drag to move the selected shape"),
            ],
        ),
        WidgetNode::panel(
            "Two fingers",
            true,
            [
                WidgetNode::label("Pan the canvas or this list"),
                WidgetNode::label("Pinch to zoom the canvas"),
            ],
        ),
        WidgetNode::panel(
            "Input routing",
            true,
            [
                WidgetNode::label("Second touch cancels the press"),
                WidgetNode::label("Gestures stay on one surface"),
                WidgetNode::label("Custom surfaces can opt in"),
            ],
        ),
    ])
    .with_width(UiLength::Px(312.0))
    .with_height(UiLength::Px(500.0));
    let Ok(root) = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("TOUCH GESTURES").with_height(UiLength::Px(32.0)),
            WidgetNode::label("Canvas pan and pinch  ·  Scroll surface pan"),
            WidgetNode::row([canvas, scroll]).with_gap(12.0),
        ])
        .with_gap(12.0)
        .with_padding(UiEdges::all(24.0)),
    )
    .spawn(world, None) else {
        eprintln!("failed to mount touch example widgets");
        return;
    };
    let Some(window_data) = world.get::<Window>(window) else {
        return;
    };
    let size = window_data.resolution.size();
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, size.x, size.y),
        1.0,
    ));
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_ui(world);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Touch Gestures".into(),
                resolution: WindowResolution::new(960, 620),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

//! Run with `cargo run -p bui --example sliders`.
//!
//! Demonstrates reusable, range-bounded Slider models declared through `bui!`.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{Slider, WidgetEventQueue};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("slider example requires a primary window");
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 10.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 760.0, 320.0), 1.0),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: world.resource::<Theme>().background,
            }),
        ))
        .id();

    let opacity = Slider::new(0.72, 0.0, 1.0, 0.01).unwrap().with_unit("%");
    let tree = bui! {
        column {
            label("Sliders · arrows adjust, Home/End jump to range ends"),
            slider("Opacity", opacity),
            integer_slider_range("Grid size", 16, 1, 64, 1),
            label("Drag the handle or track; Escape cancels a drag. Ctrl+Wheel steps the focused slider."),
        }
    };
    tree.spawn(world, Some(root))
        .expect("valid bounded slider declarations");
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("widget event: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI sliders".into(),
                resolution: WindowResolution::new(760, 320),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            log_widget_events.after(bui_core::UiSystems::Widgets),
        )
        .run();
}

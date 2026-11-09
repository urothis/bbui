//! Run with `cargo run -p bui --example hsv_color`.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::WidgetEventQueue;
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};
use bui_style::Rgba;

fn setup(world: &mut World) {
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("HSV color example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(28.0),
                gap: 10.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(
                window,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 920.0,
                    height: 620.0,
                },
                1.0,
            ),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: background,
            }),
        ))
        .id();
    let tree = bui! {
        column {
            label("HSV color editor"),
            hsv_color("Primary", Rgba(0.15, 0.48, 0.9, 0.85)),
            label("Drag the hue ring or saturation disk. Value and alpha have separate sliders."),
            label("Click H/S/V, R/G/B/A or Hex, type a value, then press Enter. Arrows adjust or move between fields."),
            hsv_color_default("Accent"),
        }
    };
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
    }
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
                title: "BUI HSV Color Editor".into(),
                resolution: WindowResolution::new(920, 620),
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

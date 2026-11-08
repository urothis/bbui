//! Run with `cargo run -p bui --example color_ramp`.

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
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("color ramp example requires a primary window");
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

    let mut ramp = ColorRamp::default();
    ramp.set_color_channel(1, bui::widgets::ColorChannel::Red, 0.95);
    ramp.set_color_channel(1, bui::widgets::ColorChannel::Green, 0.26);
    ramp.set_color_channel(1, bui::widgets::ColorChannel::Blue, 0.08);
    ramp.set_color_channel(1, bui::widgets::ColorChannel::Alpha, 1.0);
    ramp.add_stop(0.52, Rgba(0.15, 0.48, 0.9, 0.76));

    let tree = bui! {
        column {
            label("Color ramp editor"),
            label("Drag stop handles or channel bars. Double-click the ramp to add a stop."),
            color_ramp("Gradient", ramp.clone()),
            label("Keyboard: ←/→ move stop · ↑/↓ select stop · C edit RGBA · Enter interpolation · A add · Delete remove"),
        }
    };
    tree.spawn(world, Some(root))
        .expect("valid color ramp widget tree");
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
                title: "BUI Color Ramp Editor".into(),
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

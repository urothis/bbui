//! Run with `cargo run -p bbui --example color_ramp`.

use bbui::prelude::*;
use bbui::widgets::{WidgetEventQueue, WidgetNode};
use bbui_layout::{Rect, UiEdges, UiLength};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Rgba;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("color ramp example requires a primary window");
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
                    height: 400.0,
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
    ramp.set_color_channel(1, bbui::widgets::ColorChannel::Red, 0.95);
    ramp.set_color_channel(1, bbui::widgets::ColorChannel::Green, 0.26);
    ramp.set_color_channel(1, bbui::widgets::ColorChannel::Blue, 0.08);
    ramp.set_color_channel(1, bbui::widgets::ColorChannel::Alpha, 1.0);
    ramp.add_stop(0.52, Rgba(0.15, 0.48, 0.9, 0.76));

    let tree = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("Color ramp editor"),
            WidgetNode::label("Drag stop handles or channel bars. Double-click the ramp to add a stop."),
            WidgetNode::color_ramp("Gradient", ramp.clone()).with_width(UiLength::Fill(1.0)),
            WidgetNode::label(
                "Keyboard: ←/→ move stop · ↑/↓ select stop · C edit RGBA · Enter interpolation · A add · Delete remove",
            ),
        ])
        .with_gap(10.0)
        .with_width(UiLength::Fill(1.0)),
    );
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
                title: "BBUI Color Ramp Editor".into(),
                resolution: WindowResolution::new(920, 400),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            log_widget_events.after(bbui_core::UiSystems::Widgets),
        )
        .run();
}

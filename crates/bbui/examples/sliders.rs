//! Run with `cargo run -p bbui --example sliders`.
//!
//! Demonstrates reusable, range-bounded Slider models declared through `bbui!`.

use bbui::prelude::*;
use bbui::widgets::{Slider, WidgetEventQueue};
use bbui_layout::{Rect, UiEdges};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("slider example requires a primary window");
        return;
    };
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

    let Ok(opacity) = Slider::new(0.72, 0.0, 1.0, 0.01) else {
        eprintln!("invalid opacity slider range");
        return;
    };
    let opacity = opacity.with_unit("%");
    let tree = bbui! {
        column {
            label("Sliders · arrows adjust, Home/End jump to range ends"),
            slider("Opacity", opacity),
            integer_slider_range("Grid size", 16, 1, 64, 1),
            label("Drag the handle or track; Escape cancels a drag. Ctrl+Wheel steps the focused slider."),
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
                title: "BBUI sliders".into(),
                resolution: WindowResolution::new(760, 320),
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

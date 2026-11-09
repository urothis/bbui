//! Run with `cargo run -p bui --example numeric_group`.
//!
//! Drag vertically across compatible fields, then move horizontally to scrub
//! the selected values together. Shift precision, Control snapping, and Escape
//! cancellation apply to the entire gesture.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{NumericField, WidgetEventQueue, WidgetNode};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("numeric group example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 8.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 760.0, 480.0), 1.0),
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
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("Grouped numeric scrubbing"),
        WidgetNode::number("Location X", 0.0, -100.0, 100.0, 0.1).with_key("location-x"),
        WidgetNode::number("Location Y", 2.0, -50.0, 50.0, 0.25).with_key("location-y"),
        WidgetNode::number("Scale", 1.0, 0.1, 10.0, 0.01).with_key("scale"),
        WidgetNode::label("Drag down over fields to select, then right or left to scrub."),
        WidgetNode::label("Shift is precise · Control snaps · Escape restores the whole group."),
    ]));
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
        return;
    }
    let keyed_fields: Vec<_> = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .filter_map(|(entity, key)| {
            matches!(key.0.as_str(), "location-x" | "location-y" | "scale")
                .then_some((entity, key.0.clone()))
        })
        .collect();
    for (entity, key) in keyed_fields {
        if let Some(mut field) = world.get_mut::<NumericField>(entity) {
            field.unit = Some(match key.as_str() {
                "scale" => "×".into(),
                _ => "m".into(),
            });
        }
    }
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("numeric widgets: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI Numeric Group Scrub".into(),
                resolution: WindowResolution::new(760, 480),
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

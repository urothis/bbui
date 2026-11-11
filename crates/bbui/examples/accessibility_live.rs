//! Run with `cargo run -p bbui --example accessibility_live`.
//!
//! Tab to the button and press Enter, or click it. The live status label is
//! marked as a polite, atomic live region and changes between two values.

use bbui::prelude::*;
use bbui::widgets::{Label, WidgetEvent, WidgetEventQueue, WidgetNode};
use bbui_core::{UiChildren, UiLiveRegion};
use bbui_layout::{Rect, UiEdges};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[derive(Resource)]
struct LiveDemoControls {
    button: Entity,
    status: Entity,
}

fn setup(world: &mut World) {
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("accessibility live example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(28.0),
                gap: 12.0,
                ..UiLayoutNode::column(12.0)
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 800.0, 480.0), 1.0),
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
    let tree = WidgetNode::column([
        WidgetNode::label("Live accessibility status"),
        WidgetNode::label("Tab to the button and press Enter, or click it."),
        WidgetNode::label("Waiting for a change").with_key("status"),
        WidgetNode::button("Announce status").with_key("announce"),
    ]);
    let Ok(tree_root) = bbui::widgets::WidgetTree::new(tree).spawn(world, Some(root)) else {
        eprintln!("failed to spawn accessibility example tree");
        return;
    };
    let Some(children) = world.get::<UiChildren>(tree_root) else {
        eprintln!("accessibility example children are missing");
        return;
    };
    let (Some(&status), Some(&button)) = (children.0.get(2), children.0.get(3)) else {
        eprintln!("accessibility example controls are missing");
        return;
    };
    world
        .entity_mut(status)
        .insert(UiLiveRegion::polite().atomic());
    world.insert_resource(LiveDemoControls { button, status });
}

fn change_live_value(
    mut events: ResMut<WidgetEventQueue>,
    controls: Res<LiveDemoControls>,
    mut status: Query<&mut Label>,
    mut saved: Local<bool>,
) {
    for event in events.take() {
        if matches!(event, WidgetEvent::Activated { target } if target == controls.button) {
            *saved = !*saved;
            if let Ok(mut label) = status.get_mut(controls.status) {
                label.text = if *saved {
                    "Saved successfully".into()
                } else {
                    "Changes are pending".into()
                };
            }
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI accessibility live region".into(),
                resolution: WindowResolution::new(800, 480),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            change_live_value.after(bbui_core::UiSystems::Widgets),
        )
        .run();
}

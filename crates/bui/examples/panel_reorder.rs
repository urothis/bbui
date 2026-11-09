//! Reorder panels by dragging their headers or pressing Ctrl+ArrowUp/Down.
//!
//! Run with `cargo run -p bui --example panel_reorder`. The order is saved for
//! the lifetime of the widget plugin under the group's stable key.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{PanelGroup, WidgetEventQueue};
use bui_layout::Rect;

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("panel reorder example requires a primary window");
        return;
    };
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::panel(
            "Transform",
            true,
            [
                WidgetNode::label("Move the header to change this panel's position."),
                WidgetNode::slider_range("Scale", 1.0, 0.1, 4.0, 0.1),
            ],
        )
        .with_key("transform"),
        WidgetNode::panel(
            "Material",
            false,
            [
                WidgetNode::label("The keyed order survives rebuilding this group."),
                WidgetNode::dropdown("Shading", ["Lit", "Unlit", "Wireframe"], Some(0)),
            ],
        )
        .with_key("material"),
        WidgetNode::panel(
            "Output",
            false,
            [
                WidgetNode::checkbox("Transparent", false),
                WidgetNode::text_field("File name", "scene.exr"),
            ],
        )
        .with_key("output"),
        WidgetNode::label("Ctrl+Up/Down moves the focused panel. Escape cancels a drag."),
    ]));
    let Ok(root) = tree.spawn(world, None) else {
        eprintln!("failed to spawn reorderable panel tree");
        return;
    };
    world.entity_mut(root).insert((
        UiKey("settings-panels".into()),
        PanelGroup::with_panels(["transform", "material", "output"])
            .with_reordering("settings-panels"),
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 880.0, 600.0), 1.0),
    ));
}

fn log_panel_order(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        if let bui::widgets::WidgetEvent::PanelOrderCommitted {
            persistence_key,
            before,
            after,
            ..
        } = event
        {
            info!("panel order {persistence_key}: {before:?} -> {after:?}");
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI Panel Reordering".into(),
                resolution: WindowResolution::new(880, 600),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, log_panel_order.after(bui_core::UiSystems::Widgets))
        .run();
}

//! Reorder panels by dragging their headers or pressing Ctrl+ArrowUp/Down.
//!
//! Run with `cargo run -p bbui --example panel_reorder`. The order is saved for
//! the lifetime of the widget plugin under the group's stable key.

use bbui::prelude::*;
use bbui::widgets::{PanelGroup, WidgetEventQueue};
use bbui_layout::Rect;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    if let Ok(scale) = std::env::var("BBUI_EXAMPLE_MOTION_SCALE")
        && let Ok(scale) = scale.parse::<f32>()
        && scale.is_finite()
        && scale > 0.0
    {
        world.resource_mut::<Theme>().motion_scale = scale;
    }
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
        if let bbui::widgets::WidgetEvent::PanelOrderCommitted {
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
                title: "BBUI Panel Reordering".into(),
                resolution: WindowResolution::new(880, 600),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, log_panel_order.after(bbui_core::UiSystems::Widgets))
        .run();
}

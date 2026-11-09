//! Run with `cargo run -p bui --example icon_assets`.
//!
//! This example loads an SVG through Bevy's asset server, shows load errors in the widget state,
//! and keeps the existing glyph-only IconButton path alongside image-backed icons.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{IconButtonAssetState, WidgetEventQueue};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("icon assets example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 12.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 720.0, 360.0), 1.0),
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

    let tree = bui! {
        column {
            label("Image-backed icon buttons"),
            row {
                icon_button_asset("canvas/checker.svg", "Checker image"),
                icon_button_asset("icons/not-installed.svg", "Missing icon"),
                icon_button("+", "Glyph-only icon"),
            },
            label("The missing path reports an error in IconButtonAssetState. Call retry() after installing the file."),
        }
    };
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("icon asset widget tree: {error:?}");
    }
}

fn log_asset_state_changes(
    changed: Query<
        (&IconButtonAssetState, &bui::widgets::IconButton),
        Changed<IconButtonAssetState>,
    >,
    mut events: ResMut<WidgetEventQueue>,
) {
    for (state, button) in &changed {
        info!("icon {}: {:?}", button.accessible_name, state.status);
    }
    for event in events.take() {
        info!("widget event: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI IconButton assets".into(),
                resolution: WindowResolution::new(720, 360),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, log_asset_state_changes)
        .run();
}

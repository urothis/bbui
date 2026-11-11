//! Blender-inspired object properties editor built from BBUI controls.
//!
//! Run with `cargo run -p bbui --example properties_editor`.
//! Drag numbers, toggle visibility, switch modes, collapse panels, and use the menus.

#[path = "properties_editor/shared.rs"]
mod shared;

use bbui::BbuiPlugin;
use bbui_layout::Rect;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("properties editor requires a window");
        return;
    };
    shared::mount(
        world,
        window,
        Rect::new(0.0, 0.0, shared::WIDTH, shared::HEIGHT),
        None,
    );
}

fn handle_events(world: &mut World) {
    let events = world
        .resource_mut::<bbui::widgets::WidgetEventQueue>()
        .take();
    shared::process_events(world, events);
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    file_path: shared::EXAMPLE_ASSET_PATH.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "BBUI Properties Editor".into(),
                        resolution: WindowResolution::new(
                            shared::WIDTH as u32,
                            shared::HEIGHT as u32,
                        ),
                        canvas: Some("#bbui-canvas".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_events.after(bbui_core::UiSystems::Widgets))
        .run();
}

//! Visible smoke example for the Bevy-backed BBUI renderer.

use bbui::BbuiPlugin;
use bbui_render::{ClipRect, DisplayList, DrawItem, Primitive};
use bbui_style::Rgba;
use bevy::prelude::*;

fn draw(mut commands: Commands, mut list: ResMut<DisplayList>) {
    commands.spawn(Camera2d);
    let window = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 640.0,
        height: 480.0,
    };
    let panel = ClipRect {
        x: 32.0,
        y: 32.0,
        width: 360.0,
        height: 170.0,
    };
    list.push(DrawItem {
        clip: window,
        z: 0,
        primitive: Primitive::RoundedRect {
            rect: panel,
            radius: 9.0,
            color: Rgba(0.20, 0.20, 0.20, 1.0),
        },
    });
    list.push(DrawItem {
        clip: panel,
        z: 1,
        primitive: Primitive::Text {
            origin: [52.0, 52.0],
            text: "BBUI renderer".into(),
            color: Rgba(0.9, 0.9, 0.9, 1.0),
        },
    });
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, draw)
        .run();
}

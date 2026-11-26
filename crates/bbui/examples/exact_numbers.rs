//! Full-width signed and unsigned numeric controls.
//! Run with `cargo run -p bbui --example exact_numbers`.

use bbui::prelude::*;
use bbui_layout::{Rect, UiEdges};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "exact_numbers/capture.rs"]
mod capture;

fn install_ui(world: &mut World) {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("primary window");
    let root = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("EXACT NUMERIC FIELDS").with_height(UiLength::Px(36.0)),
            WidgetNode::label("Full-width signed integers and unsigned seeds retain every digit."),
            WidgetNode::panel(
                "Signed range",
                true,
                [
                    WidgetNode::property_row(
                        "Minimum",
                        WidgetNode::exact_i64("", i64::MIN).with_key("signed-min"),
                    ),
                    WidgetNode::property_row(
                        "Maximum",
                        WidgetNode::exact_i64("", i64::MAX).with_key("signed-max"),
                    ),
                ],
            )
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Unsigned range",
                true,
                [
                    WidgetNode::property_row(
                        "Seed",
                        WidgetNode::exact_u64("", u64::MAX).with_key("seed-max"),
                    ),
                    WidgetNode::property_row(
                        "Origin",
                        WidgetNode::exact_u64("", 0).with_key("seed-zero"),
                    ),
                ],
            )
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::label("Select a field to type an exact value or expression such as x - 1."),
            WidgetNode::label("Arrow keys step exactly; copy and paste preserve all 64 bits."),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0))
        .with_gap(10.0)
        .with_padding(UiEdges::all(24.0)),
    )
    .spawn(world, None)
    .expect("numeric showcase widgets");
    let size = world.get::<Window>(window).unwrap().resolution.size();
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, size.x, size.y),
        1.0,
    ));
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_ui(world);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Exact Numbers".into(),
                resolution: WindowResolution::new(900, 600),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

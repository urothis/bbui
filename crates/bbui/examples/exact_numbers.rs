//! Full-width signed and unsigned numeric controls.
//! Run with `cargo run -p bbui --example exact_numbers`.

use bbui::prelude::*;
use bbui::widgets::NumericValue;
use bbui_layout::{Rect, UiEdges};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "exact_numbers/capture.rs"]
mod capture;

fn install_ui(world: &mut World) {
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("exact numeric example requires a primary window");
        return;
    };
    let mut slider_field = NumericField::unsigned(u64::MAX - 25);
    slider_field
        .set_typed_bounds(
            Some(NumericValue::Unsigned(u64::MAX - 100)),
            Some(NumericValue::Unsigned(u64::MAX)),
        )
        .expect("valid full-width slider bounds");
    let slider = Slider::from_field(slider_field).expect("bounded exact slider");
    let Ok(root) = WidgetTree::new(
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
            WidgetNode::panel(
                "Bounded control at the precision limit",
                true,
                [
                    WidgetNode::label("Near maximum"),
                    WidgetNode::slider("", slider)
                        .with_key("exact-slider")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(48.0)),
                    WidgetNode::label(
                        "The handle uses exact bounds; dragging to either end keeps every digit.",
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
    .spawn(world, None) else {
        eprintln!("failed to mount exact numeric widgets");
        return;
    };
    let Some(window_data) = world.get::<Window>(window) else {
        return;
    };
    let size = window_data.resolution.size();
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

fn sync_clear_color(theme: Res<Theme>, mut clear: ResMut<ClearColor>) {
    if theme.is_changed() {
        let background = theme.background;
        clear.0 = Color::srgba(background.0, background.1, background.2, background.3);
    }
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
        .add_systems(Update, sync_clear_color)
        .run();
}

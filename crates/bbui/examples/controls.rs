//! Run with `cargo run -p bbui --example controls`.
//!
//! Exercise the reusable controls, including keyboard-operable radio groups, icon buttons,
//! read-only text, disabled controls, and delayed tooltips.

use bbui::prelude::*;
use bbui::widgets::{Button, RadioGroup, WidgetEventQueue};
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
        eprintln!("controls example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(20.0),
                gap: 8.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 900.0, 600.0), 1.0),
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

    let modes = RadioGroup::new(["Object", "Edit", "Sculpt"], Some(0));
    let tree = bbui! {
        column {
            label("BBUI controls · Tab to move focus"),
            row {
                button("Apply"),
                icon_button("+", "Add object"),
                button("Disabled"),
            },
            checkbox("Visible", true),
            checkbox("Selectable", false),
            radio_group("Mode", modes),
            dropdown("Tool", ["Select", "Move", "Rotate"], Some(0)),
            text_field("Name", "Cube"),
            text_field("Read only", "Managed by the current scene"),
            text_area("Notes", "Text fields and areas support keyboard editing and IME."),
            progress(0.42),
            tooltip("Applies the current scene settings") {
                button("Apply settings"),
            },
            label("Drag through checkboxes to paint the same state; Escape cancels and Ctrl+Z undoes one brush."),
            label("Use arrows and Space/Enter in the radio group. Disabled and read-only states are visible."),
        }
    };
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
        return;
    }

    let disabled_button = world.iter_entities().find_map(|entity| {
        entity
            .get::<Button>()
            .is_some_and(|button| button.label == "Disabled")
            .then_some(entity.id())
    });
    if let Some(button) = disabled_button
        && let Some(mut button) = world.get_mut::<Button>(button)
    {
        button.enabled = false;
    }
    let read_only_field = world.iter_entities().find_map(|entity| {
        entity
            .get::<bbui_widgets::Label>()
            .is_some_and(|label| label.text == "Read only")
            .then_some(entity.id())
    });
    if let Some(field) = read_only_field {
        if let Some(mut text_field) = world.get_mut::<TextField>(field) {
            text_field.read_only = true;
        }
        world.entity_mut(field).insert(bbui_text::UiReadOnly);
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
                title: "BBUI reusable controls".into(),
                resolution: WindowResolution::new(900, 600),
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

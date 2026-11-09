//! Run with `cargo run -p bui --example numeric_expressions`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{NumericExpressionVariables, WidgetNode};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("numeric expression example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 10.0,
                ..default()
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
    if let Err(error) = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("Numeric expressions"),
        WidgetNode::number("Result", 3.0, -1_000.0, 1_000.0, 0.1).with_key("result"),
        WidgetNode::number("Tiny value", 0.000_000_42, -1.0, 1.0, 0.000_000_01),
        WidgetNode::label("Double-click Result and type an expression; Enter commits."),
        WidgetNode::label("Try 2*pi, min(4, 3), round(1.7), or x if c else y."),
        WidgetNode::label("Here c = 0 and y = 7; Escape cancels an edit."),
    ]))
    .spawn(world, Some(root))
    {
        eprintln!("numeric expression widget tree: {error:?}");
        return;
    }
    let Some(field) = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "result").then_some(entity))
    else {
        eprintln!("numeric expression result field is missing");
        return;
    };
    world
        .entity_mut(field)
        .insert(NumericExpressionVariables(HashMap::from([
            ("c".to_owned(), 0.0),
            ("y".to_owned(), 7.0),
        ])));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI Numeric Expressions".into(),
                resolution: WindowResolution::new(720, 360),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

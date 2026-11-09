//! Run with `cargo run -p bui --example text_history`.
//!
//! Undo is local while a text field or area is focused. Tab or Enter commits
//! the text session into shared widget history; Ctrl+Z on another control then
//! restores the text and its directed caret/selection. IME preedit is cancelled
//! when focus leaves unless the platform sends an IME commit first.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{WidgetEvent, WidgetEventQueue, WidgetNode, WidgetTree};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("text history example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 10.0,
                ..UiLayoutNode::column(10.0)
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 760.0, 520.0), 1.0),
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

    let _ = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("Text editing and shared undo"),
        WidgetNode::label(
            "Type in either editor. Ctrl+Z stays local while focused; leave the field to commit.",
        ),
        WidgetNode::text_field("Object name", "Cube").with_key("object-name"),
        WidgetNode::text_area(
            "Notes",
            "IME preedit stays provisional until the platform commits it. Tab cancels uncommitted composition.",
        )
        .with_key("notes"),
        WidgetNode::button("Next control"),
        WidgetNode::label(
            "After leaving an editor, Ctrl+Z on this button undoes the last committed text session.",
        ),
        WidgetNode::label(
            "Undo restores the edited buffer and its caret/selection. Ctrl+Shift+Z redoes it.",
        ),
    ]))
    .spawn(world, Some(root))
    .map_err(|error| eprintln!("text history widget tree: {error:?}"));
}

fn report_text_commits(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        if let WidgetEvent::TextCommitted {
            target,
            before,
            after,
            before_selection,
            after_selection,
            ..
        } = event
        {
            info!(
                ?target,
                %before,
                %after,
                ?before_selection,
                ?after_selection,
                "text session committed to shared undo history"
            );
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI text history".into(),
                resolution: WindowResolution::new(760, 520),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            report_text_commits.after(bui_core::UiSystems::Widgets),
        )
        .run();
}

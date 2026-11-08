//! Run with `cargo run -p bui --example menus`.
//!
//! Exercise menu-bar navigation, nested fuzzy search, context menus, popovers,
//! pie selection, and delayed tooltips. Menu commands are printed to the log.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{MenuEntry, WidgetEventQueue};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("menu example requires a primary window");
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(20.0),
                gap: 12.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 900.0, 520.0), 1.0),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: background,
            }),
        ))
        .id();

    let file_entries = [
        MenuEntry::command("new", "New Scene").with_shortcut("Ctrl+N"),
        MenuEntry::command("open", "Open Scene").with_shortcut("Ctrl+O"),
        MenuEntry::separator(),
        MenuEntry::command("save", "Save Scene").with_shortcut("Ctrl+S"),
    ];
    let edit_entries = [
        MenuEntry::command("undo", "Undo").with_shortcut("Ctrl+Z"),
        MenuEntry::command("redo", "Redo").with_shortcut("Ctrl+Shift+Z"),
        MenuEntry::submenu(
            "selection",
            "Selection",
            vec![
                MenuEntry::command("select_all", "Select All"),
                MenuEntry::command("select_none", "Select None"),
            ],
        ),
    ];
    let object_entries = [
        MenuEntry::command("duplicate", "Duplicate").with_shortcut("Shift+D"),
        MenuEntry::submenu(
            "convert",
            "Convert",
            vec![
                MenuEntry::command("convert_mesh", "Mesh"),
                MenuEntry::command("convert_curve", "Curve"),
            ],
        ),
        MenuEntry::command("delete", "Delete").with_shortcut("X"),
    ];
    let pie_entries = [
        MenuEntry::command("move", "Move"),
        MenuEntry::command("rotate", "Rotate"),
        MenuEntry::command("scale", "Scale"),
        MenuEntry::command("duplicate", "Duplicate"),
        MenuEntry::command("delete", "Delete"),
    ];

    let tree = bui! {
        column {
            menu_bar {
                menu("File", file_entries.clone()),
                menu("Edit", edit_entries.clone()),
            },
            row {
                popover("Display", [
                    MenuEntry::command("solid", "Solid"),
                    MenuEntry::command("material", "Material Preview"),
                    MenuEntry::command("rendered", "Rendered"),
                ]),
                context_menu("Object Actions", object_entries.clone()),
                pie_menu("Transform Pie", pie_entries.clone()),
            },
            tooltip("Apply changes to the current scene") {
                button("Apply"),
            },
            label("Menu keys: arrows navigate, Enter picks, Escape closes; type to fuzzy-search."),
            label("Right-click Object Actions for its context popup; use the pie control for radial selection."),
        }
    };
    tree.spawn(world, Some(root))
        .expect("valid menu example widget tree");
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
                title: "BUI menus, popovers, pie menus, and tooltips".into(),
                resolution: WindowResolution::new(900, 520),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            log_widget_events.after(bui_core::UiSystems::Widgets),
        )
        .run();
}

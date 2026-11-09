//! Run with `cargo run -p bui --example menus`.
//!
//! Exercise menu-bar navigation, nested fuzzy search, context menus, popovers,
//! pie selection, and delayed tooltips. Menu commands are printed to the log.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{Label, MenuControl, MenuEntry, WidgetEvent, WidgetEventQueue};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

#[derive(Resource)]
struct MenuExampleStatus(Entity);

fn setup(world: &mut World) {
    {
        let mut theme = world.resource_mut::<Theme>();
        let pie = &mut theme.spacing.menu;
        pie.pie_popup_diameter = 256.0;
        pie.pie_inner_inset = 56.0;
        pie.pie_inner_radius = 17.0;
        pie.pie_outer_radius = 111.0;
        pie.pie_outer_item_radius = 78.0;
        pie.pie_nested_item_radius = 46.0;
        pie.pie_item_max_width = 100.0;
    }
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("menu example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(32.0),
                gap: 12.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 900.0, 360.0), 1.0),
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
            label("MENUS & COMMAND SURFACES"),
            label("Explore navigation, contextual actions, and radial transforms."),
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
            label("Click Object Actions to open its menu, or right-click to place it at the pointer."),
            label("Use the Transform Pie by moving into a segment, then click a command."),
            label("Choose a command to see the result here."),
        }
    };
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
        return;
    }
    let status = {
        let mut labels = world.query::<(Entity, &Label)>();
        labels
            .iter(world)
            .find(|(_, label)| label.text == "Choose a command to see the result here.")
            .map(|(entity, _)| entity)
    };
    if let Some(status) = status {
        world.insert_resource(MenuExampleStatus(status));
    }
}

fn log_widget_events(
    mut events: ResMut<WidgetEventQueue>,
    mut labels: Query<&mut Label>,
    menus: Query<&MenuControl>,
    status: Option<Res<MenuExampleStatus>>,
) {
    let Some(status) = status else { return };
    for event in events.take() {
        info!("widget event: {event:?}");
        if let WidgetEvent::MenuCommand { target, command_id } = &event {
            let command_label = menus
                .get(*target)
                .ok()
                .and_then(|menu| {
                    menu.tree
                        .flattened()
                        .into_iter()
                        .find(|(_, _, entry)| entry.id == *command_id)
                        .map(|(_, _, entry)| entry.label.clone())
                })
                .unwrap_or_else(|| command_id.clone());
            if let Ok(mut label) = labels.get_mut(status.0) {
                label.text = format!("Last action: {command_label}");
            }
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI menus, popovers, pie menus, and tooltips".into(),
                resolution: WindowResolution::new(900, 360),
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

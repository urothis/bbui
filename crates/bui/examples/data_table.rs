//! Run with `cargo run -p bui --example data_table`.
//! Click headers to sort, drag a header edge to resize, and use arrows,
//! Shift-selection, PageUp/PageDown, Enter, and Ctrl+Z/Y in the focused grid.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{DataTable, TableColumn, TableRow, WidgetEventQueue};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("data table example requires a primary window");
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 10.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1120.0, 720.0), 1.0),
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

    let table = DataTable {
        columns: vec![
            TableColumn::new("Object", 190.0),
            TableColumn::new("Type", 120.0),
            TableColumn::new("Collection", 170.0),
            TableColumn::new("Vertices", 110.0),
        ],
        rows: (0_usize..240)
            .map(|index| TableRow {
                id: index as u64 + 1,
                cells: vec![
                    format!("Object_{index:03}"),
                    ["Mesh", "Camera", "Light", "Curve"][index % 4].to_owned(),
                    ["Scene", "Props", "Environment"][index % 3].to_owned(),
                    format!("{}", 128 + index * 7),
                ],
            })
            .collect(),
        ..DataTable::default()
    };
    let widgets = bui! {
        column {
            label("DataTable · stable row IDs · virtualized cells"),
            data_table("Scene Objects", table),
            label("Click a header to sort · drag its right edge to resize · arrows move the active cell"),
            label("Ctrl-click toggles rows · Shift-click selects a range · PageUp/PageDown scroll · Enter activates"),
            label("Ctrl+Z / Ctrl+Y undo or redo sorting, resizing, and selection edits"),
        }
    };
    widgets
        .spawn(world, Some(root))
        .expect("valid data table declaration");
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("table: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI DataTable".into(),
                resolution: WindowResolution::new(1120, 720),
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

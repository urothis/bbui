//! Run with `cargo run -p bui --example data_table`.
//! Click headers to sort, drag a divider to resize, and use arrows,
//! Shift-selection, PageUp/PageDown, Enter, and Ctrl+Z/Y in the focused grid.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{DataTable, TableColumn, TableRow, WidgetEventQueue};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};
use bui_style::Rgba;

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 820.0;

fn title_style(color: Rgba, font_size: f32) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::Foreground, UiStyleValue::Color(color));
    style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(font_size));
    style
}

fn scene_rows() -> Vec<TableRow> {
    const ARCHETYPES: [(&str, &str, &str, u32, u32); 8] = [
        ("SM_Cliff_Rock", "Mesh", "Environment", 4_832, 2),
        ("SM_Pine_Tree", "Mesh", "Foliage", 2_360, 4),
        ("SM_Stone_Arch", "Mesh", "Architecture", 18_240, 1),
        ("SM_Grass_Patch", "Mesh", "Ground Cover", 512, 1),
        ("CAM_Main", "Camera", "Cameras", 0, 0),
        ("LGT_Sun", "Area Light", "Lighting", 0, 0),
        ("VOL_Low_Fog", "Volume", "Atmosphere", 0, 0),
        ("CRV_River_Bank", "Curve", "Waterways", 0, 0),
    ];
    const UPDATE_LABELS: [&str; 6] = [
        "Just now",
        "4 min ago",
        "12 min ago",
        "18 min ago",
        "32 min ago",
        "1 hr ago",
    ];

    (0..240)
        .map(|index| {
            let archetype_index = index % ARCHETYPES.len();
            let instance = index / ARCHETYPES.len() + 1;
            let (name, object_type, collection, base_vertices, base_materials) =
                ARCHETYPES[archetype_index];
            let vertices = if base_vertices == 0 {
                0
            } else {
                base_vertices + instance as u32 * (archetype_index as u32 * 29 + 11)
            };
            let materials = if base_materials == 0 {
                0
            } else {
                base_materials + (instance as u32 % 3)
            };

            TableRow {
                id: index as u64 + 1,
                cells: vec![
                    format!("{name}_{instance:03}"),
                    object_type.to_owned(),
                    collection.to_owned(),
                    if index % 17 == 0 {
                        "Hidden".to_owned()
                    } else {
                        "Visible".to_owned()
                    },
                    vertices.to_string(),
                    materials.to_string(),
                    UPDATE_LABELS[index % UPDATE_LABELS.len()].to_owned(),
                ],
            }
        })
        .collect()
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("data table example requires a primary window");
    let theme = world.resource::<Theme>();
    let background = theme.background;
    let title = title_style(theme.text, 24.0);
    let subtitle = title_style(theme.text_muted, 14.0);
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(28.0),
                gap: 12.0,
                ..default()
            },
            UiLayoutRoot::new(
                window,
                Rect::new(0.0, 0.0, WINDOW_WIDTH, WINDOW_HEIGHT),
                1.0,
            ),
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

    let mut table = DataTable {
        columns: vec![
            TableColumn::new("Object", 300.0),
            TableColumn::new("Type", 126.0),
            TableColumn::new("Collection", 208.0),
            TableColumn::new("Visibility", 136.0),
            TableColumn::new("Vertices", 140.0),
            TableColumn::new("Materials", 140.0),
            TableColumn::new("Updated", 174.0),
        ],
        rows: scene_rows(),
        row_height: 28,
        ..DataTable::default()
    };
    table.select_cell(1, 0, false, false);

    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::column([
            WidgetNode::label("SCENE INVENTORY")
                .with_height(UiLength::Px(30.0))
                .with_style(title),
            WidgetNode::label("MAIN SCENE  /  OBJECTS, COLLECTIONS & RENDER METRICS")
                .with_height(UiLength::Px(20.0))
                .with_style(subtitle.clone()),
        ])
        .with_gap(2.0),
        WidgetNode::data_table("Scene objects", table)
            .with_key("scene-objects")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(648.0)),
        WidgetNode::column([
            WidgetNode::label(
                "Click headers to sort  ·  Drag dividers to resize  ·  Ctrl-click toggles rows  ·  Shift-click selects a range",
            )
            .with_height(UiLength::Px(18.0))
            .with_style(subtitle.clone()),
            WidgetNode::label(
                "Arrows move the active cell  ·  Page Up / Down scroll  ·  Enter activates  ·  Ctrl+Z / Ctrl+Y undo / redo",
            )
            .with_height(UiLength::Px(18.0))
            .with_style(subtitle),
        ])
        .with_gap(0.0),
    ]));
    tree.spawn(world, Some(root))
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
                title: "BUI DataTable · Scene Inventory".into(),
                resolution: WindowResolution::new(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32),
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

//! Run with `cargo run -p bbui --example collections`.
//! Rows and tiles are virtualized. Use Ctrl/Shift for multiple selection,
//! F2 to rename, drag to reorder, and arrows/Home/End/PageUp/PageDown to move.

use bbui::prelude::*;
use bbui::widgets::{ListItem, PreviewGridItem, TreeNode, WidgetEventQueue};
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
        eprintln!("collections example requires a primary window");
        return;
    };
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 10.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 760.0), 1.0),
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

    let assets = ListModel {
        items: (0..500)
            .map(|index| ListItem {
                id: index + 1,
                value: format!("Asset_{index:03}.mesh"),
                enabled: index % 17 != 0,
            })
            .collect(),
        ..ListModel::default()
    };
    let outliner = TreeModel {
        nodes: vec![
            TreeNode {
                id: 1,
                parent: None,
                children: vec![2, 3, 4],
                value: "Scene".to_owned(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![5, 6],
                value: "Environment".to_owned(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![7, 8],
                value: "Characters".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 4,
                parent: Some(1),
                children: vec![9],
                value: "Lighting".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 5,
                parent: Some(2),
                children: vec![],
                value: "Ground".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 6,
                parent: Some(2),
                children: vec![],
                value: "Mountains".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 7,
                parent: Some(3),
                children: vec![],
                value: "Explorer".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 8,
                parent: Some(3),
                children: vec![],
                value: "Robot".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 9,
                parent: Some(4),
                children: vec![],
                value: "Key light".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..TreeModel::default()
    };
    let materials = PreviewGrid {
        items: (0..56)
            .map(|index| {
                let mut item = PreviewGridItem::new(index + 1, format!("Mat_{index:02}"));
                let hue = index as f32 / 56.0;
                item.tint = [0.36 + hue * 0.36, 0.44 + hue * 0.2, 0.8 - hue * 0.35, 1.0];
                item
            })
            .collect(),
        cell_width: 112.0,
        cell_height: 104.0,
        ..PreviewGrid::default()
    };

    let widgets = bbui! {
        column {
            label("Collections · stable ids · virtualized rows and tiles"),
            row {
                list("Assets", assets),
                tree("Scene Outliner", outliner),
                preview_grid("Materials", materials),
            },
            label("Click selects · Ctrl/Shift extends selection · F2 renames · drag reorders or reparents tree nodes"),
            label("Arrow keys navigate · tree Left/Right collapses or expands · Enter activates · Ctrl+Z/Y undo and redo"),
        }
    };
    if let Err(error) = widgets.spawn(world, Some(root)) {
        eprintln!("collection widget tree: {error:?}");
    }
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("collection: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Collections".into(),
                resolution: WindowResolution::new(1280, 760),
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

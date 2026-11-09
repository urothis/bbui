use super::*;
use crate::WidgetPlugin;
use crate::{ListItem, PanelOrderStore, WidgetNode, WidgetTree};
use bevy_app::App;
use bui_core::{
    CorePlugin, UiChildren, UiFocusable, UiHitBox, UiInputEvent, UiInputQueue, UiKey, UiKeyCode,
    UiModifiers, UiNode, UiRect, UiRoutedEvent, UiRoutedEvents, UiTabIndex,
};
use bui_style::{UiStyleDeclaration, UiStyleRule, UiStyleSelector};

fn browser_items(world: &mut World, owner: Entity) -> Vec<(Entity, FileBrowserItem)> {
    let mut query = world.query::<(Entity, &FileBrowserItem)>();
    query
        .iter(world)
        .filter(|(entity, item)| {
            item.owner == owner
                && world
                    .get::<UiVisible>(*entity)
                    .is_some_and(|visible| visible.0)
        })
        .map(|(entity, item)| (entity, item.clone()))
        .collect()
}

fn collection_items(world: &mut World, owner: Entity) -> Vec<(Entity, CollectionItemControl)> {
    let mut query = world.query::<(Entity, &CollectionItemControl)>();
    query
        .iter(world)
        .filter(|(_, item)| item.owner == owner)
        .map(|(entity, item)| (entity, item.clone()))
        .collect()
}

fn data_table_cells(world: &mut World, owner: Entity) -> Vec<(Entity, DataTableCellControl)> {
    let mut query = world.query::<(Entity, &DataTableCellControl)>();
    query
        .iter(world)
        .filter(|(_, cell)| cell.owner == owner)
        .map(|(entity, cell)| (entity, cell.clone()))
        .collect()
}

fn data_table_headers(world: &mut World, owner: Entity) -> Vec<(Entity, DataTableHeaderControl)> {
    let mut query = world.query::<(Entity, &DataTableHeaderControl)>();
    query
        .iter(world)
        .filter(|(_, header)| header.owner == owner)
        .map(|(entity, header)| (entity, header.clone()))
        .collect()
}

fn sample_table(rows: usize) -> DataTable {
    DataTable {
        columns: vec![
            crate::TableColumn::new("Name", 104.0),
            crate::TableColumn::new("Type", 92.0),
        ],
        rows: (0..rows)
            .map(|index| crate::TableRow {
                id: index as u64 + 1,
                cells: vec![
                    format!("Object {index:03}"),
                    if index % 2 == 0 {
                        "Mesh".into()
                    } else {
                        "Light".into()
                    },
                ],
            })
            .collect(),
        ..DataTable::default()
    }
}

#[path = "tests/virtual_views_and_ramp_input.rs"]
mod virtual_views_and_ramp_input;
use virtual_views_and_ramp_input::*;
#[path = "tests/color_and_menu_paint.rs"]
mod color_and_menu_paint;
#[path = "tests/gestures_and_initial_paint.rs"]
mod gestures_and_initial_paint;
#[path = "tests/incremental_paint_and_theme.rs"]
mod incremental_paint_and_theme;
#[path = "tests/paint_and_text_editing.rs"]
mod paint_and_text_editing;
use paint_and_text_editing::*;
#[path = "tests/collections_tables_and_graph.rs"]
mod collections_tables_and_graph;
#[path = "tests/curves_and_collections.rs"]
mod curves_and_collections;
#[path = "tests/file_browser_history.rs"]
mod file_browser_history;
#[path = "tests/menus_and_panels.rs"]
mod menus_and_panels;
#[path = "tests/options_and_menu_keyboard.rs"]
mod options_and_menu_keyboard;
#[path = "tests/panels_text_and_curves.rs"]
mod panels_text_and_curves;
#[path = "tests/table_graph_and_browser.rs"]
mod table_graph_and_browser;

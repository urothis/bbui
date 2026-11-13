use super::*;
use crate::WidgetPlugin;
use crate::{ListItem, PanelOrderStore, WidgetNode, WidgetTree};
use bbui_core::{
    CorePlugin, UiChildren, UiFocusable, UiHitBox, UiInputEvent, UiInputQueue, UiKey, UiKeyCode,
    UiModifiers, UiNode, UiRect, UiRoutedEvent, UiRoutedEvents, UiTabIndex,
};
use bbui_style::{UiStyleDeclaration, UiStyleRule, UiStyleSelector};
use bevy_app::App;

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

#[path = "tests/browser_input.rs"]
mod browser_input;
#[path = "tests/checkbox_gestures.rs"]
mod checkbox_gestures;
#[path = "tests/collection_browser_input.rs"]
mod collection_browser_input;
#[path = "tests/collections_tree_preview.rs"]
mod collections_tree_preview;
#[path = "tests/color_editor_paint.rs"]
mod color_editor_paint;
#[path = "tests/curve_input.rs"]
mod curve_input;
#[path = "tests/data_table_views.rs"]
mod data_table_views;
#[path = "tests/editor_semantic_paint.rs"]
mod editor_semantic_paint;
#[path = "tests/file_browser_history.rs"]
mod file_browser_history;
#[path = "tests/focus_text_history.rs"]
mod focus_text_history;
#[path = "tests/graph_input.rs"]
mod graph_input;
#[path = "tests/graph_views_and_ports.rs"]
mod graph_views_and_ports;
#[path = "tests/incremental_paint_cache.rs"]
mod incremental_paint_cache;
#[path = "tests/initial_paint.rs"]
mod initial_paint;
#[path = "tests/menu_keyboard.rs"]
mod menu_keyboard;
#[path = "tests/menu_tooltip_routing.rs"]
mod menu_tooltip_routing;
#[path = "tests/numeric_slider_gestures.rs"]
mod numeric_slider_gestures;
#[path = "tests/option_controls.rs"]
mod option_controls;
#[path = "tests/paint_controls.rs"]
mod paint_controls;
#[path = "tests/paint_order_and_theme.rs"]
mod paint_order_and_theme;
#[path = "tests/panel_gestures.rs"]
mod panel_gestures;
#[path = "tests/panel_runtime.rs"]
mod panel_runtime;
#[path = "tests/ramp_input.rs"]
mod ramp_input;
#[path = "tests/table_input.rs"]
mod table_input;
#[path = "tests/text_editing_history.rs"]
mod text_editing_history;
#[path = "tests/textarea_layout_input.rs"]
mod textarea_layout_input;
#[path = "tests/tooltip_menu_paint.rs"]
mod tooltip_menu_paint;
#[path = "tests/virtual_view_sync.rs"]
mod virtual_view_sync;

use ramp_input::*;
use text_editing_history::*;
use virtual_view_sync::*;

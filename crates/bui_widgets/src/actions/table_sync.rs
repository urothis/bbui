use super::*;

/// Materialize only table columns and rows intersecting the laid out viewport.
/// Child entities remain keyed by column index and stable row ID as sorting and
/// scrolling change their positions.
pub fn sync_data_table_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let title_height = theme.control_height.max(1.0);
    let column_header_height = theme.spacing.table.header_height.max(1.0);

    let mut query = world.query::<(Entity, &DataTable, Option<&UiLayoutResult>)>();
    let owners = query
        .iter(world)
        .filter_map(|(owner, _, layout)| {
            virtual_view_stamp::<DataTable>(world, owner)
                .map(|stamp| (owner, layout.copied(), stamp))
        })
        .collect::<Vec<_>>();
    let live_owners: HashSet<_> = owners.iter().map(|(owner, _, _)| *owner).collect();
    let mut previous = {
        let mut items = world.resource_mut::<WidgetDataTableItems>();
        std::mem::take(&mut items.items)
    };
    let mut stamps = {
        let mut cache = world.resource_mut::<WidgetDataTableItems>();
        std::mem::take(&mut cache.stamps)
    };
    let mut viewport_ticks = {
        let mut cache = world.resource_mut::<WidgetDataTableItems>();
        std::mem::take(&mut cache.viewport_ticks)
    };
    let mut active_row_positions = {
        let mut cache = world.resource_mut::<WidgetDataTableItems>();
        std::mem::take(&mut cache.active_row_positions)
    };
    let mut views = Vec::with_capacity(owners.len());
    for (owner, layout, stamp) in owners.iter().copied() {
        if stamps.get(&owner) == Some(&stamp) && previous.contains_key(&owner) {
            continue;
        }
        let Some(layout) = layout else {
            views.push(DataTableView {
                owner,
                items: Vec::new(),
            });
            continue;
        };
        let Some(table) = world.get::<DataTable>(owner) else {
            continue;
        };
        let viewport_only = viewport_ticks.get(&owner) == Some(&stamp.model_changed)
            && previous.contains_key(&owner)
            && stamps.get(&owner).is_some_and(|old| {
                old.layout_changed == stamp.layout_changed
                    && old.theme_changed == stamp.theme_changed
                    && old.style_changed == stamp.style_changed
                    && old.key_changed == stamp.key_changed
            });
        let rect = layout.rect;
        let clip = layout
            .clip
            .unwrap_or(rect)
            .intersection(rect)
            .unwrap_or_default();
        let header_y = rect.y + title_height;
        let body = Rect {
            x: rect.x,
            y: header_y + column_header_height,
            width: rect.width.max(0.0),
            height: (rect.height - title_height - column_header_height).max(0.0),
        };
        let body_clip = clip.intersection(body).unwrap_or_default();
        let row_height = if table.row_height == 24 {
            theme.spacing.table.row_height
        } else {
            table.row_height.max(1) as f32
        };
        let page_rows = (body.height / row_height).floor().max(1.0) as usize;
        let max_scroll = table.rows.len().saturating_sub(page_rows);
        let mut scroll_rows = table.scroll_rows.min(max_scroll);
        let active_row = if viewport_only {
            active_row_positions.get(&owner).copied().flatten()
        } else {
            let position = table
                .selection
                .active
                .and_then(|active| table.rows.iter().position(|row| row.id == active));
            active_row_positions.insert(owner, position);
            position
        };
        if let Some(active_row) = active_row {
            if active_row < scroll_rows {
                scroll_rows = active_row;
            } else if active_row >= scroll_rows.saturating_add(page_rows) {
                scroll_rows = active_row + 1 - page_rows;
            }
        }

        let content_width: f32 = table.columns.iter().map(|column| column.width).sum();
        let max_scroll_x = (content_width - body.width).max(0.0);
        let scroll_x = if table.scroll_x.is_finite() {
            table.scroll_x.clamp(0.0, max_scroll_x)
        } else {
            0.0
        };

        let visible_columns: Vec<_> = table
            .columns
            .iter()
            .enumerate()
            .scan(rect.x - scroll_x, |x, (column_index, column)| {
                let item_x = *x;
                *x += column.width;
                let item_rect = Rect {
                    x: item_x,
                    y: header_y,
                    width: column.width,
                    height: column_header_height,
                };
                Some((column_index, item_x, item_rect))
            })
            .filter(|(_, _, item_rect)| item_rect.intersection(clip).is_some())
            .map(|(column_index, item_x, _)| (column_index, item_x))
            .collect();
        let mut items = Vec::new();
        for (column_index, x) in visible_columns {
            let column = &table.columns[column_index];
            let header_rect = Rect {
                x,
                y: header_y,
                width: column.width,
                height: column_header_height,
            };
            let retained = viewport_only
                && previous
                    .get(&owner)
                    .and_then(|items| items.get(&DataTableItemKey::Header(column_index)))
                    .is_some_and(|entity| world.get::<DataTableHeaderControl>(*entity).is_some());
            let header = (!retained).then(|| DataTableHeaderControl {
                owner,
                column: column_index,
                label: column.label.clone(),
                width: column.width,
                sortable: column.sortable,
                sort_ascending: table
                    .sort
                    .filter(|(sorted, _)| *sorted == column_index)
                    .map(|(_, ascending)| ascending),
                active: table.active_column == column_index,
            });
            items.push(DataTableViewItem {
                key: DataTableItemKey::Header(column_index),
                header,
                cell: None,
                rect: header_rect,
                clip: clip.intersection(header_rect).unwrap_or_default(),
            });
        }

        let visible_count = (body.height / row_height).ceil() as usize + 1;
        let start = scroll_rows.min(table.rows.len());
        let end = start.saturating_add(visible_count).min(table.rows.len());
        for row_index in start..end {
            let row = &table.rows[row_index];
            let y = body.y + (row_index - start) as f32 * row_height;
            for (column_index, x) in table
                .columns
                .iter()
                .enumerate()
                .scan(rect.x - scroll_x, |x, (column_index, column)| {
                    let item_x = *x;
                    *x += column.width;
                    let item_rect = Rect {
                        x: item_x,
                        y,
                        width: column.width,
                        height: row_height,
                    };
                    Some((column_index, item_x, item_rect))
                })
                .filter(|(_, _, item_rect)| item_rect.intersection(body_clip).is_some())
                .map(|(column_index, item_x, _)| (column_index, item_x))
            {
                let column = &table.columns[column_index];
                let Some(value) = row.cells.get(column_index) else {
                    continue;
                };
                let cell_rect = Rect {
                    x,
                    y,
                    width: column.width,
                    height: row_height,
                };
                let key = DataTableItemKey::Cell {
                    row_id: row.id,
                    column: column_index,
                };
                let retained = viewport_only
                    && previous
                        .get(&owner)
                        .and_then(|items| items.get(&key))
                        .is_some_and(|entity| world.get::<DataTableCellControl>(*entity).is_some());
                let cell = (!retained).then(|| DataTableCellControl {
                    owner,
                    row_id: row.id,
                    row_index,
                    column: column_index,
                    column_label: column.label.clone(),
                    value: value.clone(),
                    selected: table.selection.contains(row.id),
                    active: table.selection.active == Some(row.id)
                        && table.active_column == column_index,
                });
                items.push(DataTableViewItem {
                    key,
                    header: None,
                    cell,
                    rect: cell_rect,
                    clip: body_clip.intersection(cell_rect).unwrap_or_default(),
                });
            }
        }

        if world.get::<DataTable>(owner).is_some_and(|current| {
            current.scroll_rows != scroll_rows || current.scroll_x != scroll_x
        }) && let Some(mut current) = world.get_mut::<DataTable>(owner)
        {
            current.scroll_rows = scroll_rows;
            current.scroll_x = scroll_x;
        }
        views.push(DataTableView { owner, items });
    }

    let did_sync = !views.is_empty();
    for view in views {
        let mut current_items = previous.remove(&view.owner).unwrap_or_default();
        let mut mounted_keys = HashSet::with_capacity(view.items.len());
        let owner_layout = world.get::<UiLayoutResult>(view.owner).copied();
        if let Some(owner_layout) = owner_layout {
            for view_item in view.items {
                mounted_keys.insert(view_item.key);
                let entity = current_items
                    .get(&view_item.key)
                    .copied()
                    .filter(|entity| world.get_entity(*entity).is_ok())
                    .unwrap_or_else(|| {
                        world.spawn((UiNode, UiEnabled(true), UiVisible(true))).id()
                    });
                let clip = view_item
                    .clip
                    .intersection(owner_layout.clip.unwrap_or(owner_layout.rect))
                    .unwrap_or_default();
                let z = owner_layout.z.saturating_add(20);
                insert_component_if_changed(world, entity, UiEnabled(true));
                insert_component_if_changed(world, entity, UiVisible(true));
                insert_component_if_changed(
                    world,
                    entity,
                    UiLayoutResult {
                        window: owner_layout.window,
                        rect: view_item.rect,
                        clip: Some(clip),
                        z,
                        content_size: [view_item.rect.width, view_item.rect.height],
                        dpi_scale: owner_layout.dpi_scale,
                    },
                );
                insert_component_if_changed(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(owner_layout.window),
                        rect: view_item.rect,
                        clip: Some(clip),
                        z_index: z,
                    },
                );
                if let Some(header) = view_item.header {
                    insert_component_if_changed(world, entity, header.clone());
                    let header_role = UiRole::new("table-header");
                    insert_component_if_changed(world, entity, header_role);
                    set_ui_style_state(world, entity, UiStyleState::ACTIVE, header.active);
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, false);
                    remove_component_if_present::<DataTableCellControl>(world, entity);
                } else if let Some(cell) = view_item.cell {
                    insert_component_if_changed(world, entity, cell.clone());
                    let cell_role = UiRole::new("table-cell");
                    insert_component_if_changed(world, entity, cell_role);
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, cell.selected);
                    set_ui_style_state(world, entity, UiStyleState::ACTIVE, cell.active);
                    remove_component_if_present::<DataTableHeaderControl>(world, entity);
                }
                let _ = set_parent(world, entity, view.owner);
                current_items.insert(view_item.key, entity);
            }
        }
        let stale_keys: Vec<_> = current_items
            .keys()
            .filter(|key| !mounted_keys.contains(key))
            .copied()
            .collect();
        for key in stale_keys {
            if let Some(entity) = current_items.remove(&key)
                && world.get_entity(entity).is_ok()
            {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
        previous.insert(view.owner, current_items);
    }
    let stale_owners: Vec<_> = previous
        .keys()
        .filter(|owner| !live_owners.contains(owner))
        .copied()
        .collect();
    for owner in stale_owners {
        let Some(items) = previous.remove(&owner) else {
            continue;
        };
        for entity in items.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
    }
    for (owner, _, stamp) in owners {
        stamps.insert(owner, stamp);
    }
    stamps.retain(|owner, _| live_owners.contains(owner));
    viewport_ticks.clear();
    active_row_positions.retain(|owner, _| live_owners.contains(owner));
    let mut cache = world.resource_mut::<WidgetDataTableItems>();
    cache.items = previous;
    cache.stamps = stamps;
    cache.viewport_ticks = viewport_ticks;
    cache.active_row_positions = active_row_positions;
    if did_sync {
        world.increment_change_tick();
    }
}

use super::*;

/// Spawn and update only file rows that intersect the current list/grid viewport.
/// Per-owner component ticks bridge direct ECS mutations into a bounded dirty-owner set; the
/// cached filtered index is maintained by FileBrowser's mutation methods.
pub fn sync_file_browser_items(world: &mut World) {
    let spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.file_browser,
        |theme| theme.spacing.file_browser,
    );
    let theme_changed = world
        .get_resource_change_ticks::<Theme>()
        .map(|ticks| ticks.changed.get());
    let mut query = world.query::<(
        Entity,
        &FileBrowser,
        Option<&UiLayoutResult>,
        Option<&FileBrowserViewState>,
    )>();
    let owners: Vec<_> = query
        .iter(world)
        .filter_map(|(owner, _, layout, view)| {
            Some((
                owner,
                layout.copied(),
                view.copied(),
                FileBrowserVirtualStamp {
                    model_changed: entity_component_changed_tick::<FileBrowser>(world, owner)?,
                    layout_changed: entity_component_changed_tick::<UiLayoutResult>(world, owner),
                    view_changed: entity_component_changed_tick::<FileBrowserViewState>(
                        world, owner,
                    ),
                    theme_changed,
                    key_changed: entity_component_changed_tick::<UiKey>(world, owner),
                    visible_changed: entity_component_changed_tick::<UiVisible>(world, owner),
                    enabled_changed: entity_component_changed_tick::<UiEnabled>(world, owner),
                },
            ))
        })
        .collect();
    let live_owners: HashSet<_> = owners.iter().map(|(owner, _, _, _)| *owner).collect();
    let dirty_owners: Vec<_> = owners
        .iter()
        .filter(|(owner, _, _, stamp)| {
            let cache = world.resource::<WidgetFileBrowserItems>();
            cache.stamps.get(owner) != Some(stamp) || !cache.items.contains_key(owner)
        })
        .copied()
        .collect();
    let stale_owners: Vec<_> = {
        let cache = world.resource::<WidgetFileBrowserItems>();
        cache
            .items
            .keys()
            .chain(cache.stamps.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|owner| !live_owners.contains(owner))
            .collect()
    };
    if dirty_owners.is_empty() && stale_owners.is_empty() {
        return;
    }

    let mut updates = Vec::with_capacity(dirty_owners.len());
    for (owner, layout, view, stamp) in dirty_owners {
        let mut rows = Vec::new();
        if let (Some(layout), Some(browser)) = (layout, world.get::<FileBrowser>(owner)) {
            let view = view.unwrap_or_default();
            let header_height = if view.header_height == 48.0 {
                spacing.header_height
            } else {
                view.header_height
            }
            .max(1.0);
            let list_row_height = if view.row_height == 22.0 {
                spacing.list_row_height
            } else {
                view.row_height
            }
            .max(1.0);
            let width = layout.rect.width;
            let height = layout.rect.height;
            let (columns, row_height, start_index, max_visible) = if browser.view == FileView::List
            {
                let rows = ((height - header_height) / list_row_height).ceil().max(1.0) as usize;
                (1, list_row_height, view.scroll_rows, rows)
            } else {
                let cell_stride = spacing.grid_cell_width + spacing.grid_gap;
                let row_stride = spacing.grid_cell_height + spacing.grid_gap;
                let columns = (width / cell_stride).floor().max(1.0) as usize;
                let rows = ((height - header_height) / row_stride).ceil().max(1.0) as usize;
                (
                    columns,
                    row_stride,
                    view.scroll_rows.saturating_mul(columns),
                    rows.saturating_mul(columns),
                )
            };
            for (slot, entry) in browser
                .visible_range(start_index, max_visible)
                .into_iter()
                .enumerate()
            {
                let index = start_index + slot;
                let (x, y, item_width, item_height) = if browser.view == FileView::List {
                    (
                        layout.rect.x,
                        layout.rect.y + header_height + slot as f32 * row_height,
                        width,
                        row_height,
                    )
                } else {
                    let col = slot % columns;
                    let row = slot / columns;
                    (
                        layout.rect.x + col as f32 * (spacing.grid_cell_width + spacing.grid_gap),
                        layout.rect.y + header_height + row as f32 * row_height,
                        spacing.grid_cell_width,
                        spacing.grid_cell_height,
                    )
                };
                rows.push((
                    FileBrowserItem {
                        owner,
                        path: entry.path.clone(),
                        name: entry.name.clone(),
                        index,
                        directory: entry.directory,
                        size: entry.size,
                        selected: browser.selected.contains(&entry.path),
                        preview: entry.preview,
                    },
                    Rect {
                        x,
                        y,
                        width: item_width,
                        height: item_height,
                    },
                    layout.window,
                    layout.clip,
                    layout.dpi_scale,
                    layout.z,
                ));
            }
        }
        updates.push((owner, stamp, rows));
    }

    let mut previous = {
        let mut cache = world.resource_mut::<WidgetFileBrowserItems>();
        std::mem::take(&mut cache.items)
    };
    let mut stamps = {
        let mut cache = world.resource_mut::<WidgetFileBrowserItems>();
        std::mem::take(&mut cache.stamps)
    };
    for (owner, stamp, rows) in updates {
        let mut cache = previous.remove(&owner).unwrap_or_default();
        let mut visible_paths = HashSet::new();
        for (item, rect, window, inherited_clip, dpi_scale, base_z) in rows {
            visible_paths.insert(item.path.clone());
            let existing = cache
                .get(&item.path)
                .copied()
                .filter(|entity| world.get_entity(*entity).is_ok());
            let entity = existing.unwrap_or_else(|| world.spawn_empty().id());
            cache.insert(item.path.clone(), entity);
            let clip = inherited_clip
                .and_then(|clip| clip.intersection(rect))
                .unwrap_or(rect);
            let z = base_z.saturating_add(20);
            let owner_visible = world
                .get::<UiVisible>(owner)
                .is_none_or(|visible| visible.0);
            let owner_enabled = world
                .get::<UiEnabled>(owner)
                .is_none_or(|enabled| enabled.0);
            let layout = UiLayoutResult {
                window,
                rect,
                clip: Some(clip),
                z,
                content_size: [rect.width, rect.height],
                dpi_scale,
            };
            let hitbox = UiHitBox {
                window: Some(window),
                rect,
                clip: Some(clip),
                z_index: z,
            };
            if existing.is_some() {
                set_enabled(world, entity, owner_enabled);
                set_visible(world, entity, owner_visible);
                insert_component_if_changed(world, entity, item);
                set_layout_result(world, entity, layout);
                set_hitbox(world, entity, hitbox);
            } else {
                world.entity_mut(entity).insert((
                    UiNode,
                    UiEnabled(owner_enabled),
                    UiVisible(owner_visible),
                    item,
                    layout,
                    hitbox,
                ));
            }
            if world
                .get::<UiParent>(entity)
                .is_none_or(|parent| parent.0 != owner)
            {
                let _ = set_parent(world, entity, owner);
            }
        }
        let stale: Vec<_> = cache
            .keys()
            .filter(|path| !visible_paths.contains(*path))
            .cloned()
            .collect();
        for path in stale {
            let Some(entity) = cache.get(&path).copied() else {
                continue;
            };
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
                cache.remove(&path);
            } else {
                cache.remove(&path);
            }
        }
        previous.insert(owner, cache);
        stamps.insert(owner, stamp);
    }
    for owner in stale_owners {
        if let Some(items) = previous.remove(&owner) {
            for entity in items.into_values() {
                if world.get_entity(entity).is_ok() {
                    detach_from_parent(world, entity);
                    world.despawn(entity);
                }
            }
        }
        stamps.remove(&owner);
    }
    previous.retain(|owner, _| live_owners.contains(owner));
    stamps.retain(|owner, _| live_owners.contains(owner));
    let mut cache = world.resource_mut::<WidgetFileBrowserItems>();
    cache.items = previous;
    cache.stamps = stamps;
}

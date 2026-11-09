use super::*;

/// Mount real, clipped Bevy item entities for the rows and preview tiles that
/// intersect each collection viewport. Model ids remain the stable item keys
/// even when virtualization despawns an offscreen entity. Component ticks form
/// the direct ECS bridge into a dirty-owner set, so unchanged models skip row enumeration.
pub fn sync_collection_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let collection_spacing = theme.spacing.collection;
    let list_spacing = theme.spacing.list;
    let header_height = collection_spacing.header_height.max(1.0);
    let item_gap = collection_spacing.item_gap.max(0.0);

    let mut query = world.query::<(
        Entity,
        Option<&ListModel<String>>,
        Option<&TreeModel<String>>,
        Option<&PreviewGrid>,
        Option<&UiLayoutResult>,
    )>();
    let owners: Vec<_> = query
        .iter(world)
        .filter(|(_, list, tree, grid, _)| list.is_some() || tree.is_some() || grid.is_some())
        .filter_map(|(owner, _, _, _, layout)| {
            collection_virtual_stamp(world, owner).map(|stamp| (owner, layout.copied(), stamp))
        })
        .collect();
    let live_owners: HashSet<_> = owners.iter().map(|(owner, _, _)| *owner).collect();
    let dirty_owners: Vec<_> = owners
        .iter()
        .filter(|(owner, _, stamp)| {
            let cache = world.resource::<WidgetCollectionItems>();
            cache.stamps.get(owner) != Some(stamp) || !cache.items.contains_key(owner)
        })
        .copied()
        .collect();
    let stale_owners: Vec<_> = {
        let cache = world.resource::<WidgetCollectionItems>();
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
    let mut views = Vec::with_capacity(dirty_owners.len());

    for (owner, layout, stamp) in dirty_owners {
        let mut list = world.get::<ListModel<String>>(owner).cloned();
        let mut tree = world.get::<TreeModel<String>>(owner).cloned();
        let mut grid = world.get::<PreviewGrid>(owner).cloned();
        let Some(layout) = layout else {
            views.push(CollectionView {
                owner,
                stamp,
                items: Vec::new(),
            });
            continue;
        };
        let viewport = Rect {
            x: layout.rect.x,
            y: layout.rect.y + header_height,
            width: layout.rect.width.max(0.0),
            height: (layout.rect.height - header_height).max(0.0),
        };
        let viewport_clip = layout
            .clip
            .and_then(|clip| clip.intersection(viewport))
            .unwrap_or(viewport);
        let mut items = Vec::new();

        if let Some(model) = list.as_mut() {
            let row_height = if model.row_height == 22.0 {
                list_spacing.row_height
            } else {
                model.row_height
            }
            .max(1.0);
            let content_height = model.items.len() as f32 * row_height;
            let max_offset = (content_height - viewport.height).max(0.0);
            model.scroll_offset = model.scroll_offset.clamp(0.0, max_offset);
            if viewport.height > 0.0
                && let Some(active) = model.selection.active
                && let Some(index) = model.items.iter().position(|item| item.id == active)
            {
                let top = index as f32 * row_height;
                let bottom = top + row_height;
                if top < model.scroll_offset {
                    model.scroll_offset = top;
                } else if bottom > model.scroll_offset + viewport.height {
                    model.scroll_offset = (bottom - viewport.height).min(max_offset);
                }
            }
            let actual_offset = model.scroll_offset;
            let start = (actual_offset / row_height).floor() as usize;
            let end = start
                .saturating_add((viewport.height / row_height).ceil() as usize + 1)
                .min(model.items.len());
            for index in start..end {
                let item = &model.items[index];
                let control = CollectionItemControl {
                    owner,
                    id: item.id,
                    kind: CollectionItemKind::List,
                    index,
                    depth: 0,
                    label: item.value.clone(),
                    enabled: item.enabled,
                    selected: model.selection.contains(item.id),
                    expanded: false,
                    has_children: false,
                    asset_id: None,
                    tint: [0.34, 0.38, 0.46, 1.0],
                };
                let rect = Rect {
                    x: viewport.x,
                    y: viewport.y + index as f32 * row_height - actual_offset,
                    width: viewport.width,
                    height: row_height,
                };
                if rect.intersection(viewport_clip).is_some() {
                    items.push((control, rect));
                }
            }
            if world
                .get::<ListModel<String>>(owner)
                .is_some_and(|current| current.scroll_offset != actual_offset)
                && let Some(mut current) = world.get_mut::<ListModel<String>>(owner)
            {
                current.scroll_offset = actual_offset;
            }
        } else if let Some(model) = tree.as_mut() {
            let row_height = if model.row_height == 22 {
                list_spacing.row_height
            } else {
                model.row_height.max(1) as f32
            }
            .max(1.0);
            let visible_ids = model.visible_ids();
            let page_rows = (viewport.height / row_height).ceil().max(1.0) as usize;
            if let Some(active_index) = model
                .selection
                .active
                .and_then(|active| visible_ids.iter().position(|id| *id == active))
            {
                if active_index < model.scroll_rows {
                    model.scroll_rows = active_index;
                } else if active_index >= model.scroll_rows.saturating_add(page_rows) {
                    model.scroll_rows = active_index + 1 - page_rows;
                }
            }
            let visible_rows = page_rows.saturating_add(1);
            let max_scroll = visible_ids.len().saturating_sub(page_rows);
            model.scroll_rows = model.scroll_rows.min(max_scroll);
            let start = model.scroll_rows;
            let end = start.saturating_add(visible_rows).min(visible_ids.len());
            for (slot, id) in visible_ids[start..end].iter().copied().enumerate() {
                let Some(node) = model.nodes.iter().find(|node| node.id == id) else {
                    continue;
                };
                let control = CollectionItemControl {
                    owner,
                    id,
                    kind: CollectionItemKind::Tree,
                    index: start + slot,
                    depth: model.depth(id),
                    label: node.value.clone(),
                    enabled: node.enabled,
                    selected: model.selection.contains(id),
                    expanded: node.expanded,
                    has_children: !node.children.is_empty(),
                    asset_id: None,
                    tint: [0.34, 0.38, 0.46, 1.0],
                };
                let rect = Rect {
                    x: viewport.x,
                    y: viewport.y + slot as f32 * row_height,
                    width: viewport.width,
                    height: row_height,
                };
                if rect.intersection(viewport_clip).is_some() {
                    items.push((control, rect));
                }
            }
            let scroll_rows = model.scroll_rows;
            if world
                .get::<TreeModel<String>>(owner)
                .is_some_and(|current| current.scroll_rows != scroll_rows)
                && let Some(mut current) = world.get_mut::<TreeModel<String>>(owner)
            {
                current.scroll_rows = scroll_rows;
            }
        } else if let Some(model) = grid.as_mut() {
            let cell_width = if model.cell_width == 112.0 {
                collection_spacing.preview_cell_width
            } else {
                model.cell_width
            }
            .max(collection_spacing.minimum_preview_cell_size);
            let cell_height = if model.cell_height == 104.0 {
                collection_spacing.preview_cell_height
            } else {
                model.cell_height
            }
            .max(collection_spacing.minimum_preview_cell_size);
            let columns =
                (((viewport.width + item_gap) / (cell_width + item_gap)).floor() as usize).max(1);
            model.columns = columns;
            let viewport_rows = (viewport.height / (cell_height + item_gap)).ceil() as usize;
            let total_rows = model.items.len().div_ceil(columns);
            let page_rows = viewport_rows.max(1);
            if let Some(active_row) = model
                .selection
                .active
                .and_then(|active| model.items.iter().position(|item| item.id == active))
                .map(|index| index / columns)
            {
                if active_row < model.scroll_rows {
                    model.scroll_rows = active_row;
                } else if active_row >= model.scroll_rows.saturating_add(page_rows) {
                    model.scroll_rows = active_row + 1 - page_rows;
                }
            }
            model.scroll_rows = model.scroll_rows.min(total_rows.saturating_sub(page_rows));
            let first = model.scroll_rows.saturating_mul(columns);
            let end = first
                .saturating_add(viewport_rows.saturating_add(1).saturating_mul(columns))
                .min(model.items.len());
            for index in first..end {
                let item = &model.items[index];
                let col = index % columns;
                let row = index / columns - model.scroll_rows;
                let control = CollectionItemControl {
                    owner,
                    id: item.id,
                    kind: CollectionItemKind::PreviewGrid,
                    index,
                    depth: 0,
                    label: item.label.clone(),
                    enabled: item.enabled,
                    selected: model.selection.contains(item.id),
                    expanded: false,
                    has_children: false,
                    asset_id: item.asset_id,
                    tint: item.tint,
                };
                let rect = Rect {
                    x: viewport.x + col as f32 * (cell_width + item_gap),
                    y: viewport.y + row as f32 * (cell_height + item_gap),
                    width: cell_width
                        .min((viewport.width - col as f32 * (cell_width + item_gap)).max(0.0)),
                    height: cell_height,
                };
                if rect.intersection(viewport_clip).is_some() {
                    items.push((control, rect));
                }
            }
            let columns = model.columns;
            let scroll_rows = model.scroll_rows;
            if world.get::<PreviewGrid>(owner).is_some_and(|current| {
                current.columns != columns || current.scroll_rows != scroll_rows
            }) && let Some(mut current) = world.get_mut::<PreviewGrid>(owner)
            {
                current.columns = columns;
                current.scroll_rows = scroll_rows;
            }
        }

        let stamp = collection_virtual_stamp(world, owner).unwrap_or(stamp);
        views.push(CollectionView {
            owner,
            stamp,
            items,
        });
    }

    let mut previous = {
        let mut cache = world.resource_mut::<WidgetCollectionItems>();
        std::mem::take(&mut cache.items)
    };
    let mut stamps = {
        let mut cache = world.resource_mut::<WidgetCollectionItems>();
        std::mem::take(&mut cache.stamps)
    };
    for view in views {
        let old_items = previous.remove(&view.owner).unwrap_or_default();
        let owner_layout = world.get::<UiLayoutResult>(view.owner).copied();
        let mut current_items = HashMap::new();
        if let Some(owner_layout) = owner_layout {
            let owner_clip = owner_layout
                .clip
                .unwrap_or(owner_layout.rect)
                .intersection(Rect {
                    x: owner_layout.rect.x,
                    y: owner_layout.rect.y + header_height,
                    width: owner_layout.rect.width.max(0.0),
                    height: (owner_layout.rect.height - header_height).max(0.0),
                })
                .unwrap_or(Rect::default());
            let owner_visible = world
                .get::<UiVisible>(view.owner)
                .is_none_or(|visible| visible.0);
            let owner_enabled = world
                .get::<UiEnabled>(view.owner)
                .is_none_or(|enabled| enabled.0);
            for (control, rect) in view.items {
                let existing = old_items
                    .get(&control.id)
                    .copied()
                    .filter(|entity| world.get_entity(*entity).is_ok());
                let id = control.id;
                let entity = existing.unwrap_or_else(|| world.spawn_empty().id());
                let z = owner_layout.z.saturating_add(20);
                let clip = rect.intersection(owner_clip).unwrap_or(owner_clip);
                let enabled = owner_enabled && control.enabled;
                let layout = UiLayoutResult {
                    window: owner_layout.window,
                    rect,
                    clip: Some(clip),
                    z,
                    content_size: [rect.width, rect.height],
                    dpi_scale: owner_layout.dpi_scale,
                };
                let hitbox = UiHitBox {
                    window: Some(owner_layout.window),
                    rect,
                    clip: Some(clip),
                    z_index: z,
                };
                if existing.is_some() {
                    set_enabled(world, entity, enabled);
                    set_visible(world, entity, owner_visible);
                    insert_component_if_changed(world, entity, control);
                    set_layout_result(world, entity, layout);
                    set_hitbox(world, entity, hitbox);
                } else {
                    world.entity_mut(entity).insert((
                        UiNode,
                        UiEnabled(enabled),
                        UiVisible(owner_visible),
                        control,
                        layout,
                        hitbox,
                    ));
                }
                if world
                    .get::<UiParent>(entity)
                    .is_none_or(|parent| parent.0 != view.owner)
                {
                    let _ = set_parent(world, entity, view.owner);
                }
                current_items.insert(id, entity);
            }
        }
        for (id, entity) in old_items {
            if !current_items.contains_key(&id) && world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
        previous.insert(view.owner, current_items);
        stamps.insert(view.owner, view.stamp);
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
    let mut cache = world.resource_mut::<WidgetCollectionItems>();
    cache.items = previous;
    cache.stamps = stamps;
}

pub(super) fn insert_component_if_changed<T: Component + PartialEq>(
    world: &mut World,
    entity: Entity,
    component: T,
) {
    if world.get::<T>(entity) != Some(&component) {
        world.entity_mut(entity).insert(component);
    }
}

pub(super) fn remove_component_if_present<T: Component>(world: &mut World, entity: Entity) {
    if world.get::<T>(entity).is_some() {
        world.entity_mut(entity).remove::<T>();
    }
}

pub(super) fn virtual_view_stamp<T: Component>(
    world: &World,
    owner: Entity,
) -> Option<VirtualViewStamp> {
    Some(VirtualViewStamp {
        model_changed: entity_component_changed_tick::<T>(world, owner)?,
        layout_changed: entity_component_changed_tick::<UiLayoutResult>(world, owner),
        theme_changed: world
            .get_resource_change_ticks::<Theme>()
            .map(|ticks| ticks.changed.get()),
        style_changed: entity_component_changed_tick::<UiComputedStyle>(world, owner),
        key_changed: entity_component_changed_tick::<UiKey>(world, owner),
    })
}

pub(super) fn collection_virtual_stamp(
    world: &World,
    owner: Entity,
) -> Option<CollectionVirtualStamp> {
    let (kind, stamp) = if let Some(stamp) = virtual_view_stamp::<ListModel<String>>(world, owner) {
        (0, stamp)
    } else if let Some(stamp) = virtual_view_stamp::<TreeModel<String>>(world, owner) {
        (1, stamp)
    } else {
        (2, virtual_view_stamp::<PreviewGrid>(world, owner)?)
    };
    Some(CollectionVirtualStamp {
        kind,
        stamp,
        visible_changed: entity_component_changed_tick::<UiVisible>(world, owner),
        enabled_changed: entity_component_changed_tick::<UiEnabled>(world, owner),
    })
}

pub(super) fn entity_component_changed_tick<T: Component>(
    world: &World,
    entity: Entity,
) -> Option<u32> {
    let component_id = world.components().component_id::<T>()?;
    world
        .get_entity(entity)
        .ok()?
        .get_change_ticks_by_id(component_id)
        .map(|ticks| ticks.changed.get())
}

pub(super) fn menu_popup_sync_stamp(
    world: &World,
    owner: Entity,
    viewport: Rect,
    viewport_changed: Option<u32>,
    theme_changed: Option<u32>,
) -> Option<MenuPopupSyncStamp> {
    Some(MenuPopupSyncStamp {
        menu_changed: entity_component_changed_tick::<MenuControl>(world, owner)?,
        layout_changed: entity_component_changed_tick::<UiLayoutResult>(world, owner)?,
        viewport_changed,
        viewport,
        theme_changed,
        owner_visible: is_visible(world, owner),
        owner_enabled: world
            .get::<UiEnabled>(owner)
            .is_none_or(|enabled| enabled.0),
        item_ticks: HashMap::new(),
    })
}

pub(super) fn menu_popup_stamp_core_matches(
    left: &MenuPopupSyncStamp,
    right: &MenuPopupSyncStamp,
) -> bool {
    left.menu_changed == right.menu_changed
        && left.layout_changed == right.layout_changed
        && left.viewport_changed == right.viewport_changed
        && left.viewport == right.viewport
        && left.theme_changed == right.theme_changed
        && left.owner_visible == right.owner_visible
        && left.owner_enabled == right.owner_enabled
}

pub(super) fn menu_item_component_ticks(
    world: &World,
    owner: Entity,
) -> HashMap<Entity, Option<u32>> {
    world
        .get::<MenuControl>(owner)
        .into_iter()
        .flat_map(|menu| &menu.items)
        .map(|entity| {
            (
                *entity,
                entity_component_changed_tick::<MenuItemControl>(world, *entity),
            )
        })
        .collect()
}

pub(super) fn menu_item_controls_changed(
    world: &World,
    owner: Entity,
    previous: &MenuPopupSyncStamp,
) -> bool {
    let Some(menu) = world.get::<MenuControl>(owner) else {
        return true;
    };
    menu.items.len() != previous.item_ticks.len()
        || menu.items.iter().any(|entity| {
            previous.item_ticks.get(entity).copied()
                != Some(entity_component_changed_tick::<MenuItemControl>(
                    world, *entity,
                ))
        })
}

pub(super) fn control_option_stamp<T: Component>(
    world: &World,
    owner: Entity,
    kind: u8,
    viewport: Rect,
    viewport_changed: Option<u32>,
    theme_changed: Option<u32>,
) -> Option<ControlOptionStamp> {
    Some(ControlOptionStamp {
        kind,
        model_changed: entity_component_changed_tick::<T>(world, owner)?,
        layout_changed: entity_component_changed_tick::<UiLayoutResult>(world, owner),
        popup_changed: (kind == 0)
            .then(|| entity_component_changed_tick::<DropdownPopupState>(world, owner))
            .flatten(),
        theme_changed,
        viewport_changed,
        viewport,
        owner_visible: is_visible(world, owner),
        owner_enabled: world
            .get::<UiEnabled>(owner)
            .is_none_or(|enabled| enabled.0),
    })
}

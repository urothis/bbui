use super::*;

/// Materialize stable option rows for Dropdown and RadioGroup owners.
pub fn sync_control_option_views(world: &mut World) {
    world.init_resource::<WidgetControlOptionItems>();
    let (row_height, popup_margin, control_height) = world.get_resource::<Theme>().map_or_else(
        || {
            let theme = Theme::default();
            (
                theme.spacing.menu.item_height.max(1.0),
                theme.spacing.menu.popup_padding.max(0.0),
                theme.control_height.max(1.0),
            )
        },
        |theme| {
            (
                theme.spacing.menu.item_height.max(1.0),
                theme.spacing.menu.popup_padding.max(0.0),
                theme.control_height.max(1.0),
            )
        },
    );
    let theme_changed = world
        .get_resource_change_ticks::<Theme>()
        .map(|ticks| ticks.changed.get());
    let mut root_query = world.query::<(Entity, &UiLayoutRoot)>();
    let mut viewports = HashMap::new();
    for (root_entity, root) in root_query.iter(world) {
        viewports.entry(root.window).or_insert((
            root.viewport,
            entity_component_changed_tick::<UiLayoutRoot>(world, root_entity),
        ));
    }

    let mut dropdown_query = world.query::<(Entity, &Dropdown, &UiLayoutResult)>();
    let mut current_dropdowns = HashSet::new();
    let mut dropdown_views = Vec::new();
    for (owner, dropdown, layout) in dropdown_query.iter(world) {
        current_dropdowns.insert(owner);
        let (viewport, viewport_changed) = viewports.get(&layout.window).copied().unwrap_or((
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            None,
        ));
        let Some(stamp) = control_option_stamp::<Dropdown>(
            world,
            owner,
            0,
            viewport,
            viewport_changed,
            theme_changed,
        ) else {
            continue;
        };
        let dirty = world
            .resource::<WidgetControlOptionItems>()
            .dropdown_stamps
            .get(&owner)
            != Some(&stamp);
        if dirty {
            dropdown_views.push((
                owner,
                dropdown.clone(),
                *layout,
                world
                    .get::<UiEnabled>(owner)
                    .is_none_or(|enabled| enabled.0),
                is_visible(world, owner),
                viewport,
                viewport_changed,
            ));
        }
    }
    let mut radio_query = world.query::<(Entity, &RadioGroup, &UiLayoutResult)>();
    let mut current_radios = HashSet::new();
    let mut radio_views = Vec::new();
    for (owner, group, layout) in radio_query.iter(world) {
        current_radios.insert(owner);
        let (viewport, viewport_changed) = viewports.get(&layout.window).copied().unwrap_or((
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            None,
        ));
        let Some(stamp) = control_option_stamp::<RadioGroup>(
            world,
            owner,
            1,
            viewport,
            viewport_changed,
            theme_changed,
        ) else {
            continue;
        };
        let dirty = world
            .resource::<WidgetControlOptionItems>()
            .radio_stamps
            .get(&owner)
            != Some(&stamp);
        if dirty {
            radio_views.push((
                owner,
                group.clone(),
                *layout,
                world
                    .get::<UiEnabled>(owner)
                    .is_none_or(|enabled| enabled.0),
                is_visible(world, owner),
                viewport,
                viewport_changed,
            ));
        }
    }

    let stale_dropdowns: Vec<_> = {
        let cache = world.resource::<WidgetControlOptionItems>();
        cache
            .dropdowns
            .keys()
            .chain(cache.dropdown_stamps.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|owner| !current_dropdowns.contains(owner))
            .collect()
    };
    let stale_radios: Vec<_> = {
        let cache = world.resource::<WidgetControlOptionItems>();
        cache
            .radios
            .keys()
            .chain(cache.radio_stamps.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|owner| !current_radios.contains(owner))
            .collect()
    };
    if dropdown_views.is_empty()
        && radio_views.is_empty()
        && stale_dropdowns.is_empty()
        && stale_radios.is_empty()
    {
        return;
    }
    let mut items = world
        .remove_resource::<WidgetControlOptionItems>()
        .unwrap_or_default();
    for owner in stale_dropdowns {
        if let Some(entities) = items.dropdowns.remove(&owner) {
            despawn_option_entities(world, owner, entities);
        }
        items.dropdown_stamps.remove(&owner);
        if world.get_entity(owner).is_ok() {
            world.entity_mut(owner).remove::<DropdownPopupState>();
        }
    }
    for owner in stale_radios {
        if let Some(entities) = items.radios.remove(&owner) {
            despawn_option_entities(world, owner, entities);
        }
        items.radio_stamps.remove(&owner);
    }

    for (owner, dropdown, layout, owner_enabled, owner_visible, viewport, viewport_changed) in
        dropdown_views
    {
        let entities = items.dropdowns.entry(owner).or_default();
        resize_dropdown_option_entities(world, owner, entities, dropdown.options.len());
        let max_rows = (viewport.height / row_height).floor().max(1.0) as usize;
        let mut visible_count = dropdown.options.len().min(8).min(max_rows);
        let mut popup_state =
            world
                .get::<DropdownPopupState>(owner)
                .copied()
                .unwrap_or(DropdownPopupState {
                    popup: PopupState::new(layout.rect),
                    viewport,
                    first_visible: 0,
                    visible_count,
                });
        popup_state.popup.anchor = layout.rect;
        popup_state.popup.open = dropdown.open;
        popup_state.viewport = viewport;
        if dropdown.open && visible_count > 0 {
            let requested_height = row_height * visible_count as f32;
            let below_space = (viewport.y + viewport.height
                - (layout.rect.y + layout.rect.height)
                - popup_margin)
                .max(0.0);
            let above_space = (layout.rect.y - viewport.y - popup_margin).max(0.0);
            let side = if below_space >= requested_height {
                PopupSide::Below
            } else if above_space >= requested_height {
                PopupSide::Above
            } else if below_space >= above_space {
                PopupSide::Below
            } else {
                PopupSide::Above
            };
            let available = match side {
                PopupSide::Below => below_space,
                PopupSide::Above => above_space,
                PopupSide::Left | PopupSide::Right => viewport.height,
            };
            visible_count = dropdown
                .options
                .len()
                .min(8)
                .min((available / row_height).floor() as usize)
                .max(1);
            let requested_height = row_height * visible_count as f32;
            popup_state.visible_count = visible_count;
            popup_state.popup.place(
                viewport,
                [layout.rect.width.max(160.0), requested_height],
                side,
                popup_margin,
            );
        } else {
            popup_state.visible_count = visible_count;
            popup_state.popup.bounds = Rect::default();
        }
        let max_first = dropdown.options.len().saturating_sub(visible_count);
        popup_state.first_visible = popup_state.first_visible.min(max_first);
        if dropdown.open
            && let Some(highlighted) = dropdown.highlighted
        {
            if highlighted < popup_state.first_visible {
                popup_state.first_visible = highlighted;
            } else if visible_count > 0 && highlighted >= popup_state.first_visible + visible_count
            {
                popup_state.first_visible =
                    highlighted.saturating_add(1).saturating_sub(visible_count);
            }
        }
        set_dropdown_popup_state(world, owner, popup_state);
        // Keep the trigger hitbox on the trigger. Popup rows have their own hitboxes
        // above it; stretching the owner across the popup makes the header untargetable
        // while open and lets the trigger steal clicks intended for an option.
        set_hitbox(
            world,
            owner,
            UiHitBox {
                window: Some(layout.window),
                rect: to_ui_rect(layout.rect),
                clip: layout.clip.map(to_ui_rect),
                z_index: layout.z,
            },
        );
        for (index, entity) in entities.iter().copied().enumerate() {
            let enabled = owner_enabled && dropdown.enabled;
            let control = DropdownOptionControl {
                owner,
                index,
                label: dropdown.options[index].clone(),
                selected: dropdown.selected == Some(index),
                highlighted: dropdown.highlighted == Some(index),
                enabled,
            };
            set_dropdown_option_control(world, entity, control);
            let visible = owner_visible
                && dropdown.open
                && index >= popup_state.first_visible
                && index < popup_state.first_visible.saturating_add(visible_count);
            set_visible(world, entity, visible);
            set_enabled(world, entity, enabled);
            if visible {
                let row = Rect {
                    x: popup_state.popup.bounds.x,
                    y: popup_state.popup.bounds.y
                        + (index - popup_state.first_visible) as f32 * row_height,
                    width: popup_state.popup.bounds.width,
                    height: row_height,
                };
                let z = layout.z.saturating_add(81);
                set_layout_result(
                    world,
                    entity,
                    UiLayoutResult {
                        window: layout.window,
                        rect: row,
                        clip: Some(popup_state.popup.bounds),
                        z,
                        content_size: [row.width, row.height],
                        dpi_scale: layout.dpi_scale,
                    },
                );
                set_hitbox(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(layout.window),
                        rect: to_ui_rect(row),
                        clip: Some(to_ui_rect(popup_state.popup.bounds)),
                        z_index: z,
                    },
                );
            }
        }
        if let Some(stamp) = control_option_stamp::<Dropdown>(
            world,
            owner,
            0,
            viewport,
            viewport_changed,
            theme_changed,
        ) {
            items.dropdown_stamps.insert(owner, stamp);
        }
    }

    for (owner, group, layout, owner_enabled, owner_visible, viewport, viewport_changed) in
        radio_views
    {
        let entities = items.radios.entry(owner).or_default();
        resize_radio_option_entities(world, owner, entities, group.options.len());
        let clip = layout
            .clip
            .and_then(|parent| layout.rect.intersection(parent))
            .unwrap_or(layout.rect);
        for (index, entity) in entities.iter().copied().enumerate() {
            let option = &group.options[index];
            let enabled = owner_enabled && group.enabled && option.enabled;
            let control = RadioOptionControl {
                owner,
                index,
                label: option.label.clone(),
                enabled,
                selected: group.selected == Some(index),
            };
            set_radio_option_control(world, entity, control);
            let row_y = layout.rect.y + control_height * (index as f32 + 1.0);
            let row = Rect {
                x: layout.rect.x,
                y: row_y,
                width: layout.rect.width,
                height: control_height,
            };
            let visible = owner_visible
                && row_y < layout.rect.y + layout.rect.height
                && row.intersection(clip).is_some();
            set_visible(world, entity, visible);
            set_enabled(world, entity, enabled);
            if visible {
                let z = layout.z.saturating_add(1 + index as i32 * 2);
                set_layout_result(
                    world,
                    entity,
                    UiLayoutResult {
                        window: layout.window,
                        rect: row,
                        clip: Some(clip),
                        z,
                        content_size: [row.width, row.height],
                        dpi_scale: layout.dpi_scale,
                    },
                );
                set_hitbox(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(layout.window),
                        rect: to_ui_rect(row),
                        clip: Some(to_ui_rect(clip)),
                        z_index: z,
                    },
                );
            }
        }
        if let Some(stamp) = control_option_stamp::<RadioGroup>(
            world,
            owner,
            1,
            viewport,
            viewport_changed,
            theme_changed,
        ) {
            items.radio_stamps.insert(owner, stamp);
        }
    }
    world.insert_resource(items);
}

pub(super) fn resize_dropdown_option_entities(
    world: &mut World,
    owner: Entity,
    items: &mut Vec<Entity>,
    count: usize,
) {
    while items.len() > count {
        let Some(entity) = items.pop() else { break };
        if world.get_entity(entity).is_ok() {
            detach_from_parent(world, entity);
            world.despawn(entity);
        }
    }
    while items.len() < count {
        let entity = world
            .spawn((
                UiNode,
                UiVisible(false),
                UiEnabled(false),
                UiRole::new("listbox-option"),
                UiWheelHandled,
            ))
            .id();
        let _ = set_parent(world, entity, owner);
        items.push(entity);
    }
}

pub(super) fn resize_radio_option_entities(
    world: &mut World,
    owner: Entity,
    items: &mut Vec<Entity>,
    count: usize,
) {
    while items.len() > count {
        let Some(entity) = items.pop() else { break };
        if world.get_entity(entity).is_ok() {
            detach_from_parent(world, entity);
            world.despawn(entity);
        }
    }
    while items.len() < count {
        let entity = world
            .spawn((
                UiNode,
                UiVisible(true),
                UiEnabled(false),
                UiRole::new("radio-button"),
            ))
            .id();
        let _ = set_parent(world, entity, owner);
        items.push(entity);
    }
}

pub(super) fn despawn_option_entities(world: &mut World, owner: Entity, entities: Vec<Entity>) {
    for entity in entities {
        if world.get_entity(entity).is_ok() {
            detach_from_parent(world, entity);
            world.despawn(entity);
        }
    }
    if world.get_entity(owner).is_ok() {
        world.entity_mut(owner).remove::<DropdownPopupState>();
    }
}

pub(super) fn set_dropdown_popup_state(
    world: &mut World,
    entity: Entity,
    value: DropdownPopupState,
) {
    if world.get::<DropdownPopupState>(entity) != Some(&value) {
        world.entity_mut(entity).insert(value);
    }
}

pub(super) fn set_dropdown_option_control(
    world: &mut World,
    entity: Entity,
    value: DropdownOptionControl,
) {
    if world.get::<DropdownOptionControl>(entity) != Some(&value) {
        world.entity_mut(entity).insert(value);
    }
}

pub(super) fn set_radio_option_control(
    world: &mut World,
    entity: Entity,
    value: RadioOptionControl,
) {
    if world.get::<RadioOptionControl>(entity) != Some(&value) {
        world.entity_mut(entity).insert(value);
    }
}

pub(super) fn set_visible(world: &mut World, entity: Entity, visible: bool) {
    if let Some(mut current) = world.get_mut::<UiVisible>(entity) {
        if current.0 != visible {
            current.0 = visible;
        }
    } else {
        world.entity_mut(entity).insert(UiVisible(visible));
    }
}

pub(super) fn set_enabled(world: &mut World, entity: Entity, enabled: bool) {
    if let Some(mut current) = world.get_mut::<UiEnabled>(entity) {
        if current.0 != enabled {
            current.0 = enabled;
        }
    } else {
        world.entity_mut(entity).insert(UiEnabled(enabled));
    }
}

pub(super) fn set_layout_result(world: &mut World, entity: Entity, layout: UiLayoutResult) {
    if world.get::<UiLayoutResult>(entity) != Some(&layout) {
        world.entity_mut(entity).insert(layout);
    }
}

pub(super) fn set_hitbox(world: &mut World, entity: Entity, hitbox: UiHitBox) {
    if world.get::<UiHitBox>(entity) != Some(&hitbox) {
        world.entity_mut(entity).insert(hitbox);
    }
}

pub(super) fn to_ui_rect(rect: Rect) -> UiRect {
    UiRect::new(rect.x, rect.y, rect.width, rect.height)
}

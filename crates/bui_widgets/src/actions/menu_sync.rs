use super::*;

/// Recompute popup item visibility, geometry, and hitboxes after widget state changes.
pub fn sync_menu_popups(world: &mut World) {
    world.init_resource::<WidgetMenuPopupStamps>();
    world.init_resource::<WidgetMenuPopupGate>();
    let changed = world.resource_scope(|world, mut gate: Mut<WidgetMenuPopupGate>| {
        if !gate.initialized {
            gate.components = vec![
                world.register_component::<UiLayoutRoot>(),
                world.register_component::<MenuControl>(),
                world.register_component::<UiLayoutResult>(),
                world.register_component::<MenuItemControl>(),
                world.register_component::<UiVisible>(),
                world.register_component::<UiEnabled>(),
            ];
            gate.initialized = true;
        }
        let now = world.read_change_tick();
        let changed = ui_components_changed_since(world, &gate.components, gate.last)
            || world
                .get_resource_change_ticks::<Theme>()
                .is_some_and(|ticks| ticks.is_changed(gate.last, now));
        gate.last = now;
        changed
    });
    if !changed {
        world.increment_change_tick();
        return;
    }
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
    let mut menu_query = world.query_filtered::<Entity, With<MenuControl>>();
    let menus: Vec<_> = menu_query.iter(world).collect();
    let live_menus: HashSet<_> = menus.iter().copied().collect();
    let mut stamps = world
        .remove_resource::<WidgetMenuPopupStamps>()
        .unwrap_or_default();
    stamps.stamps.retain(|owner, _| live_menus.contains(owner));
    for owner in menus {
        let Some(owner_layout) = world.get::<UiLayoutResult>(owner).copied() else {
            stamps.stamps.remove(&owner);
            continue;
        };
        let (viewport, viewport_changed) =
            viewports.get(&owner_layout.window).copied().unwrap_or((
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 720.0,
                },
                None,
            ));
        let Some(stamp) =
            menu_popup_sync_stamp(world, owner, viewport, viewport_changed, theme_changed)
        else {
            stamps.stamps.remove(&owner);
            continue;
        };
        let unchanged = stamps.stamps.get(&owner).is_some_and(|previous| {
            menu_popup_stamp_core_matches(previous, &stamp)
                && !menu_item_controls_changed(world, owner, previous)
        });
        if unchanged {
            continue;
        }
        let menu_spacing = world
            .get_resource::<Theme>()
            .map_or_else(|| Theme::default().spacing.menu, |theme| theme.spacing.menu);
        let menu_font_size = world
            .get_resource::<Theme>()
            .map_or_else(|| Theme::default().font_size, |theme| theme.font_size);
        let (
            open,
            display,
            query_text,
            path,
            width,
            row_height,
            item_entities,
            tree,
            search_order_ids,
            context_position,
            press_position,
            opened_by_pointer,
        ) = {
            let Some(menu) = world.get::<MenuControl>(owner) else {
                continue;
            };
            (
                menu.session.open,
                menu.display,
                menu.query.clone(),
                menu.session.path.clone(),
                menu.width,
                if menu.row_height == 24.0 {
                    menu_spacing.item_height
                } else {
                    menu.row_height
                },
                menu.items.clone(),
                menu.tree.clone(),
                if menu.query.is_empty() {
                    Vec::new()
                } else {
                    menu.tree
                        .search(&menu.search, &menu.query)
                        .into_iter()
                        .map(|entry| entry.id.clone())
                        .collect()
                },
                menu.context_position,
                menu.press_position,
                menu.opened_by_pointer,
            )
        };
        let search_ids: HashSet<String> = search_order_ids.iter().cloned().collect();
        let maximum_menu_depth = tree
            .flattened()
            .into_iter()
            .map(|(parent_path, _, _)| parent_path.len())
            .max()
            .unwrap_or(0);
        let width = width.min(
            ((viewport.width - menu_spacing.popup_padding * 2.0).max(0.0)
                / (maximum_menu_depth + 1) as f32)
                .max(1.0),
        );

        let count = if query_text.is_empty() {
            tree.entries_at(&[]).map_or(0, <[_]>::len)
        } else {
            search_ids.len()
        };
        let (bounds, popup_anchor, pie_center, popup_side) = if display == MenuDisplay::Pie {
            let diameter = menu_spacing
                .pie_popup_diameter
                .min(viewport.width)
                .min(viewport.height);
            let half = diameter * 0.5;
            let requested_center = if opened_by_pointer {
                press_position
            } else {
                context_position
            }
            .unwrap_or([
                owner_layout.rect.x + owner_layout.rect.width * 0.5,
                owner_layout.rect.y + owner_layout.rect.height * 0.5,
            ]);
            let center = [
                requested_center[0].clamp(viewport.x + half, viewport.x + viewport.width - half),
                requested_center[1].clamp(viewport.y + half, viewport.y + viewport.height - half),
            ];
            let bounds = Rect {
                x: center[0] - half,
                y: center[1] - half,
                width: diameter,
                height: diameter,
            };
            (
                bounds,
                owner_layout.rect,
                center,
                crate::menus::PopupSide::Below,
            )
        } else {
            let anchor = context_position.map_or(owner_layout.rect, |position| Rect {
                x: position[0],
                y: position[1],
                width: 1.0,
                height: 1.0,
            });
            let mut popup = world
                .get::<MenuControl>(owner)
                .map_or_else(|| crate::PopupState::new(anchor), |menu| menu.popup);
            popup.anchor = anchor;
            popup.open = open;
            let bounds = popup.place(
                viewport,
                [width, count.max(1) as f32 * row_height],
                crate::menus::PopupSide::Below,
                menu_spacing.popup_padding,
            );
            (bounds, anchor, [0.0, 0.0], popup.side)
        };
        if let Some(current) = world.get::<MenuControl>(owner) {
            let mut popup = current.popup;
            popup.anchor = popup_anchor;
            popup.bounds = bounds;
            popup.open = open;
            popup.side = popup_side;
            let mut pie_state = current.pie;
            if display == MenuDisplay::Pie {
                if pie_state.is_none() && open {
                    pie_state = Some(crate::PieMenu::new(
                        [0.0, 0.0],
                        menu_spacing.pie_inner_radius,
                        menu_spacing.pie_outer_radius,
                    ));
                }
                if let Some(pie) = &mut pie_state {
                    pie.center = pie_center;
                    pie.dead_zone = menu_spacing.pie_inner_radius;
                    pie.radius = (bounds.width.min(bounds.height) * 0.5
                        - menu_spacing.pie_inner_radius)
                        .max(0.0);
                }
            }
            if (current.popup != popup || current.pie != pie_state)
                && let Some(mut menu) = world.get_mut::<MenuControl>(owner)
            {
                menu.popup = popup;
                menu.pie = pie_state;
            }
        }
        let owner_hit_rect = if open { bounds } else { owner_layout.rect };
        let owner_hit_z = if open {
            owner_layout.z.saturating_add(90)
        } else {
            owner_layout.z
        };
        set_hitbox(
            world,
            owner,
            UiHitBox {
                window: Some(owner_layout.window),
                rect: owner_hit_rect,
                clip: if open {
                    Some(viewport)
                } else {
                    owner_layout.clip
                },
                z_index: owner_hit_z,
            },
        );

        let item_components: Vec<_> = item_entities
            .iter()
            .filter_map(|entity| {
                world
                    .get::<MenuItemControl>(*entity)
                    .cloned()
                    .map(|item| (*entity, item))
            })
            .collect();
        let pie_parent_path = if path.len() > 1 {
            path[..path.len() - 1].to_vec()
        } else {
            Vec::new()
        };
        let selectable_pie_indices: Vec<_> = tree
            .entries_at(&pie_parent_path)
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.enabled && !entry.separator)
            .map(|(index, _)| index)
            .collect();
        let pie_positions: HashMap<_, _> = selectable_pie_indices
            .iter()
            .enumerate()
            .map(|(position, index)| (*index, position))
            .collect();
        let mut search_order: Vec<_> = item_components
            .iter()
            .filter(|(_, item)| search_ids.contains(&item.id))
            .map(|(entity, item)| (*entity, item.clone()))
            .collect();
        let search_ranks: HashMap<_, _> = search_order_ids
            .iter()
            .enumerate()
            .map(|(rank, id)| (id.as_str(), rank))
            .collect();
        search_order.sort_by_key(|(_, item)| {
            search_ranks
                .get(item.id.as_str())
                .copied()
                .unwrap_or(usize::MAX)
        });
        let search_rows: HashMap<_, _> = search_order
            .iter()
            .enumerate()
            .map(|(row, (entity, _))| (*entity, row))
            .collect();
        let max_depth = maximum_menu_depth;
        let left_space = bounds.x - viewport.x;
        let right_space = viewport.x + viewport.width - (bounds.x + width);
        let left_fits = left_space >= width * max_depth as f32;
        let right_fits = right_space >= width * max_depth as f32;
        let cascade_right = right_fits || !left_fits;
        let mut search_row = search_order.len();
        for (entity, item) in item_components {
            let in_search = !query_text.is_empty();
            let pie_index = pie_positions.get(&item.index).copied();
            let visible = if !open {
                false
            } else if in_search {
                search_ids.contains(&item.id)
            } else if display == MenuDisplay::Pie {
                item.parent_path == pie_parent_path && pie_index.is_some()
            } else if item.parent_path.is_empty() {
                true
            } else {
                item.parent_path.len() <= path.len()
                    && path
                        .iter()
                        .take(item.parent_path.len())
                        .eq(item.parent_path.iter())
                    && tree
                        .entries_at(&item.parent_path)
                        .is_some_and(|entries| !entries.is_empty())
            };
            set_visible(world, entity, visible);
            set_enabled(world, entity, item.enabled && !item.separator);
            if !visible {
                continue;
            }
            let (rect, z) = if display == MenuDisplay::Pie && !in_search {
                let ring_index = pie_index.unwrap_or(0);
                let count = selectable_pie_indices.len().max(1);
                let angle = std::f32::consts::TAU * ring_index as f32 / count as f32
                    - std::f32::consts::FRAC_PI_2;
                let center = pie_center;
                let available_radius = world
                    .get::<MenuControl>(owner)
                    .and_then(|menu| menu.pie.as_ref().map(|pie| pie.radius))
                    .unwrap_or(menu_spacing.pie_outer_radius);
                let desired_radius = if pie_parent_path.is_empty() {
                    menu_spacing.pie_outer_item_radius
                } else {
                    menu_spacing.pie_nested_item_radius
                };
                let radius = desired_radius.min(available_radius);
                let item_width = (item.label.chars().count() as f32 * menu_font_size * 0.56
                    + menu_spacing.padding_x * 2.0)
                    .max(row_height * 1.8)
                    .min(menu_spacing.pie_item_max_width);
                (
                    Rect {
                        x: center[0] + angle.cos() * radius - item_width * 0.5,
                        y: center[1] + angle.sin() * radius - row_height * 0.5,
                        width: item_width,
                        height: row_height,
                    },
                    owner_layout.z.saturating_add(120),
                )
            } else if in_search {
                let row = search_rows.get(&entity).copied().unwrap_or_else(|| {
                    let row = search_row;
                    search_row += 1;
                    row
                });
                (
                    Rect {
                        x: bounds.x,
                        y: bounds.y + row as f32 * row_height,
                        width: bounds.width,
                        height: row_height,
                    },
                    owner_layout.z.saturating_add(100),
                )
            } else {
                let depth = item.parent_path.len();
                let panel_x = if depth == 0 {
                    bounds.x
                } else if cascade_right {
                    bounds.x + depth as f32 * width
                } else {
                    bounds.x - depth as f32 * width
                };
                let panel_y = (item.parent_path.iter().sum::<usize>() + item.index) as f32
                    * row_height
                    + bounds.y;
                (
                    Rect {
                        x: panel_x,
                        y: panel_y,
                        width,
                        height: row_height,
                    },
                    owner_layout
                        .z
                        .saturating_add(100 + item.parent_path.len() as i32 * 10),
                )
            };
            let clip = Some(viewport);
            set_layout_result(
                world,
                entity,
                UiLayoutResult {
                    window: owner_layout.window,
                    rect,
                    clip,
                    z,
                    content_size: [rect.width, rect.height],
                    dpi_scale: owner_layout.dpi_scale,
                },
            );
            set_hitbox(
                world,
                entity,
                UiHitBox {
                    window: Some(owner_layout.window),
                    rect,
                    clip,
                    z_index: z,
                },
            );
        }
        if let Some(mut menu) = world.get_mut::<MenuControl>(owner)
            && !open
            && !menu.query.is_empty()
        {
            menu.query.clear();
        }
        if let Some(mut stamp) =
            menu_popup_sync_stamp(world, owner, viewport, viewport_changed, theme_changed)
        {
            stamp.item_ticks = menu_item_component_ticks(world, owner);
            stamps.stamps.insert(owner, stamp);
        }
    }
    world.insert_resource(stamps);
    let now = world.read_change_tick();
    world.resource_mut::<WidgetMenuPopupGate>().last = now;
    world.increment_change_tick();
}

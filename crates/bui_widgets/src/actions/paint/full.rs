use super::*;

/// Reconcile hierarchy order while retaining ranks for the longest sequence
/// that kept its relative order. Reparent/reorder changes therefore republish
/// only moved fragments plus local neighbors if their rank gap is exhausted.
pub(in crate::actions) fn collect_widget_paint_full(
    world: &mut World,
    input_stamp: Option<u64>,
    batch: Option<UiDirtyBatch>,
) {
    let base_theme = world.resource::<Theme>().clone();
    let (
        mut fragments,
        mut paint_ranks,
        mut paint_order_keys,
        mut paint_children,
        mut paint_roots,
        mut paint_parents,
        parent_changes,
        children_changes,
        mut node_graph_chrome_stamps,
    ) = {
        let mut cache = world.resource_mut::<WidgetPaintCache>();
        (
            std::mem::take(&mut cache.fragments),
            std::mem::take(&mut cache.paint_ranks),
            std::mem::take(&mut cache.paint_order_keys),
            std::mem::take(&mut cache.paint_children),
            std::mem::take(&mut cache.paint_roots),
            std::mem::take(&mut cache.paint_parents),
            std::mem::take(&mut cache.pending_parent_changes),
            std::mem::take(&mut cache.pending_children_changes),
            std::mem::take(&mut cache.node_graph_chrome_stamps),
        )
    };
    let repaint_all = batch.as_ref().is_none_or(|batch| batch.all) || paint_ranks.is_empty();
    let mut entities: Vec<_> = if repaint_all {
        let mut query = world.query_filtered::<Entity, With<UiLayoutResult>>();
        query.iter(world).collect()
    } else {
        batch
            .as_ref()
            .map(|batch| batch.entities.iter().copied().collect())
            .unwrap_or_default()
    };
    let mut removed_fragments = HashSet::new();
    if repaint_all {
        removed_fragments = std::mem::take(&mut fragments);
        let order = widget_paint_order(world, &entities);
        paint_ranks = assign_even_paint_ranks(&order);
        paint_order_keys = paint_ranks
            .iter()
            .map(|(entity, rank)| (*rank, *entity))
            .collect();
        paint_children = widget_children_snapshot(world);
        paint_roots = widget_root_order(world);
        paint_parents = widget_parent_snapshot(world);
    } else {
        let missing_entities: Vec<_> = batch
            .as_ref()
            .into_iter()
            .flat_map(|batch| batch.entities.iter().copied())
            .filter(|entity| {
                world.get::<UiLayoutResult>(*entity).is_some() && !paint_ranks.contains_key(entity)
            })
            .collect();
        if !parent_changes.is_empty()
            || !children_changes.is_empty()
            || !missing_entities.is_empty()
        {
            let changed_order = reconcile_widget_paint_order_local(
                world,
                &parent_changes,
                &children_changes,
                &missing_entities,
                &mut paint_ranks,
                &mut paint_order_keys,
                &mut paint_children,
                &mut paint_roots,
                &mut paint_parents,
            );
            let mut included: HashSet<_> = entities.iter().copied().collect();
            for entity in changed_order {
                if fragments.contains(&entity) && included.insert(entity) {
                    entities.push(entity);
                }
            }
        }
    }
    let mut changed_outputs = HashMap::new();
    let mut repainted_entities = 0;

    for &entity in &entities {
        let graph_root = world.get::<NodeGraph>(entity).is_some();
        if !graph_root && fragments.remove(&entity) {
            removed_fragments.insert(entity);
        }
        if world.get::<UiLayoutResult>(entity).is_none() {
            if fragments.remove(&entity) {
                removed_fragments.insert(entity);
            }
            node_graph_chrome_stamps.remove(&entity);
            if let Some(rank) = paint_ranks.remove(&entity) {
                paint_order_keys.remove(&rank);
            }
            continue;
        }
        if !is_visible(world, entity) {
            if fragments.remove(&entity) {
                removed_fragments.insert(entity);
            }
            node_graph_chrome_stamps.remove(&entity);
            continue;
        }
        let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
            continue;
        };
        let is_box = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "box");
        if !is_box
            && world.get::<Checkbox>(entity).is_none()
            && world.get::<Button>(entity).is_none()
            && world.get::<IconButton>(entity).is_none()
            && world.get::<Label>(entity).is_none()
            && world.get::<RadioGroup>(entity).is_none()
            && world.get::<NumericField>(entity).is_none()
            && world.get::<Slider>(entity).is_none()
            && world.get::<TextField>(entity).is_none()
            && world.get::<TextArea>(entity).is_none()
            && world.get::<Dropdown>(entity).is_none()
            && world.get::<Panel>(entity).is_none()
            && world.get::<Progress>(entity).is_none()
            && world.get::<MenuControl>(entity).is_none()
            && world.get::<MenuItemControl>(entity).is_none()
            && world.get::<DropdownOptionControl>(entity).is_none()
            && world.get::<FileBrowser>(entity).is_none()
            && world.get::<FileBrowserItem>(entity).is_none()
            && world.get::<ListModel<String>>(entity).is_none()
            && world.get::<TreeModel<String>>(entity).is_none()
            && world.get::<PreviewGrid>(entity).is_none()
            && world.get::<CollectionItemControl>(entity).is_none()
            && world.get::<DataTable>(entity).is_none()
            && world.get::<DataTableHeaderControl>(entity).is_none()
            && world.get::<DataTableCellControl>(entity).is_none()
            && world.get::<ColorRamp>(entity).is_none()
            && world.get::<HsvColorEditor>(entity).is_none()
            && world.get::<CurveEditor>(entity).is_none()
            && world.get::<NodeGraph>(entity).is_none()
            && world.get::<NodeGraphNodeControl>(entity).is_none()
            && world.get::<NodeGraphLinkControl>(entity).is_none()
            && world.get::<Canvas>(entity).is_none()
            && world
                .get::<crate::canvas::CanvasObjectControl>(entity)
                .is_none()
            && world.get::<Tooltip>(entity).is_none()
        {
            continue;
        }
        let clip_rect = match layout.clip {
            Some(parent_clip) => {
                let Some(intersection) = parent_clip.intersection(layout.rect) else {
                    if fragments.remove(&entity) {
                        removed_fragments.insert(entity);
                    }
                    node_graph_chrome_stamps.remove(&entity);
                    continue;
                };
                intersection
            }
            None => layout.rect,
        };
        let clip = ClipRect::from(clip_rect);
        let rect = layout.rect;
        let z = layout.z;
        let entity_theme = styled_theme(world, entity, &base_theme);
        let theme = &entity_theme;
        let camera = world
            .get::<bevy_camera::Camera>(layout.window)
            .map(|_| layout.window);
        if let Some(graph) = world.get::<NodeGraph>(entity) {
            let state = world
                .get::<NodeGraphEditorState>(entity)
                .cloned()
                .unwrap_or_default();
            let label = world
                .get::<Label>(entity)
                .map_or("Node graph", |label| label.text.as_str());
            let canvas_surface = node_graph_canvas_surface(world, entity, &base_theme);
            let stamp = node_graph_chrome_stamp(
                graph,
                &state,
                label,
                layout,
                camera,
                theme,
                canvas_surface,
            );
            if !repaint_all
                && fragments.contains(&entity)
                && node_graph_chrome_stamps.get(&entity) == Some(&stamp)
            {
                continue;
            }
            node_graph_chrome_stamps.insert(entity, stamp);
        }
        if fragments.remove(&entity) {
            removed_fragments.insert(entity);
        }
        let order = paint_ranks.get(&entity).copied().unwrap_or_default();
        let mut fragment_guard = PaintFragmentGuard {
            fragment_entities: &mut fragments,
            changed_outputs: &mut changed_outputs,
            removed_fragments: &mut removed_fragments,
            entity,
            fragment: Some(WidgetRenderFragment {
                entity,
                order,
                target_camera: camera,
                items: Vec::new(),
            }),
        };
        let Some(fragment) = fragment_guard.fragment.as_mut() else {
            continue;
        };
        let camera_items = &mut fragment.items;
        repainted_entities += 1;
        let enabled = is_interactable(world, entity);

        let mut paint = PaintEntityContext {
            world,
            entity,
            layout,
            rect,
            clip,
            z,
            theme,
            base_theme: &base_theme,
            enabled,
            is_box,
            output: camera_items,
        };
        if paint_editor_widgets(&mut paint)
            || paint_buttons_and_menus(&mut paint)
            || paint_text_controls(&mut paint)
            || paint_misc_controls(&mut paint)
        {
            continue;
        }
    }

    let graph_link_groups: HashMap<_, _> = changed_outputs
        .keys()
        .map(|entity| {
            let group = world
                .get::<NodeGraphLinkControl>(*entity)
                .and_then(|control| {
                    world
                        .resource::<WidgetNodeGraphItems>()
                        .link_indices
                        .get(&control.owner)
                        .and_then(|links| links.get(&control.link_id))
                        .map(|index| WidgetFragmentBatchGroup {
                            owner: control.owner,
                            chunk: (*index / 128) as u32,
                        })
                });
            (*entity, group)
        })
        .collect();
    let live_graph_owners: HashSet<_> = world
        .resource::<WidgetNodeGraphItems>()
        .items
        .keys()
        .copied()
        .collect();
    node_graph_chrome_stamps.retain(|owner, _| live_graph_owners.contains(owner));

    let mut display_list = world.resource_mut::<DisplayList>();
    let mut published_fragments = 0;
    for (_, fragment) in changed_outputs {
        let batch_group = graph_link_groups.get(&fragment.entity).copied().flatten();
        published_fragments += usize::from(
            display_list.upsert_widget_fragment_with_batch_group(fragment, batch_group),
        );
    }
    for entity in removed_fragments {
        published_fragments += usize::from(display_list.remove_widget_fragment(entity));
    }
    let mut cache = world.resource_mut::<WidgetPaintCache>();
    cache.fragments = fragments;
    cache.paint_ranks = paint_ranks;
    cache.paint_order_keys = paint_order_keys;
    cache.paint_children = paint_children;
    cache.paint_roots = paint_roots;
    cache.paint_parents = paint_parents;
    cache.node_graph_chrome_stamps = node_graph_chrome_stamps;
    cache
        .node_graph_output_measurements
        .retain(|owner, _| live_graph_owners.contains(owner));
    cache.input_stamp = input_stamp;
    cache.rebuilt_frames = cache.rebuilt_frames.saturating_add(1);
    cache.repainted_entities = repainted_entities;
    cache.published_fragments = published_fragments;
}

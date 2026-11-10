use super::*;

pub(super) fn ensure_layout_resources(world: &mut World) {
    world.init_resource::<UiLayoutIssues>();
    world.init_resource::<UiDirty>();
    world.init_resource::<UiLayoutStats>();
    world.init_resource::<UiLayoutCache>();
    world.init_resource::<UiMeasurementCache>();
    world.init_resource::<UiViewports>();
}

pub(super) fn effective_roots(world: &mut World) -> Vec<(Entity, UiLayoutRoot)> {
    let registered: Vec<(Entity, UiLayoutRoot)> = {
        let mut query = world.query::<(Entity, &UiLayoutRoot)>();
        query
            .iter(world)
            .map(|(entity, root)| (entity, *root))
            .collect()
    };
    let viewports = world.resource::<UiViewports>().0.clone();
    let mut roots = registered;
    for (root, viewport) in viewports {
        if let Some(existing) = roots.iter_mut().find(|(entity, _)| *entity == root) {
            existing.1.viewport = viewport.rect;
            existing.1.dpi_scale = positive_or_one(viewport.dpi_scale);
        } else if world.get::<UiNode>(root).is_some() {
            roots.push((
                root,
                UiLayoutRoot::new(root, viewport.rect, viewport.dpi_scale),
            ));
        }
    }
    roots.sort_unstable_by_key(|(entity, _)| *entity);
    roots
}

pub(super) fn collect_layout_changes(world: &mut World) -> Vec<LayoutInputChange> {
    world.init_resource::<UiLayoutCollectorGate>();
    let inputs_changed = world.resource_scope(|world, mut gate: Mut<UiLayoutCollectorGate>| {
        if !gate.initialized {
            gate.components = vec![
                world.register_component::<UiNode>(),
                world.register_component::<UiLayoutRoot>(),
                world.register_component::<UiLayoutNode>(),
                world.register_component::<UiIntrinsicSize>(),
                world.register_component::<UiParent>(),
                world.register_component::<UiChildren>(),
                world.register_component::<UiZIndex>(),
                world.register_component::<UiScroll>(),
                world.register_component::<UiFlexItem>(),
                world.register_component::<UiVisible>(),
            ];
            gate.initialized = true;
        }
        let changed = ui_components_changed_since(world, &gate.components, gate.last);
        gate.last = world.read_change_tick();
        changed
    });
    let changed: Vec<Entity> = if inputs_changed {
        let mut query = world.query_filtered::<Entity, Or<(
            Changed<UiNode>,
            Changed<UiLayoutRoot>,
            Changed<UiLayoutNode>,
            Changed<UiIntrinsicSize>,
            Changed<UiParent>,
            Changed<UiChildren>,
            Changed<UiZIndex>,
            Changed<UiScroll>,
            Changed<UiFlexItem>,
            Changed<UiVisible>,
        )>>();
        query.iter(world).collect()
    } else {
        Vec::new()
    };
    world.increment_change_tick();
    let mut cache = world.remove_resource::<UiLayoutCache>().unwrap_or_default();
    let mut removed = read_removed::<UiNode>(world, &mut cache.removed_nodes);
    removed.extend(read_removed::<UiLayoutRoot>(
        world,
        &mut cache.removed_roots,
    ));
    removed.extend(read_removed::<UiLayoutNode>(
        world,
        &mut cache.removed_layout_nodes,
    ));
    removed.extend(read_removed::<UiIntrinsicSize>(
        world,
        &mut cache.removed_intrinsics,
    ));
    removed.extend(read_removed::<UiParent>(world, &mut cache.removed_parents));
    removed.extend(read_removed::<UiChildren>(
        world,
        &mut cache.removed_children,
    ));
    removed.extend(read_removed::<UiZIndex>(
        world,
        &mut cache.removed_z_indices,
    ));
    removed.extend(read_removed::<UiScroll>(world, &mut cache.removed_scrolls));
    removed.extend(read_removed::<UiFlexItem>(
        world,
        &mut cache.removed_flex_items,
    ));
    removed.extend(read_removed::<UiVisible>(
        world,
        &mut cache.removed_visibles,
    ));
    let mut dirty_entities = Vec::new();
    let mut changes = Vec::new();
    for entity in changed.into_iter().chain(removed) {
        if world.get_entity(entity).is_ok() {
            let current = layout_inputs(world, entity);
            let previous = cache.input_snapshots.get(&entity).cloned();
            if previous.as_ref() != Some(&current) {
                cache.input_snapshots.insert(entity, current.clone());
                dirty_entities.push(entity);
                changes.push(LayoutInputChange {
                    entity,
                    previous,
                    current: Some(current),
                });
            }
        } else if let Some(previous) = cache.input_snapshots.remove(&entity) {
            dirty_entities.push(entity);
            changes.push(LayoutInputChange {
                entity,
                previous: Some(previous),
                current: None,
            });
        }
    }
    world.insert_resource(cache);
    let mut dirty = world.resource_mut::<UiDirty>();
    for entity in dirty_entities {
        dirty.mark(UiDirtyDomain::Layout, entity);
    }
    changes
}

pub(super) fn layout_inputs(world: &World, entity: Entity) -> LayoutInputs {
    LayoutInputs {
        is_node: world.get::<UiNode>(entity).is_some(),
        root: world.get::<UiLayoutRoot>(entity).copied(),
        layout: world.get::<UiLayoutNode>(entity).copied(),
        intrinsic: world.get::<UiIntrinsicSize>(entity).copied(),
        parent: world.get::<UiParent>(entity).copied(),
        children: world.get::<UiChildren>(entity).cloned(),
        z_index: world.get::<UiZIndex>(entity).copied(),
        scroll: world.get::<UiScroll>(entity).copied(),
        flex_item: world.get::<UiFlexItem>(entity).copied(),
        visible: world
            .get::<UiVisible>(entity)
            .is_none_or(|visible| visible.0),
    }
}

pub(super) fn read_removed<C: Component>(
    world: &World,
    reader: &mut RemovedComponentReader<C>,
) -> Vec<Entity> {
    let Some(component_id) = world.component_id::<C>() else {
        return Vec::new();
    };
    let Some(messages) = world.removed_components().get(component_id) else {
        return Vec::new();
    };
    reader
        .read(messages)
        .map(|removed| removed.clone().into())
        .collect()
}

pub(super) fn collect_entity_roots(
    world: &World,
    entity: Entity,
    current_roots: &HashMap<Entity, UiLayoutRoot>,
    roots: &mut HashSet<Entity>,
) {
    let previous = world
        .resource::<UiLayoutCache>()
        .entity_roots
        .get(&entity)
        .cloned()
        .unwrap_or_default();
    roots.extend(previous);
    let mut at = Some(entity);
    let mut visited = HashSet::new();
    while let Some(current) = at {
        if !visited.insert(current) {
            break;
        }
        if current_roots.contains_key(&current) {
            roots.insert(current);
        }
        at = world.get::<UiParent>(current).map(|parent| parent.0);
    }
}

pub(super) fn add_path_to_root(
    world: &World,
    entity: Entity,
    root: Entity,
    required: &mut HashSet<Entity>,
) {
    let mut at = Some(entity);
    let mut visited = HashSet::new();
    while let Some(current) = at {
        if !visited.insert(current) {
            break;
        }
        required.insert(current);
        if current == root {
            break;
        }
        at = world.get::<UiParent>(current).map(|parent| parent.0);
    }
}

pub(super) fn invalidate_measurement_path(
    world: &World,
    entity: Entity,
    previous_parent: Option<Entity>,
    measurements: &mut MeasurementMap,
) {
    let mut pending = vec![entity];
    pending.extend(previous_parent);
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        measurements.remove(&current);
        pending.extend(world.get::<UiParent>(current).map(|parent| parent.0));
    }
}

pub(super) fn is_independent_intrinsic_change(world: &World, change: &LayoutInputChange) -> bool {
    let (Some(previous), Some(current)) = (&change.previous, &change.current) else {
        return false;
    };
    if previous.intrinsic == current.intrinsic {
        return false;
    }
    let mut previous_without_intrinsic = previous.clone();
    previous_without_intrinsic.intrinsic = current.intrinsic;
    if previous_without_intrinsic != *current {
        return false;
    }
    let Some(parent) = current.parent.map(|parent| parent.0) else {
        return false;
    };
    if world.get::<UiNode>(change.entity).is_none()
        || world.get::<UiLayoutResult>(change.entity).is_none()
    {
        return false;
    }
    let child_style = world
        .get::<UiLayoutNode>(change.entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    if child_style.mode != UiLayoutMode::Leaf
        && world
            .get::<UiChildren>(change.entity)
            .is_some_and(|children| !children.0.is_empty())
    {
        return false;
    }
    let old_size = previous.intrinsic.map_or([0.0, 0.0], |size| size.0);
    let new_size = current.intrinsic.map_or([0.0, 0.0], |size| size.0);
    let changed = [old_size[0] != new_size[0], old_size[1] != new_size[1]];
    if !changed.into_iter().any(|axis_changed| axis_changed) {
        return false;
    }
    let parent_style = world
        .get::<UiLayoutNode>(parent)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let size_is_fixed = |axis: usize| {
        matches!(
            axis_length(child_style.size, axis),
            UiLength::Px(_) | UiLength::Percent(_)
        )
    };
    match parent_style.mode {
        UiLayoutMode::Row => changed[0] && !changed[1] && size_is_fixed(0),
        UiLayoutMode::Column => changed[1] && !changed[0] && size_is_fixed(1),
        UiLayoutMode::Flex { direction, .. } => {
            let main_axis = flex_axes(direction).0;
            let cross_axis = flex_axes(direction).1;
            let basis = world
                .get::<UiFlexItem>(change.entity)
                .and_then(|item| item.basis)
                .unwrap_or_else(|| axis_length(child_style.size, main_axis));
            changed[main_axis]
                && !changed[cross_axis]
                && matches!(
                    basis,
                    UiLength::Px(_) | UiLength::Percent(_) | UiLength::Fill(_)
                )
        }
        UiLayoutMode::Overlay | UiLayoutMode::Grid { .. } | UiLayoutMode::ResponsiveGrid { .. } => {
            (0..2).all(|axis| !changed[axis] || size_is_fixed(axis))
        }
        UiLayoutMode::Leaf | UiLayoutMode::Inline => false,
    }
}

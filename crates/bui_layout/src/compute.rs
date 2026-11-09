use super::*;

/// Recompute invalidated layout paths and propagate geometry changes to affected
/// siblings and descendants. Measurements are memoized so unchanged branches
/// can supply cached intrinsic sizes without walking their subtrees.
pub fn compute_layout(world: &mut World) {
    ensure_layout_resources(world);
    let input_changes = collect_layout_changes(world);
    let batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Layout);
    let entities = batch.entities;
    let invalidate_all = batch.all;
    let initialized = world.resource::<UiLayoutCache>().initialized;
    let viewports_changed =
        world.resource::<UiViewports>().0 != world.resource::<UiLayoutCache>().viewports;
    let root_structure_changed = input_changes.iter().any(|change| {
        let previous = change.previous.as_ref();
        let current = change.current.as_ref();
        previous.and_then(|inputs| inputs.root) != current.and_then(|inputs| inputs.root)
            || previous.map(|inputs| inputs.is_node) != current.map(|inputs| inputs.is_node)
    });
    let rediscover_roots =
        !initialized || invalidate_all || viewports_changed || root_structure_changed;
    let previous_root_configs =
        std::mem::take(&mut world.resource_mut::<UiLayoutCache>().root_configs);
    let mut changed_root_configs = HashSet::new();
    let root_configs: HashMap<Entity, UiLayoutRoot> = if rediscover_roots {
        let current: HashMap<Entity, UiLayoutRoot> = effective_roots(world).into_iter().collect();
        for (root, info) in &current {
            if previous_root_configs.get(root) != Some(info) {
                changed_root_configs.insert(*root);
            }
        }
        for root in previous_root_configs.keys() {
            if !current.contains_key(root) {
                changed_root_configs.insert(*root);
            }
        }
        current
    } else {
        previous_root_configs
    };
    let mut dirty_roots = HashSet::new();
    let mut full_roots = HashSet::new();
    let mut local_only_entities = HashSet::new();
    let mut local_only_by_root: HashMap<Entity, Vec<Entity>> = HashMap::new();
    if !invalidate_all && input_changes.len() == 1 && entities.len() == 1 {
        let change = &input_changes[0];
        let memberships = world
            .resource::<UiLayoutCache>()
            .entity_roots
            .get(&change.entity);
        if let Some(root) = memberships
            .filter(|roots| roots.len() == 1)
            .and_then(|roots| roots.iter().next())
            .copied()
            && is_independent_intrinsic_change(world, change)
        {
            local_only_entities.insert(change.entity);
            local_only_by_root
                .entry(root)
                .or_default()
                .push(change.entity);
        }
    }
    if invalidate_all || !initialized {
        dirty_roots.extend(root_configs.keys().copied());
        full_roots.extend(dirty_roots.iter().copied());
    }
    dirty_roots.extend(changed_root_configs.iter().copied());
    full_roots.extend(changed_root_configs.iter().copied());

    for entity in &entities {
        collect_entity_roots(world, *entity, &root_configs, &mut dirty_roots);
    }
    for change in &input_changes {
        collect_entity_roots(world, change.entity, &root_configs, &mut dirty_roots);
        for parent in [
            change.previous.as_ref().and_then(|inputs| inputs.parent),
            change.current.as_ref().and_then(|inputs| inputs.parent),
        ]
        .into_iter()
        .flatten()
        {
            collect_entity_roots(world, parent.0, &root_configs, &mut dirty_roots);
        }
        let structural = match (&change.previous, &change.current) {
            (Some(previous), Some(current)) => {
                previous.is_node != current.is_node
                    || previous.parent != current.parent
                    || previous.children != current.children
                    || previous.visible != current.visible
            }
            _ => true,
        };
        if structural {
            collect_entity_roots(world, change.entity, &root_configs, &mut full_roots);
            for parent in [
                change.previous.as_ref().and_then(|inputs| inputs.parent),
                change.current.as_ref().and_then(|inputs| inputs.parent),
            ]
            .into_iter()
            .flatten()
            {
                collect_entity_roots(world, parent.0, &root_configs, &mut full_roots);
            }
        }
    }

    let mut measurements = std::mem::take(&mut world.resource_mut::<UiMeasurementCache>().0);
    if invalidate_all {
        measurements.clear();
    } else {
        let changed_entities: HashSet<Entity> =
            input_changes.iter().map(|change| change.entity).collect();
        for change in &input_changes {
            if local_only_entities.contains(&change.entity) {
                measurements.remove(&change.entity);
            } else {
                invalidate_measurement_path(
                    world,
                    change.entity,
                    change
                        .previous
                        .as_ref()
                        .and_then(|inputs| inputs.parent)
                        .map(|parent| parent.0),
                    &mut measurements,
                );
            }
        }
        for entity in &entities {
            if !changed_entities.contains(entity) {
                invalidate_measurement_path(world, *entity, None, &mut measurements);
            }
        }
        for root in &full_roots {
            let members = world
                .resource::<UiLayoutCache>()
                .root_members
                .get(root)
                .cloned()
                .unwrap_or_default();
            for entity in members {
                measurements.remove(&entity);
            }
        }
    }

    let mut required_by_root: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let incremental_roots: Vec<_> = dirty_roots
        .iter()
        .filter(|root| !full_roots.contains(root))
        .copied()
        .collect();
    for root in incremental_roots {
        let required = required_by_root.entry(root).or_default();
        for entity in &entities {
            if !local_only_entities.contains(entity) {
                add_path_to_root(world, *entity, root, required);
            }
        }
        for change in &input_changes {
            if !local_only_entities.contains(&change.entity) {
                add_path_to_root(world, change.entity, root, required);
                if let Some(parent) = change.current.as_ref().and_then(|inputs| inputs.parent) {
                    add_path_to_root(world, parent.0, root, required);
                }
            }
        }
        if required.is_empty() && !local_only_by_root.contains_key(&root) {
            full_roots.insert(root);
        }
    }

    let mut processed_roots = 0;
    let mut processed_nodes = 0;
    let mut stale_entities = HashSet::new();
    let mut clamped_scrolls = HashSet::new();
    let mut ordered_dirty: Vec<Entity> = dirty_roots.into_iter().collect();
    ordered_dirty.sort_unstable();

    for root in ordered_dirty {
        processed_roots += 1;
        let info = root_configs.get(&root).copied();
        let mut new_members = HashSet::new();
        let mut root_issues = Vec::new();
        if let Some(info) = info {
            if !info.viewport.is_finite() || info.viewport.is_empty() {
                root_issues.push((root, UiLayoutIssue::InvalidViewport));
            } else if world.get::<UiNode>(root).is_none() {
                root_issues.push((root, UiLayoutIssue::MissingNode));
            } else {
                let mut pass = LayoutPass {
                    window: info.window,
                    dpi_scale: positive_or_one(info.dpi_scale),
                    visiting: Vec::new(),
                    visited: HashSet::new(),
                    issues: Vec::new(),
                    clamped_scrolls: HashSet::new(),
                };
                let is_full = full_roots.contains(&root);
                let required = required_by_root.get(&root);
                if is_full || required.is_some_and(|required| !required.is_empty()) {
                    pass.layout_subtree(
                        world,
                        root,
                        AssignedRect::Raw(info.viewport),
                        None,
                        (!is_full).then_some(required).flatten(),
                        &mut measurements,
                    );
                }
                if !is_full && let Some(local_entities) = local_only_by_root.get(&root) {
                    for entity in local_entities {
                        if let Some(previous) = world.get::<UiLayoutResult>(*entity).copied() {
                            let local_required = HashSet::from([*entity]);
                            pass.layout_subtree(
                                world,
                                *entity,
                                AssignedRect::Resolved(previous.rect),
                                previous.clip,
                                Some(&local_required),
                                &mut measurements,
                            );
                        }
                    }
                }
                processed_nodes += pass.visited.len();
                if is_full {
                    new_members = pass.visited.clone();
                    root_issues = pass.issues;
                } else {
                    let previous_issues = world
                        .resource::<UiLayoutCache>()
                        .root_issues
                        .get(&root)
                        .cloned()
                        .unwrap_or_default();
                    root_issues = previous_issues
                        .into_iter()
                        .filter(|(entity, _)| !pass.visited.contains(entity))
                        .chain(pass.issues)
                        .collect();
                    // Membership is stable for non-structural invalidations.
                }
                clamped_scrolls.extend(pass.clamped_scrolls);
            }
        }

        let old_members = if full_roots.contains(&root) {
            world
                .resource::<UiLayoutCache>()
                .root_members
                .get(&root)
                .cloned()
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        {
            let mut cache = world.resource_mut::<UiLayoutCache>();
            if full_roots.contains(&root) {
                for entity in &old_members {
                    if let Some(roots) = cache.entity_roots.get_mut(entity) {
                        roots.remove(&root);
                        if roots.is_empty() {
                            cache.entity_roots.remove(entity);
                            stale_entities.insert(*entity);
                        }
                    }
                }
                for entity in &new_members {
                    cache.entity_roots.entry(*entity).or_default().insert(root);
                }
                if info.is_some() {
                    cache.root_members.insert(root, new_members);
                    cache.root_issues.insert(root, root_issues);
                } else {
                    cache.root_members.remove(&root);
                    cache.root_issues.remove(&root);
                }
            } else if info.is_some() {
                cache.root_issues.insert(root, root_issues);
            }
        }
    }

    for entity in stale_entities {
        if world.get_entity(entity).is_ok()
            && !world
                .resource::<UiLayoutCache>()
                .entity_roots
                .contains_key(&entity)
            && let Ok(mut entity_mut) = world.get_entity_mut(entity)
        {
            entity_mut.remove::<UiLayoutResult>();
            entity_mut.remove::<UiHitBox>();
        }
    }

    if !clamped_scrolls.is_empty() {
        let snapshots: Vec<_> = clamped_scrolls
            .into_iter()
            .filter(|entity| world.get_entity(*entity).is_ok())
            .map(|entity| (entity, layout_inputs(world, entity)))
            .collect();
        let mut cache = world.resource_mut::<UiLayoutCache>();
        for (entity, snapshot) in snapshots {
            cache.input_snapshots.insert(entity, snapshot);
        }
    }

    let issues = {
        let mut issues: Vec<_> = world
            .resource::<UiLayoutCache>()
            .root_issues
            .values()
            .flatten()
            .copied()
            .collect();
        issues.sort_unstable_by_key(|(entity, issue)| (*entity, *issue as u8));
        issues
    };
    if world.resource::<UiLayoutIssues>().0 != issues {
        world.resource_mut::<UiLayoutIssues>().0 = issues;
    }
    let viewports_snapshot = rediscover_roots.then(|| world.resource::<UiViewports>().0.clone());
    {
        let mut cache = world.resource_mut::<UiLayoutCache>();
        cache.root_configs = root_configs;
        if let Some(viewports) = viewports_snapshot {
            cache.viewports = viewports;
        }
        cache.initialized = true;
    }
    world.resource_mut::<UiMeasurementCache>().0 = measurements;
    {
        let mut stats = world.resource_mut::<UiLayoutStats>();
        stats.roots_processed = processed_roots;
        stats.nodes_processed = processed_nodes;
        stats.frames += 1;
        stats.total_roots_processed += processed_roots as u64;
        stats.total_nodes_processed += processed_nodes as u64;
        if processed_roots == 0 {
            stats.idle_frames += 1;
        }
    }
}

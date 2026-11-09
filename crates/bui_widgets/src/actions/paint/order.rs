use super::*;

pub(in crate::actions) fn widget_paint_order(
    world: &World,
    layout_entities: &[Entity],
) -> Vec<Entity> {
    let layout_entities: HashSet<_> = layout_entities.iter().copied().collect();
    let mut nodes: Vec<_> = world
        .iter_entities()
        .filter(|entity| {
            entity.contains::<UiNode>()
                || entity.contains::<UiParent>()
                || entity.contains::<UiChildren>()
                || entity.contains::<UiLayoutResult>()
        })
        .map(|entity| entity.id())
        .collect();
    nodes.sort_unstable_by_key(|entity| entity.index());
    nodes.dedup();
    let node_set: HashSet<_> = nodes.iter().copied().collect();

    let mut children_by_parent: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut assigned_parent = HashSet::new();
    for parent in &nodes {
        let Some(children) = world.get::<UiChildren>(*parent) else {
            continue;
        };
        for child in &children.0 {
            if node_set.contains(child) && assigned_parent.insert(*child) {
                children_by_parent.entry(*parent).or_default().push(*child);
            }
        }
    }
    for child in &nodes {
        let Some(parent) = world.get::<UiParent>(*child).map(|parent| parent.0) else {
            continue;
        };
        if node_set.contains(&parent) && assigned_parent.insert(*child) {
            children_by_parent.entry(parent).or_default().push(*child);
        }
    }

    let roots: Vec<_> = nodes
        .iter()
        .copied()
        .filter(|entity| !assigned_parent.contains(entity))
        .collect();
    let mut ordered = Vec::with_capacity(layout_entities.len());
    let mut visited = HashSet::with_capacity(nodes.len());
    for start in roots.iter().chain(nodes.iter()) {
        if visited.contains(start) {
            continue;
        }
        let mut pending = vec![*start];
        while let Some(entity) = pending.pop() {
            if !visited.insert(entity) {
                continue;
            }
            if layout_entities.contains(&entity) {
                ordered.push(entity);
            }
            if let Some(children) = children_by_parent.get(&entity) {
                pending.extend(children.iter().rev().copied());
            }
        }
    }
    ordered
}

pub(in crate::actions) fn widget_root_order(world: &World) -> BTreeMap<u32, Entity> {
    let mut nodes: Vec<_> = world
        .iter_entities()
        .filter(|entity| {
            entity.contains::<UiNode>()
                || entity.contains::<UiParent>()
                || entity.contains::<UiChildren>()
                || entity.contains::<UiLayoutResult>()
        })
        .map(|entity| entity.id())
        .collect();
    nodes.sort_unstable_by_key(|entity| entity.index());
    nodes.dedup();
    let node_set: HashSet<_> = nodes.iter().copied().collect();
    let mut assigned_parent = HashSet::new();
    for parent in &nodes {
        if let Some(children) = world.get::<UiChildren>(*parent) {
            assigned_parent.extend(
                children
                    .0
                    .iter()
                    .copied()
                    .filter(|child| node_set.contains(child)),
            );
        }
    }
    for child in &nodes {
        if let Some(parent) = world.get::<UiParent>(*child).map(|parent| parent.0)
            && node_set.contains(&parent)
        {
            assigned_parent.insert(*child);
        }
    }
    nodes
        .into_iter()
        .filter(|entity| !assigned_parent.contains(entity))
        .map(|entity| (entity.index_u32(), entity))
        .collect()
}

pub(in crate::actions) fn widget_subtree_paint_order(world: &World, root: Entity) -> Vec<Entity> {
    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            continue;
        }
        if world.get::<UiLayoutResult>(entity).is_some() {
            ordered.push(entity);
        }
        if let Some(children) = world.get::<UiChildren>(entity) {
            pending.extend(children.0.iter().rev().copied());
        }
    }
    ordered
}

pub(in crate::actions) fn widget_children_snapshot(world: &World) -> HashMap<Entity, Vec<Entity>> {
    let mut children: HashMap<_, _> = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiChildren>()
                .map(|children| (entity.id(), children.0.clone()))
        })
        .collect();
    let mut assigned = HashSet::new();
    for list in children.values() {
        assigned.extend(list.iter().copied());
    }
    let mut fallback: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut parented: Vec<_> = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiParent>()
                .map(|parent| (entity.id(), parent.0))
        })
        .collect();
    parented.sort_unstable_by_key(|(child, _)| child.index());
    for (child, parent) in parented {
        if !assigned.contains(&child) {
            fallback.entry(parent).or_default().push(child);
        }
    }
    for (parent, children_without_list) in fallback {
        let list = children.entry(parent).or_default();
        list.extend(children_without_list);
    }
    children
}

pub(in crate::actions) fn widget_parent_snapshot(world: &World) -> HashMap<Entity, Entity> {
    world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiParent>()
                .map(|parent| (entity.id(), parent.0))
        })
        .collect()
}

pub(in crate::actions) fn widget_subtree_rank_bounds(
    world: &World,
    root: Entity,
    ranks: &HashMap<Entity, u64>,
) -> Option<(u64, u64)> {
    widget_subtree_paint_order(world, root)
        .iter()
        .filter_map(|entity| ranks.get(entity).copied())
        .fold(None, |bounds, rank| {
            Some(bounds.map_or((rank, rank), |(start, end): (u64, u64)| {
                (start.min(rank), end.max(rank))
            }))
        })
}

pub(in crate::actions) fn assign_widget_rank_block(
    subtree: &[Entity],
    previous_ranks: &HashMap<Entity, u64>,
    lower: u64,
    upper: u64,
    ranks: &mut HashMap<Entity, u64>,
    order_keys: &mut BTreeMap<u64, Entity>,
) -> HashSet<Entity> {
    let mut left_selected = Vec::new();
    let mut right_selected = Vec::new();
    let mut left = order_keys
        .range(..=lower)
        .next_back()
        .map(|(rank, entity)| (*rank, *entity));
    let mut right = order_keys
        .range(upper..)
        .next()
        .map(|(rank, entity)| (*rank, *entity));
    let mut floor = left.map_or(0, |(rank, _)| rank);
    let mut ceiling = right.map_or(u64::MAX, |(rank, _)| rank);

    loop {
        let needed = left_selected.len() + subtree.len() + right_selected.len();
        let Some(gap) = (ceiling as u128).checked_sub(floor as u128) else {
            return HashSet::new();
        };
        if gap > needed as u128 {
            break;
        }
        if let Some((rank, entity)) = left {
            left_selected.push((rank, entity));
            left = order_keys
                .range(..rank)
                .next_back()
                .map(|(rank, entity)| (*rank, *entity));
            floor = left.map_or(0, |(rank, _)| rank);
        } else if let Some((rank, entity)) = right {
            right_selected.push((rank, entity));
            right = order_keys
                .range((std::ops::Bound::Excluded(rank), std::ops::Bound::Unbounded))
                .next()
                .map(|(rank, entity)| (*rank, *entity));
            ceiling = right.map_or(u64::MAX, |(rank, _)| rank);
        } else {
            return HashSet::new();
        }
    }

    let mut relabel: Vec<_> = left_selected.iter().rev().copied().collect();
    relabel.extend(subtree.iter().map(|entity| (0, *entity)));
    relabel.extend(right_selected.iter().copied());
    for (rank, entity) in left_selected.iter().chain(right_selected.iter()) {
        order_keys.remove(rank);
        ranks.remove(entity);
    }
    let step = ((ceiling as u128 - floor as u128) / (relabel.len() as u128 + 1)) as u64;
    debug_assert!(step > 0);

    let mut changed = HashSet::new();
    for (index, (_, entity)) in relabel.iter().enumerate() {
        let rank = floor.saturating_add(step.saturating_mul(index as u64 + 1));
        debug_assert!(!order_keys.contains_key(&rank));
        ranks.insert(*entity, rank);
        order_keys.insert(rank, *entity);
        if previous_ranks.get(entity) != Some(&rank) {
            changed.insert(*entity);
        }
    }
    changed
}

pub(in crate::actions) fn move_widget_paint_subtree(
    world: &World,
    root: Entity,
    parent: Option<Entity>,
    ranks: &mut HashMap<Entity, u64>,
    order_keys: &mut BTreeMap<u64, Entity>,
    children_snapshot: &HashMap<Entity, Vec<Entity>>,
    roots: &mut BTreeMap<u32, Entity>,
) -> HashSet<Entity> {
    let subtree = widget_subtree_paint_order(world, root);
    if subtree.is_empty() {
        return HashSet::new();
    }
    let previous_ranks: HashMap<_, _> = subtree
        .iter()
        .filter_map(|entity| ranks.get(entity).map(|rank| (*entity, *rank)))
        .collect();
    for entity in &subtree {
        if let Some(rank) = ranks.remove(entity) {
            order_keys.remove(&rank);
        }
    }

    if parent.is_some() {
        if roots.get(&root.index_u32()) == Some(&root) {
            roots.remove(&root.index_u32());
        }
    } else {
        roots.insert(root.index_u32(), root);
    }
    let (previous_sibling, next_sibling) = if let Some(parent) = parent {
        let fallback_siblings;
        let siblings = if let Some(children) = world.get::<UiChildren>(parent) {
            children.0.as_slice()
        } else {
            fallback_siblings = children_snapshot
                .get(&parent)
                .map(Vec::as_slice)
                .unwrap_or_default();
            fallback_siblings
        };
        let sibling_index = siblings.iter().position(|sibling| *sibling == root);
        let previous = sibling_index.and_then(|index| {
            siblings[..index]
                .iter()
                .rev()
                .find_map(|sibling| widget_subtree_rank_bounds(world, *sibling, ranks))
        });
        let next = sibling_index.and_then(|index| {
            siblings[index + 1..]
                .iter()
                .find_map(|sibling| widget_subtree_rank_bounds(world, *sibling, ranks))
        });
        (previous, next)
    } else {
        let previous = roots
            .range(..root.index_u32())
            .rev()
            .find_map(|(_, sibling)| widget_subtree_rank_bounds(world, *sibling, ranks));
        let next = roots
            .range((
                std::ops::Bound::Excluded(root.index_u32()),
                std::ops::Bound::Unbounded,
            ))
            .find_map(|(_, sibling)| widget_subtree_rank_bounds(world, *sibling, ranks));
        (previous, next)
    };

    let (lower, upper) = match (previous_sibling, next_sibling) {
        (Some((_, end)), Some((start, _))) => (end, start),
        (Some((_, end)), None) => {
            let upper = order_keys
                .range((std::ops::Bound::Excluded(end), std::ops::Bound::Unbounded))
                .next()
                .map(|(rank, _)| *rank)
                .unwrap_or(u64::MAX);
            (end, upper)
        }
        (None, Some((start, _))) => {
            let lower =
                if let Some(parent_rank) = parent.and_then(|parent| ranks.get(&parent).copied()) {
                    parent_rank
                } else if let Some(parent) = parent {
                    let mut top = parent;
                    while let Some(ancestor) = world.get::<UiParent>(top).map(|parent| parent.0) {
                        top = ancestor;
                    }
                    roots
                        .range(..top.index_u32())
                        .rev()
                        .find_map(|(_, root)| widget_subtree_rank_bounds(world, *root, ranks))
                        .map_or(0, |(_, end)| end)
                } else {
                    0
                };
            (lower, start)
        }
        (None, None) => {
            let parent_rank = parent.and_then(|parent| ranks.get(&parent).copied());
            if let Some(parent_rank) = parent_rank {
                let upper = order_keys
                    .range((
                        std::ops::Bound::Excluded(parent_rank),
                        std::ops::Bound::Unbounded,
                    ))
                    .next()
                    .map(|(rank, _)| *rank)
                    .unwrap_or(u64::MAX);
                (parent_rank, upper)
            } else if let Some(parent) = parent {
                let mut top = parent;
                while let Some(ancestor) = world.get::<UiParent>(top).map(|parent| parent.0) {
                    top = ancestor;
                }
                let previous_root = roots
                    .range(..top.index_u32())
                    .rev()
                    .find_map(|(_, root)| widget_subtree_rank_bounds(world, *root, ranks));
                let next_root = roots
                    .range((
                        std::ops::Bound::Excluded(top.index_u32()),
                        std::ops::Bound::Unbounded,
                    ))
                    .find_map(|(_, root)| widget_subtree_rank_bounds(world, *root, ranks));
                match (previous_root, next_root) {
                    (Some((_, end)), Some((start, _))) => (end, start),
                    (Some((_, end)), None) => {
                        let upper = order_keys
                            .range((std::ops::Bound::Excluded(end), std::ops::Bound::Unbounded))
                            .next()
                            .map(|(rank, _)| *rank)
                            .unwrap_or(u64::MAX);
                        (end, upper)
                    }
                    (None, Some((start, _))) => {
                        let lower = order_keys
                            .range(..start)
                            .next_back()
                            .map(|(rank, _)| *rank)
                            .unwrap_or(0);
                        (lower, start)
                    }
                    (None, None) => (0, u64::MAX),
                }
            } else {
                (0, u64::MAX)
            }
        }
    };
    let changed = if lower <= upper {
        assign_widget_rank_block(&subtree, &previous_ranks, lower, upper, ranks, order_keys)
    } else {
        HashSet::new()
    };
    if subtree.iter().all(|entity| ranks.contains_key(entity)) {
        return changed;
    }

    // A stale or exhausted local interval must not leave a subtree without paint ranks.
    // Rebuild the complete order only for this exceptional cache recovery path.
    let mut old_ranks = ranks.clone();
    old_ranks.extend(previous_ranks);
    let order: Vec<_> = roots
        .values()
        .flat_map(|root| widget_subtree_paint_order(world, *root))
        .collect();
    *ranks = assign_even_paint_ranks(&order);
    order_keys.clear();
    order_keys.extend(ranks.iter().map(|(entity, rank)| (*rank, *entity)));
    order
        .into_iter()
        .filter(|entity| old_ranks.get(entity) != ranks.get(entity))
        .collect()
}

pub(in crate::actions) fn changed_sibling_roots(
    old: &[Entity],
    current: &[Entity],
) -> HashSet<Entity> {
    let current_set: HashSet<_> = current.iter().copied().collect();
    let old_positions: HashMap<_, _> = old
        .iter()
        .enumerate()
        .map(|(position, entity)| (*entity, position))
        .collect();
    let mut tails: Vec<usize> = Vec::new();
    let mut tail_indices: Vec<usize> = Vec::new();
    let mut previous = vec![None; current.len()];
    for (index, entity) in current.iter().enumerate() {
        let Some(position) = old_positions.get(entity).copied() else {
            continue;
        };
        let insertion = tails.partition_point(|tail| *tail < position);
        if insertion > 0 {
            previous[index] = Some(tail_indices[insertion - 1]);
        }
        if insertion == tails.len() {
            tails.push(position);
            tail_indices.push(index);
        } else {
            tails[insertion] = position;
            tail_indices[insertion] = index;
        }
    }
    let mut retained = vec![false; current.len()];
    let mut cursor = tail_indices.last().copied();
    while let Some(index) = cursor {
        retained[index] = true;
        cursor = previous[index];
    }
    current
        .iter()
        .enumerate()
        .filter_map(|(index, entity)| (!retained[index]).then_some(*entity))
        .chain(
            old.iter()
                .filter(|entity| !current_set.contains(entity))
                .copied(),
        )
        .collect()
}

pub(in crate::actions) fn current_widget_parent(
    world: &World,
    entity: Entity,
    previous_parent: Option<Entity>,
) -> Option<Entity> {
    let parent = world.get::<UiParent>(entity).map(|parent| parent.0);
    if let Some(parent) = parent
        && world
            .get::<UiChildren>(parent)
            .is_some_and(|children| children.0.contains(&entity))
    {
        return Some(parent);
    }
    if let Some(previous_parent) = previous_parent
        && world
            .get::<UiChildren>(previous_parent)
            .is_some_and(|children| children.0.contains(&entity))
    {
        return Some(previous_parent);
    }
    parent.filter(|parent| world.get_entity(*parent).is_ok())
}

pub(in crate::actions) fn current_widget_children(
    world: &World,
    parent: Entity,
    previous: &[Entity],
    parent_snapshot: &HashMap<Entity, Entity>,
) -> Vec<Entity> {
    let mut current = world
        .get::<UiChildren>(parent)
        .map(|children| children.0.clone())
        .unwrap_or_default();
    let mut fallback: Vec<_> = previous
        .iter()
        .copied()
        .filter(|child| parent_snapshot.get(child) == Some(&parent) && !current.contains(child))
        .collect();
    fallback.sort_unstable_by_key(|child| child.index());
    current.extend(fallback);
    current
}

pub(in crate::actions) fn update_parent_child_snapshot(
    world: &World,
    parent: Entity,
    child: Entity,
    add: bool,
    children_snapshot: &mut HashMap<Entity, Vec<Entity>>,
) {
    if let Some(children) = world.get::<UiChildren>(parent) {
        children_snapshot.insert(parent, children.0.clone());
        return;
    }
    let siblings = children_snapshot.entry(parent).or_default();
    if add {
        if !siblings.contains(&child) {
            let insertion = siblings.partition_point(|sibling| sibling.index() < child.index());
            siblings.insert(insertion, child);
        }
    } else {
        siblings.retain(|sibling| *sibling != child);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the local order update mutates separate caches without scanning peers"
)]
pub(in crate::actions) fn reconcile_widget_paint_order_local(
    world: &World,
    parent_changes: &HashSet<Entity>,
    children_changes: &HashSet<Entity>,
    missing_entities: &[Entity],
    ranks: &mut HashMap<Entity, u64>,
    order_keys: &mut BTreeMap<u64, Entity>,
    children_snapshot: &mut HashMap<Entity, Vec<Entity>>,
    roots: &mut BTreeMap<u32, Entity>,
    parent_snapshot: &mut HashMap<Entity, Entity>,
) -> HashSet<Entity> {
    let mut changed = HashSet::new();
    let moved_roots: HashSet<_> = parent_changes
        .iter()
        .copied()
        .chain(missing_entities.iter().copied())
        .collect();
    let mut moved_roots_ordered: Vec<_> = moved_roots.iter().copied().collect();
    moved_roots_ordered.sort_by_key(|root| {
        let parent = current_widget_parent(world, *root, parent_snapshot.get(root).copied());
        let sibling_position = parent
            .and_then(|parent| children_snapshot.get(&parent))
            .and_then(|children| children.iter().position(|sibling| sibling == root))
            .unwrap_or(usize::MAX);
        (
            parent.map_or(root.index(), Entity::index),
            sibling_position,
            root.index(),
        )
    });
    // Free the full moved set before assigning any one rank. This makes a
    // multi-sibling permutation allocate into the final shared interval instead
    // of treating another soon-to-move sibling's stale rank as an anchor.
    for root in &moved_roots_ordered {
        for entity in widget_subtree_paint_order(world, *root) {
            if let Some(rank) = ranks.remove(&entity) {
                order_keys.remove(&rank);
            }
        }
    }
    for root in &moved_roots_ordered {
        let old_parent = parent_snapshot.get(root).copied();
        let new_parent = current_widget_parent(world, *root, old_parent);
        if old_parent != new_parent {
            if let Some(parent) = old_parent {
                update_parent_child_snapshot(world, parent, *root, false, children_snapshot);
            }
            if let Some(parent) = new_parent {
                update_parent_child_snapshot(world, parent, *root, true, children_snapshot);
                parent_snapshot.insert(*root, parent);
            } else {
                parent_snapshot.remove(root);
            }
        }
        changed.extend(move_widget_paint_subtree(
            world,
            *root,
            new_parent,
            ranks,
            order_keys,
            children_snapshot,
            roots,
        ));
    }

    let mut ordered_parents: Vec<_> = children_changes.iter().copied().collect();
    ordered_parents.sort_unstable_by_key(|parent| parent.index());
    for parent in ordered_parents {
        let old = children_snapshot.get(&parent).cloned().unwrap_or_default();
        let current = current_widget_children(world, parent, &old, parent_snapshot);
        let mut reordered_roots: Vec<_> =
            changed_sibling_roots(&old, &current).into_iter().collect();
        reordered_roots.sort_by_key(|root| {
            current
                .iter()
                .position(|candidate| candidate == root)
                .or_else(|| old.iter().position(|candidate| candidate == root))
                .unwrap_or(usize::MAX)
        });
        for root in reordered_roots
            .iter()
            .filter(|root| !moved_roots.contains(root))
        {
            for entity in widget_subtree_paint_order(world, *root) {
                if let Some(rank) = ranks.remove(&entity) {
                    order_keys.remove(&rank);
                }
            }
        }
        for root in reordered_roots {
            if !moved_roots.contains(&root) {
                changed.extend(move_widget_paint_subtree(
                    world,
                    root,
                    current_widget_parent(world, root, parent_snapshot.get(&root).copied()),
                    ranks,
                    order_keys,
                    children_snapshot,
                    roots,
                ));
            }
        }
        if current.is_empty() {
            children_snapshot.remove(&parent);
        } else {
            children_snapshot.insert(parent, current);
        }
    }
    changed
}

pub(in crate::actions) fn assign_even_paint_ranks(order: &[Entity]) -> HashMap<Entity, u64> {
    let step = u64::MAX / (order.len() as u64 + 1);
    order
        .iter()
        .enumerate()
        .map(|(index, entity)| {
            let rank = step.saturating_mul(index as u64 + 1);
            (*entity, rank)
        })
        .collect()
}

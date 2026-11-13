use super::*;

pub(in crate::actions) fn set_node_graph_gesture(
    world: &mut World,
    target: Entity,
    gesture: Option<NodeGraphGesture>,
) {
    if let Some(mut state) = world.get_mut::<NodeGraphEditorState>(target) {
        state.gesture = gesture;
    }
}

/// Move one graph node and record the affected node and incident links for scoped view updates.
/// Returns false for a missing node, a non-finite position, or an unchanged position.
pub fn move_node_graph_node(
    world: &mut World,
    owner: Entity,
    id: NodeId,
    position: [f32; 2],
) -> bool {
    if !position.into_iter().all(f32::is_finite) {
        return false;
    }
    world.init_resource::<WidgetNodeGraphItems>();
    world.init_resource::<WidgetNodeGraphDeltas>();
    let cache = world.get_resource::<WidgetNodeGraphItems>();
    let index = cache
        .and_then(|cache| cache.node_indices.get(&owner))
        .and_then(|indices| indices.get(&id))
        .copied();
    let synced_tick = cache.and_then(|cache| {
        cache
            .journaled_ticks
            .get(&owner)
            .copied()
            .or_else(|| cache.stamps.get(&owner).map(|stamp| stamp.model_changed))
    });
    let current_tick = entity_component_changed_tick::<NodeGraph>(world, owner);
    let signature_diff_required = current_tick
        .zip(synced_tick)
        .is_none_or(|(current, synced)| current != synced);
    let Some(graph) = world.get::<NodeGraph>(owner) else {
        return false;
    };
    let node = index
        .and_then(|index| graph.nodes.get(index))
        .filter(|node| node.id == id)
        .or_else(|| graph.nodes.iter().find(|node| node.id == id));
    if node.is_none_or(|node| node.position == position) {
        return false;
    }
    let changed = if let Some(mut graph) = world.get_mut::<NodeGraph>(owner) {
        if let Some(node) = index
            .and_then(|index| graph.nodes.get_mut(index))
            .filter(|node| node.id == id)
        {
            node.position = position;
            true
        } else {
            graph.move_node(id, position)
        }
    } else {
        false
    };
    if !changed {
        return false;
    }
    if let Some(tick) = entity_component_changed_tick::<NodeGraph>(world, owner) {
        world
            .resource_mut::<WidgetNodeGraphItems>()
            .journaled_ticks
            .insert(owner, tick);
    }
    let mut deltas = world.resource_mut::<WidgetNodeGraphDeltas>();
    deltas.0.entry(owner).or_default().insert(id);
    if signature_diff_required {
        deltas.1.insert(owner);
    }
    true
}

/// Pan a graph through the tracked viewport path. Direct component edits are
/// still reconciled by the full signature comparison during view sync.
pub fn pan_node_graph(world: &mut World, owner: Entity, pan: [f32; 2]) -> bool {
    if !pan.into_iter().all(f32::is_finite) {
        return false;
    }
    world.init_resource::<WidgetNodeGraphItems>();
    let cache = world.resource::<WidgetNodeGraphItems>();
    let expected = cache.viewport_ticks.get(&owner).copied().or_else(|| {
        cache
            .journaled_ticks
            .get(&owner)
            .copied()
            .or_else(|| cache.stamps.get(&owner).map(|stamp| stamp.model_changed))
    });
    let current = entity_component_changed_tick::<NodeGraph>(world, owner);
    let trusted = current.zip(expected).is_some_and(|(a, b)| a == b);
    if world
        .get::<NodeGraph>(owner)
        .is_none_or(|graph| graph.pan == pan)
    {
        return false;
    }
    world.increment_change_tick();
    if let Some(mut graph) = world.get_mut::<NodeGraph>(owner) {
        graph.pan = pan;
    }
    if trusted {
        if let Some(tick) = entity_component_changed_tick::<NodeGraph>(world, owner) {
            world
                .resource_mut::<WidgetNodeGraphItems>()
                .viewport_ticks
                .insert(owner, tick);
        }
    } else {
        world
            .resource_mut::<WidgetNodeGraphItems>()
            .viewport_ticks
            .remove(&owner);
    }
    world.increment_change_tick();
    true
}

pub(in crate::actions) fn node_graph_pan_tracking_is_current(world: &World, owner: Entity) -> bool {
    let Some(cache) = world.get_resource::<WidgetNodeGraphItems>() else {
        return false;
    };
    let expected = cache
        .viewport_ticks
        .get(&owner)
        .copied()
        .or_else(|| cache.stamps.get(&owner).map(|stamp| stamp.model_changed));
    entity_component_changed_tick::<NodeGraph>(world, owner)
        .zip(expected)
        .is_some_and(|(a, b)| a == b)
}

pub(in crate::actions) fn record_node_graph_pan(world: &mut World, owner: Entity, trusted: bool) {
    if trusted {
        if let Some(tick) = entity_component_changed_tick::<NodeGraph>(world, owner) {
            world
                .resource_mut::<WidgetNodeGraphItems>()
                .viewport_ticks
                .insert(owner, tick);
        }
    } else {
        world
            .resource_mut::<WidgetNodeGraphItems>()
            .viewport_ticks
            .remove(&owner);
    }
    world.increment_change_tick();
}

use super::*;

pub(in crate::actions) fn update_node_graph_gesture(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
) {
    let Some(gesture) = world
        .get::<NodeGraphEditorState>(target)
        .and_then(|state| state.gesture.clone())
    else {
        return;
    };
    match gesture {
        NodeGraphGesture::Pan {
            start_pointer,
            start_pan,
            ..
        } => {
            pan_node_graph(
                world,
                target,
                [
                    start_pan[0] + position[0] - start_pointer[0],
                    start_pan[1] + position[1] - start_pointer[1],
                ],
            );
        }
        NodeGraphGesture::MoveNodes {
            start_pointer,
            starts,
            ..
        } => {
            let graph_cache = world.resource::<WidgetNodeGraphItems>();
            let synced_tick = graph_cache
                .journaled_ticks
                .get(&target)
                .copied()
                .or_else(|| {
                    graph_cache
                        .stamps
                        .get(&target)
                        .map(|stamp| stamp.model_changed)
                });
            let current_tick = entity_component_changed_tick::<NodeGraph>(world, target);
            let signature_diff_required = current_tick
                .zip(synced_tick)
                .is_none_or(|(current, synced)| current != synced);
            let indexed_starts: Vec<_> = starts
                .into_iter()
                .map(|(id, start)| {
                    let index = world
                        .resource::<WidgetNodeGraphItems>()
                        .node_indices
                        .get(&target)
                        .and_then(|indices| indices.get(&id))
                        .copied();
                    (id, start, index)
                })
                .collect();
            let Some(graph) = world.get::<NodeGraph>(target) else {
                return;
            };
            let delta = [
                (position[0] - start_pointer[0]) / graph.zoom,
                (position[1] - start_pointer[1]) / graph.zoom,
            ];
            let planned: Vec<_> = indexed_starts
                .into_iter()
                .filter_map(|(id, start, index)| {
                    let next = [start[0] + delta[0], start[1] + delta[1]];
                    next.into_iter()
                        .all(f32::is_finite)
                        .then_some((id, index, next))
                })
                .collect();
            let changed_nodes: HashSet<_> = planned
                .iter()
                .filter_map(|(id, index, next)| {
                    let node = index
                        .and_then(|index| graph.nodes.get(index))
                        .filter(|node| node.id == *id)
                        .or_else(|| graph.nodes.iter().find(|node| node.id == *id));
                    node.is_some_and(|node| node.position != *next)
                        .then_some(*id)
                })
                .collect();
            if changed_nodes.is_empty() {
                return;
            }
            let mut moved_nodes = HashSet::new();
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                for (id, index, next) in planned {
                    if !changed_nodes.contains(&id) {
                        continue;
                    }
                    let changed = if let Some(node) = index
                        .and_then(|index| graph.nodes.get_mut(index))
                        .filter(|node| node.id == id)
                    {
                        node.position = next;
                        true
                    } else {
                        graph.move_node(id, next)
                    };
                    if changed {
                        moved_nodes.insert(id);
                    }
                }
            }
            if !moved_nodes.is_empty() {
                if let Some(tick) = entity_component_changed_tick::<NodeGraph>(world, target) {
                    world
                        .resource_mut::<WidgetNodeGraphItems>()
                        .journaled_ticks
                        .insert(target, tick);
                }
                let mut deltas = world.resource_mut::<WidgetNodeGraphDeltas>();
                deltas.0.entry(target).or_default().extend(moved_nodes);
                if signature_diff_required {
                    deltas.1.insert(target);
                }
            }
        }
        NodeGraphGesture::BoxSelect {
            start_pointer,
            additive,
            before,
            ..
        } => set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::BoxSelect {
                start_pointer,
                current_pointer: position,
                additive,
                before,
            }),
        ),
        NodeGraphGesture::LinkDrag {
            start_node,
            start_socket,
            start_direction,
            reconnect,
            before,
            ..
        } => set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::LinkDrag {
                start_node,
                start_socket,
                start_direction,
                reconnect,
                current_pointer: position,
                before,
            }),
        ),
        NodeGraphGesture::CutLinks { mut points, before } => {
            if points
                .last()
                .is_none_or(|last| distance(*last, position) >= 3.0)
            {
                points.push(position);
            }
            set_node_graph_gesture(
                world,
                target,
                Some(NodeGraphGesture::CutLinks { points, before }),
            );
        }
        NodeGraphGesture::Selection { .. } | NodeGraphGesture::SelectLink { .. } => {}
    }
}

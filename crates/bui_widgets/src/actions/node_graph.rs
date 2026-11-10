use super::*;

pub(super) fn select_canvas_object(
    world: &mut World,
    target: Entity,
    id: u64,
    additive: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world.get_mut::<Canvas>(target).is_some_and(|mut canvas| {
        if !canvas
            .objects
            .iter()
            .any(|object| object.id == id && object.visible && !object.locked)
        {
            return false;
        }
        let old = canvas.selected.clone();
        if !additive {
            canvas.selected.clear();
        }
        if additive && !canvas.selected.insert(id) {
            canvas.selected.remove(&id);
        } else if !additive {
            canvas.selected.insert(id);
        }
        old != canvas.selected
    });
    if changed
        && let Some(after) = world
            .get::<Canvas>(target)
            .and_then(|canvas| canvas.to_json().ok())
        && before != after
    {
        emitted.push(WidgetEvent::CanvasCommitted {
            target,
            before,
            after,
        });
    }
}

pub(super) fn edit_node_graph(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut NodeGraph) -> bool,
) {
    let Some(owner) = node_graph_owner(world, target) else {
        return;
    };
    let Some(before) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<NodeGraph>(owner)
        .is_some_and(|mut graph| edit(&mut graph));
    if !changed {
        return;
    }
    let Some(after) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    emit_node_graph_commit(owner, before, after, emitted);
}

pub(super) fn edit_node_graph_result(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut NodeGraph) -> Result<bool, NodeGraphError>,
) {
    let Some(owner) = node_graph_owner(world, target) else {
        return;
    };
    let Some(before) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    let result = world
        .get_mut::<NodeGraph>(owner)
        .map(|mut graph| edit(&mut graph));
    match result {
        Some(Ok(true)) => {
            if let Some(after) = world
                .get::<NodeGraph>(owner)
                .and_then(|graph| graph.to_json().ok())
            {
                emit_node_graph_commit(owner, before, after, emitted);
            }
        }
        Some(Err(error)) => emitted.push(WidgetEvent::NodeGraphLinkFailed {
            target: owner,
            error,
        }),
        Some(Ok(false)) | None => {}
    }
}

pub(super) fn emit_node_graph_commit(
    target: Entity,
    before: String,
    after: String,
    emitted: &mut Vec<WidgetEvent>,
) {
    if before != after {
        emitted.push(WidgetEvent::NodeGraphCommitted {
            target,
            before,
            after,
        });
    }
}

pub(super) fn has_annotated_node_graph_down(
    world: &World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
) -> bool {
    world
        .get_resource::<UiRoutedEvents>()
        .is_some_and(|events| {
            events.events.iter().any(|event| {
                matches!(event, UiRoutedEvent::PointerDownWithModifiers {
                    window: annotated_window,
                    target: annotated_target,
                    button: annotated_button,
                    position: annotated_position,
                    ..
                } if *annotated_window == window
                    && *annotated_target == target
                    && *annotated_button == button
                    && *annotated_position == position)
            })
        })
}

pub(super) fn process_node_graph_event(
    world: &mut World,
    event: &UiRoutedEvent,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    match event {
        UiRoutedEvent::PointerDown {
            window,
            target,
            button,
            position,
        } if let Some(owner) = node_graph_owner(world, *target) => {
            if !has_annotated_node_graph_down(world, *window, *target, *button, *position) {
                begin_node_graph_gesture(world, *window, owner, *button, *position, None, emitted);
            }
            true
        }
        UiRoutedEvent::PointerDownWithModifiers {
            window,
            target,
            button,
            position,
            modifiers,
        } if let Some(owner) = node_graph_owner(world, *target) => {
            begin_node_graph_gesture(
                world,
                *window,
                owner,
                *button,
                *position,
                Some(*modifiers),
                emitted,
            );
            true
        }
        UiRoutedEvent::PointerMoved {
            target,
            position,
            dragging: true,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            update_node_graph_gesture(world, owner, *position);
            true
        }
        UiRoutedEvent::PointerUp {
            target,
            position,
            cancelled,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            finish_node_graph_gesture(world, owner, *position, *cancelled, emitted);
            true
        }
        UiRoutedEvent::PointerCancel { target, .. }
            if let Some(owner) = node_graph_owner(world, *target) =>
        {
            finish_node_graph_gesture(world, owner, [0.0, 0.0], true, emitted);
            true
        }
        UiRoutedEvent::Click { target, .. } if node_graph_owner(world, *target).is_some() => true,
        UiRoutedEvent::ClickWithModifiers { target, .. }
            if node_graph_owner(world, *target).is_some() =>
        {
            true
        }
        UiRoutedEvent::Wheel {
            target,
            position,
            delta,
            unit,
            modifiers,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            node_graph_wheel(world, owner, *position, *delta, *unit, *modifiers, emitted);
            true
        }
        UiRoutedEvent::Key {
            target,
            key,
            pressed: true,
            modifiers,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            node_graph_key(world, owner, *key, *modifiers, emitted);
            true
        }
        _ => false,
    }
}

pub(super) fn begin_node_graph_gesture(
    world: &mut World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
    event_modifiers: Option<UiModifiers>,
    emitted: &mut Vec<WidgetEvent>,
) {
    let (Some(layout), Some(graph)) = (
        world.get::<UiLayoutResult>(target).copied(),
        world.get::<NodeGraph>(target).cloned(),
    ) else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    if !layout.rect.contains(position[0], position[1]) {
        return;
    }
    let Ok(before) = graph.to_json() else {
        return;
    };
    let modifiers = event_modifiers.unwrap_or_else(|| {
        world
            .get_resource::<WidgetInteractionState>()
            .and_then(|state| state.modifiers.get(&window).copied())
            .unwrap_or_default()
    });

    if matches!(button, UiPointerButton::Middle | UiPointerButton::Secondary) {
        set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::Pan {
                start_pointer: position,
                start_pan: graph.pan,
                before,
            }),
        );
        return;
    }
    if button != UiPointerButton::Primary {
        return;
    }
    if modifiers.alt {
        set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::CutLinks {
                points: vec![position],
                before,
            }),
        );
        return;
    }

    match node_graph_hit(&graph, layout.rect, position, spacing) {
        NodeGraphHit::Socket {
            node,
            socket,
            direction,
        } => {
            let reconnect = graph.links.iter().find_map(|link| {
                (direction == SocketDirection::Output
                    && link.from_node == node
                    && link.from_socket == socket)
                    .then_some((link.id, LinkEndpoint::From))
                    .or_else(|| {
                        (direction == SocketDirection::Input
                            && link.to_node == node
                            && link.to_socket == socket)
                            .then_some((link.id, LinkEndpoint::To))
                    })
            });
            set_node_graph_gesture(
                world,
                target,
                Some(NodeGraphGesture::LinkDrag {
                    start_node: node,
                    start_socket: socket,
                    start_direction: direction,
                    reconnect,
                    current_pointer: position,
                    before,
                }),
            );
        }
        NodeGraphHit::NodeHeader(id) | NodeGraphHit::NodeBody(id) => {
            let additive = modifiers.control || modifiers.shift;
            let was_selected = graph.selected.contains(&id);
            if additive {
                if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                    graph.select(id, true);
                }
                if was_selected {
                    set_node_graph_gesture(
                        world,
                        target,
                        Some(NodeGraphGesture::Selection { before }),
                    );
                    return;
                }
            } else if !was_selected {
                if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                    graph.select(id, false);
                }
            } else if world
                .get::<NodeGraph>(target)
                .is_some_and(|graph| graph.selected_link.is_some())
                && let Some(mut graph) = world.get_mut::<NodeGraph>(target)
            {
                graph.selected_link = None;
            }

            if matches!(
                node_graph_hit(&graph, layout.rect, position, spacing),
                NodeGraphHit::NodeHeader(_)
            ) {
                let Some(node) = graph.nodes.iter().find(|node| node.id == id) else {
                    return;
                };
                let node_rect = node_graph_node_rect(&graph, layout.rect, node, spacing);
                if position[0] <= node_rect.x + 22.0 * graph.zoom {
                    if let Some(mut current) = world.get_mut::<NodeGraph>(target)
                        && let Some(node) = current.nodes.iter_mut().find(|node| node.id == id)
                    {
                        node.collapsed = !node.collapsed;
                    }
                    if let Some(after) = world
                        .get::<NodeGraph>(target)
                        .and_then(|graph| graph.to_json().ok())
                    {
                        emit_node_graph_commit(target, before, after, emitted);
                    }
                    return;
                }
            }
            let starts = world
                .get::<NodeGraph>(target)
                .map(|graph| {
                    graph
                        .nodes
                        .iter()
                        .filter(|node| graph.selected.contains(&node.id))
                        .map(|node| (node.id, node.position))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            set_node_graph_gesture(
                world,
                target,
                Some(NodeGraphGesture::MoveNodes {
                    start_pointer: position,
                    starts,
                    before,
                }),
            );
        }
        NodeGraphHit::Link(id) => {
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                graph.select_link(id);
            }
            set_node_graph_gesture(world, target, Some(NodeGraphGesture::SelectLink { before }));
        }
        NodeGraphHit::Empty => set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::BoxSelect {
                start_pointer: position,
                current_pointer: position,
                additive: modifiers.control || modifiers.shift,
                before,
            }),
        ),
    }
}

pub(super) fn set_node_graph_gesture(
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

pub(super) fn node_graph_pan_tracking_is_current(world: &World, owner: Entity) -> bool {
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

pub(super) fn record_node_graph_pan(world: &mut World, owner: Entity, trusted: bool) {
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

pub(super) fn update_node_graph_gesture(world: &mut World, target: Entity, position: [f32; 2]) {
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

pub(super) fn finish_node_graph_gesture(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if !cancelled {
        update_node_graph_gesture(world, target, position);
    }
    let gesture = world
        .get_mut::<NodeGraphEditorState>(target)
        .and_then(|mut state| state.gesture.take());
    let Some(gesture) = gesture else {
        return;
    };
    let before = node_graph_gesture_before(&gesture).to_owned();
    if cancelled {
        if let Ok(graph) = NodeGraph::from_json(&before)
            && let Some(mut current) = world.get_mut::<NodeGraph>(target)
        {
            *current = graph;
        }
        return;
    }

    match gesture {
        NodeGraphGesture::BoxSelect {
            start_pointer,
            current_pointer,
            additive,
            ..
        } => {
            let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
                return;
            };
            let Some(graph) = world.get::<NodeGraph>(target).cloned() else {
                return;
            };
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let box_rect = Rect::new(
                start_pointer[0].min(current_pointer[0]),
                start_pointer[1].min(current_pointer[1]),
                (start_pointer[0] - current_pointer[0]).abs(),
                (start_pointer[1] - current_pointer[1]).abs(),
            );
            if let Some(mut current) = world.get_mut::<NodeGraph>(target) {
                if !additive {
                    current.selected.clear();
                }
                current.selected_link = None;
                for node in &graph.nodes {
                    if node_graph_node_rect(&graph, layout.rect, node, spacing)
                        .intersection(box_rect)
                        .is_some()
                    {
                        current.selected.insert(node.id);
                    }
                }
            }
        }
        NodeGraphGesture::LinkDrag {
            start_node,
            start_socket,
            start_direction,
            reconnect,
            current_pointer,
            ..
        } => {
            let (Some(layout), Some(graph)) = (
                world.get::<UiLayoutResult>(target).copied(),
                world.get::<NodeGraph>(target).cloned(),
            ) else {
                return;
            };
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let NodeGraphHit::Socket {
                node,
                socket,
                direction,
            } = node_graph_hit(&graph, layout.rect, current_pointer, spacing)
            else {
                return;
            };
            let result = if let Some((link_id, endpoint)) = reconnect {
                if direction != start_direction {
                    return;
                }
                world
                    .get_mut::<NodeGraph>(target)
                    .map(|mut graph| graph.reconnect_link(link_id, endpoint, node, socket))
            } else {
                if direction == start_direction {
                    return;
                }
                let (from_node, from_socket, to_node, to_socket) =
                    if start_direction == SocketDirection::Output {
                        (start_node, start_socket, node, socket)
                    } else {
                        (node, socket, start_node, start_socket)
                    };
                world.get_mut::<NodeGraph>(target).map(|mut graph| {
                    graph
                        .create_link(from_node, from_socket, to_node, to_socket)
                        .map(|_| ())
                })
            };
            if let Some(Err(error)) = result {
                emitted.push(WidgetEvent::NodeGraphLinkFailed { target, error });
            }
        }
        NodeGraphGesture::CutLinks { points, .. } => {
            let (Some(layout), Some(graph)) = (
                world.get::<UiLayoutResult>(target).copied(),
                world.get::<NodeGraph>(target).cloned(),
            ) else {
                return;
            };
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let removed: Vec<_> = graph
                .links
                .iter()
                .filter(|link| {
                    node_graph_link_crossed(&graph, layout.rect, **link, &points, spacing)
                })
                .map(|link| link.id)
                .collect();
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                for id in removed {
                    graph.remove_link(id);
                }
            }
        }
        NodeGraphGesture::Pan { .. }
        | NodeGraphGesture::MoveNodes { .. }
        | NodeGraphGesture::Selection { .. }
        | NodeGraphGesture::SelectLink { .. } => {}
    }
    if let Some(after) = world
        .get::<NodeGraph>(target)
        .and_then(|graph| graph.to_json().ok())
    {
        emit_node_graph_commit(target, before, after, emitted);
    }
}

pub(super) fn node_graph_gesture_before(gesture: &NodeGraphGesture) -> &str {
    match gesture {
        NodeGraphGesture::Pan { before, .. }
        | NodeGraphGesture::MoveNodes { before, .. }
        | NodeGraphGesture::BoxSelect { before, .. }
        | NodeGraphGesture::LinkDrag { before, .. }
        | NodeGraphGesture::CutLinks { before, .. }
        | NodeGraphGesture::Selection { before }
        | NodeGraphGesture::SelectLink { before } => before,
    }
}

pub(super) fn node_graph_wheel(
    world: &mut World,
    target: Entity,
    pointer: [f32; 2],
    delta: [f32; 2],
    unit: bui_core::UiWheelUnit,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    let tracked_pan = !modifiers.control && !modifiers.super_key;
    let trusted = tracked_pan && node_graph_pan_tracking_is_current(world, target);
    edit_node_graph(world, target, emitted, |graph| {
        if modifiers.control || modifiers.super_key {
            let anchor = node_graph_data_position(graph, rect, pointer, spacing);
            let scale = if unit == bui_core::UiWheelUnit::Lines {
                1.0
            } else {
                0.25
            };
            let before = graph.zoom;
            graph.zoom_by((delta[1] * scale * 0.12).exp());
            if graph.zoom == before {
                return false;
            }
            graph.pan = [
                pointer[0] - rect.x - anchor[0] * graph.zoom,
                pointer[1] - rect.y - spacing.header_height - anchor[1] * graph.zoom,
            ];
        } else {
            let scale = if unit == bui_core::UiWheelUnit::Lines {
                36.0
            } else {
                1.0
            };
            graph.pan_by([delta[0] * scale, delta[1] * scale]);
        }
        true
    });
    if tracked_pan {
        record_node_graph_pan(world, target, trusted);
    }
}

pub(super) fn node_graph_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    match key {
        UiKeyCode::Escape => {
            finish_node_graph_gesture(world, target, [0.0, 0.0], true, emitted);
        }
        UiKeyCode::A if modifiers.control || modifiers.super_key => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphSelectAll),
                emitted,
            );
        }
        UiKeyCode::Delete | UiKeyCode::Backspace | UiKeyCode::X => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphDeleteSelection),
                emitted,
            );
        }
        UiKeyCode::C => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphToggleCollapse),
                emitted,
            );
        }
        UiKeyCode::N => {
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let position = world
                .get::<UiLayoutResult>(target)
                .zip(world.get::<NodeGraph>(target))
                .map(|(layout, graph)| {
                    node_graph_data_position(
                        graph,
                        layout.rect,
                        [
                            layout.rect.x + layout.rect.width * 0.5,
                            layout.rect.y + spacing.header_height + 80.0,
                        ],
                        spacing,
                    )
                })
                .unwrap_or([0.0, 0.0]);
            apply_action(
                world,
                WidgetAction::new(
                    target,
                    WidgetActionKind::NodeGraphAddNode {
                        title: "Node".to_owned(),
                        position,
                        sockets: vec![
                            NodeSocket::input("Input", crate::SocketType::Any),
                            NodeSocket::output("Output", crate::SocketType::Any),
                        ],
                    },
                ),
                emitted,
            );
        }
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowDown => {
            if modifiers.control || modifiers.super_key {
                let factor = match key {
                    UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => 1.15,
                    UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => 1.0 / 1.15,
                    _ => return,
                };
                apply_action(
                    world,
                    WidgetAction::new(target, WidgetActionKind::NodeGraphZoomBy(factor)),
                    emitted,
                );
            } else {
                let step = if modifiers.shift { 40.0 } else { 10.0 };
                let delta = match key {
                    UiKeyCode::ArrowLeft => [-step, 0.0],
                    UiKeyCode::ArrowRight => [step, 0.0],
                    UiKeyCode::ArrowUp => [0.0, -step],
                    UiKeyCode::ArrowDown => [0.0, step],
                    _ => return,
                };
                apply_action(
                    world,
                    WidgetAction::new(target, WidgetActionKind::NodeGraphMoveSelected(delta)),
                    emitted,
                );
            }
        }
        UiKeyCode::Home => {
            edit_node_graph(world, target, emitted, |graph| {
                let changed = graph.pan != [0.0, 0.0] || graph.zoom != 1.0;
                graph.pan = [0.0, 0.0];
                graph.zoom = 1.0;
                changed
            });
        }
        _ => {}
    }
}

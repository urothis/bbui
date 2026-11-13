use super::*;

pub(in crate::actions) fn finish_node_graph_gesture(
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
            let registry = world
                .get_resource::<NodeGraphTypeRegistry>()
                .cloned()
                .unwrap_or_default();
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
                world.get_mut::<NodeGraph>(target).map(|mut graph| {
                    graph.reconnect_link_with_registry(&registry, link_id, endpoint, node, socket)
                })
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
                        .create_link_with_registry(
                            &registry,
                            from_node,
                            from_socket,
                            to_node,
                            to_socket,
                        )
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

pub(in crate::actions) fn node_graph_gesture_before(gesture: &NodeGraphGesture) -> &str {
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

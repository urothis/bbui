use super::*;

pub(in crate::actions) fn begin_node_graph_gesture(
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

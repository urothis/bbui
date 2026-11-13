use super::*;

pub(super) fn paint_graph_gesture(
    paint: NodeGraphPaint<'_>,
    target: NodeGraphPaintTarget<'_>,
    viewport_clip: ClipRect,
    output: &mut Vec<DrawItem>,
) {
    let graph = paint.graph;
    let rect = paint.rect;
    let theme = paint.theme;
    let registry = paint.registry;
    let z = paint.z;
    let state = paint.state;
    let spacing = &theme.spacing.node_graph;
    if matches!(target, NodeGraphPaintTarget::Chrome)
        && let Some(gesture) = state.gesture
    {
        match gesture {
            NodeGraphGesture::LinkDrag {
                start_node,
                start_socket,
                start_direction,
                reconnect,
                current_pointer,
                ..
            } => {
                if let Some(node) = graph.nodes.iter().find(|node| node.id == start_node)
                    && let Some(start) =
                        node_graph_socket_position(graph, rect, node, start_socket, spacing)
                {
                    let socket = &node.sockets[start_socket];
                    let is_bottom = socket.placement != SocketPlacement::Auto;
                    let control = if is_bottom {
                        ((current_pointer[0] - start[0]).powi(2)
                            + (current_pointer[1] - start[1]).powi(2))
                        .sqrt()
                            * 0.5
                    } else {
                        (current_pointer[0] - start[0]).abs() * 0.5
                    }
                    .max(spacing.connection_tangent_min * graph.zoom);
                    let sign = if start_direction == SocketDirection::Output {
                        1.0
                    } else {
                        -1.0
                    };
                    let offset = if is_bottom {
                        [0.0, control]
                    } else {
                        [control * sign, 0.0]
                    };
                    let points = [
                        start,
                        [start[0] + offset[0], start[1] + offset[1]],
                        [
                            current_pointer[0] - offset[0],
                            current_pointer[1] - offset[1],
                        ],
                        current_pointer,
                    ];
                    let fallback = NodeGraphTypeRegistry::default();
                    let registry = registry.unwrap_or(&fallback);
                    let preview_color =
                        socket_type_color(&socket.socket_type, theme, Some(registry));
                    let hover_type = match node_graph_hit(graph, rect, current_pointer, spacing) {
                        NodeGraphHit::Socket {
                            node: hover_node,
                            socket: hover_socket,
                            ..
                        } => graph
                            .nodes
                            .iter()
                            .find(|node| node.id == hover_node)
                            .and_then(|node| node.sockets.get(hover_socket))
                            .map(|socket| &socket.socket_type),
                        _ => None,
                    };
                    let invalid_target = match node_graph_hit(graph, rect, current_pointer, spacing)
                    {
                        NodeGraphHit::Socket {
                            node: hover_node,
                            socket: hover_socket,
                            ..
                        } if hover_node != start_node || hover_socket != start_socket => {
                            let endpoints = if let Some((link_id, endpoint)) = reconnect {
                                graph
                                    .links
                                    .iter()
                                    .find(|link| link.id == link_id)
                                    .map(|link| match endpoint {
                                        LinkEndpoint::From => {
                                            (hover_node, hover_socket, link.to_node, link.to_socket)
                                        }
                                        LinkEndpoint::To => (
                                            link.from_node,
                                            link.from_socket,
                                            hover_node,
                                            hover_socket,
                                        ),
                                    })
                            } else if start_direction == SocketDirection::Output {
                                Some((start_node, start_socket, hover_node, hover_socket))
                            } else {
                                Some((hover_node, hover_socket, start_node, start_socket))
                            };
                            endpoints.is_some_and(|(from_node, from_socket, to_node, to_socket)| {
                                graph
                                    .validate_link_with_registry(
                                        registry,
                                        from_node,
                                        from_socket,
                                        to_node,
                                        to_socket,
                                    )
                                    .is_err()
                            })
                        }
                        _ => false,
                    };
                    if invalid_target {
                        let (first, second) = split_node_graph_bezier(points);
                        for (segment, color) in [(first, preview_color), (second, theme.error)] {
                            output.push(item(
                                viewport_clip,
                                z.saturating_add(9),
                                Primitive::Bezier {
                                    points: segment,
                                    width: 2.0,
                                    color,
                                },
                            ));
                        }
                        let size = 7.0 * graph.zoom;
                        for (dx, dy) in [(-size, -size), (-size, size)] {
                            output.push(item(
                                viewport_clip,
                                z.saturating_add(11),
                                Primitive::Line {
                                    from: [current_pointer[0] + dx, current_pointer[1] + dy],
                                    to: [current_pointer[0] - dx, current_pointer[1] - dy],
                                    width: 2.5,
                                    color: theme.error,
                                },
                            ));
                        }
                    } else {
                        output.push(item(
                            viewport_clip,
                            z.saturating_add(9),
                            if hover_type
                                .is_some_and(|hover_type| hover_type != &socket.socket_type)
                            {
                                Primitive::BezierGradient {
                                    points,
                                    width: 2.0,
                                    start_color: preview_color,
                                    end_color: socket_type_color(
                                        hover_type.unwrap_or(&socket.socket_type),
                                        theme,
                                        Some(registry),
                                    ),
                                }
                            } else {
                                Primitive::Bezier {
                                    points,
                                    width: 2.0,
                                    color: preview_color,
                                }
                            },
                        ));
                    }
                }
            }
            NodeGraphGesture::BoxSelect {
                start_pointer,
                current_pointer,
                ..
            } => {
                let selection = Rect::new(
                    start_pointer[0].min(current_pointer[0]),
                    start_pointer[1].min(current_pointer[1]),
                    (start_pointer[0] - current_pointer[0]).abs(),
                    (start_pointer[1] - current_pointer[1]).abs(),
                );
                let outline = theme.accent;
                output.push(item(
                    viewport_clip,
                    z.saturating_add(9),
                    Primitive::RoundedRect {
                        rect: ClipRect::from(selection),
                        radius: 0.0,
                        color: theme.color(ThemeColorToken::NodeGraphSelectionOverlay),
                    },
                ));
                push_rect_outline(
                    selection,
                    outline,
                    viewport_clip,
                    z.saturating_add(10),
                    output,
                );
            }
            NodeGraphGesture::CutLinks { points, .. } => {
                for pair in points.windows(2) {
                    output.push(item(
                        viewport_clip,
                        z.saturating_add(10),
                        Primitive::Line {
                            from: pair[0],
                            to: pair[1],
                            width: 2.0,
                            color: theme.error,
                        },
                    ));
                }
            }
            NodeGraphGesture::Pan { .. }
            | NodeGraphGesture::MoveNodes { .. }
            | NodeGraphGesture::Selection { .. }
            | NodeGraphGesture::SelectLink { .. } => {}
        }
    }
}

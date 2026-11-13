use super::*;

pub(super) fn paint_graph_node(
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
    let font = paint.font;
    let output_label_widths = paint.output_label_widths;
    let spacing = &theme.spacing.node_graph;
    if let NodeGraphPaintTarget::Node(node) = target {
        let node_rect = node_graph_node_rect(graph, rect, node, spacing);
        let Some(node_clip) = intersect_clip_rect(viewport_clip, node_rect) else {
            return;
        };
        let selected = graph.selected.contains(&node.id);
        output.push(item(
            node_clip,
            z.saturating_add(4),
            Primitive::RoundedRect {
                rect: ClipRect::from(node_rect),
                radius: spacing.node_radius,
                color: if selected {
                    theme.surface.mix(theme.accent, 0.08)
                } else {
                    theme.surface
                },
            },
        ));
        let title_height = (spacing.header_height * graph.zoom).min(node_rect.height);
        let node_header = Rect {
            height: title_height,
            ..node_rect
        };
        let title_clip = intersect_clip_rect(node_clip, node_header).unwrap_or(node_clip);
        output.push(item(
            title_clip,
            z.saturating_add(5),
            Primitive::RoundedRect {
                rect: ClipRect::from(node_header),
                radius: spacing.node_radius,
                color: node_graph_category_color(node.category, theme),
            },
        ));
        let zoom = graph.zoom.max(0.01);
        let title_x = node_rect.x + (spacing.title_padding_x + 8.0) * zoom;
        let title_width =
            (node_rect.x + node_rect.width - spacing.body_padding * zoom - title_x).max(1.0);
        output.push(item(
            title_clip,
            z.saturating_add(6),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [title_x, node_rect.y + spacing.title_padding_y * zoom],
                    text: node.title.clone(),
                },
                theme.text,
                title_width,
                bevy_text::Justify::Left,
                &font,
                theme,
                zoom,
            ),
        ));
        output.push(item(
            title_clip,
            z.saturating_add(6),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [
                        node_rect.x + spacing.collapse_toggle_inset_x * zoom,
                        node_rect.y + spacing.title_padding_y * zoom,
                    ],
                    text: if node.collapsed { "▸" } else { "▾" }.to_owned(),
                },
                theme.text,
                (spacing.title_padding_x - spacing.collapse_toggle_inset_x).max(1.0) * zoom,
                bevy_text::Justify::Left,
                &font,
                theme,
                zoom,
            ),
        ));

        for (socket_index, socket) in node.sockets.iter().enumerate() {
            let Some(center) = node_graph_socket_position(graph, rect, node, socket_index, spacing)
            else {
                continue;
            };
            let color = socket_type_color(&socket.socket_type, theme, registry);
            let radius = (spacing.socket_radius * graph.zoom).max(0.0);
            output.push(item(
                viewport_clip,
                z.saturating_add(8),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        x: center[0] - radius,
                        y: center[1] - radius,
                        width: radius * 2.0,
                        height: radius * 2.0,
                    },
                    radius,
                    color,
                },
            ));
            if socket.placement != SocketPlacement::Auto {
                if !node.collapsed {
                    let inset = spacing.socket_label_inset_x * zoom;
                    let half_width = node_rect.width * 0.5;
                    let side_x = node_rect.x
                        + if socket.placement == SocketPlacement::BottomLeft {
                            0.0
                        } else {
                            half_width
                        };
                    let count = node
                        .sockets
                        .iter()
                        .filter(|candidate| candidate.placement == socket.placement)
                        .count();
                    let index = node.sockets[..socket_index]
                        .iter()
                        .filter(|candidate| candidate.placement == socket.placement)
                        .count();
                    let lane_width = half_width / count as f32;
                    output.push(item(
                        node_clip,
                        z.saturating_add(8),
                        node_graph_text_primitive(
                            TextPlacement {
                                origin: [
                                    side_x + lane_width * index as f32 + inset,
                                    center[1] - spacing.socket_spacing * zoom,
                                ],
                                text: socket.name.clone(),
                            },
                            theme.text_muted,
                            (lane_width - inset * 2.0).max(1.0),
                            bevy_text::Justify::Center,
                            &font,
                            theme,
                            zoom,
                        ),
                    ));
                }
                continue;
            }
            if socket.direction == SocketDirection::Input
                && socket.value.is_some()
                && registry
                    .and_then(|registry| registry.editor(&socket.socket_type))
                    .is_some()
                && !graph
                    .links
                    .iter()
                    .any(|link| link.to_node == node.id && link.to_socket_id == socket.id)
            {
                continue;
            }
            let label_row = node.sockets[..socket_index]
                .iter()
                .filter(|previous| {
                    previous.placement == SocketPlacement::Auto
                        && previous.direction == socket.direction
                })
                .count();
            let opposite_direction = match socket.direction {
                SocketDirection::Input => SocketDirection::Output,
                SocketDirection::Output => SocketDirection::Input,
            };
            let has_counterpart = node
                .sockets
                .iter()
                .filter(|candidate| {
                    candidate.placement == SocketPlacement::Auto
                        && candidate.direction == opposite_direction
                })
                .nth(label_row)
                .is_some();
            let content_left = node_rect.x + spacing.socket_label_inset_x * zoom;
            let content_right = node_rect.x + node_rect.width - spacing.socket_label_inset_x * zoom;
            let content_width = (content_right - content_left).max(1.0);
            let split = content_left + content_width * 0.5;
            let (text_x, max_width, text_clip) = match socket.direction {
                SocketDirection::Input if has_counterpart => (
                    content_left,
                    (split - content_left).max(1.0),
                    intersect_clip_rect(
                        node_clip,
                        Rect::new(
                            content_left,
                            node_rect.y,
                            split - content_left,
                            node_rect.height,
                        ),
                    )
                    .unwrap_or(node_clip),
                ),
                SocketDirection::Input => (content_left, content_width, node_clip),
                SocketDirection::Output => {
                    let lane_left = if has_counterpart { split } else { content_left };
                    let lane_width = (content_right - lane_left).max(1.0);
                    let label_width = (output_label_widths
                        .get(&(node.id, socket_index))
                        .copied()
                        .unwrap_or(lane_width)
                        + 1.0)
                        .clamp(1.0, lane_width);
                    let clip = if has_counterpart {
                        intersect_clip_rect(
                            node_clip,
                            Rect::new(lane_left, node_rect.y, lane_width, node_rect.height),
                        )
                        .unwrap_or(node_clip)
                    } else {
                        node_clip
                    };
                    (content_right - label_width, label_width, clip)
                }
            };
            output.push(item(
                text_clip,
                z.saturating_add(8),
                node_graph_text_primitive(
                    TextPlacement {
                        origin: [text_x, center[1] - spacing.socket_label_inset_y * zoom],
                        text: socket.name.clone(),
                    },
                    theme.text_muted,
                    max_width,
                    bevy_text::Justify::Left,
                    &font,
                    theme,
                    zoom,
                ),
            ));
        }
        push_rect_outline_with_width(
            node_rect,
            if selected {
                theme.accent
            } else {
                theme.border.mix(theme.surface, 0.2)
            },
            node_clip,
            z.saturating_add(7),
            if selected { 2.0 } else { 1.0 },
            output,
        );
    }
}

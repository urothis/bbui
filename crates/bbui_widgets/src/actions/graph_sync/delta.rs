use super::*;

pub(super) fn node_graph_delta_view_items(
    context: NodeGraphViewContext<'_>,
    moved_nodes: &HashSet<NodeId>,
    changed_links: Option<&HashSet<u64>>,
) -> (Vec<NodeGraphViewItem>, HashSet<NodeGraphItemKey>) {
    let NodeGraphViewContext {
        owner,
        graph,
        layout,
        spacing,
        node_indices,
        link_indices,
        incident_links,
    } = context;
    let viewport = layout
        .clip
        .unwrap_or(layout.rect)
        .intersection(layout.rect)
        .and_then(|clip| {
            clip.intersection(Rect {
                y: layout.rect.y + spacing.header_height,
                height: (layout.rect.height - spacing.header_height).max(0.0),
                ..layout.rect
            })
        })
        .unwrap_or_default();
    let mut items = Vec::new();
    let mut replaced_keys = HashSet::new();
    let mut affected_links = HashSet::new();

    for node_id in moved_nodes {
        let node_key = NodeGraphItemKey::Node(*node_id);
        replaced_keys.insert(node_key);
        let Some(node) = node_indices
            .get(node_id)
            .and_then(|index| graph.nodes.get(*index))
            .filter(|node| node.id == *node_id)
        else {
            continue;
        };
        let node_rect = node_graph_node_rect(graph, layout.rect, node, spacing);
        if let Some(clip) = node_rect.intersection(viewport) {
            items.push(NodeGraphViewItem {
                key: node_key,
                node: Some(NodeGraphNodeControl {
                    owner,
                    node_id: node.id,
                    label: node.title.clone(),
                    selected: graph.selected.contains(&node.id),
                    collapsed: node.collapsed,
                }),
                node_paint_state: Some(NodeGraphNodePaintState {
                    signature: node_graph_node_signature(node, graph.selected.contains(&node.id))
                        .paint,
                }),
                socket: None,
                link: None,
                link_geometry: None,
                rect: node_rect,
                clip,
                z: layout.z.saturating_add(4),
            });
        }

        for (socket_index, socket) in node.sockets.iter().enumerate() {
            let key = NodeGraphItemKey::Socket(node.id, socket_index);
            replaced_keys.insert(key);
            let Some(center) =
                node_graph_socket_position(graph, layout.rect, node, socket_index, spacing)
            else {
                continue;
            };
            let socket_rect = Rect::new(center[0] - 8.0, center[1] - 8.0, 16.0, 16.0);
            let Some(clip) = socket_rect.intersection(viewport) else {
                continue;
            };
            items.push(NodeGraphViewItem {
                key,
                node: None,
                node_paint_state: None,
                socket: Some(NodeGraphSocketControl {
                    owner,
                    node_id: node.id,
                    socket_index,
                    direction: socket.direction,
                    label: socket.name.clone(),
                    socket_type: socket.socket_type.clone(),
                }),
                link: None,
                link_geometry: None,
                rect: socket_rect,
                clip,
                z: layout.z.saturating_add(6),
            });
        }

        if changed_links.is_none() {
            affected_links.extend(incident_links.get(node_id).into_iter().flatten().copied());
        }
    }

    if let Some(changed_links) = changed_links {
        affected_links.extend(changed_links.iter().copied());
    }

    for link_id in affected_links {
        let key = NodeGraphItemKey::Link(link_id);
        replaced_keys.insert(key);
        let Some(link) = link_indices
            .get(&link_id)
            .and_then(|index| graph.links.get(*index))
            .filter(|link| link.id == link_id)
            .copied()
        else {
            continue;
        };
        let (Some(from), Some(to)) = (
            node_indices
                .get(&link.from_node)
                .and_then(|index| graph.nodes.get(*index))
                .filter(|node| node.id == link.from_node),
            node_indices
                .get(&link.to_node)
                .and_then(|index| graph.nodes.get(*index))
                .filter(|node| node.id == link.to_node),
        ) else {
            continue;
        };
        let Some(points) =
            node_graph_link_points_between(graph, layout.rect, link, from, to, spacing)
        else {
            continue;
        };
        let min_x = points
            .iter()
            .map(|point| point[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|point| point[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = points
            .iter()
            .map(|point| point[1])
            .fold(f32::INFINITY, f32::min);
        let max_y = points
            .iter()
            .map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let rect = Rect::new(
            min_x - 8.0,
            min_y - 8.0,
            (max_x - min_x).max(16.0) + 16.0,
            (max_y - min_y).max(16.0) + 16.0,
        );
        let Some(clip) = rect.intersection(viewport) else {
            continue;
        };
        items.push(NodeGraphViewItem {
            key,
            node: None,
            node_paint_state: None,
            socket: None,
            link: Some(NodeGraphLinkControl {
                owner,
                link_id,
                from_node: link.from_node,
                from_socket: link.from_socket,
                to_node: link.to_node,
                to_socket: link.to_socket,
                selected: graph.selected_link == Some(link_id),
            }),
            link_geometry: Some(NodeGraphLinkPaintGeometry {
                points: points.map(|point| [point[0].to_bits(), point[1].to_bits()]),
                source_socket_type: from
                    .sockets
                    .get(link.from_socket)
                    .map(|socket| socket.socket_type.clone()),
            }),
            rect,
            clip,
            z: layout.z.saturating_add(2),
        });
    }

    (items, replaced_keys)
}

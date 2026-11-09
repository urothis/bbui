use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NodeGraphHit {
    Empty,
    NodeHeader(NodeId),
    NodeBody(NodeId),
    Socket {
        node: NodeId,
        socket: usize,
        direction: SocketDirection,
    },
    Link(u64),
}

pub(super) fn node_graph_screen_position(
    graph: &NodeGraph,
    rect: Rect,
    point: [f32; 2],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> [f32; 2] {
    [
        rect.x + graph.pan[0] + point[0] * graph.zoom,
        rect.y + spacing.header_height + graph.pan[1] + point[1] * graph.zoom,
    ]
}

pub(super) fn node_graph_data_position(
    graph: &NodeGraph,
    rect: Rect,
    point: [f32; 2],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> [f32; 2] {
    [
        (point[0] - rect.x - graph.pan[0]) / graph.zoom,
        (point[1] - rect.y - spacing.header_height - graph.pan[1]) / graph.zoom,
    ]
}

pub(super) fn node_graph_node_rect(
    graph: &NodeGraph,
    rect: Rect,
    node: &crate::NodeModel,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Rect {
    let input_count = node
        .sockets
        .iter()
        .filter(|socket| socket.direction == SocketDirection::Input)
        .count();
    let output_count = node
        .sockets
        .iter()
        .filter(|socket| socket.direction == SocketDirection::Output)
        .count();
    let socket_rows = input_count.max(output_count) as f32;
    let graph_height = if node.collapsed {
        spacing.header_height
    } else {
        node.size[1].max(
            spacing.header_height + socket_rows * spacing.socket_spacing + spacing.body_padding,
        )
    };
    let origin = node_graph_screen_position(graph, rect, node.position, spacing);
    Rect {
        x: origin[0],
        y: origin[1],
        width: node.size[0] * graph.zoom,
        height: graph_height * graph.zoom,
    }
}

pub(super) fn node_graph_socket_position(
    graph: &NodeGraph,
    rect: Rect,
    node: &crate::NodeModel,
    socket_index: usize,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Option<[f32; 2]> {
    let socket = node.sockets.get(socket_index)?;
    let row = node.sockets[..socket_index]
        .iter()
        .filter(|previous| previous.direction == socket.direction)
        .count();
    let node_rect = node_graph_node_rect(graph, rect, node, spacing);
    let y = if node.collapsed {
        node_rect.y + spacing.header_height * graph.zoom * 0.5
    } else {
        node_rect.y
            + (spacing.header_height
                + row as f32 * spacing.socket_spacing
                + spacing.socket_spacing * 0.5)
                * graph.zoom
    };
    Some([
        if socket.direction == SocketDirection::Input {
            node_rect.x
        } else {
            node_rect.x + node_rect.width
        },
        y,
    ])
}

pub(super) fn cache_node_graph_socket_positions(
    zoom: f32,
    node: &crate::NodeModel,
    node_rect: Rect,
    spacing: &bui_style::ThemeNodeGraphSpacing,
    positions: &mut HashMap<(NodeId, usize), [f32; 2]>,
) {
    let mut input_row = 0usize;
    let mut output_row = 0usize;
    for (socket_index, socket) in node.sockets.iter().enumerate() {
        let row = match socket.direction {
            SocketDirection::Input => {
                let row = input_row;
                input_row = input_row.saturating_add(1);
                row
            }
            SocketDirection::Output => {
                let row = output_row;
                output_row = output_row.saturating_add(1);
                row
            }
        };
        let y = if node.collapsed {
            node_rect.y + spacing.header_height * zoom * 0.5
        } else {
            node_rect.y
                + (spacing.header_height
                    + row as f32 * spacing.socket_spacing
                    + spacing.socket_spacing * 0.5)
                    * zoom
        };
        let x = if socket.direction == SocketDirection::Input {
            node_rect.x
        } else {
            node_rect.x + node_rect.width
        };
        positions.insert((node.id, socket_index), [x, y]);
    }
}

pub(super) fn node_graph_link_points(
    graph: &NodeGraph,
    rect: Rect,
    link: crate::NodeLink,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Option<[[f32; 2]; 4]> {
    let from = graph.nodes.iter().find(|node| node.id == link.from_node)?;
    let to = graph.nodes.iter().find(|node| node.id == link.to_node)?;
    node_graph_link_points_between(graph, rect, link, from, to, spacing)
}

pub(super) fn node_graph_link_points_between(
    graph: &NodeGraph,
    rect: Rect,
    link: crate::NodeLink,
    from: &crate::NodeModel,
    to: &crate::NodeModel,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Option<[[f32; 2]; 4]> {
    let p0 = node_graph_socket_position(graph, rect, from, link.from_socket, spacing)?;
    let p3 = node_graph_socket_position(graph, rect, to, link.to_socket, spacing)?;
    let tangent = ((p3[0] - p0[0]).abs() * 0.5).max(spacing.connection_tangent_min * graph.zoom);
    let direction = if p3[0] >= p0[0] { 1.0 } else { -1.0 };
    Some([
        p0,
        [p0[0] + tangent * direction, p0[1]],
        [p3[0] - tangent * direction, p3[1]],
        p3,
    ])
}

pub(super) fn node_graph_bezier_bounds(points: [[f32; 2]; 4], padding: f32) -> Rect {
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
    Rect::new(
        min_x - padding,
        min_y - padding,
        max_x - min_x + padding * 2.0,
        max_y - min_y + padding * 2.0,
    )
}

pub(super) fn node_graph_hit(
    graph: &NodeGraph,
    rect: Rect,
    position: [f32; 2],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> NodeGraphHit {
    for node in graph.nodes.iter().rev() {
        for (socket_index, socket) in node.sockets.iter().enumerate() {
            if let Some(center) =
                node_graph_socket_position(graph, rect, node, socket_index, spacing)
                && distance(center, position) <= spacing.socket_radius + spacing.socket_hit_slop
            {
                return NodeGraphHit::Socket {
                    node: node.id,
                    socket: socket_index,
                    direction: socket.direction,
                };
            }
        }
    }
    for node in graph.nodes.iter().rev() {
        let node_rect = node_graph_node_rect(graph, rect, node, spacing);
        if node_rect.contains(position[0], position[1]) {
            return if position[1] <= node_rect.y + spacing.header_height * graph.zoom {
                NodeGraphHit::NodeHeader(node.id)
            } else {
                NodeGraphHit::NodeBody(node.id)
            };
        }
    }
    let nodes_by_id: HashMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    for link in graph.links.iter().rev() {
        let (Some(from), Some(to)) = (
            nodes_by_id.get(&link.from_node),
            nodes_by_id.get(&link.to_node),
        ) else {
            continue;
        };
        if let Some(points) = node_graph_link_points_between(graph, rect, *link, from, to, spacing)
            && node_graph_bezier_bounds(points, 7.0).contains(position[0], position[1])
            && bezier_distance(points, position) <= 7.0
        {
            return NodeGraphHit::Link(link.id);
        }
    }
    NodeGraphHit::Empty
}

pub(super) fn node_graph_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<NodeGraph>(target).is_some() {
        return Some(target);
    }
    world
        .get::<NodeGraphNodeControl>(target)
        .map(|control| control.owner)
        .or_else(|| {
            world
                .get::<NodeGraphSocketControl>(target)
                .map(|control| control.owner)
        })
        .or_else(|| {
            world
                .get::<NodeGraphLinkControl>(target)
                .map(|control| control.owner)
        })
}

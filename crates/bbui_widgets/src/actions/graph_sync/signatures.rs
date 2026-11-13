use super::*;

pub(super) fn node_graph_node_signature(
    node: &crate::NodeModel,
    selected: bool,
) -> NodeGraphNodeSignature {
    let mut paint = DefaultHasher::new();
    node.id.hash(&mut paint);
    node.title.hash(&mut paint);
    (node.category as u8).hash(&mut paint);
    node.position.map(f32::to_bits).hash(&mut paint);
    node.size.map(f32::to_bits).hash(&mut paint);
    node.collapsed.hash(&mut paint);
    selected.hash(&mut paint);
    node.sockets.len().hash(&mut paint);

    let mut socket_layout = DefaultHasher::new();
    for socket in &node.sockets {
        socket.name.hash(&mut paint);
        (socket.direction as u8).hash(&mut paint);
        (socket.placement as u8).hash(&mut paint);
        socket.socket_type.hash(&mut paint);
        (socket.direction as u8).hash(&mut socket_layout);
        (socket.placement as u8).hash(&mut socket_layout);
    }

    NodeGraphNodeSignature {
        paint: paint.finish(),
        geometry: NodeGraphNodeGeometrySignature {
            position: node.position.map(f32::to_bits),
            size: node.size.map(f32::to_bits),
            collapsed: node.collapsed,
            socket_layout: socket_layout.finish(),
        },
    }
}

pub(super) fn node_graph_link_signature(
    graph: &NodeGraph,
    link: crate::NodeLink,
    node_indices: &HashMap<NodeId, usize>,
    node_signatures: &HashMap<NodeId, NodeGraphNodeSignature>,
) -> NodeGraphLinkSignature {
    let source_socket_type = node_indices
        .get(&link.from_node)
        .and_then(|index| graph.nodes.get(*index))
        .and_then(|node| node.sockets.get(link.from_socket))
        .map(|socket| socket.socket_type.clone());
    NodeGraphLinkSignature {
        from_node: link.from_node,
        from_socket: link.from_socket,
        to_node: link.to_node,
        to_socket: link.to_socket,
        from_geometry: node_signatures
            .get(&link.from_node)
            .map(|signature| signature.geometry),
        to_geometry: node_signatures
            .get(&link.to_node)
            .map(|signature| signature.geometry),
        source_socket_type,
        selected: graph.selected_link == Some(link.id),
    }
}

pub(super) fn node_graph_transform_signature(graph: &NodeGraph) -> NodeGraphTransformSignature {
    NodeGraphTransformSignature {
        pan: graph.pan.map(f32::to_bits),
        zoom: graph.zoom.to_bits(),
    }
}

pub(super) fn node_graph_signature_snapshot(graph: &NodeGraph) -> NodeGraphSignatureSnapshot {
    let node_indices: HashMap<_, _> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect();
    let node_signatures: HashMap<_, _> = graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.id,
                node_graph_node_signature(node, graph.selected.contains(&node.id)),
            )
        })
        .collect();
    let node_order = graph.nodes.iter().map(|node| node.id).collect();
    let link_indices: HashMap<_, _> = graph
        .links
        .iter()
        .enumerate()
        .map(|(index, link)| (link.id, index))
        .collect();
    let link_order: Vec<_> = graph.links.iter().map(|link| link.id).collect();
    let mut incident_links = HashMap::<NodeId, Vec<u64>>::new();
    for link in &graph.links {
        incident_links
            .entry(link.from_node)
            .or_default()
            .push(link.id);
        incident_links
            .entry(link.to_node)
            .or_default()
            .push(link.id);
    }
    let link_signatures = graph
        .links
        .iter()
        .map(|link| {
            (
                link.id,
                node_graph_link_signature(graph, *link, &node_indices, &node_signatures),
            )
        })
        .collect();

    NodeGraphSignatureSnapshot {
        node_indices,
        node_signatures,
        node_order,
        link_indices,
        link_signatures,
        link_order,
        incident_links,
        transform: node_graph_transform_signature(graph),
    }
}

pub(super) fn store_node_graph_signature_snapshot(
    owner: Entity,
    snapshot: NodeGraphSignatureSnapshot,
    cache: &mut WidgetNodeGraphItems,
) {
    cache.node_indices.insert(owner, snapshot.node_indices);
    cache
        .node_signatures
        .insert(owner, snapshot.node_signatures);
    cache.node_order.insert(owner, snapshot.node_order);
    cache.link_indices.insert(owner, snapshot.link_indices);
    cache
        .link_signatures
        .insert(owner, snapshot.link_signatures);
    cache.link_order.insert(owner, snapshot.link_order);
    cache.incident_links.insert(owner, snapshot.incident_links);
    cache.transforms.insert(owner, snapshot.transform);
}

pub(super) fn changed_signature_ids<K: Copy + Eq + std::hash::Hash, V: PartialEq>(
    previous: &HashMap<K, V>,
    current: &HashMap<K, V>,
) -> HashSet<K> {
    current
        .iter()
        .filter_map(|(key, value)| (previous.get(key) != Some(value)).then_some(*key))
        .chain(
            previous
                .keys()
                .filter(|key| !current.contains_key(key))
                .copied(),
        )
        .collect()
}

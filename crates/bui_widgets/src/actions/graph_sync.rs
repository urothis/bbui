use super::*;

/// Materialize clipped, stable node/socket/link accessibility and hit-test children.
pub fn sync_node_graph_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    let mut query = world.query::<(Entity, &NodeGraph, Option<&UiLayoutResult>)>();
    let owners = query
        .iter(world)
        .filter_map(|(owner, _, layout)| {
            virtual_view_stamp::<NodeGraph>(world, owner)
                .map(|stamp| (owner, layout.copied(), stamp))
        })
        .collect::<Vec<_>>();
    let live_owners: HashSet<_> = owners.iter().map(|(owner, _, _)| *owner).collect();
    let mut cache = {
        let mut resource = world.resource_mut::<WidgetNodeGraphItems>();
        std::mem::take(&mut *resource)
    };
    let mut previous = std::mem::take(&mut cache.items);
    let mut stamps = std::mem::take(&mut cache.stamps);
    let (mut deltas, mut forced_signature_diffs) = {
        let mut graph_deltas = world.resource_mut::<WidgetNodeGraphDeltas>();
        (
            std::mem::take(&mut graph_deltas.0),
            std::mem::take(&mut graph_deltas.1),
        )
    };
    let mut views = Vec::new();

    for (owner, layout, stamp) in owners.iter().copied() {
        if stamps.get(&owner) == Some(&stamp) && previous.contains_key(&owner) {
            continue;
        }
        let moved_nodes = deltas.remove(&owner).filter(|nodes| !nodes.is_empty());
        let force_signature_diff = forced_signature_diffs.remove(&owner);
        let previous_stamp = stamps.get(&owner).copied();
        let transform = world
            .get::<NodeGraph>(owner)
            .map(node_graph_transform_signature);
        let partial_update = moved_nodes.as_ref().filter(|_| {
            !force_signature_diff && {
                previous.contains_key(&owner)
                    && previous_stamp.is_some_and(|previous| {
                        previous.layout_changed == stamp.layout_changed
                            && previous.theme_changed == stamp.theme_changed
                            && previous.style_changed == stamp.style_changed
                            && previous.key_changed == stamp.key_changed
                    })
                    && cache.node_indices.contains_key(&owner)
                    && cache.link_indices.contains_key(&owner)
                    && cache.incident_links.contains_key(&owner)
                    && cache.node_signatures.contains_key(&owner)
                    && cache.link_signatures.contains_key(&owner)
                    && cache.transforms.get(&owner) == transform.as_ref()
            }
        });
        let Some(layout) = layout else {
            views.push(NodeGraphView {
                owner,
                items: Vec::new(),
                replaced_keys: HashSet::new(),
                replace_all: true,
            });
            continue;
        };
        let Some(graph) = world.get::<NodeGraph>(owner) else {
            continue;
        };
        let pan_only = cache.viewport_ticks.get(&owner) == Some(&stamp.model_changed)
            && moved_nodes.is_none()
            && !force_signature_diff
            && previous.contains_key(&owner)
            && previous_stamp.is_some_and(|old| {
                old.layout_changed == stamp.layout_changed
                    && old.theme_changed == stamp.theme_changed
                    && old.style_changed == stamp.style_changed
                    && old.key_changed == stamp.key_changed
            })
            && cache.spatial_cells.contains_key(&owner)
            && cache.node_indices.contains_key(&owner)
            && cache.link_indices.contains_key(&owner)
            && cache.incident_links.contains_key(&owner)
            && cache
                .transforms
                .get(&owner)
                .is_some_and(|old| old.zoom == graph.zoom.to_bits());
        if pan_only {
            let candidates = node_graph_spatial_candidates(&cache, owner, graph, layout, spacing);
            let mut nodes = HashSet::new();
            let mut links = HashSet::new();
            for key in candidates
                .into_iter()
                .chain(previous[&owner].keys().copied())
            {
                match key {
                    NodeGraphItemKey::Node(id) | NodeGraphItemKey::Socket(id, _) => {
                        nodes.insert(id);
                    }
                    NodeGraphItemKey::Link(id) => {
                        links.insert(id);
                    }
                }
            }
            let (items, replaced_keys) = node_graph_delta_view_items(
                NodeGraphViewContext {
                    owner,
                    graph,
                    layout,
                    spacing,
                    node_indices: &cache.node_indices[&owner],
                    link_indices: &cache.link_indices[&owner],
                    incident_links: &cache.incident_links[&owner],
                },
                &nodes,
                Some(&links),
            );
            cache
                .transforms
                .insert(owner, node_graph_transform_signature(graph));
            views.push(NodeGraphView {
                owner,
                items,
                replaced_keys,
                replace_all: false,
            });
            continue;
        }
        if let Some(moved_nodes) = partial_update
            && let (Some(node_indices), Some(link_indices), Some(incident_links)) = (
                cache.node_indices.get(&owner),
                cache.link_indices.get(&owner),
                cache.incident_links.get(&owner),
            )
        {
            let (items, replaced_keys) = node_graph_delta_view_items(
                NodeGraphViewContext {
                    owner,
                    graph,
                    layout,
                    spacing,
                    node_indices,
                    link_indices,
                    incident_links,
                },
                moved_nodes,
                None,
            );
            let affected_links: HashSet<_> = moved_nodes
                .iter()
                .flat_map(|node_id| {
                    cache
                        .incident_links
                        .get(&owner)
                        .and_then(|incident| incident.get(node_id))
                        .into_iter()
                        .flatten()
                        .copied()
                })
                .collect();
            let affected_keys = moved_nodes
                .iter()
                .copied()
                .map(NodeGraphItemKey::Node)
                .chain(affected_links.iter().copied().map(NodeGraphItemKey::Link))
                .collect::<Vec<_>>();
            update_node_graph_spatial_index(
                &mut cache,
                owner,
                graph,
                layout.rect,
                spacing,
                &affected_keys,
            );
            if let Some(signatures) = cache.node_signatures.get_mut(&owner) {
                for node_id in moved_nodes {
                    if let Some(node) = cache
                        .node_indices
                        .get(&owner)
                        .and_then(|indices| indices.get(node_id))
                        .and_then(|index| graph.nodes.get(*index))
                    {
                        signatures.insert(
                            *node_id,
                            node_graph_node_signature(node, graph.selected.contains(node_id)),
                        );
                    }
                }
            }
            if let (Some(signatures), Some(nodes), Some(indices)) = (
                cache.link_signatures.get_mut(&owner),
                cache.node_signatures.get(&owner),
                cache.node_indices.get(&owner),
            ) {
                for link_id in affected_links {
                    if let Some(link) = cache
                        .link_indices
                        .get(&owner)
                        .and_then(|indices| indices.get(&link_id))
                        .and_then(|index| graph.links.get(*index))
                    {
                        signatures.insert(
                            link_id,
                            node_graph_link_signature(graph, *link, indices, nodes),
                        );
                    }
                }
            }
            views.push(NodeGraphView {
                owner,
                items,
                replaced_keys,
                replace_all: false,
            });
            continue;
        }
        let signature_snapshot = node_graph_signature_snapshot(graph);
        let unchanged_structure = previous.contains_key(&owner)
            && cache.node_order.get(&owner) == Some(&signature_snapshot.node_order)
            && cache.link_order.get(&owner) == Some(&signature_snapshot.link_order);
        let unchanged_context = previous_stamp.is_some_and(|previous| {
            previous.layout_changed == stamp.layout_changed
                && previous.theme_changed == stamp.theme_changed
                && previous.style_changed == stamp.style_changed
                && previous.key_changed == stamp.key_changed
        }) && cache.transforms.get(&owner)
            == Some(&signature_snapshot.transform);
        if unchanged_structure
            && unchanged_context
            && let (Some(previous_nodes), Some(previous_links)) = (
                cache.node_signatures.get(&owner),
                cache.link_signatures.get(&owner),
            )
        {
            let changed_nodes =
                changed_signature_ids(previous_nodes, &signature_snapshot.node_signatures);
            let changed_links =
                changed_signature_ids(previous_links, &signature_snapshot.link_signatures);
            if changed_nodes.is_empty() && changed_links.is_empty() {
                store_node_graph_signature_snapshot(owner, signature_snapshot, &mut cache);
                continue;
            }
            if changed_nodes.iter().any(|id| {
                previous_nodes.get(id).map(|signature| signature.geometry)
                    != signature_snapshot
                        .node_signatures
                        .get(id)
                        .map(|signature| signature.geometry)
            }) || !changed_links.is_empty()
            {
                let (cells, overflow, membership, levels) = build_node_graph_spatial_index(
                    graph,
                    layout.rect,
                    spacing,
                    &signature_snapshot.node_indices,
                    &signature_snapshot.link_indices,
                );
                cache.spatial_cells.insert(owner, cells);
                cache.spatial_overflow.insert(owner, overflow);
                cache.spatial_membership.insert(owner, membership);
                cache.spatial_levels.insert(owner, levels);
            }
            let (items, replaced_keys) = node_graph_delta_view_items(
                NodeGraphViewContext {
                    owner,
                    graph,
                    layout,
                    spacing,
                    node_indices: &signature_snapshot.node_indices,
                    link_indices: &signature_snapshot.link_indices,
                    incident_links: &signature_snapshot.incident_links,
                },
                &changed_nodes,
                Some(&changed_links),
            );
            views.push(NodeGraphView {
                owner,
                items,
                replaced_keys,
                replace_all: false,
            });
            store_node_graph_signature_snapshot(owner, signature_snapshot, &mut cache);
            continue;
        }
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
        let socket_count = graph.nodes.iter().map(|node| node.sockets.len()).sum();
        let mut socket_positions = HashMap::with_capacity(socket_count);
        for node in &graph.nodes {
            let rect = node_graph_node_rect(graph, layout.rect, node, spacing);
            let Some(clip) = rect.intersection(viewport) else {
                // Socket positions are still needed to resolve links whose nodes sit outside
                // the viewport while their curves cross it.
                cache_node_graph_socket_positions(
                    graph.zoom,
                    node,
                    rect,
                    spacing,
                    &mut socket_positions,
                );
                continue;
            };
            items.push(NodeGraphViewItem {
                key: NodeGraphItemKey::Node(node.id),
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
                rect,
                clip,
                z: layout.z.saturating_add(4),
            });
            cache_node_graph_socket_positions(
                graph.zoom,
                node,
                rect,
                spacing,
                &mut socket_positions,
            );
            for (socket_index, socket) in node.sockets.iter().enumerate() {
                let Some(center) = socket_positions.get(&(node.id, socket_index)).copied() else {
                    continue;
                };
                let socket_rect = Rect::new(center[0] - 8.0, center[1] - 8.0, 16.0, 16.0);
                let Some(socket_clip) = socket_rect.intersection(viewport) else {
                    continue;
                };
                items.push(NodeGraphViewItem {
                    key: NodeGraphItemKey::Socket(node.id, socket_index),
                    node: None,
                    node_paint_state: None,
                    socket: Some(NodeGraphSocketControl {
                        owner,
                        node_id: node.id,
                        socket_index,
                        direction: socket.direction,
                        label: socket.name.clone(),
                        socket_type: socket.socket_type,
                    }),
                    link: None,
                    link_geometry: None,
                    rect: socket_rect,
                    clip: socket_clip,
                    z: layout.z.saturating_add(6),
                });
            }
        }
        for link in &graph.links {
            let (Some(from_node), Some(to_node)) = (
                signature_snapshot
                    .node_indices
                    .get(&link.from_node)
                    .and_then(|index| graph.nodes.get(*index)),
                signature_snapshot
                    .node_indices
                    .get(&link.to_node)
                    .and_then(|index| graph.nodes.get(*index)),
            ) else {
                continue;
            };
            let Some(points) = node_graph_link_points_between(
                graph,
                layout.rect,
                *link,
                from_node,
                to_node,
                spacing,
            ) else {
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
                key: NodeGraphItemKey::Link(link.id),
                node: None,
                node_paint_state: None,
                socket: None,
                link: Some(NodeGraphLinkControl {
                    owner,
                    link_id: link.id,
                    from_node: link.from_node,
                    from_socket: link.from_socket,
                    to_node: link.to_node,
                    to_socket: link.to_socket,
                    selected: graph.selected_link == Some(link.id),
                }),
                link_geometry: Some(NodeGraphLinkPaintGeometry {
                    points: points.map(|point| [point[0].to_bits(), point[1].to_bits()]),
                    source_socket_type: from_node
                        .sockets
                        .get(link.from_socket)
                        .map(|socket| socket.socket_type),
                }),
                rect,
                clip,
                z: layout.z.saturating_add(2),
            });
        }
        let replaced_keys = items.iter().map(|item| item.key).collect();
        let (cells, overflow, membership, levels) = build_node_graph_spatial_index(
            graph,
            layout.rect,
            spacing,
            &signature_snapshot.node_indices,
            &signature_snapshot.link_indices,
        );
        cache.spatial_cells.insert(owner, cells);
        cache.spatial_overflow.insert(owner, overflow);
        cache.spatial_membership.insert(owner, membership);
        cache.spatial_levels.insert(owner, levels);
        views.push(NodeGraphView {
            owner,
            items,
            replaced_keys,
            replace_all: true,
        });
        store_node_graph_signature_snapshot(owner, signature_snapshot, &mut cache);
    }

    let did_sync = !views.is_empty();
    for view in views {
        let mut current_items = previous.remove(&view.owner).unwrap_or_default();
        let replace_all = view.replace_all;
        let ordered_keys: Vec<_> = view.items.iter().map(|item| item.key).collect();
        let mut mounted_keys = HashSet::with_capacity(view.items.len());
        let owner_layout = world.get::<UiLayoutResult>(view.owner).copied();
        let owner_key = world.get::<UiKey>(view.owner).map_or_else(
            || format!("entity:{:016x}", view.owner.to_bits()),
            |key| key.0.clone(),
        );
        if let Some(owner_layout) = owner_layout {
            for view_item in view.items {
                mounted_keys.insert(view_item.key);
                let entity = current_items
                    .get(&view_item.key)
                    .copied()
                    .filter(|entity| world.get_entity(*entity).is_ok())
                    .unwrap_or_else(|| {
                        world.spawn((UiNode, UiEnabled(true), UiVisible(true))).id()
                    });
                insert_component_if_changed(world, entity, UiEnabled(true));
                insert_component_if_changed(world, entity, UiVisible(true));
                insert_component_if_changed(
                    world,
                    entity,
                    UiLayoutResult {
                        window: owner_layout.window,
                        rect: view_item.rect,
                        clip: Some(view_item.clip),
                        z: view_item.z,
                        content_size: [view_item.rect.width, view_item.rect.height],
                        dpi_scale: owner_layout.dpi_scale,
                    },
                );
                insert_component_if_changed(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(owner_layout.window),
                        rect: view_item.rect,
                        clip: Some(view_item.clip),
                        z_index: view_item.z,
                    },
                );
                let (key_suffix, role) = match view_item.key {
                    NodeGraphItemKey::Node(id) => (format!("node:{}", id.0), "node-graph-node"),
                    NodeGraphItemKey::Socket(id, socket) => {
                        (format!("socket:{}:{socket}", id.0), "node-graph-socket")
                    }
                    NodeGraphItemKey::Link(id) => (format!("link:{id}"), "node-graph-link"),
                };
                insert_component_if_changed(
                    world,
                    entity,
                    UiKey(format!("{owner_key}/{key_suffix}")),
                );
                insert_component_if_changed(world, entity, UiRole::new(role));
                if let Some(node) = view_item.node {
                    insert_component_if_changed(world, entity, node.clone());
                    if let Some(paint_state) = view_item.node_paint_state {
                        insert_component_if_changed(world, entity, paint_state);
                    }
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, node.selected);
                    remove_component_if_present::<NodeGraphSocketControl>(world, entity);
                    remove_component_if_present::<NodeGraphLinkControl>(world, entity);
                    remove_component_if_present::<NodeGraphLinkPaintGeometry>(world, entity);
                } else if let Some(socket) = view_item.socket {
                    insert_component_if_changed(world, entity, socket);
                    remove_component_if_present::<NodeGraphNodeControl>(world, entity);
                    remove_component_if_present::<NodeGraphNodePaintState>(world, entity);
                    remove_component_if_present::<NodeGraphLinkControl>(world, entity);
                    remove_component_if_present::<NodeGraphLinkPaintGeometry>(world, entity);
                } else if let Some(link) = view_item.link {
                    insert_component_if_changed(world, entity, link);
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, link.selected);
                    if let Some(geometry) = view_item.link_geometry {
                        insert_component_if_changed(world, entity, geometry);
                    }
                    remove_component_if_present::<NodeGraphNodeControl>(world, entity);
                    remove_component_if_present::<NodeGraphNodePaintState>(world, entity);
                    remove_component_if_present::<NodeGraphSocketControl>(world, entity);
                } else {
                    remove_component_if_present::<NodeGraphLinkPaintGeometry>(world, entity);
                    remove_component_if_present::<NodeGraphNodePaintState>(world, entity);
                }
                if world
                    .get::<UiParent>(entity)
                    .is_none_or(|parent| parent.0 != view.owner)
                {
                    let _ = set_parent(world, entity, view.owner);
                }
                current_items.insert(view_item.key, entity);
            }
        }
        let stale_keys: Vec<_> = if view.replace_all {
            current_items
                .keys()
                .filter(|key| !view.replaced_keys.contains(key))
                .copied()
                .collect()
        } else {
            view.replaced_keys
                .iter()
                .filter(|key| !mounted_keys.contains(key))
                .copied()
                .collect()
        };
        for key in stale_keys {
            if let Some(entity) = current_items.remove(&key)
                && world.get_entity(entity).is_ok()
            {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
        if replace_all {
            let ordered_entities = ordered_keys
                .iter()
                .filter_map(|key| current_items.get(key).copied())
                .collect::<Vec<_>>();
            reorder_node_graph_children(world, view.owner, &ordered_entities);
        }
        previous.insert(view.owner, current_items);
    }
    let stale_owners: Vec<_> = previous
        .keys()
        .filter(|owner| !live_owners.contains(owner))
        .copied()
        .collect();
    for owner in stale_owners {
        let Some(items) = previous.remove(&owner) else {
            continue;
        };
        cache.node_indices.remove(&owner);
        cache.link_indices.remove(&owner);
        cache.incident_links.remove(&owner);
        cache.node_signatures.remove(&owner);
        cache.node_order.remove(&owner);
        cache.link_signatures.remove(&owner);
        cache.link_order.remove(&owner);
        cache.transforms.remove(&owner);
        cache.spatial_cells.remove(&owner);
        cache.spatial_overflow.remove(&owner);
        cache.spatial_membership.remove(&owner);
        cache.spatial_levels.remove(&owner);
        for entity in items.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
    }
    for (owner, _, stamp) in owners {
        stamps.insert(owner, stamp);
    }
    stamps.retain(|owner, _| live_owners.contains(owner));
    cache
        .node_indices
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .link_indices
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .incident_links
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .node_signatures
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .node_order
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .link_signatures
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .link_order
        .retain(|owner, _| live_owners.contains(owner));
    cache
        .transforms
        .retain(|owner, _| live_owners.contains(owner));
    cache.items = previous;
    cache.stamps = stamps;
    cache.journaled_ticks.clear();
    cache.viewport_ticks.clear();
    world.insert_resource(cache);
    if did_sync {
        world.increment_change_tick();
    }
}

pub(super) fn reorder_node_graph_children(world: &mut World, owner: Entity, ordered: &[Entity]) {
    let Some(mut children) = world.get_mut::<UiChildren>(owner) else {
        return;
    };
    let graph_children: HashSet<_> = ordered.iter().copied().collect();
    let insertion = children
        .0
        .iter()
        .position(|child| graph_children.contains(child))
        .unwrap_or(children.0.len());
    let mut next: Vec<_> = children
        .0
        .iter()
        .copied()
        .filter(|child| !graph_children.contains(child))
        .collect();
    let insertion = insertion.min(next.len());
    next.splice(insertion..insertion, ordered.iter().copied());
    if children.0 != next {
        children.0 = next;
    }
}

const GRAPH_SPATIAL_CELL: f32 = 256.0;

fn graph_spatial_cell(value: f32, level: u8) -> i32 {
    (value / (GRAPH_SPATIAL_CELL * 2.0_f32.powi(i32::from(level)))).floor() as i32
}

fn insert_graph_spatial_bounds(
    cells: &mut GraphSpatialCells,
    overflow: &mut Vec<NodeGraphItemKey>,
    membership: &mut GraphSpatialMembership,
    levels: &mut HashSet<u8>,
    key: NodeGraphItemKey,
    rect: Rect,
) {
    if ![rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f32::is_finite)
    {
        overflow.push(key);
        membership.insert(key, Vec::new());
        return;
    }
    if ![rect.x + rect.width, rect.y + rect.height]
        .into_iter()
        .all(f32::is_finite)
    {
        overflow.push(key);
        membership.insert(key, Vec::new());
        return;
    }
    let Some((level, x0, x1, y0, y1)) = (0..=31_u8).find_map(|level| {
        let x0 = graph_spatial_cell(rect.x, level);
        let x1 = graph_spatial_cell(rect.x + rect.width, level);
        let y0 = graph_spatial_cell(rect.y, level);
        let y1 = graph_spatial_cell(rect.y + rect.height, level);
        let width = (i64::from(x1) - i64::from(x0) + 1).max(0);
        let height = (i64::from(y1) - i64::from(y0) + 1).max(0);
        (width.saturating_mul(height) <= 256).then_some((level, x0, x1, y0, y1))
    }) else {
        overflow.push(key);
        membership.insert(key, Vec::new());
        return;
    };
    levels.insert(level);
    let mut occupied = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            cells.entry((level, x, y)).or_default().push(key);
            occupied.push((level, x, y));
        }
    }
    membership.insert(key, occupied);
}

fn graph_spatial_bounds_for_key(
    graph: &NodeGraph,
    layout: Rect,
    spacing: &bui_style::ThemeNodeGraphSpacing,
    indices: &HashMap<NodeId, usize>,
    link_indices: &HashMap<u64, usize>,
    key: NodeGraphItemKey,
) -> Option<Rect> {
    let offset = [
        layout.x + graph.pan[0],
        layout.y + spacing.header_height + graph.pan[1],
    ];
    match key {
        NodeGraphItemKey::Node(id) => {
            let node = indices.get(&id).and_then(|index| graph.nodes.get(*index))?;
            let mut rect = node_graph_node_rect(graph, layout, node, spacing);
            rect.x -= offset[0];
            rect.y -= offset[1];
            Some(rect)
        }
        NodeGraphItemKey::Link(id) => {
            let link = link_indices
                .get(&id)
                .and_then(|index| graph.links.get(*index))?;
            let from = indices
                .get(&link.from_node)
                .and_then(|index| graph.nodes.get(*index))?;
            let to = indices
                .get(&link.to_node)
                .and_then(|index| graph.nodes.get(*index))?;
            let mut points =
                node_graph_link_points_between(graph, layout, *link, from, to, spacing)?;
            for point in &mut points {
                point[0] -= offset[0];
                point[1] -= offset[1];
            }
            let mut bounds = node_graph_bezier_bounds(points, 8.0);
            // Link controls use a minimum 16 px span before their 8 px padding.
            // Keep the index conservative for nearly vertical or horizontal curves.
            bounds.width = bounds.width.max(32.0);
            bounds.height = bounds.height.max(32.0);
            Some(bounds)
        }
        NodeGraphItemKey::Socket(_, _) => None,
    }
}

fn build_node_graph_spatial_index(
    graph: &NodeGraph,
    layout: Rect,
    spacing: &bui_style::ThemeNodeGraphSpacing,
    indices: &HashMap<NodeId, usize>,
    link_indices: &HashMap<u64, usize>,
) -> (
    GraphSpatialCells,
    Vec<NodeGraphItemKey>,
    GraphSpatialMembership,
    HashSet<u8>,
) {
    let mut cells = HashMap::new();
    let mut overflow = Vec::new();
    let mut membership = HashMap::new();
    let mut levels = HashSet::new();
    for node in &graph.nodes {
        let key = NodeGraphItemKey::Node(node.id);
        if let Some(rect) =
            graph_spatial_bounds_for_key(graph, layout, spacing, indices, link_indices, key)
        {
            insert_graph_spatial_bounds(
                &mut cells,
                &mut overflow,
                &mut membership,
                &mut levels,
                key,
                rect,
            );
        }
    }
    for link in &graph.links {
        let key = NodeGraphItemKey::Link(link.id);
        if let Some(rect) =
            graph_spatial_bounds_for_key(graph, layout, spacing, indices, link_indices, key)
        {
            insert_graph_spatial_bounds(
                &mut cells,
                &mut overflow,
                &mut membership,
                &mut levels,
                key,
                rect,
            );
        }
    }
    (cells, overflow, membership, levels)
}

fn update_node_graph_spatial_index(
    cache: &mut WidgetNodeGraphItems,
    owner: Entity,
    graph: &NodeGraph,
    layout: Rect,
    spacing: &bui_style::ThemeNodeGraphSpacing,
    keys: &[NodeGraphItemKey],
) {
    let (
        Some(cells),
        Some(overflow),
        Some(membership),
        Some(levels),
        Some(indices),
        Some(link_indices),
    ) = (
        cache.spatial_cells.get_mut(&owner),
        cache.spatial_overflow.get_mut(&owner),
        cache.spatial_membership.get_mut(&owner),
        cache.spatial_levels.get_mut(&owner),
        cache.node_indices.get(&owner),
        cache.link_indices.get(&owner),
    )
    else {
        return;
    };
    for &key in keys {
        if let Some(occupied) = membership.remove(&key) {
            for cell in occupied {
                if let Some(entries) = cells.get_mut(&cell) {
                    entries.retain(|entry| *entry != key);
                }
            }
        }
        overflow.retain(|entry| *entry != key);
        if let Some(rect) =
            graph_spatial_bounds_for_key(graph, layout, spacing, indices, link_indices, key)
        {
            insert_graph_spatial_bounds(cells, overflow, membership, levels, key, rect);
        }
    }
}

fn node_graph_spatial_candidates(
    cache: &WidgetNodeGraphItems,
    owner: Entity,
    graph: &NodeGraph,
    layout: UiLayoutResult,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> HashSet<NodeGraphItemKey> {
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
    let offset = [
        layout.rect.x + graph.pan[0],
        layout.rect.y + spacing.header_height + graph.pan[1],
    ];
    let mut candidates = HashSet::new();
    if let (Some(cells), Some(levels)) = (
        cache.spatial_cells.get(&owner),
        cache.spatial_levels.get(&owner),
    ) {
        for &level in levels {
            let x0 = graph_spatial_cell(viewport.x - offset[0], level);
            let x1 = graph_spatial_cell(viewport.x + viewport.width - offset[0], level);
            let y0 = graph_spatial_cell(viewport.y - offset[1], level);
            let y1 = graph_spatial_cell(viewport.y + viewport.height - offset[1], level);
            let width = (i64::from(x1) - i64::from(x0) + 1).max(0);
            let height = (i64::from(y1) - i64::from(y0) + 1).max(0);
            if width.saturating_mul(height) > 4096 {
                candidates.extend(
                    cells
                        .iter()
                        .filter(|((entry_level, x, y), _)| {
                            *entry_level == level
                                && (*x >= x0 && *x <= x1)
                                && (*y >= y0 && *y <= y1)
                        })
                        .flat_map(|(_, entries)| entries.iter().copied()),
                );
                continue;
            }
            for x in x0..=x1 {
                for y in y0..=y1 {
                    candidates.extend(cells.get(&(level, x, y)).into_iter().flatten().copied());
                }
            }
        }
    }
    candidates.extend(
        cache
            .spatial_overflow
            .get(&owner)
            .into_iter()
            .flatten()
            .copied(),
    );
    candidates
}

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
        (socket.socket_type as u8).hash(&mut paint);
        (socket.direction as u8).hash(&mut socket_layout);
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
        .map(|socket| socket.socket_type);
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

pub(super) struct NodeGraphViewContext<'a> {
    owner: Entity,
    graph: &'a NodeGraph,
    layout: UiLayoutResult,
    spacing: &'a bui_style::ThemeNodeGraphSpacing,
    node_indices: &'a HashMap<NodeId, usize>,
    link_indices: &'a HashMap<u64, usize>,
    incident_links: &'a HashMap<NodeId, Vec<u64>>,
}

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
                    socket_type: socket.socket_type,
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
                    .map(|socket| socket.socket_type),
            }),
            rect,
            clip,
            z: layout.z.saturating_add(2),
        });
    }

    (items, replaced_keys)
}

#[cfg(test)]
mod spatial_tests {
    use super::*;

    #[test]
    fn near_vertical_link_index_covers_minimum_control_bounds() {
        let mut graph = NodeGraph::default();
        let from = graph.add_node(
            "Source",
            [0.0, 0.0],
            vec![NodeSocket::output("out", SocketType::Float)],
        );
        let source_width = graph.nodes[0].size[0];
        let to = graph.add_node(
            "Sink",
            [source_width, 0.0],
            vec![NodeSocket::input("in", SocketType::Float)],
        );
        let link = graph.create_link(from, 0, to, 0).unwrap();
        let nodes = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, index))
            .collect();
        let links = graph
            .links
            .iter()
            .enumerate()
            .map(|(index, link)| (link.id, index))
            .collect();
        let bounds = graph_spatial_bounds_for_key(
            &graph,
            Rect::new(0.0, 0.0, 500.0, 300.0),
            &Theme::default().spacing.node_graph,
            &nodes,
            &links,
            NodeGraphItemKey::Link(link),
        )
        .unwrap();
        assert!(bounds.width >= 32.0 && bounds.height >= 32.0);
    }
}

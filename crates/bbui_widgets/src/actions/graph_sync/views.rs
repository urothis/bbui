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
                        socket_type: socket.socket_type.clone(),
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
                        .map(|socket| socket.socket_type.clone()),
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

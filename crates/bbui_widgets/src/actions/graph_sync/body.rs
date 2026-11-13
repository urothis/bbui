use super::*;

fn place_graph_widget_tree(node: &mut crate::WidgetNode, z: i32) {
    let mut layout = node
        .layout
        .unwrap_or_else(|| crate::declarative::default_widget_layout(node));
    layout.z_index = layout.z_index.saturating_add(z);
    node.layout = Some(layout);
    for child in &mut node.children {
        place_graph_widget_tree(child, z);
    }
}

fn clip_graph_widget_tree(world: &mut World, entity: Entity, clip: Rect) {
    if let Some(mut layout) = world.get_mut::<UiLayoutResult>(entity) {
        let clipped = layout
            .clip
            .unwrap_or(clip)
            .intersection(clip)
            .unwrap_or_default();
        if layout.clip != Some(clipped) {
            layout.clip = Some(clipped);
        }
    }
    if let Some(mut hit_box) = world.get_mut::<UiHitBox>(entity) {
        let clipped = hit_box
            .clip
            .unwrap_or(clip)
            .intersection(clip)
            .unwrap_or_default();
        if hit_box.clip != Some(clipped) {
            hit_box.clip = Some(clipped);
        }
    }
    let children = world
        .get::<UiChildren>(entity)
        .map_or_else(Vec::new, |children| children.0.clone());
    for child in children {
        clip_graph_widget_tree(world, child, clip);
    }
}

/// Copy edits from mounted socket controls into the graph model before declarations
/// are reconciled. Custom editors supply their own event-to-value decoder.
pub fn sync_node_graph_socket_values(world: &mut World) {
    let Some(queue) = world.get_resource::<WidgetEventQueue>() else {
        return;
    };
    let (generation, count) = (queue.generation(), queue.0.len());
    let (start, events) = {
        let state = world.resource::<WidgetNodeGraphBodies>();
        let start = if state.event_generation == generation {
            state.event_count.min(count)
        } else {
            0
        };
        (start, queue.0[start..].to_vec())
    };
    if start == count {
        return;
    }
    for event in &events {
        let target = match event {
            WidgetEvent::ValueChanged { target, .. } | WidgetEvent::TextChanged { target, .. } => {
                *target
            }
            _ => continue,
        };
        let mut current = Some(target);
        let mut binding = None;
        let mut body_owner = None;
        while let Some(entity) = current {
            if let Some(found) = world.get::<NodeGraphSocketEditorBinding>(entity) {
                binding = Some(*found);
                break;
            }
            if let Some(node_entity) = world
                .resource::<WidgetNodeGraphBodies>()
                .bodies
                .iter()
                .find_map(|(node_entity, root)| (*root == entity).then_some(*node_entity))
            {
                body_owner = world.get::<NodeGraphNodeControl>(node_entity).cloned();
                break;
            }
            current = world.get::<UiParent>(entity).map(|parent| parent.0);
        }
        if let Some(control) = body_owner {
            let node = world
                .get::<NodeGraph>(control.owner)
                .and_then(|graph| graph.nodes.iter().find(|node| node.id == control.node_id))
                .cloned();
            if let Some(node) = node {
                let data = world
                    .get_resource::<NodeGraphTypeRegistry>()
                    .and_then(|registry| registry.node(node.type_id.as_deref()?))
                    .and_then(|definition| definition.handle_event(world, event, &node));
                if let Some(data) = data
                    && let Some(mut graph) = world.get_mut::<NodeGraph>(control.owner)
                    && let Some(node) = graph
                        .nodes
                        .iter_mut()
                        .find(|node| node.id == control.node_id)
                {
                    node.data = data;
                }
            }
            continue;
        }
        let Some(binding) = binding else {
            continue;
        };
        let Some(socket_type) = world
            .get::<NodeGraph>(binding.owner)
            .and_then(|graph| graph.nodes.iter().find(|node| node.id == binding.node_id))
            .and_then(|node| {
                node.sockets
                    .iter()
                    .find(|socket| socket.id == binding.socket_id)
            })
            .map(|socket| socket.socket_type.clone())
        else {
            continue;
        };
        let value = world
            .get_resource::<NodeGraphTypeRegistry>()
            .and_then(|registry| registry.editor(&socket_type))
            .and_then(|editor| editor.read(world, event));
        if let Some(value) = value
            && let Some(mut graph) = world.get_mut::<NodeGraph>(binding.owner)
            && let Some(node) = graph
                .nodes
                .iter_mut()
                .find(|node| node.id == binding.node_id)
            && let Some(socket) = node
                .sockets
                .iter_mut()
                .find(|socket| socket.id == binding.socket_id)
            && socket.direction == SocketDirection::Input
        {
            socket.value = Some(value);
        }
    }
    let mut state = world.resource_mut::<WidgetNodeGraphBodies>();
    state.event_generation = generation;
    state.event_count = count;
}

/// Mount registered node bodies as ordinary BBUI widget trees. Each body has its own
/// viewport, so its controls retain normal layout, focus, and input behavior while
/// the graph pans or nodes move.
pub fn sync_node_graph_bodies(world: &mut World) {
    let spacing = world
        .get_resource::<Theme>()
        .cloned()
        .unwrap_or_default()
        .spacing
        .node_graph;
    let mut query = world.query::<(Entity, &NodeGraphNodeControl, &UiLayoutResult)>();
    let declarations = query
        .iter(world)
        .filter_map(|(entity, control, layout)| {
            let graph = world.get::<NodeGraph>(control.owner)?;
            let node = graph.nodes.iter().find(|node| node.id == control.node_id)?;
            if node.collapsed {
                return None;
            }
            let definition = world
                .get_resource::<NodeGraphTypeRegistry>()?
                .node(node.type_id.as_deref()?)?;
            let rows = node
                .sockets
                .iter()
                .filter(|socket| {
                    socket.placement == SocketPlacement::Auto
                        && socket.direction == SocketDirection::Input
                })
                .count()
                .max(
                    node.sockets
                        .iter()
                        .filter(|socket| {
                            socket.placement == SocketPlacement::Auto
                                && socket.direction == SocketDirection::Output
                        })
                        .count(),
                );
            let inset = 10.0 * graph.zoom;
            let top = layout.rect.y
                + (spacing.header_height + rows as f32 * spacing.socket_spacing + 4.0) * graph.zoom;
            let bottom = layout.rect.y + layout.rect.height
                - if node
                    .sockets
                    .iter()
                    .any(|socket| socket.placement != SocketPlacement::Auto)
                {
                    spacing.socket_spacing * graph.zoom
                } else {
                    inset
                };
            let viewport = Rect::new(
                layout.rect.x + inset,
                top,
                (layout.rect.width - 2.0 * inset).max(0.0),
                (bottom - top).max(0.0),
            );
            if viewport.width < 1.0 || viewport.height < 1.0 {
                return None;
            }
            let mut body = definition.build_body(node);
            place_graph_widget_tree(&mut body, layout.z.saturating_add(9));
            Some((
                entity,
                WidgetTree::new(body),
                UiLayoutRoot::new(layout.window, viewport, layout.dpi_scale),
            ))
        })
        .collect::<Vec<_>>();
    let mut mounted = std::mem::take(&mut world.resource_mut::<WidgetNodeGraphBodies>().bodies);
    let live = declarations
        .iter()
        .map(|(entity, _, _)| *entity)
        .collect::<HashSet<_>>();
    for (node_entity, tree, root_layout) in declarations {
        let root = mounted
            .get(&node_entity)
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok());
        let result = if let Some(root) = root {
            tree.reconcile(world, root).or_else(|_| {
                crate::declarative::remove_widget_subtree(world, root);
                tree.spawn(world, None)
            })
        } else {
            tree.spawn(world, None)
        };
        if let Ok(root) = result {
            world
                .entity_mut(root)
                .insert((root_layout, UiFixedLayoutRoot));
            if let Some(clip) = world
                .get::<UiLayoutResult>(node_entity)
                .and_then(|layout| layout.clip)
                && let Some(clip) = clip.intersection(root_layout.viewport)
            {
                clip_graph_widget_tree(world, root, clip);
            }
            mounted.insert(node_entity, root);
        }
    }
    for (node_entity, root) in mounted
        .iter()
        .filter(|(entity, _)| !live.contains(entity))
        .map(|(entity, root)| (*entity, *root))
        .collect::<Vec<_>>()
    {
        if world.get_entity(root).is_ok() {
            crate::declarative::remove_widget_subtree(world, root);
        }
        mounted.remove(&node_entity);
    }
    world.resource_mut::<WidgetNodeGraphBodies>().bodies = mounted;

    let mut query = world.query::<(Entity, &NodeGraphNodeControl, &UiLayoutResult)>();
    let editors = query
        .iter(world)
        .flat_map(|(node_entity, control, node_layout)| {
            let Some(graph) = world.get::<NodeGraph>(control.owner) else {
                return Vec::new();
            };
            let Some(node) = graph
                .nodes
                .iter()
                .find(|node| node.id == control.node_id && !node.collapsed)
            else {
                return Vec::new();
            };
            let Some(owner_layout) = world.get::<UiLayoutResult>(control.owner) else {
                return Vec::new();
            };
            let Some(registry) = world.get_resource::<NodeGraphTypeRegistry>() else {
                return Vec::new();
            };
            node.sockets
                .iter()
                .enumerate()
                .filter_map(|(index, socket)| {
                    if socket.direction != SocketDirection::Input
                        || socket.value.is_none()
                        || graph
                            .links
                            .iter()
                            .any(|link| link.to_node == node.id && link.to_socket_id == socket.id)
                    {
                        return None;
                    }
                    let editor = registry.editor(&socket.socket_type)?;
                    let center = node_graph_socket_position(
                        graph,
                        owner_layout.rect,
                        node,
                        index,
                        &spacing,
                    )?;
                    let inset = 9.0 * graph.zoom;
                    let width = (node_layout.rect.width - 2.0 * inset).max(1.0);
                    let (x, y) = match socket.placement {
                        SocketPlacement::Auto => (
                            node_layout.rect.x + node_layout.rect.width - width - inset,
                            center[1] - 10.0 * graph.zoom,
                        ),
                        SocketPlacement::BottomLeft => {
                            (node_layout.rect.x + inset, center[1] - 35.0 * graph.zoom)
                        }
                        SocketPlacement::BottomRight => (
                            node_layout.rect.x + node_layout.rect.width - width - inset,
                            center[1] - 35.0 * graph.zoom,
                        ),
                    };
                    let viewport = Rect::new(x, y, width, 21.0 * graph.zoom);
                    let mut declaration = editor.build(socket);
                    place_graph_widget_tree(&mut declaration, node_layout.z.saturating_add(10));
                    Some((
                        (node_entity, socket.id),
                        NodeGraphSocketEditorBinding {
                            owner: control.owner,
                            node_id: node.id,
                            socket_id: socket.id,
                        },
                        WidgetTree::new(declaration),
                        UiLayoutRoot::new(node_layout.window, viewport, node_layout.dpi_scale),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let live = editors
        .iter()
        .map(|(key, _, _, _)| *key)
        .collect::<HashSet<_>>();
    let mut mounted = std::mem::take(&mut world.resource_mut::<WidgetNodeGraphBodies>().editors);
    for (key, binding, tree, root_layout) in editors {
        let root = mounted
            .get(&key)
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok());
        let result = if let Some(root) = root {
            tree.reconcile(world, root).or_else(|_| {
                crate::declarative::remove_widget_subtree(world, root);
                tree.spawn(world, None)
            })
        } else {
            tree.spawn(world, None)
        };
        if let Ok(root) = result {
            world
                .entity_mut(root)
                .insert((root_layout, UiFixedLayoutRoot, binding));
            if let Some(clip) = world
                .get::<UiLayoutResult>(key.0)
                .and_then(|layout| layout.clip)
                && let Some(clip) = clip.intersection(root_layout.viewport)
            {
                clip_graph_widget_tree(world, root, clip);
            }
            mounted.insert(key, root);
        }
    }
    for (key, root) in mounted
        .iter()
        .filter(|(key, _)| !live.contains(key))
        .map(|(key, root)| (*key, *root))
        .collect::<Vec<_>>()
    {
        if world.get_entity(root).is_ok() {
            crate::declarative::remove_widget_subtree(world, root);
        }
        mounted.remove(&key);
    }
    world.resource_mut::<WidgetNodeGraphBodies>().editors = mounted;
}

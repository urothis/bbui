use super::*;

#[test]
fn converted_node_links_shade_from_output_to_input_socket_color() {
    let mut registry = NodeGraphTypeRegistry::default();
    registry.allow_conversion(SocketType::Float, SocketType::Color);
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Float",
        [30.0, 40.0],
        vec![NodeSocket::output("Value", SocketType::Float)],
    );
    let target = graph.add_node(
        "Color",
        [300.0, 50.0],
        vec![NodeSocket::input("Value", SocketType::Color)],
    );
    graph
        .create_link_with_registry(&registry, source, 0, target, 0)
        .unwrap();
    let theme = Theme::default();
    let rect = Rect::new(0.0, 0.0, 600.0, 300.0);
    let mut items = Vec::new();
    paint_node_graph(
        NodeGraphPaint {
            graph: &graph,
            state: NodeGraphEditorState::default(),
            output_label_widths: HashMap::new(),
            registry: Some(&registry),
            canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
            label: "Graph",
            rect,
            clip: ClipRect::from(rect),
            z: 1,
            theme: &theme,
            font: ui_default_text_font(theme.font_size),
        },
        &mut items,
    );
    let gradients = items
        .iter()
        .filter_map(|item| match item.primitive {
            Primitive::BezierGradient {
                start_color,
                end_color,
                ..
            } => Some((start_color, end_color)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        gradients,
        vec![(
            socket_type_color(&SocketType::Float, &theme, Some(&registry)),
            socket_type_color(&SocketType::Color, &theme, Some(&registry))
        )]
    );
}

#[test]
fn graph_body_and_socket_editors_write_values_back_to_the_model() {
    let mut world = World::new();
    world.init_resource::<WidgetNodeGraphBodies>();
    world.init_resource::<WidgetEventQueue>();
    let mut registry = NodeGraphTypeRegistry::default();
    registry
        .register_node(
            crate::NodeTypeDefinition::new(
                "test:value",
                1,
                NodeCategory::General,
                vec![
                    NodeSocket::input("Factor", SocketType::Float)
                        .with_id(SocketId(7))
                        .with_value(serde_json::json!(0.5)),
                ],
                |node| {
                    crate::WidgetNode::checkbox(
                        "Clamp",
                        node.data["clamp"].as_bool().unwrap_or(false),
                    )
                },
            )
            .with_event_handler(|_, event, node| {
                let WidgetEvent::ValueChanged {
                    after: WidgetValue::Bool(value),
                    ..
                } = event
                else {
                    return None;
                };
                let mut data = node.data.clone();
                data["clamp"] = (*value).into();
                Some(data)
            }),
        )
        .unwrap();
    let mut graph = NodeGraph::default();
    let node_id = graph
        .add_defined_node(
            &registry,
            "test:value",
            "Value",
            [0.0, 0.0],
            serde_json::json!({"clamp": true}),
        )
        .unwrap();
    let owner = world.spawn(graph).id();
    let node_entity = world
        .spawn(NodeGraphNodeControl {
            owner,
            node_id,
            label: "Value".into(),
            selected: false,
            collapsed: false,
        })
        .id();
    let body_root = world.spawn(UiNode).id();
    world
        .resource_mut::<WidgetNodeGraphBodies>()
        .bodies
        .insert(node_entity, body_root);
    world.insert_resource(registry);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::ValueChanged {
            target: body_root,
            before: WidgetValue::Bool(true),
            after: WidgetValue::Bool(false),
        });
    sync_node_graph_socket_values(&mut world);
    assert_eq!(
        world.get::<NodeGraph>(owner).unwrap().nodes[0].data["clamp"],
        false
    );

    let editor = world
        .spawn(NodeGraphSocketEditorBinding {
            owner,
            node_id,
            socket_id: SocketId(7),
        })
        .id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::ValueChanged {
            target: editor,
            before: WidgetValue::Number(0.5),
            after: WidgetValue::Number(0.75),
        });
    sync_node_graph_socket_values(&mut world);
    assert_eq!(
        world.get::<NodeGraph>(owner).unwrap().nodes[0].sockets[0].value,
        Some(serde_json::json!(0.75))
    );
}

#[test]
pub(super) fn bottom_node_sockets_have_distinct_positions_and_vertical_link_tangents() {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Bottom sockets",
        [100.0, 50.0],
        vec![
            NodeSocket::input("Bottom in", SocketType::Color).bottom_left(),
            NodeSocket::output("Bottom out", SocketType::Color).bottom_right(),
        ],
    );
    let target = graph.add_node(
        "Target",
        [420.0, 180.0],
        vec![NodeSocket::input("In", SocketType::Color)],
    );
    let upstream = graph.add_node(
        "Upstream",
        [0.0, 180.0],
        vec![NodeSocket::output("Out", SocketType::Color)],
    );
    let link_id = graph.create_link(source, 1, target, 0).unwrap();
    let incoming_id = graph.create_link(upstream, 0, source, 0).unwrap();
    let link = *graph.links.iter().find(|link| link.id == link_id).unwrap();
    let incoming = *graph
        .links
        .iter()
        .find(|link| link.id == incoming_id)
        .unwrap();
    let spacing = Theme::default().spacing.node_graph;
    let rect = UiRect::new(0.0, 0.0, 800.0, 600.0);
    let source_node = graph.nodes.iter().find(|node| node.id == source).unwrap();
    let source_rect = node_graph_node_rect(&graph, rect, source_node, &spacing);
    let input = node_graph_socket_position(&graph, rect, source_node, 0, &spacing).unwrap();
    let output = node_graph_socket_position(&graph, rect, source_node, 1, &spacing).unwrap();
    assert!((input[0] - (source_rect.x + source_rect.width * 0.25)).abs() < 0.01);
    assert!((output[0] - (source_rect.x + source_rect.width * 0.75)).abs() < 0.01);
    assert_eq!(input[1], source_rect.y + source_rect.height);
    assert_eq!(output[1], input[1]);
    assert!(
        matches!(node_graph_hit(&graph, rect, input, &spacing), NodeGraphHit::Socket { node, socket: 0, .. } if node == source)
    );
    let points = node_graph_link_points(&graph, rect, link, &spacing).unwrap();
    assert_eq!(points[0], output);
    assert_eq!(points[1][0], output[0]);
    assert!(points[1][1] > output[1]);
    assert_eq!(points[2][1], points[3][1]);
    assert!(points[2][0] < points[3][0]);
    let incoming_points = node_graph_link_points(&graph, rect, incoming, &spacing).unwrap();
    assert_eq!(incoming_points[3], input);
    assert_eq!(incoming_points[2][0], input[0]);
    assert!(incoming_points[2][1] > input[1]);

    let mut cached = HashMap::new();
    cache_node_graph_socket_positions(graph.zoom, source_node, source_rect, &spacing, &mut cached);
    assert_eq!(cached[&(source, 0)], input);
    assert_eq!(cached[&(source, 1)], output);
}

#[test]
pub(super) fn backward_node_graph_link_leaves_output_and_enters_input_outward() {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [300.0, 0.0],
        vec![NodeSocket::output("Exit", SocketType::Any)],
    );
    let target = graph.add_node(
        "Target",
        [0.0, 100.0],
        vec![NodeSocket::input("Enter", SocketType::Any)],
    );
    let link_id = graph.create_link(source, 0, target, 0).unwrap();
    let link = *graph.links.iter().find(|link| link.id == link_id).unwrap();
    let points = node_graph_link_points(
        &graph,
        UiRect::new(0.0, 0.0, 800.0, 600.0),
        link,
        &Theme::default().spacing.node_graph,
    )
    .unwrap();

    assert!(points[3][0] < points[0][0]);
    assert!(
        points[1][0] > points[0][0],
        "output tangent must point away from its node"
    );
    assert!(
        points[2][0] < points[3][0],
        "input tangent must point away from its node"
    );
    assert!(bezier_point(points, 0.1)[0] > points[0][0]);
}

#[test]
pub(super) fn virtual_table_and_graph_sync_skip_idle_component_changes() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetPlugin))
        .init_resource::<VirtualSyncChanges>()
        .add_systems(Update, count_virtual_sync_changes.in_set(UiSystems::Render));
    let window = app.world_mut().spawn_empty().id();
    let table = WidgetTree::new(WidgetNode::data_table("Objects", sample_table(100)))
        .spawn(app.world_mut(), None)
        .unwrap();
    let table_rect = UiRect::new(0.0, 0.0, 260.0, 150.0);
    app.world_mut().entity_mut(table).insert(UiLayoutResult {
        window,
        rect: table_rect,
        clip: None,
        z: 4,
        content_size: [table_rect.width, table_rect.height],
        dpi_scale: 1.0,
    });

    let mut graph = NodeGraph::default();
    let first = graph.add_node(
        "First",
        [40.0, 40.0],
        vec![NodeSocket::output("out", crate::SocketType::Float)],
    );
    graph.add_node(
        "Second",
        [240.0, 80.0],
        vec![NodeSocket::input("in", crate::SocketType::Float)],
    );
    let graph_owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
        .spawn(app.world_mut(), None)
        .unwrap();
    let graph_rect = UiRect::new(300.0, 0.0, 640.0, 440.0);
    app.world_mut()
        .entity_mut(graph_owner)
        .insert(UiLayoutResult {
            window,
            rect: graph_rect,
            clip: None,
            z: 8,
            content_size: [graph_rect.width, graph_rect.height],
            dpi_scale: 1.0,
        });

    app.update();
    assert!(app.world().resource::<VirtualSyncChanges>().table_cells > 0);
    assert!(app.world().resource::<VirtualSyncChanges>().graph_nodes > 0);

    app.update();
    assert_eq!(
        *app.world().resource::<VirtualSyncChanges>(),
        VirtualSyncChanges::default(),
        "an idle view refresh keeps all visible model components and geometry unchanged"
    );

    app.world_mut().get_mut::<DataTable>(table).unwrap().rows[0].cells[0] =
        "Updated visible cell".into();
    app.world_mut()
        .get_mut::<NodeGraph>(graph_owner)
        .unwrap()
        .nodes
        .iter_mut()
        .find(|node| node.id == first)
        .unwrap()
        .title = "Updated visible node".into();
    app.update();
    let changes = app.world().resource::<VirtualSyncChanges>();
    assert_eq!(changes.table_cells, 1);
    assert_eq!(changes.graph_nodes, 1);
    assert_eq!(changes.table_headers, 0);
    assert_eq!(changes.graph_sockets, 0);
    assert_eq!(changes.graph_links, 0);
    assert_eq!(changes.layout_results, 0);
    assert_eq!(changes.hit_boxes, 0);
}

#[test]
pub(super) fn node_graph_moves_and_direct_edits_repaint_only_changed_node_and_links() {
    fn graph_fragments(
        app: &App,
        owner: Entity,
    ) -> HashMap<NodeGraphItemKey, bbui_render::WidgetRenderFragment> {
        let entities = &app.world().resource::<WidgetNodeGraphItems>().items[&owner];
        let fragments = app.world().resource::<DisplayList>().widget_fragments();
        entities
            .iter()
            .filter_map(|(key, entity)| {
                fragments
                    .get(entity)
                    .cloned()
                    .map(|fragment| (*key, fragment))
            })
            .collect()
    }

    fn changed_keys(
        before: &HashMap<NodeGraphItemKey, bbui_render::WidgetRenderFragment>,
        after: &HashMap<NodeGraphItemKey, bbui_render::WidgetRenderFragment>,
    ) -> HashSet<NodeGraphItemKey> {
        before
            .keys()
            .chain(after.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .filter(|key| before.get(key) != after.get(key))
            .collect()
    }

    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut graph = NodeGraph::default();
    let moved = graph.add_node(
        "Moved",
        [40.0, 40.0],
        vec![NodeSocket::output("out", crate::SocketType::Float)],
    );
    let first_sink = graph.add_node(
        "First sink",
        [300.0, 30.0],
        vec![
            NodeSocket::input("in", crate::SocketType::Float),
            NodeSocket::output("out", crate::SocketType::Float),
        ],
    );
    let second_sink = graph.add_node(
        "Second sink",
        [300.0, 180.0],
        vec![NodeSocket::input("in", crate::SocketType::Float)],
    );
    let distant_sink = graph.add_node(
        "Distant sink",
        [560.0, 30.0],
        vec![NodeSocket::input("in", crate::SocketType::Float)],
    );
    let first_link = graph.create_link(moved, 0, first_sink, 0).unwrap();
    let second_link = graph.create_link(moved, 0, second_sink, 0).unwrap();
    let distant_link = graph.create_link(first_sink, 1, distant_sink, 0).unwrap();
    graph.selected.insert(moved);
    let owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(20.0, 30.0, 720.0, 500.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: Some(UiRect::new(26.0, 34.0, 700.0, 480.0)),
        z: 8,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.5,
    });
    app.update();

    let initial = graph_fragments(&app, owner);
    let spacing = app.world().resource::<Theme>().spacing.node_graph;
    let graph_model = app.world().get::<NodeGraph>(owner).unwrap();
    let moved_node = graph_model
        .nodes
        .iter()
        .find(|node| node.id == moved)
        .unwrap();
    let moved_rect = node_graph_node_rect(graph_model, rect, moved_node, &spacing);
    let start = [moved_rect.x + 48.0, moved_rect.y + 10.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerDown {
        window,
        target: owner,
        button: UiPointerButton::Primary,
        position: start,
    }];
    app.update();

    let root_before = app
        .world()
        .resource::<DisplayList>()
        .widget_fragments()
        .get(&owner)
        .cloned();
    let before_gesture = graph_fragments(&app, owner);
    let end = [start[0] + 26.0, start[1] + 18.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerMoved {
        window,
        target: owner,
        position: end,
        delta: [26.0, 18.0],
        dragging: true,
    }];
    app.update();
    let after_gesture = graph_fragments(&app, owner);
    assert_eq!(
        changed_keys(&before_gesture, &after_gesture),
        HashSet::from([
            NodeGraphItemKey::Node(moved),
            NodeGraphItemKey::Link(first_link),
            NodeGraphItemKey::Link(second_link),
        ]),
        "a routed node drag republishes that node and its incident links"
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&owner),
        root_before.as_ref(),
        "moving a node leaves graph chrome unchanged"
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        3
    );

    let before_tracked = graph_fragments(&app, owner);
    assert!(move_node_graph_node(
        app.world_mut(),
        owner,
        distant_sink,
        [574.0, 30.0],
    ));
    app.update();
    let after_tracked = graph_fragments(&app, owner);
    assert_eq!(
        changed_keys(&before_tracked, &after_tracked),
        HashSet::from([
            NodeGraphItemKey::Node(distant_sink),
            NodeGraphItemKey::Link(distant_link),
        ]),
        "a tracked node edit republishes only that node and its incident link"
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        2
    );

    let before_direct = graph_fragments(&app, owner);
    app.world_mut()
        .get_mut::<NodeGraph>(owner)
        .unwrap()
        .nodes
        .iter_mut()
        .find(|node| node.id == distant_sink)
        .unwrap()
        .position[0] += 14.0;
    app.update();
    let after_direct = graph_fragments(&app, owner);
    assert_eq!(
        changed_keys(&before_direct, &after_direct),
        HashSet::from([
            NodeGraphItemKey::Node(distant_sink),
            NodeGraphItemKey::Link(distant_link),
        ]),
        "a direct public ECS edit is diffed to the changed node and its adjacency"
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&owner),
        root_before.as_ref()
    );

    let before_mixed = graph_fragments(&app, owner);
    app.world_mut()
        .get_mut::<NodeGraph>(owner)
        .unwrap()
        .nodes
        .iter_mut()
        .find(|node| node.id == second_sink)
        .unwrap()
        .position[1] += 12.0;
    let next = [end[0] + 12.0, end[1]];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerMoved {
        window,
        target: owner,
        position: next,
        delta: [12.0, 0.0],
        dragging: true,
    }];
    app.update();
    let after_mixed = graph_fragments(&app, owner);
    assert_eq!(
        changed_keys(&before_mixed, &after_mixed),
        HashSet::from([
            NodeGraphItemKey::Node(moved),
            NodeGraphItemKey::Node(second_sink),
            NodeGraphItemKey::Link(first_link),
            NodeGraphItemKey::Link(second_link),
        ]),
        "a public edit between journaled drag updates is included by signature diff"
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&owner),
        root_before.as_ref()
    );
    assert_eq!(initial.len(), after_mixed.len());
    assert!(initial.contains_key(&NodeGraphItemKey::Node(first_sink)));
    assert!(initial.contains_key(&NodeGraphItemKey::Link(distant_link)));

    app.world_mut()
        .get_mut::<NodeGraph>(owner)
        .unwrap()
        .nodes
        .swap(0, 1);
    app.update();
    let reordered = graph_fragments(&app, owner);
    assert!(
        reordered[&NodeGraphItemKey::Node(first_sink)].order
            < reordered[&NodeGraphItemKey::Node(moved)].order,
        "a model node reorder is reflected in stable virtual fragment rank"
    );
}

#[test]
pub(super) fn graph_transform_node_move_and_resize_refresh_nodes_and_links() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [40.0, 40.0],
        vec![NodeSocket::output("out", crate::SocketType::Float)],
    );
    let sink = graph.add_node(
        "Sink",
        [300.0, 40.0],
        vec![NodeSocket::input("in", crate::SocketType::Float)],
    );
    let link = graph.create_link(source, 0, sink, 0).unwrap();
    let owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(20.0, 30.0, 800.0, 500.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: Some(rect),
        z: 8,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    assert!(move_node_graph_node(
        app.world_mut(),
        owner,
        source,
        [70.0, 65.0],
    ));
    {
        let mut graph = app.world_mut().get_mut::<NodeGraph>(owner).unwrap();
        graph.pan = [60.0, 25.0];
        graph.zoom = 1.2;
    }
    app.update();

    let graph = app.world().get::<NodeGraph>(owner).unwrap();
    let sink_node = graph.nodes.iter().find(|node| node.id == sink).unwrap();
    let expected = node_graph_node_rect(
        graph,
        rect,
        sink_node,
        &app.world().resource::<Theme>().spacing.node_graph,
    );
    let entity =
        app.world().resource::<WidgetNodeGraphItems>().items[&owner][&NodeGraphItemKey::Node(sink)];
    assert_eq!(
        app.world().get::<UiLayoutResult>(entity).unwrap().rect,
        expected
    );

    assert!(move_node_graph_node(
        app.world_mut(),
        owner,
        source,
        [90.0, 75.0],
    ));
    let resized = UiRect::new(42.0, 58.0, 620.0, 430.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: resized,
        clip: Some(resized),
        z: 8,
        content_size: [resized.width, resized.height],
        dpi_scale: 1.0,
    });
    app.update();
    let graph = app.world().get::<NodeGraph>(owner).unwrap();
    let spacing = &app.world().resource::<Theme>().spacing.node_graph;
    for node in &graph.nodes {
        let entity = app.world().resource::<WidgetNodeGraphItems>().items[&owner]
            [&NodeGraphItemKey::Node(node.id)];
        assert_eq!(
            app.world().get::<UiLayoutResult>(entity).unwrap().rect,
            node_graph_node_rect(graph, resized, node, spacing),
        );
    }
    let link_entity =
        app.world().resource::<WidgetNodeGraphItems>().items[&owner][&NodeGraphItemKey::Link(link)];
    let link_fragment = &app.world().resource::<DisplayList>().widget_fragments()[&link_entity];
    let actual_points = link_fragment.items.iter().find_map(|item| {
        if let bbui_render::Primitive::Bezier { points, .. } = item.primitive {
            Some(points)
        } else {
            None
        }
    });
    let expected_points = node_graph_link_points(graph, resized, graph.links[0], spacing);
    assert_eq!(actual_points, expected_points);
}

#[test]
pub(super) fn tracked_graph_pan_culls_and_enters_nodes_and_raw_edits_reconcile() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut graph = NodeGraph::default();
    let first = graph.add_node("First", [30.0, 30.0], vec![]);
    let second = graph.add_node("Second", [580.0, 30.0], vec![]);
    let owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 320.0, 240.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: Some(rect),
        z: 3,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    sync_node_graph_views(app.world_mut());
    assert!(
        app.world().resource::<WidgetNodeGraphItems>().items[&owner]
            .contains_key(&NodeGraphItemKey::Node(first))
    );
    assert!(
        !app.world().resource::<WidgetNodeGraphItems>().items[&owner]
            .contains_key(&NodeGraphItemKey::Node(second))
    );

    assert!(pan_node_graph(app.world_mut(), owner, [-450.0, 0.0]));
    sync_node_graph_views(app.world_mut());
    assert!(
        !app.world().resource::<WidgetNodeGraphItems>().items[&owner]
            .contains_key(&NodeGraphItemKey::Node(first))
    );
    assert!(
        app.world().resource::<WidgetNodeGraphItems>().items[&owner]
            .contains_key(&NodeGraphItemKey::Node(second))
    );

    app.world_mut()
        .get_mut::<NodeGraph>(owner)
        .unwrap()
        .nodes
        .iter_mut()
        .find(|node| node.id == second)
        .unwrap()
        .title = "Edited".into();
    assert!(pan_node_graph(app.world_mut(), owner, [-440.0, 0.0]));
    sync_node_graph_views(app.world_mut());
    let entity = app.world().resource::<WidgetNodeGraphItems>().items[&owner]
        [&NodeGraphItemKey::Node(second)];
    assert_eq!(
        app.world()
            .get::<NodeGraphNodeControl>(entity)
            .unwrap()
            .label,
        "Edited"
    );

    assert!(move_node_graph_node(
        app.world_mut(),
        owner,
        first,
        [540.0, 30.0]
    ));
    sync_node_graph_views(app.world_mut());
    assert!(pan_node_graph(app.world_mut(), owner, [-430.0, 0.0]));
    sync_node_graph_views(app.world_mut());
    assert!(
        app.world().resource::<WidgetNodeGraphItems>().items[&owner]
            .contains_key(&NodeGraphItemKey::Node(first))
    );
}

use super::*;

#[test]
pub(super) fn node_graph_connects_reconnects_cuts_links_and_undoes_each_gesture() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut graph = NodeGraph::default();
    let source_a = graph.add_node(
        "Source A",
        [40.0, 40.0],
        vec![NodeSocket::output("value", crate::SocketType::Float)],
    );
    let source_b = graph.add_node(
        "Source B",
        [40.0, 220.0],
        vec![NodeSocket::output("value", crate::SocketType::Float)],
    );
    let sink = graph.add_node(
        "Sink",
        [360.0, 120.0],
        vec![NodeSocket::input("value", crate::SocketType::Float)],
    );
    let owner = WidgetTree::new(WidgetNode::node_graph("Shader", graph))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(20.0, 30.0, 640.0, 440.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 8,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    let spacing = app.world().resource::<Theme>().spacing.node_graph;
    let socket_point = |node_id, socket_index| {
        let graph = app.world().get::<NodeGraph>(owner).unwrap();
        let node = graph.nodes.iter().find(|node| node.id == node_id).unwrap();
        node_graph_socket_position(graph, rect, node, socket_index, &spacing).unwrap()
    };
    let from_a = socket_point(source_a, 0);
    let from_b = socket_point(source_b, 0);
    let to_sink = socket_point(sink, 0);
    let before_history = app.world().resource::<bbui_core::UiHistory>().undo_len();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: from_a,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position: to_sink,
            delta: [to_sink[0] - from_a[0], to_sink[1] - from_a[1]],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: to_sink,
            cancelled: false,
        },
    ];
    app.update();
    let link = *app
        .world()
        .get::<NodeGraph>(owner)
        .unwrap()
        .links
        .first()
        .unwrap();
    assert_eq!((link.from_node, link.to_node), (source_a, sink));
    assert_eq!(
        app.world().resource::<bbui_core::UiHistory>().undo_len(),
        before_history + 1,
        "connection drag is one history transaction"
    );
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::NodeGraphCommitted { target, .. } if *target == owner)
    ));
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Bezier { .. }))
    );
    let link_control = app
        .world()
        .iter_entities()
        .find(|entity| {
            entity
                .get::<NodeGraphLinkControl>()
                .is_some_and(|control| control.owner == owner)
        })
        .expect("visible connection has an accessible virtual control");
    let stable_link_id = link_control.get::<NodeGraphLinkControl>().unwrap().link_id;

    let undo = |app: &mut App, redo: bool| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: if redo { UiKeyCode::Y } else { UiKeyCode::Z },
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
    };
    undo(&mut app, false);
    assert!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .links
            .is_empty()
    );
    undo(&mut app, true);
    let link = app.world().get::<NodeGraph>(owner).unwrap().links[0];
    assert_eq!(link.id, stable_link_id);
    assert!(app.world().iter_entities().any(|entity| {
        entity
            .get::<NodeGraphLinkControl>()
            .is_some_and(|control| control.owner == owner && control.link_id == stable_link_id)
    }));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: from_a,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: from_b,
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(
        app.world().get::<NodeGraph>(owner).unwrap().links[0].from_node,
        source_b,
        "dragging an existing output endpoint reconnects the link"
    );

    let graph = app.world().get::<NodeGraph>(owner).unwrap().clone();
    let points = node_graph_link_points(&graph, rect, graph.links[0], &spacing).unwrap();
    let middle = bezier_point(points, 0.5);
    let cut_end = [middle[0], middle[1] + 18.0];
    app.world_mut()
        .resource_mut::<WidgetInteractionState>()
        .modifiers
        .insert(
            window,
            UiModifiers {
                alt: true,
                ..UiModifiers::default()
            },
        );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: middle,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position: cut_end,
            delta: [0.0, 18.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: cut_end,
            cancelled: false,
        },
    ];
    app.update();
    assert!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .links
            .is_empty()
    );
    app.world_mut()
        .resource_mut::<WidgetInteractionState>()
        .modifiers
        .remove(&window);
    undo(&mut app, false);
    assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().links.len(), 1);
}

#[test]
pub(super) fn node_graph_routes_selection_move_collapse_box_pan_zoom_add_and_delete() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut graph = NodeGraph::default();
    let first = graph.add_node(
        "First",
        [40.0, 40.0],
        vec![NodeSocket::output("out", crate::SocketType::Float)],
    );
    let second = graph.add_node(
        "Second",
        [330.0, 80.0],
        vec![NodeSocket::input("in", crate::SocketType::Float)],
    );
    let owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 600.0, 420.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 5,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();
    let spacing = app.world().resource::<Theme>().spacing.node_graph;
    let first_header = [
        rect.x + 40.0 + spacing.title_padding_x + 10.0,
        rect.y + spacing.header_height + 40.0 + spacing.header_height * 0.5,
    ];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: first_header,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: first_header,
            cancelled: false,
        },
    ];
    app.update();
    assert!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .selected
            .contains(&first)
    );

    let key = |key, modifiers| UiRoutedEvent::Key {
        window,
        target: owner,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::ArrowRight, UiModifiers::default())];
    app.update();
    assert_eq!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.id == first)
            .unwrap()
            .position[0],
        50.0
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::C, UiModifiers::default())];
    app.update();
    assert!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.id == first)
            .unwrap()
            .collapsed
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: [320.0, 100.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position: [420.0, 170.0],
            delta: [100.0, 70.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: [420.0, 170.0],
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(
        app.world().get::<NodeGraph>(owner).unwrap().selected,
        [second].into(),
        "dragging an empty-area marquee selects intersecting nodes"
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    )];
    app.update();
    assert_eq!(
        app.world().get::<NodeGraph>(owner).unwrap().selected,
        [first, second].into()
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::N, UiModifiers::default())];
    app.update();
    assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().nodes.len(), 3);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    )];
    app.update();
    assert_eq!(
        app.world().get::<NodeGraph>(owner).unwrap().selected.len(),
        3
    );

    let start_pan = app.world().get::<NodeGraph>(owner).unwrap().pan;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Middle,
            position: [570.0, 390.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position: [590.0, 400.0],
            delta: [20.0, 10.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Middle,
            position: [590.0, 400.0],
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(
        app.world().get::<NodeGraph>(owner).unwrap().pan,
        [start_pan[0] + 20.0, start_pan[1] + 10.0]
    );
    let zoom = app.world().get::<NodeGraph>(owner).unwrap().zoom;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [260.0, 220.0],
        delta: [0.0, 1.0],
        unit: bbui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert!(app.world().get::<NodeGraph>(owner).unwrap().zoom > zoom);
    let zoomed_by_wheel = app.world().get::<NodeGraph>(owner).unwrap().zoom;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::ArrowUp,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    )];
    app.update();
    assert!(app.world().get::<NodeGraph>(owner).unwrap().zoom > zoomed_by_wheel);
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::Home, UiModifiers::default())];
    app.update();
    assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().zoom, 1.0);
    assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().pan, [0.0, 0.0]);

    let graph = app.world().get::<NodeGraph>(owner).unwrap().clone();
    let node = graph.nodes.iter().find(|node| node.id == first).unwrap();
    let spacing = app.world().resource::<Theme>().spacing.node_graph;
    let position = node_graph_node_rect(&graph, rect, node, &spacing);
    let before_cancel = graph
        .nodes
        .iter()
        .find(|node| node.id == first)
        .unwrap()
        .position;
    let drag_start = [position.x + 60.0, position.y + 10.0];
    let drag_end = [drag_start[0] + 40.0, drag_start[1] + 20.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: drag_start,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position: drag_end,
            delta: [40.0, 20.0],
            dragging: true,
        },
        UiRoutedEvent::PointerCancel {
            window,
            target: owner,
        },
    ];
    app.update();
    assert_eq!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.id == first)
            .unwrap()
            .position,
        before_cancel
    );

    let before_escape = app.world().get::<NodeGraph>(owner).unwrap().clone();
    let node = before_escape
        .nodes
        .iter()
        .find(|node| node.id == first)
        .unwrap();
    let node_rect = node_graph_node_rect(&before_escape, rect, node, &spacing);
    let start = [node_rect.x + 60.0, node_rect.y + 12.0];
    let moved = [start[0] + 24.0, start[1] + 16.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position: start,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position: moved,
            delta: [24.0, 16.0],
            dragging: true,
        },
        key(UiKeyCode::Escape, UiModifiers::default()),
    ];
    app.update();
    assert_eq!(
        app.world().get::<NodeGraph>(owner).unwrap(),
        &before_escape,
        "Escape restores the graph snapshot from the active gesture"
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::Delete, UiModifiers::default())];
    app.update();
    assert!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .nodes
            .is_empty()
    );
}

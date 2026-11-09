use super::*;

#[test]
pub(super) fn data_table_header_sorts_and_resize_gesture_is_one_undo_transaction() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut table = sample_table(12);
    table.rows.reverse();
    let owner = WidgetTree::new(WidgetNode::data_table("Objects", table))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(10.0, 20.0, 240.0, 150.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: Some(UiRect::new(20.0, 20.0, 220.0, 140.0)),
        z: 8,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();
    let headers = data_table_headers(app.world_mut(), owner);
    let (header_entity, header) = headers
        .iter()
        .find(|(_, header)| header.column == 0)
        .unwrap();
    let header_rect = app.world().get::<UiHitBox>(*header_entity).unwrap().rect;
    assert_eq!(header.width, 104.0);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: *header_entity,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [header_rect.x + 8.0, header_rect.y + 6.0],
    }];
    app.update();
    let table = app.world().get::<DataTable>(owner).unwrap();
    assert_eq!(table.sort, Some((0, true)));
    assert_eq!(table.rows[0].id, 1);

    let before_undo_count = app.world().resource::<bui_core::UiHistory>().undo_len();
    let hit_x = header_rect.x + header_rect.width - 1.0;
    let y = header_rect.y + 8.0;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: *header_entity,
            button: UiPointerButton::Primary,
            position: [hit_x, y],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: *header_entity,
            position: [hit_x + 30.0, y],
            delta: [30.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: *header_entity,
            button: UiPointerButton::Primary,
            position: [hit_x + 30.0, y],
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(
        app.world().get::<DataTable>(owner).unwrap().columns[0].width,
        134.0
    );
    assert_eq!(
        app.world().resource::<bui_core::UiHistory>().undo_len(),
        before_undo_count + 1,
        "the entire drag should commit as one undo step"
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::Z,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert_eq!(
        app.world().get::<DataTable>(owner).unwrap().columns[0].width,
        104.0
    );
}

#[test]
pub(super) fn data_table_painter_emits_header_cell_text_and_selection_gpu_primitives() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut table = sample_table(3);
    table.select_cell(2, 1, false, false);
    let owner = WidgetTree::new(WidgetNode::data_table("Inventory", table))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(15.0, 20.0, 250.0, 144.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: Some(UiRect::new(20.0, 20.0, 220.0, 110.0)),
        z: 12,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();
    let items = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert!(items.len() >= 20);
    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, .. } if text.contains("Inventory")
    )));
    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, .. } if text == "Name" || text == "Type"
    )));
    let first_cell_text = items
        .iter()
        .find(|item| {
            matches!(
                &item.primitive,
                Primitive::TextWrapped { text, .. } if text == "Object 001"
            )
        })
        .unwrap();
    assert!(first_cell_text.clip.width <= 104.0);
    assert!(items.iter().all(|item| item.z >= 12));
    assert!(items.iter().all(|item| item.clip.x >= 20.0));
    assert!(
        items
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Line { .. }))
    );
}

#[test]
pub(super) fn data_table_mounts_columns_after_a_fully_scrolled_off_column() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut table = sample_table(4);
    table.columns = vec![
        crate::TableColumn::new("Hidden", 160.0),
        crate::TableColumn::new("Visible", 160.0),
        crate::TableColumn::new("Later", 160.0),
    ];
    for row in &mut table.rows {
        row.cells.push("Value 2".to_owned());
    }
    table.scroll_x = 170.0;
    let owner = WidgetTree::new(WidgetNode::data_table("Objects", table))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 250.0, 180.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 4,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });

    sync_data_table_views(app.world_mut());

    let headers = data_table_headers(app.world_mut(), owner);
    assert!(!headers.iter().any(|(_, header)| header.column == 0));
    assert!(headers.iter().any(|(_, header)| header.column == 1));
    assert!(headers.iter().any(|(_, header)| header.column == 2));
    let cells = data_table_cells(app.world_mut(), owner);
    assert!(!cells.is_empty());
    assert!(cells.iter().all(|(_, cell)| cell.column > 0));
}

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
    let before_history = app.world().resource::<bui_core::UiHistory>().undo_len();
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
        app.world().resource::<bui_core::UiHistory>().undo_len(),
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
        unit: bui_core::UiWheelUnit::Lines,
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

#[test]
pub(super) fn file_navigate_to_updates_directory_reports_errors_and_resets_scroll() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "bui-widgets-navigate-to-{}-{unique}",
        std::process::id()
    ));
    let nested = root.join("Nested");
    std::fs::create_dir_all(&nested).unwrap();
    let browser = FileBrowser::new(&root).unwrap();
    let owner = app.world_mut().spawn(browser).id();
    app.world_mut()
        .entity_mut(owner)
        .insert(FileBrowserViewState {
            scroll_rows: 17,
            ..FileBrowserViewState::default()
        });

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileNavigateTo(nested.clone()),
        ));
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().directory,
        std::fs::canonicalize(&nested).unwrap()
    );
    assert_eq!(
        app.world()
            .get::<FileBrowserViewState>(owner)
            .unwrap()
            .scroll_rows,
        0
    );
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::FileDirectoryChanged { target, .. } if *target == owner)
        ));

    app.world_mut()
        .get_mut::<FileBrowserViewState>(owner)
        .unwrap()
        .scroll_rows = 11;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileNavigateTo(root.join("missing")),
        ));
    app.update();
    assert_eq!(
        app.world()
            .get::<FileBrowserViewState>(owner)
            .unwrap()
            .scroll_rows,
        0
    );
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::FileOperationFailed { target, .. } if *target == owner)
    ));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn file_browser_grid_keyboard_selection_and_bookmarks_route_end_to_end() {
    let root = std::env::temp_dir().join(format!(
        "bui-widgets-browser-routes-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    for index in 0..10 {
        std::fs::write(root.join(format!("entry-{index:02}.txt")), "item").unwrap();
    }

    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut browser = FileBrowser::new(&root).unwrap();
    browser.view = FileView::Grid;
    let owner = app.world_mut().spawn(browser).id();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 250.0, 130.0),
        clip: None,
        z: 2,
        content_size: [250.0, 130.0],
        dpi_scale: 1.0,
    });
    app.world_mut()
        .get_mut::<FileBrowser>(owner)
        .unwrap()
        .select(0, false, false);

    let send_key = |app: &mut App, key, modifiers| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        app.update();
    };
    let selected_index = |app: &App| {
        let browser = app.world().get::<FileBrowser>(owner).unwrap();
        browser
            .visible_range(0, browser.visible_count())
            .iter()
            .position(|entry| Some(&entry.path) == browser.cursor.as_ref())
            .unwrap()
    };

    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(selected_index(&app), 2, "grid down advances one column");
    send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
    assert_eq!(selected_index(&app), 3, "grid right advances one cell");
    send_key(&mut app, UiKeyCode::ArrowLeft, UiModifiers::default());
    assert_eq!(selected_index(&app), 2, "grid left reverses one cell");
    send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        5,
        "page down advances a visible grid row"
    );
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        3,
        "page up reverses the visible-row stride"
    );

    app.world_mut().get_mut::<FileBrowser>(owner).unwrap().view = FileView::List;
    app.world_mut()
        .get_mut::<FileBrowser>(owner)
        .unwrap()
        .select(0, false, false);
    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(selected_index(&app), 1, "list down advances one row");
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        4,
        "list page down advances visible rows"
    );
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        1,
        "list page up reverses the row stride"
    );

    send_key(
        &mut app,
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .selected
            .len(),
        10
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileSelectionChanged { target, selected } if *target == owner && selected.len() == 10)));

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileBookmarkCurrent,
        ));
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().bookmarks,
        vec![std::fs::canonicalize(&root).unwrap()]
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { target, bookmarks } if *target == owner && bookmarks.len() == 1)));

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileRemoveBookmarkCurrent,
        ));
    app.update();
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .bookmarks
            .is_empty()
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { target, bookmarks } if *target == owner && bookmarks.is_empty())));

    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .cursor
            .as_ref()
            .and_then(|path| path.file_name())
            .unwrap(),
        "entry-00.txt"
    );
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .cursor
            .as_ref()
            .and_then(|path| path.file_name())
            .unwrap(),
        "entry-09.txt"
    );
    send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileActivated { target, path } if *target == owner && path.ends_with("entry-09.txt"))));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "entry".to_owned(),
    }];
    app.update();
    send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().filter(),
        "entr"
    );
    send_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .filter()
            .is_empty()
    );
    assert!(
        !app.world()
            .resource::<WidgetFileBrowserFilterEdits>()
            .active
            .contains_key(&owner),
        "Escape must close the filter draft before the next key is routed"
    );
    send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().directory,
        std::fs::canonicalize(root.parent().unwrap()).unwrap(),
        "Backspace leaves an empty filter by navigating to the parent directory"
    );

    std::fs::remove_dir_all(root).unwrap();
}

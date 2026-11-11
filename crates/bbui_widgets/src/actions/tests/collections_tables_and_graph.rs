use super::*;

#[test]
pub(super) fn collection_keyboard_rename_and_history_restore_serialized_model() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let model = ListModel {
        items: vec![
            ListItem {
                id: 10,
                value: "Oak".to_owned(),
                enabled: true,
            },
            ListItem {
                id: 20,
                value: "Pine".to_owned(),
                enabled: true,
            },
        ],
        ..ListModel::default()
    };
    let owner = WidgetTree::new(WidgetNode::list("Trees", model))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 220.0, 120.0),
        clip: None,
        z: 0,
        content_size: [220.0, 120.0],
        dpi_scale: 1.0,
    });
    app.update();

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowDown,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
        UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::F2,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
        UiRoutedEvent::Text {
            window,
            target: owner,
            text: "Birch 🌿".to_owned(),
        },
        UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ];
    app.update();
    assert_eq!(
        app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
        "Birch 🌿"
    );
    assert!(app.world().resource::<bbui_core::UiHistory>().undo_len() > 0);

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
        app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
        "Oak"
    );
    assert!(app.world().resource::<bbui_core::UiHistory>().redo_len() > 0);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::Y,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert_eq!(
        app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
        "Birch 🌿"
    );
}

#[test]
pub(super) fn tree_rows_route_expand_drag_reorder_and_single_undo() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let tree = TreeModel {
        nodes: vec![
            crate::TreeNode {
                id: 1,
                parent: None,
                children: vec![2, 3],
                value: "Scene".to_owned(),
                expanded: false,
                enabled: true,
            },
            crate::TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![],
                value: "Camera".to_owned(),
                expanded: false,
                enabled: true,
            },
            crate::TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
                value: "Light".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..TreeModel::default()
    };
    let owner = WidgetTree::new(WidgetNode::tree("Outliner", tree))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 240.0, 130.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 0,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    let root_row = collection_items(app.world_mut(), owner)
        .into_iter()
        .find(|(_, item)| item.id == 1)
        .unwrap();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: root_row.0,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [10.0, 40.0],
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<TreeModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::ArrowLeft,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<TreeModel<String>>(owner)
            .unwrap()
            .visible_ids(),
        [1]
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::ArrowRight,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<TreeModel<String>>(owner)
            .unwrap()
            .visible_ids(),
        [1, 2, 3]
    );

    let rows = collection_items(app.world_mut(), owner);
    let camera = rows.iter().find(|(_, item)| item.id == 2).unwrap().clone();
    let light = rows.iter().find(|(_, item)| item.id == 3).unwrap().clone();
    let camera_hit = app.world().get::<UiHitBox>(camera.0).unwrap().rect;
    let light_hit = app.world().get::<UiHitBox>(light.0).unwrap().rect;
    let start = [light_hit.x + 20.0, light_hit.y + light_hit.height * 0.5];
    let end = [camera_hit.x + 20.0, camera_hit.y + camera_hit.height * 0.25];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: light.0,
            button: UiPointerButton::Primary,
            position: start,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: camera.0,
            position: end,
            delta: [end[0] - start[0], end[1] - start[1]],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: camera.0,
            button: UiPointerButton::Primary,
            position: end,
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(
        app.world().get::<TreeModel<String>>(owner).unwrap().nodes[0].children,
        [3, 2]
    );
    assert!(app.world().resource::<bbui_core::UiHistory>().undo_len() > 0);

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
        app.world().get::<TreeModel<String>>(owner).unwrap().nodes[0].children,
        [2, 3]
    );
}

#[test]
pub(super) fn preview_grid_selects_spatially_and_paints_registered_images_or_tints() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut grid = PreviewGrid {
        items: (0..16)
            .map(|index| {
                let mut item = crate::PreviewGridItem::new(index + 1, format!("Material {index}"));
                item.tint = [0.2 + index as f32 * 0.035, 0.42, 0.72, 1.0];
                if index == 0 {
                    item.asset_id = Some(901);
                }
                item
            })
            .collect(),
        cell_width: 112.0,
        cell_height: 104.0,
        ..PreviewGrid::default()
    };
    grid.select(3, false, false);
    let owner = WidgetTree::new(WidgetNode::preview_grid("Materials", grid))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 250.0, 180.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 0,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    let rows = collection_items(app.world_mut(), owner);
    let image_item = rows.iter().find(|(_, item)| item.id == 1).unwrap().0;
    let other_item = rows.iter().find(|(_, item)| item.id == 4).unwrap().0;
    assert_eq!(
        app.world()
            .get::<CollectionItemControl>(image_item)
            .unwrap()
            .asset_id,
        Some(901)
    );
    let other_layout = app.world().get::<UiLayoutResult>(other_item).unwrap().rect;
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|draw| matches!(draw.primitive, Primitive::Image { asset_id: 901, .. }))
    );
    assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|draw| matches!(draw.primitive, Primitive::RoundedRect { color, .. } if (color.0 - 0.305).abs() < 0.0001 && (color.1 - 0.42).abs() < 0.0001))
        );

    app.world_mut()
        .resource_mut::<WidgetInteractionState>()
        .modifiers
        .insert(
            window,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: other_item,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [other_layout.x + 10.0, other_layout.y + 40.0],
    }];
    app.update();
    let grid = app.world().get::<PreviewGrid>(owner).unwrap();
    assert!(grid.selection.contains(3));
    assert!(grid.selection.contains(4));
    assert_eq!(grid.selection.active, Some(4));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::ArrowUp,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<PreviewGrid>(owner)
            .unwrap()
            .selection
            .active,
        Some(2)
    );
}

#[test]
pub(super) fn preview_grid_keyboard_routes_column_page_edge_and_range_navigation() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let grid = PreviewGrid {
        items: (1..=12)
            .map(|id| crate::PreviewGridItem::new(id, format!("Item {id}")))
            .collect(),
        columns: 3,
        cell_width: 60.0,
        cell_height: 56.0,
        ..PreviewGrid::default()
    };
    let owner = WidgetTree::new(WidgetNode::preview_grid("Items", grid))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 240.0, 220.0),
        clip: None,
        z: 0,
        content_size: [240.0, 220.0],
        dpi_scale: 1.0,
    });
    app.update();
    app.world_mut()
        .get_mut::<PreviewGrid>(owner)
        .unwrap()
        .select(1, false, false);
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
    let active = |app: &App| {
        app.world()
            .get::<PreviewGrid>(owner)
            .unwrap()
            .selection
            .active
            .unwrap()
    };

    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(active(&app), 4, "Down advances one grid column");
    send_key(
        &mut app,
        UiKeyCode::ArrowRight,
        UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(active(&app), 5, "Right advances one item");
    assert_eq!(
        app.world()
            .get::<PreviewGrid>(owner)
            .unwrap()
            .selection
            .selected,
        [4, 5].into(),
        "Shift extends the active item range"
    );
    send_key(&mut app, UiKeyCode::ArrowLeft, UiModifiers::default());
    assert_eq!(active(&app), 4);
    send_key(&mut app, UiKeyCode::ArrowUp, UiModifiers::default());
    assert_eq!(active(&app), 1);
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    assert!(active(&app) > 1, "PageDown advances by a grid row page");
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(active(&app), 1);
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(active(&app), 12);
    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(active(&app), 1);
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
            .get::<PreviewGrid>(owner)
            .unwrap()
            .selection
            .selected
            .len(),
        12
    );
    send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::CollectionItemActivated { target, id: 12 } if *target == owner)));
}

#[test]
pub(super) fn data_table_virtualizes_stable_cells_routes_selection_keyboard_and_scroll() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut table = sample_table(100);
    table.columns[0].width = 180.0;
    table.columns[1].width = 120.0;
    let owner = WidgetTree::new(WidgetNode::data_table("Objects", table))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 260.0, 150.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 4,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    let initial = data_table_cells(app.world_mut(), owner);
    let visible_rows: HashSet<_> = initial.iter().map(|(_, cell)| cell.row_id).collect();
    assert_eq!(visible_rows.len(), 5);
    assert_eq!(initial.len(), visible_rows.len() * 2);
    let stable_entity = initial
        .iter()
        .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
        .unwrap()
        .0;
    assert_eq!(data_table_headers(app.world_mut(), owner).len(), 2);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [20.0, 80.0],
        delta: [0.0, -1.0],
        unit: bbui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(app.world().get::<DataTable>(owner).unwrap().scroll_rows, 1);
    let after_scroll = data_table_cells(app.world_mut(), owner);
    assert_eq!(
        after_scroll
            .iter()
            .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
            .unwrap()
            .0,
        stable_entity,
        "a row that remains visible should keep its cell entity"
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [220.0, 80.0],
        delta: [-35.0, 0.0],
        unit: bbui_core::UiWheelUnit::Pixels,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(app.world().get::<DataTable>(owner).unwrap().scroll_x, 35.0);
    assert_eq!(
        data_table_cells(app.world_mut(), owner)
            .iter()
            .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
            .unwrap()
            .0,
        stable_entity,
        "horizontal scrolling also preserves visible cell identity"
    );

    let cell = data_table_cells(app.world_mut(), owner)
        .iter()
        .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
        .unwrap()
        .0;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: cell,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [40.0, 90.0],
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .active,
        Some(3)
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::DataTableCommitted { target, .. } if *target == owner)));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::ArrowRight,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(
        app.world().get::<DataTable>(owner).unwrap().active_column,
        1
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::ArrowDown,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    let table = app.world().get::<DataTable>(owner).unwrap();
    assert_eq!(table.selection.active, Some(4));
    assert_eq!(table.selection.selected, [3, 4].into());

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::Enter,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::DataTableCellActivated { target, row_id: 4, column: 1 } if *target == owner)));

    app.world_mut()
        .resource_mut::<WidgetInteractionState>()
        .modifiers
        .insert(
            window,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
    let cell = data_table_cells(app.world_mut(), owner)
        .into_iter()
        .find(|(_, cell)| cell.row_id == 6 && cell.column == 0)
        .unwrap()
        .0;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: cell,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [45.0, 135.0],
    }];
    app.update();
    let table = app.world().get::<DataTable>(owner).unwrap();
    assert_eq!(table.selection.active, Some(6));
    assert_eq!(table.selection.selected, [3, 4, 6].into());

    let key = |key, modifiers| UiRoutedEvent::Key {
        window,
        target: owner,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    )];
    app.update();
    assert_eq!(
        app.world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .selected
            .len(),
        100,
        "Ctrl+A selects every row"
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::Home, UiModifiers::default())];
    app.update();
    assert_eq!(
        app.world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::PageDown, UiModifiers::default())];
    app.update();
    let paged_row = app
        .world()
        .get::<DataTable>(owner)
        .unwrap()
        .selection
        .active
        .unwrap();
    assert!(paged_row > 1, "PageDown advances by the viewport row count");
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::PageUp, UiModifiers::default())];
    app.update();
    assert_eq!(
        app.world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .active,
        Some(1),
        "PageUp reverses the same row stride"
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::End, UiModifiers::default())];
    app.update();
    assert_eq!(
        app.world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .active,
        Some(100)
    );
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

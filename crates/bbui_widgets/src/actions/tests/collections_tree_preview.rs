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

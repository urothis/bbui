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

    let before_undo_count = app.world().resource::<bbui_core::UiHistory>().undo_len();
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
        app.world().resource::<bbui_core::UiHistory>().undo_len(),
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

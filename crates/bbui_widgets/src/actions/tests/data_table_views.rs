use super::*;

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

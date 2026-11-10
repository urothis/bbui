use super::*;

#[test]
pub(super) fn virtual_collection_and_browser_sync_skip_idle_work_and_only_rewrite_selected_rows() {
    let mut world = World::new();
    world.init_resource::<Theme>();
    world.init_resource::<WidgetCollectionItems>();
    world.init_resource::<WidgetFileBrowserItems>();
    let window = world.spawn_empty().id();
    let list = ListModel {
        items: (0..10_000)
            .map(|id| ListItem {
                id,
                value: format!("Item {id}"),
                enabled: true,
            })
            .collect(),
        ..ListModel::default()
    };
    let collection_rect = UiRect::new(0.0, 0.0, 400.0, 300.0);
    let collection_owner = world
        .spawn((
            list,
            UiLayoutResult {
                window,
                rect: collection_rect,
                clip: None,
                z: 1,
                content_size: [collection_rect.width, collection_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();

    let directory = std::env::temp_dir().join(format!(
        "bui-virtual-browser-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    for index in 0..32 {
        std::fs::write(directory.join(format!("entry-{index:02}.txt")), b"entry").unwrap();
    }
    let browser = FileBrowser::new(&directory).unwrap();
    let browser_rect = UiRect::new(500.0, 0.0, 400.0, 300.0);
    let browser_owner = world
        .spawn((
            browser,
            FileBrowserViewState::default(),
            UiLayoutResult {
                window,
                rect: browser_rect,
                clip: None,
                z: 1,
                content_size: [browser_rect.width, browser_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();

    sync_collection_views(&mut world);
    sync_file_browser_items(&mut world);
    let collection_items = world
        .resource::<WidgetCollectionItems>()
        .items
        .get(&collection_owner)
        .unwrap()
        .clone();
    let browser_items = world
        .resource::<WidgetFileBrowserItems>()
        .items
        .get(&browser_owner)
        .unwrap()
        .clone();
    assert!(
        collection_items.len() < 32,
        "only viewport rows are mounted"
    );
    assert!(browser_items.len() < 32, "only viewport rows are mounted");

    let collection_ticks: HashMap<_, _> = collection_items
        .iter()
        .map(|(id, entity)| {
            (
                *id,
                entity_component_changed_tick::<CollectionItemControl>(&world, *entity).unwrap(),
            )
        })
        .collect();
    let browser_ticks: HashMap<_, _> = browser_items
        .iter()
        .map(|(path, entity)| {
            (
                path.clone(),
                entity_component_changed_tick::<FileBrowserItem>(&world, *entity).unwrap(),
            )
        })
        .collect();
    let collection_cache_tick = world
        .get_resource_change_ticks::<WidgetCollectionItems>()
        .unwrap()
        .changed
        .get();
    let browser_cache_tick = world
        .get_resource_change_ticks::<WidgetFileBrowserItems>()
        .unwrap()
        .changed
        .get();

    world.increment_change_tick();
    sync_collection_views(&mut world);
    sync_file_browser_items(&mut world);
    assert_eq!(
        world
            .get_resource_change_ticks::<WidgetCollectionItems>()
            .unwrap()
            .changed
            .get(),
        collection_cache_tick,
        "idle collection sync does not mutate its cache"
    );
    assert_eq!(
        world
            .get_resource_change_ticks::<WidgetFileBrowserItems>()
            .unwrap()
            .changed
            .get(),
        browser_cache_tick,
        "idle file-browser sync does not mutate its cache"
    );
    for (id, entity) in &collection_items {
        assert_eq!(
            entity_component_changed_tick::<CollectionItemControl>(&world, *entity),
            collection_ticks.get(id).copied()
        );
    }
    for (path, entity) in &browser_items {
        assert_eq!(
            entity_component_changed_tick::<FileBrowserItem>(&world, *entity),
            browser_ticks.get(path).copied()
        );
    }

    let selected_id = *collection_items.keys().min().unwrap();
    let selected_path = browser_items.keys().next().unwrap().clone();
    world.increment_change_tick();
    assert!(
        world
            .get_mut::<ListModel<String>>(collection_owner)
            .unwrap()
            .select(selected_id, false, false)
    );
    world
        .get_mut::<FileBrowser>(browser_owner)
        .unwrap()
        .selected
        .insert(selected_path.clone());
    world.increment_change_tick();
    sync_collection_views(&mut world);
    sync_file_browser_items(&mut world);

    let changed_collection_rows = collection_items
        .iter()
        .filter(|(id, entity)| {
            entity_component_changed_tick::<CollectionItemControl>(&world, **entity)
                != collection_ticks.get(id).copied()
        })
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    assert_eq!(changed_collection_rows, [selected_id]);
    let changed_browser_rows = browser_items
        .iter()
        .filter(|(path, entity)| {
            entity_component_changed_tick::<FileBrowserItem>(&world, **entity)
                != browser_ticks.get(*path).copied()
        })
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    assert_eq!(changed_browser_rows, [selected_path]);

    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
pub(super) fn data_table_sync_prunes_keys_for_a_wide_and_tall_viewport() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut table = sample_table(96);
    table.columns = (0..48)
        .map(|column| crate::TableColumn::new(format!("Column {column}"), 30.0))
        .collect();
    for row in &mut table.rows {
        row.cells = (0..48)
            .map(|column| format!("Row {} Column {column}", row.id))
            .collect();
    }
    let owner = WidgetTree::new(WidgetNode::data_table("Wide", table))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 1_000.0, 800.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 3,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });

    sync_data_table_views(app.world_mut());
    let initial_cells = data_table_cells(app.world_mut(), owner);
    assert!(
        initial_cells.len() > 1_000,
        "the viewport is both wide and tall"
    );

    {
        let mut table = app.world_mut().get_mut::<DataTable>(owner).unwrap();
        table.scroll_rows = 12;
        table.scroll_x = 120.0;
    }
    app.world_mut().increment_change_tick();
    sync_data_table_views(app.world_mut());

    let cells = data_table_cells(app.world_mut(), owner);
    let headers = data_table_headers(app.world_mut(), owner);
    let cached_count = app
        .world()
        .resource::<WidgetDataTableItems>()
        .items
        .get(&owner)
        .unwrap()
        .len();
    assert_eq!(cached_count, cells.len() + headers.len());
    assert!(cells.len() > 1_000);
}

#[test]
pub(super) fn tracked_table_scroll_retains_controls_and_raw_edits_reconcile() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::data_table("Table", sample_table(80)))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(0.0, 0.0, 420.0, 300.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: Some(rect),
        z: 3,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    sync_data_table_views(app.world_mut());
    let key = DataTableItemKey::Cell {
        row_id: 3,
        column: 0,
    };
    let entity = app.world().resource::<WidgetDataTableItems>().items[&owner][&key];
    let before = entity_component_changed_tick::<DataTableCellControl>(app.world(), entity);
    set_data_table_viewport(app.world_mut(), owner, 1, 0.0);
    sync_data_table_views(app.world_mut());
    assert_eq!(
        app.world().resource::<WidgetDataTableItems>().items[&owner][&key],
        entity
    );
    assert_eq!(
        entity_component_changed_tick::<DataTableCellControl>(app.world(), entity),
        before
    );

    app.world_mut()
        .get_mut::<DataTable>(owner)
        .unwrap()
        .rows
        .iter_mut()
        .find(|row| row.id == 3)
        .unwrap()
        .cells[0] = "Raw edit".into();
    set_data_table_viewport(app.world_mut(), owner, 2, 0.0);
    sync_data_table_views(app.world_mut());
    assert_eq!(
        app.world()
            .get::<DataTableCellControl>(entity)
            .unwrap()
            .value,
        "Raw edit"
    );
}

#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub(super) struct VirtualSyncChanges {
    pub(super) table_cells: usize,
    pub(super) table_headers: usize,
    pub(super) graph_nodes: usize,
    pub(super) graph_sockets: usize,
    pub(super) graph_links: usize,
    pub(super) layout_results: usize,
    pub(super) hit_boxes: usize,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn count_virtual_sync_changes(
    table_cells: Query<Entity, Changed<DataTableCellControl>>,
    table_headers: Query<Entity, Changed<DataTableHeaderControl>>,
    graph_nodes: Query<Entity, Changed<NodeGraphNodeControl>>,
    graph_sockets: Query<Entity, Changed<NodeGraphSocketControl>>,
    graph_links: Query<Entity, Changed<NodeGraphLinkControl>>,
    table_layouts: Query<Entity, (With<DataTableCellControl>, Changed<UiLayoutResult>)>,
    graph_layouts: Query<
        Entity,
        (
            Or<(
                With<NodeGraphNodeControl>,
                With<NodeGraphSocketControl>,
                With<NodeGraphLinkControl>,
            )>,
            Changed<UiLayoutResult>,
        ),
    >,
    table_hits: Query<Entity, (With<DataTableCellControl>, Changed<UiHitBox>)>,
    graph_hits: Query<
        Entity,
        (
            Or<(
                With<NodeGraphNodeControl>,
                With<NodeGraphSocketControl>,
                With<NodeGraphLinkControl>,
            )>,
            Changed<UiHitBox>,
        ),
    >,
    mut changes: ResMut<VirtualSyncChanges>,
) {
    *changes = VirtualSyncChanges {
        table_cells: table_cells.iter().count(),
        table_headers: table_headers.iter().count(),
        graph_nodes: graph_nodes.iter().count(),
        graph_sockets: graph_sockets.iter().count(),
        graph_links: graph_links.iter().count(),
        layout_results: table_layouts.iter().count() + graph_layouts.iter().count(),
        hit_boxes: table_hits.iter().count() + graph_hits.iter().count(),
    };
}

pub(super) fn spawn_color_ramp_widget(app: &mut App, rect: UiRect) -> (Entity, Entity) {
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let entity = WidgetTree::new(WidgetNode::color_ramp_default("Gradient"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(entity).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 7,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    (window, entity)
}

pub(super) fn spawn_hsv_color_widget(app: &mut App, rect: UiRect) -> (Entity, Entity) {
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let entity = WidgetTree::new(WidgetNode::hsv_color_default("Color"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(entity).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 9,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    (window, entity)
}

pub(super) fn spawn_curve_editor_widget(app: &mut App, rect: UiRect) -> (Entity, Entity) {
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let entity = WidgetTree::new(WidgetNode::curve_editor_default("Curve"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(entity).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 11,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    (window, entity)
}

pub(super) fn route_curve_key(
    app: &mut App,
    window: Entity,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
) {
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    }];
    app.update();
}

#[test]
pub(super) fn plugin_routes_keyboard_and_pointer_activations_into_value_events() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let checkbox = app.world_mut().spawn((
        UiNode,
        UiFocusable,
        UiHitBox {
            rect: UiRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            clip: None,
            z_index: 0,
            window: Some(window),
        },
        Checkbox::new("check", false),
    ));
    let id = checkbox.id();
    // Exercise the same routed queue consumed by the plugin without requiring CorePlugin here.
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: id,
            button: UiPointerButton::Primary,
            position: [5.0, 5.0],
        },
        UiRoutedEvent::PointerUp {
            window,
            target: id,
            button: UiPointerButton::Primary,
            position: [5.0, 5.0],
            cancelled: false,
        },
        UiRoutedEvent::Click {
            window,
            target: id,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [5.0, 5.0],
        },
    ];
    app.update();
    assert!(app.world().get::<Checkbox>(id).unwrap().is_checked());
    let events = app.world_mut().resource_mut::<WidgetEventQueue>().take();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WidgetEvent::CheckboxPaintCommitted { .. }))
    );

    let button = app.world_mut().spawn((UiNode, Button::new("Run"))).id();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: button,
        key: UiKeyCode::Enter,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(app.world().get::<Button>(button).unwrap().activations, 1);
    assert!(
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == button))
    );
}

#[test]
pub(super) fn color_ramp_routes_stop_reorder_and_gesture_undo_redo() {
    let mut app = App::new();
    let rect = UiRect::new(20.0, 30.0, 360.0, 194.0);
    let (window, entity) = spawn_color_ramp_widget(&mut app, rect);
    {
        let mut ramp = app.world_mut().get_mut::<ColorRamp>(entity).unwrap();
        ramp.add_stop(0.25, Rgba(1.0, 0.0, 0.0, 1.0)).unwrap();
        ramp.add_stop(0.75, Rgba(0.0, 1.0, 0.0, 1.0)).unwrap();
    }
    let selected = app
        .world()
        .get::<ColorRamp>(entity)
        .unwrap()
        .stops
        .iter()
        .find(|stop| (stop.position - 0.25).abs() < f32::EPSILON)
        .unwrap()
        .id;
    let track = color_ramp_track_rect(rect, Theme::default().spacing.color_ramp);
    let start = [track.x + track.width * 0.25, track.y + track.height + 10.0];
    let end = [track.x + track.width * 0.9, start[1]];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: start,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: entity,
            position: end,
            delta: [end[0] - start[0], 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: end,
            cancelled: false,
        },
        UiRoutedEvent::Click {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: end,
        },
    ];
    app.update();
    let ramp = app.world().get::<ColorRamp>(entity).unwrap();
    assert!(
        (ramp
            .stops
            .iter()
            .find(|stop| stop.id == selected)
            .unwrap()
            .position
            - 0.9)
            .abs()
            < 0.01
    );
    let order: Vec<_> = ramp.stops.iter().map(|stop| stop.id).collect();
    assert_eq!(order, vec![1, 4, selected, 2]);
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ColorRampCommitted { target, .. } if *target == entity)));

    let key = |key, modifiers| UiRoutedEvent::Key {
        window,
        target: entity,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    )];
    app.update();
    assert!(
        (app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .stops
            .iter()
            .find(|stop| stop.id == selected)
            .unwrap()
            .position
            - 0.25)
            .abs()
            < 0.01
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::Y,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    )];
    app.update();
    assert!(
        (app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .stops
            .iter()
            .find(|stop| stop.id == selected)
            .unwrap()
            .position
            - 0.9)
            .abs()
            < 0.01
    );
}

#[test]
pub(super) fn color_ramp_click_and_keyboard_edit_interpolation_and_rgba() {
    let mut app = App::new();
    let rect = UiRect::new(10.0, 20.0, 360.0, 194.0);
    let (window, entity) = spawn_color_ramp_widget(&mut app, rect);
    let mut spacing = Theme::default().spacing.color_ramp;
    spacing.button_step = 58.0;
    spacing.button_width = 52.0;
    spacing.track_x = 18.0;
    spacing.track_y = 42.0;
    spacing.track_width_inset = 36.0;
    spacing.channel_x = 54.0;
    spacing.channel_y = 90.0;
    spacing.channel_width_inset = 72.0;
    app.world_mut().resource_mut::<Theme>().spacing.color_ramp = spacing;
    let click = |position, click_count| UiRoutedEvent::Click {
        window,
        target: entity,
        button: UiPointerButton::Primary,
        click_count,
        position,
    };
    let add_x = rect.x + rect.width - spacing.button_right_inset;
    let remove_x = add_x + spacing.button_step;
    let interpolation_x = remove_x + spacing.button_step;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(
        [interpolation_x + spacing.button_width * 0.5, rect.y + 10.0],
        1,
    )];
    app.update();
    assert_eq!(
        app.world().get::<ColorRamp>(entity).unwrap().interpolation,
        Interpolation::Smooth
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(
        [add_x + spacing.button_width * 0.5, rect.y + 10.0],
        1,
    )];
    app.update();
    assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 3);
    let track = color_ramp_track_rect(rect, spacing);
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![click([track.x + track.width * 0.8, track.y + 8.0], 2)];
    app.update();
    assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 4);

    let channel_row = color_ramp_channel_rect(rect, ColorChannel::Red.index(), spacing);
    let start = [channel_row.x + channel_row.width * 0.2, channel_row.y + 5.0];
    let end = [channel_row.x + channel_row.width * 0.7, start[1]];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: start,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: entity,
            position: end,
            delta: [end[0] - start[0], 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: end,
            cancelled: false,
        },
    ];
    app.update();
    let ramp = app.world().get::<ColorRamp>(entity).unwrap();
    assert!((ramp.selected_stop().unwrap().color.0 - 0.7).abs() < 0.01);
    assert!(
        app.world()
            .get::<ColorRampEditorState>(entity)
            .unwrap()
            .editing_color
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: entity,
        key: UiKeyCode::ArrowRight,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert!(
        (app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .color
            .0
            - 0.71)
            .abs()
            < 0.01
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![click([rect.x + rect.width - 80.0, rect.y + 10.0], 1)];
    app.update();
    assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 3);
    let key = |key| UiRoutedEvent::Key {
        window,
        target: entity,
        key,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
    app.update();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
    app.update();
    assert_eq!(
        app.world().get::<ColorRamp>(entity).unwrap().interpolation,
        Interpolation::Constant
    );
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(item.primitive, Primitive::TextWrapped { .. }))
    );
}

#[test]
pub(super) fn color_ramp_keyboard_routes_stop_position_and_channel_editing() {
    let mut app = App::new();
    let (window, entity) = spawn_color_ramp_widget(&mut app, UiRect::new(0.0, 0.0, 360.0, 194.0));
    let send_key = |app: &mut App, key, modifiers| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: entity,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        app.update();
    };

    send_key(&mut app, UiKeyCode::A, UiModifiers::default());
    let added_id = app
        .world()
        .get::<ColorRamp>(entity)
        .unwrap()
        .selected
        .unwrap();
    assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 3);
    let start = app
        .world()
        .get::<ColorRamp>(entity)
        .unwrap()
        .selected_stop()
        .unwrap()
        .position;
    send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
    let normal_step = app
        .world()
        .get::<ColorRamp>(entity)
        .unwrap()
        .selected_stop()
        .unwrap()
        .position;
    assert!((normal_step - start - 0.01).abs() < 0.001);
    send_key(
        &mut app,
        UiKeyCode::ArrowRight,
        UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    );
    let fine_step = app
        .world()
        .get::<ColorRamp>(entity)
        .unwrap()
        .selected_stop()
        .unwrap()
        .position;
    assert!((fine_step - normal_step - 0.001).abs() < 0.0002);
    send_key(
        &mut app,
        UiKeyCode::ArrowRight,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    let coarse_step = app
        .world()
        .get::<ColorRamp>(entity)
        .unwrap()
        .selected_stop()
        .unwrap()
        .position;
    assert!((coarse_step - fine_step - 0.1).abs() < 0.001);
    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .stops
            .iter()
            .find(|stop| stop.id == added_id)
            .unwrap()
            .position,
        0.0
    );
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .stops
            .iter()
            .find(|stop| stop.id == added_id)
            .unwrap()
            .position,
        1.0
    );
    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(
        app.world().get::<ColorRamp>(entity).unwrap().selected,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::ArrowUp, UiModifiers::default());
    assert_eq!(
        app.world().get::<ColorRamp>(entity).unwrap().selected,
        Some(added_id)
    );
    send_key(&mut app, UiKeyCode::Delete, UiModifiers::default());
    assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 2);
    send_key(&mut app, UiKeyCode::A, UiModifiers::default());
    send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
    assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 2);

    app.world_mut()
        .get_mut::<ColorRamp>(entity)
        .unwrap()
        .select_stop(1);
    send_key(&mut app, UiKeyCode::C, UiModifiers::default());
    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ColorRampEditorState>(entity)
            .unwrap()
            .active_channel,
        ColorChannel::Green
    );
    send_key(
        &mut app,
        UiKeyCode::ArrowRight,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert!(
        (app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .color
            .1
            - 0.1)
            .abs()
            < 0.001
    );
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .color
            .1,
        1.0
    );
    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .color
            .1,
        0.0
    );
    send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(
        !app.world()
            .get::<ColorRampEditorState>(entity)
            .unwrap()
            .editing_color
    );
}

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
        "bbui-virtual-browser-{}-{}",
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

use bevy::{
    asset::AssetPlugin,
    camera::Camera,
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    prelude::{App, ImagePlugin, MinimalPlugins, Node, WindowPlugin},
};
use bui::{
    BuiPlugin,
    render::{DisplayList, RenderSyncStats},
    widgets::{Button, NodeGraph, NodeSocket, WidgetNode, WidgetTree},
};
use bui_core::{UiChildren, UiEnabled, UiNode, UiVisible, set_parent};
use bui_layout::{Rect, UiLayoutResult};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BuiPlugin);
    app
}

fn parent(app: &mut App) -> bevy::prelude::Entity {
    app.world_mut().spawn((UiNode, UiChildren::default())).id()
}

fn button(
    app: &mut App,
    parent: bevy::prelude::Entity,
    window: bevy::prelude::Entity,
    label: &str,
    rect: Rect,
) -> bevy::prelude::Entity {
    let entity = WidgetTree::new(WidgetNode::button(label))
        .spawn(app.world_mut(), Some(parent))
        .unwrap();
    app.world_mut().entity_mut(entity).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 4,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    entity
}

fn stats(app: &App) -> RenderSyncStats {
    *app.world().resource::<RenderSyncStats>()
}

fn node_count(app: &mut App) -> usize {
    let mut query = app.world_mut().query::<&Node>();
    query.iter(app.world()).count()
}

fn assert_incremental_since(
    app: &App,
    previous: RenderSyncStats,
    maximum_scanned_keys: usize,
    scenario: &str,
) {
    let current = stats(app);
    assert_eq!(
        current.full_syncs, previous.full_syncs,
        "{scenario} unexpectedly triggered a complete renderer sync"
    );
    assert!(
        current.incremental_syncs > previous.incremental_syncs,
        "{scenario} did not run an incremental renderer sync: {current:?}"
    );
    assert!(
        current.scanned_draw_keys <= maximum_scanned_keys,
        "{scenario} scanned too many draw keys: {current:?}"
    );
    assert!(
        current.updated_render_entries > 0,
        "{scenario} did not update any renderer entries: {current:?}"
    );
}

#[test]
fn public_plugin_keeps_mount_reparent_camera_and_graph_edits_incremental() {
    let mut app = app();
    // Preallocate roots so a late widget can exercise local root insertion without relying on
    // whether Bevy reuses a despawned entity index in this update schedule.
    let root_slot = app
        .world_mut()
        .spawn((UiNode, UiEnabled(true), UiVisible(true)))
        .id();
    let dead_root = app
        .world_mut()
        .spawn((UiNode, UiEnabled(true), UiVisible(true)))
        .id();
    let camera = app.world_mut().spawn(Camera::default()).id();
    let first_parent = parent(&mut app);
    let second_parent = parent(&mut app);

    let stationary = button(
        &mut app,
        first_parent,
        camera,
        "Stationary",
        Rect::new(10.0, 10.0, 140.0, 30.0),
    );
    let moving = button(
        &mut app,
        second_parent,
        camera,
        "Moving",
        Rect::new(10.0, 50.0, 140.0, 30.0),
    );
    let peer = button(
        &mut app,
        second_parent,
        camera,
        "Peer",
        Rect::new(10.0, 90.0, 140.0, 30.0),
    );
    app.update();
    let initial = stats(&app);
    assert!(
        initial.full_syncs > 0,
        "initial scene should establish the cache"
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .len(),
        3
    );
    let initial_nodes = node_count(&mut app);

    app.world_mut().entity_mut(dead_root).despawn();
    app.update();
    let root_rect = Rect::new(170.0, 10.0, 140.0, 30.0);
    app.world_mut().entity_mut(root_slot).insert((
        Button::new("Mounted root"),
        UiLayoutResult {
            window: camera,
            rect: root_rect,
            clip: None,
            z: 4,
            content_size: [root_rect.width, root_rect.height],
            dpi_scale: 1.0,
        },
    ));
    app.update();
    assert_incremental_since(&app, initial, 16, "single widget mount");
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .len(),
        4,
        "the new widget contributes one retained paint fragment"
    );
    assert!(
        node_count(&mut app) > initial_nodes,
        "mounting a button should create its render nodes"
    );
    let mounted_order = app.world().resource::<DisplayList>().widget_fragments()[&root_slot].order;
    let stationary_order =
        app.world().resource::<DisplayList>().widget_fragments()[&stationary].order;
    let moving_order = app.world().resource::<DisplayList>().widget_fragments()[&moving].order;
    assert!(
        root_slot.index() < stationary.index()
            && mounted_order < stationary_order
            && stationary_order < moving_order,
        "a lower-index root mounted late is inserted at its deterministic root position"
    );
    let after_mount = stats(&app);

    let mounted = button(
        &mut app,
        first_parent,
        camera,
        "Mounted child",
        Rect::new(170.0, 50.0, 140.0, 30.0),
    );
    app.update();
    assert_incremental_since(&app, after_mount, 16, "single child mount");
    let after_child_mount = stats(&app);

    set_parent(app.world_mut(), mounted, second_parent).unwrap();
    app.update();
    assert_incremental_since(&app, after_child_mount, 20, "widget reparent");
    let fragments = app.world().resource::<DisplayList>().widget_fragments();
    assert!(fragments[&stationary].order < fragments[&moving].order);
    assert!(fragments[&moving].order < fragments[&peer].order);
    assert!(fragments[&peer].order < fragments[&mounted].order);
    let after_reparent = stats(&app);

    app.world_mut()
        .get_mut::<UiChildren>(second_parent)
        .unwrap()
        .0 = vec![mounted, peer, moving];
    app.update();
    assert_incremental_since(&app, after_reparent, 48, "multiple sibling reorder");
    let fragments = app.world().resource::<DisplayList>().widget_fragments();
    assert!(fragments[&mounted].order < fragments[&peer].order);
    assert!(fragments[&peer].order < fragments[&moving].order);
    let after_sibling_reorder = stats(&app);

    let moving_layout = *app.world().get::<UiLayoutResult>(moving).unwrap();
    app.world_mut()
        .entity_mut(moving)
        .remove::<UiLayoutResult>();
    app.update();
    assert_incremental_since(&app, after_sibling_reorder, 48, "layout removal");
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .contains_key(&moving)
    );
    let after_layout_removal = stats(&app);

    app.world_mut().entity_mut(moving).insert(moving_layout);
    app.update();
    assert_incremental_since(&app, after_layout_removal, 48, "layout restoration");
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .contains_key(&moving)
    );
    let after_layout_restoration = stats(&app);

    app.world_mut().entity_mut(camera).remove::<Camera>();
    app.update();
    assert_incremental_since(&app, after_layout_restoration, 64, "camera removal");
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .values()
            .all(|fragment| fragment.target_camera.is_none())
    );
    let after_camera_removal = stats(&app);

    let mut graph = NodeGraph::default();
    graph.add_node(
        "Output",
        [120.0, 70.0],
        vec![NodeSocket::input("value", bui::widgets::SocketType::Float)],
    );
    let graph_entity = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
        .spawn(app.world_mut(), Some(first_parent))
        .unwrap();
    let graph_rect = Rect::new(350.0, 20.0, 420.0, 280.0);
    app.world_mut()
        .entity_mut(graph_entity)
        .insert(UiLayoutResult {
            window: camera,
            rect: graph_rect,
            clip: None,
            z: 4,
            content_size: [graph_rect.width, graph_rect.height],
            dpi_scale: 1.0,
        });
    app.update();
    assert_incremental_since(&app, after_camera_removal, 2_100, "graph mount");
    let before_pan = app.world().resource::<DisplayList>().widget_fragments()[&graph_entity]
        .items
        .clone();
    let after_graph_mount = stats(&app);

    app.world_mut()
        .get_mut::<NodeGraph>(graph_entity)
        .unwrap()
        .pan = [17.0, 9.0];
    app.update();
    assert_incremental_since(
        &app,
        after_graph_mount,
        before_pan.len().saturating_mul(2).saturating_add(8),
        "graph pan",
    );
    let after_pan = &app.world().resource::<DisplayList>().widget_fragments()[&graph_entity].items;
    assert_ne!(
        before_pan, *after_pan,
        "panning changes graph paint geometry"
    );
}

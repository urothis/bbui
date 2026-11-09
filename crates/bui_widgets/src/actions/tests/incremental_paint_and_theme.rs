use super::*;

#[test]
pub(super) fn widget_paint_cache_skips_idle_and_invalidates_visual_and_layout_inputs() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let entity = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("cached", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(1.0, 2.0, 120.0, 24.0),
                clip: None,
                z: 3,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    app.update();
    let initial = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert!(!initial.is_empty());
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 1);

    app.update();
    assert_eq!(app.world().resource::<WidgetPaintCache>().skipped_frames, 1);
    assert_eq!(
        app.world().resource::<DisplayList>().widget_items(),
        initial
    );

    app.world_mut()
        .get_mut::<Checkbox>(entity)
        .unwrap()
        .toggle();
    app.update();
    let checked = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        checked, initial,
        "a value mutation repaints in the same frame"
    );
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 2);

    app.world_mut().resource_mut::<Theme>().surface = Rgba(0.12, 0.34, 0.56, 1.0);
    app.update();
    let themed = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        themed, checked,
        "theme changes invalidate widget paint immediately"
    );
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 3);

    app.world_mut()
        .get_mut::<UiLayoutResult>(entity)
        .unwrap()
        .rect
        .x += 16.0;
    app.update();
    let moved = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        moved, themed,
        "layout changes repaint before the frame is presented"
    );
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 4);
}

#[test]
pub(super) fn one_changed_checkbox_repaints_only_its_fragment() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let first = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("first", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("second", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 30.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();
    let fragment_orders: HashMap<_, _> = app
        .world()
        .resource::<DisplayList>()
        .widget_fragments()
        .values()
        .map(|fragment| (fragment.entity, fragment.order))
        .collect();
    let overlay = item(
        ClipRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        },
        9,
        Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            radius: 2.0,
            color: Rgba(0.2, 0.3, 0.4, 1.0),
        },
    );
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_overlay_widget_items(vec![overlay.clone()]);
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .repainted_entities,
        2
    );
    app.world_mut().get_mut::<Checkbox>(first).unwrap().toggle();
    app.world_mut().resource_mut::<WidgetPaintCache>().prepared = true;
    app.world_mut()
        .resource_mut::<UiDirty>()
        .mark(UiDirtyDomain::Paint, first);
    collect_widget_paint(app.world_mut());
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(cache.repainted_entities, 1);
    assert_eq!(
        cache.published_fragments, 1,
        "one dirty control publishes only its own changed fragment"
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&first)
            .map(|fragment| fragment.order),
        fragment_orders.get(&first).copied()
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&second)
            .map(|fragment| fragment.order),
        fragment_orders.get(&second).copied(),
        "updating one entity keeps its peers at their existing paint ranks"
    );
    assert_eq!(
        app.world().resource::<DisplayList>().overlay_widget_items(),
        &[overlay],
        "per-entity widget updates preserve the post-widget overlay layer"
    );
}

#[test]
pub(super) fn widget_fragment_camera_migration_and_removal_update_only_that_entity() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let first_camera = app.world_mut().spawn(bevy_camera::Camera::default()).id();
    let second_camera = app.world_mut().spawn_empty().id();
    let migrating = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("migrate", false),
            UiLayoutResult {
                window: first_camera,
                rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z: 2,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("stays", false),
        UiLayoutResult {
            window: first_camera,
            rect: UiRect::new(0.0, 30.0, 120.0, 24.0),
            clip: None,
            z: 1,
            content_size: [120.0, 24.0],
            dpi_scale: 1.0,
        },
    ));

    app.update();
    app.world_mut()
        .get_mut::<UiLayoutResult>(migrating)
        .unwrap()
        .window = second_camera;
    app.update();

    let has_label = |items: &[DrawItem], label: &str| {
        items
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::Text { text, .. } if text == label))
    };
    let display_list = app.world().resource::<DisplayList>();
    assert!(
        display_list
            .camera_items(first_camera)
            .is_some_and(|items| has_label(items, "stays"))
    );
    assert!(has_label(display_list.widget_items(), "migrate"));
    assert!(
        display_list
            .camera_items(second_camera)
            .is_none_or(|items| items.is_empty())
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "moving to a window without a camera updates the same entity key"
    );

    app.world_mut()
        .entity_mut(second_camera)
        .insert(bevy_camera::Camera::default());
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(
        display_list
            .camera_items(second_camera)
            .is_some_and(|items| has_label(items, "migrate"))
    );
    assert!(!has_label(display_list.widget_items(), "migrate"));
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "adding a camera retargets associated widget fragments"
    );

    app.world_mut()
        .entity_mut(second_camera)
        .remove::<bevy_camera::Camera>();
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(has_label(display_list.widget_items(), "migrate"));
    assert!(
        display_list
            .camera_items(second_camera)
            .is_none_or(|items| items.is_empty())
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "removing a camera retargets associated widget fragments to the screen"
    );

    app.world_mut().entity_mut(migrating).despawn();
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(
        display_list
            .camera_items(first_camera)
            .is_some_and(|items| has_label(items, "stays"))
    );
    assert!(
        !has_label(display_list.widget_items(), "migrate"),
        "despawning a screen-migrated control removes its cached fragment"
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "despawning a control removes only its cached fragment"
    );
}

#[test]
pub(super) fn topology_edits_repaint_only_new_or_reordered_widget_fragments() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let first_group = app.world_mut().spawn((UiNode, UiChildren::default())).id();
    let second_group = app.world_mut().spawn((UiNode, UiChildren::default())).id();
    let spawn_child = |world: &mut World, parent: Entity, label: &str, y: f32| {
        world
            .spawn((
                UiNode,
                UiParent(parent),
                Checkbox::new(label, false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, y, 120.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id()
    };
    let first = spawn_child(app.world_mut(), first_group, "first", 0.0);
    let second = spawn_child(app.world_mut(), first_group, "second", 30.0);
    let third = spawn_child(app.world_mut(), second_group, "third", 60.0);
    let fourth = spawn_child(app.world_mut(), first_group, "fourth", 45.0);
    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0 = vec![first, second, fourth];
    app.world_mut()
        .get_mut::<UiChildren>(second_group)
        .unwrap()
        .0 = vec![third];
    for index in 0..10_000 {
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("peer", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(200.0, index as f32 * 24.0, 20.0, 20.0),
                clip: None,
                z: 1,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ));
    }
    app.update();

    let added = spawn_child(app.world_mut(), first_group, "added", 90.0);
    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0
        .push(added);
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 1,
        "mounting a widget paints only it"
    );
    assert_eq!(cache.published_fragments, 1);
    let order = |world: &World, entity| {
        world
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&entity)
            .unwrap()
            .order
    };
    assert!(order(app.world(), first) < order(app.world(), second));
    assert!(order(app.world(), second) < order(app.world(), added));
    assert!(order(app.world(), fourth) < order(app.world(), added));
    assert!(
        order(app.world(), added) < order(app.world(), third),
        "added rank {} should precede second group rank {}",
        order(app.world(), added),
        order(app.world(), third)
    );

    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0 = vec![added, first, second];
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 1,
        "sibling reorder repaints the moved rank"
    );
    assert_eq!(cache.published_fragments, 1);
    assert!(order(app.world(), added) < order(app.world(), first));
    assert!(order(app.world(), first) < order(app.world(), second));

    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0 = vec![second, fourth, added, first];
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 2,
        "a multi-sibling reorder republishes both moved fragments"
    );
    assert_eq!(cache.published_fragments, 2);
    assert!(order(app.world(), second) < order(app.world(), fourth));
    assert!(order(app.world(), fourth) < order(app.world(), added));
    assert!(order(app.world(), added) < order(app.world(), first));

    bui_core::set_parent(app.world_mut(), second, second_group).unwrap();
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 1,
        "reparenting repaints the moved subtree only"
    );
    assert_eq!(cache.published_fragments, 1);
    assert!(order(app.world(), fourth) < order(app.world(), added));
    assert!(order(app.world(), added) < order(app.world(), first));
    assert!(order(app.world(), first) < order(app.world(), third));
    assert!(order(app.world(), third) < order(app.world(), second));
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .len(),
        10_005,
        "unaffected peers retain their fragments"
    );
}

#[test]
pub(super) fn local_paint_order_expands_exhausted_rank_gaps_without_collisions() {
    let mut world = World::new();
    let first = world.spawn_empty().id();
    let last = world.spawn_empty().id();
    let mut ranks = HashMap::from([(first, 100), (last, 101)]);
    let mut order_keys = BTreeMap::from([(100, first), (101, last)]);
    let mut inserted = Vec::new();
    let mut saw_neighbor_rebalance = false;

    for _ in 0..80 {
        let entity = world.spawn_empty().id();
        let lower = ranks[&first];
        let upper = *order_keys
            .range((std::ops::Bound::Excluded(lower), std::ops::Bound::Unbounded))
            .next()
            .expect("the last anchor remains after every insertion")
            .0;
        let changed = assign_widget_rank_block(
            &[entity],
            &HashMap::new(),
            lower,
            upper,
            &mut ranks,
            &mut order_keys,
        );
        saw_neighbor_rebalance |= changed.len() > 1;
        inserted.push(entity);
    }

    let actual: Vec<_> = order_keys.values().copied().collect();
    let expected: Vec<_> = std::iter::once(first)
        .chain(inserted.into_iter().rev())
        .chain(std::iter::once(last))
        .collect();
    assert_eq!(
        actual, expected,
        "neighbor expansion preserves requested order"
    );
    assert!(
        saw_neighbor_rebalance,
        "a saturated local gap rebalances neighbors"
    );
    assert_eq!(order_keys.len(), ranks.len());
    for (rank, entity) in &order_keys {
        assert_eq!(ranks.get(entity), Some(rank));
    }
}

#[test]
pub(super) fn ten_thousand_peers_only_repaint_for_inherited_state_and_active_menu_changes() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut().resource_mut::<Theme>().motion.menu_open = std::time::Duration::from_secs(1);
    let state_root = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiChildren::default(),
        ))
        .id();
    let visible_child = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(state_root),
            Checkbox::new("visible child", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 0.0, 160.0, 24.0),
                clip: None,
                z: 1,
                content_size: [160.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let enabled_child = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(state_root),
            Checkbox::new("enabled child", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 30.0, 160.0, 24.0),
                clip: None,
                z: 1,
                content_size: [160.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut()
        .entity_mut(state_root)
        .insert(UiChildren(vec![visible_child, enabled_child]));
    for index in 0..10_000 {
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(200.0, index as f32 * 24.0, 20.0, 20.0),
                clip: None,
                z: 1,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ));
    }

    app.update();
    assert_eq!(
        app.world().resource::<WidgetPaintCache>().fragments.len(),
        10_002
    );

    app.world_mut().get_mut::<UiVisible>(state_root).unwrap().0 = false;
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(cache.repainted_entities, 0);
    assert_eq!(
        cache.published_fragments, 2,
        "hiding a parent removes only its two child fragments"
    );
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .contains_key(&visible_child)
    );
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .contains_key(&enabled_child)
    );

    app.world_mut().get_mut::<UiVisible>(state_root).unwrap().0 = true;
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .repainted_entities,
        2
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        2,
        "showing a parent republishes only its two child fragments"
    );

    app.world_mut().get_mut::<UiEnabled>(state_root).unwrap().0 = false;
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .repainted_entities,
        2
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        2,
        "disabling a parent refreshes only its two child fragments"
    );

    app.world_mut().get_mut::<UiEnabled>(state_root).unwrap().0 = true;
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .repainted_entities,
        2
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        2
    );

    let menu = app
        .world_mut()
        .spawn((
            UiNode,
            MenuControl::new(
                "Actions",
                crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
                MenuDisplay::Dropdown,
            ),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(20.0, 20.0, 120.0, 24.0),
                clip: None,
                z: 2,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
            UiChildren::default(),
        ))
        .id();
    let menu_item = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(menu),
            UiVisible(false),
            MenuItemControl {
                owner: menu,
                id: "open".to_owned(),
                label: "Open".to_owned(),
                parent_path: Vec::new(),
                index: 0,
                enabled: true,
                separator: false,
                has_submenu: false,
                shortcut: None,
            },
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 0.0, 1.0, 1.0),
                clip: None,
                z: 0,
                content_size: [1.0, 1.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut()
        .get_mut::<UiChildren>(menu)
        .unwrap()
        .0
        .push(menu_item);
    app.world_mut()
        .get_mut::<MenuControl>(menu)
        .unwrap()
        .items
        .push(menu_item);
    app.update();
    app.world_mut()
        .get_mut::<MenuControl>(menu)
        .unwrap()
        .open(None);
    app.update();
    app.update();

    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 2,
        "a menu transition repaints only its owner and item subtree"
    );
    assert_eq!(
        cache.published_fragments, 2,
        "menu animation output is scoped to the active menu subtree"
    );
}

#[test]
pub(super) fn pointer_position_alone_does_not_repaint_widgets() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin).init_resource::<UiFocus>();
    let window = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .pointer_position = Some([4.0, 4.0]);
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("steady", false),
        UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
            clip: None,
            z: 0,
            content_size: [120.0, 24.0],
            dpi_scale: 1.0,
        },
    ));
    app.update();
    let rebuilt = app.world().resource::<WidgetPaintCache>().rebuilt_frames;

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .pointer_position = Some([5.0, 5.0]);
    app.update();
    assert_eq!(
        app.world().resource::<WidgetPaintCache>().rebuilt_frames,
        rebuilt
    );
}

#[test]
pub(super) fn widget_paint_culls_items_outside_the_ancestor_clip() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("visible", false),
        UiLayoutResult {
            window: Entity::PLACEHOLDER,
            rect: UiRect::new(0.0, 0.0, 20.0, 20.0),
            clip: Some(UiRect::new(0.0, 0.0, 20.0, 20.0)),
            z: 1,
            content_size: [20.0, 20.0],
            dpi_scale: 1.0,
        },
    ));
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("clipped", false),
        UiLayoutResult {
            window: Entity::PLACEHOLDER,
            rect: UiRect::new(100.0, 100.0, 20.0, 20.0),
            clip: Some(UiRect::new(0.0, 0.0, 20.0, 20.0)),
            z: 1,
            content_size: [20.0, 20.0],
            dpi_scale: 1.0,
        },
    ));

    app.update();
    let items = app.world().resource::<DisplayList>().widget_items();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|item| item.clip
        == ClipRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        }));
}

#[test]
pub(super) fn runtime_theme_round_trip_keeps_tree_identity_and_refreshes_widget_paint() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("Apply").with_key("apply"),
        WidgetNode::checkbox("Visible", true).with_key("visible"),
        WidgetNode::number("Scale", 1.0, 0.0, 10.0, 0.1).with_key("scale"),
    ]));
    tree.spawn(app.world_mut(), None)
        .expect("the same declared tree remains mounted while themes switch");
    let keyed_entities: Vec<_> = app
        .world()
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiKey>()
                .map(|key| (key.0.clone(), entity.id()))
        })
        .collect();
    for (key, entity) in &keyed_entities {
        let y = match key.as_str() {
            "apply" => 0.0,
            "visible" => 30.0,
            "scale" => 60.0,
            _ => panic!(),
        };
        app.world_mut().entity_mut(*entity).insert(UiLayoutResult {
            window,
            rect: UiRect::new(12.0, y, 180.0, 24.0),
            clip: None,
            z: 4,
            content_size: [180.0, 24.0],
            dpi_scale: 1.0,
        });
    }

    app.update();
    let dark_theme = app.world().resource::<Theme>().clone();
    let dark_items = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert!(dark_items.iter().any(|item| {
            matches!(item.primitive, Primitive::RoundedRect { color, .. } if color == dark_theme.surface)
        }));

    *app.world_mut().resource_mut::<Theme>() = Theme::new(bui_style::ThemeKind::Light);
    app.update();
    let light_theme = app.world().resource::<Theme>().clone();
    let light_items = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        dark_items, light_items,
        "theme changes invalidate widget paint"
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        3,
        "a global theme change republishes each affected control fragment"
    );
    assert!(light_items.iter().any(|item| {
            matches!(item.primitive, Primitive::RoundedRect { color, .. } if color == light_theme.surface)
        }));
    assert_eq!(
        keyed_entities,
        app.world()
            .iter_entities()
            .filter_map(|entity| {
                entity
                    .get::<UiKey>()
                    .map(|key| (key.0.clone(), entity.id()))
            })
            .collect::<Vec<_>>(),
        "switching a theme must preserve the mounted entity identities"
    );
    for (_, entity) in &keyed_entities {
        assert_eq!(
            app.world().get::<UiLayoutResult>(*entity).unwrap().rect,
            UiRect::new(
                12.0,
                match app.world().get::<UiKey>(*entity).unwrap().0.as_str() {
                    "apply" => 0.0,
                    "visible" => 30.0,
                    "scale" => 60.0,
                    _ => panic!(),
                },
                180.0,
                24.0,
            )
        );
    }

    *app.world_mut().resource_mut::<Theme>() = Theme::new(bui_style::ThemeKind::Dark);
    app.update();
    assert_eq!(
        app.world().resource::<DisplayList>().widget_items(),
        dark_items.as_slice(),
        "returning to the original theme restores its paint deterministically"
    );
}

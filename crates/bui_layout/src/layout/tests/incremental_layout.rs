use super::*;

#[test]
pub(super) fn unchanged_layout_results_and_hit_boxes_do_not_trigger_change_detection() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin))
        .init_resource::<LayoutChanges>()
        .add_systems(Update, count_layout_changes.in_set(UiSystems::Render));
    let (window, parent) = root(app.world_mut(), Rect::new(0.0, 0.0, 100.0, 50.0));
    let child = app
        .world_mut()
        .spawn((
            UiNode,
            UiIntrinsicSize([1.0, 1.0]),
            UiLayoutNode::leaf(UiLength::Px(30.0), UiLength::Px(20.0)),
        ))
        .id();
    set_parent(app.world_mut(), child, parent).unwrap();

    app.update();
    let first = *app.world().resource::<LayoutChanges>();
    let first_work = *app.world().resource::<UiLayoutStats>();
    assert_eq!(first.results, 2);
    assert_eq!(first.hit_boxes, 2);
    assert_eq!(first_work.roots_processed, 1);
    assert_eq!(first_work.nodes_processed, 2);
    app.update();
    assert_eq!(*app.world().resource::<LayoutChanges>(), first);
    let idle_work = *app.world().resource::<UiLayoutStats>();
    assert_eq!(idle_work.roots_processed, 0);
    assert_eq!(idle_work.nodes_processed, 0);
    assert_eq!(idle_work.idle_frames, first_work.idle_frames + 1);

    let layout = *app.world().get::<UiLayoutNode>(child).unwrap();
    app.world_mut()
        .entity_mut(child)
        .insert((layout, UiIntrinsicSize([1.0, 1.0])));
    app.update();
    assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 0);
    assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 0);

    invalidate_ui_layout(app.world_mut(), child);
    app.update();
    assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
    assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 2);

    app.world_mut()
        .get_mut::<UiLayoutRoot>(parent)
        .unwrap()
        .viewport = Rect::new(0.0, 0.0, 120.0, 50.0);
    app.update();
    let resized = *app.world().resource::<LayoutChanges>();
    let resized_work = *app.world().resource::<UiLayoutStats>();
    assert!(resized.results > first.results);
    assert!(resized.hit_boxes > first.hit_boxes);
    assert_eq!(resized_work.roots_processed, 1);
    assert_eq!(resized_work.nodes_processed, 2);
    assert_eq!(resized.issues, first.issues);
    assert_eq!(
        app.world().get::<UiLayoutResult>(child).unwrap().window,
        window
    );
}

#[test]
pub(super) fn intrinsic_and_ancestor_style_changes_refresh_descendant_geometry_and_clip() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 100.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(60.0),
                },
                align_items: UiAlign::Start,
                clip_contents: true,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let text = app
        .world_mut()
        .spawn((
            UiNode,
            UiIntrinsicSize([120.0, 24.0]),
            UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
        ))
        .id();
    set_parent(app.world_mut(), parent, root).unwrap();
    set_parent(app.world_mut(), text, parent).unwrap();

    app.update();
    let initial = *app.world().get::<UiLayoutResult>(text).unwrap();
    assert_eq!(initial.rect.width, 120.0);
    assert_eq!(initial.clip, Some(Rect::new(0.0, 0.0, 80.0, 60.0)));

    app.world_mut()
        .entity_mut(text)
        .insert(UiIntrinsicSize([150.0, 24.0]));
    app.update();
    let intrinsic = *app.world().get::<UiLayoutResult>(text).unwrap();
    assert_eq!(intrinsic.rect.width, 150.0);
    assert_eq!(intrinsic.clip, initial.clip);
    assert_eq!(
        app.world().get::<UiHitBox>(text).unwrap().clip,
        intrinsic.clip
    );
    assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
    assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 3);

    let mut parent_style = app.world_mut().get_mut::<UiLayoutNode>(parent).unwrap();
    parent_style.size.width = UiLength::Px(50.0);
    app.update();
    let changed_ancestor = *app.world().get::<UiLayoutResult>(text).unwrap();
    assert_eq!(changed_ancestor.rect.width, 150.0);
    assert_eq!(changed_ancestor.clip, Some(Rect::new(0.0, 0.0, 50.0, 60.0)));
    assert_eq!(
        app.world().get::<UiHitBox>(text).unwrap().clip,
        changed_ancestor.clip
    );
    assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
    assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 3);
}

#[test]
pub(super) fn reparent_and_detach_invalidate_old_and_new_roots_and_clear_stale_hitboxes() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window_a = app.world_mut().spawn_empty().id();
    let window_b = app.world_mut().spawn_empty().id();
    let root_a = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window_a, Rect::new(0.0, 0.0, 100.0, 50.0), 1.0),
            UiLayoutNode::default(),
        ))
        .id();
    let root_b = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window_b, Rect::new(200.0, 0.0, 100.0, 50.0), 2.0),
            UiLayoutNode::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(30.0), UiLength::Px(20.0)),
        ))
        .id();
    set_parent(app.world_mut(), child, root_a).unwrap();
    app.update();
    assert_eq!(
        app.world().get::<UiLayoutResult>(child).unwrap().window,
        window_a
    );

    set_parent(app.world_mut(), child, root_b).unwrap();
    app.update();
    let moved = app.world().get::<UiLayoutResult>(child).unwrap();
    assert_eq!(moved.window, window_b);
    assert_eq!(moved.rect.x, 200.0);
    assert_eq!(moved.dpi_scale, 2.0);
    assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 2);

    bui_core::detach_from_parent(app.world_mut(), child);
    app.update();
    assert!(app.world().get::<UiLayoutResult>(child).is_none());
    assert!(app.world().get::<UiHitBox>(child).is_none());
    assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
}

#[test]
pub(super) fn viewport_registry_rect_and_dpi_changes_are_detected_without_tree_mutation() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let root = app.world_mut().spawn(UiNode).id();
    app.world_mut().resource_mut::<UiViewports>().0.insert(
        root,
        UiViewport {
            rect: Rect::new(3.0, 4.0, 80.0, 50.0),
            dpi_scale: 1.0,
        },
    );
    app.update();
    let first = *app.world().get::<UiLayoutResult>(root).unwrap();
    assert_eq!(first.rect, Rect::new(3.0, 4.0, 80.0, 50.0));
    assert_eq!(first.dpi_scale, 1.0);

    app.world_mut().resource_mut::<UiViewports>().0.insert(
        root,
        UiViewport {
            rect: Rect::new(3.0, 4.0, 90.0, 55.0),
            dpi_scale: 2.0,
        },
    );
    app.update();
    let resized = app.world().get::<UiLayoutResult>(root).unwrap();
    assert_eq!(resized.rect, Rect::new(3.0, 4.0, 90.0, 55.0));
    assert_eq!(resized.dpi_scale, 2.0);
    assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 1);
}

#[test]
pub(super) fn intrinsic_change_recomputes_only_its_node_when_parent_allocation_is_independent() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 300.0, 120.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let branch = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(60.0),
                },
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let sibling_branch = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(50.0),
                    height: UiLength::Px(50.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let dirty_leaf = app
        .world_mut()
        .spawn((
            UiNode,
            UiIntrinsicSize([2.0, 3.0]),
            UiLayoutNode::leaf(UiLength::Px(20.0), UiLength::Px(10.0)),
        ))
        .id();
    let sibling_leaf = app
        .world_mut()
        .spawn((
            UiNode,
            UiIntrinsicSize([4.0, 5.0]),
            UiLayoutNode::leaf(UiLength::Px(15.0), UiLength::Px(12.0)),
        ))
        .id();
    set_parent(app.world_mut(), branch, root).unwrap();
    set_parent(app.world_mut(), sibling_branch, root).unwrap();
    set_parent(app.world_mut(), dirty_leaf, branch).unwrap();
    set_parent(app.world_mut(), sibling_leaf, sibling_branch).unwrap();

    app.update();
    let sibling_before = *app.world().get::<UiLayoutResult>(sibling_leaf).unwrap();
    let sibling_hit_box_before = *app.world().get::<UiHitBox>(sibling_leaf).unwrap();
    let dirty_rect_before = app.world().get::<UiLayoutResult>(dirty_leaf).unwrap().rect;
    let dirty_hit_box_before = *app.world().get::<UiHitBox>(dirty_leaf).unwrap();
    app.world_mut()
        .entity_mut(dirty_leaf)
        .insert(UiIntrinsicSize([9.0, 7.0]));
    app.update();

    let dirty = app.world().get::<UiLayoutResult>(dirty_leaf).unwrap();
    assert_eq!(dirty.content_size, [9.0, 7.0]);
    assert_eq!(dirty.rect, dirty_rect_before);
    assert_eq!(
        *app.world().get::<UiHitBox>(dirty_leaf).unwrap(),
        dirty_hit_box_before
    );
    assert_eq!(
        *app.world().get::<UiLayoutResult>(sibling_leaf).unwrap(),
        sibling_before
    );
    assert_eq!(
        *app.world().get::<UiHitBox>(sibling_leaf).unwrap(),
        sibling_hit_box_before
    );
    let work = app.world().resource::<UiLayoutStats>();
    assert_eq!(work.roots_processed, 1);
    assert_eq!(
        work.nodes_processed, 1,
        "only the changed leaf should be visited"
    );
}

#[test]
pub(super) fn flex_intrinsic_change_reflows_and_visits_only_geometry_affected_siblings() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 60.0), 1.0),
            UiLayoutNode::flex(UiFlexDirection::Row, false, 0.0),
        ))
        .id();
    let children: Vec<_> = (0..3)
        .map(|_| {
            app.world_mut()
                .spawn((
                    UiNode,
                    UiIntrinsicSize([40.0, 20.0]),
                    UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                    UiFlexItem {
                        grow: 0.0,
                        shrink: 0.0,
                        ..UiFlexItem::default()
                    },
                ))
                .id()
        })
        .collect();
    for child in &children {
        set_parent(app.world_mut(), *child, root).unwrap();
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[2])
            .unwrap()
            .rect
            .x,
        80.0
    );

    app.world_mut()
        .entity_mut(children[0])
        .insert(UiIntrinsicSize([60.0, 20.0]));
    app.update();

    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[1])
            .unwrap()
            .rect
            .x,
        60.0
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[2])
            .unwrap()
            .rect
            .x,
        100.0
    );
    let work = app.world().resource::<UiLayoutStats>();
    assert_eq!(work.roots_processed, 1);
    assert_eq!(
        work.nodes_processed, 4,
        "the dirty item and two shifted siblings should be visited with the root"
    );
}

#[test]
pub(super) fn responsive_grid_intrinsic_change_updates_row_height_and_following_rows() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 100.0), 1.0),
            UiLayoutNode::responsive_grid(80, 2, 0.0),
        ))
        .id();
    let children: Vec<_> = (0..4)
        .map(|_| {
            app.world_mut()
                .spawn((
                    UiNode,
                    UiIntrinsicSize([80.0, 10.0]),
                    UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                ))
                .id()
        })
        .collect();
    for child in &children {
        set_parent(app.world_mut(), *child, root).unwrap();
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[2])
            .unwrap()
            .rect
            .y,
        10.0
    );

    app.world_mut()
        .entity_mut(children[0])
        .insert(UiIntrinsicSize([80.0, 40.0]));
    app.update();

    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[0])
            .unwrap()
            .rect
            .height,
        40.0
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[1])
            .unwrap()
            .rect
            .height,
        40.0
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[2])
            .unwrap()
            .rect
            .y,
        40.0
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[3])
            .unwrap()
            .rect
            .y,
        40.0
    );
    let work = app.world().resource::<UiLayoutStats>();
    assert_eq!(work.roots_processed, 1);
    assert_eq!(
        work.nodes_processed, 5,
        "all cells whose row geometry changed should be visited"
    );
}

#[test]
pub(super) fn ten_thousand_fixed_basis_flex_siblings_recompute_only_the_changed_leaf() {
    const NODE_COUNT: usize = 10_000;
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
            UiLayoutNode::flex(UiFlexDirection::Row, true, 3.0),
        ))
        .id();
    let mut first = None;
    for _ in 0..NODE_COUNT {
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([44.0, 22.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                UiFlexItem {
                    basis: Some(UiLength::Px(44.0)),
                    grow: 0.25,
                    ..UiFlexItem::default()
                },
            ))
            .id();
        set_parent(app.world_mut(), child, root).unwrap();
        first.get_or_insert(child);
    }
    let first = first.unwrap();
    app.update();
    let result_before = *app.world().get::<UiLayoutResult>(first).unwrap();
    let sibling = app.world().get::<UiChildren>(root).unwrap().0[1];
    let sibling_before = *app.world().get::<UiLayoutResult>(sibling).unwrap();

    app.world_mut()
        .entity_mut(first)
        .insert(UiIntrinsicSize([45.0, 22.0]));
    app.update();

    assert_eq!(
        app.world().get::<UiLayoutResult>(first).unwrap().rect,
        result_before.rect
    );
    assert_eq!(
        *app.world().get::<UiLayoutResult>(sibling).unwrap(),
        sibling_before
    );
    let work = app.world().resource::<UiLayoutStats>();
    assert_eq!(work.roots_processed, 1);
    assert_eq!(work.nodes_processed, 1);
}

#[test]
pub(super) fn measurement_cache_stays_bounded_when_ancestor_width_changes_repeatedly() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 400.0, 200.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(100.0),
                    height: UiLength::Px(80.0),
                },
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let leaf = app
        .world_mut()
        .spawn((
            UiNode,
            UiIntrinsicSize([20.0, 10.0]),
            UiLayoutNode::leaf(UiLength::Percent(0.5), UiLength::Px(10.0)),
        ))
        .id();
    set_parent(app.world_mut(), parent, root).unwrap();
    set_parent(app.world_mut(), leaf, parent).unwrap();
    app.update();

    for frame in 0..32 {
        let width = 80.0 + frame as f32 * 3.5;
        app.world_mut()
            .get_mut::<UiLayoutNode>(parent)
            .unwrap()
            .size
            .width = UiLength::Px(width);
        app.update();

        assert_eq!(
            app.world().get::<UiLayoutResult>(leaf).unwrap().rect.width,
            width * 0.5
        );
        let cache = app.world().resource::<UiMeasurementCache>();
        let leaf_measurements = cache
            .0
            .get(&leaf)
            .expect("leaf should have cached measurements")
            .len();
        assert!(
            leaf_measurements <= MAX_MEASUREMENT_ENTRIES,
            "leaf cache grew to {leaf_measurements} entries"
        );
    }

    let leaf_measurements = app
        .world()
        .resource::<UiMeasurementCache>()
        .0
        .get(&leaf)
        .unwrap()
        .len();
    assert_eq!(leaf_measurements, MAX_MEASUREMENT_ENTRIES);
}

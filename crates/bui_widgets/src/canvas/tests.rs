use super::*;

#[test]
fn clipped_hit_testing_respects_z_order_and_transform() {
    let mut canvas = Canvas::default();
    let back = canvas
        .add(
            [10.0, 10.0],
            CanvasShape::Rectangle { size: [40.0, 40.0] },
            [1.0; 4],
        )
        .unwrap();
    let front = canvas
        .add(
            [20.0, 20.0],
            CanvasShape::Ellipse { size: [40.0, 40.0] },
            [1.0; 4],
        )
        .unwrap();
    assert!(canvas.zoom_at([50.0, 50.0], 2.0));
    let point = canvas.world_to_screen([40.0, 40.0]);
    let viewport = UiRect::new(0.0, 0.0, 100.0, 100.0);
    assert_eq!(canvas.hit_test(point, viewport), Some(front));
    assert!(canvas.select_at(point, viewport, false));
    assert_eq!(canvas.selected, BTreeSet::from([front]));
    assert_eq!(
        canvas.hit_test(point, UiRect::new(0.0, 0.0, 10.0, 10.0)),
        None
    );
    canvas.objects[1].locked = true;
    assert_eq!(canvas.hit_test(point, viewport), Some(back));
}

#[test]
fn spatial_index_paints_visible_objects_and_tracks_one_object_move() {
    let mut canvas = Canvas::default();
    for index in 0..100_000 {
        let (x, y) = if index < 100 {
            ((index % 10) as f32 * 10.0, (index / 10) as f32 * 10.0)
        } else {
            (1_000.0 + index as f32 * 12.0, 1_000.0)
        };
        canvas
            .add(
                [x, y],
                CanvasShape::Rectangle { size: [8.0, 8.0] },
                [1.0; 4],
            )
            .unwrap();
    }
    let moved = canvas.objects[0].id;
    canvas.selected.insert(moved);
    let viewport = UiRect::new(0.0, 0.0, 100.0, 100.0);
    let clip = ClipRect::from(viewport);
    let theme = Theme::default();
    let index = CanvasSpatialIndex::new(&canvas);
    let visible = index.visible_indices(&canvas, viewport, clip, 8.0).unwrap();
    assert_eq!(visible.len(), 100);

    let mut full = Vec::new();
    let mut indexed = Vec::new();
    paint_canvas(&canvas, viewport, clip, 0, &theme, None, &mut full);
    paint_canvas_indexed(
        &canvas,
        &visible,
        viewport,
        clip,
        0,
        &theme,
        None,
        &mut indexed,
    );
    assert_eq!(indexed, full);
    assert_eq!(indexed.len(), 106);

    let mut world = World::new();
    let window = world.spawn_empty().id();
    let owner = world
        .spawn((
            UiNode,
            canvas,
            UiLayoutResult {
                window,
                rect: viewport,
                clip: None,
                z: 0,
                content_size: [viewport.width, viewport.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    world.insert_resource(CanvasSpatialIndexes::default());
    world.insert_resource(CanvasObjectItems::default());
    world.insert_resource(UiDirty::default());
    assert!(ensure_canvas_spatial_index(&mut world, owner));
    sync_canvas_object_views(&mut world);
    assert_eq!(world.resource::<CanvasObjectItems>().0[&owner].len(), 100);
    let moved_entity = world.resource::<CanvasObjectItems>().0[&owner][&moved];
    assert_eq!(
        world
            .resource_mut::<UiDirty>()
            .take(UiDirtyDomain::Paint)
            .entities
            .len(),
        100
    );

    assert!(move_selected_objects(&mut world, owner, [10.0, 0.0]));
    sync_canvas_object_views(&mut world);
    let moved_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
    assert_eq!(moved_batch.entities, HashSet::from([moved_entity]));
    assert_eq!(world.resource::<CanvasObjectItems>().0[&owner].len(), 100);

    assert!(move_selected_objects(&mut world, owner, [200.0, 0.0]));
    sync_canvas_object_views(&mut world);
    let removed_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
    assert_eq!(removed_batch.entities, HashSet::from([moved_entity]));
    assert!(world.get_entity(moved_entity).is_err());
    let directly_edited_id = world.get::<Canvas>(owner).unwrap().objects[1].id;
    let directly_edited_entity =
        world.resource::<CanvasObjectItems>().0[&owner][&directly_edited_id];
    assert!(edit_canvas_object(
        &mut world,
        owner,
        directly_edited_id,
        |object| object.color[0] = 0.5,
    ));
    sync_canvas_object_views(&mut world);
    let tracked_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
    assert_eq!(
        tracked_batch.entities,
        HashSet::from([directly_edited_entity])
    );
    assert!(!edit_canvas_object(
        &mut world,
        owner,
        directly_edited_id,
        |object| object.id = 0,
    ));
    assert_eq!(
        world.get::<Canvas>(owner).unwrap().objects[1].id,
        directly_edited_id
    );
    world.get_mut::<Canvas>(owner).unwrap().objects[1].color[0] = 0.25;
    sync_canvas_object_views(&mut world);
    let direct_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
    assert_eq!(
        direct_batch.entities,
        HashSet::from([directly_edited_entity])
    );
    let canvas = world.get::<Canvas>(owner).unwrap();
    assert_eq!(canvas.objects[0].origin, [210.0, 0.0]);
    let indexes = world.resource::<CanvasSpatialIndexes>();
    let visible_after = indexes
        .0
        .get(&owner)
        .unwrap()
        .visible_indices(canvas, viewport, clip, 8.0)
        .unwrap();
    assert_eq!(visible_after.len(), 99);
}

#[test]
fn line_move_zoom_anchor_and_serialization_are_stable() {
    let mut canvas = Canvas::default();
    let id = canvas
        .add(
            [0.0, 0.0],
            CanvasShape::Line { end: [100.0, 0.0] },
            [0.5, 0.4, 0.3, 1.0],
        )
        .unwrap();
    canvas.selected.insert(id);
    assert!(canvas.move_selected([10.0, 20.0]));
    assert_eq!(
        canvas.hit_test([50.0, 24.0], UiRect::new(0.0, 0.0, 200.0, 100.0)),
        Some(id)
    );
    let anchor = canvas.screen_to_world([50.0, 40.0]);
    assert!(canvas.zoom_at([50.0, 40.0], 1.5));
    let after = canvas.screen_to_world([50.0, 40.0]);
    assert!((anchor[0] - after[0]).abs() < 0.0001);
    assert!((anchor[1] - after[1]).abs() < 0.0001);
    let restored = Canvas::from_json(&canvas.to_json().unwrap()).unwrap();
    assert_eq!(restored, canvas);
    assert_eq!(
        restored.objects[0].bounds(),
        UiRect::new(10.0, 20.0, 100.0, 0.0)
    );
}

#[test]
fn invalid_edits_and_snapshots_do_not_corrupt_canvas() {
    let mut canvas = Canvas::default();
    assert_eq!(
        canvas.add(
            [0.0, 0.0],
            CanvasShape::Image {
                size: [0.0, 2.0],
                asset: "a.png".to_owned()
            },
            [1.0; 4]
        ),
        None
    );
    assert_eq!(
        canvas.add(
            [0.0, 0.0],
            CanvasShape::Image {
                size: [10.0, 10.0],
                asset: "../outside.png".to_owned(),
            },
            [1.0; 4]
        ),
        None
    );
    assert!(!canvas.pan_by([f32::NAN, 1.0]));
    assert!(!canvas.zoom_at([0.0, 0.0], -1.0));
    assert_eq!(
        canvas.add(
            [f32::MAX * 0.75, 0.0],
            CanvasShape::Rectangle {
                size: [f32::MAX * 0.75, 10.0],
            },
            [1.0; 4],
        ),
        None
    );
    assert_eq!(canvas, Canvas::default());
    let id = canvas
        .add(
            [0.0, 0.0],
            CanvasShape::Image {
                size: [10.0, 10.0],
                asset: "a.png".to_owned(),
            },
            [1.0; 4],
        )
        .unwrap();
    canvas.selected.insert(id);
    canvas.objects[0].locked = true;
    assert_eq!(canvas.remove_selected(), 0);
    assert_eq!(canvas.objects.len(), 1);
    canvas.objects[0].locked = false;
    canvas.objects[0].origin = [f32::MAX * 0.75, 0.0];
    assert!(!canvas.move_selected([f32::MAX * 0.75, 0.0]));
    assert_eq!(canvas.objects[0].origin, [f32::MAX * 0.75, 0.0]);
    let mut saved: serde_json::Value = serde_json::from_str(&canvas.to_json().unwrap()).unwrap();
    saved["objects"][0]["id"] = serde_json::json!(999);
    assert!(Canvas::from_json(&saved.to_string()).is_err());
}

#[test]
fn pointer_move_commits_once_and_cancel_restores_original() {
    let mut world = World::new();
    world.init_resource::<UiRoutedEvents>();
    world.init_resource::<WidgetEventQueue>();
    world.init_resource::<CanvasInputState>();
    let window = world.spawn_empty().id();
    let mut canvas = Canvas::default();
    let id = canvas
        .add(
            [10.0, 10.0],
            CanvasShape::Rectangle { size: [40.0, 40.0] },
            [1.0; 4],
        )
        .unwrap();
    let target = world
        .spawn((
            canvas,
            UiLayoutResult {
                window,
                rect: UiRect::new(100.0, 200.0, 100.0, 100.0),
                clip: None,
                z: 0,
                content_size: [100.0, 100.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    world.resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [120.0, 220.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [130.0, 225.0],
            delta: [10.0, 5.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [130.0, 225.0],
            cancelled: false,
        },
    ];
    process_canvas_input(&mut world);
    let canvas = world.get::<Canvas>(target).unwrap();
    assert_eq!(canvas.objects[0].origin, [20.0, 15.0]);
    assert_eq!(canvas.selected, BTreeSet::from([id]));
    assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 1);

    world.resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [130.0, 225.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [140.0, 240.0],
            delta: [10.0, 15.0],
            dragging: true,
        },
        UiRoutedEvent::PointerCancel { window, target },
    ];
    process_canvas_input(&mut world);
    assert_eq!(
        world.get::<Canvas>(target).unwrap().objects[0].origin,
        [20.0, 15.0]
    );
    assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 1);
    world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target,
        position: [150.0, 250.0],
        delta: [0.0, 2.0],
        unit: UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    process_canvas_input(&mut world);
    assert!(world.get::<Canvas>(target).unwrap().zoom > 1.0);
    assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 2);
    world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target,
        key: UiKeyCode::Home,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    process_canvas_input(&mut world);
    assert_eq!(world.get::<Canvas>(target).unwrap().zoom, 1.0);
    assert_eq!(world.get::<Canvas>(target).unwrap().pan, [0.0, 0.0]);
    assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 3);
}

#[test]
fn routed_keyboard_selects_moves_zooms_resets_deletes_and_cancels_drag() {
    let mut world = World::new();
    world.init_resource::<UiRoutedEvents>();
    world.init_resource::<WidgetEventQueue>();
    world.init_resource::<CanvasInputState>();
    let window = world.spawn_empty().id();
    let mut canvas = Canvas::default();
    let movable = canvas
        .add(
            [10.0, 10.0],
            CanvasShape::Rectangle { size: [20.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    let locked = canvas
        .add(
            [60.0, 10.0],
            CanvasShape::Rectangle { size: [20.0, 20.0] },
            [0.5; 4],
        )
        .unwrap();
    canvas.objects[1].locked = true;
    let target = world
        .spawn((
            canvas,
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z: 0,
                content_size: [100.0, 100.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let key = |key, modifiers| UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    };

    world.resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    )];
    process_canvas_input(&mut world);
    assert_eq!(
        world.get::<Canvas>(target).unwrap().selected,
        BTreeSet::from([movable]),
        "Ctrl+A selects unlocked objects and skips locked objects"
    );

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::ArrowRight, UiModifiers::default())];
    process_canvas_input(&mut world);
    world.resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::ArrowDown,
        UiModifiers {
            shift: true,
            ..Default::default()
        },
    )];
    process_canvas_input(&mut world);
    world.resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::ArrowLeft,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    )];
    process_canvas_input(&mut world);
    assert_eq!(
        world.get::<Canvas>(target).unwrap().objects[0].origin,
        [10.9, 20.0]
    );

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::PageUp, UiModifiers::default())];
    process_canvas_input(&mut world);
    assert!(world.get::<Canvas>(target).unwrap().zoom > 1.0);
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::PageDown, UiModifiers::default())];
    process_canvas_input(&mut world);
    assert!((world.get::<Canvas>(target).unwrap().zoom - 1.0).abs() < 0.0001);

    {
        let mut canvas = world.get_mut::<Canvas>(target).unwrap();
        canvas.pan = [12.0, -4.0];
        canvas.zoom = 2.0;
    }
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::Home, UiModifiers::default())];
    process_canvas_input(&mut world);
    assert_eq!(world.get::<Canvas>(target).unwrap().pan, [0.0, 0.0]);
    assert_eq!(world.get::<Canvas>(target).unwrap().zoom, 1.0);

    let original = world.get::<Canvas>(target).unwrap().objects[0].origin;
    world.resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [original[0] + 2.0, original[1] + 2.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [original[0] + 12.0, original[1] + 2.0],
            delta: [10.0, 0.0],
            dragging: true,
        },
        key(UiKeyCode::Escape, UiModifiers::default()),
    ];
    process_canvas_input(&mut world);
    assert_eq!(
        world.get::<Canvas>(target).unwrap().objects[0].origin,
        original
    );
    assert!(!world.resource::<CanvasInputState>().0.contains_key(&window));

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::Delete, UiModifiers::default())];
    process_canvas_input(&mut world);
    let canvas = world.get::<Canvas>(target).unwrap();
    assert_eq!(canvas.objects.len(), 1);
    assert_eq!(canvas.objects[0].id, locked);
    assert!(canvas.selected.is_empty());
}

#[test]
fn paint_uses_the_same_zoomed_coordinates_and_clip_as_hit_testing() {
    let mut canvas = Canvas::default();
    let id = canvas
        .add(
            [10.0, 20.0],
            CanvasShape::Ellipse { size: [30.0, 40.0] },
            [0.2, 0.4, 0.6, 1.0],
        )
        .unwrap();
    canvas.zoom_at([0.0, 0.0], 2.0);
    canvas.selected.insert(id);
    let rect = UiRect::new(100.0, 200.0, 200.0, 150.0);
    let clip = ClipRect::from(UiRect::new(100.0, 200.0, 150.0, 100.0));
    let mut items = Vec::new();
    paint_canvas(&canvas, rect, clip, 5, &Theme::default(), None, &mut items);
    assert!(items.iter().all(|item| item.clip == clip));
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::Ellipse { rect: shape, .. }
            if shape.x == 120.0 && shape.y == 240.0 && shape.width == 60.0 && shape.height == 80.0
    )));
    assert_eq!(
        canvas.hit_test([40.0, 60.0], UiRect::new(0.0, 0.0, 200.0, 150.0)),
        Some(id)
    );
    canvas
        .add(
            [50.0, 10.0],
            CanvasShape::Image {
                size: [20.0, 30.0],
                asset: "images/sample.png".to_owned(),
            },
            [1.0; 4],
        )
        .unwrap();
    let mut images = CanvasImageAssets::default();
    images.ids.insert("images/sample.png".to_owned(), 57);
    images
        .states
        .insert("images/sample.png".to_owned(), CanvasImageState::Ready);
    items.clear();
    paint_canvas(
        &canvas,
        rect,
        clip,
        5,
        &Theme::default(),
        Some(&images),
        &mut items,
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Icon { asset_id: 57, .. }))
    );
    images.states.insert(
        "images/sample.png".to_owned(),
        CanvasImageState::Failed("missing file".to_owned()),
    );
    items.clear();
    paint_canvas(
        &canvas,
        rect,
        clip,
        5,
        &Theme::default(),
        Some(&images),
        &mut items,
    );
    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::Text { text, .. } if text.contains("Image error: missing file")
    )));
}

#[test]
fn paint_culls_objects_outside_the_clipped_viewport() {
    let mut canvas = Canvas::default();
    let visible = canvas
        .add(
            [5.0, 5.0],
            CanvasShape::Rectangle { size: [10.0, 10.0] },
            [1.0; 4],
        )
        .unwrap();
    for x in 0..10_000 {
        let id = canvas
            .add(
                [200.0 + x as f32 * 20.0, 5.0],
                CanvasShape::Rectangle { size: [10.0, 10.0] },
                [1.0; 4],
            )
            .unwrap();
        canvas.selected.insert(id);
    }
    canvas.selected.insert(visible);
    let mut items = Vec::new();
    paint_canvas(
        &canvas,
        UiRect::new(0.0, 0.0, 100.0, 100.0),
        ClipRect::from(UiRect::new(0.0, 0.0, 50.0, 50.0)),
        0,
        &Theme::default(),
        None,
        &mut items,
    );
    assert_eq!(items.len(), 7); // background, object, selected fill, and four edges
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { color, .. }
            if color == Theme::default().color(ThemeColorToken::CanvasSelectionOverlay)
    )));
}

#[test]
fn paint_preserves_a_line_touching_the_viewport_edge() {
    let mut canvas = Canvas::default();
    canvas
        .add(
            [-10.0, 20.0],
            CanvasShape::Line { end: [0.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    let mut items = Vec::new();
    paint_canvas(
        &canvas,
        UiRect::new(0.0, 0.0, 100.0, 100.0),
        ClipRect::from(UiRect::new(0.0, 0.0, 50.0, 50.0)),
        0,
        &Theme::default(),
        None,
        &mut items,
    );
    assert_eq!(items.len(), 2);
    assert!(matches!(items[1].primitive, Primitive::Line { .. }));
}

#[test]
fn paint_skips_finite_geometry_that_overflows_the_view_transform() {
    let mut canvas = Canvas::default();
    canvas
        .add(
            [f32::MAX / 4.0, 0.0],
            CanvasShape::Rectangle { size: [10.0, 10.0] },
            [1.0; 4],
        )
        .unwrap();
    assert!(canvas.zoom_at([0.0, 0.0], 32.0));
    let mut items = Vec::new();
    paint_canvas(
        &canvas,
        UiRect::new(0.0, 0.0, 100.0, 100.0),
        ClipRect::from(UiRect::new(0.0, 0.0, 50.0, 50.0)),
        0,
        &Theme::default(),
        None,
        &mut items,
    );
    assert_eq!(items.len(), 1);
}

#[test]
fn image_assets_follow_viewport_and_visibility() {
    let mut world = World::new();
    let window = world.spawn_empty().id();
    let mut canvas = Canvas::default();
    for (asset, origin) in [
        ("visible.png", [10.0, 10.0]),
        ("offscreen.png", [300.0, 300.0]),
    ] {
        canvas
            .add(
                origin,
                CanvasShape::Image {
                    size: [40.0, 40.0],
                    asset: asset.to_owned(),
                },
                [1.0; 4],
            )
            .unwrap();
    }
    let entity = world
        .spawn((
            canvas,
            UiVisible(true),
            UiLayoutResult {
                window,
                rect: UiRect::new(100.0, 200.0, 100.0, 100.0),
                clip: Some(UiRect::new(100.0, 200.0, 80.0, 80.0)),
                z: 0,
                content_size: [100.0, 100.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    assert_eq!(
        visible_canvas_image_paths(&mut world),
        HashSet::from(["visible.png".to_owned()])
    );
    world.get_mut::<UiVisible>(entity).unwrap().0 = false;
    assert!(visible_canvas_image_paths(&mut world).is_empty());
}

#[test]
fn modifier_annotated_click_adds_to_canvas_selection_once() {
    let mut world = World::new();
    world.init_resource::<UiRoutedEvents>();
    world.init_resource::<WidgetEventQueue>();
    world.init_resource::<CanvasInputState>();
    let window = world.spawn_empty().id();
    let mut canvas = Canvas::default();
    let first = canvas
        .add(
            [0.0, 0.0],
            CanvasShape::Rectangle { size: [20.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    let second = canvas
        .add(
            [40.0, 0.0],
            CanvasShape::Rectangle { size: [20.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    let target = world
        .spawn((
            canvas,
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z: 0,
                content_size: [100.0, 100.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    for (position, modifiers) in [
        ([10.0, 10.0], UiModifiers::default()),
        (
            [50.0, 10.0],
            UiModifiers {
                control: true,
                ..Default::default()
            },
        ),
    ] {
        world.resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
            },
            UiRoutedEvent::PointerDownWithModifiers {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
                modifiers,
            },
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
                cancelled: false,
            },
        ];
        process_canvas_input(&mut world);
    }
    assert_eq!(
        world.get::<Canvas>(target).unwrap().selected,
        BTreeSet::from([first, second])
    );
    assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 2);
    world.resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [10.0, 10.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [20.0, 15.0],
            delta: [10.0, 5.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [20.0, 15.0],
            cancelled: false,
        },
    ];
    process_canvas_input(&mut world);
    let canvas = world.get::<Canvas>(target).unwrap();
    assert_eq!(canvas.selected, BTreeSet::from([first, second]));
    assert_eq!(canvas.objects[0].origin, [10.0, 5.0]);
    assert_eq!(canvas.objects[1].origin, [50.0, 5.0]);
    assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 3);
}

#[test]
fn object_controls_keep_identity_and_virtualize_offscreen_content() {
    let mut world = World::new();
    world.init_resource::<CanvasObjectItems>();
    let window = world.spawn_empty().id();
    let mut canvas = Canvas::default();
    let visible = canvas
        .add(
            [10.0, 10.0],
            CanvasShape::Rectangle { size: [20.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    canvas
        .add(
            [1000.0, 1000.0],
            CanvasShape::Ellipse { size: [20.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    let owner = world
        .spawn((
            UiNode,
            canvas,
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z: 3,
                content_size: [100.0, 100.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    sync_canvas_object_views(&mut world);
    let controls = &world.resource::<CanvasObjectItems>().0[&owner];
    assert_eq!(controls.len(), 1);
    let entity = controls[&visible];
    assert_eq!(
        world.get::<CanvasObjectControl>(entity).unwrap().owner,
        owner
    );
    assert_eq!(world.get::<UiParent>(entity).unwrap().0, owner);
    world.init_resource::<UiRoutedEvents>();
    world.init_resource::<WidgetEventQueue>();
    world.init_resource::<CanvasInputState>();
    world.resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: [20.0, 20.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: entity,
            position: [25.0, 20.0],
            delta: [5.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: [25.0, 20.0],
            cancelled: false,
        },
    ];
    process_canvas_input(&mut world);
    assert_eq!(
        world.get::<Canvas>(owner).unwrap().objects[0].origin,
        [15.0, 10.0]
    );
    world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: entity,
        key: UiKeyCode::ArrowRight,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    process_canvas_input(&mut world);
    assert_eq!(
        world.get::<Canvas>(owner).unwrap().objects[0].origin,
        [16.0, 10.0]
    );
    sync_canvas_object_views(&mut world);
    assert_eq!(
        world.resource::<CanvasObjectItems>().0[&owner][&visible],
        entity
    );
    assert!(world.get::<CanvasObjectControl>(entity).unwrap().selected);
    world.get_mut::<Canvas>(owner).unwrap().pan = [-500.0, -500.0];
    sync_canvas_object_views(&mut world);
    assert!(world.resource::<CanvasObjectItems>().0[&owner].is_empty());
    assert!(world.get_entity(entity).is_err());
}

#[test]
fn spatial_indexes_are_pruned_when_canvas_owners_are_despawned() {
    let mut world = World::new();
    world.init_resource::<CanvasSpatialIndexes>();
    world.init_resource::<CanvasObjectItems>();
    let window = world.spawn_empty().id();
    let owner = world
        .spawn((
            UiNode,
            Canvas::default(),
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z: 0,
                content_size: [100.0, 100.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    sync_canvas_object_views(&mut world);
    assert!(
        world
            .resource::<CanvasSpatialIndexes>()
            .0
            .contains_key(&owner)
    );

    world.despawn(owner);
    sync_canvas_object_views(&mut world);

    assert!(
        !world
            .resource::<CanvasSpatialIndexes>()
            .0
            .contains_key(&owner)
    );
    assert!(!world.resource::<CanvasObjectItems>().0.contains_key(&owner));
}

use super::*;

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

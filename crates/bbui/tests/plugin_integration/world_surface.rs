use super::*;

#[test]
fn crosshair_surface_respects_occlusion_and_keeps_a_drag_outside_the_mesh() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.world_mut().init_resource::<Assets<Mesh>>();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(400.0, 200.0, 0.05));
    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    let camera = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Camera>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(surface).insert((
        Mesh3d(mesh.clone()),
        Aabb::from_min_max(
            Vec3::new(-200.0, -100.0, -0.025),
            Vec3::new(200.0, 100.0, 0.025),
        ),
        InheritedVisibility::VISIBLE,
        ViewVisibility::VISIBLE,
    ));
    app.world_mut()
        .entity_mut(camera)
        .insert(UiCrosshairPointer { max_distance: 20.0 });
    app.update();
    assert_eq!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .map(|(target, _)| target),
        Some(scene.logical_window)
    );

    let blocker = app
        .world_mut()
        .spawn((
            Mesh3d(mesh),
            Aabb::from_min_max(
                Vec3::new(-200.0, -100.0, -0.025),
                Vec3::new(200.0, 100.0, 0.025),
            ),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
            InheritedVisibility::VISIBLE,
            ViewVisibility::VISIBLE,
        ))
        .id();
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
    app.world_mut().despawn(blocker);
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_some()
    );

    let field_center = center(&app, scene.text_field);
    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(
            field_center.x - 200.0,
            100.0 - field_center.y,
            10.0,
        )));
    app.update();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut()
            .write_message(bevy::input::mouse::MouseButtonInput {
                button: bevy::input::mouse::MouseButton::Left,
                state,
                window: scene.physical_window,
            });
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(scene.logical_window)
            .and_then(|focus| focus.focused),
        Some(scene.text_field)
    );

    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
    app.update();

    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Pressed,
            window: scene.physical_window,
        });
    app.update();
    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(
            300.0, 0.0, 10.0,
        )));
    app.update();
    let (_, position) = app
        .world()
        .resource::<WorldPointerState>()
        .hover_target(scene.physical_window)
        .unwrap();
    assert!(
        position[0] > 400.0,
        "captured world drag should keep unbounded UI coordinates"
    );

    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Released,
            window: scene.physical_window,
        });
    app.update();
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
}

#[test]
fn crosshair_camera_deactivation_clears_world_hover() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(400.0, 200.0, 0.05));
    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    let camera = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Camera>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(surface).insert((
        Mesh3d(mesh),
        Aabb::from_min_max(
            Vec3::new(-200.0, -100.0, -0.025),
            Vec3::new(200.0, 100.0, 0.025),
        ),
        InheritedVisibility::VISIBLE,
        ViewVisibility::VISIBLE,
    ));
    app.world_mut()
        .entity_mut(camera)
        .insert(UiCrosshairPointer { max_distance: 20.0 });
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_some()
    );
    app.world_mut().get_mut::<Camera>(camera).unwrap().is_active = false;
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
}

#[test]
fn removed_world_surface_clears_stationary_pointer_hover() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();
    let position = screen_position_for_surface_ui(&scene, center(&app, scene.text_field));
    app.world_mut().write_message(CursorMoved {
        window: scene.physical_window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_some()
    );
    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(surface)
        .remove::<UiWorldSurface>();
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
}

#[test]
fn world_surface_routes_pointer_wheel_keyboard_text_and_ime_to_its_camera() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();

    let ui_position = center(&app, scene.text_field);
    let position = screen_position_for_surface_ui(&scene, ui_position);
    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.world_mut().write_message(CursorMoved {
        window: scene.physical_window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.update();

    app.world_mut()
        .write_message(bevy::input::mouse::MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: scene.physical_window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    app.update();
    assert!(
        app.world()
            .resource::<UiRoutedEvents>()
            .events
            .iter()
            .any(|event| {
                matches!(event, UiRoutedEvent::Wheel { window, target, .. }
            if *window == scene.logical_window && *target == scene.text_field)
            })
    );

    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Pressed,
            window: scene.physical_window,
        });
    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Released,
            window: scene.physical_window,
        });
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(scene.logical_window)
            .unwrap()
            .focused,
        Some(scene.text_field)
    );

    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyX,
        logical_key: bevy::input::keyboard::Key::Character("x".into()),
        state: ButtonState::Pressed,
        text: Some("x".into()),
        repeat: false,
        window: scene.physical_window,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .as_str(),
        "draftx"
    );

    app.world_mut().write_message(Ime::Preedit {
        window: scene.physical_window,
        value: "かな".into(),
        cursor: Some((0, 2)),
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .composition()
            .unwrap()
            .text,
        "かな"
    );
    app.world_mut().write_message(Ime::Commit {
        window: scene.physical_window,
        value: "かな".into(),
    });
    app.update();
    assert_eq!(
        app.world().get::<TextField>(scene.text_field).unwrap().text,
        "draftxかな"
    );
}

#[test]
fn world_surface_cursor_capture_uses_the_physical_window_and_routes_motion() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();

    let ui_position = center(&app, scene.text_field);
    let position = screen_position_for_surface_ui(&scene, ui_position);
    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.world_mut().write_message(CursorMoved {
        window: scene.physical_window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.update();
    let initial_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();

    app.world_mut()
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::Begin {
            window: scene.logical_window,
            target: scene.text_field,
            hide_cursor: true,
            continuous: true,
        });
    app.update();
    let cursor = app
        .world()
        .get::<CursorOptions>(scene.physical_window)
        .unwrap();
    assert!(!cursor.visible);
    assert_eq!(cursor.grab_mode, bevy::window::CursorGrabMode::Locked);

    app.world_mut()
        .write_message(bevy::input::mouse::MouseMotion {
            delta: Vec2::new(7.0, 0.0),
        });
    app.update();
    let routed_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();
    assert_eq!(
        routed_position,
        [initial_position[0] + 7.0, initial_position[1]]
    );

    // Raw mouse motion is physical pixels. A captured world-surface pointer
    // must use the current physical window's DPI even while the OS cursor is
    // hidden and the virtual pointer is routed to another UI window.
    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(2.0));
    app.world_mut()
        .write_message(bevy::input::mouse::MouseMotion {
            delta: Vec2::new(12.0, 0.0),
        });
    app.update();
    let high_dpi_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();
    assert_eq!(
        high_dpi_position,
        [routed_position[0] + 6.0, routed_position[1]]
    );

    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(1.0));
    app.world_mut()
        .write_message(bevy::input::mouse::MouseMotion {
            delta: Vec2::new(12.0, 0.0),
        });
    app.update();
    let low_dpi_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();
    assert_eq!(
        low_dpi_position,
        [high_dpi_position[0] + 12.0, high_dpi_position[1]]
    );

    app.world_mut()
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::End {
            window: scene.logical_window,
            target: scene.text_field,
            restore_position: true,
        });
    app.update();
    let cursor = app
        .world()
        .get::<CursorOptions>(scene.physical_window)
        .unwrap();
    assert!(cursor.visible);
    assert_eq!(cursor.grab_mode, bevy::window::CursorGrabMode::None);
}

#[test]
fn window_resize_and_dpi_refresh_layout_text_clip_hitbox_and_render_in_one_update() {
    let mut app = test_app();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    app.add_systems(Update, load_font_assets_into_font_collection);
    app.update();

    let mut window = Window::default();
    window.resolution.set(400.0, 200.0);
    let window = app.world_mut().spawn(window).id();
    let root = WidgetTree::new(WidgetNode::column([WidgetNode::text_area(
        "Memo",
        "Responsive text wraps consistently as the window changes size and scale.",
    )]))
    .spawn(app.world_mut(), None)
    .unwrap();
    let mut root_node = *app.world().get::<UiLayoutNode>(root).unwrap();
    root_node.clip_contents = true;
    root_node.align_items = UiAlign::Start;
    root_node.mode = UiLayoutMode::Overlay;
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 400.0, 200.0), 1.0),
        root_node,
    ));
    let area = app.world().get::<UiChildren>(root).unwrap().0[0];
    app.world_mut().get_mut::<UiTextLayout>(area).unwrap().font =
        Some(TextFont::from_font_size(14.0).with_font(font));

    app.update();

    let initial_root = *app.world().get::<UiLayoutResult>(root).unwrap();
    let initial_text = app
        .world()
        .get::<UiTextLayout>(area)
        .unwrap()
        .layout()
        .unwrap()
        .bounds();
    assert_eq!(initial_root.rect, Rect::new(0.0, 0.0, 400.0, 200.0));
    assert_eq!(initial_root.dpi_scale, 1.0);
    assert!(initial_text.width > 0.0);

    let mut window_mut = app.world_mut().get_mut::<Window>(window).unwrap();
    window_mut.resolution.set_physical_resolution(320, 160);
    window_mut.resolution.set_scale_factor_override(Some(2.0));

    app.update();

    let synced_root = *app.world().get::<UiLayoutRoot>(root).unwrap();
    let root_layout = *app.world().get::<UiLayoutResult>(root).unwrap();
    let area_layout = *app.world().get::<UiLayoutResult>(area).unwrap();
    let hit_box = *app.world().get::<UiHitBox>(area).unwrap();
    assert_eq!(synced_root.viewport, Rect::new(0.0, 0.0, 160.0, 80.0));
    assert_eq!(synced_root.dpi_scale, 2.0);
    assert_eq!(root_layout.rect, synced_root.viewport);
    assert_eq!(root_layout.dpi_scale, 2.0);
    assert_eq!(area_layout.dpi_scale, 2.0);
    assert_eq!(area_layout.rect, Rect::new(0.0, 0.0, 220.0, 96.0));
    assert_eq!(area_layout.clip, Some(synced_root.viewport));
    assert_eq!(hit_box.rect, area_layout.rect);
    assert_eq!(hit_box.clip, area_layout.clip);
    assert!(hit_box.rect.contains(170.0, 10.0));
    assert!(!hit_box.contains([170.0, 10.0]));

    let text_layout = app.world().get::<UiTextLayout>(area).unwrap();
    let resized_text = text_layout.layout().unwrap().bounds();
    assert!((resized_text.width / initial_text.width - 2.0).abs() < 0.05);
    assert!((resized_text.height / initial_text.height - 2.0).abs() < 0.05);

    let (paint_clip, paint_width, paint_text) = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .iter()
        .find_map(|item| match &item.primitive {
            Primitive::TextWrapped {
                text, max_width, ..
            } if text.starts_with("Memo:") => Some((item.clip, *max_width, text.clone())),
            _ => None,
        })
        .expect("text area should be present in the same-frame display list");
    assert_eq!(
        paint_clip,
        bbui_render::ClipRect::from(synced_root.viewport)
    );

    let mut rendered_text = app.world_mut().query::<(&Text, &Node, &TextBounds)>();
    let rendered = rendered_text
        .iter(app.world())
        .find(|(text, _, bounds)| text.0 == paint_text && bounds.width == Some(paint_width))
        .expect("renderer should publish the resized text node in the same update");
    assert_eq!(rendered.1.width, bevy::ui::Val::Px(paint_width));
}

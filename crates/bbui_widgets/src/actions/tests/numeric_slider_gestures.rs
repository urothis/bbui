use super::*;

#[test]
pub(super) fn numeric_scrub_commits_and_requests_cursor_capture_once() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            NumericField::new(1.0, None, None, 0.1, NumericFieldKind::Float).unwrap(),
        ))
        .id();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [5.0, 5.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [10.0, 5.0],
            delta: [5.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [10.0, 5.0],
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(app.world().get::<NumericField>(target).unwrap().value, 1.5);
    let requests: Vec<_> = app
        .world_mut()
        .resource_mut::<CursorCaptureRequestQueue>()
        .drain()
        .collect();
    assert_eq!(
        requests,
        [
            CursorCaptureRequest::Begin {
                window,
                target,
                hide_cursor: true,
                continuous: true
            },
            CursorCaptureRequest::End {
                window,
                target,
                restore_position: true
            },
        ]
    );
}

#[test]
pub(super) fn slider_routes_drag_keyboard_wheel_cancellation_and_numeric_actions() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let rect = UiRect::new(0.0, 0.0, 260.0, 32.0);
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            UiVisible(true),
            UiEnabled(true),
            UiFocusable,
            Label::new("Opacity"),
            Slider::new(0.2, 0.0, 1.0, 0.1).unwrap(),
            UiHitBox {
                window: Some(window),
                rect,
                clip: None,
                z_index: 1,
            },
            UiLayoutResult {
                window,
                rect,
                clip: None,
                z: 1,
                content_size: [rect.width, rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();
    let initial_paint = app.world().resource::<DisplayList>().widget_items();
    assert!(
        initial_paint
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Ellipse { .. }))
    );
    assert!(
        initial_paint
            .iter()
            .any(|item| matches!(item.primitive, Primitive::RoundedRect { .. }))
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [130.0, 24.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [220.0, 24.0],
            delta: [90.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [220.0, 24.0],
            cancelled: false,
        },
    ];
    app.update();
    app.world_mut().resource_mut::<UiRoutedEvents>().clear();
    let dragged_value = app.world().get::<Slider>(target).unwrap().value;
    assert!(dragged_value > 0.8 && dragged_value <= 1.0);
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::NumericCommitted { target: event_target, before, after }
                if *event_target == target && before == &[0.2] && after == &[dragged_value])
        ));
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);
    app.world_mut().resource_mut::<WidgetEventQueue>().take();

    let key = |key, modifiers| UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    )];
    app.update();
    assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.2);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            shift: true,
            ..UiModifiers::default()
        },
    )];
    app.update();
    assert_eq!(
        app.world().get::<Slider>(target).unwrap().value,
        dragged_value
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::Home, UiModifiers::default())];
    app.update();
    assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.0);
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::ArrowRight, UiModifiers::default())];
    app.update();
    assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.1);
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::End, UiModifiers::default())];
    app.update();
    assert_eq!(app.world().get::<Slider>(target).unwrap().value, 1.0);

    app.world_mut().resource_mut::<UiRoutedEvents>().clear();
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(target, WidgetActionKind::SetNumber(0.5)));
    app.update();
    assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.5);
    app.world_mut().resource_mut::<UiRoutedEvents>().clear();
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(target, WidgetActionKind::StepNumber(2)));
    app.update();
    assert!((app.world().get::<Slider>(target).unwrap().value - 0.7).abs() < 1e-9);
    app.world_mut().resource_mut::<UiRoutedEvents>().clear();
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(target, WidgetActionKind::ResetNumber));
    app.update();
    assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.2);
    app.world_mut().resource_mut::<UiRoutedEvents>().clear();
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(target, WidgetActionKind::SetNumber(0.5)));
    app.update();
    app.world_mut().resource_mut::<UiRoutedEvents>().clear();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target,
        position: [130.0, 24.0],
        delta: [0.0, 1.0],
        unit: bbui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert!((app.world().get::<Slider>(target).unwrap().value - 0.6).abs() < 1e-9);
    app.world_mut().resource_mut::<UiRoutedEvents>().clear();

    let before_cancel = app.world().get::<Slider>(target).unwrap().value;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target,
            button: UiPointerButton::Primary,
            position: [60.0, 24.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target,
            position: [210.0, 24.0],
            delta: [150.0, 0.0],
            dragging: true,
        },
        key(UiKeyCode::Escape, UiModifiers::default()),
    ];
    app.update();
    assert_eq!(
        app.world().get::<Slider>(target).unwrap().value,
        before_cancel
    );
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::NumericCancelled { target: event_target, restored }
                if *event_target == target && restored == &[before_cancel])
    ));

    app.world_mut().get_mut::<Slider>(target).unwrap().enabled = false;
    app.world_mut().resource_mut::<UiRoutedEvents>().events =
        vec![key(UiKeyCode::End, UiModifiers::default())];
    app.update();
    assert_eq!(
        app.world().get::<Slider>(target).unwrap().value,
        before_cancel
    );
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(target, WidgetActionKind::SetNumber(0.1)));
    app.update();
    assert_eq!(
        app.world().get::<Slider>(target).unwrap().value,
        before_cancel
    );

    app.world_mut().get_mut::<Slider>(target).unwrap().enabled = true;
    app.world_mut().get_mut::<Slider>(target).unwrap().read_only = true;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(target, WidgetActionKind::StepNumber(1)));
    app.update();
    assert_eq!(
        app.world().get::<Slider>(target).unwrap().value,
        before_cancel
    );
}

#[test]
pub(super) fn slider_paint_uses_runtime_theme_and_disabled_semantic_tokens() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let rect = UiRect::new(0.0, 0.0, 260.0, 32.0);
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            UiVisible(true),
            UiEnabled(true),
            UiFocusable,
            Label::new("Opacity"),
            Slider::new(0.4, 0.0, 1.0, 0.1).unwrap(),
            UiHitBox {
                window: Some(window),
                rect,
                clip: None,
                z_index: 1,
            },
            UiLayoutResult {
                window,
                rect,
                clip: None,
                z: 1,
                content_size: [rect.width, rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();
    let handle_color = |world: &World| {
        world
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .find_map(|item| match item.primitive {
                Primitive::Ellipse { color, .. } => Some(color),
                _ => None,
            })
            .expect("slider paint includes a GPU ellipse handle")
    };
    let dark = handle_color(app.world());
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(bbui_style::ThemeKind::Light);
    app.update();
    let light = handle_color(app.world());
    assert_ne!(dark, light, "theme switch invalidates slider handle paint");

    app.world_mut().get_mut::<Slider>(target).unwrap().enabled = false;
    app.update();
    let expected = app
        .world()
        .resource::<Theme>()
        .color(ThemeColorToken::ControlDisabledMark);
    assert_eq!(handle_color(app.world()), expected);
}

#[test]
pub(super) fn numeric_group_scrub_selects_compatible_rows_and_undoes_as_one_transaction() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let spawn_field = |world: &mut World, y: f32, field: NumericField| {
        world
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(0.0, y, 100.0, 18.0),
                    clip: None,
                    z_index: 0,
                },
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, y, 100.0, 18.0),
                    clip: None,
                    z: 0,
                    content_size: [100.0, 18.0],
                    dpi_scale: 1.0,
                },
                field,
            ))
            .id()
    };
    let first = spawn_field(
        app.world_mut(),
        0.0,
        NumericField::new(0.3, Some(0.0), Some(1.0), 0.2, NumericFieldKind::Float).unwrap(),
    );
    let second = spawn_field(
        app.world_mut(),
        20.0,
        NumericField::new(10.2, Some(10.0), Some(10.6), 0.5, NumericFieldKind::Float).unwrap(),
    );
    let integer = spawn_field(app.world_mut(), 40.0, NumericField::integer(4));
    let mut other_unit = NumericField::float(2.0).unwrap();
    other_unit.unit = Some("m".into());
    let unit_field = spawn_field(app.world_mut(), 60.0, other_unit);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: first,
            button: UiPointerButton::Primary,
            position: [8.0, 9.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [8.0, 69.0],
            delta: [0.0, 60.0],
            dragging: true,
        },
    ];
    app.update();
    let scrub = app
        .world()
        .resource::<WidgetInteractionState>()
        .numeric_scrubs
        .get(&window)
        .expect("the vertical drag keeps a group selection active");
    assert_eq!(scrub.targets, [first, second]);
    let selected_surface = app
        .world()
        .resource::<Theme>()
        .color(ThemeColorToken::EditorActiveFieldSurface);
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(item.primitive,
                Primitive::RoundedRect { rect, color, .. }
                    if rect.x == 0.0 && rect.y == 20.0 && color == selected_surface))
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Other(0),
            pressed: false,
            repeat: false,
            modifiers: UiModifiers {
                shift: true,
                ..UiModifiers::default()
            },
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [10.0, 69.0],
            delta: [2.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Other(0),
            pressed: false,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [12.0, 69.0],
            delta: [2.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Other(0),
            pressed: false,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [12.0, 69.0],
            delta: [0.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: first,
            button: UiPointerButton::Primary,
            position: [12.0, 69.0],
            cancelled: false,
        },
    ];
    app.update();

    assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 0.8);
    assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 10.6);
    assert_eq!(app.world().get::<NumericField>(integer).unwrap().value, 4.0);
    assert_eq!(
        app.world().get::<NumericField>(unit_field).unwrap().value,
        2.0
    );
    let changes = app
        .world()
        .resource::<WidgetEventQueue>()
        .0
        .iter()
        .find_map(|event| match event {
            WidgetEvent::NumericGroupCommitted { changes } => Some(changes),
            _ => None,
        })
        .expect("one group commit is emitted for the whole drag");
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .any(|(entity, before, after)| { *entity == first && *before == 0.3 && *after == 0.8 })
    );
    assert!(
        changes.iter().any(|(entity, before, after)| {
            *entity == second && *before == 10.2 && *after == 10.6
        })
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: first,
        key: UiKeyCode::Z,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 0.3);
    assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 10.2);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: first,
        key: UiKeyCode::Y,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 0.8);
    assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 10.6);
}

#[test]
pub(super) fn numeric_group_escape_restores_every_selected_value_and_ends_capture() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let spawn_field = |world: &mut World, y: f32, value: f64| {
        world
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(0.0, y, 100.0, 18.0),
                    clip: None,
                    z_index: 0,
                },
                NumericField::new(value, None, None, 1.0, NumericFieldKind::Float).unwrap(),
            ))
            .id()
    };
    let first = spawn_field(app.world_mut(), 0.0, 2.0);
    let second = spawn_field(app.world_mut(), 20.0, 8.0);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: first,
            button: UiPointerButton::Primary,
            position: [8.0, 9.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [8.0, 29.0],
            delta: [0.0, 20.0],
            dragging: true,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [13.0, 29.0],
            delta: [5.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
        UiRoutedEvent::PointerUp {
            window,
            target: first,
            button: UiPointerButton::Primary,
            position: [13.0, 29.0],
            cancelled: false,
        },
    ];
    app.update();
    assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 2.0);
    assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 8.0);
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::NumericCancelled { target, restored }
                if *target == first && restored == &[2.0, 8.0])
    ));
    assert!(
        !app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::NumericGroupCommitted { .. }))
    );
    let requests: Vec<_> = app
        .world_mut()
        .resource_mut::<CursorCaptureRequestQueue>()
        .drain()
        .collect();
    assert_eq!(requests.len(), 2);
    assert!(matches!(requests[0], CursorCaptureRequest::Begin { target, .. } if target == first));
    assert!(matches!(requests[1], CursorCaptureRequest::End { target, .. } if target == first));
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
}

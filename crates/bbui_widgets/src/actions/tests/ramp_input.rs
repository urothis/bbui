use super::*;

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

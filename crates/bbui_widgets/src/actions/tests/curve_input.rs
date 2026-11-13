use super::*;

#[test]
pub(super) fn curve_editor_shift_drag_adds_one_undoable_point_transaction() {
    let mut app = App::new();
    let rect = UiRect::new(0.0, 0.0, 320.0, 240.0);
    let (window, entity) = spawn_curve_editor_widget(&mut app, rect);
    let plot = curve_plot_rect(rect, &Theme::default());
    let start = [plot.x + plot.width * 0.5, plot.y + plot.height * 0.5];
    let end = [plot.x + plot.width * 0.68, plot.y + plot.height * 0.28];
    app.world_mut()
        .resource_mut::<WidgetInteractionState>()
        .modifiers
        .insert(
            window,
            UiModifiers {
                shift: true,
                ..Default::default()
            },
        );
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
            delta: [end[0] - start[0], end[1] - start[1]],
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

    let editor = app.world().get::<CurveEditor>(entity).unwrap();
    assert_eq!(editor.points.len(), 3);
    let selected = editor
        .points
        .iter()
        .find(|point| Some(point.id) == editor.selected)
        .unwrap();
    assert!((selected.position[0] - 0.68).abs() < 0.002);
    assert!((selected.position[1] - 0.72).abs() < 0.002);
    let commits = app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .into_iter()
            .filter(|event| matches!(event, WidgetEvent::CurveEditorCommitted { target, .. } if *target == entity))
            .count();
    assert_eq!(commits, 1);
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);

    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    );
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().points.len(),
        2
    );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().points.len(),
        3
    );
}

#[test]
pub(super) fn curve_editor_keyboard_cycles_selection_and_edits_points() {
    let mut app = App::new();
    let (window, entity) = spawn_curve_editor_widget(&mut app, UiRect::new(0.0, 0.0, 320.0, 240.0));
    let control = UiModifiers {
        control: true,
        ..Default::default()
    };
    route_curve_key(&mut app, window, entity, UiKeyCode::ArrowRight, control);
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().selected,
        Some(1)
    );
    route_curve_key(&mut app, window, entity, UiKeyCode::ArrowRight, control);
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().selected,
        Some(2)
    );
    route_curve_key(&mut app, window, entity, UiKeyCode::ArrowLeft, control);
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().selected,
        Some(1)
    );

    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::A,
        UiModifiers::default(),
    );
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().points.len(),
        3
    );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::C,
        UiModifiers::default(),
    );
    assert_eq!(
        app.world()
            .get::<CurveEditor>(entity)
            .unwrap()
            .interpolation,
        Interpolation::Smooth
    );
    let before_y = app
        .world()
        .get::<CurveEditor>(entity)
        .unwrap()
        .points
        .iter()
        .find(|point| Some(point.id) == app.world().get::<CurveEditor>(entity).unwrap().selected)
        .unwrap()
        .position[1];
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::ArrowUp,
        UiModifiers::default(),
    );
    let editor = app.world().get::<CurveEditor>(entity).unwrap();
    let selected = editor
        .points
        .iter()
        .find(|point| Some(point.id) == editor.selected)
        .unwrap();
    assert!(selected.position[1] > before_y);
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::Home,
        UiModifiers::default(),
    );
    assert_eq!(
            app.world()
                .get::<CurveEditor>(entity)
                .unwrap()
                .points
                .iter()
                .find(|point| Some(point.id)
                    == app.world().get::<CurveEditor>(entity).unwrap().selected)
                .unwrap()
                .position[0],
            0.0
        );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::End,
        UiModifiers::default(),
    );
    assert_eq!(
            app.world()
                .get::<CurveEditor>(entity)
                .unwrap()
                .points
                .iter()
                .find(|point| Some(point.id)
                    == app.world().get::<CurveEditor>(entity).unwrap().selected)
                .unwrap()
                .position[0],
            1.0
        );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::Home,
        UiModifiers::default(),
    );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::ArrowRight,
        UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::ArrowRight,
        UiModifiers::default(),
    );
    let editor = app.world().get::<CurveEditor>(entity).unwrap();
    let selected = editor
        .points
        .iter()
        .find(|point| Some(point.id) == editor.selected)
        .unwrap();
    assert!(
        (selected.position[0] - 0.11).abs() < 0.001,
        "Shift arrow precision should accumulate to 0.11, got {}",
        selected.position[0]
    );
    route_curve_key(
        &mut app,
        window,
        entity,
        UiKeyCode::Delete,
        UiModifiers::default(),
    );
    assert_eq!(
        app.world().get::<CurveEditor>(entity).unwrap().points.len(),
        2
    );
}

#[test]
pub(super) fn curve_editor_handle_drag_cancels_or_commits_and_view_zooms_and_pans() {
    let mut app = App::new();
    let rect = UiRect::new(0.0, 0.0, 360.0, 240.0);
    let (window, entity) = spawn_curve_editor_widget(&mut app, rect);
    let id = {
        let mut editor = app.world_mut().get_mut::<CurveEditor>(entity).unwrap();
        editor.interpolation = Interpolation::Smooth;
        let id = editor.add_point(0.5, 0.5).unwrap();
        editor.selected = Some(id);
        id
    };
    let plot = curve_plot_rect(rect, &Theme::default());
    let start = curve_screen_position(
        app.world().get::<CurveEditor>(entity).unwrap(),
        plot,
        [0.65, 0.5],
    );
    let end = [start[0] + 12.0, start[1] - 8.0];
    let original_handle = app
        .world()
        .get::<CurveEditor>(entity)
        .unwrap()
        .points
        .iter()
        .find(|point| point.id == id)
        .unwrap()
        .right_handle;

    for cancelled in [true, false] {
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
                delta: [end[0] - start[0], end[1] - start[1]],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: end,
                cancelled,
            },
        ];
        app.update();
        let point = app
            .world()
            .get::<CurveEditor>(entity)
            .unwrap()
            .points
            .iter()
            .find(|point| point.id == id)
            .unwrap();
        if cancelled {
            assert_eq!(point.right_handle, original_handle);
            assert!(
                app.world_mut()
                    .resource_mut::<WidgetEventQueue>()
                    .take()
                    .iter()
                    .all(|event| !matches!(event, WidgetEvent::CurveEditorCommitted { .. }))
            );
        } else {
            assert_ne!(point.right_handle, original_handle);
            assert!(app
                    .world_mut()
                    .resource_mut::<WidgetEventQueue>()
                    .take()
                    .iter()
                    .any(|event| matches!(event, WidgetEvent::CurveEditorCommitted { target, .. } if *target == entity)));
        }
    }

    let zoom_before = app.world().get::<CurveEditor>(entity).unwrap().zoom;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: entity,
        position: [plot.x + plot.width * 0.4, plot.y + plot.height * 0.6],
        delta: [0.0, 1.0],
        unit: bbui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert!(app.world().get::<CurveEditor>(entity).unwrap().zoom > zoom_before);

    let before_pan = app.world().get::<CurveEditor>(entity).unwrap().pan;
    let pan_start = [plot.x + plot.width * 0.5, plot.y + plot.height * 0.5];
    let pan_end = [pan_start[0] + 14.0, pan_start[1] - 9.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: entity,
            button: UiPointerButton::Middle,
            position: pan_start,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: entity,
            position: pan_end,
            delta: [14.0, -9.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: entity,
            button: UiPointerButton::Middle,
            position: pan_end,
            cancelled: false,
        },
    ];
    app.update();
    assert_ne!(
        app.world().get::<CurveEditor>(entity).unwrap().pan,
        before_pan
    );
}

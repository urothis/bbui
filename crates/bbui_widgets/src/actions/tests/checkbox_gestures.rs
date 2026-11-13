use super::*;

#[test]
pub(super) fn checkbox_paint_sets_each_traversed_control_once_and_commits_one_record() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let first = app
        .world_mut()
        .spawn((
            UiNode,
            UiHitBox {
                rect: UiRect::new(0.0, 0.0, 20.0, 20.0),
                clip: None,
                z_index: 0,
                window: Some(window),
            },
            Checkbox::new("first", false),
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            UiNode,
            UiHitBox {
                rect: UiRect::new(20.0, 0.0, 20.0, 20.0),
                clip: None,
                z_index: 0,
                window: Some(window),
            },
            Checkbox::new("second", false),
        ))
        .id();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: first,
            button: UiPointerButton::Primary,
            position: [5.0, 5.0],
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [30.0, 5.0],
            delta: [25.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerMoved {
            window,
            target: first,
            position: [30.0, 5.0],
            delta: [0.0, 0.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: first,
            button: UiPointerButton::Primary,
            position: [30.0, 5.0],
            cancelled: false,
        },
        UiRoutedEvent::Click {
            window,
            target: first,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [5.0, 5.0],
        },
    ];
    app.update();
    assert!(app.world().get::<Checkbox>(first).unwrap().is_checked());
    assert!(app.world().get::<Checkbox>(second).unwrap().is_checked());
    let events = app.world_mut().resource_mut::<WidgetEventQueue>().take();
    let Some(WidgetEvent::CheckboxPaintCommitted { changes }) = events
        .iter()
        .find(|event| matches!(event, WidgetEvent::CheckboxPaintCommitted { .. }))
    else {
        panic!("one brush transaction should be emitted");
    };
    assert_eq!(changes.len(), 2);
    assert_ne!(changes[0].entity, changes[1].entity);
}

#[test]
pub(super) fn checkbox_brush_cancel_restores_and_commit_undoes_once_with_disabled_keyboard_guards()
{
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let spawn_checkbox = |world: &mut World, label: &str, x: f32| {
        world
            .spawn((
                UiNode,
                Checkbox::new(label, false),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(x, 0.0, 20.0, 20.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id()
    };
    let first = spawn_checkbox(app.world_mut(), "first", 0.0);
    let second = spawn_checkbox(app.world_mut(), "second", 20.0);
    let mut locked_checkbox = Checkbox::new("locked", false);
    locked_checkbox.read_only = true;
    let locked_checkbox = app.world_mut().spawn(locked_checkbox).id();
    let mut disabled_button = Button::new("disabled");
    disabled_button.enabled = false;
    let disabled_button = app.world_mut().spawn(disabled_button).id();
    let mut read_only_text = TextField::new("locked text");
    read_only_text.read_only = true;
    let read_only_text = app.world_mut().spawn(read_only_text).id();

    let brush_events = |cancelled| {
        vec![
            UiRoutedEvent::PointerDown {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [5.0, 5.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [30.0, 5.0],
                delta: [25.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [30.0, 5.0],
                cancelled,
            },
        ]
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = brush_events(true);
    app.update();
    assert!(!app.world().get::<Checkbox>(first).unwrap().is_checked());
    assert!(!app.world().get::<Checkbox>(second).unwrap().is_checked());
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::CheckboxPaintCancelled { changes } if changes.len() == 2)
        ));
    app.world_mut().resource_mut::<WidgetEventQueue>().take();

    app.world_mut().resource_mut::<UiRoutedEvents>().events = brush_events(false);
    app.update();
    assert!(app.world().get::<Checkbox>(first).unwrap().is_checked());
    assert!(app.world().get::<Checkbox>(second).unwrap().is_checked());
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);
    assert_eq!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .filter(|event| matches!(event, WidgetEvent::CheckboxPaintCommitted { .. }))
            .count(),
        1
    );

    let key = |target, key, modifiers| UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    };
    let ctrl = UiModifiers {
        control: true,
        ..UiModifiers::default()
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(first, UiKeyCode::Z, ctrl)];
    app.update();
    assert!(!app.world().get::<Checkbox>(first).unwrap().is_checked());
    assert!(!app.world().get::<Checkbox>(second).unwrap().is_checked());
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().redo_len(), 1);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
        first,
        UiKeyCode::Z,
        UiModifiers {
            shift: true,
            ..ctrl
        },
    )];
    app.update();
    assert!(app.world().get::<Checkbox>(first).unwrap().is_checked());
    assert!(app.world().get::<Checkbox>(second).unwrap().is_checked());

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        key(disabled_button, UiKeyCode::Space, UiModifiers::default()),
        key(disabled_button, UiKeyCode::Enter, UiModifiers::default()),
        key(locked_checkbox, UiKeyCode::Space, UiModifiers::default()),
        key(read_only_text, UiKeyCode::Backspace, UiModifiers::default()),
        UiRoutedEvent::Text {
            window,
            target: read_only_text,
            text: " edit".to_owned(),
        },
    ];
    app.update();
    assert_eq!(
        app.world()
            .get::<Button>(disabled_button)
            .unwrap()
            .activations,
        0
    );
    assert!(
        !app.world()
            .get::<Checkbox>(locked_checkbox)
            .unwrap()
            .is_checked()
    );
    assert_eq!(
        app.world().get::<TextField>(read_only_text).unwrap().text,
        "locked text"
    );
    assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(|event| {
            matches!(event, WidgetEvent::Activated { target } if *target == disabled_button)
                || matches!(event, WidgetEvent::TextChanged { target, .. } if *target == read_only_text)
        }));
}

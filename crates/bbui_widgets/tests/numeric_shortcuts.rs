use bbui_core::{UiKeyCode, UiModifiers, UiNode, UiRoutedEvent, UiRoutedEvents, UiWheelUnit};
use bbui_widgets::{
    ClipboardRequest, ClipboardRequestQueue, NumericField, NumericFieldKind, WidgetPlugin,
};
use bevy_app::App;
use bevy_ecs::entity::Entity;

fn drive(app: &mut App, event: UiRoutedEvent) {
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![event];
    app.update();
}

fn key(window: Entity, target: Entity, key: UiKeyCode, modifiers: UiModifiers) -> UiRoutedEvent {
    UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn numeric_keyboard_wheel_clipboard_and_reset_keep_values_valid() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut field =
        NumericField::new(5.0, Some(0.0), Some(10.0), 0.5, NumericFieldKind::Float).unwrap();
    field.unit = Some("m".to_owned());
    field.set_value(6.0).unwrap();
    let target = app.world_mut().spawn((UiNode, field)).id();

    drive(
        &mut app,
        key(window, target, UiKeyCode::ArrowUp, UiModifiers::default()),
    );
    assert_eq!(app.world().get::<NumericField>(target).unwrap().value, 6.5);
    drive(
        &mut app,
        key(
            window,
            target,
            UiKeyCode::ArrowDown,
            UiModifiers {
                shift: true,
                ..Default::default()
            },
        ),
    );
    assert!((app.world().get::<NumericField>(target).unwrap().value - 6.45).abs() < 1e-12);
    drive(
        &mut app,
        UiRoutedEvent::Wheel {
            window,
            target,
            position: [5.0, 5.0],
            delta: [0.0, 1.0],
            unit: UiWheelUnit::Lines,
            modifiers: UiModifiers {
                control: true,
                ..Default::default()
            },
        },
    );
    assert!((app.world().get::<NumericField>(target).unwrap().value - 6.95).abs() < 1e-12);

    let control = UiModifiers {
        control: true,
        ..Default::default()
    };
    drive(&mut app, key(window, target, UiKeyCode::C, control));
    assert_eq!(
        app.world_mut()
            .resource_mut::<ClipboardRequestQueue>()
            .drain()
            .collect::<Vec<_>>(),
        vec![ClipboardRequest::Write("6.95".to_owned())]
    );
    drive(&mut app, key(window, target, UiKeyCode::V, control));
    assert_eq!(
        app.world_mut()
            .resource_mut::<ClipboardRequestQueue>()
            .drain()
            .collect::<Vec<_>>(),
        vec![ClipboardRequest::Read { target }]
    );
    drive(
        &mut app,
        UiRoutedEvent::Text {
            window,
            target,
            text: "6.95".to_owned(),
        },
    );
    drive(
        &mut app,
        key(window, target, UiKeyCode::Enter, UiModifiers::default()),
    );
    assert!((app.world().get::<NumericField>(target).unwrap().value - 6.95).abs() < 1e-12);

    drive(
        &mut app,
        key(window, target, UiKeyCode::Backspace, UiModifiers::default()),
    );
    assert_eq!(app.world().get::<NumericField>(target).unwrap().value, 5.0);

    drive(
        &mut app,
        UiRoutedEvent::Text {
            window,
            target,
            text: "bad paste".to_owned(),
        },
    );
    drive(
        &mut app,
        key(window, target, UiKeyCode::Enter, UiModifiers::default()),
    );
    let current = app.world().get::<NumericField>(target).unwrap();
    assert_eq!(current.value, 5.0);
    assert!(current.expression_error.is_some());
    drive(
        &mut app,
        key(window, target, UiKeyCode::Escape, UiModifiers::default()),
    );
    let current = app.world().get::<NumericField>(target).unwrap();
    assert_eq!(current.value, 5.0);
    assert_eq!(current.expression_error, None);
}

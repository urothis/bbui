use std::collections::HashMap;

use bbui_core::{
    UiKeyCode, UiModifiers, UiNode, UiPointerButton, UiRect, UiRoutedEvent, UiRoutedEvents,
};
use bbui_layout::UiLayoutResult;
use bbui_render::{DisplayList, Primitive};
use bbui_style::Theme;
use bbui_widgets::{
    NumericExpressionVariables, NumericField, WidgetEvent, WidgetEventQueue, WidgetPlugin,
};
use bevy_app::App;
use bevy_ecs::entity::Entity;

fn edit(app: &mut App, window: Entity, field: Entity, text: &str) {
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::Click {
            window,
            target: field,
            button: UiPointerButton::Primary,
            click_count: 2,
            position: [5.0, 5.0],
        },
        UiRoutedEvent::Text {
            window,
            target: field,
            text: text.to_owned(),
        },
        UiRoutedEvent::Key {
            window,
            target: field,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ];
    app.update();
}

#[test]
fn typed_numeric_expression_uses_field_variables_and_preserves_invalid_edits() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let field = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 300.0, 28.0),
                clip: None,
                z: 0,
                content_size: [300.0, 28.0],
                dpi_scale: 1.0,
            },
            NumericField::float(3.0).unwrap(),
            NumericExpressionVariables(HashMap::from([
                ("c".to_owned(), 0.0),
                ("y".to_owned(), 7.0),
                ("x".to_owned(), 999.0),
            ])),
        ))
        .id();

    edit(&mut app, window, field, "x if c else y");
    assert_eq!(app.world().get::<NumericField>(field).unwrap().value, 7.0);
    assert!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| {
                matches!(event, WidgetEvent::ValueChanged { target, .. } if *target == field)
            })
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);

    edit(&mut app, window, field, "2*pi");
    assert_eq!(
        app.world().get::<NumericField>(field).unwrap().value,
        2.0 * std::f64::consts::PI
    );

    edit(&mut app, window, field, "1/0");
    let current = app.world().get::<NumericField>(field).unwrap();
    assert_eq!(current.value, 2.0 * std::f64::consts::PI);
    assert_eq!(current.editing.as_deref(), Some("1/0"));
    assert_eq!(
        current.expression_error.as_ref().map(ToString::to_string),
        Some("division by zero".to_owned())
    );
    assert!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| {
                matches!(event, WidgetEvent::NumericExpressionError { target, error }
            if *target == field && error == "division by zero")
            })
    );
    let theme_error = app.world().resource::<Theme>().error;
    assert!(
        app.world()
            .resource::<DisplayList>()
            .items()
            .iter()
            .any(|item| {
                matches!(item.primitive, Primitive::Line { color, .. } if color == theme_error)
            })
    );
    assert!(
        app.world()
            .resource::<DisplayList>()
            .items()
            .iter()
            .any(|item| {
                matches!(&item.primitive, Primitive::TextWrapped { text, .. }
            if text.contains("division by zero"))
            })
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 2);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::Key {
            window,
            target: field,
            key: UiKeyCode::Backspace,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
        UiRoutedEvent::Text {
            window,
            target: field,
            text: "2".to_owned(),
        },
        UiRoutedEvent::Key {
            window,
            target: field,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ];
    app.update();
    let current = app.world().get::<NumericField>(field).unwrap();
    assert_eq!(current.value, 0.5);
    assert_eq!(current.expression_error, None);
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 3);

    for (source, expected) in [
        ("sqrt(-1)", "expression is outside the function domain"),
        ("1e308*10", "numeric overflow"),
        ("1e", "invalid number at byte 0"),
    ] {
        edit(&mut app, window, field, source);
        let current = app.world().get::<NumericField>(field).unwrap();
        assert_eq!(current.value, 0.5);
        assert_eq!(
            current.expression_error.as_ref().map(ToString::to_string),
            Some(expected.to_owned())
        );
        assert!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .any(|event| {
                    matches!(event, WidgetEvent::NumericExpressionError { target, error }
                if *target == field && error == expected)
                })
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: field,
            key: UiKeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<NumericField>(field)
                .unwrap()
                .expression_error,
            None
        );
    }
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 3);
}

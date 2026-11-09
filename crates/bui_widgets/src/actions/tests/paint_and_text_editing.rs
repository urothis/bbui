use super::*;

#[test]
pub(super) fn widget_paint_uses_computed_colors_opacity_and_corner_radius() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut style = UiComputedStyle::default();
    style.properties.insert(
        UiStyleProperty::Background,
        bui_style::UiStyleValue::Color(Rgba(0.2, 0.3, 0.4, 1.0)),
    );
    style.properties.insert(
        UiStyleProperty::Foreground,
        bui_style::UiStyleValue::Color(Rgba(0.6, 0.7, 0.8, 1.0)),
    );
    style.properties.insert(
        UiStyleProperty::CornerRadius,
        bui_style::UiStyleValue::Scalar(11.0),
    );
    style.properties.insert(
        UiStyleProperty::Opacity,
        bui_style::UiStyleValue::Scalar(0.5),
    );
    style.properties.insert(
        UiStyleProperty::FontSize,
        bui_style::UiStyleValue::Scalar(20.0),
    );
    style.properties.insert(
        UiStyleProperty::LineHeight,
        bui_style::UiStyleValue::Scalar(1.5),
    );
    let entity = app
        .world_mut()
        .spawn((
            UiNode,
            Button::new("styled"),
            style,
            UiLayoutResult {
                window,
                rect: UiRect::new(4.0, 8.0, 90.0, 26.0),
                clip: None,
                z: 3,
                content_size: [90.0, 26.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();

    let items = app.world().resource::<DisplayList>().widget_items();
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { radius: 11.0, color, .. }
            if color == Rgba(0.2, 0.3, 0.4, 0.5)
    )));
    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { color, style: Some(style), .. }
            if *color == Rgba(0.6, 0.7, 0.8, 0.5)
                && style.font.as_ref().is_some_and(|font| matches!(
                    font.font_size,
                    bevy_text::FontSize::Px(20.0)
                ))
                && style.line_height == Some(LineHeight::RelativeToFont(1.5))
    )));
    app.world_mut().entity_mut(entity).despawn();
}

#[test]
pub(super) fn widget_paint_uses_semantic_button_tokens_and_control_spacing() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut theme = Theme::new(bui_style::ThemeKind::Light);
    theme.spacing.control.padding_x = 13.0;
    theme.spacing.control.padding_y = 7.0;
    let disabled_surface = theme.color(ThemeColorToken::ButtonDisabledSurface);
    let disabled_text = theme.color(ThemeColorToken::DisabledText);
    *app.world_mut().resource_mut::<Theme>() = theme;
    let mut button = Button::new("disabled");
    button.enabled = false;
    app.world_mut().spawn((
        UiNode,
        button,
        UiLayoutResult {
            window,
            rect: UiRect::new(20.0, 30.0, 120.0, 28.0),
            clip: None,
            z: 3,
            content_size: [120.0, 28.0],
            dpi_scale: 1.0,
        },
    ));
    app.update();

    let items = app.world().resource::<DisplayList>().widget_items();
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { color, .. } if color == disabled_surface
    )));
    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { origin, color, .. }
            if *origin == [33.0, 37.0] && *color == disabled_text
    )));
}

#[test]
pub(super) fn widget_paint_keeps_screen_and_camera_layers_separate() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let screen = app.world_mut().spawn_empty().id();
    let camera = app.world_mut().spawn(bevy_camera::Camera::default()).id();
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("screen", true),
        UiLayoutResult {
            window: screen,
            rect: UiRect::new(0.0, 0.0, 80.0, 20.0),
            clip: None,
            z: 1,
            content_size: [80.0, 20.0],
            dpi_scale: 1.0,
        },
    ));
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("world", true),
        UiLayoutResult {
            window: camera,
            rect: UiRect::new(0.0, 0.0, 80.0, 20.0),
            clip: None,
            z: 2,
            content_size: [80.0, 20.0],
            dpi_scale: 1.0,
        },
    ));
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(!display_list.widget_items().is_empty());
    assert!(
        display_list
            .camera_items(camera)
            .is_some_and(|items| !items.is_empty())
    );
    assert!(display_list.items().iter().any(|item| item.z == 1));
    assert!(display_list.items().iter().any(|item| item.z == 2));
}

#[test]
pub(super) fn application_actions_are_applied_in_plugin_widget_phase() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let entity = app.world_mut().spawn(Checkbox::new("enabled", false)).id();
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(entity, WidgetActionKind::SetBool(true)));
    app.update();
    assert!(app.world().get::<Checkbox>(entity).unwrap().is_checked());
    assert_eq!(
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .len(),
        1
    );
}

#[test]
pub(super) fn radio_group_and_icon_button_are_visible_and_route_pointer_keyboard_and_events() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut radio = RadioGroup::new(["Object", "Edit", "Sculpt"], Some(0));
    radio.options[2].enabled = false;
    let tree = WidgetTree::new(WidgetNode::row([
        WidgetNode::radio_group("Mode", radio),
        WidgetNode::icon_button("+", "Add object"),
        WidgetNode::button("Disabled"),
        WidgetNode::text_field("Read only", "locked"),
    ]));
    let root = tree.spawn(app.world_mut(), None).unwrap();
    let controls = app.world().get::<UiChildren>(root).unwrap().0.clone();
    let radio = controls[0];
    let icon = controls[1];
    let disabled = controls[2];
    let read_only = controls[3];
    app.world_mut().get_mut::<Button>(disabled).unwrap().enabled = false;
    app.world_mut()
        .get_mut::<TextField>(read_only)
        .unwrap()
        .read_only = true;
    app.world_mut().entity_mut(read_only).insert(UiReadOnly);
    for (entity, rect) in [
        (radio, UiRect::new(10.0, 10.0, 220.0, 96.0)),
        (icon, UiRect::new(260.0, 10.0, 32.0, 28.0)),
        (disabled, UiRect::new(300.0, 10.0, 100.0, 28.0)),
    ] {
        app.world_mut().entity_mut(entity).insert((
            UiLayoutResult {
                window,
                rect,
                clip: None,
                z: 5,
                content_size: [rect.width, rect.height],
                dpi_scale: 1.0,
            },
            UiHitBox {
                window: Some(window),
                rect,
                clip: None,
                z_index: 5,
            },
        ));
    }
    app.update();
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(
                |item| matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "+")
            )
    );
    assert!(app.world().resource::<DisplayList>().widget_items().iter().any(
            |item| matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "Edit")
        ));
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Ellipse { .. }))
    );

    let click = |target, position| UiRoutedEvent::Click {
        window,
        target,
        button: UiPointerButton::Primary,
        click_count: 1,
        position,
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(radio, [24.0, 70.0])];
    app.update();
    assert_eq!(
        app.world().get::<RadioGroup>(radio).unwrap().selected,
        Some(1)
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(0)), after: WidgetValue::Option(Some(1)) } if *target == radio)));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(radio, [24.0, 94.0])];
    app.update();
    assert_eq!(
        app.world().get::<RadioGroup>(radio).unwrap().selected,
        Some(1),
        "disabled options cannot be selected by pointer"
    );

    let key = |key| UiRoutedEvent::Key {
        window,
        target: radio,
        key,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::ArrowRight)];
    app.update();
    assert_eq!(
        app.world().get::<RadioGroup>(radio).unwrap().selected,
        Some(0)
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, .. } if *target == radio)));
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
    app.update();
    assert_eq!(
        app.world().get::<RadioGroup>(radio).unwrap().selected,
        Some(1)
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, .. } if *target == radio)));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(icon, [276.0, 24.0])];
    app.update();
    assert_eq!(app.world().get::<IconButton>(icon).unwrap().activations, 1);
    assert!(
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == icon))
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(disabled, [320.0, 24.0])];
    app.update();
    assert_eq!(app.world().get::<Button>(disabled).unwrap().activations, 0);
    assert!(
        !app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == disabled))
    );
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            read_only,
            WidgetActionKind::InsertText(" edit".to_owned()),
        ));
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(read_only).unwrap().as_str(),
        "locked",
        "read-only text controls ignore routed insert actions"
    );
    assert!(!app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::TextChanged { target, .. } if *target == read_only)));
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: icon,
        key: UiKeyCode::Space,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(app.world().get::<IconButton>(icon).unwrap().activations, 2);
    assert!(
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == icon))
    );
}

pub(super) fn send_tab(app: &mut App, window: Entity, shift: bool) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                shift,
                ..UiModifiers::default()
            },
        });
    app.update();
}

pub(super) fn send_key(app: &mut App, window: Entity, key: UiKeyCode, modifiers: UiModifiers) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        });
    app.update();
}

#[test]
pub(super) fn committed_text_blur_uses_global_history_after_local_undo_priority() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let text = WidgetTree::new(WidgetNode::text_field("Name", "a🧊c"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(text).insert((
        UiTabIndex(0),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
            clip: None,
            z_index: 0,
        },
    ));
    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(120.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .get_mut::<TextBuffer>(text)
        .unwrap()
        .set_selection(5, 1);
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(text);
    app.update();

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Text {
            window,
            text: "Z".into(),
        });
    app.update();
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "aZc");
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 1);

    let control = UiModifiers {
        control: true,
        ..UiModifiers::default()
    };
    send_key(&mut app, window, UiKeyCode::Z, control);
    assert_eq!(
        app.world().get::<TextBuffer>(text).unwrap().as_str(),
        "a🧊c"
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    send_key(&mut app, window, UiKeyCode::Y, control);
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "aZc");
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);

    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(other)
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 0);
    assert!(app
            .world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::TextCommitted { target, before, after, before_selection, after_selection, .. }
                if *target == text
                    && before == "a🧊c"
                    && after == "aZc"
                    && *before_selection == TextSelection { anchor: 5, caret: 1 }
                    && *after_selection == TextSelection { anchor: 2, caret: 2 })));
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

    send_key(&mut app, window, UiKeyCode::Z, control);
    let buffer = app.world().get::<TextBuffer>(text).unwrap();
    assert_eq!(buffer.as_str(), "a🧊c");
    assert_eq!(buffer.anchor(), 5);
    assert_eq!(buffer.caret(), 1);
    assert_eq!(buffer.undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(text)
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    assert_eq!(app.world().resource::<bui_core::UiHistory>().redo_len(), 1);

    send_key(&mut app, window, UiKeyCode::Y, control);
    let buffer = app.world().get::<TextBuffer>(text).unwrap();
    assert_eq!(buffer.as_str(), "aZc");
    assert_eq!(buffer.anchor(), 2);
    assert_eq!(buffer.caret(), 2);
    assert_eq!(
        buffer.undo_len(),
        0,
        "global redo must not leave a duplicate local edit"
    );

    send_tab(&mut app, window, false);
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
    assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(|event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == text))
                .count(),
            1,
            "redo rebases the focused draft instead of recommitting the same text"
        );
}

#[test]
pub(super) fn locally_cancelled_text_is_not_committed_and_enter_commits_once() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let text = WidgetTree::new(WidgetNode::text_field("Name", "a"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(text).insert((
        UiTabIndex(0),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
            clip: None,
            z_index: 0,
        },
    ));
    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(120.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(text);
    app.update();

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Text {
            window,
            text: "b".into(),
        });
    app.update();
    send_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "a");
    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(other)
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == text)
    ));

    send_tab(&mut app, window, true);
    send_key(
        &mut app,
        window,
        UiKeyCode::Y,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "ab");
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    send_key(&mut app, window, UiKeyCode::Enter, UiModifiers::default());
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "ab");
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 0);
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
    assert_eq!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .filter(
                |event| matches!(event, WidgetEvent::TextCommitted { target, before, after, .. }
                    if *target == text && before == "a" && after == "ab")
            )
            .count(),
        1
    );

    send_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "a");
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(text)
    );
}

#[test]
pub(super) fn textarea_ime_preedit_cancels_and_ime_commit_enters_global_history_once() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let area = WidgetTree::new(WidgetNode::text_area("Notes", "draft"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(area).insert((
        UiTabIndex(0),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 0.0, 160.0, 60.0),
            clip: None,
            z_index: 0,
        },
    ));
    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(1),
            UiEnabled(true),
            UiVisible(true),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(180.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(area);
    app.update();

    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::ImePreedit {
            window,
            text: "仮".into(),
            cursor_range: None,
        },
        UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ]);
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draft"
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == area)
    ));
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(other)
    );

    send_tab(&mut app, window, true);
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::ImePreedit {
            window,
            text: "かな".into(),
            cursor_range: None,
        },
        UiInputEvent::ImeCommit {
            window,
            text: "かな".into(),
        },
        UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ]);
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draftかな"
    );
    assert_eq!(app.world().get::<TextBuffer>(area).unwrap().undo_len(), 0);
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

    send_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draft"
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(area)
    );
}

#[test]
pub(super) fn tab_leaves_open_popovers_and_dropdowns_without_restoring_trigger_focus() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let spawn_focusable = |world: &mut World, tab_index: i32, x: f32| {
        world
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(tab_index),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(x, 0.0, 80.0, 24.0),
                    clip: None,
                    z_index: tab_index,
                },
            ))
            .id()
    };
    let before = spawn_focusable(app.world_mut(), 0, 0.0);
    let mut popover = MenuControl::new(
        "Tools",
        crate::MenuTree::new(vec![crate::MenuEntry::command("apply", "Apply")]),
        MenuDisplay::Popover,
    );
    popover.open(Some("tools-popover".into()));
    popover.popup.open = true;
    let menu = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(1),
            UiKey("tools-popover".into()),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(90.0, 0.0, 80.0, 24.0),
                clip: None,
                z_index: 1,
            },
            UiLayoutResult {
                window,
                rect: UiRect::new(90.0, 0.0, 80.0, 24.0),
                clip: None,
                z: 1,
                content_size: [80.0, 24.0],
                dpi_scale: 1.0,
            },
            popover,
        ))
        .id();
    let dropdown = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(2),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(180.0, 0.0, 80.0, 24.0),
                clip: None,
                z_index: 2,
            },
            Dropdown::new(["A", "B"], Some(0)),
        ))
        .id();
    let after = spawn_focusable(app.world_mut(), 3, 270.0);
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(menu);

    app.update();
    assert!(app.world().get::<MenuControl>(menu).unwrap().session.open);
    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(dropdown)
    );
    assert!(!app.world().get::<MenuControl>(menu).unwrap().session.open);
    assert!(!app.world().get::<MenuControl>(menu).unwrap().popup.open);

    app.world_mut()
        .get_mut::<Dropdown>(dropdown)
        .unwrap()
        .open();
    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(after)
    );
    assert!(!app.world().get::<Dropdown>(dropdown).unwrap().open);

    {
        let mut menu = app.world_mut().get_mut::<MenuControl>(menu).unwrap();
        menu.open(Some("tools-popover".into()));
        menu.popup.open = true;
    }
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(menu);
    app.update();
    send_tab(&mut app, window, true);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(before)
    );
    assert!(!app.world().get::<MenuControl>(menu).unwrap().session.open);
}

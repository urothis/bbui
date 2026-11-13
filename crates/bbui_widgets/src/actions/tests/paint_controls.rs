use super::*;

#[test]
pub(super) fn widget_paint_uses_computed_colors_opacity_and_corner_radius() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut style = UiComputedStyle::default();
    style.properties.insert(
        UiStyleProperty::Background,
        bbui_style::UiStyleValue::Color(Rgba(0.2, 0.3, 0.4, 1.0)),
    );
    style.properties.insert(
        UiStyleProperty::Foreground,
        bbui_style::UiStyleValue::Color(Rgba(0.6, 0.7, 0.8, 1.0)),
    );
    style.properties.insert(
        UiStyleProperty::CornerRadius,
        bbui_style::UiStyleValue::Scalar(11.0),
    );
    style.properties.insert(
        UiStyleProperty::Opacity,
        bbui_style::UiStyleValue::Scalar(0.5),
    );
    style.properties.insert(
        UiStyleProperty::FontSize,
        bbui_style::UiStyleValue::Scalar(20.0),
    );
    style.properties.insert(
        UiStyleProperty::LineHeight,
        bbui_style::UiStyleValue::Scalar(1.5),
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
    let mut theme = Theme::new(bbui_style::ThemeKind::Light);
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

use super::*;

#[test]
pub(super) fn color_ramp_painter_emits_gradient_and_channel_gpu_primitives() {
    let mut app = App::new();
    let (window, entity) = spawn_color_ramp_widget(&mut app, UiRect::new(0.0, 0.0, 360.0, 194.0));
    assert_ne!(window, Entity::PLACEHOLDER);
    app.update();
    let items = app.world().resource::<DisplayList>().widget_items();
    assert!(items.len() < 60, "the ramp should use bounded draw items");
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.primitive, Primitive::LinearGradient { .. }))
            .count(),
        5,
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Line { .. }))
    );
    app.world_mut().entity_mut(entity).despawn();
}

#[test]
pub(super) fn color_ramp_default_spacing_separates_details_channels_and_values() {
    let theme = Theme::default();
    let spacing = theme.spacing.color_ramp;
    let rect = Rect::new(0.0, 0.0, 360.0, 220.0);
    let track = color_ramp_track_rect(rect, spacing);
    let handle_bottom = track.y
        + track.height
        + spacing.marker_handle_top_inset
        + spacing.marker_handle_radius * 2.0;
    let details_top = rect.y + spacing.details_y;
    let details_bottom = details_top + theme.font_size * theme.line_height;
    let first_channel = color_ramp_channel_rect(rect, 0, spacing);
    let last_channel_bottom = color_ramp_channel_rect(rect, 3, spacing).y + spacing.channel_height;
    let value_label_x = rect.x + rect.width - spacing.channel_value_right_inset;
    let editing_label_y = rect.y + rect.height - spacing.editing_bottom_inset;

    assert!(details_top >= handle_bottom + 4.0);
    assert!(first_channel.y >= details_bottom + 4.0);
    assert!(first_channel.x + first_channel.width + 4.0 <= value_label_x);
    assert!(editing_label_y >= last_channel_bottom + 8.0);
    assert!(editing_label_y + theme.font_size * theme.line_height <= rect.y + rect.height);
}

#[test]
pub(super) fn color_ramp_cancelled_drag_restores_position_without_commit() {
    let mut app = App::new();
    let rect = UiRect::new(0.0, 0.0, 360.0, 194.0);
    let (window, entity) = spawn_color_ramp_widget(&mut app, rect);
    let track = color_ramp_track_rect(rect, Theme::default().spacing.color_ramp);
    let start = [track.x, track.y + track.height + 10.0];
    let end = [track.x + track.width * 0.7, start[1]];
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
        UiRoutedEvent::PointerCancel {
            window,
            target: entity,
        },
    ];
    app.update();
    assert_eq!(
        app.world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .stops
            .iter()
            .find(|stop| stop.id == 1)
            .unwrap()
            .position,
        0.0
    );
    assert!(
        !app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::ColorRampCommitted { .. }))
    );
}

#[test]
pub(super) fn hsv_wheel_drag_is_one_undoable_gesture_and_value_alpha_tracks_route() {
    let mut app = App::new();
    let rect = UiRect::new(10.0, 20.0, 448.0, 236.0);
    let (window, entity) = spawn_hsv_color_widget(&mut app, rect);
    let mut spacing = Theme::default().spacing.hsv;
    spacing.wheel_center_x = 96.0;
    spacing.wheel_center_y = 108.0;
    spacing.wheel_outer_radius = 62.0;
    spacing.wheel_inner_radius = 50.0;
    app.world_mut().resource_mut::<Theme>().spacing.hsv = spacing;
    let (center, _, _) = hsv_wheel_geometry(rect, spacing);
    let at_green = [center[0] + 20.0, center[1] + 34.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: at_green,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            position: at_green,
            cancelled: false,
        },
    ];
    app.update();
    let editor = app.world().get::<HsvColorEditor>(entity).unwrap();
    assert!((editor.hue_degrees - 60.0).abs() < 1.0);
    assert!((editor.saturation - 1556.0_f32.sqrt() / 50.0).abs() < 0.03);
    assert!(editor.color().0 > editor.color().2);
    assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(|event| matches!(event, WidgetEvent::HsvColorCommitted { target, .. } if *target == entity))
                .count(),
            1,
            "a drag produces one committed edit"
        );
    app.world_mut().resource_mut::<WidgetEventQueue>().take();

    let key = |key| UiRoutedEvent::Key {
        window,
        target: entity,
        key,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..Default::default()
        },
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Z)];
    app.update();
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .hue_degrees)
            .abs()
            < 0.01
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Y)];
    app.update();
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .hue_degrees
            - 60.0)
            .abs()
            < 1.0
    );

    for (kind, track, value) in [
        (HsvColorDragKind::Value, hsv_value_rect(rect, spacing), 0.3),
        (HsvColorDragKind::Alpha, hsv_alpha_rect(rect, spacing), 0.65),
    ] {
        let from = [track.x + track.width * 0.8, track.y + 5.0];
        let to = [track.x + track.width * value, from[1]];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: from,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: to,
                delta: [to[0] - from[0], 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: to,
                cancelled: false,
            },
        ];
        app.update();
        let editor = app.world().get::<HsvColorEditor>(entity).unwrap();
        let actual = match kind {
            HsvColorDragKind::Value => editor.value,
            HsvColorDragKind::Alpha => editor.alpha,
            HsvColorDragKind::Wheel => panic!(),
        };
        assert!((actual - value).abs() < 0.01);
    }
}

#[test]
pub(super) fn hsv_editor_routes_hex_and_rgb_numeric_entry_and_action_updates() {
    let mut app = App::new();
    let rect = UiRect::new(0.0, 0.0, 448.0, 236.0);
    let (window, entity) = spawn_hsv_color_widget(&mut app, rect);
    let mut spacing = Theme::default().spacing.hsv;
    spacing.controls_x = 184.0;
    spacing.field_cell_width = 70.0;
    spacing.field_gap = 10.0;
    spacing.field_first_row_y = 118.0;
    spacing.field_second_row_y = 150.0;
    spacing.field_hex_y = 184.0;
    app.world_mut().resource_mut::<Theme>().spacing.hsv = spacing;
    let click_field = |field| {
        let field_rect = hsv_field_rect(rect, field, spacing);
        UiRoutedEvent::Click {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [field_rect.x + 4.0, field_rect.y + 4.0],
        }
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        click_field(HsvColorField::Hex),
        UiRoutedEvent::Text {
            window,
            target: entity,
            text: "#00FF0080".into(),
        },
        UiRoutedEvent::Key {
            window,
            target: entity,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ];
    app.update();
    let editor = app.world().get::<HsvColorEditor>(entity).unwrap();
    assert_eq!(editor.hex(), "#00FF0080");
    assert!((editor.color().1 - 1.0).abs() < 0.001);
    assert!((editor.alpha - 128.0 / 255.0).abs() < 0.001);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        click_field(HsvColorField::Red),
        UiRoutedEvent::Text {
            window,
            target: entity,
            text: "128".into(),
        },
        UiRoutedEvent::Key {
            window,
            target: entity,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        },
    ];
    app.update();
    assert!(
        (app.world().get::<HsvColorEditor>(entity).unwrap().color().0 - 128.0 / 255.0).abs()
            < 0.001
    );

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            entity,
            WidgetActionKind::HsvColorSetHex("#663399CC".into()),
        ));
    app.update();
    assert_eq!(
        app.world().get::<HsvColorEditor>(entity).unwrap().hex(),
        "#663399CC"
    );
}

#[test]
pub(super) fn hsv_editor_routes_navigation_modifiers_edit_commit_and_cancel() {
    let mut app = App::new();
    let (window, entity) = spawn_hsv_color_widget(&mut app, UiRect::new(0.0, 0.0, 448.0, 236.0));
    let route_key = |app: &mut App, key, modifiers| {
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

    route_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .hue_degrees
            - 1.0)
            .abs()
            < 0.01
    );
    route_key(
        &mut app,
        UiKeyCode::ArrowRight,
        UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    );
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .hue_degrees
            - 1.1)
            .abs()
            < 0.01
    );
    route_key(
        &mut app,
        UiKeyCode::ArrowRight,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .hue_degrees
            - 11.1)
            .abs()
            < 0.01
    );

    route_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<HsvColorEditorState>(entity)
            .unwrap()
            .active_field,
        HsvColorField::Saturation
    );
    route_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .saturation,
        1.0
    );

    route_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(
        app.world()
            .get::<HsvColorEditorState>(entity)
            .unwrap()
            .editing
    );
    route_key(&mut app, UiKeyCode::Delete, UiModifiers::default());
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: entity,
        text: "35".to_owned(),
    }];
    app.update();
    route_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .saturation
            - 0.35)
            .abs()
            < 0.01
    );
    assert!(
        !app.world()
            .get::<HsvColorEditorState>(entity)
            .unwrap()
            .editing
    );

    route_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    route_key(&mut app, UiKeyCode::Delete, UiModifiers::default());
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: entity,
        text: "90".to_owned(),
    }];
    app.update();
    route_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
    assert!(
        (app.world()
            .get::<HsvColorEditor>(entity)
            .unwrap()
            .saturation
            - 0.35)
            .abs()
            < 0.01
    );
    let state = app.world().get::<HsvColorEditorState>(entity).unwrap();
    assert!(!state.editing);
    assert!(state.edit_buffer.is_empty());
}

#[test]
pub(super) fn hsv_editor_paints_a_bounded_gpu_wheel_preview_and_entry_fields() {
    let mut app = App::new();
    let (_, entity) = spawn_hsv_color_widget(&mut app, UiRect::new(0.0, 0.0, 448.0, 236.0));
    app.update();
    let items = app.world().resource::<DisplayList>().widget_items();
    assert!((100..140).contains(&items.len()));
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.primitive, Primitive::HsvWheel { .. }))
            .count(),
        1,
        "the wheel must be a single GPU-shaded display item"
    );
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::HsvWheel {
            hue_degrees: 0.0,
            saturation: 1.0,
            value: 1.0,
            ..
        }
    )));
    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, .. } if text == "#FF0000FF"
    )));
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect {
            color: Rgba(1.0, 0.0, 0.0, 1.0),
            ..
        }
    )));
    app.world_mut().entity_mut(entity).despawn();
}

#[test]
pub(super) fn typed_editor_spacing_drives_hsv_curve_and_color_ramp_paint() {
    let mut theme = Theme::default();
    theme.spacing.hsv.wheel_center_x = 96.0;
    theme.spacing.hsv.wheel_center_y = 108.0;
    theme.spacing.hsv.wheel_outer_radius = 62.0;
    theme.spacing.hsv.controls_x = 188.0;
    theme.spacing.curve.interpolation_status_width = 124.0;
    theme.spacing.color_ramp.track_x = 17.0;
    theme.spacing.color_ramp.track_y = 44.0;
    theme.spacing.color_ramp.track_width_inset = 38.0;
    theme.spacing.color_ramp.track_height = 21.0;
    theme.spacing.color_ramp.channel_x = 55.0;
    theme.spacing.color_ramp.channel_y = 91.0;
    theme.spacing.color_ramp.channel_width_inset = 70.0;
    theme.spacing.color_ramp.channel_height = 12.0;
    theme.spacing.node_graph.header_hint_width = 244.0;

    let rect = Rect::new(10.0, 20.0, 448.0, 236.0);
    let clip = ClipRect::from(rect);
    let mut hsv_items = Vec::new();
    paint_hsv_color_editor(
        HsvColorPaint {
            editor: HsvColorEditor::default(),
            state: HsvColorEditorState::default(),
            label: "HSV",
            rect,
            clip,
            z: 1,
            theme: &theme,
        },
        &mut hsv_items,
    );
    assert!(hsv_items.iter().any(|item| matches!(
        item.primitive,
        Primitive::HsvWheel { rect: wheel, .. }
            if wheel.x == rect.x + 96.0 - 62.0
                && wheel.y == rect.y + 108.0 - 62.0
                && wheel.width == 124.0
                && wheel.height == 124.0
    )));
    assert!(hsv_items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, origin, .. }
            if text == "Value" && *origin == [rect.x + 188.0, rect.y + 27.0]
    )));

    let curve = CurveEditor::default();
    let mut curve_items = Vec::new();
    paint_curve_editor(
        CurveEditorPaint {
            editor: &curve,
            label: "Curve",
            rect,
            clip,
            z: 1,
            theme: &theme,
        },
        &mut curve_items,
    );
    let interpolation = format!("{:?}", curve.interpolation);
    assert!(curve_items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, origin, .. }
                if text == &interpolation
                    && (origin[0] - (rect.x + rect.width - 124.0 - theme.spacing.editor.padding_x)).abs() < 0.01
        )));

    let ramp = ColorRamp::default();
    let mut ramp_items = Vec::new();
    paint_color_ramp(
        ColorRampPaint {
            ramp: &ramp,
            state: ColorRampEditorState::default(),
            label: "Ramp",
            rect,
            clip,
            z: 1,
            theme: &theme,
        },
        &mut ramp_items,
    );
    assert!(ramp_items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { rect: track, color, .. }
            if color == theme.background
                && track.x == rect.x + 17.0
                && track.y == rect.y + 44.0
                && track.width == rect.width - 38.0
                && track.height == 21.0
    )));
    assert!(ramp_items.iter().any(|item| matches!(
        item.primitive,
        Primitive::LinearGradient { rect: row, .. }
            if row.x == rect.x + 55.0
                && row.y == rect.y + 91.0
                && row.width == rect.width - 70.0
                && row.height == 12.0
    )));
    assert_eq!(
        ramp_items
            .iter()
            .filter(|item| matches!(item.primitive, Primitive::LinearGradient { .. }))
            .count(),
        5,
        "one GPU gradient paints the ramp and each RGBA channel",
    );

    let graph = NodeGraph::default();
    let mut graph_items = Vec::new();
    paint_node_graph(
        NodeGraphPaint {
            registry: None,
            graph: &graph,
            state: NodeGraphEditorState::default(),
            output_label_widths: HashMap::new(),
            canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
            label: "Graph",
            rect,
            clip,
            z: 1,
            theme: &theme,
            font: ui_default_text_font(theme.font_size),
        },
        &mut graph_items,
    );
    let header_text = graph_items
        .iter()
        .filter_map(|item| match &item.primitive {
            Primitive::TextWrapped {
                text,
                origin,
                max_width,
                ..
            } if item.z == 3 => Some((text.as_str(), origin[0], *max_width)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(header_text.len(), 2);
    assert!(header_text.iter().any(|(text, _, _)| *text == "Graph"));
    assert!(
        header_text
            .iter()
            .any(|(text, _, _)| *text == "0 nodes · 0 links")
    );
    assert!(!header_text.iter().any(|(text, _, _)| text.contains("Drag")));
    let title = header_text
        .iter()
        .find(|(text, _, _)| *text == "Graph")
        .unwrap();
    let summary = header_text
        .iter()
        .find(|(text, _, _)| *text == "0 nodes · 0 links")
        .unwrap();
    let title_right = title.1 + title.2;
    let summary_left = summary.1;
    assert!(summary_left - title_right >= 8.0);
}

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

#[test]
pub(super) fn tooltip_painter_uses_typed_bounds_padding_margin_and_anchor_gap() {
    let mut world = World::new();
    let window = world.spawn_empty().id();
    world.spawn(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 500.0, 300.0),
        1.0,
    ));
    let trigger = world
        .spawn(UiLayoutResult {
            window,
            rect: Rect::new(100.0, 40.0, 24.0, 20.0),
            clip: None,
            z: 4,
            content_size: [24.0, 20.0],
            dpi_scale: 1.0,
        })
        .id();
    let owner = world
        .spawn(Tooltip::new("A tooltip with a long caption"))
        .id();
    world.insert_resource(WidgetTooltips::default());
    world.resource_mut::<WidgetTooltips>().0.insert(
        window,
        TooltipHover {
            trigger,
            owner,
            elapsed_seconds: 1.0,
        },
    );
    let mut theme = Theme::default();
    theme.spacing.tooltip.padding_x = 10.0;
    theme.spacing.tooltip.padding_y = 6.0;
    theme.spacing.tooltip.maximum_width = 72.0;
    theme.spacing.tooltip.viewport_margin = 8.0;
    theme.spacing.tooltip.anchor_gap = 9.0;
    let tooltip = world.get::<Tooltip>(owner).unwrap().clone();
    let mut items = Vec::new();
    paint_tooltip_overlay(&world, owner, &tooltip, window, &theme, &mut items);

    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, max_width, origin, .. }
            if text == &tooltip.text && *max_width == 72.0 && *origin == [110.0, 75.0]
    )));
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { rect, .. }
            if rect.x == 100.0 && rect.y == 69.0
                && rect.width == 92.0 && rect.height > 0.0
    )));
}

#[test]
pub(super) fn popup_widget_painters_emit_theme_dependent_shadow_primitives() {
    let mut colors_by_theme = Vec::new();
    for kind in [bbui_style::ThemeKind::Dark, bbui_style::ThemeKind::Light] {
        let theme = Theme::new(kind);
        let expected_color = theme.color(ThemeColorToken::PopupShadow);
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();

        let window = app.world_mut().spawn_empty().id();
        let viewport = Rect::new(0.0, 0.0, 500.0, 360.0);
        app.world_mut()
            .spawn(UiLayoutRoot::new(window, viewport, 1.0));

        let mut menu = MenuControl::new(
            "File",
            crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
            MenuDisplay::Dropdown,
        );
        menu.open(None);
        menu.popup.bounds = Rect::new(40.0, 60.0, 120.0, 72.0);
        menu.popup.open = true;
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: Rect::new(40.0, 24.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
            menu,
        ));

        let mut pie_menu = MenuControl::new(
            "Radial",
            crate::MenuTree::new(vec![crate::MenuEntry::command("one", "One")]),
            MenuDisplay::Pie,
        );
        pie_menu.open(None);
        pie_menu.popup.bounds = Rect::new(220.0, 48.0, 112.0, 112.0);
        pie_menu.popup.open = true;
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: Rect::new(260.0, 8.0, 32.0, 24.0),
                clip: None,
                z: 2,
                content_size: [32.0, 24.0],
                dpi_scale: 1.0,
            },
            pie_menu,
        ));

        let dropdown_rect = Rect::new(40.0, 160.0, 160.0, 26.0);
        let dropdown_popup_rect = Rect::new(40.0, 186.0, 160.0, 56.0);
        let mut popup = crate::PopupState::new(dropdown_rect);
        popup.open = true;
        popup.bounds = dropdown_popup_rect;
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: dropdown_rect,
                clip: None,
                z: 3,
                content_size: [dropdown_rect.width, dropdown_rect.height],
                dpi_scale: 1.0,
            },
            Dropdown {
                open: true,
                ..Dropdown::new(["First", "Second"], Some(0))
            },
            DropdownPopupState {
                popup,
                viewport,
                first_visible: 0,
                visible_count: 2,
            },
        ));

        let trigger = app
            .world_mut()
            .spawn(UiLayoutResult {
                window,
                rect: Rect::new(320.0, 120.0, 24.0, 24.0),
                clip: None,
                z: 4,
                content_size: [24.0, 24.0],
                dpi_scale: 1.0,
            })
            .id();
        let tooltip_owner = app
            .world_mut()
            .spawn((
                UiNode,
                Tooltip {
                    visible: true,
                    ..Tooltip::new("Themed popup tooltip")
                },
                UiLayoutResult {
                    window,
                    rect: Rect::new(320.0, 120.0, 24.0, 24.0),
                    clip: None,
                    z: 5,
                    content_size: [24.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.world_mut().resource_mut::<WidgetTooltips>().0.insert(
            window,
            TooltipHover {
                trigger,
                owner: tooltip_owner,
                elapsed_seconds: 1.0,
            },
        );

        collect_widget_paint(app.world_mut());
        let items = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        let shadows: Vec<_> = items
            .iter()
            .filter_map(|item| match &item.primitive {
                Primitive::Shadow {
                    rect,
                    shadow,
                    offset,
                    blur,
                    spread,
                    ..
                } => Some((*rect, *shadow, *offset, *blur, *spread, item.clip)),
                _ => None,
            })
            .collect();
        assert_eq!(
            shadows.len(),
            4,
            "menu, pie, dropdown, and tooltip each paint a shadow: {shadows:?}"
        );
        assert!(
            shadows
                .iter()
                .all(|(_, color, ..)| *color == expected_color)
        );
        assert!(shadows.iter().all(|(_, _, offset, blur, spread, _)| {
            let layer = theme.effects.popup_shadow.shadows[0];
            *offset == [layer.x_offset, layer.y_offset]
                && *blur == layer.blur_radius
                && *spread == layer.spread_radius
        }));
        assert!(
            shadows
                .iter()
                .all(|(_, _, _, _, _, clip)| *clip == ClipRect::from(viewport))
        );
        colors_by_theme.push(expected_color);
    }
    assert_ne!(colors_by_theme[0], colors_by_theme[1]);
}

#[test]
pub(super) fn menu_transition_uses_theme_motion_and_snaps_without_delaying_hitboxes() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut()
        .init_resource::<bbui_style::UiReducedMotion>();
    app.world_mut().resource_mut::<Theme>().motion.menu_open =
        std::time::Duration::from_millis(100);
    let window = app.world_mut().spawn_empty().id();
    let viewport = Rect::new(0.0, 0.0, 500.0, 360.0);
    app.world_mut()
        .spawn(UiLayoutRoot::new(window, viewport, 1.0));

    let mut menu = MenuControl::new(
        "Menu",
        crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
        MenuDisplay::Dropdown,
    );
    menu.width = 220.0;
    let anchor = Rect::new(20.0, 10.0, 80.0, 20.0);
    let owner = app
        .world_mut()
        .spawn((
            UiNode,
            menu,
            UiLayoutResult {
                window,
                rect: anchor,
                clip: None,
                z: 1,
                content_size: [anchor.width, anchor.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let row = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            MenuItemControl {
                owner,
                id: "open".to_owned(),
                label: "Open".to_owned(),
                parent_path: Vec::new(),
                index: 0,
                enabled: true,
                separator: false,
                has_submenu: false,
                shortcut: None,
            },
            UiLayoutResult {
                window,
                rect: Rect::new(0.0, 0.0, 1.0, 1.0),
                clip: None,
                z: 0,
                content_size: [1.0, 1.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .items
        .push(row);

    sync_menu_popups(app.world_mut());
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .open(None);
    sync_menu_popups(app.world_mut());
    let stable_menu_tick =
        entity_component_changed_tick::<MenuControl>(app.world(), owner).unwrap();
    let stable_owner_hitbox_tick =
        entity_component_changed_tick::<UiHitBox>(app.world(), owner).unwrap();
    let stable_row_control_tick =
        entity_component_changed_tick::<MenuItemControl>(app.world(), row).unwrap();
    let stable_row_layout_tick =
        entity_component_changed_tick::<UiLayoutResult>(app.world(), row).unwrap();
    app.world_mut().increment_change_tick();
    sync_menu_popups(app.world_mut());
    assert_eq!(
        entity_component_changed_tick::<MenuControl>(app.world(), owner),
        Some(stable_menu_tick),
        "idle menu sync must not touch its source component"
    );
    assert_eq!(
        entity_component_changed_tick::<UiHitBox>(app.world(), owner),
        Some(stable_owner_hitbox_tick),
        "idle menu sync must not rewrite the owner hitbox"
    );
    assert_eq!(
        entity_component_changed_tick::<MenuItemControl>(app.world(), row),
        Some(stable_row_control_tick)
    );
    assert_eq!(
        entity_component_changed_tick::<UiLayoutResult>(app.world(), row),
        Some(stable_row_layout_tick)
    );
    app.world_mut()
        .get_mut::<MenuItemControl>(row)
        .unwrap()
        .enabled = false;
    sync_menu_popups(app.world_mut());
    assert!(!app.world().get::<UiEnabled>(row).unwrap().0);
    advance_menu_motion_by(app.world_mut(), 0.05);

    let popup = app.world().get::<MenuControl>(owner).unwrap().popup.bounds;
    let owner_hit_rect = app.world().get::<UiHitBox>(owner).unwrap().rect;
    let row_rect = app.world().get::<UiHitBox>(row).unwrap().rect;
    assert_eq!(
        owner_hit_rect, popup,
        "open hit testing uses final popup bounds"
    );
    assert_eq!(
        row_rect,
        app.world().get::<UiLayoutResult>(row).unwrap().rect
    );
    let state = app
        .world()
        .resource::<WidgetMenuAnimations>()
        .0
        .get(&owner)
        .copied()
        .unwrap();
    assert!(
        (state.progress - 0.75).abs() < 0.001,
        "eased progress is stored in the shared track"
    );

    collect_widget_paint(app.world_mut());
    let progress =
        menu_animation_progress(app.world(), owner, app.world().resource::<Theme>()).unwrap();
    let animated_popup = interpolate_rect(anchor, popup, progress);
    let row_paint = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .iter()
        .find(|item| {
            matches!(
                &item.primitive,
                Primitive::RoundedRect { rect, .. }
                    if rect.x == row_rect.x
                        && rect.y == row_rect.y
                        && rect.width == row_rect.width
                        && rect.height == row_rect.height
            )
        })
        .unwrap();
    assert!(row_paint.clip.y >= animated_popup.y);
    assert!(row_paint.clip.y + row_paint.clip.height <= animated_popup.y + animated_popup.height);
    assert!(
        row_paint.clip.height < row_rect.height,
        "rows are clipped during opening"
    );

    app.world_mut().increment_change_tick();
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .close();
    sync_menu_popups(app.world_mut());
    assert_eq!(app.world().get::<UiHitBox>(owner).unwrap().rect, anchor);
    assert!(!app.world().get::<UiVisible>(row).unwrap().0);
    advance_menu_motion_by(app.world_mut(), 0.025);
    let state = app
        .world()
        .resource::<WidgetMenuAnimations>()
        .0
        .get(&owner)
        .copied()
        .unwrap();
    assert!((state.progress - 0.421875).abs() < 0.001);
    collect_widget_paint(app.world_mut());
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::Text { text, .. } if text == "Open"))
    );

    app.world_mut().increment_change_tick();
    app.world_mut()
        .resource_mut::<bbui_style::UiReducedMotion>()
        .0 = true;
    advance_menu_motion_by(app.world_mut(), 0.0);
    assert!(
        !app.world()
            .resource::<WidgetMenuAnimations>()
            .0
            .contains_key(&owner)
    );
    collect_widget_paint(app.world_mut());
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::Shadow { .. }))
    );
}

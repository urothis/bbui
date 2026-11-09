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
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

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
        unit: bui_core::UiWheelUnit::Lines,
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

#[test]
pub(super) fn text_area_uses_shared_shape_for_caret_paint_and_scrolling() {
    let mut app = App::new();
    app.add_plugins((bui_style::StylePlugin, bui_text::TextPlugin, WidgetPlugin));
    app.init_resource::<UiFocus>();
    app.world_mut()
        .resource_mut::<bevy_asset::Assets<bevy_text::Font>>()
        .insert(
            bevy_asset::AssetId::default(),
            bevy_text::Font::from_bytes(bevy_text::DEFAULT_FONT_DATA.to_vec()),
        )
        .unwrap();
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::text_area(
        "Notes",
        "first line with enough text\nsecond line",
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(20.0, 30.0, 120.0, 44.0),
        clip: None,
        z: 7,
        content_size: [120.0, 44.0],
        dpi_scale: 1.0,
    });
    {
        let mut layout = app.world_mut().get_mut::<UiTextLayout>(owner).unwrap();
        layout.font = Some(bevy_text::TextFont::from_font_size(18.0));
        layout.line_height = Some(LineHeight::Px(24.0));
        layout.letter_spacing = bevy_text::LetterSpacing::Px(0.5);
    }
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(owner);

    app.update();
    bui_text::update_text_layouts(app.world_mut());
    let first_origin = app
        .world()
        .get::<UiTextLayout>(owner)
        .unwrap()
        .text_origin()
        .expect("text layout should publish its origin");
    let text_layout = app.world().get::<UiTextLayout>(owner).unwrap();
    assert!(
        text_layout.layout().is_some(),
        "text area should use Bevy's shared text shaper: {:?}",
        text_layout.last_error()
    );
    let click_position = [first_origin[0] + 90.0, first_origin[1] + 5.0];
    let expected_caret = text_layout.hit_test(click_position).unwrap();
    assert!(expected_caret > 0);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerDown {
        window,
        target: owner,
        button: UiPointerButton::Primary,
        position: click_position,
    }];
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(owner).unwrap().caret(),
        expected_caret
    );

    app.world_mut()
        .resource_mut::<UiRoutedEvents>()
        .events
        .clear();
    app.world_mut()
        .get_mut::<TextBuffer>(owner)
        .unwrap()
        .select_all();
    app.update();
    collect_widget_paint(app.world_mut());
    let paint = app.world().resource::<DisplayList>().widget_items();
    let text = paint.iter().find_map(|item| match &item.primitive {
        Primitive::TextWrapped {
            wrap,
            max_width,
            style: Some(style),
            ..
        } => Some((*wrap, *max_width, style)),
        _ => None,
    });
    let (wrap, max_width, style) = text.expect("editable area should emit wrapped text paint");
    assert!(wrap);
    assert!(max_width > 0.0 && max_width < 120.0);
    assert_eq!(
        style.font.as_ref().unwrap().font_size,
        bevy_text::FontSize::Px(18.0)
    );
    assert_eq!(style.line_height, Some(LineHeight::Px(24.0)));
    assert_eq!(style.letter_spacing, bevy_text::LetterSpacing::Px(0.5));
    assert!(
        paint
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Line { .. }))
    );
    assert!(paint.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { color, .. } if color.3 == 0.32
    )));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::ImePreedit {
        window,
        target: owner,
        text: "xy".into(),
        cursor_range: Some([0, 2]),
    }];
    bui_text::process_text_events(app.world_mut());
    bui_text::update_text_layouts(app.world_mut());
    sync_text_controls(app.world_mut());
    collect_widget_paint(app.world_mut());
    let preedit_rects = app
        .world()
        .get::<UiTextLayout>(owner)
        .unwrap()
        .preedit_rects();
    assert!(!preedit_rects.is_empty());
    let preedit_paint = app.world().resource::<DisplayList>().widget_items();
    assert!(preedit_paint.iter().any(|item| {
        let Primitive::Line { from, to, .. } = &item.primitive else {
            return false;
        };
        preedit_rects.iter().any(|rect| {
            let y = rect.y + rect.height.max(1.0) - 1.0;
            (from[0] - rect.x).abs() < 0.01
                && (from[1] - y).abs() < 0.01
                && (to[0] - (rect.x + rect.width)).abs() < 0.01
                && (to[1] - y).abs() < 0.01
        })
    }));
    for kind in [bui_style::ThemeKind::Dark, bui_style::ThemeKind::Light] {
        let theme = Theme::new(kind);
        app.world_mut()
            .resource_mut::<UiRoutedEvents>()
            .events
            .clear();
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        app.update();
        bui_style::resolve_ui_styles(app.world_mut());
        collect_widget_paint(app.world_mut());
        let layout = app.world().get::<UiTextLayout>(owner).unwrap();
        let buffer = app.world().get::<TextBuffer>(owner).unwrap();
        let caret = layout.caret_rect(buffer).unwrap();
        let preedit_rects = layout.preedit_rects();
        let paint = app.world().resource::<DisplayList>().widget_items();
        assert!(
            paint.iter().any(|item| {
                let Primitive::Line {
                    from, to, color, ..
                } = &item.primitive
                else {
                    return false;
                };
                *color == theme.color(ThemeColorToken::EditorCaret)
                    && (from[0] - caret.x).abs() < 0.01
                    && (from[1] - caret.y).abs() < 0.01
                    && (to[0] - caret.x).abs() < 0.01
                    && (to[1] - (caret.y + caret.height)).abs() < 0.01
            }),
            "{kind:?} caret={caret:?}"
        );
        assert!(paint.iter().any(|item| {
            let Primitive::Line {
                from, to, color, ..
            } = &item.primitive
            else {
                return false;
            };
            *color == theme.color(ThemeColorToken::EditorImeUnderline)
                && preedit_rects.iter().any(|rect| {
                    let y = rect.y + rect.height.max(1.0) - 1.0;
                    (from[0] - rect.x).abs() < 0.01
                        && (from[1] - y).abs() < 0.01
                        && (to[0] - (rect.x + rect.width)).abs() < 0.01
                        && (to[1] - y).abs() < 0.01
                })
        }));
    }
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::ImeDisabled {
        window,
        target: owner,
    }];
    bui_text::process_text_events(app.world_mut());
    bui_text::update_text_layouts(app.world_mut());

    app.world_mut()
        .resource_mut::<UiRoutedEvents>()
        .events
        .clear();
    app.world_mut()
        .get_mut::<TextBuffer>(owner)
        .unwrap()
        .collapse_to(0);
    app.world_mut()
        .get_mut::<TextArea>(owner)
        .unwrap()
        .scroll_line = 0;
    app.world_mut()
        .get_mut::<UiTextLayout>(owner)
        .unwrap()
        .scroll_offset[1] = 0.0;
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = None;
    app.update();

    let scroll_before = app.world().get::<TextArea>(owner).unwrap().scroll_line;
    let max_scroll = app
        .world()
        .get::<UiTextLayout>(owner)
        .unwrap()
        .layout()
        .unwrap()
        .bounds()
        .height;
    let max_scroll = (max_scroll - 38.0).max(0.0);
    let max_scroll_lines = (max_scroll / 24.0).round() as usize;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [30.0, 40.0],
        delta: [0.0, -2.0],
        unit: bui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    let scrolled_line = app.world().get::<TextArea>(owner).unwrap().scroll_line;
    assert_eq!(scrolled_line, (scroll_before + 2).min(max_scroll_lines));
    assert_eq!(
        app.world()
            .get::<UiTextLayout>(owner)
            .unwrap()
            .scroll_offset[1],
        scrolled_line as f32 * 24.0
    );
    let layout = app.world().get::<UiTextLayout>(owner).unwrap();
    let layout_result = app.world().get::<UiLayoutResult>(owner).unwrap();
    let padding_y = app
        .world()
        .get::<UiLayoutNode>(owner)
        .map_or(0.0, |node| node.padding.top);
    let expected_origin_y = layout_result.rect.y + padding_y + layout.inset[1].max(0.0)
        - layout.scroll_offset[1].max(0.0);
    let actual_origin_y = layout.text_origin().unwrap()[1];
    assert!(
        (actual_origin_y - expected_origin_y).abs() < 0.01,
        "origin={actual_origin_y} expected={expected_origin_y} rect_y={} padding_y={padding_y} inset_y={} scroll_y={}",
        layout_result.rect.y,
        layout.inset[1],
        layout.scroll_offset[1]
    );
    app.world_mut()
        .resource_mut::<UiRoutedEvents>()
        .events
        .clear();
    app.update();
}

#[test]
pub(super) fn widget_tree_file_browser_virtualizes_rows_selects_filters_and_opens_directories() {
    let directory = std::env::temp_dir().join(format!(
        "bui-widget-browser-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(directory.join("subfolder")).unwrap();
    std::fs::write(directory.join("alpha.txt"), "a").unwrap();
    std::fs::write(directory.join("beta.bin"), "b").unwrap();
    std::fs::write(directory.join("subfolder/nested.txt"), "n").unwrap();
    // FileBrowser entries use canonical paths; macOS may canonicalize
    // temp_dir's `/var` prefix to `/private/var`.
    let directory = std::fs::canonicalize(directory).unwrap();

    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::file_browser(directory.clone()))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 360.0, 170.0),
        clip: None,
        z: 5,
        content_size: [360.0, 170.0],
        dpi_scale: 1.0,
    });
    app.update();

    let visible_items = browser_items(app.world_mut(), owner);
    assert_eq!(visible_items.len(), 3);
    assert!(
        visible_items
            .iter()
            .all(|(entity, _)| app.world().get::<UiHitBox>(*entity).is_some())
    );
    let alpha = visible_items
        .iter()
        .find(|(_, item)| item.name == "alpha.txt")
        .unwrap()
        .clone();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: alpha.0,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [8.0, 54.0],
    }];
    app.update();
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .selected
            .contains(&directory.join("alpha.txt"))
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileSelected { path, selected: true, .. } if path.ends_with("alpha.txt"))));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "alpha".into(),
    }];
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().filter(),
        "alpha"
    );
    assert_eq!(browser_items(app.world_mut(), owner).len(), 2); // Directories stay visible while filtering.

    let subfolder = browser_items(app.world_mut(), owner)
        .into_iter()
        .find(|(_, item)| item.name == "subfolder")
        .unwrap();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: subfolder.0,
        button: UiPointerButton::Primary,
        click_count: 2,
        position: [10.0, 54.0],
    }];
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().directory,
        std::fs::canonicalize(directory.join("subfolder")).unwrap()
    );
    assert!(
        app.world()
            .get::<FileBrowserItem>(subfolder.0)
            .is_none_or(|item| !item.selected)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
pub(super) fn list_view_virtualizes_stable_items_and_paints_clipped_rows() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let model = ListModel {
        items: (0..2_000)
            .map(|index| ListItem {
                id: index + 1,
                value: format!("Asset {index:04}"),
                enabled: index % 13 != 0,
            })
            .collect(),
        ..ListModel::default()
    };
    let owner = WidgetTree::new(WidgetNode::list("Assets", model))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(12.0, 16.0, 240.0, 130.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 4,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    let visible = collection_items(app.world_mut(), owner);
    assert!((5..=7).contains(&visible.len()));
    assert!(visible.iter().all(|(entity, item)| {
        item.kind == CollectionItemKind::List
            && app
                .world()
                .get::<UiParent>(*entity)
                .is_some_and(|parent| parent.0 == owner)
            && app.world().get::<UiHitBox>(*entity).is_some_and(|hit| {
                hit.window == Some(window)
                    && hit.clip.is_some_and(|clip| {
                        clip.y >= rect.y + 28.0 && clip.bottom() <= rect.bottom()
                    })
            })
    }));
    assert!(visible.len() < 2_000);
    let stable_id = visible.iter().find(|(_, item)| item.id == 5).unwrap().0;
    let first = visible.iter().map(|(_, item)| item.id).min().unwrap();
    assert_eq!(first, 1);
    assert!(app.world().resource::<DisplayList>().widget_items().iter().any(
            |draw| matches!(&draw.primitive, Primitive::TextWrapped { text, .. } if text.contains("Asset 0000"))
        ));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [30.0, 80.0],
        delta: [0.0, -1.0],
        unit: bui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    let scrolled = collection_items(app.world_mut(), owner);
    assert_eq!(scrolled.iter().map(|(_, item)| item.id).min(), Some(2));
    assert_eq!(
        app.world()
            .resource::<WidgetCollectionItems>()
            .items
            .get(&owner)
            .and_then(|items| items.get(&5)),
        Some(&stable_id)
    );
}

#[test]
pub(super) fn list_keyboard_routes_page_edges_select_all_activate_and_rename_cancel() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let model = ListModel {
        items: (0..10)
            .map(|index| ListItem {
                id: index + 1,
                value: format!("Asset {index}"),
                enabled: true,
            })
            .collect(),
        ..ListModel::default()
    };
    let owner = WidgetTree::new(WidgetNode::list("Assets", model))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 240.0, 150.0),
        clip: None,
        z: 0,
        content_size: [240.0, 150.0],
        dpi_scale: 1.0,
    });
    app.update();
    let send_key = |app: &mut App, key, modifiers| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        app.update();
    };

    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    let page_row = app
        .world()
        .get::<ListModel<String>>(owner)
        .unwrap()
        .selection
        .active
        .unwrap();
    assert!(page_row > 1);
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(10)
    );
    send_key(
        &mut app,
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .selected
            .len(),
        10
    );
    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::CollectionItemActivated { target, id: 1 } if *target == owner)));

    send_key(&mut app, UiKeyCode::F2, UiModifiers::default());
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "Cancelled rename".to_owned(),
    }];
    app.update();
    send_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
    assert_eq!(
        app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
        "Asset 0"
    );
    assert!(
        app.world()
            .get::<CollectionEditorState>(owner)
            .unwrap()
            .rename_id
            .is_none()
    );
}

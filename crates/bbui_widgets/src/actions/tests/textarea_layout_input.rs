use super::*;

#[test]
pub(super) fn text_area_uses_shared_shape_for_caret_paint_and_scrolling() {
    let mut app = App::new();
    app.add_plugins((bbui_style::StylePlugin, bbui_text::TextPlugin, WidgetPlugin));
    app.init_resource::<UiFocus>();
    app.world_mut()
        .resource_mut::<bevy_asset::Assets<bevy_text::Font>>()
        .insert(
            bevy_asset::AssetId::default(),
            bevy_text::Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()),
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
    bbui_text::update_text_layouts(app.world_mut());
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
    bbui_text::process_text_events(app.world_mut());
    bbui_text::update_text_layouts(app.world_mut());
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
    for kind in [bbui_style::ThemeKind::Dark, bbui_style::ThemeKind::Light] {
        let theme = Theme::new(kind);
        app.world_mut()
            .resource_mut::<UiRoutedEvents>()
            .events
            .clear();
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        app.update();
        bbui_style::resolve_ui_styles(app.world_mut());
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
    bbui_text::process_text_events(app.world_mut());
    bbui_text::update_text_layouts(app.world_mut());

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
        unit: bbui_core::UiWheelUnit::Lines,
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

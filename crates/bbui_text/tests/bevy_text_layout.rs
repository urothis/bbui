use bbui_core::{CorePlugin, UiFocus, UiParent, UiRect};
use bbui_layout::{Rect, UiIntrinsicSize, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bbui_style::{
    StylePlugin, UiAnimationSpec, UiAutoAnimations, UiInlineStyle, UiStyleProperty, UiStyleValue,
};
use bbui_text::{
    BevyTextLayoutConfig, TextBuffer, TextLayout, TextRange, TextRect, UiTextLayout,
    measure_text_intrinsics, ui_default_text_font, update_text_layouts,
};
use bevy_app::{App, Update};
use bevy_asset::{AssetId, Assets};
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{Res, ResMut, SystemState};
use bevy_math::Vec2;
use bevy_text::{
    Font, FontCx, FontSource, Justify, LayoutCx, LetterSpacing, LineBreak, LineHeight, RemSize,
    TextFont, TextPipeline,
};
use std::time::Duration;

fn assert_logical_rect_matches_shape(
    logical: TextRect,
    shaped: TextRect,
    origin: [f32; 2],
    scale: f32,
) {
    assert!((logical.x - (origin[0] + shaped.x / scale)).abs() < 0.1);
    assert!((logical.y - (origin[1] + shaped.y / scale)).abs() < 0.1);
    assert!((logical.width - shaped.width / scale).abs() < 0.1);
    assert!((logical.height - shaped.height / scale).abs() < 0.1);
}

#[test]
fn bevy_shaping_drives_unicode_wrapping_hit_testing_and_selection() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    app.update();

    let text = "AV🙂 café e\u{301} wraps UI";
    let font = TextFont::from_font_size(16.0).with_font(font_handle);
    let mut state = SystemState::<(
        ResMut<TextPipeline>,
        Res<Assets<Font>>,
        ResMut<FontCx>,
        ResMut<LayoutCx>,
    )>::new(app.world_mut());
    let (mut pipeline, fonts, mut font_cx, mut layout_cx) = state
        .get_mut(app.world_mut())
        .expect("the text pipeline resources are initialized");
    let layout = TextLayout::shape_with_bevy(
        text,
        60.0,
        &font,
        &mut pipeline,
        &fonts,
        &mut font_cx,
        &mut layout_cx,
    )
    .expect("the registered Bevy font should shape successfully");
    state.apply(app.world_mut());

    assert!(layout.line_count() > 1);
    assert!(layout.bounds().height > 16.0);
    assert!(
        layout
            .glyphs()
            .iter()
            .any(|glyph| glyph.byte_range.len() > 1)
    );
    for glyph in layout.glyphs() {
        assert!(text.is_char_boundary(glyph.byte_range.start));
        assert!(text.is_char_boundary(glyph.byte_range.end));
        assert!(glyph.x.is_finite() && glyph.y.is_finite() && glyph.advance.is_finite());
        let byte = layout.hit_test(
            glyph.x + glyph.advance * 0.5,
            glyph.y + layout.caret_rect(glyph.byte_range.start).height * 0.5,
        );
        assert!(
            byte == glyph.byte_range.start || byte == glyph.byte_range.end,
            "hit test {byte} should land on cluster {:?}",
            glyph.byte_range
        );
    }

    let selection = layout.selection_rects(0..text.len());
    assert!(selection.len() > 1, "selection should follow wrapped lines");
    let caret = layout.caret_rect(text.len());
    assert!(caret.x.is_finite() && caret.y.is_finite());
    assert!(caret.y > 0.0);
}

#[test]
fn ui_text_layout_keeps_hit_testing_and_caret_geometry_logical_at_dpi_scale() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    app.update();

    let mut text_layout = UiTextLayout::default();
    text_layout.font = Some(TextFont::from_font_size(16.0).with_font(font_handle));
    text_layout.scale_factor = Some(2.0);
    text_layout.inset = [0.0, 0.0];
    let widget = app
        .world_mut()
        .spawn((
            TextBuffer::new("abc"),
            text_layout,
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(10.0, 20.0, 200.0, 40.0),
                clip: None,
                z: 0,
                content_size: [200.0, 40.0],
                dpi_scale: 2.0,
            },
        ))
        .id();
    update_text_layouts(app.world_mut());

    let layout = app
        .world()
        .get::<UiTextLayout>(widget)
        .expect("text layout component remains attached");
    let shaped = layout.layout().expect("Bevy text should shape");
    let first_glyph = &shaped.glyphs()[0];
    let raw_caret = shaped.caret_rect(3);
    let buffer = app.world().get::<TextBuffer>(widget).unwrap();
    let logical_caret = layout.caret_rect(buffer).unwrap();
    assert!((logical_caret.height * 2.0 - raw_caret.height).abs() < 0.1);
    assert!((logical_caret.x - (10.0 + raw_caret.x / 2.0)).abs() < 0.1);

    // Hit testing accepts logical window coordinates even though Parley stores
    // the Bevy layout at the window's physical-pixel scale.
    let point = [
        10.0 + (first_glyph.x + first_glyph.advance * 0.2) / 2.0,
        20.0 + raw_caret.height / 8.0,
    ];
    assert_eq!(layout.hit_test(point), Some(0));
}

#[test]
fn wrapped_selection_caret_and_hit_testing_match_across_dpi_scales() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    let fallback_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_SYMBOL_FONT_DATA.to_vec()));
    app.update();

    let text = "The quick brown fox wraps over several lines \u{1F7D5}".to_string();
    let selection = TextRange::new(4, text.len());
    let focus_byte = text.find("wraps").unwrap();
    let fallback_start = text.len() - '\u{1F7D5}'.len_utf8();
    let mut font = TextFont::from_font_size(16.0).with_font(font_handle.clone());
    font.font = FontSource::list([
        FontSource::Handle(font_handle),
        FontSource::Handle(fallback_handle),
    ]);
    let mut layout_1x = UiTextLayout::text_area("", true);
    layout_1x.font = Some(font);
    layout_1x.inset = [0.0, 0.0];
    let mut layout_2x = layout_1x.clone();
    layout_2x.scale_factor = Some(2.0);
    let mut buffer_1x = TextBuffer::new(text.clone());
    buffer_1x.set_selection(selection.start, selection.end);
    let buffer_2x = buffer_1x.clone();

    let window = app.world_mut().spawn_empty().id();
    let rect = UiRect::new(10.0, 20.0, 88.0, 150.0);
    let entity_1x = app
        .world_mut()
        .spawn((
            buffer_1x,
            layout_1x,
            UiLayoutResult {
                window,
                rect,
                clip: None,
                z: 0,
                content_size: [rect.width, rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let entity_2x = app
        .world_mut()
        .spawn((
            buffer_2x,
            layout_2x,
            UiLayoutResult {
                window,
                rect,
                clip: None,
                z: 0,
                content_size: [rect.width, rect.height],
                dpi_scale: 2.0,
            },
        ))
        .id();
    update_text_layouts(app.world_mut());

    let world = app.world();
    let ui_1x = world.get::<UiTextLayout>(entity_1x).unwrap();
    let ui_2x = world.get::<UiTextLayout>(entity_2x).unwrap();
    let shaped_1x = ui_1x.layout().unwrap();
    let shaped_2x = ui_2x.layout().unwrap();
    assert!(shaped_1x.line_count() > 1);
    assert_eq!(shaped_1x.line_count(), shaped_2x.line_count());
    assert!((shaped_2x.bounds().width / 2.0 - shaped_1x.bounds().width).abs() < 0.1);
    assert!((shaped_2x.bounds().height / 2.0 - shaped_1x.bounds().height).abs() < 0.1);

    let buffer_1x = world.get::<TextBuffer>(entity_1x).unwrap();
    let buffer_2x = world.get::<TextBuffer>(entity_2x).unwrap();
    let caret_1x = ui_1x.caret_rect(buffer_1x).unwrap();
    let caret_2x = ui_2x.caret_rect(buffer_2x).unwrap();
    assert_logical_rect_matches_shape(
        caret_1x,
        shaped_1x.caret_rect(buffer_1x.caret()),
        [rect.x, rect.y],
        1.0,
    );
    assert_logical_rect_matches_shape(
        caret_2x,
        shaped_2x.caret_rect(buffer_2x.caret()),
        [rect.x, rect.y],
        2.0,
    );
    assert!((caret_1x.x - caret_2x.x).abs() < 0.1);
    // Bevy rounds font metrics to physical pixels. At 2x this can shift a
    // logical baseline by half a pixel; each editor metric must still agree
    // with its corresponding renderer layout above.
    assert!((caret_1x.y - caret_2x.y).abs() <= 0.51);
    assert!((caret_1x.height - caret_2x.height).abs() < 0.1);

    let selection_1x = ui_1x.selection_rects(buffer_1x);
    let selection_2x = ui_2x.selection_rects(buffer_2x);
    let shaped_selection_1x = shaped_1x.selection_rects(selection.start..selection.end);
    let shaped_selection_2x = shaped_2x.selection_rects(selection.start..selection.end);
    assert!(selection_1x.len() > 1, "selection crosses wrapped lines");
    assert_eq!(selection_1x.len(), selection_2x.len());
    assert_eq!(selection_1x.len(), shaped_selection_1x.len());
    assert_eq!(selection_2x.len(), shaped_selection_2x.len());
    for (logical, shaped) in selection_1x.iter().copied().zip(shaped_selection_1x) {
        assert_logical_rect_matches_shape(logical, shaped, [rect.x, rect.y], 1.0);
    }
    for (logical, shaped) in selection_2x.iter().copied().zip(shaped_selection_2x) {
        assert_logical_rect_matches_shape(logical, shaped, [rect.x, rect.y], 2.0);
    }
    for (one, two) in selection_1x.iter().zip(&selection_2x) {
        assert!((one.x - two.x).abs() <= 0.51);
        assert!((one.y - two.y).abs() <= 0.51);
        assert!((one.width - two.width).abs() <= 0.51);
        assert!((one.height - two.height).abs() <= 0.51);
    }

    let glyph = shaped_1x
        .glyphs()
        .iter()
        .find(|glyph| glyph.byte_range.start == focus_byte)
        .expect("the wrapped word has a cluster placement");
    let click = [
        rect.x + glyph.x + glyph.advance * 0.2,
        rect.y + glyph.y + shaped_1x.caret_rect(focus_byte).height * 0.5,
    ];
    assert_eq!(ui_1x.hit_test(click), ui_2x.hit_test(click));
    assert_eq!(ui_1x.hit_test(click), Some(focus_byte));

    let fallback_glyph = shaped_1x
        .glyphs()
        .iter()
        .find(|glyph| {
            glyph.byte_range.start <= fallback_start && glyph.byte_range.end > fallback_start
        })
        .expect("the fallback glyph has a cluster placement");
    let fallback_click = [
        rect.x + fallback_glyph.x + fallback_glyph.advance * 0.5,
        rect.y + fallback_glyph.y + shaped_1x.caret_rect(fallback_start).height * 0.5,
    ];
    let fallback_hit_1x = ui_1x.hit_test(fallback_click);
    let fallback_hit_2x = ui_2x.hit_test(fallback_click);
    assert_eq!(fallback_hit_1x, fallback_hit_2x);
    assert!(matches!(fallback_hit_1x, Some(hit) if hit == fallback_start || hit == text.len()));
}

#[test]
fn wrapped_intrinsic_measurement_matches_final_layout_with_fallback_at_different_dpi() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    let primary = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    let fallback = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_SYMBOL_FONT_DATA.to_vec()));
    app.update();

    let mut font = TextFont::from_font_size(16.0).with_font(primary.clone());
    font.font = FontSource::list([FontSource::Handle(primary), FontSource::Handle(fallback)]);
    let text = "The quick brown fox wraps over several lines \u{1F7D5}".to_string();
    let mut fields = Vec::new();
    for dpi_scale in [1.0, 2.0] {
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn(UiLayoutRoot::new(
                window,
                Rect::new(0.0, 0.0, 320.0, 200.0),
                dpi_scale,
            ))
            .id();
        let mut layout = UiTextLayout::text_area("", true);
        layout.font = Some(font.clone());
        let field = app
            .world_mut()
            .spawn((
                TextBuffer::new(text.clone()),
                layout,
                UiLayoutNode::leaf(UiLength::Px(88.0), UiLength::Auto),
                UiParent(root),
            ))
            .id();
        fields.push((field, window, dpi_scale));
    }

    measure_text_intrinsics(app.world_mut());
    let measured = fields
        .iter()
        .map(|(field, _, _)| {
            app.world()
                .get::<UiIntrinsicSize>(*field)
                .expect("text measurement publishes an intrinsic size")
                .0
        })
        .collect::<Vec<_>>();

    for (field, window, dpi_scale) in &fields {
        app.world_mut().entity_mut(*field).insert(UiLayoutResult {
            window: *window,
            rect: UiRect::new(10.0, 20.0, 88.0, 150.0),
            clip: None,
            z: 0,
            content_size: [88.0, 150.0],
            dpi_scale: *dpi_scale,
        });
    }
    update_text_layouts(app.world_mut());

    let world = app.world();
    for ((field, _, dpi_scale), intrinsic) in fields.iter().zip(&measured) {
        let layout = world.get::<UiTextLayout>(*field).unwrap();
        let shaped = layout.layout().expect("final widget layout is shaped");
        let scale = *dpi_scale;
        assert!(shaped.line_count() > 1);
        assert!((intrinsic[0] - (shaped.bounds().width / scale + 12.0)).abs() < 0.1);
        assert!((intrinsic[1] - (shaped.bounds().height / scale + 6.0)).abs() < 0.1);
        assert!(
            shaped
                .glyphs()
                .iter()
                .any(|glyph| glyph.byte_range.start <= text.len() - 4
                    && glyph.byte_range.end > text.len() - 4),
            "the shaped layout includes the fallback symbol"
        );
    }
    assert!((measured[0][0] - measured[1][0]).abs() <= 0.51);
    assert!((measured[0][1] - measured[1][1]).abs() <= 0.51);
}

#[test]
fn ui_text_layout_shapes_preedit_replacing_selection_and_exposes_underline_geometry() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    app.update();

    let mut buffer = TextBuffer::new("abXYz");
    buffer.set_selection(2, 4);
    buffer.set_composition("🙂", Some(TextRange::new(0, 4)));
    let mut ui_text = UiTextLayout::text_field("N");
    ui_text.font = Some(TextFont::from_font_size(16.0).with_font(font_handle));
    ui_text.inset = [0.0, 0.0];
    let widget = app
        .world_mut()
        .spawn((
            buffer,
            ui_text,
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(20.0, 10.0, 200.0, 40.0),
                clip: None,
                z: 0,
                content_size: [200.0, 40.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    update_text_layouts(app.world_mut());

    let world = app.world();
    let buffer = world.get::<TextBuffer>(widget).unwrap();
    let ui_text = world.get::<UiTextLayout>(widget).unwrap();
    assert_eq!(ui_text.display_text(), Some("N: ab🙂z"));
    assert_eq!(ui_text.preedit_range(), Some(5..9));
    assert!(ui_text.selection_rects(buffer).is_empty());
    let preedit_rects = ui_text.preedit_rects();
    assert!(!preedit_rects.is_empty());
    assert!(
        preedit_rects
            .iter()
            .all(|rect| rect.width > 0.0 && rect.height > 0.0)
    );

    let shaped = ui_text.layout().unwrap();
    let raw_caret = shaped.caret_rect(9);
    let caret = ui_text.caret_rect(buffer).unwrap();
    assert!((caret.x - (20.0 + raw_caret.x)).abs() < 0.1);
    assert!((caret.y - (10.0 + raw_caret.y)).abs() < 0.1);

    let rect = preedit_rects[0];
    let hit = ui_text.hit_test([rect.x + rect.width * 0.5, rect.y + rect.height * 0.5]);
    assert!(
        matches!(hit, Some(2 | 4)),
        "preedit hit maps to replaced text edge: {hit:?}"
    );
}

#[test]
fn focused_text_fields_and_areas_scroll_caret_into_view() {
    let mut app = App::new();
    app.init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .init_resource::<UiFocus>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    app.update();
    let window = app.world_mut().spawn_empty().id();
    let mut field_layout = UiTextLayout::default();
    field_layout.font = Some(TextFont::from_font_size(16.0).with_font(font_handle.clone()));
    field_layout.inset = [0.0, 0.0];
    let field = app
        .world_mut()
        .spawn((
            TextBuffer::new("abcdefghijk"),
            field_layout,
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 42.0, 24.0),
                clip: None,
                z: 0,
                content_size: [42.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let mut area_layout = UiTextLayout::text_area("", false);
    area_layout.font = Some(TextFont::from_font_size(16.0).with_font(font_handle));
    area_layout.inset = [0.0, 0.0];
    let area = app
        .world_mut()
        .spawn((
            TextBuffer::new("one\ntwo\nthree\nfour\nfive"),
            area_layout,
            UiLayoutResult {
                window,
                rect: UiRect::new(60.0, 0.0, 70.0, 24.0),
                clip: None,
                z: 0,
                content_size: [70.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(field);
    update_text_layouts(app.world_mut());
    {
        let world = app.world();
        let layout = world.get::<UiTextLayout>(field).unwrap();
        assert!(layout.scroll_offset[0] > 0.0);
        let caret = layout
            .caret_rect(world.get::<TextBuffer>(field).unwrap())
            .unwrap();
        assert!(
            caret.x >= -0.1 && caret.x + caret.width <= 42.1,
            "field caret {:?}, offset {:?}, bounds {:?}",
            caret,
            layout.scroll_offset,
            layout.layout().unwrap().bounds()
        );
        assert_eq!(
            world.get::<UiTextLayout>(area).unwrap().scroll_offset[1],
            0.0
        );
    }

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(area);
    update_text_layouts(app.world_mut());
    let world = app.world();
    let layout = world.get::<UiTextLayout>(area).unwrap();
    assert!(layout.scroll_offset[1] > 0.0);
    let caret = layout
        .caret_rect(world.get::<TextBuffer>(area).unwrap())
        .unwrap();
    assert!(caret.y >= -0.1 && caret.y + caret.height <= 24.1);
}

#[test]
fn text_metrics_follow_interpolated_computed_font_and_line_height() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, StylePlugin))
        .init_resource::<Assets<Font>>()
        .init_resource::<FontCx>()
        .init_resource::<LayoutCx>()
        .init_resource::<TextPipeline>()
        .init_resource::<bevy_time::Time>()
        .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            AssetId::default(),
            Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()),
        )
        .expect("the default font asset slot is empty");
    app.update();
    let window = app.world_mut().spawn_empty().id();
    let mut inline = UiInlineStyle::default();
    inline.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(16.0));
    inline.set(UiStyleProperty::LineHeight, UiStyleValue::Scalar(1.0));
    let mut transitions = UiAutoAnimations::default();
    transitions.set(
        UiStyleProperty::FontSize,
        UiAnimationSpec::new(Duration::from_secs(1)),
    );
    transitions.set(
        UiStyleProperty::LineHeight,
        UiAnimationSpec::new(Duration::from_secs(1)),
    );
    let widget = app
        .world_mut()
        .spawn((
            bbui_core::UiNode,
            TextBuffer::new("abc"),
            UiTextLayout::default(),
            inline,
            transitions,
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 200.0, 50.0),
                clip: None,
                z: 0,
                content_size: [200.0, 50.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();
    update_text_layouts(app.world_mut());
    let initial_height = app
        .world()
        .get::<UiTextLayout>(widget)
        .unwrap()
        .caret_rect(app.world().get::<TextBuffer>(widget).unwrap())
        .unwrap()
        .height;
    assert!(initial_height > 0.0);

    {
        let mut inline = app.world_mut().get_mut::<UiInlineStyle>(widget).unwrap();
        inline.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(24.0));
        inline.set(UiStyleProperty::LineHeight, UiStyleValue::Scalar(2.0));
    }
    app.update();
    update_text_layouts(app.world_mut());
    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(Duration::from_millis(500));
    app.update();
    update_text_layouts(app.world_mut());

    let (font_size, line_height, caret_height) = {
        let world = app.world();
        let style = world.get::<bbui_style::UiComputedStyle>(widget).unwrap();
        let caret = world
            .get::<UiTextLayout>(widget)
            .unwrap()
            .caret_rect(world.get::<TextBuffer>(widget).unwrap())
            .unwrap();
        (
            style.scalar(UiStyleProperty::FontSize).unwrap(),
            style.scalar(UiStyleProperty::LineHeight).unwrap(),
            caret.height,
        )
    };
    assert!((font_size - 20.0).abs() < 0.001);
    assert!((line_height - 1.5).abs() < 0.001);
    assert!(caret_height > initial_height);
    let expected_font = ui_default_text_font(font_size);
    let mut shape_state = SystemState::<(
        ResMut<TextPipeline>,
        Res<Assets<Font>>,
        ResMut<FontCx>,
        ResMut<LayoutCx>,
    )>::new(app.world_mut());
    let (mut pipeline, fonts, mut font_cx, mut layout_cx) = shape_state
        .get_mut(app.world_mut())
        .expect("text shaping resources remain initialized");
    let expected = TextLayout::shape_with_bevy_config(
        "abc",
        188.0,
        BevyTextLayoutConfig {
            font: &expected_font,
            line_height: LineHeight::RelativeToFont(line_height),
            letter_spacing: LetterSpacing::default(),
            line_break: LineBreak::NoWrap,
            justify: Justify::default(),
            scale_factor: 1.0,
            logical_viewport_size: Vec2::ZERO,
            rem_size: RemSize::default(),
        },
        &mut pipeline,
        &fonts,
        &mut font_cx,
        &mut layout_cx,
    )
    .expect("computed style metrics shape through Bevy's text pipeline");
    shape_state.apply(app.world_mut());
    assert!((caret_height - expected.caret_rect(3).height).abs() < 0.1);
}

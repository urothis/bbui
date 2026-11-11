use bbui_core::{
    CorePlugin, UiFocus, UiKeyCode, UiModifiers, UiNode, UiRoutedEvent, UiRoutedEvents, UiSystems,
    set_parent,
};
use bbui_layout::{
    LayoutPlugin, Rect, UiIntrinsicSize, UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot,
    UiLength, UiSize,
};
use bbui_style::StylePlugin;
use bbui_text::{TextBuffer, UiMultiline, UiTextLayout, measure_text_intrinsics};
use bevy_app::{App, Update};
use bevy_asset::Assets;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_text::{Font, TextFont};

#[test]
fn text_widget_uses_shared_shaping_for_layout_and_edit_geometry() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin, StylePlugin, bbui_text::TextPlugin));
    app.add_systems(
        Update,
        bevy_text::load_font_assets_into_font_collection
            .in_set(UiSystems::TextMeasure)
            .before(measure_text_intrinsics),
    );

    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 320.0, 200.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(320.0),
                    height: UiLength::Px(200.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let mut text_layout = UiTextLayout::text_area("", true);
    text_layout.font = Some(TextFont::from_font_size(16.0).with_font(font_handle));
    text_layout.inset = [6.0, 3.0];
    let field = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(96.0), UiLength::Px(72.0)),
            TextBuffer::new("AV🙂 café e\u{301} wraps across words"),
            text_layout,
        ))
        .id();
    set_parent(app.world_mut(), field, root).expect("text widget attaches to its root");

    app.update();

    let world = app.world();
    let result = world
        .get::<UiLayoutResult>(field)
        .copied()
        .expect("layout pass publishes widget geometry");
    let intrinsic = world
        .get::<UiIntrinsicSize>(field)
        .expect("text measurement publishes intrinsic size");
    assert!(intrinsic.0[0].is_finite() && intrinsic.0[1].is_finite());
    let buffer = world.get::<TextBuffer>(field).unwrap();
    let metrics = world.get::<UiTextLayout>(field).unwrap();
    let shaped = metrics.layout().expect("TextPlugin shapes the widget text");
    assert!(shaped.line_count() > 1);
    assert!(
        shaped
            .glyphs()
            .iter()
            .any(|glyph| glyph.byte_range.len() > 1)
    );
    assert_eq!(
        metrics.text_origin(),
        Some([result.rect.x + 6.0, result.rect.y + 3.0])
    );

    let glyph = &shaped.glyphs()[0];
    let point = [
        result.rect.x + 6.0 + glyph.x + glyph.advance * 0.4,
        result.rect.y + 3.0 + glyph.y + 4.0,
    ];
    let hit = metrics.hit_test(point).expect("widget has text geometry");
    assert!(hit <= buffer.len_bytes());
    let mut placed = buffer.clone();
    assert!(metrics.place_caret(&mut placed, point, false));
    assert_eq!(placed.caret(), hit);

    let mut selected_buffer = buffer.clone();
    selected_buffer.set_selection(0, "AV🙂".len());
    let selection = metrics.selection_rects(&selected_buffer);
    assert!(!selection.is_empty());
    let caret = metrics
        .caret_rect(&selected_buffer)
        .expect("caret geometry exists");
    assert!(caret.x.is_finite() && caret.y.is_finite());
}

#[test]
fn wrapped_text_area_navigation_follows_visual_lines_and_retains_horizontal_position() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin, StylePlugin, bbui_text::TextPlugin));
    app.add_systems(
        Update,
        bevy_text::load_font_assets_into_font_collection
            .in_set(UiSystems::TextMeasure)
            .before(measure_text_intrinsics),
    );
    let font_handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 120.0, 220.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(120.0),
                    height: UiLength::Px(220.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let text = "the quick brown fox jumps over the lazy dog again";
    let mut text_layout = UiTextLayout::text_area("", true);
    text_layout.font = Some(TextFont::from_font_size(16.0).with_font(font_handle));
    let field = app
        .world_mut()
        .spawn((
            UiNode,
            UiMultiline,
            UiLayoutNode::leaf(UiLength::Px(104.0), UiLength::Px(180.0)),
            TextBuffer::new(text),
            text_layout,
        ))
        .id();
    set_parent(app.world_mut(), field, root).expect("text area attaches to layout root");
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(field);
    app.update();

    let shaped = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .layout()
        .expect("text area is shaped by Bevy");
    assert!(shaped.line_count() >= 3, "test text should soft wrap");
    let start_glyph = shaped
        .glyphs()
        .iter()
        .find(|glyph| glyph.line == 0 && glyph.x >= 16.0)
        .expect("first line has a glyph at the test column");
    let start_byte = start_glyph.byte_range.start;
    app.world_mut()
        .get_mut::<TextBuffer>(field)
        .unwrap()
        .collapse_to(start_byte);
    bbui_text::update_text_layouts(app.world_mut());

    let press = |app: &mut App, key| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: field,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        bbui_text::process_text_events(app.world_mut());
        bbui_text::update_text_layouts(app.world_mut());
    };

    let first_layout = app.world().get::<UiTextLayout>(field).unwrap();
    let first_buffer = app.world().get::<TextBuffer>(field).unwrap();
    let first_caret = first_layout
        .layout()
        .unwrap()
        .caret_rect(first_buffer.caret());
    let preferred_x = first_caret.x;

    press(&mut app, UiKeyCode::ArrowDown);
    let second_buffer = app.world().get::<TextBuffer>(field).unwrap();
    let second_caret = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .layout()
        .unwrap()
        .caret_rect(second_buffer.caret());
    assert!(second_caret.y > first_caret.y);
    assert!((second_caret.x - preferred_x).abs() < 12.0);

    press(&mut app, UiKeyCode::ArrowDown);
    let third_buffer = app.world().get::<TextBuffer>(field).unwrap();
    let third_caret = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .layout()
        .unwrap()
        .caret_rect(third_buffer.caret());
    assert!(third_caret.y > second_caret.y);
    assert!((third_caret.x - preferred_x).abs() < 12.0);

    app.world_mut()
        .get_mut::<TextBuffer>(field)
        .unwrap()
        .collapse_to(start_byte);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: field,
        key: UiKeyCode::ArrowDown,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    }];
    bbui_text::process_text_events(app.world_mut());
    let selection_buffer = app.world().get::<TextBuffer>(field).unwrap();
    assert_eq!(selection_buffer.anchor(), start_byte);
    assert_ne!(selection_buffer.caret(), start_byte);

    press(&mut app, UiKeyCode::Home);
    let line_start_buffer = app.world().get::<TextBuffer>(field).unwrap();
    let line_start = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .caret_rect(line_start_buffer)
        .unwrap();
    let line_start_byte = line_start_buffer.caret();
    press(&mut app, UiKeyCode::End);
    let line_end_buffer = app.world().get::<TextBuffer>(field).unwrap();
    let line_end = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .caret_rect(line_end_buffer)
        .unwrap();
    let line_end_byte = line_end_buffer.caret();
    assert!(
        (line_end.y - line_start.y).abs() < 0.1,
        "visual Home/End should stay on one line: start={line_start:?} byte={} end={line_end:?} byte={}",
        line_start_byte,
        line_end_byte
    );
    assert!(line_end.x > line_start.x);

    press(&mut app, UiKeyCode::ArrowRight);
    let wrap_downstream = app.world().get::<TextBuffer>(field).unwrap();
    let wrap_downstream_caret = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .caret_rect(wrap_downstream)
        .unwrap();
    assert_eq!(wrap_downstream.caret(), line_end_byte);
    assert_eq!(
        wrap_downstream.caret_affinity(),
        bbui_text::CaretAffinity::Downstream
    );
    assert!(wrap_downstream_caret.y > line_end.y);

    press(&mut app, UiKeyCode::ArrowRight);
    let after_wrap_character = app.world().get::<TextBuffer>(field).unwrap().caret();
    assert!(after_wrap_character > line_end_byte);
    press(&mut app, UiKeyCode::ArrowLeft);
    let wrap_return = app.world().get::<TextBuffer>(field).unwrap();
    assert_eq!(wrap_return.caret(), line_end_byte);
    assert_eq!(
        wrap_return.caret_affinity(),
        bbui_text::CaretAffinity::Downstream
    );
    press(&mut app, UiKeyCode::ArrowLeft);
    let wrap_upstream = app.world().get::<TextBuffer>(field).unwrap();
    assert_eq!(wrap_upstream.caret(), line_end_byte);
    assert_eq!(
        wrap_upstream.caret_affinity(),
        bbui_text::CaretAffinity::Upstream
    );
    let wrap_upstream_caret = app
        .world()
        .get::<UiTextLayout>(field)
        .unwrap()
        .caret_rect(wrap_upstream)
        .unwrap();
    assert!((wrap_upstream_caret.y - line_end.y).abs() < 0.1);
}

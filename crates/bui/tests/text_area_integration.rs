use bevy::{
    asset::{AssetPlugin, Assets},
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    prelude::{App, ImagePlugin, MinimalPlugins, Update, Window},
    text::{DEFAULT_FONT_DATA, Font, TextFont, load_font_assets_into_font_collection},
    window::{Ime, WindowPlugin},
};
use bui::{
    BuiPlugin,
    widgets::{WidgetNode, WidgetTree},
};
use bui_core::{
    UiChildren, UiFocus, UiHitBox, UiInputEvent, UiInputQueue, UiKeyCode, UiModifiers,
    UiRoutedEvent, UiRoutedEvents,
};
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bui_render::{DisplayList, Primitive};
use bui_text::{TextBuffer, UiTextLayout};
use bui_widgets::{ClipboardRequestQueue, TextArea, WidgetEvent, WidgetEventQueue};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BuiPlugin);
    app.add_systems(Update, load_font_assets_into_font_collection);
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec()));
    // Populate Bevy's font collection before the first text widget is shaped.
    app.update();
    app
}

fn push_key(app: &mut App, window: bevy::prelude::Entity, key: UiKeyCode, modifiers: UiModifiers) {
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
fn text_area_wraps_navigates_composes_uses_clipboard_and_scrolls_end_to_end() {
    let mut app = headless_app();
    let mut native_window = Window::default();
    native_window.resolution.set(360.0, 220.0);
    let window = app.world_mut().spawn(native_window).id();
    let value = "The first long paragraph wraps into multiple visual lines so the editor can test shaped caret movement and selection.\n\
        The second paragraph stays in the same buffer and demonstrates explicit newlines.\n\
        A third line keeps enough content below the viewport to exercise wheel scrolling and caret into view.\n\
        Unicode such as café, Ελληνικά, and naïve shares the same shaping path as the Bevy renderer.";
    let root = WidgetTree::new(WidgetNode::column([WidgetNode::text_area("Memo", value)]))
        .spawn(app.world_mut(), None)
        .unwrap();
    let mut root_node = *app.world().get::<UiLayoutNode>(root).unwrap();
    root_node.padding = bui_layout::UiEdges::all(8.0);
    root_node.gap = 4.0;
    root_node.clip_contents = true;
    app.world_mut().entity_mut(root).insert((
        root_node,
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 360.0, 220.0), 1.0),
    ));
    let area = app.world().get::<UiChildren>(root).unwrap().0[0];
    {
        let mut node = app.world_mut().get_mut::<UiLayoutNode>(area).unwrap();
        node.size.width = UiLength::Px(210.0);
        node.size.height = UiLength::Px(62.0);
    }
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(DEFAULT_FONT_DATA.to_vec()));
    app.world_mut().get_mut::<UiTextLayout>(area).unwrap().font =
        Some(TextFont::from_font_size(14.0).with_font(font));
    app.update();

    let area_result = *app.world().get::<UiLayoutResult>(area).unwrap();
    let hit_box = *app.world().get::<UiHitBox>(area).unwrap();
    assert_eq!(hit_box.rect, area_result.rect);
    assert_eq!(hit_box.clip, area_result.clip);
    assert!(hit_box.contains([
        area_result.rect.x + area_result.rect.width / 2.0,
        area_result.rect.y + area_result.rect.height / 2.0,
    ]));

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(area);
    app.update();
    let initial_lines = app
        .world()
        .get::<UiTextLayout>(area)
        .unwrap()
        .layout()
        .expect("TextArea should use Bevy's text shaper")
        .line_count();
    assert!(
        initial_lines >= 8,
        "expected wrapped content, got {initial_lines} lines"
    );
    let initial_buffer = app
        .world()
        .get::<TextBuffer>(area)
        .unwrap()
        .as_str()
        .to_owned();
    let buffer = app.world().get::<TextBuffer>(area).unwrap();
    let caret = app
        .world()
        .get::<UiTextLayout>(area)
        .unwrap()
        .caret_rect(buffer)
        .unwrap();
    assert!(
        caret.y >= area_result.rect.y,
        "focused caret should be scrolled into view"
    );
    assert!(
        caret.y + caret.height <= area_result.rect.y + area_result.rect.height + 0.1,
        "focused caret should remain inside the TextArea viewport: {caret:?}"
    );

    let before_up = app.world().get::<TextBuffer>(area).unwrap().caret();
    let before_up_rect = app
        .world()
        .get::<UiTextLayout>(area)
        .unwrap()
        .caret_rect(app.world().get::<TextBuffer>(area).unwrap())
        .unwrap();
    push_key(&mut app, window, UiKeyCode::ArrowUp, UiModifiers::default());
    let after_up = app.world().get::<TextBuffer>(area).unwrap().caret();
    let after_up_rect = app
        .world()
        .get::<UiTextLayout>(area)
        .unwrap()
        .caret_rect(app.world().get::<TextBuffer>(area).unwrap())
        .unwrap();
    assert!(
        after_up < before_up,
        "Up should move to an earlier shaped position"
    );
    assert!(
        after_up_rect.y < before_up_rect.y,
        "Up should move to the previous visual line"
    );
    push_key(&mut app, window, UiKeyCode::Enter, UiModifiers::default());
    assert!(
        app.world()
            .get::<TextBuffer>(area)
            .unwrap()
            .as_str()
            .contains("\n")
    );
    let before_composition = app
        .world()
        .get::<TextBuffer>(area)
        .unwrap()
        .as_str()
        .to_owned();
    assert_eq!(
        app.world().get::<TextArea>(area).unwrap().field.text,
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "the legacy widget view must sync in the same frame"
    );

    push_key(
        &mut app,
        window,
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    );
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "é".into(),
        cursor: Some((0, 2)),
    });
    app.update();
    let text_layout = app.world().get::<UiTextLayout>(area).unwrap();
    assert!(text_layout.display_text().unwrap().contains("é"));
    let preedit_rects = text_layout.preedit_rects();
    assert!(
        !preedit_rects.is_empty(),
        "IME preedit should have shaped underline geometry"
    );
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| {
                let Primitive::Line { from, to, .. } = &item.primitive else {
                    return false;
                };
                preedit_rects.iter().any(|rect| {
                    let underline_y = rect.y + rect.height.max(1.0) - 1.0;
                    (from[0] - rect.x).abs() < 0.01
                        && (from[1] - underline_y).abs() < 0.01
                        && (to[0] - (rect.x + rect.width)).abs() < 0.01
                        && (to[1] - underline_y).abs() < 0.01
                })
            }),
        "IME underline should be painted from the shared shaped geometry"
    );
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "é".into(),
    });
    app.update();
    assert_eq!(app.world().get::<TextBuffer>(area).unwrap().as_str(), "é");
    push_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    );
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        before_composition
    );
    assert_eq!(
        app.world().get::<TextArea>(area).unwrap().field.text,
        before_composition
    );

    // Ctrl+V asks the host for a one-shot read. Injecting its response through
    // UiInputQueue exercises the same focused Text event path as the platform bridge.
    let end = app.world().get::<TextBuffer>(area).unwrap().len_bytes();
    app.world_mut()
        .get_mut::<TextBuffer>(area)
        .unwrap()
        .collapse_to(end);
    app.update();
    push_key(
        &mut app,
        window,
        UiKeyCode::V,
        UiModifiers {
            control: true,
            ..Default::default()
        },
    );
    assert!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| {
                matches!(event, WidgetEvent::ClipboardRead { target } if *target == area)
            })
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<ClipboardRequestQueue>()
            .drain()
            .count(),
        0,
        "the BUI host bridge consumes the one-shot OS clipboard request"
    );
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Text {
            window,
            text: "host paste\nsecond line".into(),
        });
    app.update();
    let expected_paste = format!("{before_composition}host paste\nsecond line");
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        expected_paste
    );
    assert_eq!(
        app.world().get::<TextArea>(area).unwrap().field.text,
        expected_paste
    );

    let center = [
        area_result.rect.x + area_result.rect.width / 2.0,
        area_result.rect.y + area_result.rect.height / 2.0,
    ];
    app.world_mut()
        .get_mut::<TextBuffer>(area)
        .unwrap()
        .collapse_to(0);
    app.world_mut()
        .get_mut::<TextArea>(area)
        .unwrap()
        .scroll_line = 0;
    app.world_mut()
        .get_mut::<UiTextLayout>(area)
        .unwrap()
        .scroll_offset[1] = 0.0;
    app.update();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window,
            position: center,
            delta: [0.0, -2.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        });
    app.update();
    assert!(
        app.world()
            .resource::<UiRoutedEvents>()
            .events
            .iter()
            .any(|event| {
                matches!(event, UiRoutedEvent::Wheel { target, .. } if *target == area)
            })
    );
    assert!(app.world().get::<TextArea>(area).unwrap().scroll_line > 0);
    let scroll = app.world().get::<TextArea>(area).unwrap().scroll_line;
    let layout = app.world().get::<UiTextLayout>(area).unwrap();
    assert!(layout.scroll_offset[1] > 0.0);
    let theme = app.world().resource::<bui_style::Theme>();
    let line_height = theme.font_size * theme.line_height;
    assert!((layout.scroll_offset[1] - line_height * scroll as f32).abs() < 0.01);

    assert!(
        initial_buffer.contains("café") && initial_buffer.contains("Ελληνικά"),
        "the integration fixture exercises Unicode content"
    );
}

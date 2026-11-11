use bbui_core::{
    CorePlugin, UiFocus, UiKeyCode, UiModifiers, UiNode, UiRoutedEvent, UiRoutedEvents, UiSystems,
};
use bbui_layout::{UiLayoutNode, UiLayoutResult, UiLength};
use bbui_text::{
    TextBuffer, TextPlugin, UiMultiline, UiTextLayout, measure_text_intrinsics,
    process_text_events, update_text_layouts,
};
use bevy_app::{App, Update};
use bevy_asset::Assets;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_text::{Font, TextFont};

fn main() -> Result<(), &'static str> {
    let mut app = App::new();
    app.add_plugins((CorePlugin, TextPlugin));
    app.add_systems(
        Update,
        bevy_text::load_font_assets_into_font_collection
            .in_set(UiSystems::TextMeasure)
            .before(measure_text_intrinsics),
    );

    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    let window = app.world_mut().spawn_empty().id();
    let value = "Use the arrow keys to move through wrapped visual lines.\nShift extends a selection; Home and End follow the current wrapped line.";
    let mut text_layout = UiTextLayout::text_area("", true);
    text_layout.font = Some(TextFont::from_font_size(16.0).with_font(font));
    text_layout.inset = [4.0, 3.0];
    let editor = app
        .world_mut()
        .spawn((
            UiNode,
            UiMultiline,
            UiLayoutNode::leaf(UiLength::Px(220.0), UiLength::Px(120.0)),
            TextBuffer::new(value),
            text_layout,
            UiLayoutResult {
                window,
                rect: bbui_core::UiRect::new(12.0, 16.0, 220.0, 120.0),
                clip: None,
                z: 0,
                content_size: [220.0, 120.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(editor);
    app.update();

    let start = app
        .world()
        .get::<TextBuffer>(editor)
        .ok_or("text buffer is missing")?
        .as_str()
        .find("wrapped")
        .ok_or("example text lacks the navigation target")?;
    app.world_mut()
        .get_mut::<TextBuffer>(editor)
        .ok_or("text buffer is missing")?
        .collapse_to(start);
    update_text_layouts(app.world_mut());
    print_editor_state(&app, editor, "before Down")?;

    route_key(
        &mut app,
        window,
        editor,
        UiKeyCode::ArrowDown,
        UiModifiers::default(),
    );
    print_editor_state(&app, editor, "after visual Down")?;
    route_key(
        &mut app,
        window,
        editor,
        UiKeyCode::ArrowDown,
        UiModifiers {
            shift: true,
            ..UiModifiers::default()
        },
    );
    let selection = app
        .world()
        .get::<TextBuffer>(editor)
        .ok_or("text buffer is missing")?
        .selection();
    println!(
        "shifted selection bytes: {}..{}",
        selection.start, selection.end
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::ImePreedit {
        window,
        target: editor,
        text: "draft".into(),
        cursor_range: Some([5, 5]),
    }];
    process_text_events(app.world_mut());
    update_text_layouts(app.world_mut());
    let layout = app
        .world()
        .get::<UiTextLayout>(editor)
        .ok_or("text layout is missing")?;
    println!(
        "IME preedit display={:?}, underline rects={}",
        layout.display_text().unwrap_or_default(),
        layout.preedit_rects().len()
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::ImeCommit {
        window,
        target: editor,
        text: "notes".into(),
    }];
    process_text_events(app.world_mut());
    update_text_layouts(app.world_mut());
    route_key(
        &mut app,
        window,
        editor,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    println!(
        "after local undo: {:?}",
        app.world()
            .get::<TextBuffer>(editor)
            .ok_or("text buffer is missing")?
            .as_str()
    );
    Ok(())
}

fn route_key(
    app: &mut App,
    window: bevy_ecs::entity::Entity,
    target: bevy_ecs::entity::Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
) {
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers,
    }];
    process_text_events(app.world_mut());
    update_text_layouts(app.world_mut());
}

fn print_editor_state(
    app: &App,
    editor: bevy_ecs::entity::Entity,
    label: &str,
) -> Result<(), &'static str> {
    let buffer = app
        .world()
        .get::<TextBuffer>(editor)
        .ok_or("text buffer is missing")?;
    let layout = app
        .world()
        .get::<UiTextLayout>(editor)
        .ok_or("text layout is missing")?;
    let caret = layout
        .caret_rect(buffer)
        .ok_or("caret rectangle is missing")?;
    println!(
        "{label}: caret={} affinity={:?} at ({:.1}, {:.1}), visual lines={}",
        buffer.caret(),
        buffer.caret_affinity(),
        caret.x,
        caret.y,
        layout
            .layout()
            .ok_or("shaped layout is missing")?
            .line_count()
    );
    Ok(())
}

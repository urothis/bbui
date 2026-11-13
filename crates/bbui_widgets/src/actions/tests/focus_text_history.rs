use super::*;

#[test]
pub(super) fn focus_departure_commits_composite_edits_and_text_ime_keeps_local_undo() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bbui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let input_hit_box = |tab_index| {
        (
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(tab_index),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(tab_index as f32 * 100.0, 0.0, 90.0, 28.0),
                clip: None,
                z_index: tab_index,
            },
        )
    };
    let mut number = NumericField::float(1.0).unwrap();
    number.edit_insert("5");
    let number = app.world_mut().spawn((input_hit_box(0), number)).id();
    let editor = HsvColorEditor::from_rgba(Rgba(1.0, 0.0, 0.0, 1.0)).unwrap();
    let hsv = app
        .world_mut()
        .spawn((
            input_hit_box(1),
            editor,
            HsvColorEditorState {
                active_field: HsvColorField::Hue,
                editing: true,
                edit_buffer: "180".into(),
            },
        ))
        .id();
    let text = WidgetTree::new(WidgetNode::text_field("Name", "draft"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(text).insert((
        UiTabIndex(2),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(200.0, 0.0, 90.0, 28.0),
            clip: None,
            z_index: 2,
        },
    ));
    let area = WidgetTree::new(WidgetNode::text_area("Notes", "draft"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(area).insert((
        UiTabIndex(3),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(300.0, 0.0, 90.0, 28.0),
            clip: None,
            z_index: 3,
        },
    ));
    let after = app.world_mut().spawn(input_hit_box(4)).id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(number);

    send_tab(&mut app, window, false);
    assert_eq!(app.world().get::<NumericField>(number).unwrap().value, 5.0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(hsv)
    );
    send_tab(&mut app, window, false);
    let editor = app.world().get::<HsvColorEditor>(hsv).unwrap();
    assert!((editor.hue_degrees - 180.0).abs() < 0.01);
    let edit_state = app.world().get::<HsvColorEditorState>(hsv).unwrap();
    assert!(!edit_state.editing);
    assert!(edit_state.edit_buffer.is_empty());
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(text)
    );

    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::Text {
            window,
            text: "x".into(),
        },
        UiInputEvent::ImePreedit {
            window,
            text: "に".into(),
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
        app.world().get::<TextBuffer>(text).unwrap().as_str(),
        "draftx"
    );
    assert!(
        app.world()
            .get::<TextBuffer>(text)
            .unwrap()
            .composition()
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(area)
    );
    let app_undo_depth = app.world().resource::<bbui_core::UiHistory>().undo_len();

    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::Text {
            window,
            text: "y".into(),
        },
        UiInputEvent::ImePreedit {
            window,
            text: "かな".into(),
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
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(after)
    );
    let buffer = app.world().get::<TextBuffer>(area).unwrap();
    assert_eq!(buffer.as_str(), "drafty");
    assert!(
        buffer.composition().is_none(),
        "Tab cancels outgoing TextArea IME preedit"
    );

    send_tab(&mut app, window, true);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(area)
    );
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        });
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draft"
    );
    assert_eq!(
        app.world().resource::<bbui_core::UiHistory>().undo_len(),
        app_undo_depth,
        "the blurred TextArea edit is committed and undone through shared history"
    );

    send_tab(&mut app, window, true);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(text)
    );
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        });
    app.update();
    assert_eq!(
        app.world().get::<TextBuffer>(text).unwrap().as_str(),
        "draft"
    );
    assert_eq!(
        app.world().resource::<bbui_core::UiHistory>().undo_len(),
        app_undo_depth - 1,
        "the previously blurred TextField edit is the next shared undo step"
    );
}

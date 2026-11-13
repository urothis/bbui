use super::*;

pub(super) fn send_tab(app: &mut App, window: Entity, shift: bool) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                shift,
                ..UiModifiers::default()
            },
        });
    app.update();
}

pub(super) fn send_key(app: &mut App, window: Entity, key: UiKeyCode, modifiers: UiModifiers) {
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
pub(super) fn committed_text_blur_uses_global_history_after_local_undo_priority() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bbui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let text = WidgetTree::new(WidgetNode::text_field("Name", "a🧊c"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(text).insert((
        UiTabIndex(0),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
            clip: None,
            z_index: 0,
        },
    ));
    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(120.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .get_mut::<TextBuffer>(text)
        .unwrap()
        .set_selection(5, 1);
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(text);
    app.update();

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Text {
            window,
            text: "Z".into(),
        });
    app.update();
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "aZc");
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 1);

    let control = UiModifiers {
        control: true,
        ..UiModifiers::default()
    };
    send_key(&mut app, window, UiKeyCode::Z, control);
    assert_eq!(
        app.world().get::<TextBuffer>(text).unwrap().as_str(),
        "a🧊c"
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    send_key(&mut app, window, UiKeyCode::Y, control);
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "aZc");
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);

    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(other)
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 0);
    assert!(app
            .world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::TextCommitted { target, before, after, before_selection, after_selection, .. }
                if *target == text
                    && before == "a🧊c"
                    && after == "aZc"
                    && *before_selection == TextSelection { anchor: 5, caret: 1 }
                    && *after_selection == TextSelection { anchor: 2, caret: 2 })));
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);

    send_key(&mut app, window, UiKeyCode::Z, control);
    let buffer = app.world().get::<TextBuffer>(text).unwrap();
    assert_eq!(buffer.as_str(), "a🧊c");
    assert_eq!(buffer.anchor(), 5);
    assert_eq!(buffer.caret(), 1);
    assert_eq!(buffer.undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(text)
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().redo_len(), 1);

    send_key(&mut app, window, UiKeyCode::Y, control);
    let buffer = app.world().get::<TextBuffer>(text).unwrap();
    assert_eq!(buffer.as_str(), "aZc");
    assert_eq!(buffer.anchor(), 2);
    assert_eq!(buffer.caret(), 2);
    assert_eq!(
        buffer.undo_len(),
        0,
        "global redo must not leave a duplicate local edit"
    );

    send_tab(&mut app, window, false);
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);
    assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(|event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == text))
                .count(),
            1,
            "redo rebases the focused draft instead of recommitting the same text"
        );
}

#[test]
pub(super) fn locally_cancelled_text_is_not_committed_and_enter_commits_once() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bbui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let text = WidgetTree::new(WidgetNode::text_field("Name", "a"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(text).insert((
        UiTabIndex(0),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
            clip: None,
            z_index: 0,
        },
    ));
    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(120.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(text);
    app.update();

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Text {
            window,
            text: "b".into(),
        });
    app.update();
    send_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "a");
    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(other)
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == text)
    ));

    send_tab(&mut app, window, true);
    send_key(
        &mut app,
        window,
        UiKeyCode::Y,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "ab");
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    send_key(&mut app, window, UiKeyCode::Enter, UiModifiers::default());
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "ab");
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 0);
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);
    assert_eq!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .filter(
                |event| matches!(event, WidgetEvent::TextCommitted { target, before, after, .. }
                    if *target == text && before == "a" && after == "ab")
            )
            .count(),
        1
    );

    send_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "a");
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(text)
    );
}

#[test]
pub(super) fn textarea_ime_preedit_cancels_and_ime_commit_enters_global_history_once() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bbui_text::TextPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let area = WidgetTree::new(WidgetNode::text_area("Notes", "draft"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(area).insert((
        UiTabIndex(0),
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 0.0, 160.0, 60.0),
            clip: None,
            z_index: 0,
        },
    ));
    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(1),
            UiEnabled(true),
            UiVisible(true),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(180.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(area);
    app.update();

    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::ImePreedit {
            window,
            text: "仮".into(),
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
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draft"
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == area)
    ));
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(other)
    );

    send_tab(&mut app, window, true);
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::ImePreedit {
            window,
            text: "かな".into(),
            cursor_range: None,
        },
        UiInputEvent::ImeCommit {
            window,
            text: "かな".into(),
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
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draftかな"
    );
    assert_eq!(app.world().get::<TextBuffer>(area).unwrap().undo_len(), 0);
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);

    send_key(
        &mut app,
        window,
        UiKeyCode::Z,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(
        app.world().get::<TextBuffer>(area).unwrap().as_str(),
        "draft"
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(area)
    );
}

#[test]
pub(super) fn tab_leaves_open_popovers_and_dropdowns_without_restoring_trigger_focus() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let spawn_focusable = |world: &mut World, tab_index: i32, x: f32| {
        world
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(tab_index),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(x, 0.0, 80.0, 24.0),
                    clip: None,
                    z_index: tab_index,
                },
            ))
            .id()
    };
    let before = spawn_focusable(app.world_mut(), 0, 0.0);
    let mut popover = MenuControl::new(
        "Tools",
        crate::MenuTree::new(vec![crate::MenuEntry::command("apply", "Apply")]),
        MenuDisplay::Popover,
    );
    popover.open(Some("tools-popover".into()));
    popover.popup.open = true;
    let menu = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(1),
            UiKey("tools-popover".into()),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(90.0, 0.0, 80.0, 24.0),
                clip: None,
                z_index: 1,
            },
            UiLayoutResult {
                window,
                rect: UiRect::new(90.0, 0.0, 80.0, 24.0),
                clip: None,
                z: 1,
                content_size: [80.0, 24.0],
                dpi_scale: 1.0,
            },
            popover,
        ))
        .id();
    let dropdown = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiFocusable,
            UiTabIndex(2),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(180.0, 0.0, 80.0, 24.0),
                clip: None,
                z_index: 2,
            },
            Dropdown::new(["A", "B"], Some(0)),
        ))
        .id();
    let after = spawn_focusable(app.world_mut(), 3, 270.0);
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(menu);

    app.update();
    assert!(app.world().get::<MenuControl>(menu).unwrap().session.open);
    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(dropdown)
    );
    assert!(!app.world().get::<MenuControl>(menu).unwrap().session.open);
    assert!(!app.world().get::<MenuControl>(menu).unwrap().popup.open);

    app.world_mut()
        .get_mut::<Dropdown>(dropdown)
        .unwrap()
        .open();
    send_tab(&mut app, window, false);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(after)
    );
    assert!(!app.world().get::<Dropdown>(dropdown).unwrap().open);

    {
        let mut menu = app.world_mut().get_mut::<MenuControl>(menu).unwrap();
        menu.open(Some("tools-popover".into()));
        menu.popup.open = true;
    }
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(menu);
    app.update();
    send_tab(&mut app, window, true);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(before)
    );
    assert!(!app.world().get::<MenuControl>(menu).unwrap().session.open);
}

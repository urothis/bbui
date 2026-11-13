use super::*;

#[test]
pub(super) fn widget_tree_menu_opens_searches_routes_items_and_emits_commands() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let tree = WidgetTree::new(WidgetNode::menu(
        "File",
        [
            crate::MenuEntry::command("open", "Open"),
            crate::MenuEntry::submenu(
                "more",
                "More",
                vec![crate::MenuEntry::command("advanced", "Advanced")],
            ),
        ],
    ));
    let owner = tree.spawn(app.world_mut(), None).unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(12.0, 18.0, 90.0, 24.0),
        clip: None,
        z: 4,
        content_size: [90.0, 24.0],
        dpi_scale: 1.0,
    });
    let items = app.world().get::<MenuControl>(owner).unwrap().items.clone();
    let more = items
        .iter()
        .copied()
        .find(|entity| app.world().get::<MenuItemControl>(*entity).unwrap().id == "more")
        .unwrap();
    let advanced = items
        .iter()
        .copied()
        .find(|entity| app.world().get::<MenuItemControl>(*entity).unwrap().id == "advanced")
        .unwrap();

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: owner,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [20.0, 20.0],
    }];
    app.update();
    assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert!(app.world().get::<UiVisible>(more).unwrap().0);
    assert!(!app.world().get::<UiVisible>(advanced).unwrap().0);
    assert!(app.world().get::<UiHitBox>(more).is_some());

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerEnter {
        window,
        target: more,
    }];
    app.update();
    assert!(app.world().get::<UiVisible>(advanced).unwrap().0);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "adv".into(),
    }];
    app.update();
    assert!(app.world().get::<UiVisible>(advanced).unwrap().0);
    assert!(!app.world().get::<UiVisible>(more).unwrap().0);
    let hit = *app.world().get::<UiHitBox>(advanced).unwrap();
    let position = [hit.rect.x + 2.0, hit.rect.y + 2.0];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerDown {
            window,
            target: advanced,
            button: UiPointerButton::Primary,
            position,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: advanced,
            button: UiPointerButton::Primary,
            position,
            cancelled: false,
        },
        UiRoutedEvent::Click {
            window,
            target: advanced,
            button: UiPointerButton::Primary,
            click_count: 1,
            position,
        },
    ];
    app.update();
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::MenuCommand { command_id, .. } if command_id == "advanced")));
}

#[test]
pub(super) fn menu_keyboard_opens_submenus_activates_commands_and_cancels() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::menu(
        "File",
        [
            crate::MenuEntry::submenu(
                "more",
                "More",
                vec![crate::MenuEntry::command("advanced", "Advanced")],
            ),
            crate::MenuEntry::command("open", "Open"),
        ],
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
        clip: None,
        z: 1,
        content_size: [100.0, 28.0],
        dpi_scale: 1.0,
    });
    let send_key = |app: &mut App, key| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
    };

    send_key(&mut app, UiKeyCode::ArrowDown);
    assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
    send_key(&mut app, UiKeyCode::ArrowDown);
    assert_eq!(
        app.world().get::<MenuControl>(owner).unwrap().session.path,
        [0]
    );
    send_key(&mut app, UiKeyCode::ArrowRight);
    assert_eq!(
        app.world().get::<MenuControl>(owner).unwrap().session.path,
        [0, 0]
    );
    send_key(&mut app, UiKeyCode::Escape);
    assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert_eq!(
        app.world().get::<MenuControl>(owner).unwrap().session.path,
        [0],
        "Escape closes the submenu before closing the popup"
    );
    send_key(&mut app, UiKeyCode::ArrowRight);
    send_key(&mut app, UiKeyCode::Enter);
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::MenuCommand { target, command_id } if *target == owner && command_id == "advanced")));

    send_key(&mut app, UiKeyCode::Space);
    assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
    send_key(&mut app, UiKeyCode::Escape);
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
}

#[test]
pub(super) fn menu_bar_arrow_keys_switch_open_menu_and_focus() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut().init_resource::<UiFocus>();
    let window = app.world_mut().spawn_empty().id();
    let root = WidgetTree::new(WidgetNode::menu_bar([
        WidgetNode::menu("File", [crate::MenuEntry::command("open", "Open")]),
        WidgetNode::menu("Edit", [crate::MenuEntry::command("copy", "Copy")]),
    ]))
    .spawn(app.world_mut(), None)
    .unwrap();
    let menus = app.world().get::<UiChildren>(root).unwrap().0.clone();
    assert_eq!(menus.len(), 2);
    for (index, menu) in menus.iter().enumerate() {
        app.world_mut().entity_mut(*menu).insert(UiLayoutResult {
            window,
            rect: UiRect::new(index as f32 * 90.0, 0.0, 90.0, 28.0),
            clip: None,
            z: 1,
            content_size: [90.0, 28.0],
            dpi_scale: 1.0,
        });
    }
    let send_key = |app: &mut App, target, key| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
    };

    send_key(&mut app, menus[0], UiKeyCode::ArrowDown);
    assert!(
        app.world()
            .get::<MenuControl>(menus[0])
            .unwrap()
            .session
            .open
    );
    send_key(&mut app, menus[0], UiKeyCode::ArrowRight);
    assert!(
        !app.world()
            .get::<MenuControl>(menus[0])
            .unwrap()
            .session
            .open
    );
    assert!(
        app.world()
            .get::<MenuControl>(menus[1])
            .unwrap()
            .session
            .open
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(menus[1])
    );
    send_key(&mut app, menus[1], UiKeyCode::ArrowLeft);
    assert!(
        app.world()
            .get::<MenuControl>(menus[0])
            .unwrap()
            .session
            .open
    );
    assert!(
        !app.world()
            .get::<MenuControl>(menus[1])
            .unwrap()
            .session
            .open
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(menus[0])
    );
}

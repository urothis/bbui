use super::*;

#[test]
pub(super) fn menu_drag_open_picks_under_release_and_restores_trigger_focus() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(
        WidgetNode::menu(
            "File",
            [
                crate::MenuEntry::command("open", "Open"),
                crate::MenuEntry::command("save", "Save"),
            ],
        )
        .with_key("file-menu"),
    )
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(20.0, 20.0, 100.0, 28.0),
        clip: None,
        z: 2,
        content_size: [100.0, 28.0],
        dpi_scale: 1.0,
    });
    app.world_mut().entity_mut(owner).insert(UiHitBox {
        window: Some(window),
        rect: UiRect::new(20.0, 20.0, 100.0, 28.0),
        clip: None,
        z_index: 2,
    });
    app.world_mut().insert_resource(UiFocus::default());
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(owner);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerDown {
        window,
        target: owner,
        button: UiPointerButton::Primary,
        position: [28.0, 28.0],
    }];
    app.update();
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerMoved {
        window,
        target: owner,
        position: [34.0, 48.0],
        delta: [6.0, 20.0],
        dragging: true,
    }];
    app.update();
    let menu = app.world().get::<MenuControl>(owner).unwrap();
    assert!(menu.session.open);
    assert!(menu.opened_by_pointer);
    let save = menu
        .items
        .iter()
        .copied()
        .find(|entity| {
            app.world()
                .get::<MenuItemControl>(*entity)
                .is_some_and(|item| item.id == "save")
        })
        .unwrap();
    let hit = *app.world().get::<UiHitBox>(save).unwrap();
    let position = [
        hit.rect.x + hit.rect.width * 0.5,
        hit.rect.y + hit.rect.height * 0.5,
    ];
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::PointerMoved {
            window,
            target: owner,
            position,
            delta: [position[0] - 34.0, position[1] - 48.0],
            dragging: true,
        },
        UiRoutedEvent::PointerUp {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            position,
            cancelled: false,
        },
    ];
    app.update();

    let menu = app.world().get::<MenuControl>(owner).unwrap();
    assert!(!menu.session.open);
    assert!(!menu.opened_by_pointer);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .and_then(|state| state.focused),
        Some(owner)
    );
    assert!(
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(
                |event| matches!(event, WidgetEvent::MenuCommand { target, command_id }
                if *target == owner && command_id == "save")
            )
    );
}

#[test]
pub(super) fn context_menu_anchors_at_pointer_flips_and_outside_click_closes_without_stealing_focus()
 {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().spawn(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 320.0, 220.0),
        1.0,
    ));
    let owner = WidgetTree::new(
        WidgetNode::context_menu(
            "Object",
            [
                crate::MenuEntry::command("duplicate", "Duplicate"),
                crate::MenuEntry::submenu(
                    "convert",
                    "Convert",
                    vec![crate::MenuEntry::command("mesh", "Mesh")],
                ),
            ],
        )
        .with_key("object-context"),
    )
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(40.0, 30.0, 100.0, 28.0),
        clip: None,
        z: 2,
        content_size: [100.0, 28.0],
        dpi_scale: 1.0,
    });
    app.world_mut().entity_mut(owner).insert(UiHitBox {
        window: Some(window),
        rect: UiRect::new(40.0, 30.0, 100.0, 28.0),
        clip: None,
        z_index: 2,
    });
    app.world_mut().insert_resource(UiFocus::default());

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerDown {
        window,
        target: owner,
        button: UiPointerButton::Secondary,
        position: [310.0, 210.0],
    }];
    app.update();
    let (popup, items) = {
        let menu = app.world().get::<MenuControl>(owner).unwrap();
        (menu.popup, menu.items.clone())
    };
    assert_eq!(popup.anchor, Rect::new(310.0, 210.0, 1.0, 1.0));
    assert_eq!(popup.side, crate::menus::PopupSide::Above);
    assert!(popup.bounds.x >= 0.0);
    assert!(popup.bounds.y >= 0.0);
    assert!(popup.bounds.x + popup.bounds.width <= 320.0);
    assert!(popup.bounds.y + popup.bounds.height <= 220.0);
    let convert = items
        .iter()
        .copied()
        .find(|entity| {
            app.world()
                .get::<MenuItemControl>(*entity)
                .is_some_and(|item| item.id == "convert")
        })
        .unwrap();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerEnter {
        window,
        target: convert,
    }];
    app.update();
    let mesh = items
        .iter()
        .copied()
        .find(|entity| {
            app.world()
                .get::<MenuItemControl>(*entity)
                .is_some_and(|item| item.id == "mesh")
        })
        .unwrap();
    let mesh_rect = app.world().get::<UiLayoutResult>(mesh).unwrap().rect;
    assert!(mesh_rect.x >= 0.0 && mesh_rect.x + mesh_rect.width <= 320.0);

    let outside = app
        .world_mut()
        .spawn((UiNode, UiFocusable, UiKey("outside".into())))
        .id();
    app.world_mut().entity_mut(outside).insert(UiLayoutResult {
        window,
        rect: UiRect::new(8.0, 8.0, 20.0, 20.0),
        clip: None,
        z: 20,
        content_size: [20.0, 20.0],
        dpi_scale: 1.0,
    });
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(outside);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: outside,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [10.0, 10.0],
    }];
    app.update();
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .and_then(|state| state.focused),
        Some(outside)
    );
}

#[test]
pub(super) fn pie_menu_pointer_dead_zone_keyboard_and_cancel_are_routed() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut()
        .resource_mut::<Theme>()
        .spacing
        .menu
        .pie_inner_inset = 23.0;
    app.world_mut()
        .resource_mut::<Theme>()
        .spacing
        .menu
        .pie_popup_diameter = 180.0;
    app.world_mut()
        .resource_mut::<Theme>()
        .spacing
        .menu
        .pie_inner_radius = 14.0;
    app.world_mut()
        .resource_mut::<Theme>()
        .spacing
        .menu
        .pie_outer_radius = 78.0;
    app.world_mut()
        .resource_mut::<Theme>()
        .spacing
        .menu
        .pie_outer_item_radius = 66.0;
    app.world_mut()
        .resource_mut::<Theme>()
        .spacing
        .menu
        .pie_item_max_width = 110.0;
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().spawn(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 320.0, 220.0),
        1.0,
    ));
    let owner = WidgetTree::new(
        WidgetNode::pie_menu(
            "Add",
            [
                crate::MenuEntry::command("cube", "Cube"),
                crate::MenuEntry::command("sphere", "Sphere"),
                crate::MenuEntry::command("cone", "Cone"),
                crate::MenuEntry::command("torus", "Torus"),
            ],
        )
        .with_key("add-pie"),
    )
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(100.0, 80.0, 40.0, 40.0),
        clip: None,
        z: 2,
        content_size: [40.0, 40.0],
        dpi_scale: 1.0,
    });
    app.world_mut().entity_mut(owner).insert(UiHitBox {
        window: Some(window),
        rect: UiRect::new(100.0, 80.0, 40.0, 40.0),
        clip: None,
        z_index: 2,
    });
    app.world_mut().insert_resource(UiFocus::default());
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(owner);
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: owner,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [110.0, 90.0],
    }];
    app.update();
    advance_menu_motion_by(app.world_mut(), 1.0);
    app.world_mut().increment_change_tick();
    collect_widget_paint(app.world_mut());
    let center = app
        .world()
        .get::<MenuControl>(owner)
        .unwrap()
        .pie
        .unwrap()
        .center;
    let pie_bounds = app.world().get::<MenuControl>(owner).unwrap().popup.bounds;
    let background = app.world().resource::<Theme>().background;
    assert_eq!(pie_bounds.width, 180.0);
    assert_eq!(
        app.world()
            .get::<MenuControl>(owner)
            .unwrap()
            .pie
            .unwrap()
            .radius,
        76.0
    );
    assert!(
        app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(
                item.primitive,
                Primitive::Ellipse { rect, color }
                    if color == background
                        && rect.x == pie_bounds.x + 23.0
                        && rect.y == pie_bounds.y + 23.0
                        && rect.width == pie_bounds.width - 46.0
                        && rect.height == pie_bounds.height - 46.0
            ))
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerMoved {
        window,
        target: owner,
        position: [center[0], center[1] - 32.0],
        delta: [0.0, 0.0],
        dragging: false,
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<MenuControl>(owner)
            .unwrap()
            .pie
            .unwrap()
            .selected,
        Some(0)
    );
    assert_eq!(
        app.world().get::<MenuControl>(owner).unwrap().session.path,
        [0]
    );
    let root_item = app
        .world()
        .get::<MenuControl>(owner)
        .unwrap()
        .items
        .iter()
        .copied()
        .find(|entity| {
            app.world()
                .get::<MenuItemControl>(*entity)
                .is_some_and(|item| item.id == "cube")
        })
        .unwrap();
    let root_item_rect = app.world().get::<UiLayoutResult>(root_item).unwrap().rect;
    assert!(root_item_rect.width > 40.0 && root_item_rect.width <= 110.0);
    assert!((root_item_rect.x + root_item_rect.width * 0.5 - center[0]).abs() < 0.01);
    assert!((root_item_rect.y + root_item_rect.height * 0.5 - (center[1] - 66.0)).abs() < 0.01);
    assert!(app.world().get::<UiVisible>(root_item).unwrap().0);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerMoved {
        window,
        target: owner,
        position: center,
        delta: [0.0, 0.0],
        dragging: false,
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<MenuControl>(owner)
            .unwrap()
            .pie
            .unwrap()
            .selected,
        None
    );
    assert!(
        app.world()
            .get::<MenuControl>(owner)
            .unwrap()
            .session
            .path
            .is_empty()
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::ArrowDown,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(
        app.world().get::<MenuControl>(owner).unwrap().session.path,
        [0]
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::Escape,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .and_then(|state| state.focused),
        Some(owner)
    );
}

#[test]
pub(super) fn clear_menu_history_action_resets_recency_without_affecting_other_menus() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let entries = [
        crate::MenuEntry::command("a", "Select"),
        crate::MenuEntry::command("z", "Select"),
    ];
    let first = app
        .world_mut()
        .spawn(MenuControl::new(
            "First",
            crate::MenuTree::new(entries.to_vec()),
            MenuDisplay::Dropdown,
        ))
        .id();
    let second = app
        .world_mut()
        .spawn(MenuControl::new(
            "Second",
            crate::MenuTree::new(entries.to_vec()),
            MenuDisplay::Dropdown,
        ))
        .id();
    app.world_mut()
        .get_mut::<MenuControl>(first)
        .unwrap()
        .search
        .pick("z");
    app.world_mut()
        .get_mut::<MenuControl>(second)
        .unwrap()
        .search
        .pick("z");
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(first, WidgetActionKind::ClearMenuHistory));
    app.update();
    let first_rank = app
        .world()
        .get::<MenuControl>(first)
        .unwrap()
        .search
        .rank("select", [("z", "Select"), ("a", "Select")]);
    let second_rank = app
        .world()
        .get::<MenuControl>(second)
        .unwrap()
        .search
        .rank("select", [("z", "Select"), ("a", "Select")]);
    assert_eq!(first_rank, ["a", "z"]);
    assert_eq!(second_rank, ["z", "a"]);
}

#[test]
pub(super) fn tooltip_waits_for_pointer_dwell_then_paints_and_hides_on_leave() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    app.world_mut().spawn(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 180.0, 100.0),
        1.0,
    ));
    let host = WidgetTree::new(WidgetNode::tooltip(
        "Applies the current settings",
        WidgetNode::button("Apply").with_key("apply-button"),
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    let child = app.world().get::<UiChildren>(host).unwrap().0[0];
    app.world_mut().entity_mut(host).insert(UiLayoutResult {
        window,
        rect: UiRect::new(140.0, 72.0, 32.0, 20.0),
        clip: None,
        z: 2,
        content_size: [32.0, 20.0],
        dpi_scale: 1.0,
    });
    app.world_mut().entity_mut(child).insert(UiLayoutResult {
        window,
        rect: UiRect::new(140.0, 72.0, 32.0, 20.0),
        clip: None,
        z: 3,
        content_size: [32.0, 20.0],
        dpi_scale: 1.0,
    });
    app.world_mut()
        .get_mut::<Tooltip>(host)
        .unwrap()
        .delay_millis = 200;
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerEnter {
        window,
        target: child,
    }];
    app.update();
    assert!(!app.world().get::<Tooltip>(host).unwrap().visible);

    advance_tooltips_by(app.world_mut(), 0.25);
    assert!(app.world().get::<Tooltip>(host).unwrap().visible);
    collect_widget_paint(app.world_mut());
    let painted = app.world().resource::<DisplayList>().widget_items();
    assert!(painted.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, .. } if text == "Applies the current settings"
    )));
    assert!(painted.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { rect, .. }
            if rect.x >= 0.0 && rect.x + rect.width <= 180.0
    )));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::PointerLeave {
        window,
        target: child,
    }];
    app.update();
    assert!(!app.world().get::<Tooltip>(host).unwrap().visible);
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(
                &item.primitive,
                Primitive::TextWrapped { text, .. } if text == "Applies the current settings"
            ))
    );
}

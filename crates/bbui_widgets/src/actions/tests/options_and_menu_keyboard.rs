use super::*;

#[test]
pub(super) fn dropdown_popup_clamps_to_viewport_keeps_trigger_hitbox_and_routes_options() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let viewport = Rect::new(0.0, 0.0, 320.0, 220.0);
    app.world_mut()
        .spawn(UiLayoutRoot::new(window, viewport, 1.0));
    let options: Vec<_> = (0..12).map(|index| format!("Option {index}")).collect();
    let owner = WidgetTree::new(WidgetNode::dropdown("Quality", options, Some(0)))
        .spawn(app.world_mut(), None)
        .unwrap();
    let trigger = Rect::new(42.0, 155.0, 132.0, 24.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: trigger,
        clip: None,
        z: 7,
        content_size: [trigger.width, trigger.height],
        dpi_scale: 1.0,
    });
    app.world_mut().get_mut::<Dropdown>(owner).unwrap().open();
    app.update();

    let popup_state = *app.world().get::<DropdownPopupState>(owner).unwrap();
    assert!(popup_state.popup.open);
    assert_eq!(popup_state.popup.side, PopupSide::Above);
    assert!(popup_state.popup.bounds.x >= viewport.x);
    assert!(popup_state.popup.bounds.y >= viewport.y);
    assert!(
        popup_state.popup.bounds.x + popup_state.popup.bounds.width <= viewport.x + viewport.width
    );
    assert!(
        popup_state.popup.bounds.y + popup_state.popup.bounds.height
            <= viewport.y + viewport.height
    );
    assert!(popup_state.popup.bounds.y + popup_state.popup.bounds.height <= trigger.y);

    let owner_hit = app.world().get::<UiHitBox>(owner).unwrap();
    assert_eq!(
        owner_hit.rect, trigger,
        "open popup must not steal trigger hitbox"
    );
    let mut option_query = app.world_mut().query::<(Entity, &DropdownOptionControl)>();
    let options: Vec<_> = option_query
        .iter(app.world())
        .filter(|(_, option)| option.owner == owner)
        .map(|(entity, option)| (entity, option.clone()))
        .collect();
    assert_eq!(options.len(), 12);
    for (entity, option) in &options {
        let expected_visible = option.index >= popup_state.first_visible
            && option.index
                < popup_state
                    .first_visible
                    .saturating_add(popup_state.visible_count);
        assert_eq!(
            app.world()
                .get::<UiVisible>(*entity)
                .is_some_and(|visible| visible.0),
            expected_visible,
            "option row visibility follows the popup's virtual range"
        );
        if expected_visible {
            assert!(app.world().get::<UiHitBox>(*entity).is_some());
        }
    }
    assert!(app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "Option 0")));

    let click = |target| UiRoutedEvent::Click {
        window,
        target,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [trigger.x + 4.0, trigger.y + 4.0],
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(owner)];
    app.update();
    assert!(!app.world().get::<Dropdown>(owner).unwrap().open);
    assert_eq!(app.world().get::<UiHitBox>(owner).unwrap().rect, trigger);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(owner)];
    app.update();
    assert!(app.world().get::<Dropdown>(owner).unwrap().open);
    let option_entity = options
        .iter()
        .find_map(|(entity, option)| (option.index == 2).then_some(*entity))
        .unwrap();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: option_entity,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [trigger.x + 4.0, trigger.y + 4.0],
    }];
    app.update();
    let dropdown = app.world().get::<Dropdown>(owner).unwrap();
    assert_eq!(dropdown.selected, Some(2));
    assert!(!dropdown.open);
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(0)), after: WidgetValue::Option(Some(2)) } if *target == owner)));

    let key = |key| UiRoutedEvent::Key {
        window,
        target: owner,
        key,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    };
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
    app.update();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::PageDown)];
    app.update();
    assert_eq!(
        app.world().get::<Dropdown>(owner).unwrap().highlighted,
        Some(8)
    );
    assert_eq!(
        app.world()
            .get::<DropdownPopupState>(owner)
            .unwrap()
            .first_visible,
        3
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [trigger.x + 8.0, trigger.y + 8.0],
        delta: [0.0, -1.0],
        unit: bbui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<DropdownPopupState>(owner)
            .unwrap()
            .first_visible,
        6,
        "wheel scrolling advances the virtual option viewport"
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::End)];
    app.update();
    assert_eq!(
        app.world().get::<Dropdown>(owner).unwrap().highlighted,
        Some(11)
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
    app.update();
    assert_eq!(
        app.world().get::<Dropdown>(owner).unwrap().selected,
        Some(11)
    );
    assert!(!app.world().get::<Dropdown>(owner).unwrap().open);
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(2)), after: WidgetValue::Option(Some(11)) } if *target == owner)));
}

#[test]
pub(super) fn radio_group_materializes_stable_option_controls_for_pointer_and_accessibility() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::radio_group(
        "Mode",
        RadioGroup::new(["Object", "Edit", "Sculpt"], Some(0)),
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    let rect = Rect::new(12.0, 18.0, 180.0, 96.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 2,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();
    let mut query = app.world_mut().query::<(Entity, &RadioOptionControl)>();
    let before: Vec<_> = query
        .iter(app.world())
        .filter(|(_, option)| option.owner == owner)
        .map(|(entity, option)| (entity, option.clone()))
        .collect();
    assert_eq!(before.len(), 3);
    assert_eq!(before[0].1.label, "Object");
    assert!(before[0].1.selected);
    assert_eq!(before[1].1.label, "Edit");
    assert!(!before[1].1.selected);
    assert!(before.iter().all(|(entity, _)| {
        app.world().get::<UiHitBox>(*entity).is_some()
            && app
                .world()
                .get::<UiLayoutResult>(*entity)
                .is_some_and(|layout| layout.window == window)
    }));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: before[1].0,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [24.0, 55.0],
    }];
    app.update();
    assert_eq!(
        app.world().get::<RadioGroup>(owner).unwrap().selected,
        Some(1)
    );
    let mut query = app.world_mut().query::<(Entity, &RadioOptionControl)>();
    let after: Vec<_> = query
        .iter(app.world())
        .filter(|(_, option)| option.owner == owner)
        .map(|(entity, option)| (entity, option.clone()))
        .collect();
    assert_eq!(
        after.iter().map(|(entity, _)| *entity).collect::<Vec<_>>(),
        before.iter().map(|(entity, _)| *entity).collect::<Vec<_>>()
    );
    assert!(after[1].1.selected);
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(0)), after: WidgetValue::Option(Some(1)) } if *target == owner)));
}

#[test]
pub(super) fn option_views_skip_idle_rows_and_track_direct_model_writes() {
    let mut world = World::new();
    world.init_resource::<Theme>();
    world.init_resource::<WidgetControlOptionItems>();
    let window = world.spawn_empty().id();
    let viewport = Rect::new(0.0, 0.0, 500.0, 320.0);
    world.spawn(UiLayoutRoot::new(window, viewport, 1.0));
    let dropdown_rect = Rect::new(20.0, 20.0, 180.0, 24.0);
    let mut dropdown = Dropdown::new(["First", "Second", "Third"], Some(0));
    dropdown.open();
    let dropdown_owner = world
        .spawn((
            UiNode,
            dropdown,
            UiLayoutResult {
                window,
                rect: dropdown_rect,
                clip: None,
                z: 1,
                content_size: [dropdown_rect.width, dropdown_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let radio_rect = Rect::new(20.0, 140.0, 180.0, 100.0);
    let radio_owner = world
        .spawn((
            UiNode,
            RadioGroup::new(["A", "B", "C"], Some(0)),
            UiLayoutResult {
                window,
                rect: radio_rect,
                clip: None,
                z: 1,
                content_size: [radio_rect.width, radio_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();

    sync_control_option_views(&mut world);
    let dropdown_rows =
        world.resource::<WidgetControlOptionItems>().dropdowns[&dropdown_owner].clone();
    let radio_rows = world.resource::<WidgetControlOptionItems>().radios[&radio_owner].clone();
    let dropdown_ticks = dropdown_rows
        .iter()
        .map(|entity| {
            (
                entity_component_changed_tick::<DropdownOptionControl>(&world, *entity),
                entity_component_changed_tick::<UiVisible>(&world, *entity),
                entity_component_changed_tick::<UiEnabled>(&world, *entity),
                entity_component_changed_tick::<UiLayoutResult>(&world, *entity),
                entity_component_changed_tick::<UiHitBox>(&world, *entity),
            )
        })
        .collect::<Vec<_>>();
    let radio_ticks = radio_rows
        .iter()
        .map(|entity| {
            (
                entity_component_changed_tick::<RadioOptionControl>(&world, *entity),
                entity_component_changed_tick::<UiVisible>(&world, *entity),
                entity_component_changed_tick::<UiEnabled>(&world, *entity),
                entity_component_changed_tick::<UiLayoutResult>(&world, *entity),
                entity_component_changed_tick::<UiHitBox>(&world, *entity),
            )
        })
        .collect::<Vec<_>>();
    world.increment_change_tick();
    sync_control_option_views(&mut world);
    assert_eq!(
        dropdown_rows
            .iter()
            .map(|entity| (
                entity_component_changed_tick::<DropdownOptionControl>(&world, *entity),
                entity_component_changed_tick::<UiVisible>(&world, *entity),
                entity_component_changed_tick::<UiEnabled>(&world, *entity),
                entity_component_changed_tick::<UiLayoutResult>(&world, *entity),
                entity_component_changed_tick::<UiHitBox>(&world, *entity),
            ))
            .collect::<Vec<_>>(),
        dropdown_ticks,
        "idle dropdown sync must not rewrite option components"
    );
    assert_eq!(
        radio_rows
            .iter()
            .map(|entity| (
                entity_component_changed_tick::<RadioOptionControl>(&world, *entity),
                entity_component_changed_tick::<UiVisible>(&world, *entity),
                entity_component_changed_tick::<UiEnabled>(&world, *entity),
                entity_component_changed_tick::<UiLayoutResult>(&world, *entity),
                entity_component_changed_tick::<UiHitBox>(&world, *entity),
            ))
            .collect::<Vec<_>>(),
        radio_ticks,
        "idle radio sync must not rewrite option components"
    );

    world.get_mut::<Dropdown>(dropdown_owner).unwrap().selected = Some(1);
    world.get_mut::<RadioGroup>(radio_owner).unwrap().selected = Some(1);
    sync_control_option_views(&mut world);
    assert!(
        world
            .get::<DropdownOptionControl>(dropdown_rows[1])
            .unwrap()
            .selected
    );
    assert!(
        world
            .get::<RadioOptionControl>(radio_rows[1])
            .unwrap()
            .selected
    );
}

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

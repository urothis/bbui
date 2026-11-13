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

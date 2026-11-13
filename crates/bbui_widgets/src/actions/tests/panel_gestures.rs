use super::*;

#[test]
pub(super) fn ctrl_click_exclusive_panel_collapses_siblings_and_advances_animation() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::panel("One", false, [WidgetNode::button("One child")]).with_key("one"),
        WidgetNode::panel("Two", false, [WidgetNode::button("Two child")]).with_key("two"),
    ]));
    let root = tree.spawn(app.world_mut(), None).unwrap();
    app.world_mut()
        .entity_mut(root)
        .insert(PanelGroup::with_panels(["one", "two"]));
    let panels = app
        .world()
        .get::<bbui_core::UiChildren>(root)
        .unwrap()
        .0
        .clone();
    let child = app
        .world()
        .get::<bbui_core::UiChildren>(panels[1])
        .unwrap()
        .0[0];
    for (index, panel_entity) in panels.iter().copied().enumerate() {
        {
            let mut panel = app.world_mut().get_mut::<Panel>(panel_entity).unwrap();
            panel.exclusive = true;
            panel.persistence_key = Some(if index == 0 { "one" } else { "two" }.into());
            if index == 0 {
                panel.state.set_expanded(true);
            }
        }
        app.world_mut().entity_mut(panel_entity).insert((
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, index as f32 * 120.0, 180.0, 120.0),
                clip: None,
                z: index as i32,
                content_size: [180.0, 120.0],
                dpi_scale: 1.0,
            },
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, index as f32 * 120.0, 180.0, 120.0),
                clip: None,
                z_index: index as i32,
            },
        ));
    }
    app.world_mut().entity_mut(child).insert((
        UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 144.0, 180.0, 80.0),
            clip: None,
            z: 3,
            content_size: [180.0, 80.0],
            dpi_scale: 1.0,
        },
        UiHitBox {
            window: Some(window),
            rect: UiRect::new(0.0, 144.0, 180.0, 80.0),
            clip: None,
            z_index: 3,
        },
    ));
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
        UiRoutedEvent::Key {
            window,
            target: panels[1],
            key: UiKeyCode::Other(17),
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..Default::default()
            },
        },
        UiRoutedEvent::Click {
            window,
            target: panels[1],
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [8.0, 128.0],
        },
    ];
    app.update();

    assert!(
        !app.world()
            .get::<Panel>(panels[0])
            .unwrap()
            .state
            .target_expanded
    );
    let expanded = app.world().get::<Panel>(panels[1]).unwrap();
    assert!(expanded.state.target_expanded);
    assert!(expanded.state.progress > 0.0 && expanded.state.progress < 1.0);
    assert!(app.world().get::<UiVisible>(child).unwrap().0);
    let child_hit = app.world().get::<UiHitBox>(child).unwrap();
    assert!(
        child_hit
            .clip
            .is_some_and(|clip| clip.height < child_hit.rect.height)
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelGroupCommitted { changes } if changes.len() == 2 && changes.iter().any(|(entity, before, after)| *entity == panels[0] && *before && !*after) && changes.iter().any(|(entity, before, after)| *entity == panels[1] && !*before && *after))));
}

#[test]
pub(super) fn exclusive_panels_with_duplicate_keys_stay_inside_their_group() {
    let mut world = World::new();
    let first_root = world
        .spawn((UiNode, PanelGroup::with_panels(["shared", "first-local"])))
        .id();
    let second_root = world
        .spawn((UiNode, PanelGroup::with_panels(["shared", "second-local"])))
        .id();
    let first_shared = world.spawn((UiNode, Panel::new("Shared", false))).id();
    let first_local = world.spawn((UiNode, Panel::new("First local", true))).id();
    let second_shared = world.spawn((UiNode, Panel::new("Shared", false))).id();
    let second_local = world.spawn((UiNode, Panel::new("Second local", true))).id();
    for (entity, key) in [
        (first_shared, "shared"),
        (first_local, "first-local"),
        (second_shared, "shared"),
        (second_local, "second-local"),
    ] {
        let mut panel = world.get_mut::<Panel>(entity).unwrap();
        panel.exclusive = true;
        panel.persistence_key = Some(key.to_owned());
    }
    world
        .get_mut::<PanelGroup>(second_root)
        .unwrap()
        .exclusive_open = Some("second-local".to_owned());
    bbui_core::set_parent(&mut world, first_shared, first_root).unwrap();
    bbui_core::set_parent(&mut world, first_local, first_root).unwrap();
    bbui_core::set_parent(&mut world, second_shared, second_root).unwrap();
    bbui_core::set_parent(&mut world, second_local, second_root).unwrap();
    let mut events = Vec::new();

    toggle_exclusive_panel(&mut world, first_shared, &mut events);

    assert!(
        world
            .get::<Panel>(first_shared)
            .unwrap()
            .state
            .target_expanded
    );
    assert!(
        !world
            .get::<Panel>(first_local)
            .unwrap()
            .state
            .target_expanded
    );
    assert!(
        !world
            .get::<Panel>(second_shared)
            .unwrap()
            .state
            .target_expanded
    );
    assert!(
        world
            .get::<Panel>(second_local)
            .unwrap()
            .state
            .target_expanded
    );
    assert_eq!(
        world
            .get::<PanelGroup>(second_root)
            .unwrap()
            .exclusive_open
            .as_deref(),
        Some("second-local")
    );
    assert!(matches!(
        events.as_slice(),
        [WidgetEvent::PanelGroupCommitted { changes }]
            if changes.len() == 2
                && changes.iter().any(|(entity, before, after)| *entity == first_shared && !*before && *after)
                && changes.iter().any(|(entity, before, after)| *entity == first_local && *before && !*after)
    ));
}

#[test]
pub(super) fn ordinary_panel_toggle_is_one_reversible_value_edit() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let panel = app.world_mut().spawn(Panel::new("Details", false)).id();
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    assert!(
        app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );
    assert_eq!(app.world().resource::<bbui_core::UiHistory>().undo_len(), 1);

    let route_history = |app: &mut App, redo: bool| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window: Entity::PLACEHOLDER,
            target: panel,
            key: if redo { UiKeyCode::Y } else { UiKeyCode::Z },
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
    };
    route_history(&mut app, false);
    assert!(
        !app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );
    route_history(&mut app, true);
    assert!(
        app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window: Entity::PLACEHOLDER,
        target: panel,
        key: UiKeyCode::Space,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert!(
        !app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window: Entity::PLACEHOLDER,
        target: panel,
        key: UiKeyCode::Enter,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    assert!(
        app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );
}

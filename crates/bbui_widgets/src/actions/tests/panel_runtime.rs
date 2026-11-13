use super::*;

#[test]
pub(super) fn collapsing_panel_moves_following_panel_and_hit_box_during_motion() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bbui_layout::LayoutPlugin, WidgetPlugin))
        .init_resource::<bevy_time::Time>();
    app.world_mut().resource_mut::<Theme>().motion.panel_expand =
        std::time::Duration::from_millis(400);
    let window = app.world_mut().spawn_empty().id();
    let root = crate::WidgetTree::new(
        crate::WidgetNode::column([
            crate::WidgetNode::panel(
                "Transform",
                true,
                [crate::WidgetNode::label("Location")
                    .with_key("transform-content")
                    .with_height(bbui_layout::UiLength::Px(100.0))],
            )
            .with_key("transform"),
            crate::WidgetNode::panel("Visibility", false, []).with_key("visibility"),
        ])
        .with_gap(4.0),
    )
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(root).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 240.0, 300.0),
        1.0,
    ));
    let find = |world: &World, key: &str| {
        world
            .iter_entities()
            .find_map(|entry| {
                entry
                    .get::<UiKey>()
                    .is_some_and(|value| value.0 == key)
                    .then_some(entry.id())
            })
            .unwrap()
    };
    let transform = find(app.world(), "transform");
    let content = find(app.world(), "transform-content");
    let visibility = find(app.world(), "visibility");
    app.update();
    let expanded_y = app
        .world()
        .get::<UiLayoutResult>(visibility)
        .unwrap()
        .rect
        .y;

    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(std::time::Duration::from_millis(200));
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(transform, WidgetActionKind::TogglePanel));
    app.update();
    let halfway_y = app
        .world()
        .get::<UiLayoutResult>(visibility)
        .unwrap()
        .rect
        .y;
    assert!(
        halfway_y < expanded_y,
        "following panel must move before collapse completes"
    );
    assert_eq!(
        app.world().get::<UiHitBox>(visibility).unwrap().rect.y,
        halfway_y
    );
    let clip = app.world().get::<UiHitBox>(content).unwrap().clip.unwrap();
    assert!(clip.y + clip.height <= halfway_y);

    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(std::time::Duration::from_millis(200));
    app.update();
    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(std::time::Duration::ZERO);
    app.update();
    let collapsed_y = app
        .world()
        .get::<UiLayoutResult>(visibility)
        .unwrap()
        .rect
        .y;
    assert!(collapsed_y < halfway_y);
    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(std::time::Duration::ZERO);
    app.update();
    assert_eq!(
        app.world()
            .resource::<bbui_layout::UiLayoutStats>()
            .roots_processed,
        0
    );
}

#[test]
pub(super) fn reorderable_panel_group_routes_keyboard_drag_cancel_and_persists_order() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bbui_layout::LayoutPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let make_group = |world: &mut World, order: [&str; 3]| {
        let group = world
            .spawn((
                UiNode,
                UiKey("settings".into()),
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 240.0, 300.0), 1.0),
                UiLayoutNode::column(4.0),
                PanelGroup::with_panels(order).with_reordering("settings"),
            ))
            .id();
        let mut panels = HashMap::new();
        for key in order {
            let mut panel = Panel::new(key, false);
            panel.persistence_key = Some(key.to_owned());
            let entity = world
                .spawn((
                    UiNode,
                    UiVisible(true),
                    UiEnabled(true),
                    UiFocusable,
                    UiKey(key.to_owned()),
                    UiLayoutNode::leaf(
                        bbui_layout::UiLength::Fill(1.0),
                        bbui_layout::UiLength::Px(80.0),
                    ),
                    panel,
                ))
                .id();
            set_parent(world, entity, group).unwrap();
            panels.insert(key.to_owned(), entity);
        }
        (group, panels)
    };
    let (group, panels) = make_group(app.world_mut(), ["alpha", "beta", "gamma"]);
    app.update();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(panels["alpha"]);

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::ArrowDown,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        });
    app.update();
    app.update();
    let after_keyboard = vec!["beta".to_owned(), "alpha".to_owned(), "gamma".to_owned()];
    assert_eq!(
        app.world().get::<PanelGroup>(group).unwrap().panel_keys,
        after_keyboard
    );
    assert_eq!(
        app.world().get::<UiChildren>(group).unwrap().0,
        [panels["beta"], panels["alpha"], panels["gamma"]]
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelOrderCommitted { group: target, persistence_key, before, after } if *target == group && persistence_key == "settings" && before == &["alpha", "beta", "gamma"] && after == &after_keyboard)));
    assert_eq!(
        app.world().resource::<PanelOrderStore>().get("settings"),
        Some(after_keyboard.as_slice())
    );

    let alpha_y = app
        .world()
        .get::<UiLayoutResult>(panels["alpha"])
        .unwrap()
        .rect
        .y;
    let gamma_y = app
        .world()
        .get::<UiLayoutResult>(panels["gamma"])
        .unwrap()
        .rect
        .y;
    let beta_y = app
        .world()
        .get::<UiLayoutResult>(panels["beta"])
        .unwrap()
        .rect
        .y;
    assert!(
        beta_y < alpha_y && alpha_y < gamma_y,
        "sibling layout should follow keyed order: beta={beta_y}, alpha={alpha_y}, gamma={gamma_y}"
    );

    let start = [8.0, alpha_y + 8.0];
    let end = [8.0, 290.0];
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 1.0,
        },
        UiInputEvent::PointerMove {
            window,
            position: end,
            time_seconds: 1.2,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: end,
            time_seconds: 1.3,
        },
    ]);
    app.update();
    let after_drag = vec!["beta".to_owned(), "gamma".to_owned(), "alpha".to_owned()];
    assert_eq!(
        app.world().get::<PanelGroup>(group).unwrap().panel_keys,
        after_drag
    );
    assert!(
        !app.world()
            .get::<Panel>(panels["alpha"])
            .unwrap()
            .state
            .target_expanded
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelOrderCommitted { group: target, before, after, .. } if *target == group && before == &after_keyboard && after == &after_drag)));
    assert_eq!(
        app.world().resource::<PanelOrderStore>().get("settings"),
        Some(after_drag.as_slice())
    );

    let beta = panels["beta"];
    let beta_y = app.world().get::<UiLayoutResult>(beta).unwrap().rect.y;
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [8.0, beta_y + 8.0],
            time_seconds: 2.0,
        },
        UiInputEvent::PointerMove {
            window,
            position: [8.0, 250.0],
            time_seconds: 2.2,
        },
    ]);
    app.update();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerCancel { window });
    app.update();
    assert_eq!(
        app.world().get::<PanelGroup>(group).unwrap().panel_keys,
        after_drag
    );
    assert_eq!(
        app.world().resource::<PanelOrderStore>().get("settings"),
        Some(after_drag.as_slice()),
        "cancel restores the persisted committed order"
    );

    for entity in [panels["alpha"], panels["beta"], panels["gamma"], group] {
        app.world_mut().despawn(entity);
    }
    let (rebuilt, rebuilt_panels) = make_group(app.world_mut(), ["alpha", "beta", "gamma"]);
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<PanelGroup>(rebuilt).unwrap().panel_keys,
        after_drag
    );
    assert_eq!(
        app.world().get::<UiChildren>(rebuilt).unwrap().0,
        [
            rebuilt_panels["beta"],
            rebuilt_panels["gamma"],
            rebuilt_panels["alpha"]
        ]
    );
    assert_eq!(
        app.world().get::<UiKey>(rebuilt).unwrap().0,
        "settings",
        "the reorder group's stable UiKey survives tree reconstruction"
    );
}

#[test]
pub(super) fn panel_widget_system_snaps_for_zero_scale_and_reduced_motion() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut()
        .init_resource::<bbui_style::UiReducedMotion>();
    let panel = app.world_mut().spawn(Panel::new("Motion", false)).id();

    app.world_mut().resource_mut::<Theme>().motion_scale = 0.0;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert!(state.target_expanded);
    assert_eq!(state.progress, 1.0);

    app.world_mut().resource_mut::<Theme>().motion_scale = 1.0;
    app.world_mut()
        .resource_mut::<bbui_style::UiReducedMotion>()
        .0 = true;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert!(!state.target_expanded);
    assert_eq!(state.progress, 0.0);
}

#[test]
pub(super) fn panel_widget_uses_typed_theme_duration_and_leaves_settled_state_unchanged() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetPlugin))
        .init_resource::<bevy_time::Time>()
        .init_resource::<bbui_style::UiReducedMotion>();
    let panel = app.world_mut().spawn(Panel::new("Motion", false)).id();
    app.world_mut().resource_mut::<Theme>().motion.panel_expand =
        std::time::Duration::from_millis(400);
    app.update();

    let panel_id = app.world().components().component_id::<Panel>().unwrap();
    let changed_before = app
        .world()
        .get_entity(panel)
        .unwrap()
        .get_change_ticks_by_id(panel_id)
        .unwrap()
        .changed
        .get();
    app.update();
    let changed_after = app
        .world()
        .get_entity(panel)
        .unwrap()
        .get_change_ticks_by_id(panel_id)
        .unwrap()
        .changed
        .get();
    assert_eq!(changed_after, changed_before, "settled panels stay clean");

    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(std::time::Duration::from_millis(200));
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert!(
        app.world()
            .resource::<bbui_motion::MotionPlayer>()
            .is_animating(panel, panel_progress_binding().id(),)
    );
    assert!((state.progress - 0.5).abs() < 0.001);

    app.world_mut()
        .resource_mut::<bbui_style::UiReducedMotion>()
        .0 = true;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert_eq!(state.progress, 0.0, "reduced motion keeps toggling usable");
}

#[test]
pub(super) fn panel_clip_sync_visits_only_changed_panels_after_initialization() {
    let mut app = App::new();
    app.init_resource::<Theme>()
        .add_systems(Update, sync_panel_children);
    let window = app.world_mut().spawn_empty().id();
    let make_panel = |world: &mut World| {
        world
            .spawn((
                UiNode,
                Panel::new("Section", true),
                UiLayoutResult {
                    window,
                    rect: Rect::new(0.0, 0.0, 160.0, 80.0),
                    clip: None,
                    z: 0,
                    content_size: [160.0, 80.0],
                    dpi_scale: 1.0,
                },
            ))
            .id()
    };
    let changed = make_panel(app.world_mut());
    let untouched = make_panel(app.world_mut());
    app.update();
    let sentinel = PanelSyncStamp {
        panel_changed: 0,
        children_changed: None,
        layout_changed: None,
        theme_changed: None,
        progress_bits: 0,
    };
    app.world_mut()
        .resource_mut::<WidgetPanelSyncCache>()
        .stamps
        .insert(untouched, sentinel);
    app.world_mut()
        .get_mut::<Panel>(changed)
        .unwrap()
        .state
        .progress = 0.5;
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPanelSyncCache>()
            .stamps
            .get(&untouched),
        Some(&sentinel),
        "an unrelated panel must never enter the clip sync worklist"
    );
}

#[test]
pub(super) fn panel_clip_sync_is_idle_safe_and_restores_clips_after_reopen_or_reparent() {
    let mut app = App::new();
    app.init_resource::<Theme>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetPanelSyncCache>()
        .init_resource::<WidgetPanelSyncInvalidations>()
        .add_systems(
            Update,
            (collect_panel_descendant_invalidations, sync_panel_children).chain(),
        );
    let window = app.world_mut().spawn_empty().id();
    let panel_rect = Rect::new(0.0, 0.0, 160.0, 120.0);
    let panel = app
        .world_mut()
        .spawn((
            UiNode,
            Panel::new("Details", false),
            UiLayoutResult {
                window,
                rect: panel_rect,
                clip: None,
                z: 1,
                content_size: [panel_rect.width, panel_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let child_rect = Rect::new(8.0, 32.0, 140.0, 24.0);
    let child = app
        .world_mut()
        .spawn((
            UiNode,
            UiVisible(true),
            UiFocusable,
            crate::WidgetElement {
                key: None,
                spec: crate::declarative::WidgetSpec::Label {
                    text: "Child".into(),
                },
            },
            UiLayoutResult {
                window,
                rect: child_rect,
                clip: None,
                z: 2,
                content_size: [child_rect.width, child_rect.height],
                dpi_scale: 1.0,
            },
            UiHitBox {
                window: Some(window),
                rect: to_ui_rect(child_rect),
                clip: None,
                z_index: 2,
            },
        ))
        .id();
    set_parent(app.world_mut(), child, panel).unwrap();

    app.update();
    let collapsed_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert_eq!(collapsed_clip.height, 0.0);
    app.update();
    app.update();
    let stable_layout_tick =
        entity_component_changed_tick::<UiLayoutResult>(app.world(), child).unwrap();
    let stable_hitbox_tick = entity_component_changed_tick::<UiHitBox>(app.world(), child).unwrap();
    app.update();
    assert_eq!(
        entity_component_changed_tick::<UiLayoutResult>(app.world(), child),
        Some(stable_layout_tick),
        "idle collapsed panels must not rewrite descendant layouts"
    );
    assert_eq!(
        entity_component_changed_tick::<UiHitBox>(app.world(), child),
        Some(stable_hitbox_tick),
        "idle collapsed panels must not rewrite descendant hitboxes"
    );

    {
        let mut layout = app.world_mut().get_mut::<UiLayoutResult>(child).unwrap();
        layout.rect.y += 4.0;
        layout.clip = None;
    }
    app.update();
    let direct_edit_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert_eq!(direct_edit_clip.height, 0.0);
    assert_eq!(
        app.world().get::<UiLayoutResult>(child).unwrap().rect.y,
        36.0
    );

    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.25;
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .target_expanded = true;
    app.update();
    let quarter_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert!(quarter_clip.height > 0.0);
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.5;
    app.update();
    let half_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert!(half_clip.height > quarter_clip.height);
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 1.0;
    app.update();
    assert_eq!(app.world().get::<UiLayoutResult>(child).unwrap().clip, None);
    assert_eq!(app.world().get::<UiHitBox>(child).unwrap().clip, None);
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(child);
    app.world_mut().resource_mut::<UiFocus>().set_active(window);

    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.0;
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .target_expanded = false;
    app.update();
    assert_eq!(app.world().resource::<UiFocus>().focused, None);
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.25;
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .target_expanded = true;
    app.update();
    assert!(
        app.world()
            .get::<UiLayoutResult>(child)
            .unwrap()
            .clip
            .unwrap()
            .height
            > 0.0
    );

    detach_from_parent(app.world_mut(), child);
    app.update();
    assert_eq!(app.world().get::<UiLayoutResult>(child).unwrap().clip, None);
    assert_eq!(app.world().get::<UiHitBox>(child).unwrap().clip, None);
    assert!(
        !app.world()
            .resource::<WidgetPanelSyncCache>()
            .clips
            .contains_key(&child)
    );
}

#[test]
pub(super) fn panel_pointer_click_toggles_only_from_header() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let panel = app.world_mut().spawn(Panel::new("Details", false)).id();
    app.world_mut().entity_mut(panel).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 180.0, 100.0),
        clip: None,
        z: 1,
        content_size: [180.0, 100.0],
        dpi_scale: 1.0,
    });

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: panel,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [12.0, 45.0],
    }];
    app.update();
    assert!(
        !app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: panel,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [12.0, 12.0],
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

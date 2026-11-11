use super::*;
use crate::{UiChildren, UiNode, UiParent, UiVisible, cleanup_removed_node, set_parent};

fn setup() -> (World, Entity, Entity, Entity) {
    let mut world = World::new();
    world.init_resource::<UiFocus>();
    world.init_resource::<UiInputQueue>();
    world.init_resource::<UiRoutedEvents>();
    let window = world.spawn_empty().id();
    let root = world
        .spawn((
            UiNode,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z_index: 0,
            },
        ))
        .id();
    let button = world
        .spawn((
            UiNode,
            UiFocusable,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(10.0, 10.0, 20.0, 20.0),
                clip: Some(UiRect::new(0.0, 0.0, 25.0, 25.0)),
                z_index: 1,
            },
        ))
        .id();
    set_parent(&mut world, button, root).unwrap();
    (world, window, root, button)
}

#[test]
fn stationary_pointer_retests_only_after_hit_geometry_changes() {
    let mut app = bevy_app::App::new();
    app.add_plugins(crate::CorePlugin);
    let window = app.world_mut().spawn_empty().id();
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z_index: 0,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position: [20.0, 20.0],
            time_seconds: 0.0,
        });
    app.update();
    let first = app.world().resource::<UiHoverValidation>().hit_tests;
    assert_eq!(app.world().resource::<UiFocus>().hovered, Some(target));

    app.update();
    assert_eq!(app.world().resource::<UiHoverValidation>().hit_tests, first);

    let other = app
        .world_mut()
        .spawn((
            UiNode,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(200.0, 200.0, 20.0, 20.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.update();
    let after_insert = app.world().resource::<UiHoverValidation>().hit_tests;
    app.world_mut().get_mut::<UiHitBox>(other).unwrap().rect =
        UiRect::new(250.0, 250.0, 20.0, 20.0);
    app.update();
    assert_eq!(
        app.world().resource::<UiHoverValidation>().hit_tests,
        after_insert
    );

    app.world_mut().entity_mut(other).insert(UiVisible(false));
    app.update();
    let after_hide = app.world().resource::<UiHoverValidation>().hit_tests;
    assert_eq!(after_hide, after_insert);
    app.world_mut().get_mut::<UiVisible>(other).unwrap().0 = true;
    app.update();
    assert_eq!(
        app.world().resource::<UiHoverValidation>().hit_tests,
        after_hide
    );
    app.world_mut().entity_mut(other).remove::<UiVisible>();
    app.update();
    assert_eq!(
        app.world().resource::<UiHoverValidation>().hit_tests,
        after_hide
    );

    app.world_mut().get_mut::<UiHitBox>(target).unwrap().rect =
        UiRect::new(200.0, 200.0, 100.0, 100.0);
    app.update();
    assert!(app.world().resource::<UiHoverValidation>().hit_tests > first);
    assert_eq!(app.world().resource::<UiFocus>().hovered, None);

    app.world_mut().get_mut::<UiHitBox>(other).unwrap().rect = UiRect::new(10.0, 10.0, 20.0, 20.0);
    app.update();
    assert_eq!(app.world().resource::<UiFocus>().hovered, Some(other));
}

#[test]
fn pointer_capture_focus_and_click_route_to_the_topmost_target() {
    let (mut world, window, _root, button) = setup();
    world.resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 0.0,
        },
        UiInputEvent::PointerMove {
            window,
            position: [70.0, 70.0],
            time_seconds: 0.1,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: [70.0, 70.0],
            time_seconds: 0.2,
        },
    ]);
    process_ui_input(&mut world);
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerDown {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0]
    }));
    assert!(events.iter().any(|event| matches!(event, UiRoutedEvent::PointerMoved { target, dragging: true, .. } if *target == button)));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiRoutedEvent::Click { .. }))
    );
    assert_eq!(
        world.resource::<UiFocus>().state(window).unwrap().focused,
        Some(button)
    );
}

#[test]
fn modifier_pointer_events_preserve_legacy_delivery_and_annotate_edges() {
    let (mut world, window, _root, button) = setup();
    let modifiers = UiModifiers {
        shift: true,
        control: true,
        alt: false,
        super_key: false,
    };
    world.resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButtonWithModifiers {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 0.0,
            modifiers,
        },
        UiInputEvent::PointerButtonWithModifiers {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: [15.0, 15.0],
            time_seconds: 0.1,
            modifiers,
        },
    ]);

    process_ui_input(&mut world);

    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerDown {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
    }));
    assert!(events.contains(&UiRoutedEvent::PointerDownWithModifiers {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
        modifiers,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
        cancelled: false,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUpWithModifiers {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
        modifiers,
    }));
    assert!(events.contains(&UiRoutedEvent::Click {
        window,
        target: button,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [15.0, 15.0],
    }));
    assert!(events.contains(&UiRoutedEvent::ClickWithModifiers {
        window,
        target: button,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [15.0, 15.0],
        modifiers,
    }));
}

#[test]
fn tab_skips_hidden_and_disabled_controls_and_wraps_within_its_window() {
    let (mut world, window, _root, first) = setup();
    let second = world
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(40.0, 10.0, 10.0, 10.0),
                clip: None,
                z_index: 0,
            },
        ))
        .id();
    let hidden = world
        .spawn((UiNode, UiFocusable, UiTabIndex(2), UiVisible(false)))
        .id();
    let disabled = world
        .spawn((UiNode, UiFocusable, UiTabIndex(3), UiEnabled(false)))
        .id();
    let hidden_parent = world.spawn((UiNode, UiVisible(false))).id();
    let hidden_child = world.spawn((UiNode, UiFocusable, UiTabIndex(4))).id();
    let clipped = world
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(4),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(50.0, 10.0, 10.0, 10.0),
                clip: Some(UiRect::ZERO),
                z_index: 0,
            },
        ))
        .id();
    let third = world.spawn((UiNode, UiFocusable, UiTabIndex(5))).id();
    set_parent(&mut world, second, _root).unwrap();
    set_parent(&mut world, hidden, _root).unwrap();
    set_parent(&mut world, disabled, _root).unwrap();
    set_parent(&mut world, hidden_parent, _root).unwrap();
    set_parent(&mut world, hidden_child, hidden_parent).unwrap();
    set_parent(&mut world, clipped, _root).unwrap();
    set_parent(&mut world, third, _root).unwrap();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(first);
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });
    process_ui_input(&mut world);
    assert_eq!(focused(&world, window), Some(second));
    assert_eq!(
        traverse_focus(&mut world, window, Some(second), false),
        Some(third)
    );
    assert_eq!(
        traverse_focus(&mut world, window, Some(third), false),
        Some(first)
    );
    assert_eq!(
        traverse_focus(&mut world, window, Some(first), true),
        Some(third)
    );
}

#[test]
fn fully_clipped_focus_is_cleared_before_routing_keyboard_input() {
    let (mut world, window, root, _) = setup();
    let clipped = world
        .spawn((
            UiNode,
            UiFocusable,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(20.0, 20.0, 20.0, 20.0),
                clip: Some(UiRect::ZERO),
                z_index: 1,
            },
        ))
        .id();
    set_parent(&mut world, clipped, root).unwrap();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(clipped);
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });
    process_ui_input(&mut world);
    assert_eq!(focused(&world, window), None);
    assert!(world.resource::<UiRoutedEvents>().events.iter().any(|event| {
        matches!(event, UiRoutedEvent::FocusChanged { previous: Some(target), current: None, .. } if *target == clipped)
    }));
    assert!(
        !world
            .resource::<UiRoutedEvents>()
            .events
            .iter()
            .any(|event| {
                matches!(event, UiRoutedEvent::Key { target, .. } if *target == clipped)
            })
    );
}

#[test]
fn tab_traversal_never_focuses_a_control_from_another_window() {
    let (mut world, window_a, root_a, first) = setup();
    let window_b = world.spawn_empty().id();
    let root_b = world
        .spawn((
            UiNode,
            UiHitBox {
                window: Some(window_b),
                rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                clip: None,
                z_index: 0,
            },
        ))
        .id();
    let next_a = world
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window_a),
                rect: UiRect::new(40.0, 10.0, 10.0, 10.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    let other_window = world
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(1),
            UiHitBox {
                window: Some(window_b),
                rect: UiRect::new(10.0, 10.0, 10.0, 10.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    set_parent(&mut world, next_a, root_a).unwrap();
    set_parent(&mut world, other_window, root_b).unwrap();
    world.resource_mut::<UiFocus>().state_mut(window_a).focused = Some(first);
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window: window_a,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });

    process_ui_input(&mut world);

    assert_eq!(focused(&world, window_a), Some(next_a));
    assert_eq!(focused(&world, window_b), None);
    assert!(world.resource::<UiRoutedEvents>().events.iter().any(
            |event| matches!(event, UiRoutedEvent::FocusChanged { window, current: Some(target), .. } if *window == window_a && *target == next_a)
        ));
}

#[test]
fn tab_traversal_skips_focusables_without_surface_geometry() {
    let (mut world, window, root, first) = setup();
    let unassigned = world.spawn((UiNode, UiFocusable, UiTabIndex(1))).id();
    let assigned = world
        .spawn((
            UiNode,
            UiFocusable,
            UiTabIndex(2),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(40.0, 10.0, 10.0, 10.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    set_parent(&mut world, assigned, root).unwrap();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(first);
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });

    process_ui_input(&mut world);

    assert_eq!(focused(&world, window), Some(assigned));
    assert!(is_interactable_in_window(&world, unassigned, window));
    assert!(!crate::has_window_assignment(&world, unassigned, window));
}

#[test]
fn overlapping_surfaces_are_hit_tested_independently() {
    let (mut world, window, _root, button) = setup();
    let other_window = world.spawn_empty().id();
    let overlay = world
        .spawn((
            UiNode,
            UiHitBox {
                window: Some(other_window),
                rect: UiRect::new(10.0, 10.0, 20.0, 20.0),
                clip: None,
                z_index: 100,
            },
        ))
        .id();
    assert_eq!(hit_test(&mut world, window, [15.0, 15.0]), Some(button));
    assert_eq!(
        hit_test(&mut world, other_window, [15.0, 15.0]),
        Some(overlay)
    );
}

#[test]
fn deeply_nested_control_wins_hit_testing_over_its_containers() {
    let mut world = World::new();
    for _ in 0..400 {
        world.spawn_empty();
    }
    let window = world.spawn_empty().id();
    let hit = UiHitBox {
        window: Some(window),
        rect: UiRect::new(0.0, 0.0, 32.0, 32.0),
        clip: None,
        z_index: 0,
    };
    let root = world.spawn((UiNode, hit)).id();
    let mut parent = root;
    for _ in 0..16 {
        let sibling = world.spawn(UiNode).id();
        set_parent(&mut world, sibling, parent).unwrap();
        let container = world.spawn((UiNode, hit)).id();
        set_parent(&mut world, container, parent).unwrap();
        parent = container;
    }
    let button = world.spawn((UiNode, UiFocusable, hit)).id();
    set_parent(&mut world, button, parent).unwrap();

    assert!(hierarchy_order(&world, root) < hierarchy_order(&world, button));
    assert_eq!(hit_test(&mut world, window, [16.0, 16.0]), Some(button));
}

#[test]
fn removed_or_disabled_capture_is_cancelled_and_children_detach_on_cleanup() {
    let (mut world, window, root, button) = setup();
    world.resource_mut::<UiFocus>().state_mut(window).captured = Some(button);
    world.entity_mut(button).insert(crate::UiEnabled(false));
    process_ui_input(&mut world);
    assert!(world.resource::<UiRoutedEvents>().events.iter().any(
        |event| matches!(event, UiRoutedEvent::PointerCancel { target, .. } if *target == button)
    ));
    assert_eq!(
        world.resource::<UiFocus>().state(window).unwrap().captured,
        None
    );
    cleanup_removed_node(&mut world, button);
    assert!(world.get::<UiChildren>(root).unwrap().0.is_empty());
    assert!(world.get::<UiParent>(button).is_none());
}

#[test]
fn disabled_ancestor_cancels_capture_focus_and_stationary_hover_completely() {
    let (mut world, window, root, button) = setup();
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Secondary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 1.0,
        });
    process_ui_input(&mut world);
    assert_eq!(
        world.resource::<UiFocus>().state(window).unwrap().captured,
        Some(button)
    );
    assert_eq!(
        world
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .captured_button,
        Some(UiPointerButton::Secondary)
    );
    world.resource_mut::<UiRoutedEvents>().clear();

    world.entity_mut(root).insert(crate::UiEnabled(false));
    process_ui_input(&mut world);

    let state = world.resource::<UiFocus>().state(window).unwrap();
    assert_eq!(state.focused, None);
    assert_eq!(state.hovered, None);
    assert_eq!(state.captured, None);
    assert_eq!(state.selection, None);
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerCancel {
        window,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window,
        target: button,
        button: UiPointerButton::Secondary,
        position: [15.0, 15.0],
        cancelled: true,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerLeave {
        window,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::ImeDisabled {
        window,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::FocusChanged {
        window,
        previous: Some(button),
        current: None,
    }));
}

#[test]
fn hidden_target_cancels_focus_capture_and_hover_on_stationary_pointer() {
    let (mut world, window, root, button) = setup();
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 1.0,
        });
    process_ui_input(&mut world);
    world.resource_mut::<UiRoutedEvents>().clear();

    world.entity_mut(button).insert(UiVisible(false));
    process_ui_input(&mut world);

    let state = world.resource::<UiFocus>().state(window).unwrap();
    assert_eq!(state.focused, None);
    assert_eq!(state.hovered, Some(root));
    assert_eq!(state.captured, None);
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerCancel {
        window,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
        cancelled: true,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerLeave {
        window,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerEnter {
        window,
        target: root,
    }));
    assert!(events.contains(&UiRoutedEvent::FocusChanged {
        window,
        previous: Some(button),
        current: None,
    }));
}

#[test]
fn direct_despawn_cancels_state_even_without_cleanup_helper() {
    let (mut world, window, root, button) = setup();
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 1.5,
        });
    process_ui_input(&mut world);
    world.resource_mut::<UiRoutedEvents>().clear();

    world.despawn(button);
    process_ui_input(&mut world);

    let state = world.resource::<UiFocus>().state(window).unwrap();
    assert_eq!(state.focused, None);
    assert_eq!(state.captured, None);
    assert_eq!(state.hovered, Some(root));
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerCancel {
        window,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window,
        target: button,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
        cancelled: true,
    }));
    assert!(events.contains(&UiRoutedEvent::FocusChanged {
        window,
        previous: Some(button),
        current: None,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerEnter {
        window,
        target: root,
    }));
}

#[test]
fn moving_a_captured_control_to_another_window_cancels_old_surface_state() {
    let (mut world, window_a, root_a, button) = setup();
    let window_b = world.spawn_empty().id();
    let root_b = world
        .spawn((
            UiNode,
            UiHitBox {
                window: Some(window_b),
                rect: UiRect::new(200.0, 0.0, 100.0, 100.0),
                clip: None,
                z_index: 0,
            },
        ))
        .id();
    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerButton {
            window: window_a,
            button: UiPointerButton::Middle,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 2.0,
        });
    process_ui_input(&mut world);
    world.resource_mut::<UiRoutedEvents>().clear();

    set_parent(&mut world, button, root_b).unwrap();
    world.entity_mut(button).insert(UiHitBox {
        window: Some(window_b),
        rect: UiRect::new(210.0, 10.0, 20.0, 20.0),
        clip: None,
        z_index: 1,
    });
    process_ui_input(&mut world);

    assert_eq!(
        world.resource::<UiFocus>().state(window_a).unwrap().focused,
        None
    );
    assert_eq!(
        world.resource::<UiFocus>().state(window_a).unwrap().hovered,
        Some(root_a)
    );
    assert_eq!(
        world
            .resource::<UiFocus>()
            .state(window_a)
            .unwrap()
            .captured,
        None
    );
    assert!(is_interactable_in_window(&world, button, window_b));
    assert!(!is_interactable_in_window(&world, button, window_a));
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerCancel {
        window: window_a,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window: window_a,
        target: button,
        button: UiPointerButton::Middle,
        position: [15.0, 15.0],
        cancelled: true,
    }));
    assert!(events.contains(&UiRoutedEvent::FocusChanged {
        window: window_a,
        previous: Some(button),
        current: None,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerLeave {
        window: window_a,
        target: button,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerEnter {
        window: window_a,
        target: root_a,
    }));
    assert!(!events.iter().any(
        |event| matches!(event, UiRoutedEvent::PointerCancel { window, .. } if *window == window_b)
    ));
}

#[test]
fn window_blur_cancels_only_that_windows_capture_and_focus() {
    let (mut world, window_a, _root_a, button_a) = setup();
    let window_b = world.spawn_empty().id();
    let button_b = world
        .spawn((
            UiNode,
            UiFocusable,
            UiHitBox {
                window: Some(window_b),
                rect: UiRect::new(10.0, 10.0, 20.0, 20.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    world.resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window: window_a,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 3.0,
        },
        UiInputEvent::PointerButton {
            window: window_b,
            button: UiPointerButton::Secondary,
            pressed: true,
            position: [15.0, 15.0],
            time_seconds: 3.0,
        },
    ]);
    process_ui_input(&mut world);
    world.resource_mut::<UiRoutedEvents>().clear();

    world
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::WindowBlur { window: window_a });
    process_ui_input(&mut world);

    assert_eq!(
        world
            .resource::<UiFocus>()
            .state(window_a)
            .unwrap()
            .captured,
        None
    );
    assert_eq!(
        world.resource::<UiFocus>().state(window_a).unwrap().focused,
        None
    );
    assert_eq!(
        world
            .resource::<UiFocus>()
            .state(window_b)
            .unwrap()
            .captured,
        Some(button_b)
    );
    assert_eq!(
        world.resource::<UiFocus>().state(window_b).unwrap().focused,
        Some(button_b)
    );
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerCancel {
        window: window_a,
        target: button_a,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window: window_a,
        target: button_a,
        button: UiPointerButton::Primary,
        position: [15.0, 15.0],
        cancelled: true,
    }));
    assert!(!events.iter().any(
        |event| matches!(event, UiRoutedEvent::PointerCancel { window, .. } if *window == window_b)
    ));
}

#[test]
fn cleanup_cancellation_events_survive_until_the_next_routing_pass() {
    let (mut world, window, root, button) = setup();
    {
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        state.focused = Some(button);
        state.hovered = Some(button);
        state.captured = Some(button);
        state.captured_button = Some(UiPointerButton::Middle);
        state.pointer_position = Some([18.0, 17.0]);
        focus.set_active(window);
    }
    cleanup_removed_node(&mut world, button);
    assert!(world.resource::<UiRoutedEvents>().events.is_empty());
    process_ui_input(&mut world);
    assert!(world.get::<UiChildren>(root).unwrap().0.is_empty());
    let events = &world.resource::<UiRoutedEvents>().events;
    assert!(events.contains(&UiRoutedEvent::PointerCancel {
        window,
        target: button
    }));
    assert!(events.contains(&UiRoutedEvent::PointerUp {
        window,
        target: button,
        button: UiPointerButton::Middle,
        position: [18.0, 17.0],
        cancelled: true,
    }));
    assert!(events.contains(&UiRoutedEvent::PointerLeave {
        window,
        target: button
    }));
    assert!(events.contains(&UiRoutedEvent::FocusChanged {
        window,
        previous: Some(button),
        current: None,
    }));
}

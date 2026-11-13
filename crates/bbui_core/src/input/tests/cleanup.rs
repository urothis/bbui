use super::*;

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

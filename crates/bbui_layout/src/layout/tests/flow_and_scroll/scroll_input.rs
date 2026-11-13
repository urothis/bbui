use super::*;

#[test]
pub(super) fn pointer_routing_hits_scrolled_descendants_and_rejects_clipped_regions() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let scroll = app
        .world_mut()
        .spawn((
            UiNode,
            UiScroll::at(0.0, 40.0),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(30.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let spacer = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(40.0)),
        ))
        .id();
    let visible_button = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
        ))
        .id();
    let clipped_button = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
        ))
        .id();
    set_parent(app.world_mut(), scroll, root).unwrap();
    set_parent(app.world_mut(), spacer, scroll).unwrap();
    set_parent(app.world_mut(), visible_button, scroll).unwrap();
    set_parent(app.world_mut(), clipped_button, scroll).unwrap();
    app.update();

    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(visible_button)
            .unwrap()
            .rect,
        Rect::new(0.0, 0.0, 80.0, 20.0)
    );
    let clipped_bounds = app.world().get::<UiHitBox>(clipped_button).unwrap();
    assert!(clipped_bounds.rect.contains(10.0, 35.0));
    assert!(!clipped_bounds.contains([10.0, 35.0]));

    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [10.0, 10.0],
            time_seconds: 0.0,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: [10.0, 10.0],
            time_seconds: 0.1,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [10.0, 35.0],
            time_seconds: 0.2,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: [10.0, 35.0],
            time_seconds: 0.3,
        },
    ]);
    app.update();
    let clicks: Vec<_> = app
        .world()
        .resource::<UiRoutedEvents>()
        .events
        .iter()
        .filter_map(|event| match event {
            UiRoutedEvent::Click { target, .. } => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(clicks.first(), Some(&visible_button));
    assert_eq!(clicks.len(), 2);
    assert!(!clicks.contains(&clipped_button));
}

#[test]
pub(super) fn routed_wheel_and_keyboard_navigation_update_layout_in_the_same_frame() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin))
        .init_resource::<ScrollPhaseObservation>()
        .add_systems(Update, observe_text_phase.in_set(UiSystems::Text))
        .add_systems(Update, observe_widget_phase.in_set(UiSystems::Widgets));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let scroll = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiScroll::new(),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(40.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let children: Vec<_> = (0..6)
        .map(|index| {
            let mut entity = app.world_mut().spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            ));
            if index == 1 {
                entity.insert(UiFocusable);
            }
            entity.id()
        })
        .collect();
    set_parent(app.world_mut(), scroll, root).unwrap();
    for child in &children {
        set_parent(app.world_mut(), *child, scroll).unwrap();
    }
    app.world_mut()
        .resource_mut::<ScrollPhaseObservation>()
        .target = Some(children[0]);
    app.update();

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window,
            position: [10.0, 10.0],
            delta: [0.0, -1.0],
            unit: UiWheelUnit::Lines,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 40.0]
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[0])
            .unwrap()
            .rect
            .y,
        -40.0
    );
    assert_eq!(
        app.world().get::<UiHitBox>(children[0]).unwrap().clip,
        Some(Rect::new(0.0, 0.0, 80.0, 40.0))
    );
    let same_frame = app.world().resource::<ScrollPhaseObservation>();
    assert_eq!(same_frame.text_rect.unwrap().y, -40.0);
    assert_eq!(same_frame.widget_hit_box.unwrap().rect.y, -40.0);

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(children[3]); // Visible after the first 40 px scroll.
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::PageDown,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 80.0]
    );
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(children[5]); // Visible at the end of the scroll range.
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::PageUp,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 40.0]
    );

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(scroll);
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::End,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 80.0]
    );
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Home,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 0.0]
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(children[0])
            .unwrap()
            .rect
            .y,
        0.0
    );
}

#[test]
pub(super) fn wheel_delta_propagates_from_inner_scroll_at_its_boundary_to_parent() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 120.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let outer = app
        .world_mut()
        .spawn((
            UiNode,
            UiScroll::new(),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(100.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let inner = app
        .world_mut()
        .spawn((
            UiNode,
            UiScroll::at(0.0, 40.0),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(50.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let inner_children: Vec<_> = (0..2)
        .map(|_| {
            app.world_mut()
                .spawn((
                    UiNode,
                    UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(50.0)),
                ))
                .id()
        })
        .collect();
    let outer_filler = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(100.0)),
        ))
        .id();
    set_parent(app.world_mut(), outer, root).unwrap();
    set_parent(app.world_mut(), inner, outer).unwrap();
    for child in &inner_children {
        set_parent(app.world_mut(), *child, inner).unwrap();
    }
    set_parent(app.world_mut(), outer_filler, outer).unwrap();
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(inner).unwrap().offset,
        [0.0, 40.0]
    );
    assert_eq!(
        app.world().get::<UiScroll>(outer).unwrap().offset,
        [0.0, 0.0]
    );

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window,
            position: [10.0, 5.0],
            delta: [0.0, -1.0],
            unit: UiWheelUnit::Lines,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(inner).unwrap().offset,
        [0.0, 50.0]
    );
    assert_eq!(
        app.world().get::<UiScroll>(outer).unwrap().offset,
        [0.0, 30.0]
    );
    assert_eq!(
        app.world()
            .get::<UiLayoutResult>(inner_children[1])
            .unwrap()
            .rect
            .y,
        -30.0
    );

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window,
            position: [10.0, 5.0],
            delta: [0.0, -1.0],
            unit: UiWheelUnit::Lines,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(inner).unwrap().offset,
        [0.0, 50.0]
    );
    assert_eq!(
        app.world().get::<UiScroll>(outer).unwrap().offset,
        [0.0, 50.0]
    );
}

use super::*;
use accesskit::{Action, ActionRequest as AccessKitActionRequest, NodeId, Role, TreeId};
use bevy::a11y::{AccessibilityNode, ActionRequest};

#[test]
fn chrome_uses_shared_layout_and_pointer_drag_moves_pane() {
    let mut app = app();
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "split".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![1.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new("left", vec!["a".into()])),
            DockNode::Tabs(TabGroup::new("right", vec!["b".into()])),
        ],
    });
    app.insert_resource(Workspace::new(layout).unwrap());
    app.update();
    let tab = keyed(app.world_mut(), "ws:tab:left:a");
    let right = keyed(app.world_mut(), "ws:group:right");
    let tab_rect = app.world().get::<UiLayoutResult>(tab).unwrap().rect;
    let right_rect = app.world().get::<UiLayoutResult>(right).unwrap().rect;
    assert!(tab_rect.width > 0.0 && right_rect.x > tab_rect.x);
    assert!(app.world().resource::<DisplayList>().widget_items().iter().any(|item| {
        matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text.contains("Missing: a"))
    }));
    let window = app.world().resource::<Workspace>().windows["main"];
    let start = center(&app, tab);
    let destination = center(&app, right);
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window,
            position: start,
            time_seconds: 0.0,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 0.1,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window,
            position: destination,
            time_seconds: 0.2,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: destination,
            time_seconds: 0.3,
        },
    );
    assert!(
        app.world()
            .resource::<Workspace>()
            .layout
            .group("right")
            .unwrap()
            .panes
            .contains(&PaneId::from("a"))
    );
}

#[test]
fn pointer_drag_can_dock_into_another_bevy_window() {
    let mut app = app();
    app.insert_resource(
        Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
            "main-group",
            vec!["a".into(), "b".into()],
        ))))
        .unwrap(),
    );
    app.update();
    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Detach {
            pane: "b".into(),
            window: "floating".into(),
            group: "floating-group".into(),
        })
        .unwrap();
    app.update();
    let tab = keyed(app.world_mut(), "ws:tab:main-group:a");
    let destination = keyed(app.world_mut(), "ws:group:floating-group");
    let main_window = app.world().resource::<Workspace>().windows["main"];
    let floating_window = app.world().resource::<Workspace>().windows["floating"];
    let start = center(&app, tab);
    let end = center(&app, destination);
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window: main_window,
            position: start,
            time_seconds: 0.0,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window: main_window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 0.1,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window: floating_window,
            position: end,
            time_seconds: 0.2,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window: floating_window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: end,
            time_seconds: 0.3,
        },
    );
    assert!(
        app.world()
            .resource::<Workspace>()
            .layout
            .group("floating-group")
            .unwrap()
            .panes
            .contains(&PaneId::from("a"))
    );
}

#[test]
fn dropping_on_a_tab_reorders_instead_of_splitting() {
    let mut app = app();
    app.insert_resource(
        Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
            "editors",
            vec!["a".into(), "b".into()],
        ))))
        .unwrap(),
    );
    app.update();
    let a = keyed(app.world_mut(), "ws:tab:editors:a");
    let b = keyed(app.world_mut(), "ws:tab:editors:b");
    let window = app.world().resource::<Workspace>().windows["main"];
    let start = center(&app, a);
    let mut end = center(&app, b);
    end[0] += app.world().get::<UiLayoutResult>(b).unwrap().rect.width * 0.25;
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window,
            position: start,
            time_seconds: 0.0,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 0.1,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window,
            position: end,
            time_seconds: 0.2,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: end,
            time_seconds: 0.3,
        },
    );
    assert_eq!(
        app.world()
            .resource::<Workspace>()
            .layout
            .group("editors")
            .unwrap()
            .panes,
        vec![PaneId::from("b"), PaneId::from("a")]
    );
}

#[test]
fn tabs_and_splitters_expose_and_accept_accessibility_actions() {
    let mut app = app();
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "split".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![1.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new("left", vec!["a".into(), "preview".into()])),
            DockNode::Tabs(TabGroup::new("right", vec!["b".into()])),
        ],
    });
    app.insert_resource(Workspace::new(layout).unwrap());
    app.update();
    let tab = keyed(app.world_mut(), "ws:tab:left:preview");
    let splitter = keyed(app.world_mut(), "ws:splitter:split:0");
    assert_eq!(
        app.world().get::<AccessibilityNode>(tab).unwrap().0.role(),
        Role::Tab
    );
    let split_node = &app.world().get::<AccessibilityNode>(splitter).unwrap().0;
    assert_eq!(split_node.role(), Role::Splitter);
    assert!(split_node.supports_action(Action::Increment));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(tab.to_bits()),
            data: None,
        }));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Increment,
            target_tree: TreeId::ROOT,
            target_node: NodeId(splitter.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Workspace>()
            .layout
            .group("left")
            .unwrap()
            .active,
        Some("preview".into())
    );
    let DockNode::Split { shares, .. } =
        &app.world().resource::<Workspace>().layout.windows[0].root
    else {
        panic!("expected split")
    };
    assert!(shares[0] > 1.0);
}

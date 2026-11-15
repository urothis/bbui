use super::*;
use bbui_core::{UiKeyCode, UiModifiers};
use bbui_widgets::{WidgetEvent, WidgetEventQueue, WorkspaceEvent, WorkspaceRestoreSource};
use bevy::a11y::AccessibilityNode;

fn changes(app: &mut App) -> Vec<WorkspaceEvent> {
    app.world_mut()
        .resource_mut::<WidgetEventQueue>()
        .take()
        .into_iter()
        .filter_map(|event| match event {
            WidgetEvent::Workspace(change) => Some(change),
            _ => None,
        })
        .collect()
}

fn key(app: &mut App, window: Entity, key: UiKeyCode, control: bool) {
    push_pointer(
        app,
        UiInputEvent::Key {
            window,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control,
                ..default()
            },
        },
    );
}

#[test]
fn completed_edits_emit_detailed_events_only_after_a_change() {
    let mut app = app();
    app.insert_resource(
        Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
            "editors",
            vec!["a".into(), "b".into()],
        ))))
        .unwrap(),
    );
    app.update();
    assert!(changes(&mut app).is_empty());

    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Select {
            group: "editors".into(),
            pane: "b".into(),
        })
        .unwrap();
    app.update();
    assert_eq!(
        changes(&mut app),
        vec![WorkspaceEvent::TabSelected {
            group: "editors".into(),
            pane: "b".into(),
        }]
    );

    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Select {
            group: "editors".into(),
            pane: "b".into(),
        })
        .unwrap();
    assert!(
        app.world_mut()
            .resource_mut::<Workspace>()
            .apply(WorkspaceOperation::Move {
                pane: "a".into(),
                group: "missing".into(),
                index: 0,
            })
            .is_err()
    );
    app.update();
    assert!(changes(&mut app).is_empty());

    assert!(app.world_mut().resource_mut::<Workspace>().undo());
    app.update();
    assert_eq!(
        changes(&mut app),
        vec![WorkspaceEvent::LayoutRestored {
            source: WorkspaceRestoreSource::Undo,
        }]
    );

    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Move {
            pane: "a".into(),
            group: "editors".into(),
            index: 2,
        })
        .unwrap();
    app.update();
    assert_eq!(
        changes(&mut app),
        vec![WorkspaceEvent::TabReordered {
            pane: "a".into(),
            group: "editors".into(),
            index: 1,
        }]
    );
}

#[test]
fn keyboard_move_mode_docks_and_reports_the_result() {
    let mut app = app();
    app.insert_resource(
        Workspace::new(WorkspaceLayout::new(DockNode::Split {
            id: "split".into(),
            axis: SplitAxis::Horizontal,
            shares: vec![1.0, 1.0],
            children: vec![
                DockNode::Tabs(TabGroup::new("left", vec!["a".into(), "b".into()])),
                DockNode::Tabs(TabGroup::new("right", vec!["c".into()])),
            ],
        }))
        .unwrap(),
    );
    app.update();
    let tab = keyed(app.world_mut(), "ws:tab:left:a");
    let window = app.world().resource::<Workspace>().window.unwrap();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(tab);

    key(&mut app, window, UiKeyCode::Space, true);
    assert!(changes(&mut app).is_empty());
    let description = &app.world().get::<AccessibilityNode>(tab).unwrap().0;
    assert!(description.description().is_some());
    key(&mut app, window, UiKeyCode::ArrowRight, false);
    key(&mut app, window, UiKeyCode::Enter, false);
    assert!(
        app.world()
            .resource::<Workspace>()
            .layout
            .group("right")
            .unwrap()
            .panes
            .contains(&PaneId::from("a"))
    );
    assert_eq!(
        changes(&mut app),
        vec![WorkspaceEvent::PaneMoved {
            pane: "a".into(),
            from: "left".into(),
            to: "right".into(),
            index: 1,
        }]
    );
    app.update();
    let moved_tab = keyed(app.world_mut(), "ws:tab:right:a");
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(moved_tab)
    );

    key(&mut app, window, UiKeyCode::Space, true);
    key(&mut app, window, UiKeyCode::Escape, false);
    assert!(changes(&mut app).is_empty());
}

#[test]
fn keyboard_move_mode_can_create_a_split_or_cancel() {
    let mut app = app();
    app.insert_resource(
        Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
            "editors",
            vec!["a".into(), "b".into()],
        ))))
        .unwrap(),
    );
    app.update();
    let tab = keyed(app.world_mut(), "ws:tab:editors:a");
    let window = app.world().resource::<Workspace>().window.unwrap();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(tab);
    key(&mut app, window, UiKeyCode::Space, true);
    key(&mut app, window, UiKeyCode::Escape, false);
    assert!(changes(&mut app).is_empty());
    assert!(matches!(
        app.world().resource::<Workspace>().layout.root,
        DockNode::Tabs(_)
    ));

    key(&mut app, window, UiKeyCode::Space, true);
    key(&mut app, window, UiKeyCode::ArrowRight, false);
    key(&mut app, window, UiKeyCode::Enter, false);
    assert!(matches!(
        app.world().resource::<Workspace>().layout.root,
        DockNode::Split { .. }
    ));
    let events = changes(&mut app);
    assert!(matches!(events.as_slice(), [WorkspaceEvent::PaneDocked {
        pane, target, axis: SplitAxis::Horizontal, after: false, ..
    }] if pane == &PaneId::from("a") && target == "editors"));
}

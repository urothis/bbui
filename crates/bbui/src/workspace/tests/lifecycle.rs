use super::*;

#[test]
fn explicit_close_respects_host_guard() {
    let mut workspace = Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
        "main-group",
        vec!["a".into()],
    ))))
    .unwrap();
    workspace.register_pane("a", "A", |_, _| {});
    assert!(workspace.set_close_guard(&PaneId::from("a"), || false));
    assert_eq!(
        workspace.apply(WorkspaceOperation::Close { pane: "a".into() }),
        Err(WorkspaceError::DropRejected)
    );
    assert_eq!(
        workspace.layout().group("main-group").unwrap().panes,
        vec![PaneId::from("a")]
    );
    assert!(workspace.set_close_guard(&PaneId::from("a"), || true));
    assert!(
        workspace
            .apply(WorkspaceOperation::Close { pane: "a".into() })
            .is_ok()
    );
}

#[test]
fn moving_preserves_live_content_while_closing_rebuilds_it() {
    let mut app = app();
    let count = Arc::new(AtomicUsize::new(0));
    let mut workspace = Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
        "main-group",
        vec!["a".into(), "b".into()],
    ))))
    .unwrap();
    let built = count.clone();
    workspace.register_pane("a", "A", move |world, parent| {
        built.fetch_add(1, Ordering::SeqCst);
        WidgetTree::new(WidgetNode::label("Content A"))
            .spawn(world, Some(parent))
            .unwrap();
    });
    workspace.register_pane("b", "B", |world, parent| {
        WidgetTree::new(WidgetNode::label("Content B"))
            .spawn(world, Some(parent))
            .unwrap();
    });
    app.insert_resource(workspace);
    app.update();
    let original = app.world().resource::<Workspace>().live_panes[&PaneId::from("a")];
    assert_eq!(count.load(Ordering::SeqCst), 1);
    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Split {
            pane: "a".into(),
            target: "main-group".into(),
            id: "split".into(),
            group: "second".into(),
            axis: SplitAxis::Horizontal,
            after: true,
        })
        .unwrap();
    app.update();
    assert_eq!(
        app.world().resource::<Workspace>().live_panes[&PaneId::from("a")],
        original
    );
    let host = keyed(app.world_mut(), "ws:content:second");
    assert!(
        app.world()
            .get::<UiChildren>(host)
            .unwrap()
            .0
            .contains(&original)
    );
    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Close { pane: "a".into() })
        .unwrap();
    app.update();
    assert!(app.world().get_entity(original).is_err());
    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Open {
            pane: "a".into(),
            group: "main-group".into(),
            index: 0,
        })
        .unwrap();
    app.update();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn detached_window_is_real_and_closing_it_removes_its_panes() {
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
    let detached = app.world().resource::<Workspace>().windows["floating"];
    assert!(app.world().get::<Window>(detached).is_some());
    app.world_mut().despawn(detached);
    app.update();
    assert!(
        app.world()
            .resource::<Workspace>()
            .layout
            .group("floating-group")
            .is_none()
    );
    assert!(
        !app.world()
            .resource::<Workspace>()
            .live_panes
            .contains_key(&PaneId::from("b"))
    );
}

#[test]
fn moving_focused_pane_to_another_window_transfers_focus() {
    let mut app = app();
    let mut workspace = Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
        "main-group",
        vec!["a".into(), "b".into()],
    ))))
    .unwrap();
    workspace.register_pane("a", "A", |world, parent| {
        WidgetTree::new(WidgetNode::text_field("Name", "draft"))
            .spawn(world, Some(parent))
            .unwrap();
    });
    app.insert_resource(workspace);
    app.update();
    let original = app.world().resource::<Workspace>().live_panes[&PaneId::from("a")];
    let field = app.world().get::<UiChildren>(original).unwrap().0[0];
    let main = app.world().resource::<Workspace>().windows["main"];
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(main)
        .focused = Some(field);
    app.world_mut()
        .resource_mut::<Workspace>()
        .apply(WorkspaceOperation::Detach {
            pane: "a".into(),
            window: "floating".into(),
            group: "floating-group".into(),
        })
        .unwrap();
    app.update();
    let detached = app.world().resource::<Workspace>().windows["floating"];
    assert_eq!(
        app.world().resource::<Workspace>().live_panes[&PaneId::from("a")],
        original
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(detached)
            .unwrap()
            .focused,
        Some(field)
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(main)
            .unwrap()
            .focused,
        None
    );
}

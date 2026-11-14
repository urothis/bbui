use super::*;

#[test]
fn move_split_detach_and_close_validate() {
    let mut layout = WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
        "a",
        vec!["one".into(), "two".into()],
    )));
    layout
        .apply(WorkspaceOperation::Split {
            pane: "two".into(),
            target: "a".into(),
            id: "s".into(),
            group: "b".into(),
            axis: SplitAxis::Horizontal,
            after: true,
        })
        .unwrap();
    layout
        .apply(WorkspaceOperation::Detach {
            pane: "two".into(),
            window: "floating".into(),
            group: "c".into(),
        })
        .unwrap();
    assert_eq!(layout.windows.len(), 2);
    layout
        .apply(WorkspaceOperation::CloseWindow {
            window: "floating".into(),
        })
        .unwrap();
    assert_eq!(layout.windows.len(), 1);
    layout.validate().unwrap();
}

#[test]
fn rejected_transaction_does_not_change_layout() {
    let mut layout = WorkspaceLayout::new(DockNode::Tabs(TabGroup::new("a", vec!["one".into()])));
    let before = layout.clone();
    assert!(
        layout
            .apply(WorkspaceOperation::Move {
                pane: "one".into(),
                group: "missing".into(),
                index: 0
            })
            .is_err()
    );
    assert_eq!(layout, before);
}

#[test]
fn round_trip() {
    let layout = WorkspaceLayout::new(DockNode::Tabs(TabGroup::new("a", vec!["one".into()])));
    let json = serde_json::to_string(&layout).unwrap();
    let restored: WorkspaceLayout = serde_json::from_str(&json).unwrap();
    assert_eq!(layout, restored);
}

#[test]
fn closed_pane_can_reopen_when_group_rejects_drag_drops() {
    let mut group = TabGroup::new("editors", vec!["scene".into()]);
    group.options.accepts_drops = false;
    let mut layout = WorkspaceLayout::new(DockNode::Tabs(group));
    layout
        .apply(WorkspaceOperation::Close {
            pane: "scene".into(),
        })
        .unwrap();
    layout
        .apply(WorkspaceOperation::Open {
            pane: "scene".into(),
            group: "editors".into(),
            index: 0,
        })
        .unwrap();
    assert_eq!(
        layout.group("editors").unwrap().active,
        Some("scene".into())
    );
}

#[test]
fn last_pane_closes_and_reopens_from_an_empty_main_workspace() {
    let mut layout = WorkspaceLayout::new(DockNode::Split {
        id: "root-split".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![1.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new("left", vec!["a".into()])),
            DockNode::Tabs(TabGroup::new("right", vec!["b".into()])),
        ],
    });
    layout
        .apply(WorkspaceOperation::Close { pane: "a".into() })
        .unwrap();
    layout
        .apply(WorkspaceOperation::Close { pane: "b".into() })
        .unwrap();
    assert!(matches!(layout.windows[0].root, DockNode::Tabs(_)));
    let DockNode::Tabs(group) = &layout.windows[0].root else {
        return;
    };
    let empty_group = group.id.clone();
    assert!(group.panes.is_empty());
    layout
        .apply(WorkspaceOperation::Open {
            pane: "a".into(),
            group: empty_group.clone(),
            index: 0,
        })
        .unwrap();
    assert_eq!(layout.group(&empty_group).unwrap().active, Some("a".into()));
}

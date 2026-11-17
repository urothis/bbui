use super::*;
use bbui_core::UiParent;

fn verify_splitter_preview(axis: SplitAxis) {
    let mut app = app();
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "split".into(),
        axis,
        shares: vec![1.0, 1.0, 1.0],
        children: ["a", "b", "c"]
            .into_iter()
            .map(|pane| DockNode::Tabs(TabGroup::new(format!("group-{pane}"), vec![pane.into()])))
            .collect(),
    });
    let original_layout = layout.clone();
    let mut workspace = Workspace::new(layout).unwrap();
    for pane in ["a", "b", "c"] {
        workspace.register_pane(pane, pane, |world, parent| {
            WidgetTree::new(WidgetNode::label("Live pane"))
                .spawn(world, Some(parent))
                .unwrap();
        });
    }
    app.insert_resource(workspace);
    for _ in 0..3 {
        app.update();
    }

    let first_group = keyed(app.world_mut(), "ws:group:group-a");
    let splitter = keyed(app.world_mut(), "ws:splitter:split:0");
    let extent = |app: &App| {
        let rect = app.world().get::<UiLayoutResult>(first_group).unwrap().rect;
        match axis {
            SplitAxis::Horizontal => rect.width,
            SplitAxis::Vertical => rect.height,
        }
    };
    let original_extent = extent(&app);
    let original_fraction = app
        .world()
        .get::<WorkspaceSplitterSemantic>(splitter)
        .unwrap()
        .fraction;
    let originals: Vec<_> = app
        .world()
        .resource::<Workspace>()
        .live_panes
        .values()
        .map(|entity| (*entity, app.world().get::<UiParent>(*entity).unwrap().0))
        .collect();
    let window = app.world().resource::<Workspace>().window.unwrap();
    let start = center(&app, splitter);
    let moved = match axis {
        SplitAxis::Horizontal => [start[0] + 20.0, start[1]],
        SplitAxis::Vertical => [start[0], start[1] + 20.0],
    };
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
            time_seconds: 0.01,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window,
            position: moved,
            time_seconds: 0.02,
        },
    );
    assert!(!app.world().resource::<Workspace>().dirty);
    app.update(); // Apply the changed inline share through style and layout.

    assert!(extent(&app) > original_extent + 10.0);
    assert!(
        app.world()
            .get::<WorkspaceSplitterSemantic>(splitter)
            .unwrap()
            .fraction
            > original_fraction
    );
    for (entity, parent) in originals {
        assert_eq!(app.world().get::<UiParent>(entity).unwrap().0, parent);
    }

    let mut appearance = app.world().resource::<Workspace>().appearance().clone();
    appearance.drop_preview_alpha = 0.2;
    app.world_mut()
        .resource_mut::<Workspace>()
        .set_appearance(appearance);
    app.update();
    assert!(extent(&app) > original_extent + 10.0);

    app.world_mut()
        .resource_mut::<Workspace>()
        .cancel_preview(original_layout);
    app.update();
    assert!((extent(&app) - original_extent).abs() < 0.1);
    assert_eq!(
        app.world()
            .get::<WorkspaceSplitterSemantic>(splitter)
            .unwrap()
            .fraction,
        original_fraction
    );
}

#[test]
fn horizontal_splitter_preview_resizes_live_geometry_without_remounting_panes() {
    verify_splitter_preview(SplitAxis::Horizontal);
}

#[test]
fn vertical_splitter_preview_resizes_live_geometry_without_remounting_panes() {
    verify_splitter_preview(SplitAxis::Vertical);
}

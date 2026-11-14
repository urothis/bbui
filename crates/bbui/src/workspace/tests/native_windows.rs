use super::*;
use bevy::input::{
    ButtonState,
    mouse::{MouseButton, MouseButtonInput},
};

#[test]
fn native_release_outside_window_detaches_tab() {
    let mut app = app();
    app.insert_resource(
        Workspace::new(WorkspaceLayout::new(DockNode::Tabs(TabGroup::new(
            "main-group",
            vec!["a".into(), "b".into()],
        ))))
        .unwrap(),
    );
    app.update();
    let tab = keyed(app.world_mut(), "ws:tab:main-group:a");
    let window = app.world().resource::<Workspace>().windows["main"];
    let start = center(&app, tab);
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
            position: [start[0] + 50.0, start[1] + 50.0],
            time_seconds: 0.2,
        },
    );
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window,
    });
    app.update();
    assert_eq!(
        app.world().resource::<Workspace>().layout().windows.len(),
        2
    );
}

#[test]
fn native_source_release_docks_into_hovered_window() {
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
    let source = app.world().resource::<Workspace>().windows["main"];
    let floating = app.world().resource::<Workspace>().windows["floating"];
    let start = center(&app, tab);
    let end = center(&app, destination);
    push_pointer(
        &mut app,
        UiInputEvent::PointerButton {
            window: source,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 0.1,
        },
    );
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window: source,
            position: [start[0] + 50.0, start[1] + 50.0],
            time_seconds: 0.2,
        },
    );
    app.world_mut()
        .get_mut::<Window>(floating)
        .unwrap()
        .set_cursor_position(Some(Vec2::from_array(end)));
    push_pointer(
        &mut app,
        UiInputEvent::PointerMove {
            window: floating,
            position: end,
            time_seconds: 0.3,
        },
    );
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window: source,
    });
    app.update();
    let workspace = app.world().resource::<Workspace>();
    assert_eq!(workspace.layout().windows.len(), 2);
    assert!(
        workspace
            .layout()
            .group("floating-group")
            .unwrap()
            .panes
            .contains(&PaneId::from("a"))
    );
}

use bevy_app::App;
use bui_core::{
    CorePlugin, UiFocus, UiFocusable, UiInputEvent, UiInputQueue, UiKeyCode, UiModifiers, UiNode,
    UiWheelUnit, set_parent,
};
use bui_layout::{
    LayoutPlugin, Rect, UiAlign, UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot,
    UiLength, UiScroll, UiSize,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));

    let surface = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(surface, Rect::new(0.0, 0.0, 320.0, 200.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let viewport = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiScroll::new(),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(180.0),
                    height: UiLength::Px(100.0),
                },
                gap: 4.0,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    set_parent(app.world_mut(), viewport, root).expect("valid viewport parent");

    let mut rows = Vec::new();
    for _ in 0..8 {
        let row = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(180.0), UiLength::Px(28.0)),
            ))
            .id();
        set_parent(app.world_mut(), row, viewport).expect("valid row parent");
        rows.push(row);
    }

    app.update();
    println!(
        "initial: viewport={:?}, row 3={:?}",
        app.world().get::<UiLayoutResult>(viewport).unwrap().rect,
        app.world().get::<UiLayoutResult>(rows[3]).unwrap().rect,
    );

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window: surface,
            position: [20.0, 20.0],
            delta: [0.0, -2.0],
            unit: UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        });
    app.update();
    println!(
        "wheel: offset={:?}, row 3={:?}, clip={:?}",
        app.world().get::<UiScroll>(viewport).unwrap().offset,
        app.world().get::<UiLayoutResult>(rows[3]).unwrap().rect,
        app.world().get::<UiLayoutResult>(rows[3]).unwrap().clip,
    );

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(surface)
        .focused = Some(viewport);
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window: surface,
            key: UiKeyCode::PageDown,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });
    app.update();
    println!(
        "PageDown: offset={:?}, row 3={:?}",
        app.world().get::<UiScroll>(viewport).unwrap().offset,
        app.world().get::<UiLayoutResult>(rows[3]).unwrap().rect,
    );
}

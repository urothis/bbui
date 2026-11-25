//! CPU cost of modal input gating and confirmation lifecycle.

use bbui::{
    BbuiPlugin,
    modal::{ModalAction, ModalDialog, ModalDialogs, dismiss_dialog},
};
use bbui_core::{
    CorePlugin, UiFocus, UiFocusable, UiHitBox, UiInputEvent, UiInputQueue, UiKey, UiKeyCode,
    UiModalScopes, UiModifiers, UiNode, UiRect, set_parent,
};
use bbui_layout::{Rect, UiLayoutRoot, UiLength};
use bbui_widgets::{WidgetNode, WidgetTree};
use bevy::{
    asset::AssetPlugin,
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    prelude::{App, Entity, ImagePlugin, MinimalPlugins, Window},
    window::{WindowPlugin, WindowResolution},
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

const INPUT_CONTROLS: usize = 10_000;

fn input_scene(modal_open: bool) -> Result<(App, Entity), String> {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    let window = app.world_mut().spawn_empty().id();
    for index in 0..INPUT_CONTROLS {
        let column = (index % 100) as f32;
        let row = (index / 100) as f32;
        app.world_mut().spawn((
            UiNode,
            UiFocusable,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(column * 24.0, row * 24.0, 20.0, 20.0),
                clip: None,
                z_index: 0,
            },
        ));
    }
    let root = app.world_mut().spawn(UiNode).id();
    let action = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 20.0, 20.0),
                clip: None,
                z_index: 10_000,
            },
        ))
        .id();
    set_parent(app.world_mut(), action, root)
        .map_err(|error| format!("modal input benchmark hierarchy: {error:?}"))?;
    if modal_open {
        app.world_mut().init_resource::<UiModalScopes>();
        app.world_mut()
            .resource_mut::<UiModalScopes>()
            .0
            .insert(window, root);
    }
    app.update();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position: [36.0, 12.0],
            time_seconds: 0.0,
        });
    app.update();
    let hovered = app
        .world()
        .resource::<UiFocus>()
        .state(window)
        .and_then(|state| state.hovered);
    if hovered.is_some() == modal_open {
        return Err("modal input benchmark background pointer routing is incorrect".to_owned());
    }
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });
    app.update();
    let focused = app
        .world()
        .resource::<UiFocus>()
        .state(window)
        .and_then(|state| state.focused);
    if modal_open && focused != Some(action) || !modal_open && focused.is_none() {
        return Err("modal input benchmark focus routing is incorrect".to_owned());
    }
    Ok((app, window))
}

fn pointer_move(app: &mut App, window: Entity, first: bool) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position: if first { [12.0, 12.0] } else { [36.0, 12.0] },
            time_seconds: 0.0,
        });
    app.update();
    black_box(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .and_then(|state| state.hovered),
    );
}

fn tab(app: &mut App, window: Entity) {
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::Tab,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        });
    app.update();
    black_box(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .and_then(|state| state.focused),
    );
}

fn lifecycle_scene(controls: usize) -> Result<(App, Entity, Entity), String> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BbuiPlugin);
    let window = app
        .world_mut()
        .spawn(Window {
            resolution: WindowResolution::new(1280, 720),
            ..Default::default()
        })
        .id();
    let page = WidgetTree::new(
        WidgetNode::column((0..controls).map(|index| {
            WidgetNode::button(format!("Control {index}"))
                .with_key(format!("bench:background:{index}"))
        }))
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
    .spawn(app.world_mut(), None)
    .map_err(|error| format!("modal lifecycle benchmark page: {error:?}"))?;
    app.world_mut().entity_mut(page).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 1280.0, 720.0),
        1.0,
    ));
    for _ in 0..3 {
        app.update();
    }
    let first = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &UiKey)>();
        query
            .iter(world)
            .find_map(|(entity, key)| (key.0 == "bench:background:0").then_some(entity))
    }
    .ok_or_else(|| "modal lifecycle benchmark first control is missing".to_owned())?;
    Ok((app, window, first))
}

fn open_and_close(app: &mut App, window: Entity, background: Entity) {
    let result = ModalDialog::new(
        "Confirm action",
        "Proceed with the action?",
        vec![
            ModalAction::new("confirm", "Confirm"),
            ModalAction::new("cancel", "Cancel"),
        ],
    )
    .show(app.world_mut(), window);
    assert!(
        result.is_ok(),
        "modal lifecycle benchmark could not open dialog"
    );
    app.update();
    assert!(app.world().resource::<ModalDialogs>().is_open(window));
    assert!(
        app.world()
            .get::<bevy::a11y::AccessibilityNode>(background)
            .is_some_and(|node| node.0.is_hidden()),
        "modal lifecycle benchmark did not hide background accessibility"
    );
    assert!(dismiss_dialog(app.world_mut(), window));
    app.update();
    assert!(!app.world().resource::<ModalDialogs>().is_open(window));
    assert!(
        app.world()
            .get::<bevy::a11y::AccessibilityNode>(background)
            .is_some_and(|node| !node.0.is_hidden()),
        "modal lifecycle benchmark did not restore background accessibility"
    );
    black_box(app.world().resource::<ModalDialogs>().is_open(window));
}

pub fn bench(c: &mut Criterion) {
    let mut input = c.benchmark_group("modal/input_scope");
    input.throughput(Throughput::Elements(INPUT_CONTROLS as u64));
    for (name, modal_open) in [("closed", false), ("open", true)] {
        let Ok((mut pointer, window)) = input_scene(modal_open) else {
            eprintln!("failed to create {name} modal pointer benchmark scene");
            continue;
        };
        let mut first = false;
        input.bench_function(format!("pointer/{name}/10000"), |b| {
            b.iter(|| {
                first = !first;
                pointer_move(&mut pointer, window, first);
            });
        });

        let Ok((mut keyboard, window)) = input_scene(modal_open) else {
            eprintln!("failed to create {name} modal keyboard benchmark scene");
            continue;
        };
        input.bench_function(format!("tab/{name}/10000"), |b| {
            b.iter(|| tab(&mut keyboard, window));
        });
    }
    input.finish();

    let mut lifecycle = c.benchmark_group("modal/open_close");
    for controls in [100, 1_000] {
        lifecycle.throughput(Throughput::Elements(controls as u64));
        let Ok((mut app, window, background)) = lifecycle_scene(controls) else {
            eprintln!("failed to create {controls}-control modal lifecycle benchmark scene");
            continue;
        };
        lifecycle.bench_with_input(
            BenchmarkId::new("accessibility", controls),
            &controls,
            |b, _| {
                b.iter(|| open_and_close(&mut app, window, background));
            },
        );
    }
    lifecycle.finish();
}

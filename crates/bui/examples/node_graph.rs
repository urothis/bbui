//! Run with `cargo run -p bui --example node_graph`.
//! Drag node headers to move nodes; drag output sockets to inputs to connect.
//! Drag an existing socket to reconnect it, Alt-drag across links to cut them,
//! and use middle-drag/wheel to pan and zoom. Ctrl+A, arrows, C, N, Delete,
//! Ctrl+Z, and Ctrl+Y edit the graph.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{NodeCategory, NodeSocket, SocketType, WidgetEventQueue, WidgetNode};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn sample_graph() -> Result<NodeGraph, String> {
    let mut graph = NodeGraph::default();
    let noise = graph.add_node_with_category(
        "Noise Texture",
        [30.0, 45.0],
        vec![NodeSocket::output("Factor", SocketType::Float)],
        NodeCategory::Texture,
    );
    let color = graph.add_node_with_category(
        "Color Ramp",
        [30.0, 260.0],
        vec![
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Color,
    );
    let mix = graph.add_node_with_category(
        "Mix",
        [400.0, 150.0],
        vec![
            NodeSocket::input("A", SocketType::Color),
            NodeSocket::input("B", SocketType::Color),
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Converter,
    );
    let output = graph.add_node_with_category(
        "Material Output",
        [810.0, 150.0],
        vec![NodeSocket::input("Surface", SocketType::Color)],
        NodeCategory::Output,
    );
    for (from, from_socket, to, to_socket) in [
        (noise, 0, color, 0),
        (color, 1, mix, 0),
        (color, 1, mix, 1),
        (noise, 0, mix, 2),
        (mix, 3, output, 0),
    ] {
        graph
            .create_link(from, from_socket, to, to_socket)
            .map_err(|error| format!("sample graph link: {error:?}"))?;
    }
    // Center the sample network inside the editor viewport while keeping its model coordinates
    // independent from the current window and layout size.
    graph.pan = [80.0, 105.0];
    Ok(graph)
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("NodeGraph example requires a primary window");
        return;
    };
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 8.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 820.0), 1.0),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: background,
            }),
        ))
        .id();

    let Ok(sample_graph) = sample_graph() else {
        eprintln!("failed to build the sample node graph");
        return;
    };
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("NodeGraph · typed sockets · stable links"),
        WidgetNode::node_graph("Material graph", sample_graph).with_key("material-graph"),
        WidgetNode::label(
            "Drag headers to move · drag sockets to connect or reconnect · Alt-drag cuts links",
        ),
        WidgetNode::label(
            "Middle-drag pans · Ctrl-wheel zooms · Ctrl+A selects nodes · C collapses",
        ),
        WidgetNode::label(
            "N adds a node · arrows move selection · Delete removes · Ctrl+Z/Y undo/redo",
        ),
    ]));
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
        return;
    }
    let Some(graph) = world
        .iter_entities()
        .find(|entity| {
            entity
                .get::<UiKey>()
                .is_some_and(|key| key.0 == "material-graph")
        })
        .map(|entity| entity.id())
    else {
        eprintln!("sample node graph control is missing");
        return;
    };
    let Some(mut layout) = world.get_mut::<UiLayoutNode>(graph) else {
        eprintln!("sample node graph layout is missing");
        return;
    };
    layout.size.width = UiLength::Px(1180.0);
    layout.size.height = UiLength::Px(600.0);
    layout.clip_contents = true;
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("NodeGraph: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI NodeGraph Editor".into(),
                resolution: WindowResolution::new(1280, 820),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            log_widget_events.after(bui_core::UiSystems::Widgets),
        )
        .run();
}

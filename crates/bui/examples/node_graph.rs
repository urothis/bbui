//! Run with `cargo run -p bui --example node_graph`.
//! Drag node headers to move nodes; drag output sockets to inputs to connect.
//! Drag an existing socket to reconnect it, Alt-drag across links to cut them,
//! and use middle-drag/wheel to pan and zoom. Ctrl+A, arrows, C, N, Delete,
//! Ctrl+Z, and Ctrl+Y edit the graph.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{NodeSocket, SocketType, WidgetEventQueue, WidgetNode};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};

fn sample_graph() -> NodeGraph {
    let mut graph = NodeGraph::default();
    let noise = graph.add_node(
        "Noise Texture",
        [30.0, 45.0],
        vec![NodeSocket::output("Factor", SocketType::Float)],
    );
    let color = graph.add_node(
        "Color Ramp",
        [30.0, 260.0],
        vec![
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
    );
    let mix = graph.add_node(
        "Mix",
        [400.0, 150.0],
        vec![
            NodeSocket::input("A", SocketType::Color),
            NodeSocket::input("B", SocketType::Color),
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
    );
    let output = graph.add_node(
        "Material Output",
        [810.0, 150.0],
        vec![NodeSocket::input("Surface", SocketType::Color)],
    );
    graph
        .create_link(noise, 0, color, 0)
        .expect("matching Float sockets");
    graph
        .create_link(color, 1, mix, 0)
        .expect("matching Color sockets");
    graph
        .create_link(color, 1, mix, 1)
        .expect("matching Color sockets");
    graph
        .create_link(noise, 0, mix, 2)
        .expect("matching Float sockets");
    graph
        .create_link(mix, 3, output, 0)
        .expect("matching Color sockets");
    graph
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("NodeGraph example requires a primary window");
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

    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("NodeGraph · typed sockets · stable links"),
        WidgetNode::node_graph("Material graph", sample_graph()).with_key("material-graph"),
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
    tree.spawn(world, Some(root))
        .expect("valid NodeGraph declaration");
    let graph = world
        .iter_entities()
        .find(|entity| {
            entity
                .get::<UiKey>()
                .is_some_and(|key| key.0 == "material-graph")
        })
        .expect("keyed graph declaration was spawned")
        .id();
    let mut layout = world.get_mut::<UiLayoutNode>(graph).unwrap();
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

//! Run with `cargo run -p bbui --example node_graph`.
//! Drag node headers to move nodes; drag output sockets to inputs to connect.
//! Drag an existing socket to reconnect it, Alt-drag across links to cut them,
//! and use middle-drag/wheel to pan and zoom. Ctrl+A, arrows, C, N, Delete,
//! Ctrl+Z, and Ctrl+Y edit the graph.

use bbui::prelude::*;
use bbui::widgets::{
    NodeCategory, NodeGraphTypeRegistry, NodeSocket, NodeTypeDefinition, SocketId, SocketType,
    SocketTypeDefinition, WidgetEvent, WidgetEventQueue, WidgetNode, WidgetValue,
};
use bbui_layout::{Rect, UiEdges};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Rgba;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn sample_graph(registry: &NodeGraphTypeRegistry) -> Result<NodeGraph, String> {
    let mut graph = NodeGraph::default();
    let noise = graph.add_node_with_category(
        "Noise Texture",
        [55.0, 185.0],
        vec![NodeSocket::output("Factor", SocketType::Float)],
        NodeCategory::Texture,
    );
    let color = graph.add_node_with_category(
        "Color Ramp",
        [330.0, 165.0],
        vec![
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Color,
    );
    let base_color = graph.add_node_with_category(
        "Base Color",
        [330.0, 390.0],
        vec![NodeSocket::output("Color", SocketType::Color)],
        NodeCategory::Color,
    );
    let mix = graph.add_node_with_category(
        "Mix Color",
        [625.0, 160.0],
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
        [925.0, 270.0],
        vec![NodeSocket::input("Surface", SocketType::Color)],
        NodeCategory::Output,
    );
    let bottom_ports = graph.add_defined_node(
        registry,
        "example:bottom_mix",
        "Bottom Mix",
        [625.0, 285.0],
        serde_json::json!({"clamp": true}),
    )?;
    if let Some(node) = graph.nodes.iter_mut().find(|node| node.id == bottom_ports) {
        node.size = [235.0, 180.0];
    }
    let preview = graph.add_node_with_category(
        "Preview",
        [925.0, 430.0],
        vec![NodeSocket::input("Color", SocketType::Color)],
        NodeCategory::Output,
    );
    for (from, from_socket, to, to_socket) in [
        (noise, 0, color, 0),
        (color, 1, mix, 0),
        (base_color, 0, mix, 1),
        (mix, 3, output, 0),
        (base_color, 0, bottom_ports, 0),
        (bottom_ports, 1, preview, 0),
    ] {
        graph
            .create_link_with_registry(registry, from, from_socket, to, to_socket)
            .map_err(|error| format!("sample graph link: {error:?}"))?;
    }
    // Center the sample network inside the editor viewport while keeping its model coordinates
    // independent from the current window and layout size.
    graph.pan = [35.0, 45.0];
    Ok(graph)
}

fn setup(world: &mut World) {
    {
        let mut registry = world.resource_mut::<NodeGraphTypeRegistry>();
        let tint = SocketType::custom("example:tint");
        if let Err(error) = registry.register(SocketTypeDefinition::new(
            "example:tint",
            Rgba::new(0.48, 0.72, 1.0, 1.0),
        )) {
            eprintln!("sample socket type: {error}");
            return;
        }
        registry.allow_conversion(SocketType::Color, tint.clone());
        registry.allow_conversion(tint.clone(), SocketType::Color);
        if let Err(error) = registry.register_node(
            NodeTypeDefinition::new(
                "example:bottom_mix",
                1,
                NodeCategory::Converter,
                vec![
                    NodeSocket::input("Color in", tint.clone())
                        .with_id(SocketId(1))
                        .bottom_left(),
                    NodeSocket::output("Color out", tint)
                        .with_id(SocketId(2))
                        .bottom_right(),
                    NodeSocket::input("Factor", SocketType::Float)
                        .with_id(SocketId(3))
                        .with_value(serde_json::json!(0.5)),
                ],
                |node| {
                    WidgetNode::column([
                        WidgetNode::label("Custom node body"),
                        WidgetNode::checkbox(
                            "Clamp",
                            node.data
                                .get("clamp")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                        ),
                    ])
                },
            )
            .with_event_handler(|_, event, node| {
                let WidgetEvent::ValueChanged {
                    after: WidgetValue::Bool(clamp),
                    ..
                } = event
                else {
                    return None;
                };
                let mut data = node.data.clone();
                data["clamp"] = (*clamp).into();
                Some(data)
            }),
        ) {
            eprintln!("sample node type: {error}");
            return;
        }
    }
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

    let Ok(sample_graph) = sample_graph(world.resource::<NodeGraphTypeRegistry>()) else {
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
    layout.size.width = UiLength::Px(1232.0);
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
                title: "BBUI NodeGraph Editor".into(),
                resolution: WindowResolution::new(1280, 820),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            log_widget_events.after(bbui_core::UiSystems::Widgets),
        )
        .run();
}

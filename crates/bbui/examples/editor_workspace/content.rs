//! Pane content for the composed editor example.

use bbui::prelude::*;
use bbui::widgets::{NodeCategory, NodeSocket, SocketType, WidgetNode, WidgetTree};
use bbui_layout::{UiEdges, UiLength};
use bbui_style::{Rgba, ThemeKind};
use bevy::prelude::{Entity, World};

pub fn configure_theme(theme: &mut Theme) {
    theme.font_size = 11.5;
    theme.corner_radius = 3.0;
    theme.spacing.control.padding_x = 6.0;
    theme.spacing.control.padding_y = 2.0;
    theme.spacing.panel.content_padding = 7.0;
    theme.spacing.property_row.label_width = 105.0;
    theme.spacing.property_row.gap = 5.0;
    if theme.kind == ThemeKind::Dark {
        theme.background = Rgba(48.0 / 255.0, 48.0 / 255.0, 48.0 / 255.0, 1.0);
        theme.surface = Rgba(61.0 / 255.0, 61.0 / 255.0, 61.0 / 255.0, 1.0);
        theme.surface_hover = Rgba(84.0 / 255.0, 84.0 / 255.0, 84.0 / 255.0, 1.0);
    }
}

fn field(label: &str, value: f64) -> WidgetNode {
    WidgetNode::property_row_with_decorator(
        label,
        WidgetNode::number("", value, -10_000.0, 10_000.0, 0.1)
            .with_role("centered-number")
            .with_height(UiLength::Px(20.0)),
    )
    .with_height(UiLength::Px(20.0))
}

fn panel(title: &str, children: impl IntoIterator<Item = WidgetNode>) -> WidgetNode {
    WidgetNode::panel(title, true, children)
        .with_width(UiLength::Fill(1.0))
        .with_gap(3.0)
}

fn sample_graph() -> NodeGraph {
    let mut graph = NodeGraph::default();
    let texture = graph.add_node_with_category(
        "Noise Texture",
        [45.0, 140.0],
        vec![NodeSocket::output("Factor", SocketType::Float)],
        NodeCategory::Texture,
    );
    let ramp = graph.add_node_with_category(
        "Color Ramp",
        [295.0, 130.0],
        vec![
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Color,
    );
    let mix = graph.add_node_with_category(
        "Mix Color",
        [540.0, 220.0],
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
        [790.0, 220.0],
        vec![NodeSocket::input("Surface", SocketType::Color)],
        NodeCategory::Output,
    );
    let _ = graph.create_link(texture, 0, ramp, 0);
    let _ = graph.create_link(ramp, 1, mix, 0);
    let _ = graph.create_link(mix, 3, output, 0);
    graph.zoom = 0.8;
    graph.pan = [35.0, 100.0];
    graph
}

fn graph_pane() -> WidgetNode {
    WidgetNode::column([
        WidgetNode::row([
            WidgetNode::dropdown("", ["Material", "World"], Some(0))
                .with_role("compact-dropdown")
                .with_width(UiLength::Px(145.0))
                .with_height(UiLength::Px(22.0)),
            WidgetNode::label("Surface  ·  Material.001").with_height(UiLength::Px(22.0)),
        ])
        .with_gap(8.0)
        .with_height(UiLength::Px(24.0)),
        WidgetNode::node_graph("Material graph", sample_graph())
            .with_key("editor-material-graph")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
    .with_gap(5.0)
    .with_padding(UiEdges::all(5.0))
}

fn scene_pane() -> WidgetNode {
    WidgetNode::column([
        WidgetNode::text_field("Search", "")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(22.0)),
        panel(
            "Scene Collection",
            [
                WidgetNode::label("▾  Collection").with_height(UiLength::Px(22.0)),
                WidgetNode::label("    ◉  Camera").with_height(UiLength::Px(22.0)),
                WidgetNode::label("    ◉  Cube").with_height(UiLength::Px(22.0)),
                WidgetNode::label("    ◉  Light").with_height(UiLength::Px(22.0)),
            ],
        ),
        panel(
            "Assets",
            [
                WidgetNode::label("▾  Materials").with_height(UiLength::Px(22.0)),
                WidgetNode::label("    ◉  Material.001").with_height(UiLength::Px(22.0)),
                WidgetNode::label("▸  Textures").with_height(UiLength::Px(22.0)),
            ],
        ),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
    .with_gap(6.0)
    .with_padding(UiEdges::all(8.0))
}

fn properties_pane() -> WidgetNode {
    WidgetNode::column([
        WidgetNode::text_field("Object", "Cube")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(22.0)),
        panel(
            "Transform",
            [
                WidgetNode::label("Location").with_height(UiLength::Px(18.0)),
                field("X", 0.0),
                field("Y", 1.25),
                field("Z", 0.0),
                WidgetNode::label("Rotation").with_height(UiLength::Px(18.0)),
                field("X", 0.0),
                field("Y", 0.0),
                field("Z", 90.0),
                WidgetNode::property_row_with_decorator(
                    "Scale",
                    WidgetNode::slider_range("", 1.0, 0.0, 4.0, 0.01)
                        .with_role("filled-slider")
                        .with_height(UiLength::Px(20.0)),
                )
                .with_height(UiLength::Px(20.0)),
            ],
        ),
        panel(
            "Material",
            [
                WidgetNode::label("Material.001").with_height(UiLength::Px(20.0)),
                WidgetNode::property_row_with_decorator(
                    "Metallic",
                    WidgetNode::slider_range("", 0.35, 0.0, 1.0, 0.01)
                        .with_role("filled-slider")
                        .with_height(UiLength::Px(20.0)),
                ),
                WidgetNode::property_row_with_decorator(
                    "Roughness",
                    WidgetNode::slider_range("", 0.45, 0.0, 1.0, 0.01)
                        .with_role("filled-slider")
                        .with_height(UiLength::Px(20.0)),
                ),
            ],
        ),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
    .with_gap(6.0)
    .with_padding(UiEdges::all(8.0))
}

fn console_pane() -> WidgetNode {
    WidgetNode::column([
        WidgetNode::label("[10:42:18]  Loaded scene").with_height(UiLength::Px(19.0)),
        WidgetNode::label("[10:42:19]  Material.001 compiled").with_height(UiLength::Px(19.0)),
        WidgetNode::label("[10:42:20]  Render engine ready").with_height(UiLength::Px(19.0)),
        WidgetNode::text_field("Command", "")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(22.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
    .with_gap(4.0)
    .with_padding(UiEdges::all(8.0))
}

pub fn mount(world: &mut World, parent: Entity, id: &str) {
    let pane = match id {
        "scene" => scene_pane(),
        "graph" => graph_pane(),
        "properties" => properties_pane(),
        _ => console_pane(),
    };
    if let Err(error) = WidgetTree::new(pane).spawn(world, Some(parent)) {
        bevy::log::error!("Could not mount {id} pane: {error:?}");
    }
}

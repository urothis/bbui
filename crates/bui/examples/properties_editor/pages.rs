//! Secondary property pages matching the controls in BUI's component demo.

use super::*;
use bui::widgets::{
    ListItem, ListModel, NodeCategory, NodeSocket, PreviewGrid, PreviewGridItem, SocketType,
    TreeModel, TreeNode,
};

fn auxiliary_page(key: &str, title: &str, content: WidgetNode) -> WidgetNode {
    WidgetNode::column([
        WidgetNode::label(format!("{title} Properties")).with_height(UiLength::Px(21.0)),
        content
            .with_key(format!("{key}-content"))
            .with_width(UiLength::Fill(1.0)),
    ])
    .with_key(key)
    .with_gap(3.0)
    .with_width(UiLength::Fill(1.0))
}

fn property_value(label: &str, value: WidgetNode) -> WidgetNode {
    WidgetNode::property_row_with_decorator(label, value).with_height(UiLength::Px(21.0))
}

pub(super) fn modifier_page() -> WidgetNode {
    let mut ramp = bui::widgets::ColorRamp::default();
    let _ = ramp.add_stop(0.45, Rgba(0.9, 0.3, 0.1, 1.0));
    let _ = ramp.set_color(1, Rgba(0.05, 0.05, 0.2, 1.0));
    let _ = ramp.set_color(2, Rgba(1.0, 0.95, 0.6, 1.0));
    auxiliary_page(
        PAGE_KEYS[1],
        "Modifier",
        WidgetNode::column([
            WidgetNode::button("Add Modifier")
                .with_key("add-modifier")
                .with_height(UiLength::Px(24.0)),
            WidgetNode::panel(
                "Subdivision",
                true,
                [
                    WidgetNode::dropdown("", ["Catmull-Clark", "Simple"], Some(0))
                        .with_role("compact-dropdown")
                        .with_height(UiLength::Px(21.0)),
                    property_value(
                        "Levels Viewport",
                        WidgetNode::number("", 1.0, 0.0, 6.0, 1.0),
                    ),
                    property_value("Render", WidgetNode::number("", 2.0, 0.0, 6.0, 1.0)),
                    check("Optimal Display", true),
                ],
            )
            .with_key("modifier-subdivision")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Color Ramp",
                true,
                [WidgetNode::color_ramp("", ramp)
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Px(125.0))],
            )
            .with_key("modifier-ramp")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "RGB Curves",
                true,
                [WidgetNode::curve_editor_default("")
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Px(150.0))],
            )
            .with_key("modifier-curves")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::column([WidgetNode::panel(
                "Mirror",
                true,
                [
                    check("X", true),
                    check("Y", false),
                    check("Z", false),
                    check("Clipping", true),
                ],
            )
            .with_key("mirror-modifier")
            .with_width(UiLength::Fill(1.0))])
            .with_width(UiLength::Fill(1.0)),
        ])
        .with_gap(4.0)
        .with_width(UiLength::Fill(1.0)),
    )
}

pub(super) fn material_page() -> WidgetNode {
    let preview = PreviewGrid {
        items: [
            ("Metal", [0.70, 0.72, 0.75, 1.0]),
            ("Gold", [0.95, 0.75, 0.30, 1.0]),
            ("Glass", [0.60, 0.85, 0.95, 1.0]),
            ("Rubber", [0.15, 0.15, 0.15, 1.0]),
            ("Emission", [1.0, 0.50, 0.20, 1.0]),
            ("Wood", [0.50, 0.30, 0.15, 1.0]),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, tint))| PreviewGridItem {
            id: index as u64 + 1,
            label: name.into(),
            asset_id: None,
            tint,
            enabled: true,
        })
        .collect(),
        columns: 3,
        cell_width: 122.0,
        cell_height: 72.0,
        ..PreviewGrid::default()
    };
    auxiliary_page(
        PAGE_KEYS[2],
        "Material",
        WidgetNode::column([
            WidgetNode::panel(
                "Preview",
                true,
                [WidgetNode::preview_grid("Materials", preview)
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Px(155.0))],
            )
            .with_key("material-preview")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Surface",
                true,
                [
                    WidgetNode::hsv_color_default("Base Color")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(236.0)),
                    property_value(
                        "Metallic",
                        WidgetNode::slider_range("", 0.0, 0.0, 1.0, 0.01)
                            .with_role("filled-slider"),
                    ),
                    property_value(
                        "Roughness",
                        WidgetNode::slider_range("", 0.5, 0.0, 1.0, 0.01)
                            .with_role("filled-slider"),
                    ),
                    property_value("IOR", WidgetNode::number("", 1.45, 1.0, 4.0, 0.01)),
                ],
            )
            .with_key("material-surface")
            .with_width(UiLength::Fill(1.0)),
        ])
        .with_gap(4.0)
        .with_width(UiLength::Fill(1.0)),
    )
}

pub(super) fn outliner_page() -> WidgetNode {
    let names = [
        "Scene Collection",
        "Camera",
        "Light",
        "Props",
        "Cube",
        "Suzanne",
        "Plane",
    ];
    let parents = [None, Some(1), Some(1), Some(1), Some(4), Some(4), Some(4)];
    let children: [Vec<u64>; 7] = [
        vec![2, 3, 4],
        vec![],
        vec![],
        vec![5, 6, 7],
        vec![],
        vec![],
        vec![],
    ];
    let tree = TreeModel {
        nodes: names
            .into_iter()
            .enumerate()
            .map(|(index, name)| TreeNode {
                id: index as u64 + 1,
                parent: parents[index],
                children: children[index].clone(),
                value: name.into(),
                expanded: true,
                enabled: true,
            })
            .collect(),
        ..TreeModel::default()
    };
    let list = ListModel {
        items: [
            "Cube", "Camera", "Light", "Suzanne", "Plane", "Cylinder", "Empty", "Torus",
        ]
        .into_iter()
        .enumerate()
        .map(|(index, name)| ListItem {
            id: index as u64 + 1,
            value: name.into(),
            enabled: true,
        })
        .collect(),
        ..ListModel::default()
    };
    auxiliary_page(
        PAGE_KEYS[3],
        "Outliner",
        WidgetNode::column([
            WidgetNode::text_field("Search", "").with_height(UiLength::Px(23.0)),
            WidgetNode::tree("Scene", tree)
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(205.0)),
            WidgetNode::panel(
                "UI List",
                true,
                [WidgetNode::list("Objects", list)
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Px(160.0))],
            )
            .with_key("outliner-list")
            .with_width(UiLength::Fill(1.0)),
        ])
        .with_gap(4.0)
        .with_width(UiLength::Fill(1.0)),
    )
}

pub(super) fn preferences_page() -> WidgetNode {
    auxiliary_page(
        PAGE_KEYS[4],
        "Preferences",
        WidgetNode::column([
            WidgetNode::panel(
                "Input",
                true,
                [
                    property_value("Search", WidgetNode::text_field("", "F3")),
                    property_value("Render", WidgetNode::text_field("", "F12")),
                    property_value(
                        "Emulate",
                        WidgetNode::dropdown("", ["Off", "3 Button", "Numpad"], Some(0))
                            .with_role("compact-dropdown"),
                    ),
                ],
            )
            .with_key("prefs-input")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "File Paths",
                true,
                [
                    property_value("Fonts", WidgetNode::text_field("", "//fonts/")),
                    property_value("Textures", WidgetNode::text_field("", "//textures/")),
                    property_value("Temp", WidgetNode::text_field("", "//tmp/")),
                ],
            )
            .with_key("prefs-paths")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Add-ons",
                true,
                [
                    WidgetNode::text_field("Search", ""),
                    check("Add-on 01", true),
                    check("Add-on 02", false),
                    check("Add-on 03", true),
                ],
            )
            .with_key("prefs-addons")
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Interface",
                true,
                [
                    check("Show Gizmos", true),
                    check("Show Overlays", true),
                    property_value(
                        "Scale",
                        WidgetNode::slider_range("", 1.0, 0.5, 2.0, 0.05)
                            .with_role("filled-slider"),
                    ),
                ],
            )
            .with_key("prefs-interface")
            .with_width(UiLength::Fill(1.0)),
        ])
        .with_gap(4.0)
        .with_width(UiLength::Fill(1.0)),
    )
}

pub(super) fn nodes_page() -> WidgetNode {
    let mut graph = NodeGraph::default();
    let input = graph.add_node_with_category(
        "Group Input",
        [10.0, 120.0],
        vec![NodeSocket::output("Geometry", SocketType::Any)],
        NodeCategory::Input,
    );
    let position = graph.add_node_with_category(
        "Set Position",
        [260.0, 100.0],
        vec![
            NodeSocket::input("Geometry", SocketType::Any),
            NodeSocket::input("Offset", SocketType::Vector3),
            NodeSocket::output("Geometry", SocketType::Any),
        ],
        NodeCategory::Geometry,
    );
    let output = graph.add_node_with_category(
        "Group Output",
        [510.0, 120.0],
        vec![NodeSocket::input("Geometry", SocketType::Any)],
        NodeCategory::Output,
    );
    if let Err(error) = graph.create_link(input, 0, position, 0) {
        eprintln!("geometry graph input link: {error:?}");
    }
    if let Err(error) = graph.create_link(position, 2, output, 0) {
        eprintln!("geometry graph output link: {error:?}");
    }
    graph.zoom = 0.65;
    graph.pan = [18.0, 60.0];
    auxiliary_page(
        PAGE_KEYS[5],
        "Geometry Nodes",
        WidgetNode::node_graph("", graph)
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(490.0)),
    )
}

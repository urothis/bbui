use super::*;

pub(in crate::actions) struct NodeGraphPaint<'a> {
    pub(in crate::actions) graph: &'a NodeGraph,
    pub(in crate::actions) state: NodeGraphEditorState,
    pub(in crate::actions) output_label_widths: HashMap<(NodeId, usize), f32>,
    pub(in crate::actions) registry: Option<&'a NodeGraphTypeRegistry>,
    pub(in crate::actions) canvas_surface: Rgba,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) theme: &'a Theme,
    pub(in crate::actions) font: TextFont,
}

#[derive(Clone, Copy)]
pub(in crate::actions) enum NodeGraphPaintTarget<'a> {
    Chrome,
    Node(&'a crate::NodeModel),
    Link {
        link: crate::NodeLink,
        from: &'a crate::NodeModel,
        to: &'a crate::NodeModel,
    },
}

#[cfg(test)]
pub(in crate::actions) fn paint_node_graph(paint: NodeGraphPaint<'_>, output: &mut Vec<DrawItem>) {
    let NodeGraphPaint {
        graph,
        state,
        output_label_widths,
        registry,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font,
    } = paint;
    let make_paint = |output_label_widths| NodeGraphPaint {
        graph,
        state: state.clone(),
        output_label_widths,
        registry,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font: font.clone(),
    };
    paint_node_graph_target(
        make_paint(output_label_widths.clone()),
        NodeGraphPaintTarget::Chrome,
        output,
    );
    let nodes_by_id: HashMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    for link in &graph.links {
        let (Some(from), Some(to)) = (
            nodes_by_id.get(&link.from_node).copied(),
            nodes_by_id.get(&link.to_node).copied(),
        ) else {
            continue;
        };
        paint_node_graph_target(
            make_paint(HashMap::new()),
            NodeGraphPaintTarget::Link {
                link: *link,
                from,
                to,
            },
            output,
        );
    }
    for node in &graph.nodes {
        paint_node_graph_target(
            make_paint(output_label_widths.clone()),
            NodeGraphPaintTarget::Node(node),
            output,
        );
    }
}

mod target;
pub(in crate::actions) use target::paint_node_graph_target;

mod helpers;
use helpers::split_node_graph_bezier;
pub(in crate::actions) use helpers::*;

mod link;
use link::paint_graph_link;

mod node;
use node::paint_graph_node;

mod gesture;
use gesture::paint_graph_gesture;

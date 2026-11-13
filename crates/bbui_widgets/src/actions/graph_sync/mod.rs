use super::*;

mod body;
pub use body::{sync_node_graph_bodies, sync_node_graph_socket_values};

mod views;
pub use views::sync_node_graph_views;

mod spatial;
use spatial::*;

mod signatures;
use signatures::*;

mod delta;
use delta::*;

pub(super) struct NodeGraphViewContext<'a> {
    owner: Entity,
    graph: &'a NodeGraph,
    layout: UiLayoutResult,
    spacing: &'a bbui_style::ThemeNodeGraphSpacing,
    node_indices: &'a HashMap<NodeId, usize>,
    link_indices: &'a HashMap<u64, usize>,
    incident_links: &'a HashMap<NodeId, Vec<u64>>,
}

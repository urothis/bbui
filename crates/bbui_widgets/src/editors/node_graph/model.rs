use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SocketId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketType {
    Any,
    Boolean,
    Integer,
    Float,
    Vector2,
    Vector3,
    Color,
    Text,
    Custom(String),
}

impl SocketType {
    pub fn custom(id: impl Into<String>) -> Self {
        Self::Custom(id.into())
    }

    pub(super) fn compatible_with(&self, other: &Self) -> bool {
        self == &Self::Any || other == &Self::Any || self == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketDirection {
    Input,
    Output,
}

/// Where a socket sits on its node. `Auto` keeps inputs on the left and outputs on the right.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketPlacement {
    #[default]
    Auto,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSocket {
    /// Stable identity within its node. Zero is assigned when the node is added to a graph.
    #[serde(default)]
    pub id: SocketId,
    pub name: String,
    pub direction: SocketDirection,
    pub socket_type: SocketType,
    /// Value used by an unconnected input. Applications may leave this unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Placement on the node edge; defaults to the direction's side for older saved graphs.
    #[serde(default)]
    pub placement: SocketPlacement,
}

impl NodeSocket {
    pub fn input(name: impl Into<String>, socket_type: SocketType) -> Self {
        Self {
            id: SocketId::default(),
            name: name.into(),
            direction: SocketDirection::Input,
            socket_type,
            value: None,
            placement: SocketPlacement::Auto,
        }
    }
    pub fn output(name: impl Into<String>, socket_type: SocketType) -> Self {
        Self {
            id: SocketId::default(),
            name: name.into(),
            direction: SocketDirection::Output,
            socket_type,
            value: None,
            placement: SocketPlacement::Auto,
        }
    }

    pub fn with_id(mut self, id: SocketId) -> Self {
        self.id = id;
        self
    }

    pub fn with_value(mut self, value: serde_json::Value) -> Self {
        self.value = Some(value);
        self
    }

    /// Place this socket along the left half of the bottom edge.
    pub fn bottom_left(mut self) -> Self {
        self.placement = SocketPlacement::BottomLeft;
        self
    }

    /// Place this socket along the right half of the bottom edge.
    pub fn bottom_right(mut self) -> Self {
        self.placement = SocketPlacement::BottomRight;
        self
    }
}

/// Visual node-header grouping. Categories are explicit model data so painter colors do not
/// depend on localized or user-editable node titles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    #[default]
    General,
    Input,
    Texture,
    Color,
    Geometry,
    Converter,
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeModel {
    pub id: NodeId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_id: Option<String>,
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub data: serde_json::Value,
    /// Category palette used for the node header. Missing in older serialized graphs, defaults to
    /// `General` when deserialized.
    #[serde(default)]
    pub category: NodeCategory,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub sockets: Vec<NodeSocket>,
    pub collapsed: bool,
}

impl NodeModel {
    pub fn new(
        id: NodeId,
        title: impl Into<String>,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
    ) -> Self {
        Self::new_with_category(id, title, position, sockets, NodeCategory::General)
    }

    pub fn new_with_category(
        id: NodeId,
        title: impl Into<String>,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
        category: NodeCategory,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            type_id: None,
            schema_version: 0,
            data: serde_json::Value::Null,
            category,
            position,
            size: [180.0, 58.0],
            sockets,
            collapsed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeLink {
    pub id: u64,
    pub from_node: NodeId,
    pub from_socket: usize,
    #[serde(default)]
    pub from_socket_id: SocketId,
    pub to_node: NodeId,
    pub to_socket: usize,
    #[serde(default)]
    pub to_socket_id: SocketId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkEndpoint {
    From,
    To,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeGraphError {
    MissingNode,
    MissingSocket,
    WrongDirection,
    IncompatibleSockets,
    SelfLink,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[component(summary_tick)]
#[serde(default)]
pub struct NodeGraph {
    pub nodes: Vec<NodeModel>,
    pub links: Vec<NodeLink>,
    pub selected: BTreeSet<NodeId>,
    pub selected_link: Option<u64>,
    pub pan: [f32; 2],
    pub zoom: f32,
    pub(super) next_node: u64,
    pub(super) next_link: u64,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            selected: BTreeSet::new(),
            selected_link: None,
            pan: [0.0, 0.0],
            zoom: 1.0,
            next_node: 1,
            next_link: 1,
        }
    }
}

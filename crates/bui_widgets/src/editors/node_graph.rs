use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketType {
    Any,
    Boolean,
    Float,
    Vector2,
    Vector3,
    Color,
    Text,
}

impl SocketType {
    fn compatible_with(self, other: Self) -> bool {
        self == Self::Any || other == Self::Any || self == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSocket {
    pub name: String,
    pub direction: SocketDirection,
    pub socket_type: SocketType,
}

impl NodeSocket {
    pub fn input(name: impl Into<String>, socket_type: SocketType) -> Self {
        Self {
            name: name.into(),
            direction: SocketDirection::Input,
            socket_type,
        }
    }
    pub fn output(name: impl Into<String>, socket_type: SocketType) -> Self {
        Self {
            name: name.into(),
            direction: SocketDirection::Output,
            socket_type,
        }
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
    pub to_node: NodeId,
    pub to_socket: usize,
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
#[serde(default)]
pub struct NodeGraph {
    pub nodes: Vec<NodeModel>,
    pub links: Vec<NodeLink>,
    pub selected: BTreeSet<NodeId>,
    pub selected_link: Option<u64>,
    pub pan: [f32; 2],
    pub zoom: f32,
    next_node: u64,
    next_link: u64,
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

impl NodeGraph {
    pub fn add_node(
        &mut self,
        title: impl Into<String>,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
    ) -> NodeId {
        self.add_node_with_category(title, position, sockets, NodeCategory::General)
    }

    pub fn add_node_with_category(
        &mut self,
        title: impl Into<String>,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
        category: NodeCategory,
    ) -> NodeId {
        let id = NodeId(self.next_node);
        self.next_node = self.next_node.saturating_add(1);
        self.nodes.push(NodeModel::new_with_category(
            id, title, position, sockets, category,
        ));
        id
    }

    pub fn move_node(&mut self, id: NodeId, position: [f32; 2]) -> bool {
        if !position.into_iter().all(f32::is_finite) {
            return false;
        }
        let Some(node) = self.nodes.iter_mut().find(|node| node.id == id) else {
            return false;
        };
        let changed = node.position != position;
        node.position = position;
        changed
    }

    pub fn select(&mut self, id: NodeId, additive: bool) -> bool {
        if !self.nodes.iter().any(|node| node.id == id) {
            return false;
        }
        let before = self.selected.clone();
        let before_link = self.selected_link;
        self.selected_link = None;
        if !additive {
            self.selected.clear();
        }
        if additive && self.selected.contains(&id) {
            self.selected.remove(&id);
        } else {
            self.selected.insert(id);
        }
        before != self.selected || before_link.is_some()
    }

    pub fn select_link(&mut self, id: u64) -> bool {
        if !self.links.iter().any(|link| link.id == id) {
            return false;
        }
        let changed = self.selected_link != Some(id) || !self.selected.is_empty();
        self.selected.clear();
        self.selected_link = Some(id);
        changed
    }

    pub fn select_all(&mut self) -> bool {
        let before = self.selected.clone();
        let before_link = self.selected_link;
        self.selected = self.nodes.iter().map(|node| node.id).collect();
        self.selected_link = None;
        before != self.selected || before_link.is_some()
    }

    pub fn move_selected_by(&mut self, delta: [f32; 2]) -> bool {
        if !delta.into_iter().all(f32::is_finite) {
            return false;
        }
        let mut changed = false;
        for node in &mut self.nodes {
            if self.selected.contains(&node.id) {
                let next = [node.position[0] + delta[0], node.position[1] + delta[1]];
                if next.into_iter().all(f32::is_finite) && node.position != next {
                    node.position = next;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn toggle_selected_collapsed(&mut self) -> bool {
        let selected: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| self.selected.contains(&node.id))
            .map(|node| node.id)
            .collect();
        if selected.is_empty() {
            return false;
        }
        let collapse = self
            .nodes
            .iter()
            .find(|node| node.id == selected[0])
            .is_some_and(|node| !node.collapsed);
        for node in &mut self.nodes {
            if self.selected.contains(&node.id) {
                node.collapsed = collapse;
            }
        }
        true
    }

    pub fn create_link(
        &mut self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<u64, NodeGraphError> {
        self.validate_link(from_node, from_socket, to_node, to_socket)?;
        self.links
            .retain(|link| !(link.to_node == to_node && link.to_socket == to_socket));
        let id = self.next_link;
        self.next_link = self.next_link.saturating_add(1);
        self.links.push(NodeLink {
            id,
            from_node,
            from_socket,
            to_node,
            to_socket,
        });
        Ok(id)
    }

    pub fn reconnect_link(
        &mut self,
        link_id: u64,
        endpoint: LinkEndpoint,
        node_id: NodeId,
        socket_index: usize,
    ) -> Result<(), NodeGraphError> {
        let old = *self
            .links
            .iter()
            .find(|link| link.id == link_id)
            .ok_or(NodeGraphError::MissingSocket)?;
        let (from_node, from_socket, to_node, to_socket) = match endpoint {
            LinkEndpoint::From => (node_id, socket_index, old.to_node, old.to_socket),
            LinkEndpoint::To => (old.from_node, old.from_socket, node_id, socket_index),
        };
        self.validate_link(from_node, from_socket, to_node, to_socket)?;
        if endpoint == LinkEndpoint::To {
            self.links.retain(|link| {
                link.id == link_id || link.to_node != to_node || link.to_socket != to_socket
            });
        }
        let link = self
            .links
            .iter_mut()
            .find(|link| link.id == link_id)
            .ok_or(NodeGraphError::MissingSocket)?;
        link.from_node = from_node;
        link.from_socket = from_socket;
        link.to_node = to_node;
        link.to_socket = to_socket;
        Ok(())
    }

    pub fn remove_link(&mut self, id: u64) -> bool {
        let before = self.links.len();
        self.links.retain(|link| link.id != id);
        if self.selected_link == Some(id) {
            self.selected_link = None;
        }
        self.links.len() != before
    }

    pub fn remove_node(&mut self, id: NodeId) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|node| node.id != id);
        if self.nodes.len() == before {
            return false;
        }
        self.links
            .retain(|link| link.from_node != id && link.to_node != id);
        self.selected.remove(&id);
        self.selected_link = self
            .selected_link
            .filter(|link_id| self.links.iter().any(|link| link.id == *link_id));
        true
    }

    pub fn pan_by(&mut self, delta: [f32; 2]) {
        if delta.into_iter().all(f32::is_finite) {
            self.pan[0] += delta[0];
            self.pan[1] += delta[1];
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(0.1, 10.0);
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string(self).map_err(|error| error.to_string())
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut graph: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        graph.next_node = graph
            .nodes
            .iter()
            .map(|node| node.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(graph.next_node);
        graph.next_link = graph
            .links
            .iter()
            .map(|link| link.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(graph.next_link);
        graph.validate()?;
        Ok(graph)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.zoom.is_finite() || self.zoom <= 0.0 || !self.pan.into_iter().all(f32::is_finite) {
            return Err("node graph view transform must be finite with positive zoom".into());
        }
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            if node.id.0 == 0 || !node_ids.insert(node.id) {
                return Err("node IDs must be unique and nonzero".into());
            }
            if !node.position.into_iter().all(f32::is_finite)
                || !node.size.into_iter().all(f32::is_finite)
                || node.size[0] < 80.0
                || node.size[1] < 32.0
            {
                return Err(format!("node {} has invalid geometry", node.id.0));
            }
        }
        if !self.selected.iter().all(|id| node_ids.contains(id)) {
            return Err("selected node IDs must exist in the graph".into());
        }
        let mut link_ids = BTreeSet::new();
        let mut destinations = BTreeSet::new();
        for link in &self.links {
            if link.id == 0 || !link_ids.insert(link.id) {
                return Err("link IDs must be unique and nonzero".into());
            }
            self.validate_link(
                link.from_node,
                link.from_socket,
                link.to_node,
                link.to_socket,
            )
            .map_err(|error| format!("link {} is invalid: {error:?}", link.id))?;
            if !destinations.insert((link.to_node, link.to_socket)) {
                return Err("input sockets can have only one link".into());
            }
        }
        if self.selected_link.is_some_and(|id| !link_ids.contains(&id)) {
            return Err("selected link ID must exist in the graph".into());
        }
        Ok(())
    }

    fn validate_link(
        &self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<(), NodeGraphError> {
        if from_node == to_node {
            return Err(NodeGraphError::SelfLink);
        }
        let from_node = self
            .nodes
            .iter()
            .find(|node| node.id == from_node)
            .ok_or(NodeGraphError::MissingNode)?;
        let to_node = self
            .nodes
            .iter()
            .find(|node| node.id == to_node)
            .ok_or(NodeGraphError::MissingNode)?;
        let from = from_node
            .sockets
            .get(from_socket)
            .ok_or(NodeGraphError::MissingSocket)?;
        let to = to_node
            .sockets
            .get(to_socket)
            .ok_or(NodeGraphError::MissingSocket)?;
        if from.direction != SocketDirection::Output || to.direction != SocketDirection::Input {
            return Err(NodeGraphError::WrongDirection);
        }
        if !from.socket_type.compatible_with(to.socket_type) {
            return Err(NodeGraphError::IncompatibleSockets);
        }
        Ok(())
    }
}

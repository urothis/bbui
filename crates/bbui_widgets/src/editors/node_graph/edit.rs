use super::*;

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
        let mut node = NodeModel::new_with_category(id, title, position, sockets, category);
        Self::assign_socket_ids(&mut node);
        self.nodes.push(node);
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
}

impl NodeGraph {
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
}

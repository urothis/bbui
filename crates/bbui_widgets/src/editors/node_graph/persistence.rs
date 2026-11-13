use super::*;

impl NodeGraph {
    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string(self).map_err(|error| error.to_string())
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut graph: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        for node in &mut graph.nodes {
            Self::assign_socket_ids(node);
        }
        for link in &mut graph.links {
            if link.from_socket_id.0 == 0 {
                link.from_socket_id = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == link.from_node)
                    .and_then(|node| node.sockets.get(link.from_socket))
                    .map_or(SocketId(0), |socket| socket.id);
            }
            if link.to_socket_id.0 == 0 {
                link.to_socket_id = graph
                    .nodes
                    .iter()
                    .find(|node| node.id == link.to_node)
                    .and_then(|node| node.sockets.get(link.to_socket))
                    .map_or(SocketId(0), |socket| socket.id);
            }
        }
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
            let ids: BTreeSet<_> = node.sockets.iter().map(|socket| socket.id).collect();
            if ids.len() != node.sockets.len() || ids.contains(&SocketId(0)) {
                return Err(format!(
                    "node {} has duplicate or missing socket IDs",
                    node.id.0
                ));
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
            let (from, to) = self
                .link_sockets(
                    link.from_node,
                    link.from_socket,
                    link.to_node,
                    link.to_socket,
                )
                .map_err(|error| format!("link {} is invalid: {error:?}", link.id))?;
            if from.id != link.from_socket_id || to.id != link.to_socket_id {
                return Err(format!("link {} has stale socket indexes", link.id));
            }
            if !destinations.insert((link.to_node, link.to_socket)) {
                return Err("input sockets can have only one link".into());
            }
        }
        if self.selected_link.is_some_and(|id| !link_ids.contains(&id)) {
            return Err("selected link ID must exist in the graph".into());
        }
        Ok(())
    }
}

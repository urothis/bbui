use super::*;

impl NodeGraph {
    pub fn add_defined_node(
        &mut self,
        registry: &NodeGraphTypeRegistry,
        type_id: &str,
        title: impl Into<String>,
        position: [f32; 2],
        data: serde_json::Value,
    ) -> Result<NodeId, String> {
        let definition = registry
            .node(type_id)
            .ok_or_else(|| format!("unknown node type: {type_id}"))?;
        let id = self.add_node_with_category(
            title,
            position,
            definition.sockets.clone(),
            definition.category,
        );
        if let Some(node) = self.nodes.last_mut() {
            node.type_id = Some(type_id.to_owned());
            node.schema_version = definition.version;
            node.data = data;
        }
        Ok(id)
    }

    /// Upgrade registered nodes in a clone, then remap every link using stable socket IDs.
    /// Unknown types stay in the graph unchanged so applications can recover their data.
    pub fn migrate_registered_nodes(
        &mut self,
        registry: &NodeGraphTypeRegistry,
    ) -> Result<(), String> {
        let mut updated = self.clone();
        for node in &mut updated.nodes {
            let Some(definition) = node.type_id.as_deref().and_then(|id| registry.node(id)) else {
                continue;
            };
            definition.migrate(node)?;
            if node.schema_version == definition.version {
                node.category = definition.category;
            }
        }
        for link in &mut updated.links {
            for (node_id, socket_id, index, direction) in [
                (
                    link.from_node,
                    link.from_socket_id,
                    &mut link.from_socket,
                    SocketDirection::Output,
                ),
                (
                    link.to_node,
                    link.to_socket_id,
                    &mut link.to_socket,
                    SocketDirection::Input,
                ),
            ] {
                let node = updated
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .ok_or_else(|| format!("link {} references missing node", link.id))?;
                *index = node
                    .sockets
                    .iter()
                    .position(|socket| socket.id == socket_id && socket.direction == direction)
                    .ok_or_else(|| {
                        format!("link {} references missing socket {}", link.id, socket_id.0)
                    })?;
            }
        }
        updated
            .validate()
            .map_err(|error| format!("migrated graph is invalid: {error:?}"))?;
        for link in &updated.links {
            let unresolved = [
                (link.from_node, link.from_socket),
                (link.to_node, link.to_socket),
            ]
            .into_iter()
            .filter_map(|(node_id, socket_index)| {
                updated
                    .nodes
                    .iter()
                    .find(|node| node.id == node_id)
                    .and_then(|node| node.sockets.get(socket_index))
            })
            .any(|socket| {
                matches!(&socket.socket_type, SocketType::Custom(_))
                    && registry.get(&socket.socket_type).is_none()
            });
            if !unresolved {
                updated
                    .validate_link_with_registry(
                        registry,
                        link.from_node,
                        link.from_socket,
                        link.to_node,
                        link.to_socket,
                    )
                    .map_err(|error| {
                        format!("link {} has incompatible types: {error:?}", link.id)
                    })?;
            }
        }
        *self = updated;
        Ok(())
    }

    pub(super) fn assign_socket_ids(node: &mut NodeModel) {
        let mut next = node
            .sockets
            .iter()
            .map(|socket| socket.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        for socket in &mut node.sockets {
            if socket.id.0 == 0 {
                socket.id = SocketId(next);
                next = next.saturating_add(1);
            }
        }
    }

    /// Replace a node's socket schema while retaining links by stable socket ID.
    /// A migration maps old IDs to their replacements when a socket was renamed or split.
    /// The graph is left unchanged if any connected socket cannot be resolved.
    pub fn replace_node_sockets(
        &mut self,
        node_id: NodeId,
        mut sockets: Vec<NodeSocket>,
        migration: &std::collections::HashMap<SocketId, SocketId>,
    ) -> Result<(), NodeGraphError> {
        let Some(node_index) = self.nodes.iter().position(|node| node.id == node_id) else {
            return Err(NodeGraphError::MissingNode);
        };
        let mut replacement = self.nodes[node_index].clone();
        replacement.sockets = std::mem::take(&mut sockets);
        Self::assign_socket_ids(&mut replacement);
        let ids: BTreeSet<_> = replacement.sockets.iter().map(|socket| socket.id).collect();
        if ids.len() != replacement.sockets.len() {
            return Err(NodeGraphError::MissingSocket);
        }
        let mut links = self.links.clone();
        for link in &mut links {
            if link.from_node == node_id {
                let id = migration
                    .get(&link.from_socket_id)
                    .copied()
                    .unwrap_or(link.from_socket_id);
                let Some(index) = replacement.sockets.iter().position(|socket| {
                    socket.id == id && socket.direction == SocketDirection::Output
                }) else {
                    return Err(NodeGraphError::MissingSocket);
                };
                link.from_socket = index;
                link.from_socket_id = id;
            }
            if link.to_node == node_id {
                let id = migration
                    .get(&link.to_socket_id)
                    .copied()
                    .unwrap_or(link.to_socket_id);
                let Some(index) = replacement.sockets.iter().position(|socket| {
                    socket.id == id && socket.direction == SocketDirection::Input
                }) else {
                    return Err(NodeGraphError::MissingSocket);
                };
                link.to_socket = index;
                link.to_socket_id = id;
            }
        }
        let old_node = std::mem::replace(&mut self.nodes[node_index], replacement);
        let old_links = std::mem::replace(&mut self.links, links);
        if self.validate().is_err() {
            self.nodes[node_index] = old_node;
            self.links = old_links;
            return Err(NodeGraphError::IncompatibleSockets);
        }
        Ok(())
    }

    /// Replace a schema and verify all resulting links against registered conversions.
    pub fn replace_node_sockets_with_registry(
        &mut self,
        registry: &NodeGraphTypeRegistry,
        node_id: NodeId,
        sockets: Vec<NodeSocket>,
        migration: &std::collections::HashMap<SocketId, SocketId>,
    ) -> Result<(), String> {
        let mut updated = self.clone();
        updated
            .replace_node_sockets(node_id, sockets, migration)
            .map_err(|error| format!("socket replacement failed: {error:?}"))?;
        updated.validate_registered_links(registry)?;
        *self = updated;
        Ok(())
    }

    /// Check link types using the application registry. Unknown custom types fail
    /// validation but remain intact when a graph is loaded or saved.
    pub fn validate_registered_links(
        &self,
        registry: &NodeGraphTypeRegistry,
    ) -> Result<(), String> {
        self.validate()?;
        for link in &self.links {
            self.validate_link_with_registry(
                registry,
                link.from_node,
                link.from_socket,
                link.to_node,
                link.to_socket,
            )
            .map_err(|error| format!("link {} has incompatible types: {error:?}", link.id))?;
        }
        Ok(())
    }
}

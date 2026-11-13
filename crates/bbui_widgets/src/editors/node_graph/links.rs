use super::*;

impl NodeGraph {
    pub fn create_link(
        &mut self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<u64, NodeGraphError> {
        self.validate_link(from_node, from_socket, to_node, to_socket)?;
        Ok(self.insert_link(from_node, from_socket, to_node, to_socket))
    }

    pub fn create_link_with_registry(
        &mut self,
        registry: &NodeGraphTypeRegistry,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<u64, NodeGraphError> {
        self.validate_link_with_registry(registry, from_node, from_socket, to_node, to_socket)?;
        Ok(self.insert_link(from_node, from_socket, to_node, to_socket))
    }

    fn insert_link(
        &mut self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> u64 {
        self.links
            .retain(|link| !(link.to_node == to_node && link.to_socket == to_socket));
        let id = self.next_link;
        self.next_link = self.next_link.saturating_add(1);
        self.links.push(NodeLink {
            id,
            from_node,
            from_socket,
            from_socket_id: self
                .nodes
                .iter()
                .find(|node| node.id == from_node)
                .map_or(SocketId(0), |node| node.sockets[from_socket].id),
            to_node,
            to_socket,
            to_socket_id: self
                .nodes
                .iter()
                .find(|node| node.id == to_node)
                .map_or(SocketId(0), |node| node.sockets[to_socket].id),
        });
        id
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
        self.update_link(
            link_id,
            endpoint,
            from_node,
            from_socket,
            to_node,
            to_socket,
        )
    }

    pub fn reconnect_link_with_registry(
        &mut self,
        registry: &NodeGraphTypeRegistry,
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
        self.validate_link_with_registry(registry, from_node, from_socket, to_node, to_socket)?;
        self.update_link(
            link_id,
            endpoint,
            from_node,
            from_socket,
            to_node,
            to_socket,
        )
    }

    fn update_link(
        &mut self,
        link_id: u64,
        endpoint: LinkEndpoint,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<(), NodeGraphError> {
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
        link.from_socket_id = self
            .nodes
            .iter()
            .find(|node| node.id == from_node)
            .map_or(SocketId(0), |node| node.sockets[from_socket].id);
        link.to_node = to_node;
        link.to_socket = to_socket;
        link.to_socket_id = self
            .nodes
            .iter()
            .find(|node| node.id == to_node)
            .map_or(SocketId(0), |node| node.sockets[to_socket].id);
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
}

impl NodeGraph {
    fn validate_link(
        &self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<(), NodeGraphError> {
        let (from, to) = self.link_sockets(from_node, from_socket, to_node, to_socket)?;
        if !from.socket_type.compatible_with(&to.socket_type) {
            return Err(NodeGraphError::IncompatibleSockets);
        }
        Ok(())
    }

    pub fn validate_link_with_registry(
        &self,
        registry: &NodeGraphTypeRegistry,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<(), NodeGraphError> {
        let (from, to) = self.link_sockets(from_node, from_socket, to_node, to_socket)?;
        if !registry.can_connect(&from.socket_type, &to.socket_type) {
            return Err(NodeGraphError::IncompatibleSockets);
        }
        Ok(())
    }

    pub(super) fn link_sockets(
        &self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<(&NodeSocket, &NodeSocket), NodeGraphError> {
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
        Ok((from, to))
    }
}

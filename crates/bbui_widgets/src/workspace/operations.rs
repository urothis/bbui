use super::*;

impl WorkspaceLayout {
    /// Validate a complete transaction before replacing the current layout.
    pub fn apply(&mut self, operation: WorkspaceOperation) -> Result<(), WorkspaceError> {
        self.validate()?;
        let mut next = self.clone();
        next.apply_inner(operation)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn apply_inner(&mut self, operation: WorkspaceOperation) -> Result<(), WorkspaceError> {
        match operation {
            WorkspaceOperation::Open { pane, group, index } => {
                if containing_group(&self.root, &pane).is_some() {
                    return Err(WorkspaceError::DuplicateOrEmptyId);
                }
                let destination = self
                    .root
                    .group_mut(&group)
                    .ok_or(WorkspaceError::UnknownGroup)?;
                destination
                    .panes
                    .insert(index.min(destination.panes.len()), pane.clone());
                destination.active = Some(pane);
            }
            WorkspaceOperation::Select { group, pane } => {
                let group = self
                    .root
                    .group_mut(&group)
                    .ok_or(WorkspaceError::UnknownGroup)?;
                if !group.panes.contains(&pane) {
                    return Err(WorkspaceError::UnknownPane);
                }
                group.active = Some(pane);
            }
            WorkspaceOperation::Close { pane } => {
                if !self.root.remove_pane(&pane) {
                    return Err(WorkspaceError::UnknownPane);
                }
                self.compact();
            }
            WorkspaceOperation::Move { pane, group, index } => {
                let destination = self.group(&group).ok_or(WorkspaceError::UnknownGroup)?;
                if !destination.options.accepts_drops {
                    return Err(WorkspaceError::DropRejected);
                }
                let source =
                    containing_group(&self.root, &pane).ok_or(WorkspaceError::UnknownPane)?;
                if source.id == group && !source.options.reorderable {
                    return Err(WorkspaceError::DropRejected);
                }
                if source.id != group && !source.options.movable {
                    return Err(WorkspaceError::DropRejected);
                }
                let index = if source.id == group {
                    let old = source
                        .panes
                        .iter()
                        .position(|candidate| candidate == &pane)
                        .unwrap_or(0);
                    index.saturating_sub(usize::from(old < index))
                } else {
                    index
                };
                if !self.root.remove_pane(&pane) {
                    return Err(WorkspaceError::UnknownPane);
                }
                let destination = self
                    .root
                    .group_mut(&group)
                    .ok_or(WorkspaceError::UnknownGroup)?;
                destination
                    .panes
                    .insert(index.min(destination.panes.len()), pane.clone());
                destination.active = Some(pane);
                self.compact();
            }
            WorkspaceOperation::Split {
                pane,
                target,
                id,
                group,
                axis,
                after,
            } => {
                self.take_for_new_group(&pane, &target)?;
                let fresh = TabGroup::new(group, vec![pane]);
                if !self.root.insert_split(&target, &id, axis, after, fresh) {
                    return Err(WorkspaceError::UnknownGroup);
                }
                self.compact();
            }
            WorkspaceOperation::Grid {
                pane,
                target,
                id,
                group,
                columns,
            } => {
                if columns == 0 {
                    return Err(WorkspaceError::InvalidContainer);
                }
                self.take_for_new_group(&pane, &target)?;
                let root =
                    find_node_mut(&mut self.root, &target).ok_or(WorkspaceError::UnknownGroup)?;
                let old = std::mem::replace(root, DockNode::Tabs(TabGroup::new("", Vec::new())));
                *root = DockNode::Grid {
                    id,
                    columns,
                    children: vec![old, DockNode::Tabs(TabGroup::new(group, vec![pane]))],
                };
                self.compact();
            }
            WorkspaceOperation::Resize { split, shares } => {
                let node =
                    find_node_mut(&mut self.root, &split).ok_or(WorkspaceError::UnknownGroup)?;
                let DockNode::Split {
                    shares: current, ..
                } = node
                else {
                    return Err(WorkspaceError::InvalidContainer);
                };
                *current = shares;
            }
            WorkspaceOperation::SetGroupOptions { group, options } => {
                self.root
                    .group_mut(&group)
                    .ok_or(WorkspaceError::UnknownGroup)?
                    .options = options;
            }
        }
        Ok(())
    }

    fn take_for_new_group(&mut self, pane: &PaneId, target: &str) -> Result<(), WorkspaceError> {
        let target_group = self.group(target).ok_or(WorkspaceError::UnknownGroup)?;
        if !target_group.options.accepts_drops {
            return Err(WorkspaceError::DropRejected);
        }
        let source = containing_group(&self.root, pane).ok_or(WorkspaceError::UnknownPane)?;
        if !source.options.movable {
            return Err(WorkspaceError::DropRejected);
        }
        if !self.root.remove_pane(pane) {
            return Err(WorkspaceError::UnknownPane);
        }
        Ok(())
    }

    fn compact(&mut self) {
        let id = match &self.root {
            DockNode::Tabs(group) => group.id.clone(),
            DockNode::Split { id, .. } | DockNode::Grid { id, .. } => id.clone(),
        };
        if !self.root.compact() {
            self.root = DockNode::Tabs(TabGroup::new(id, Vec::new()));
        }
    }
}

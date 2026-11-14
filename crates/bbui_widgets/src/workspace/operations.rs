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
                if self
                    .windows
                    .iter()
                    .any(|window| containing_group(&window.root, &pane).is_some())
                {
                    return Err(WorkspaceError::DuplicateOrEmptyId);
                }
                let destination = self
                    .windows
                    .iter_mut()
                    .find_map(|window| window.root.group_mut(&group))
                    .ok_or(WorkspaceError::UnknownGroup)?;
                destination
                    .panes
                    .insert(index.min(destination.panes.len()), pane.clone());
                destination.active = Some(pane);
            }
            WorkspaceOperation::Select { group, pane } => {
                let group = self
                    .windows
                    .iter_mut()
                    .find_map(|window| window.root.group_mut(&group))
                    .ok_or(WorkspaceError::UnknownGroup)?;
                if !group.panes.contains(&pane) {
                    return Err(WorkspaceError::UnknownPane);
                }
                group.active = Some(pane);
            }
            WorkspaceOperation::Close { pane } => {
                if !self
                    .windows
                    .iter_mut()
                    .any(|window| window.root.remove_pane(&pane))
                {
                    return Err(WorkspaceError::UnknownPane);
                }
                self.compact();
            }
            WorkspaceOperation::Move { pane, group, index } => {
                let destination = self.group(&group).ok_or(WorkspaceError::UnknownGroup)?;
                if !destination.options.accepts_drops {
                    return Err(WorkspaceError::DropRejected);
                }
                let source = self
                    .windows
                    .iter()
                    .find_map(|window| containing_group(&window.root, &pane))
                    .ok_or(WorkspaceError::UnknownPane)?;
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
                if !self
                    .windows
                    .iter_mut()
                    .any(|window| window.root.remove_pane(&pane))
                {
                    return Err(WorkspaceError::UnknownPane);
                }
                let destination = self
                    .windows
                    .iter_mut()
                    .find_map(|window| window.root.group_mut(&group))
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
                if !self.windows.iter_mut().any(|window| {
                    window
                        .root
                        .insert_split(&target, &id, axis, after, fresh.clone())
                }) {
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
                let root = self
                    .windows
                    .iter_mut()
                    .find_map(|window| find_node_mut(&mut window.root, &target))
                    .ok_or(WorkspaceError::UnknownGroup)?;
                let old = std::mem::replace(root, DockNode::Tabs(TabGroup::new("", Vec::new())));
                *root = DockNode::Grid {
                    id,
                    columns,
                    children: vec![old, DockNode::Tabs(TabGroup::new(group, vec![pane]))],
                };
                self.compact();
            }
            WorkspaceOperation::Detach {
                pane,
                window,
                group,
            } => {
                let source = self
                    .windows
                    .iter()
                    .find_map(|entry| containing_group(&entry.root, &pane))
                    .ok_or(WorkspaceError::UnknownPane)?;
                if !source.options.detachable || !source.options.movable {
                    return Err(WorkspaceError::DropRejected);
                }
                if self.windows.iter().any(|entry| entry.id == window) {
                    return Err(WorkspaceError::DuplicateOrEmptyId);
                }
                self.windows.iter_mut().for_each(|entry| {
                    entry.root.remove_pane(&pane);
                });
                self.compact();
                self.windows.push(WorkspaceWindow {
                    id: window,
                    root: DockNode::Tabs(TabGroup::new(group, vec![pane])),
                });
            }
            WorkspaceOperation::Resize { split, shares } => {
                let node = self
                    .windows
                    .iter_mut()
                    .find_map(|window| find_node_mut(&mut window.root, &split))
                    .ok_or(WorkspaceError::UnknownGroup)?;
                let DockNode::Split {
                    shares: current, ..
                } = node
                else {
                    return Err(WorkspaceError::InvalidContainer);
                };
                *current = shares;
            }
            WorkspaceOperation::SetGroupOptions { group, options } => {
                self.windows
                    .iter_mut()
                    .find_map(|window| window.root.group_mut(&group))
                    .ok_or(WorkspaceError::UnknownGroup)?
                    .options = options;
            }
            WorkspaceOperation::CloseWindow { window } => {
                if window == "main" {
                    return Err(WorkspaceError::DropRejected);
                }
                let before = self.windows.len();
                self.windows.retain(|entry| entry.id != window);
                if self.windows.len() == before {
                    return Err(WorkspaceError::UnknownWindow);
                }
            }
        }
        Ok(())
    }

    fn take_for_new_group(&mut self, pane: &PaneId, target: &str) -> Result<(), WorkspaceError> {
        let target_group = self.group(target).ok_or(WorkspaceError::UnknownGroup)?;
        if !target_group.options.accepts_drops {
            return Err(WorkspaceError::DropRejected);
        }
        let source = self
            .windows
            .iter()
            .find_map(|entry| containing_group(&entry.root, pane))
            .ok_or(WorkspaceError::UnknownPane)?;
        if !source.options.movable {
            return Err(WorkspaceError::DropRejected);
        }
        if !self
            .windows
            .iter_mut()
            .any(|window| window.root.remove_pane(pane))
        {
            return Err(WorkspaceError::UnknownPane);
        }
        Ok(())
    }

    fn compact(&mut self) {
        let mut index = 0;
        while index < self.windows.len() {
            let root = &mut self.windows[index].root;
            let id = match root {
                DockNode::Tabs(group) => group.id.clone(),
                DockNode::Split { id, .. } | DockNode::Grid { id, .. } => id.clone(),
            };
            if root.compact() {
                index += 1;
            } else if self.windows[index].id == "main" {
                self.windows[index].root = DockNode::Tabs(TabGroup::new(id, Vec::new()));
                index += 1;
            } else {
                self.windows.remove(index);
            }
        }
    }
}

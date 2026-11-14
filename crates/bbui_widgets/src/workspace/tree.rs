use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PaneId(pub String);

impl From<&str> for PaneId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for PaneId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabGroupOptions {
    pub show_single_tab: bool,
    pub reorderable: bool,
    pub movable: bool,
    pub accepts_drops: bool,
    pub detachable: bool,
}

impl Default for TabGroupOptions {
    fn default() -> Self {
        Self {
            show_single_tab: true,
            reorderable: true,
            movable: true,
            accepts_drops: true,
            detachable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabGroup {
    pub id: String,
    pub panes: Vec<PaneId>,
    pub active: Option<PaneId>,
    #[serde(default)]
    pub options: TabGroupOptions,
}

impl TabGroup {
    pub fn new(id: impl Into<String>, panes: Vec<PaneId>) -> Self {
        let active = panes.first().cloned();
        Self {
            id: id.into(),
            panes,
            active,
            options: TabGroupOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DockNode {
    Tabs(TabGroup),
    Split {
        id: String,
        axis: SplitAxis,
        children: Vec<DockNode>,
        shares: Vec<f32>,
    },
    Grid {
        id: String,
        columns: usize,
        children: Vec<DockNode>,
    },
}

impl DockNode {
    pub fn group(&self, id: &str) -> Option<&TabGroup> {
        match self {
            Self::Tabs(group) => (group.id == id).then_some(group),
            Self::Split { children, .. } | Self::Grid { children, .. } => {
                children.iter().find_map(|child| child.group(id))
            }
        }
    }

    pub(super) fn group_mut(&mut self, id: &str) -> Option<&mut TabGroup> {
        match self {
            Self::Tabs(group) => (group.id == id).then_some(group),
            Self::Split { children, .. } | Self::Grid { children, .. } => {
                children.iter_mut().find_map(|child| child.group_mut(id))
            }
        }
    }

    pub(super) fn remove_pane(&mut self, pane: &PaneId) -> bool {
        match self {
            Self::Tabs(group) => {
                let Some(index) = group.panes.iter().position(|candidate| candidate == pane) else {
                    return false;
                };
                group.panes.remove(index);
                if group.active.as_ref() == Some(pane) {
                    group.active = group
                        .panes
                        .get(index)
                        .or_else(|| group.panes.last())
                        .cloned();
                }
                true
            }
            Self::Split { children, .. } | Self::Grid { children, .. } => {
                children.iter_mut().any(|child| child.remove_pane(pane))
            }
        }
    }

    pub(super) fn insert_split(
        &mut self,
        target: &str,
        id: &str,
        axis: SplitAxis,
        after: bool,
        group: TabGroup,
    ) -> bool {
        if matches!(self, Self::Tabs(existing) if existing.id == target) {
            let old = std::mem::replace(self, Self::Tabs(TabGroup::new("", Vec::new())));
            let fresh = Self::Tabs(group);
            let children = if after {
                vec![old, fresh]
            } else {
                vec![fresh, old]
            };
            *self = Self::Split {
                id: id.to_owned(),
                axis,
                children,
                shares: vec![1.0, 1.0],
            };
            return true;
        }
        match self {
            Self::Split { children, .. } | Self::Grid { children, .. } => children
                .iter_mut()
                .any(|child| child.insert_split(target, id, axis, after, group.clone())),
            Self::Tabs(_) => false,
        }
    }

    pub(super) fn compact(&mut self) -> bool {
        match self {
            Self::Tabs(group) => !group.panes.is_empty(),
            Self::Split {
                children, shares, ..
            } => {
                let mut index = 0;
                while index < children.len() {
                    if children[index].compact() {
                        index += 1;
                    } else {
                        children.remove(index);
                        shares.remove(index);
                    }
                }
                let nonempty = !children.is_empty();
                if children.len() == 1 {
                    *self = children.remove(0);
                }
                nonempty
            }
            Self::Grid { children, .. } => {
                children.retain_mut(DockNode::compact);
                let nonempty = !children.is_empty();
                if children.len() == 1 {
                    *self = children.remove(0);
                }
                nonempty
            }
        }
    }

    pub(super) fn validate(
        &self,
        ids: &mut HashSet<String>,
        panes: &mut HashSet<PaneId>,
    ) -> Result<(), WorkspaceError> {
        let id = match self {
            Self::Tabs(group) => &group.id,
            Self::Split { id, .. } | Self::Grid { id, .. } => id,
        };
        if id.is_empty() || !ids.insert(id.clone()) {
            return Err(WorkspaceError::DuplicateOrEmptyId);
        }
        match self {
            Self::Tabs(group) => {
                if group
                    .active
                    .as_ref()
                    .is_some_and(|active| !group.panes.contains(active))
                    || (group.active.is_none() && !group.panes.is_empty())
                {
                    return Err(WorkspaceError::InvalidSelection);
                }
                for pane in &group.panes {
                    if pane.0.is_empty() || !panes.insert(pane.clone()) {
                        return Err(WorkspaceError::DuplicateOrEmptyId);
                    }
                }
            }
            Self::Split {
                children, shares, ..
            } => {
                if children.len() < 2
                    || children.len() != shares.len()
                    || shares
                        .iter()
                        .any(|share| !share.is_finite() || *share <= 0.0)
                {
                    return Err(WorkspaceError::InvalidContainer);
                }
                for child in children {
                    child.validate(ids, panes)?;
                }
            }
            Self::Grid {
                columns, children, ..
            } => {
                if *columns == 0 || children.is_empty() {
                    return Err(WorkspaceError::InvalidContainer);
                }
                for child in children {
                    child.validate(ids, panes)?;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn find_node_mut<'a>(node: &'a mut DockNode, id: &str) -> Option<&'a mut DockNode> {
    let matches = match node {
        DockNode::Tabs(group) => group.id == id,
        DockNode::Split { id: own, .. } | DockNode::Grid { id: own, .. } => own == id,
    };
    if matches {
        return Some(node);
    }
    match node {
        DockNode::Tabs(_) => None,
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => children
            .iter_mut()
            .find_map(|child| find_node_mut(child, id)),
    }
}

pub(super) fn containing_group<'a>(node: &'a DockNode, pane: &PaneId) -> Option<&'a TabGroup> {
    match node {
        DockNode::Tabs(group) => group.panes.contains(pane).then_some(group),
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => children
            .iter()
            .find_map(|child| containing_group(child, pane)),
    }
}

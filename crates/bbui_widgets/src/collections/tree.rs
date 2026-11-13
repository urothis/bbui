use super::{ListItem, ListSelection};
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeNode<T> {
    pub id: u64,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub value: T,
    pub expanded: bool,
    pub enabled: bool,
}

#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[component(summary_tick)]
pub struct TreeModel<T> {
    pub nodes: Vec<TreeNode<T>>,
    pub selection: ListSelection,
    pub row_height: u32,
    pub scroll_rows: usize,
}

impl<T> Default for TreeModel<T> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            selection: ListSelection::default(),
            row_height: 22,
            scroll_rows: 0,
        }
    }
}

impl<T> TreeModel<T> {
    pub fn visible_ids(&self) -> Vec<u64> {
        let lookup: HashMap<u64, &TreeNode<T>> =
            self.nodes.iter().map(|node| (node.id, node)).collect();
        let roots: Vec<u64> = self
            .nodes
            .iter()
            .filter(|node| node.parent.is_none())
            .map(|node| node.id)
            .collect();
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack: Vec<u64> = roots.into_iter().rev().collect();
        while let Some(id) = stack.pop() {
            let Some(node) = lookup.get(&id) else {
                continue;
            };
            out.push(id);
            if node.expanded {
                stack.extend(node.children.iter().rev().copied());
            }
        }
        out
    }

    pub fn toggle_expanded(&mut self, id: u64) -> bool {
        let Some(node) = self
            .nodes
            .iter_mut()
            .find(|node| node.id == id && !node.children.is_empty())
        else {
            return false;
        };
        node.expanded = !node.expanded;
        true
    }

    pub fn select(&mut self, id: u64, ctrl: bool, shift: bool) -> bool {
        let visible: Vec<_> = self
            .visible_ids()
            .into_iter()
            .filter_map(|id| self.nodes.iter().find(|node| node.id == id))
            .map(|node| ListItem {
                id: node.id,
                value: (),
                enabled: node.enabled,
            })
            .collect();
        self.selection.select(&visible, id, ctrl, shift)
    }

    pub fn visible_range(
        &self,
        scroll_rows: usize,
        viewport_rows: usize,
    ) -> std::ops::Range<usize> {
        let count = self.visible_ids().len();
        scroll_rows.min(count)..scroll_rows.saturating_add(viewport_rows).min(count)
    }

    pub fn move_active(&mut self, delta: i32, extend: bool) -> Option<u64> {
        let visible_ids = self.visible_ids();
        if visible_ids.is_empty() || delta == 0 {
            return self.selection.active;
        }
        let current = self
            .selection
            .active
            .and_then(|active| visible_ids.iter().position(|id| *id == active));
        let start = current.map_or(
            if delta > 0 {
                -1
            } else {
                visible_ids.len() as i64
            },
            |index| index as i64,
        );
        let direction = delta.signum() as i64;
        let mut at = start;
        for _ in 0..visible_ids.len() {
            at = (at + direction).clamp(0, visible_ids.len() as i64 - 1);
            let id = visible_ids[at as usize];
            if self
                .nodes
                .iter()
                .find(|node| node.id == id)
                .is_some_and(|node| node.enabled)
            {
                self.select(id, false, extend);
                return Some(id);
            }
        }
        None
    }

    pub fn depth(&self, id: u64) -> usize {
        let lookup: HashMap<u64, &TreeNode<T>> =
            self.nodes.iter().map(|node| (node.id, node)).collect();
        let mut depth = 0;
        let mut parent = lookup.get(&id).and_then(|node| node.parent);
        while let Some(parent_id) = parent {
            depth += 1;
            parent = lookup.get(&parent_id).and_then(|node| node.parent);
            if depth > self.nodes.len() {
                break;
            }
        }
        depth
    }

    pub fn reorder_sibling(&mut self, id: u64, index: usize) -> bool {
        let Some(parent) = self
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.parent)
        else {
            return false;
        };
        self.move_node(id, parent, index)
    }

    pub fn move_node(&mut self, id: u64, new_parent: Option<u64>, index: usize) -> bool {
        let Some(node_index) = self.nodes.iter().position(|node| node.id == id) else {
            return false;
        };
        if new_parent.is_some_and(|parent| parent == id || self.is_descendant(parent, id))
            || new_parent.is_some_and(|parent| !self.nodes.iter().any(|node| node.id == parent))
        {
            return false;
        }
        let old_parent = self.nodes[node_index].parent;
        if old_parent == new_parent {
            if let Some(parent) = old_parent {
                let Some(parent_node) = self.nodes.iter_mut().find(|node| node.id == parent) else {
                    return false;
                };
                let Some(old_index) = parent_node.children.iter().position(|child| *child == id)
                else {
                    return false;
                };
                let child = parent_node.children.remove(old_index);
                let destination = index.min(parent_node.children.len());
                let changed = old_index != destination;
                parent_node.children.insert(destination, child);
                return changed;
            }
            return self.reorder_root(id, index);
        }
        if let Some(parent) = old_parent
            && let Some(parent_node) = self.nodes.iter_mut().find(|node| node.id == parent)
        {
            parent_node.children.retain(|child| *child != id);
        }
        self.nodes[node_index].parent = new_parent;
        if let Some(parent) = new_parent {
            let Some(parent_node) = self.nodes.iter_mut().find(|node| node.id == parent) else {
                return false;
            };
            let destination = index.min(parent_node.children.len());
            parent_node.children.insert(destination, id);
        } else {
            self.reorder_root(id, index);
        }
        true
    }

    pub fn rename(&mut self, id: u64, value: T) -> bool
    where
        T: PartialEq,
    {
        let Some(node) = self.nodes.iter_mut().find(|node| node.id == id) else {
            return false;
        };
        if node.value == value {
            return false;
        }
        node.value = value;
        true
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.row_height == 0 {
            return Err("tree row height must be positive".to_owned());
        }
        let ids: BTreeSet<_> = self.nodes.iter().map(|node| node.id).collect();
        if ids.len() != self.nodes.len() {
            return Err("tree node ids must be unique".to_owned());
        }
        for node in &self.nodes {
            if node.id == 0 {
                return Err("tree node ids must be nonzero".to_owned());
            }
            if let Some(parent) = node.parent
                && (!ids.contains(&parent)
                    || self
                        .nodes
                        .iter()
                        .find(|candidate| candidate.id == parent)
                        .is_none_or(|candidate| !candidate.children.contains(&node.id)))
            {
                return Err("tree parent and child references must agree".to_owned());
            }
            let children: BTreeSet<_> = node.children.iter().copied().collect();
            if children.len() != node.children.len()
                || node.children.iter().any(|child| {
                    !self.nodes.iter().any(|candidate| {
                        candidate.id == *child && candidate.parent == Some(node.id)
                    })
                })
            {
                return Err("tree child references must be unique and reciprocal".to_owned());
            }
            let mut ancestors = BTreeSet::new();
            let mut parent = node.parent;
            while let Some(parent_id) = parent {
                if parent_id == node.id || !ancestors.insert(parent_id) {
                    return Err("tree nodes cannot form a cycle".to_owned());
                }
                parent = self
                    .nodes
                    .iter()
                    .find(|candidate| candidate.id == parent_id)
                    .and_then(|candidate| candidate.parent);
            }
        }
        if self.selection.selected.iter().any(|id| !ids.contains(id))
            || self.selection.active.is_some_and(|id| !ids.contains(&id))
            || self.selection.anchor.is_some_and(|id| !ids.contains(&id))
        {
            return Err("tree selection references a missing node".to_owned());
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error>
    where
        T: Serialize,
    {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, String>
    where
        T: DeserializeOwned,
    {
        let model: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        model.validate()?;
        Ok(model)
    }

    fn is_descendant(&self, node_id: u64, ancestor_id: u64) -> bool {
        let mut parent = self
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .and_then(|node| node.parent);
        while let Some(parent_id) = parent {
            if parent_id == ancestor_id {
                return true;
            }
            parent = self
                .nodes
                .iter()
                .find(|node| node.id == parent_id)
                .and_then(|node| node.parent);
        }
        false
    }

    fn reorder_root(&mut self, id: u64, index: usize) -> bool {
        let Some(current_root_index) = self
            .nodes
            .iter()
            .filter(|node| node.parent.is_none())
            .position(|node| node.id == id)
        else {
            return false;
        };
        let Some(current) = self.nodes.iter().position(|node| node.id == id) else {
            return false;
        };
        let node = self.nodes.remove(current);
        let root_indices: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| node.parent.is_none().then_some(index))
            .collect();
        let destination = root_indices.get(index).copied().unwrap_or(self.nodes.len());
        let changed = current_root_index != index.min(root_indices.len());
        self.nodes.insert(destination, node);
        changed
    }
}

//! Data-only list, tree, and table models with identity-based selection and virtualization.

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem<T> {
    pub id: u64,
    pub value: T,
    pub enabled: bool,
}

#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListSelection {
    pub selected: BTreeSet<u64>,
    pub active: Option<u64>,
    pub anchor: Option<u64>,
}

impl ListSelection {
    pub fn clear(&mut self) {
        self.selected.clear();
        self.active = None;
        self.anchor = None;
    }

    pub fn contains(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    pub fn select<T>(&mut self, items: &[ListItem<T>], id: u64, ctrl: bool, shift: bool) -> bool {
        let Some(target_index) = items.iter().position(|item| item.id == id && item.enabled) else {
            return false;
        };
        let before = self.selected.clone();
        if shift {
            let anchor_index = self
                .anchor
                .and_then(|anchor| items.iter().position(|item| item.id == anchor))
                .unwrap_or(target_index);
            if !ctrl {
                self.selected.clear();
            }
            for item in &items[anchor_index.min(target_index)..=anchor_index.max(target_index)] {
                if item.enabled {
                    self.selected.insert(item.id);
                }
            }
        } else if ctrl {
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
            self.anchor = Some(id);
        } else {
            self.selected.clear();
            self.selected.insert(id);
            self.anchor = Some(id);
        }
        self.active = Some(id);
        before != self.selected
    }

    pub fn range(
        &self,
        item_count: usize,
        scroll_offset: f32,
        row_height: f32,
        viewport_height: f32,
    ) -> std::ops::Range<usize> {
        if item_count == 0 || row_height <= 0.0 || !row_height.is_finite() {
            return 0..0;
        }
        let start = (scroll_offset.max(0.0) / row_height).floor() as usize;
        let count = (viewport_height.max(0.0) / row_height).ceil() as usize + 1;
        start.min(item_count)..(start.saturating_add(count)).min(item_count)
    }
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListModel<T> {
    pub items: Vec<ListItem<T>>,
    pub selection: ListSelection,
    pub row_height: f32,
    pub scroll_offset: f32,
    pub multi_select: bool,
}

impl<T> Default for ListModel<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selection: ListSelection::default(),
            row_height: 22.0,
            scroll_offset: 0.0,
            multi_select: true,
        }
    }
}

impl<T> ListModel<T> {
    pub fn select(&mut self, id: u64, ctrl: bool, shift: bool) -> bool {
        self.selection.select(
            &self.items,
            id,
            self.multi_select && ctrl,
            self.multi_select && shift,
        )
    }

    pub fn move_active(&mut self, delta: i32, extend: bool) -> Option<u64> {
        if self.items.is_empty() || delta == 0 {
            return self.selection.active;
        }
        let start = self
            .selection
            .active
            .and_then(|active| self.items.iter().position(|item| item.id == active))
            .map_or(
                if delta > 0 {
                    -1
                } else {
                    self.items.len() as i64
                },
                |index| index as i64,
            );
        let direction = delta.signum() as i64;
        let mut at = start;
        for _ in 0..self.items.len() {
            at = (at + direction).clamp(0, self.items.len() as i64 - 1);
            let item = &self.items[at as usize];
            if item.enabled {
                if extend {
                    self.selection.select(&self.items, item.id, false, true);
                } else {
                    self.selection.select(&self.items, item.id, false, false);
                }
                return Some(item.id);
            }
        }
        None
    }

    pub fn visible_range(&self, viewport_height: f32) -> std::ops::Range<usize> {
        self.selection.range(
            self.items.len(),
            self.scroll_offset,
            self.row_height,
            viewport_height,
        )
    }

    pub fn visible_items(&self, viewport_height: f32) -> impl Iterator<Item = &ListItem<T>> {
        self.items[self.visible_range(viewport_height)].iter()
    }

    pub fn reorder(&mut self, id: u64, index: usize) -> bool {
        let Some(current) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        let item = self.items.remove(current);
        let destination = index.min(self.items.len());
        let changed = current != destination;
        self.items.insert(destination, item);
        changed
    }

    pub fn rename(&mut self, id: u64, value: T) -> bool
    where
        T: PartialEq,
    {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return false;
        };
        if item.value == value {
            return false;
        }
        item.value = value;
        true
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.row_height.is_finite() || self.row_height <= 0.0 {
            return Err("list row height must be finite and positive".to_owned());
        }
        if !self.scroll_offset.is_finite() || self.scroll_offset < 0.0 {
            return Err("list scroll offset must be finite and nonnegative".to_owned());
        }
        let ids: BTreeSet<_> = self.items.iter().map(|item| item.id).collect();
        if ids.len() != self.items.len() {
            return Err("list item ids must be unique".to_owned());
        }
        if self.selection.selected.iter().any(|id| !ids.contains(id))
            || self.selection.active.is_some_and(|id| !ids.contains(&id))
            || self.selection.anchor.is_some_and(|id| !ids.contains(&id))
        {
            return Err("list selection references a missing item".to_owned());
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
}

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewGridItem {
    pub id: u64,
    pub label: String,
    pub asset_id: Option<u64>,
    /// sRGB tint used for a registered thumbnail or a solid preview fallback.
    pub tint: [f32; 4],
    pub enabled: bool,
}

impl PreviewGridItem {
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            asset_id: None,
            tint: [0.34, 0.38, 0.46, 1.0],
            enabled: true,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewGrid {
    pub items: Vec<PreviewGridItem>,
    pub selection: ListSelection,
    pub cell_width: f32,
    pub cell_height: f32,
    pub columns: usize,
    pub scroll_rows: usize,
    pub multi_select: bool,
}

impl Default for PreviewGrid {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selection: ListSelection::default(),
            cell_width: 112.0,
            cell_height: 104.0,
            columns: 4,
            scroll_rows: 0,
            multi_select: true,
        }
    }
}

impl PreviewGrid {
    pub fn select(&mut self, id: u64, ctrl: bool, shift: bool) -> bool {
        let before = self.selection.clone();
        let items: Vec<_> = self
            .items
            .iter()
            .map(|item| ListItem {
                id: item.id,
                value: (),
                enabled: item.enabled,
            })
            .collect();
        self.selection.select(
            &items,
            id,
            self.multi_select && ctrl,
            self.multi_select && shift,
        );
        before != self.selection
    }

    pub fn move_active(&mut self, delta: i32, extend: bool) -> Option<u64> {
        let items: Vec<_> = self
            .items
            .iter()
            .map(|item| ListItem {
                id: item.id,
                value: (),
                enabled: item.enabled,
            })
            .collect();
        let start = self
            .selection
            .active
            .and_then(|active| items.iter().position(|item| item.id == active))
            .map_or(if delta > 0 { -1 } else { items.len() as i64 }, |index| {
                index as i64
            });
        if items.is_empty() || delta == 0 {
            return self.selection.active;
        }
        let mut at = start;
        for _ in 0..items.len() {
            at = (at + delta.signum() as i64).clamp(0, items.len() as i64 - 1);
            let item = &items[at as usize];
            if item.enabled {
                self.select(item.id, false, extend);
                return Some(item.id);
            }
        }
        None
    }

    pub fn visible_range(&self, viewport_height: f32) -> std::ops::Range<usize> {
        if self.items.is_empty() || self.columns == 0 || self.cell_height <= 0.0 {
            return 0..0;
        }
        let start_row = self.scroll_rows;
        let visible_rows = (viewport_height.max(0.0) / self.cell_height).ceil() as usize + 1;
        let start = start_row.saturating_mul(self.columns).min(self.items.len());
        let end = start_row
            .saturating_add(visible_rows)
            .saturating_mul(self.columns)
            .min(self.items.len());
        start..end
    }

    pub fn reorder(&mut self, id: u64, index: usize) -> bool {
        let Some(current) = self.items.iter().position(|item| item.id == id) else {
            return false;
        };
        let item = self.items.remove(current);
        let destination = index.min(self.items.len());
        let changed = current != destination;
        self.items.insert(destination, item);
        changed
    }

    pub fn rename(&mut self, id: u64, label: String) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return false;
        };
        if item.label == label {
            return false;
        }
        item.label = label;
        true
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.cell_width.is_finite()
            || !self.cell_height.is_finite()
            || self.cell_width < 40.0
            || self.cell_height < 40.0
            || self.columns == 0
        {
            return Err("preview grid cell size and columns must be positive".to_owned());
        }
        let ids: BTreeSet<_> = self.items.iter().map(|item| item.id).collect();
        if ids.len() != self.items.len() || ids.contains(&0) {
            return Err("preview grid item ids must be unique and nonzero".to_owned());
        }
        if self.items.iter().any(|item| {
            !item
                .tint
                .iter()
                .all(|channel| channel.is_finite() && (0.0..=1.0).contains(channel))
        }) {
            return Err("preview grid tints must be finite sRGB values in 0..=1".to_owned());
        }
        if self.selection.selected.iter().any(|id| !ids.contains(id))
            || self.selection.active.is_some_and(|id| !ids.contains(&id))
            || self.selection.anchor.is_some_and(|id| !ids.contains(&id))
        {
            return Err("preview grid selection references a missing item".to_owned());
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let grid: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        grid.validate()?;
        Ok(grid)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    pub label: String,
    pub width: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub sortable: bool,
}

impl TableColumn {
    pub fn new(label: impl Into<String>, width: f32) -> Self {
        Self {
            label: label.into(),
            width: width.max(24.0),
            min_width: 24.0,
            max_width: f32::MAX,
            sortable: true,
        }
    }

    pub fn resize(&mut self, width: f32) -> bool {
        if !width.is_finite() {
            return false;
        }
        let width = width.clamp(self.min_width.max(0.0), self.max_width.max(self.min_width));
        let changed = self.width != width;
        self.width = width;
        changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    pub id: u64,
    pub cells: Vec<String>,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTable {
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRow>,
    pub selection: ListSelection,
    pub sort: Option<(usize, bool)>,
    pub row_height: u32,
    pub scroll_rows: usize,
    pub scroll_x: f32,
    pub active_column: usize,
    pub multi_select: bool,
}

impl Default for DataTable {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            selection: ListSelection::default(),
            sort: None,
            row_height: 24,
            scroll_rows: 0,
            scroll_x: 0.0,
            active_column: 0,
            multi_select: true,
        }
    }
}

impl DataTable {
    pub fn sort_by(&mut self, column: usize, ascending: bool) -> bool {
        if !self
            .columns
            .get(column)
            .is_some_and(|column| column.sortable)
        {
            return false;
        }
        let numeric_column = !self.rows.is_empty()
            && self.rows.iter().all(|row| {
                row.cells
                    .get(column)
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .is_some_and(f64::is_finite)
            });
        self.rows.sort_by(|a, b| {
            let left = a.cells.get(column).map(String::as_str).unwrap_or("");
            let right = b.cells.get(column).map(String::as_str).unwrap_or("");
            let order = if numeric_column {
                let left = left.trim().parse::<f64>().unwrap_or_default();
                let right = right.trim().parse::<f64>().unwrap_or_default();
                left.total_cmp(&right)
            } else {
                left.to_lowercase().cmp(&right.to_lowercase())
            };
            let order = if ascending { order } else { order.reverse() };
            order.then_with(|| a.id.cmp(&b.id))
        });
        self.sort = Some((column, ascending));
        true
    }

    pub fn select(&mut self, id: u64, ctrl: bool, shift: bool) -> bool {
        let items: Vec<_> = self
            .rows
            .iter()
            .map(|row| ListItem {
                id: row.id,
                value: (),
                enabled: true,
            })
            .collect();
        self.selection.select(
            &items,
            id,
            self.multi_select && ctrl,
            self.multi_select && shift,
        )
    }

    pub fn select_cell(&mut self, id: u64, column: usize, ctrl: bool, shift: bool) -> bool {
        if column >= self.columns.len() {
            return false;
        }
        let old_column = self.active_column;
        self.active_column = column;
        self.select(id, ctrl, shift) || old_column != column
    }

    pub fn set_active_column(&mut self, column: usize) -> bool {
        if self.columns.is_empty() {
            return false;
        }
        let column = column.min(self.columns.len() - 1);
        let changed = self.active_column != column;
        self.active_column = column;
        changed
    }

    pub fn move_active_column(&mut self, delta: i32) -> Option<usize> {
        if self.columns.is_empty() || delta == 0 {
            return Some(self.active_column);
        }
        let next = (self.active_column as i64 + delta.signum() as i64)
            .clamp(0, self.columns.len() as i64 - 1) as usize;
        self.active_column = next;
        Some(next)
    }

    pub fn toggle_sort(&mut self, column: usize) -> bool {
        let ascending = self
            .sort
            .filter(|(sorted, _)| *sorted == column)
            .is_none_or(|(_, ascending)| !ascending);
        self.sort_by(column, ascending)
    }

    pub fn resize_column(&mut self, column: usize, width: f32) -> bool {
        self.columns
            .get_mut(column)
            .is_some_and(|column| column.resize(width))
    }

    pub fn select_all(&mut self) -> bool {
        let before = self.selection.clone();
        self.selection.selected = self.rows.iter().map(|row| row.id).collect();
        self.selection.active = self.rows.last().map(|row| row.id);
        self.selection.anchor = self.selection.active;
        before != self.selection
    }

    pub fn set_active_row(&mut self, id: u64, extend: bool) -> bool {
        if !self.rows.iter().any(|row| row.id == id) {
            return false;
        }
        let before = self.selection.clone();
        if extend {
            self.select(id, false, true);
        } else {
            self.select(id, false, false);
        }
        before != self.selection
    }

    pub fn move_active(&mut self, delta: i32, extend: bool) -> Option<u64> {
        if self.rows.is_empty() || delta == 0 {
            return self.selection.active;
        }
        let current = self
            .selection
            .active
            .and_then(|id| self.rows.iter().position(|row| row.id == id));
        let start = current.map_or(
            if delta > 0 {
                -1
            } else {
                self.rows.len() as i64
            },
            |index| index as i64,
        );
        let at = (start + i64::from(delta.signum())).clamp(0, self.rows.len() as i64 - 1) as usize;
        let id = self.rows[at].id;
        self.select(id, false, extend);
        Some(id)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.row_height == 0 {
            return Err("table row height must be positive".to_owned());
        }
        if self.columns.iter().any(|column| {
            !column.width.is_finite()
                || !column.min_width.is_finite()
                || !column.max_width.is_finite()
                || column.min_width < 1.0
                || column.max_width < column.min_width
                || column.width < column.min_width
                || column.width > column.max_width
        }) {
            return Err("table column widths must be finite and within their limits".to_owned());
        }
        if self
            .rows
            .iter()
            .any(|row| row.id == 0 || row.cells.len() != self.columns.len())
        {
            return Err(
                "table row ids must be nonzero and cell counts must match columns".to_owned(),
            );
        }
        let ids: BTreeSet<_> = self.rows.iter().map(|row| row.id).collect();
        if ids.len() != self.rows.len() {
            return Err("table row ids must be unique".to_owned());
        }
        if self.selection.selected.iter().any(|id| !ids.contains(id))
            || self.selection.active.is_some_and(|id| !ids.contains(&id))
            || self.selection.anchor.is_some_and(|id| !ids.contains(&id))
        {
            return Err("table selection references a missing row".to_owned());
        }
        if self.active_column >= self.columns.len() && !self.columns.is_empty() {
            return Err("table active column is out of range".to_owned());
        }
        if !self.scroll_x.is_finite() || self.scroll_x < 0.0 {
            return Err("table horizontal scroll offset must be finite and nonnegative".to_owned());
        }
        if self
            .sort
            .is_some_and(|(column, _)| column >= self.columns.len())
        {
            return Err("table sort column is out of range".to_owned());
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let table: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        table.validate()?;
        Ok(table)
    }

    pub fn visible_rows(&self, scroll_rows: usize, viewport_rows: usize) -> &[TableRow] {
        let start = scroll_rows.min(self.rows.len());
        let end = start.saturating_add(viewport_rows).min(self.rows.len());
        &self.rows[start..end]
    }
}

#[cfg(test)]
#[path = "collections/tests.rs"]
mod tests;

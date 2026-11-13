use super::{ListItem, ListSelection};
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
#[component(summary_tick)]
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

use super::{ListItem, ListSelection};
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
#[component(summary_tick)]
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

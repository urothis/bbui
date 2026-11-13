use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeSet;

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
#[component(summary_tick)]
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

use super::*;

/// Associates numeric image IDs in [`Primitive::Image`] with Bevy image assets.
#[derive(Resource)]
pub struct ImageRegistry {
    pub(super) images: HashMap<u64, Handle<Image>>,
    pub(super) next_id: u64,
    pub(super) changed_ids: HashSet<u64>,
}

impl Default for ImageRegistry {
    fn default() -> Self {
        Self {
            images: HashMap::new(),
            next_id: 1,
            changed_ids: HashSet::new(),
        }
    }
}

impl ImageRegistry {
    /// Allocate a monotonically increasing, nonzero image ID not already in use.
    ///
    /// IDs registered explicitly with [`insert`](Self::insert) are skipped. `None` is returned
    /// only if every nonzero `u64` is currently registered.
    pub fn allocate_id(&mut self) -> Option<u64> {
        let start = self.next_id.max(1);
        let mut candidate = start;
        loop {
            if !self.images.contains_key(&candidate) {
                self.next_id = candidate.wrapping_add(1).max(1);
                return Some(candidate);
            }
            candidate = candidate.wrapping_add(1).max(1);
            if candidate == start {
                return None;
            }
        }
    }

    /// Register or replace the Bevy image referenced by `asset_id`.
    pub fn insert(&mut self, asset_id: u64, image: Handle<Image>) -> Option<Handle<Image>> {
        let changed = self.images.get(&asset_id) != Some(&image);
        let previous = self.images.insert(asset_id, image);
        if changed {
            self.changed_ids.insert(asset_id);
        }
        previous
    }

    pub fn remove(&mut self, asset_id: u64) -> Option<Handle<Image>> {
        let previous = self.images.remove(&asset_id);
        if previous.is_some() {
            self.changed_ids.insert(asset_id);
        }
        previous
    }

    pub fn get(&self, asset_id: u64) -> Option<&Handle<Image>> {
        self.images.get(&asset_id)
    }

    pub(super) fn pending_changed_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.changed_ids.iter().copied()
    }

    pub(super) fn take_changed_ids(&mut self) -> HashSet<u64> {
        std::mem::take(&mut self.changed_ids)
    }
}

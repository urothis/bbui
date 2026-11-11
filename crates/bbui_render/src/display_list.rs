use super::*;

#[derive(Default, Debug, Resource)]
pub struct DisplayList {
    pub(super) manual_items: Vec<DrawItem>,
    pub(super) widget_items: Vec<DrawItem>,
    pub(super) overlay_widget_items: Vec<DrawItem>,
    pub(super) camera_items: HashMap<Entity, Vec<DrawItem>>,
    pub(super) overlay_camera_items: HashMap<Entity, Vec<DrawItem>>,
    pub(super) widget_fragments: HashMap<Entity, WidgetRenderFragment>,
    pub(super) widget_fragment_batch_groups: HashMap<Entity, WidgetFragmentBatchGroup>,
    pub(super) widget_batch_group_members: HashMap<WidgetFragmentBatchGroup, HashSet<Entity>>,
    pub(super) pending_widget_batch_group_changes: HashSet<WidgetFragmentBatchGroup>,
    pub(super) pending_widget_fragment_changes: HashSet<Entity>,
    pub(super) pending_full_change: bool,
    pub(super) widget_items_view: OnceLock<Vec<DrawItem>>,
    pub(super) camera_items_view: OnceLock<HashMap<Entity, Vec<DrawItem>>>,
    pub(super) items_view: OnceLock<Vec<DrawItem>>,
    pub(super) item_count: usize,
}

impl DisplayList {
    pub fn push(&mut self, item: DrawItem) {
        self.manual_items.push(item);
        self.item_count = self.item_count.saturating_add(1);
        self.mark_full_change();
    }

    /// Replace the widget-generated layer while preserving manually pushed draw items.
    pub fn replace_widget_items(&mut self, items: Vec<DrawItem>) {
        if self.widget_items != items {
            self.item_count = self
                .item_count
                .saturating_sub(self.widget_items.len())
                .saturating_add(items.len());
            self.widget_items = items;
            self.mark_full_change();
        }
    }

    /// Replace a post-widget overlay for the screen UI layer without overwriting base or manual
    /// items. Overlay items sort after the base widget layer at equal z.
    pub fn replace_overlay_widget_items(&mut self, items: Vec<DrawItem>) {
        if self.overlay_widget_items != items {
            self.item_count = self
                .item_count
                .saturating_sub(self.overlay_widget_items.len())
                .saturating_add(items.len());
            self.overlay_widget_items = items;
            self.mark_full_change();
        }
    }

    /// Publish or replace one widget's ordered paint fragment.
    ///
    /// Returns `true` only when the fragment differs from the previously published fragment.
    /// This records a per-entity delta and does not flatten or sort the complete display list.
    pub fn upsert_widget_fragment(&mut self, fragment: WidgetRenderFragment) -> bool {
        self.upsert_widget_fragment_with_batch_group(fragment, None)
    }

    /// Recolor one retained rounded rectangle without rebuilding its fragment.
    /// Returns `None` when the fragment or primitive no longer matches.
    pub fn recolor_widget_fragment_rounded_rect(
        &mut self,
        entity: Entity,
        item_index: usize,
        next: Rgba,
    ) -> Option<bool> {
        let item = self
            .widget_fragments
            .get_mut(&entity)?
            .items
            .get_mut(item_index)?;
        let Primitive::RoundedRect { color, .. } = &mut item.primitive else {
            return None;
        };
        if *color == next {
            return Some(false);
        }
        *color = next;
        self.pending_widget_fragment_changes.insert(entity);
        self.invalidate_views();
        Some(true)
    }

    /// Publish a widget fragment with optional membership in a bounded cross-fragment Bezier
    /// batch. The group is metadata only; fragment equality remains local to its items/order.
    pub fn upsert_widget_fragment_with_batch_group(
        &mut self,
        fragment: WidgetRenderFragment,
        batch_group: Option<WidgetFragmentBatchGroup>,
    ) -> bool {
        let entity = fragment.entity;
        let group_changed = self.widget_fragment_batch_groups.get(&entity).copied() != batch_group;
        if group_changed {
            if let Some(previous) = self.widget_fragment_batch_groups.remove(&entity) {
                self.pending_widget_batch_group_changes.insert(previous);
                if let Some(members) = self.widget_batch_group_members.get_mut(&previous) {
                    members.remove(&entity);
                    if members.is_empty() {
                        self.widget_batch_group_members.remove(&previous);
                    }
                }
            }
            if let Some(group) = batch_group {
                self.widget_fragment_batch_groups.insert(entity, group);
                self.widget_batch_group_members
                    .entry(group)
                    .or_default()
                    .insert(entity);
                self.pending_widget_batch_group_changes.insert(group);
            }
            self.pending_widget_fragment_changes.insert(entity);
        }
        if self.widget_fragments.get(&fragment.entity) == Some(&fragment) {
            if group_changed {
                self.invalidate_views();
            }
            return group_changed;
        }
        let new_len = fragment.items.len();
        if let Some(previous) = self.widget_fragments.insert(entity, fragment) {
            self.item_count = self.item_count.saturating_sub(previous.items.len());
        }
        self.item_count = self.item_count.saturating_add(new_len);
        self.pending_widget_fragment_changes.insert(entity);
        self.invalidate_views();
        true
    }

    /// Remove a previously published widget fragment.
    ///
    /// Returns `true` only when a fragment existed. The renderer consumes this as a targeted
    /// removal and leaves other widget fragments untouched.
    pub fn remove_widget_fragment(&mut self, entity: Entity) -> bool {
        let Some(previous) = self.widget_fragments.remove(&entity) else {
            return false;
        };
        if let Some(group) = self.widget_fragment_batch_groups.remove(&entity) {
            self.pending_widget_batch_group_changes.insert(group);
            if let Some(members) = self.widget_batch_group_members.get_mut(&group) {
                members.remove(&entity);
                if members.is_empty() {
                    self.widget_batch_group_members.remove(&group);
                }
            }
        }
        self.item_count = self.item_count.saturating_sub(previous.items.len());
        self.pending_widget_fragment_changes.insert(entity);
        self.invalidate_views();
        true
    }

    /// Read the currently published keyed widget fragments without materializing a merged list.
    pub fn widget_fragments(&self) -> &HashMap<Entity, WidgetRenderFragment> {
        &self.widget_fragments
    }

    pub fn widget_fragment_batch_group(&self, entity: Entity) -> Option<WidgetFragmentBatchGroup> {
        self.widget_fragment_batch_groups.get(&entity).copied()
    }

    pub fn widget_batch_group_members(
        &self,
        group: WidgetFragmentBatchGroup,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.widget_batch_group_members
            .get(&group)
            .into_iter()
            .flat_map(|members| members.iter().copied())
    }

    /// Number of draw items across the merged display list, computed without allocating a view.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    pub(crate) fn has_pending_full_change(&self) -> bool {
        self.pending_full_change
    }

    pub(crate) fn pending_widget_fragment_changes(&self) -> impl Iterator<Item = Entity> + '_ {
        self.pending_widget_fragment_changes.iter().copied()
    }

    pub(crate) fn take_widget_fragment_changes(&mut self) -> Vec<Entity> {
        let mut entities: Vec<_> = self.pending_widget_fragment_changes.drain().collect();
        entities.sort_unstable_by_key(|entity| entity.to_bits());
        entities
    }

    pub(crate) fn take_widget_batch_group_changes(&mut self) -> Vec<WidgetFragmentBatchGroup> {
        let mut groups: Vec<_> = self.pending_widget_batch_group_changes.drain().collect();
        groups.sort_unstable_by_key(|group| (group.owner.to_bits(), group.chunk));
        groups
    }

    pub(crate) fn take_pending_full_change(&mut self) -> bool {
        std::mem::take(&mut self.pending_full_change)
    }

    /// Alias for [`replace_widget_items`](Self::replace_widget_items).
    pub fn set_widget_items(&mut self, items: Vec<DrawItem>) {
        self.replace_widget_items(items);
    }

    /// Replace draw items for a target camera. `None` is Bevy's default screen UI camera.
    ///
    /// `Some(camera)` keeps this layer independent from screen items and routes its generated
    /// nodes to that camera. Passing an empty vector removes the camera's layer.
    pub fn replace_camera_items(&mut self, camera: Option<Entity>, items: Vec<DrawItem>) {
        match camera {
            None => self.replace_widget_items(items),
            Some(camera) => {
                let previous_len = self.camera_items.get(&camera).map_or(0, Vec::len);
                let changed = if items.is_empty() {
                    self.camera_items.remove(&camera).is_some()
                } else if self.camera_items.get(&camera) != Some(&items) {
                    self.camera_items.insert(camera, items);
                    true
                } else {
                    false
                };
                if changed {
                    self.item_count = self
                        .item_count
                        .saturating_sub(previous_len)
                        .saturating_add(self.camera_items.get(&camera).map_or(0, Vec::len));
                    self.mark_full_change();
                }
            }
        }
    }

    /// Replace an auxiliary layer for one camera without overwriting its widget-generated items.
    ///
    /// This is intended for systems that add renderer-backed content after the main widget paint
    /// pass, such as thumbnails owned by a virtualized browser. Its items join the same stable z
    /// ordering as camera items, after the base camera layer at equal z.
    pub fn replace_overlay_camera_items(&mut self, camera: Entity, items: Vec<DrawItem>) {
        let previous_len = self.overlay_camera_items.get(&camera).map_or(0, Vec::len);
        let changed = if items.is_empty() {
            self.overlay_camera_items.remove(&camera).is_some()
        } else if self.overlay_camera_items.get(&camera) != Some(&items) {
            self.overlay_camera_items.insert(camera, items);
            true
        } else {
            false
        };
        if changed {
            self.item_count = self
                .item_count
                .saturating_sub(previous_len)
                .saturating_add(self.overlay_camera_items.get(&camera).map_or(0, Vec::len));
            self.mark_full_change();
        }
    }

    /// Read the widget-generated layer before z-order merging.
    pub fn widget_items(&self) -> &[DrawItem] {
        self.widget_items_view.get_or_init(|| {
            let mut fragments: Vec<_> = self
                .widget_fragments
                .values()
                .filter(|fragment| fragment.target_camera.is_none())
                .collect();
            fragments.sort_unstable_by_key(|fragment| (fragment.order, fragment.entity.to_bits()));
            let mut items = Vec::new();
            for fragment in fragments {
                items.extend(fragment.items.iter().cloned());
            }
            items.extend(self.widget_items.iter().cloned());
            items
        })
    }

    /// Read the screen UI overlay layer before z-order merging.
    pub fn overlay_widget_items(&self) -> &[DrawItem] {
        &self.overlay_widget_items
    }

    pub fn camera_items(&self, camera: Entity) -> Option<&[DrawItem]> {
        self.camera_items_view
            .get_or_init(|| {
                let mut view = self.camera_items.clone();
                let mut fragments: Vec<_> = self
                    .widget_fragments
                    .values()
                    .filter(|fragment| fragment.target_camera.is_some())
                    .collect();
                fragments
                    .sort_unstable_by_key(|fragment| (fragment.order, fragment.entity.to_bits()));
                for fragment in fragments {
                    if let Some(camera) = fragment.target_camera {
                        view.entry(camera)
                            .or_default()
                            .extend(fragment.items.iter().cloned());
                    }
                }
                view
            })
            .get(&camera)
            .map(Vec::as_slice)
    }

    /// Read the auxiliary camera layer used for post-widget GPU overlays.
    pub fn overlay_camera_items(&self, camera: Entity) -> Option<&[DrawItem]> {
        self.overlay_camera_items.get(&camera).map(Vec::as_slice)
    }

    /// Read manually pushed items before z-order merging.
    pub fn manual_items(&self) -> &[DrawItem] {
        &self.manual_items
    }

    pub fn ordered(&mut self) -> &[DrawItem] {
        let mut items = self.flatten_items();
        items.sort_by_key(|item| item.z);
        self.items_view.take();
        let _ = self.items_view.set(items);
        self.items_view.get().map(Vec::as_slice).unwrap_or_default()
    }

    /// Read the draw items in ascending z order.
    pub fn items(&self) -> &[DrawItem] {
        self.items_view
            .get_or_init(|| self.flatten_items())
            .as_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.item_count() == 0
    }

    pub fn clear(&mut self) {
        self.manual_items.clear();
        self.widget_items.clear();
        self.overlay_widget_items.clear();
        self.camera_items.clear();
        self.overlay_camera_items.clear();
        self.widget_fragments.clear();
        self.pending_widget_fragment_changes.clear();
        self.item_count = 0;
        self.mark_full_change();
    }

    pub(super) fn mark_full_change(&mut self) {
        self.pending_full_change = true;
        self.invalidate_views();
    }

    pub(super) fn invalidate_views(&mut self) {
        self.widget_items_view.take();
        self.camera_items_view.take();
        self.items_view.take();
    }

    pub(super) fn flatten_items(&self) -> Vec<DrawItem> {
        let mut tagged = Vec::with_capacity(self.item_count());
        tagged.extend(
            self.manual_items
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, item)| (item.z, 0u8, 0u64, 0u64, 0u64, index as u64, 0u64, item)),
        );
        tagged.extend(
            self.widget_items
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, item)| (item.z, 2, 1, index as u64, 0, 0, 0, item)),
        );
        let mut fragments: Vec<_> = self
            .widget_fragments
            .values()
            .filter(|fragment| fragment.target_camera.is_none())
            .collect();
        fragments.sort_unstable_by_key(|fragment| (fragment.order, fragment.entity.to_bits()));
        for fragment in fragments {
            tagged.extend(
                fragment
                    .items
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(index, item)| {
                        (
                            item.z,
                            2,
                            0,
                            fragment.order,
                            fragment.entity.to_bits(),
                            index as u64,
                            0,
                            item,
                        )
                    }),
            );
        }
        tagged.extend(
            self.overlay_widget_items
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, item)| (item.z, 3, 0, index as u64, 0, 0, 0, item)),
        );
        let mut cameras: Vec<_> = self
            .camera_items
            .keys()
            .copied()
            .chain(self.overlay_camera_items.keys().copied())
            .chain(
                self.widget_fragments
                    .values()
                    .filter_map(|fragment| fragment.target_camera),
            )
            .collect();
        cameras.sort_unstable_by_key(|camera| camera.to_bits());
        cameras.dedup();
        for (camera_rank, camera) in cameras.into_iter().enumerate() {
            if let Some(items) = self.camera_items.get(&camera) {
                tagged.extend(items.iter().cloned().enumerate().map(|(index, item)| {
                    (item.z, 4, 0, camera_rank as u64, 0, index as u64, 0, item)
                }));
            }
            let mut fragments: Vec<_> = self
                .widget_fragments
                .values()
                .filter(|fragment| fragment.target_camera == Some(camera))
                .collect();
            fragments.sort_unstable_by_key(|fragment| (fragment.order, fragment.entity.to_bits()));
            for fragment in fragments {
                tagged.extend(
                    fragment
                        .items
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(index, item)| {
                            (
                                item.z,
                                4,
                                1,
                                camera_rank as u64,
                                fragment.order,
                                fragment.entity.to_bits(),
                                index as u64,
                                item,
                            )
                        }),
                );
            }
            if let Some(items) = self.overlay_camera_items.get(&camera) {
                tagged.extend(items.iter().cloned().enumerate().map(|(index, item)| {
                    (item.z, 5, 0, camera_rank as u64, 0, index as u64, 0, item)
                }));
            }
        }
        tagged.sort_unstable_by_key(
            |(z, layer, sublayer, major, minor, tertiary, quaternary, _)| {
                (
                    *z,
                    *layer,
                    *sublayer,
                    *major,
                    *minor,
                    *tertiary,
                    *quaternary,
                )
            },
        );
        tagged
            .into_iter()
            .map(|(_, _, _, _, _, _, _, item)| item)
            .collect()
    }
}

#[cfg(test)]
mod retained_tests {
    use super::*;

    #[test]
    fn recoloring_one_primitive_keeps_text_and_publishes_one_delta() {
        let entity = Entity::PLACEHOLDER;
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 24.0,
        };
        let initial = Rgba(0.2, 0.2, 0.2, 1.0);
        let changed = Rgba(0.4, 0.3, 0.2, 1.0);
        let mut display = DisplayList::default();
        display.upsert_widget_fragment(WidgetRenderFragment {
            entity,
            order: 1,
            target_camera: None,
            items: vec![
                DrawItem {
                    clip,
                    z: 1,
                    primitive: Primitive::RoundedRect {
                        rect: clip,
                        radius: 3.0,
                        color: initial,
                    },
                },
                DrawItem {
                    clip,
                    z: 2,
                    primitive: Primitive::Text {
                        origin: [24.0, 0.0],
                        text: "stable".into(),
                        color: initial,
                    },
                },
            ],
        });
        display.take_widget_fragment_changes();
        let text_address = match &display.widget_fragments()[&entity].items[1].primitive {
            Primitive::Text { text, .. } => text.as_ptr(),
            _ => panic!("expected text"),
        };
        assert_eq!(
            display.recolor_widget_fragment_rounded_rect(entity, 0, changed),
            Some(true)
        );
        assert_eq!(display.take_widget_fragment_changes(), vec![entity]);
        assert_eq!(display.item_count(), 2);
        assert_eq!(
            display.recolor_widget_fragment_rounded_rect(entity, 0, changed),
            Some(false)
        );
        assert!(display.take_widget_fragment_changes().is_empty());
        assert_eq!(
            display.recolor_widget_fragment_rounded_rect(entity, 1, changed),
            None
        );
        let new_text_address = match &display.widget_fragments()[&entity].items[1].primitive {
            Primitive::Text { text, .. } => text.as_ptr(),
            _ => panic!("expected text"),
        };
        assert_eq!(new_text_address, text_address);
    }
}

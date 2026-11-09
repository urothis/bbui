//! Editable canvas geometry in logical coordinates. Rendering and input use the
//! same world/screen transform and object order.

use std::collections::{BTreeSet, HashMap, HashSet};

use bevy_asset::{AssetServer, LoadState};
use bevy_ecs::prelude::{Component, Entity, Resource, With, World};
use bevy_image::Image;
use bui_core::{
    UiChildren, UiDirty, UiDirtyDomain, UiEnabled, UiFocusable, UiKey, UiKeyCode, UiModifiers,
    UiNode, UiParent, UiPointerButton, UiRect, UiRoutedEvent, UiRoutedEvents, UiVisible,
    UiWheelUnit, detach_from_parent, set_parent,
};
use bui_layout::UiLayoutResult;
use bui_render::{ClipRect, DrawItem, ImageRegistry, Primitive};
use bui_style::{Rgba, Theme, ThemeColorToken};
use serde::{Deserialize, Serialize};

use crate::actions::{WidgetEvent, WidgetEventQueue};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanvasShape {
    Rectangle { size: [f32; 2] },
    Ellipse { size: [f32; 2] },
    Line { end: [f32; 2] },
    Image { size: [f32; 2], asset: String },
}

impl CanvasShape {
    fn valid(&self) -> bool {
        match self {
            Self::Rectangle { size } | Self::Ellipse { size } => {
                size.iter().all(|v| v.is_finite() && *v > 0.0)
            }
            Self::Image { size, asset } => {
                !asset.is_empty()
                    && std::path::Path::new(asset)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                    && size.iter().all(|v| v.is_finite() && *v > 0.0)
            }
            Self::Line { end } => end.iter().all(|v| v.is_finite()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasObject {
    pub id: u64,
    pub origin: [f32; 2],
    pub shape: CanvasShape,
    /// Linear RGBA; image objects use this as a tint.
    pub color: [f32; 4],
    pub visible: bool,
    pub locked: bool,
}

impl CanvasObject {
    fn valid(&self) -> bool {
        let bounds = self.bounds();
        self.id != 0
            && self.origin.iter().all(|v| v.is_finite())
            && self
                .color
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self.shape.valid()
            && bounds.is_finite()
            && bounds.right().is_finite()
            && bounds.bottom().is_finite()
    }

    pub fn bounds(&self) -> UiRect {
        match &self.shape {
            CanvasShape::Rectangle { size }
            | CanvasShape::Ellipse { size }
            | CanvasShape::Image { size, .. } => {
                UiRect::new(self.origin[0], self.origin[1], size[0], size[1])
            }
            CanvasShape::Line { end } => UiRect::new(
                self.origin[0].min(end[0]),
                self.origin[1].min(end[1]),
                (self.origin[0] - end[0]).abs(),
                (self.origin[1] - end[1]).abs(),
            ),
        }
    }

    fn hit(&self, point: [f32; 2], tolerance: f32) -> bool {
        if !self.visible || self.locked {
            return false;
        }
        match &self.shape {
            CanvasShape::Rectangle { .. } | CanvasShape::Image { .. } => {
                self.bounds().contains(point[0], point[1])
            }
            CanvasShape::Ellipse { size } => {
                let x = (point[0] - self.origin[0]) / size[0] * 2.0 - 1.0;
                let y = (point[1] - self.origin[1]) / size[1] * 2.0 - 1.0;
                x * x + y * y <= 1.0
            }
            CanvasShape::Line { end } => {
                let dx = end[0] - self.origin[0];
                let dy = end[1] - self.origin[1];
                let length_sq = dx * dx + dy * dy;
                let t = if length_sq > 0.0 {
                    (((point[0] - self.origin[0]) * dx + (point[1] - self.origin[1]) * dy)
                        / length_sq)
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let ex = point[0] - (self.origin[0] + t * dx);
                let ey = point[1] - (self.origin[1] + t * dy);
                ex * ex + ey * ey <= tolerance * tolerance
            }
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Canvas {
    pub objects: Vec<CanvasObject>,
    pub selected: BTreeSet<u64>,
    /// Screen-space translation in logical UI pixels.
    pub pan: [f32; 2],
    pub zoom: f32,
    next_id: u64,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            selected: BTreeSet::new(),
            pan: [0.0, 0.0],
            zoom: 1.0,
            next_id: 1,
        }
    }
}

impl Canvas {
    pub const MIN_ZOOM: f32 = 0.05;
    pub const MAX_ZOOM: f32 = 32.0;

    pub fn world_to_screen(&self, point: [f32; 2]) -> [f32; 2] {
        [
            point[0] * self.zoom + self.pan[0],
            point[1] * self.zoom + self.pan[1],
        ]
    }

    pub fn screen_to_world(&self, point: [f32; 2]) -> [f32; 2] {
        [
            (point[0] - self.pan[0]) / self.zoom,
            (point[1] - self.pan[1]) / self.zoom,
        ]
    }

    pub fn pan_by(&mut self, delta: [f32; 2]) -> bool {
        if !delta.iter().all(|v| v.is_finite()) {
            return false;
        }
        let next = [self.pan[0] + delta[0], self.pan[1] + delta[1]];
        if !next.iter().all(|v| v.is_finite()) {
            return false;
        }
        let changed = next != self.pan;
        self.pan = next;
        changed
    }

    pub fn zoom_at(&mut self, screen: [f32; 2], factor: f32) -> bool {
        if !screen.iter().all(|v| v.is_finite()) || !factor.is_finite() || factor <= 0.0 {
            return false;
        }
        let next_zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        if next_zoom == self.zoom {
            return false;
        }
        let anchor = self.screen_to_world(screen);
        let next_pan = [
            screen[0] - anchor[0] * next_zoom,
            screen[1] - anchor[1] * next_zoom,
        ];
        if !next_pan.iter().all(|v| v.is_finite()) {
            return false;
        }
        self.zoom = next_zoom;
        self.pan = next_pan;
        true
    }

    pub fn add(&mut self, origin: [f32; 2], shape: CanvasShape, color: [f32; 4]) -> Option<u64> {
        let id = self.next_id;
        let next_id = id.checked_add(1)?;
        let object = CanvasObject {
            id,
            origin,
            shape,
            color,
            visible: true,
            locked: false,
        };
        if !object.valid() {
            return None;
        }
        self.objects.push(object);
        self.next_id = next_id;
        Some(id)
    }

    pub fn remove_selected(&mut self) -> usize {
        let before = self.objects.len();
        self.objects
            .retain(|object| object.locked || !self.selected.contains(&object.id));
        self.selected.clear();
        before - self.objects.len()
    }

    pub fn move_selected(&mut self, delta: [f32; 2]) -> bool {
        if !delta.iter().all(|v| v.is_finite()) {
            return false;
        }
        let mut changed = false;
        for object in &self.objects {
            if !self.selected.contains(&object.id) || object.locked {
                continue;
            }
            let mut candidate = object.clone();
            candidate.origin[0] += delta[0];
            candidate.origin[1] += delta[1];
            if let CanvasShape::Line { end } = &mut candidate.shape {
                end[0] += delta[0];
                end[1] += delta[1];
            }
            if !candidate.valid() {
                return false;
            }
        }
        for object in &mut self.objects {
            if self.selected.contains(&object.id) && !object.locked {
                object.origin[0] += delta[0];
                object.origin[1] += delta[1];
                if let CanvasShape::Line { end } = &mut object.shape {
                    end[0] += delta[0];
                    end[1] += delta[1];
                }
                changed = true;
            }
        }
        changed
    }

    /// Returns the topmost unlocked object intersecting the clipped viewport.
    pub fn hit_test(&self, screen: [f32; 2], viewport: UiRect) -> Option<u64> {
        if !viewport.contains(screen[0], screen[1]) {
            return None;
        }
        let point = self.screen_to_world(screen);
        self.objects
            .iter()
            .rev()
            .find(|object| object.hit(point, 6.0 / self.zoom))
            .map(|object| object.id)
    }

    pub fn hit_test_indexed(
        &self,
        index: &CanvasSpatialIndex,
        screen: [f32; 2],
        viewport: UiRect,
    ) -> Option<u64> {
        if !index.usable {
            return self.hit_test(screen, viewport);
        }
        if !viewport.contains(screen[0], screen[1]) || !self.zoom.is_finite() || self.zoom <= 0.0 {
            return None;
        }
        let point = self.screen_to_world(screen);
        let tolerance = 6.0 / self.zoom;
        let query = UiRect::new(
            point[0] - tolerance,
            point[1] - tolerance,
            tolerance * 2.0,
            tolerance * 2.0,
        );
        index
            .query(query)?
            .into_iter()
            .rev()
            .filter_map(|position| self.objects.get(position))
            .find(|object| object.hit(point, tolerance))
            .map(|object| object.id)
    }

    pub fn select_at(&mut self, screen: [f32; 2], viewport: UiRect, extend: bool) -> bool {
        self.select_hit(self.hit_test(screen, viewport), extend)
    }

    pub fn select_hit(&mut self, hit: Option<u64>, extend: bool) -> bool {
        let before = self.selected.clone();
        if !extend {
            self.selected.clear();
        }
        if let Some(id) = hit {
            if extend && !self.selected.insert(id) {
                self.selected.remove(&id);
            } else if !extend {
                self.selected.insert(id);
            }
        }
        self.selected != before
    }

    fn move_selected_at_indices(&mut self, indices: &[usize], delta: [f32; 2]) -> bool {
        if !delta.iter().all(|value| value.is_finite()) {
            return false;
        }
        for index in indices {
            let Some(object) = self.objects.get(*index) else {
                continue;
            };
            if !self.selected.contains(&object.id) || object.locked {
                continue;
            }
            let mut candidate = object.clone();
            candidate.origin[0] += delta[0];
            candidate.origin[1] += delta[1];
            if let CanvasShape::Line { end } = &mut candidate.shape {
                end[0] += delta[0];
                end[1] += delta[1];
            }
            if !candidate.valid() {
                return false;
            }
        }
        let mut changed = false;
        for index in indices {
            let Some(object) = self.objects.get_mut(*index) else {
                continue;
            };
            if !self.selected.contains(&object.id) || object.locked {
                continue;
            }
            object.origin[0] += delta[0];
            object.origin[1] += delta[1];
            if let CanvasShape::Line { end } = &mut object.shape {
                end[0] += delta[0];
                end[1] += delta[1];
            }
            changed = true;
        }
        changed
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&CanvasSnapshot {
            version: 1,
            objects: self.objects.clone(),
            selected: self.selected.clone(),
            pan: self.pan,
            zoom: self.zoom,
            next_id: self.next_id,
        })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let saved: CanvasSnapshot =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if saved.version != 1 {
            return Err("unsupported canvas version".to_owned());
        }
        if !saved.pan.iter().all(|v| v.is_finite())
            || !saved.zoom.is_finite()
            || !(Self::MIN_ZOOM..=Self::MAX_ZOOM).contains(&saved.zoom)
            || saved.next_id == 0
        {
            return Err("invalid canvas transform".to_owned());
        }
        let ids: BTreeSet<_> = saved.objects.iter().map(|object| object.id).collect();
        if ids.len() != saved.objects.len()
            || saved.objects.iter().any(|object| !object.valid())
            || ids
                .iter()
                .next_back()
                .is_some_and(|id| *id >= saved.next_id)
            || !saved.selected.is_subset(&ids)
        {
            return Err("invalid canvas objects or selection".to_owned());
        }
        Ok(Self {
            objects: saved.objects,
            selected: saved.selected,
            pan: saved.pan,
            zoom: saved.zoom,
            next_id: saved.next_id,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct CanvasSnapshot {
    version: u32,
    objects: Vec<CanvasObject>,
    selected: BTreeSet<u64>,
    pan: [f32; 2],
    zoom: f32,
    next_id: u64,
}

#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct CanvasImageAssets {
    ids: HashMap<String, u64>,
    states: HashMap<String, CanvasImageState>,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct CanvasObjectControl {
    pub owner: Entity,
    pub object_id: u64,
    pub label: String,
    pub selected: bool,
    pub locked: bool,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub(crate) struct CanvasObjectPaint(pub CanvasObject);

#[derive(Resource, Debug, Default)]
pub struct CanvasObjectItems(HashMap<Entity, HashMap<u64, Entity>>);

#[derive(Resource, Debug, Default)]
pub struct CanvasSpatialIndexes(HashMap<Entity, CanvasSpatialIndex>);

const CANVAS_SPATIAL_CELL_SIZE: f32 = 128.0;
const CANVAS_SPATIAL_MAX_CELLS_PER_OBJECT: i128 = 128;
const CANVAS_SPATIAL_MAX_QUERY_CELLS: i128 = 65_536;

#[derive(Debug, Default)]
pub struct CanvasSpatialIndex {
    objects: HashMap<u64, CanvasSpatialEntry>,
    cells: HashMap<(i64, i64), HashSet<u64>>,
    broad: HashSet<u64>,
    changed_tick: u32,
    usable: bool,
}

#[derive(Debug)]
struct CanvasSpatialEntry {
    index: usize,
    bounds: UiRect,
    cells: Vec<(i64, i64)>,
    broad: bool,
}

impl CanvasSpatialIndex {
    pub fn new(canvas: &Canvas) -> Self {
        Self::build(canvas, 0)
    }

    pub fn visible_indices(
        &self,
        canvas: &Canvas,
        rect: UiRect,
        clip: ClipRect,
        padding: f32,
    ) -> Option<Vec<usize>> {
        let viewport = canvas_world_viewport(canvas, rect, clip, padding)?;
        self.query(viewport)
    }

    fn build(canvas: &Canvas, changed_tick: u32) -> Self {
        let mut index = Self {
            changed_tick,
            usable: true,
            ..Default::default()
        };
        for (position, object) in canvas.objects.iter().enumerate() {
            if index.objects.contains_key(&object.id) {
                // Direct edits can violate the model's unique-ID invariant. Keep the old
                // full-scan behavior until the canvas is repaired instead of dropping one.
                index.usable = false;
                return index;
            }
            index.insert_object(position, object.id, object.bounds());
        }
        index
    }

    fn insert_object(&mut self, position: usize, id: u64, bounds: UiRect) {
        let cells = spatial_cells(bounds, CANVAS_SPATIAL_MAX_CELLS_PER_OBJECT);
        let broad = cells.is_none();
        let cells = cells.unwrap_or_default();
        if broad {
            self.broad.insert(id);
        } else {
            for cell in &cells {
                self.cells.entry(*cell).or_default().insert(id);
            }
        }
        self.objects.insert(
            id,
            CanvasSpatialEntry {
                index: position,
                bounds,
                cells,
                broad,
            },
        );
    }

    fn update_object(&mut self, position: usize, id: u64, bounds: UiRect) {
        if let Some(previous) = self.objects.remove(&id) {
            if previous.broad {
                self.broad.remove(&id);
            }
            for cell in previous.cells {
                if let Some(ids) = self.cells.get_mut(&cell) {
                    ids.remove(&id);
                    if ids.is_empty() {
                        self.cells.remove(&cell);
                    }
                }
            }
        }
        self.insert_object(position, id, bounds);
    }

    fn query(&self, bounds: UiRect) -> Option<Vec<usize>> {
        if !self.usable || !bounds.is_finite() {
            return None;
        }
        let mut ids = HashSet::new();
        if let Some(cells) = spatial_cells(bounds, CANVAS_SPATIAL_MAX_QUERY_CELLS) {
            for cell in cells {
                if let Some(found) = self.cells.get(&cell) {
                    ids.extend(found.iter().copied());
                }
            }
            ids.extend(self.broad.iter().copied());
        } else {
            // A very zoomed-out viewport can cover most of the grid. Enumerating the
            // index once is cheaper and remains correct in that case.
            ids.extend(self.objects.keys().copied());
        }
        let mut positions = ids
            .into_iter()
            .filter_map(|id| {
                self.objects
                    .get(&id)
                    .filter(|object| rects_overlap(object.bounds, bounds))
                    .map(|object| object.index)
            })
            .collect::<Vec<_>>();
        positions.sort_unstable();
        Some(positions)
    }
}

impl CanvasSpatialIndexes {
    fn query(&self, owner: Entity, bounds: UiRect) -> Option<Vec<usize>> {
        self.0.get(&owner)?.query(bounds)
    }

    fn position(&self, owner: Entity, id: u64) -> Option<usize> {
        self.0
            .get(&owner)?
            .objects
            .get(&id)
            .map(|object| object.index)
    }
}

fn spatial_cells(bounds: UiRect, limit: i128) -> Option<Vec<(i64, i64)>> {
    if !bounds.is_finite() || !bounds.right().is_finite() || !bounds.bottom().is_finite() {
        return None;
    }
    let left = (bounds.x / CANVAS_SPATIAL_CELL_SIZE).floor() as i64;
    let top = (bounds.y / CANVAS_SPATIAL_CELL_SIZE).floor() as i64;
    let right = (bounds.right() / CANVAS_SPATIAL_CELL_SIZE).floor() as i64;
    let bottom = (bounds.bottom() / CANVAS_SPATIAL_CELL_SIZE).floor() as i64;
    let width = i128::from(right) - i128::from(left) + 1;
    let height = i128::from(bottom) - i128::from(top) + 1;
    if width <= 0 || height <= 0 || width.saturating_mul(height) > limit {
        return None;
    }
    let mut cells = Vec::with_capacity((width * height) as usize);
    for x in left..=right {
        for y in top..=bottom {
            cells.push((x, y));
        }
    }
    Some(cells)
}

fn rects_overlap(left: UiRect, right: UiRect) -> bool {
    left.right() >= right.x
        && right.right() >= left.x
        && left.bottom() >= right.y
        && right.bottom() >= left.y
}

fn canvas_world_viewport(
    canvas: &Canvas,
    rect: UiRect,
    clip: ClipRect,
    padding: f32,
) -> Option<UiRect> {
    if !canvas.zoom.is_finite() || canvas.zoom <= 0.0 || !padding.is_finite() {
        return None;
    }
    let viewport = UiRect::new(clip.x, clip.y, clip.width, clip.height).intersection(rect)?;
    let left = (viewport.x - rect.x - padding - canvas.pan[0]) / canvas.zoom;
    let top = (viewport.y - rect.y - padding - canvas.pan[1]) / canvas.zoom;
    let right = (viewport.right() - rect.x + padding - canvas.pan[0]) / canvas.zoom;
    let bottom = (viewport.bottom() - rect.y + padding - canvas.pan[1]) / canvas.zoom;
    let world = UiRect::new(
        left.min(right),
        top.min(bottom),
        (right - left).abs(),
        (bottom - top).abs(),
    );
    world.is_finite().then_some(world)
}

fn query_canvas_object_indices(
    world: &World,
    owner: Entity,
    canvas: &Canvas,
    rect: UiRect,
    clip: ClipRect,
    padding: f32,
) -> Option<Vec<usize>> {
    let world_view = canvas_world_viewport(canvas, rect, clip, padding)?;
    world
        .get_resource::<CanvasSpatialIndexes>()?
        .query(owner, world_view)
}

fn canvas_component_tick(world: &World, owner: Entity) -> Option<u32> {
    let component_id = world.components().component_id::<Canvas>()?;
    world
        .get_entity(owner)
        .ok()?
        .get_change_ticks_by_id(component_id)
        .map(|ticks| ticks.changed.get())
}

/// Ensure the spatial index reflects direct public ECS edits. Internal Canvas gestures call
/// `note_canvas_metadata_edit` or `note_canvas_object_edits` to keep ordinary edits incremental.
pub(crate) fn ensure_canvas_spatial_index(world: &mut World, owner: Entity) -> bool {
    world.init_resource::<CanvasSpatialIndexes>();
    let Some(changed_tick) = canvas_component_tick(world, owner) else {
        return false;
    };
    let current = world
        .resource::<CanvasSpatialIndexes>()
        .0
        .get(&owner)
        .is_some_and(|index| index.changed_tick == changed_tick);
    if current {
        return true;
    }
    let Some(index) = world
        .get::<Canvas>(owner)
        .map(|canvas| CanvasSpatialIndex::build(canvas, changed_tick))
    else {
        return false;
    };
    world
        .resource_mut::<CanvasSpatialIndexes>()
        .0
        .insert(owner, index);
    true
}

/// Edit one Canvas object while keeping its spatial index in sync. The object's ID is stable;
/// use a structural Canvas edit to add, remove, or reorder objects.
/// Returns false when the object is missing, the edit is unchanged, or it makes the object invalid.
pub fn edit_canvas_object(
    world: &mut World,
    owner: Entity,
    id: u64,
    edit: impl FnOnce(&mut CanvasObject),
) -> bool {
    if !ensure_canvas_spatial_index(world, owner) {
        return false;
    }
    let Some(position) = world
        .resource::<CanvasSpatialIndexes>()
        .0
        .get(&owner)
        .filter(|index| index.usable)
        .and_then(|index| index.objects.get(&id))
        .map(|entry| entry.index)
    else {
        return false;
    };
    let Some(original) = world
        .get::<Canvas>(owner)
        .and_then(|canvas| canvas.objects.get(position))
        .filter(|object| object.id == id)
    else {
        return false;
    };
    let mut updated = original.clone();
    edit(&mut updated);
    if updated.id != id || !updated.valid() || updated == *original {
        return false;
    }
    let Some(mut canvas) = world.get_mut::<Canvas>(owner) else {
        return false;
    };
    let Some(object) = canvas.objects.get_mut(position) else {
        return false;
    };
    *object = updated;
    note_canvas_object_edits(world, owner, &[position]);
    true
}

pub(crate) fn note_canvas_metadata_edit(world: &mut World, owner: Entity) {
    let Some(changed_tick) = canvas_component_tick(world, owner) else {
        return;
    };
    let Some(mut indexes) = world.get_resource_mut::<CanvasSpatialIndexes>() else {
        return;
    };
    let Some(index) = indexes.0.get_mut(&owner) else {
        return;
    };
    if index.usable {
        index.changed_tick = changed_tick;
    }
}

fn note_canvas_object_edits(world: &mut World, owner: Entity, positions: &[usize]) {
    let Some(changed_tick) = canvas_component_tick(world, owner) else {
        return;
    };
    let updates = {
        let Some(canvas) = world.get::<Canvas>(owner) else {
            return;
        };
        positions
            .iter()
            .filter_map(|position| {
                canvas
                    .objects
                    .get(*position)
                    .map(|object| (*position, object.id, object.bounds()))
            })
            .collect::<Vec<_>>()
    };
    let Some(mut indexes) = world.get_resource_mut::<CanvasSpatialIndexes>() else {
        return;
    };
    let Some(index) = indexes.0.get_mut(&owner) else {
        return;
    };
    if !index.usable {
        return;
    }
    for (position, id, bounds) in updates {
        index.update_object(position, id, bounds);
    }
    index.changed_tick = changed_tick;
}

/// Keep a stable entity only for each object intersecting the visible canvas
/// clip. These entities expose per-object accessibility without scaling entity
/// count with offscreen content.
pub fn sync_canvas_object_views(world: &mut World) {
    let live_canvas_owners = {
        let mut query = world.query_filtered::<Entity, With<Canvas>>();
        query.iter(world).collect::<HashSet<_>>()
    };
    if let Some(mut indexes) = world.get_resource_mut::<CanvasSpatialIndexes>() {
        indexes
            .0
            .retain(|owner, _| live_canvas_owners.contains(owner));
    }
    let mut query = world.query::<(Entity, &Canvas, &UiLayoutResult)>();
    let owners = query
        .iter(world)
        .map(|(owner, _, layout)| (owner, *layout))
        .collect::<Vec<_>>();
    let mut previous = world
        .remove_resource::<CanvasObjectItems>()
        .unwrap_or_default();
    let mut previous_entities = std::mem::take(&mut previous.0);
    let mut next_entities = HashMap::new();
    for (owner, layout) in owners {
        if !visible_in_hierarchy(world, owner) || world.get::<UiNode>(owner).is_none() {
            continue;
        }
        ensure_canvas_spatial_index(world, owner);
        let Some(canvas) = world.get::<Canvas>(owner) else {
            continue;
        };
        let clip = layout.clip.unwrap_or(layout.rect).intersection(layout.rect);
        let mut items = Vec::new();
        let visible_indices = query_canvas_object_indices(
            world,
            owner,
            canvas,
            layout.rect,
            ClipRect::from(clip.unwrap_or_default()),
            8.0,
        )
        .unwrap_or_else(|| (0..canvas.objects.len()).collect());
        for index in visible_indices.iter().copied() {
            let Some(object) = canvas.objects.get(index) else {
                continue;
            };
            if !object.visible {
                continue;
            }
            let paint = CanvasObjectPaint(object.clone());
            let bounds = object.bounds();
            let position = canvas.world_to_screen([bounds.x, bounds.y]);
            let rect = UiRect::new(
                layout.rect.x + position[0] - 6.0,
                layout.rect.y + position[1] - 6.0,
                (bounds.width * canvas.zoom).max(1.0) + 12.0,
                (bounds.height * canvas.zoom).max(1.0) + 12.0,
            );
            let Some(visible_rect) = clip.and_then(|clip| clip.intersection(rect)) else {
                continue;
            };
            let label = match &object.shape {
                CanvasShape::Rectangle { .. } => format!("Rectangle {}", object.id),
                CanvasShape::Ellipse { .. } => format!("Ellipse {}", object.id),
                CanvasShape::Line { .. } => format!("Line {}", object.id),
                CanvasShape::Image { asset, .. } => format!("Image {}: {asset}", object.id),
            };
            let control = CanvasObjectControl {
                owner,
                object_id: object.id,
                label,
                selected: canvas.selected.contains(&object.id),
                locked: object.locked,
            };
            let child_layout = UiLayoutResult {
                window: layout.window,
                rect: visible_rect,
                clip,
                z: layout.z.saturating_add(1),
                content_size: [rect.width, rect.height],
                dpi_scale: layout.dpi_scale,
            };
            items.push((control, paint, child_layout));
        }
        let mut owned = previous_entities.remove(&owner).unwrap_or_default();
        let mut current = HashMap::new();
        let mut ordered_entities = Vec::with_capacity(items.len());
        for (control, paint, child_layout) in items {
            let object_id = control.object_id;
            let locked = control.locked;
            let entity = if let Some(entity) = owned
                .remove(&object_id)
                .filter(|entity| world.get_entity(*entity).is_ok())
            {
                entity
            } else {
                let entity = world
                    .spawn((
                        UiNode,
                        UiFocusable,
                        UiKey(format!("canvas:{owner:?}:{object_id}")),
                    ))
                    .id();
                if set_parent(world, entity, owner).is_err() {
                    world.despawn(entity);
                    continue;
                }
                entity
            };
            let mut paint_changed = false;
            let mut accessibility_changed = false;
            if world.get::<CanvasObjectControl>(entity) != Some(&control) {
                world.entity_mut(entity).insert(control);
                paint_changed = true;
                accessibility_changed = true;
            }
            if world.get::<CanvasObjectPaint>(entity) != Some(&paint) {
                world.entity_mut(entity).insert(paint);
                paint_changed = true;
            }
            if world.get::<UiLayoutResult>(entity) != Some(&child_layout) {
                world.entity_mut(entity).insert(child_layout);
                paint_changed = true;
            }
            if world
                .get::<UiEnabled>(entity)
                .is_none_or(|enabled| enabled.0 == locked)
            {
                world.entity_mut(entity).insert(UiEnabled(!locked));
                paint_changed = true;
                accessibility_changed = true;
            }
            if paint_changed || accessibility_changed {
                world.init_resource::<UiDirty>();
                let mut dirty = world.resource_mut::<UiDirty>();
                if paint_changed {
                    dirty.mark(UiDirtyDomain::Paint, entity);
                }
                if accessibility_changed {
                    dirty.mark(UiDirtyDomain::Accessibility, entity);
                }
            }
            current.insert(object_id, entity);
            ordered_entities.push(entity);
        }
        for entity in owned.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
            world.init_resource::<UiDirty>();
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Paint, entity);
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Accessibility, entity);
        }
        if let Some(children) = world
            .get::<UiChildren>(owner)
            .map(|children| children.0.clone())
        {
            let mut reordered = children
                .iter()
                .copied()
                .filter(|child| {
                    world
                        .get::<CanvasObjectControl>(*child)
                        .is_none_or(|control| control.owner != owner)
                })
                .collect::<Vec<_>>();
            reordered.extend(ordered_entities.iter().copied());
            if reordered != children {
                world.entity_mut(owner).insert(UiChildren(reordered));
            }
        }
        next_entities.insert(owner, current);
    }
    for owned in previous_entities.into_values() {
        for entity in owned.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
            world.init_resource::<UiDirty>();
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Paint, entity);
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Accessibility, entity);
        }
    }
    world.insert_resource(CanvasObjectItems(next_entities));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasImageState {
    Loading,
    Ready,
    Failed(String),
}

impl CanvasImageAssets {
    pub fn id(&self, asset: &str) -> Option<u64> {
        self.ids.get(asset).copied()
    }

    pub fn state(&self, asset: &str) -> Option<&CanvasImageState> {
        self.states.get(asset)
    }
}

/// Keeps only images used by mounted, visible canvas objects in Bevy's image
/// registry. The asset server owns decoding; dropping the last handle lets Bevy
/// release the texture when a canvas leaves the viewport or is removed.
pub fn sync_canvas_images(world: &mut World) {
    let Some(asset_server) = world.get_resource::<AssetServer>().cloned() else {
        return;
    };
    if world.get_resource::<ImageRegistry>().is_none() {
        return;
    }
    let required = visible_canvas_image_paths(world);
    let state_is_current = world
        .get_resource::<CanvasImageAssets>()
        .is_some_and(|assets| {
            assets.ids.len() == required.len()
                && assets.states.len() == required.len()
                && required.iter().all(|asset| {
                    let Some(id) = assets.ids.get(asset) else {
                        return false;
                    };
                    let status = world
                        .resource::<ImageRegistry>()
                        .get(*id)
                        .and_then(|handle| asset_server.get_load_state(handle.id()))
                        .map(canvas_image_state);
                    assets.states.get(asset) == Some(&status.unwrap_or(CanvasImageState::Loading))
                })
        });
    if state_is_current {
        return;
    }
    let previous = world
        .get_resource::<CanvasImageAssets>()
        .cloned()
        .unwrap_or_default();
    let mut assets = world
        .remove_resource::<CanvasImageAssets>()
        .unwrap_or_default();
    let mut registry = world.resource_mut::<ImageRegistry>();
    assets.ids.retain(|asset, id| {
        if required.contains(asset) {
            true
        } else {
            registry.remove(*id);
            false
        }
    });
    assets.states.retain(|asset, _| required.contains(asset));
    for asset in required {
        if assets.ids.contains_key(&asset) {
            continue;
        }
        let Some(id) = registry.allocate_id() else {
            break;
        };
        let handle = asset_server.load::<Image>(asset.clone());
        registry.insert(id, handle);
        assets.ids.insert(asset, id);
    }
    for (asset, id) in &assets.ids {
        let state = registry
            .get(*id)
            .and_then(|handle| asset_server.get_load_state(handle.id()));
        let status = state
            .map(canvas_image_state)
            .unwrap_or(CanvasImageState::Loading);
        assets.states.insert(asset.clone(), status);
    }
    let mut changed_assets = HashSet::new();
    changed_assets.extend(
        previous
            .ids
            .keys()
            .chain(assets.ids.keys())
            .filter(|asset| {
                let asset = *asset;
                previous.ids.get(asset) != assets.ids.get(asset)
                    || previous.states.get(asset) != assets.states.get(asset)
            })
            .cloned(),
    );
    world.insert_resource(assets);
    if !changed_assets.is_empty() {
        let changed_items = world
            .get_resource::<CanvasObjectItems>()
            .into_iter()
            .flat_map(|items| items.0.values())
            .flat_map(HashMap::values)
            .filter_map(|entity| {
                let paint = world.get::<CanvasObjectPaint>(*entity)?;
                let CanvasShape::Image { asset, .. } = &paint.0.shape else {
                    return None;
                };
                changed_assets.contains(asset).then_some(*entity)
            })
            .collect::<Vec<_>>();
        world.init_resource::<UiDirty>();
        let mut dirty = world.resource_mut::<UiDirty>();
        for entity in changed_items {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
}

fn canvas_image_state(state: LoadState) -> CanvasImageState {
    match state {
        LoadState::Loaded => CanvasImageState::Ready,
        LoadState::Failed(error) => CanvasImageState::Failed(error.to_string()),
        _ => CanvasImageState::Loading,
    }
}

fn visible_canvas_image_paths(world: &mut World) -> HashSet<String> {
    let mut query = world.query::<(Entity, &Canvas, &UiLayoutResult)>();
    let owners = query
        .iter(world)
        .map(|(owner, _, layout)| (owner, *layout))
        .collect::<Vec<_>>();
    let mut paths = HashSet::new();
    for (owner, layout) in owners {
        if !visible_in_hierarchy(world, owner) {
            continue;
        }
        ensure_canvas_spatial_index(world, owner);
        let Some(canvas) = world.get::<Canvas>(owner) else {
            continue;
        };
        let Some(viewport) = layout.clip.unwrap_or(layout.rect).intersection(layout.rect) else {
            continue;
        };
        let indices = query_canvas_object_indices(
            world,
            owner,
            canvas,
            layout.rect,
            ClipRect::from(viewport),
            0.0,
        )
        .unwrap_or_else(|| (0..canvas.objects.len()).collect());
        for index in indices {
            let Some(object) = canvas.objects.get(index) else {
                continue;
            };
            let CanvasShape::Image { size, asset } = &object.shape else {
                continue;
            };
            if !object.visible || !object.shape.valid() {
                continue;
            }
            let position = canvas.world_to_screen(object.origin);
            let screen_rect = UiRect::new(
                layout.rect.x + position[0],
                layout.rect.y + position[1],
                size[0] * canvas.zoom,
                size[1] * canvas.zoom,
            );
            if viewport.intersection(screen_rect).is_some() {
                paths.insert(asset.clone());
            }
        }
    }
    paths
}

fn visible_in_hierarchy(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    for _ in 0..1024 {
        let Some(at) = current else {
            return true;
        };
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    false
}

/// Draws a canvas from the same transform and clip used by hit testing.
pub fn paint_canvas(
    canvas: &Canvas,
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    paint_canvas_objects(
        canvas,
        0..canvas.objects.len(),
        rect,
        clip,
        z,
        theme,
        images,
        output,
    );
}

/// Paint only the supplied object positions after building a spatial query for the viewport.
/// The Canvas widget uses per-object child fragments; this entry point is useful for direct
/// canvas rendering and the stress harness.
#[expect(
    clippy::too_many_arguments,
    reason = "Direct Canvas painting keeps the model, sparse positions, viewport, theme, assets, and output explicit"
)]
pub fn paint_canvas_indexed(
    canvas: &Canvas,
    object_indices: &[usize],
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    paint_canvas_objects(
        canvas,
        object_indices.iter().copied(),
        rect,
        clip,
        z,
        theme,
        images,
        output,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "The shared Canvas loop carries the model, object positions, viewport, theme, assets, and output"
)]
fn paint_canvas_objects(
    canvas: &Canvas,
    object_indices: impl IntoIterator<Item = usize>,
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    let viewport = UiRect::new(clip.x, clip.y, clip.width, clip.height).intersection(rect);
    paint_canvas_background(rect, clip, z, theme, output);
    for object_index in object_indices {
        let Some(object) = canvas.objects.get(object_index) else {
            continue;
        };
        if !object.visible {
            continue;
        }
        let bounds = object.bounds();
        let position = canvas.world_to_screen([bounds.x, bounds.y]);
        // Include the selection stroke and line width at the viewport edge.
        let screen_bounds = UiRect::new(
            rect.x + position[0] - 8.0,
            rect.y + position[1] - 8.0,
            (bounds.width * canvas.zoom).max(1.0) + 16.0,
            (bounds.height * canvas.zoom).max(1.0) + 16.0,
        );
        if !screen_bounds.is_finite()
            || !viewport.is_some_and(|viewport| viewport.intersection(screen_bounds).is_some())
        {
            continue;
        }
        let to_screen = |point: [f32; 2]| {
            let local = canvas.world_to_screen(point);
            [rect.x + local[0], rect.y + local[1]]
        };
        let origin = to_screen(object.origin);
        let color = Rgba(
            object.color[0],
            object.color[1],
            object.color[2],
            object.color[3],
        );
        let primitive = match &object.shape {
            CanvasShape::Rectangle { size } => Some(Primitive::RoundedRect {
                rect: ClipRect {
                    x: origin[0],
                    y: origin[1],
                    width: size[0] * canvas.zoom,
                    height: size[1] * canvas.zoom,
                },
                radius: 0.0,
                color,
            }),
            CanvasShape::Ellipse { size } => Some(Primitive::Ellipse {
                rect: ClipRect {
                    x: origin[0],
                    y: origin[1],
                    width: size[0] * canvas.zoom,
                    height: size[1] * canvas.zoom,
                },
                color,
            }),
            CanvasShape::Line { end } => Some(Primitive::Line {
                from: origin,
                to: to_screen(*end),
                width: (2.0 * canvas.zoom).clamp(1.0, 16.0),
                color,
            }),
            CanvasShape::Image { size, asset } => images
                .filter(|images| images.state(asset) == Some(&CanvasImageState::Ready))
                .and_then(|images| images.id(asset))
                .map(|asset_id| Primitive::Icon {
                    rect: ClipRect {
                        x: origin[0],
                        y: origin[1],
                        width: size[0] * canvas.zoom,
                        height: size[1] * canvas.zoom,
                    },
                    asset_id,
                    tint: color,
                }),
        };
        if let Some(primitive) = primitive {
            output.push(DrawItem {
                clip,
                z: z.saturating_add(1),
                primitive,
            });
        }
        if let CanvasShape::Image { asset, .. } = &object.shape
            && !matches!(
                images.and_then(|images| images.state(asset)),
                Some(CanvasImageState::Ready)
            )
        {
            let message = match images.and_then(|images| images.state(asset)) {
                Some(CanvasImageState::Failed(reason)) => format!("Image error: {reason}"),
                _ => format!("Loading image: {asset}"),
            };
            output.push(DrawItem {
                clip,
                z: z.saturating_add(1),
                primitive: Primitive::Text {
                    origin: [origin[0] + 4.0, origin[1] + 4.0],
                    text: message,
                    color: theme.text_muted,
                },
            });
        }
        if canvas.selected.contains(&object.id) {
            let bounds = object.bounds();
            let a = to_screen([bounds.x, bounds.y]);
            let b = to_screen([bounds.right(), bounds.bottom()]);
            let x0 = a[0].min(b[0]);
            let y0 = a[1].min(b[1]);
            let x1 = a[0].max(b[0]);
            let y1 = a[1].max(b[1]);
            if x1 > x0 && y1 > y0 {
                output.push(DrawItem {
                    clip,
                    z: z.saturating_add(2),
                    primitive: Primitive::RoundedRect {
                        rect: ClipRect {
                            x: x0,
                            y: y0,
                            width: x1 - x0,
                            height: y1 - y0,
                        },
                        radius: 0.0,
                        color: theme.color(ThemeColorToken::CanvasSelectionOverlay),
                    },
                });
            }
            for (from, to) in [
                ([x0, y0], [x1, y0]),
                ([x1, y0], [x1, y1]),
                ([x1, y1], [x0, y1]),
                ([x0, y1], [x0, y0]),
            ] {
                output.push(DrawItem {
                    clip,
                    z: z.saturating_add(2),
                    primitive: Primitive::Line {
                        from,
                        to,
                        width: 1.5,
                        color: theme.selection,
                    },
                });
            }
        }
    }
}

pub(crate) fn paint_canvas_background(
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    output.push(DrawItem {
        clip,
        z,
        primitive: Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.background,
        },
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "One stable Canvas fragment needs its model, object, viewport, z, theme, assets, and output"
)]
pub(crate) fn paint_canvas_object_fragment(
    canvas: &Canvas,
    object: &CanvasObject,
    rect: UiRect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    images: Option<&CanvasImageAssets>,
    output: &mut Vec<DrawItem>,
) {
    if !object.visible {
        return;
    }
    let viewport = UiRect::new(clip.x, clip.y, clip.width, clip.height).intersection(rect);
    let bounds = object.bounds();
    let position = canvas.world_to_screen([bounds.x, bounds.y]);
    let screen_bounds = UiRect::new(
        rect.x + position[0] - 8.0,
        rect.y + position[1] - 8.0,
        (bounds.width * canvas.zoom).max(1.0) + 16.0,
        (bounds.height * canvas.zoom).max(1.0) + 16.0,
    );
    if !screen_bounds.is_finite()
        || !viewport.is_some_and(|viewport| viewport.intersection(screen_bounds).is_some())
    {
        return;
    }
    let to_screen = |point: [f32; 2]| {
        let local = canvas.world_to_screen(point);
        [rect.x + local[0], rect.y + local[1]]
    };
    let origin = to_screen(object.origin);
    let color = Rgba(
        object.color[0],
        object.color[1],
        object.color[2],
        object.color[3],
    );
    let primitive = match &object.shape {
        CanvasShape::Rectangle { size } => Some(Primitive::RoundedRect {
            rect: ClipRect {
                x: origin[0],
                y: origin[1],
                width: size[0] * canvas.zoom,
                height: size[1] * canvas.zoom,
            },
            radius: 0.0,
            color,
        }),
        CanvasShape::Ellipse { size } => Some(Primitive::Ellipse {
            rect: ClipRect {
                x: origin[0],
                y: origin[1],
                width: size[0] * canvas.zoom,
                height: size[1] * canvas.zoom,
            },
            color,
        }),
        CanvasShape::Line { end } => Some(Primitive::Line {
            from: origin,
            to: to_screen(*end),
            width: (2.0 * canvas.zoom).clamp(1.0, 16.0),
            color,
        }),
        CanvasShape::Image { size, asset } => images
            .filter(|images| images.state(asset) == Some(&CanvasImageState::Ready))
            .and_then(|images| images.id(asset))
            .map(|asset_id| Primitive::Icon {
                rect: ClipRect {
                    x: origin[0],
                    y: origin[1],
                    width: size[0] * canvas.zoom,
                    height: size[1] * canvas.zoom,
                },
                asset_id,
                tint: color,
            }),
    };
    if let Some(primitive) = primitive {
        output.push(DrawItem {
            clip,
            z: z.saturating_add(1),
            primitive,
        });
    }
    if let CanvasShape::Image { asset, .. } = &object.shape
        && !matches!(
            images.and_then(|images| images.state(asset)),
            Some(CanvasImageState::Ready)
        )
    {
        let message = match images.and_then(|images| images.state(asset)) {
            Some(CanvasImageState::Failed(reason)) => format!("Image error: {reason}"),
            _ => format!("Loading image: {asset}"),
        };
        output.push(DrawItem {
            clip,
            z: z.saturating_add(1),
            primitive: Primitive::Text {
                origin: [origin[0] + 4.0, origin[1] + 4.0],
                text: message,
                color: theme.text_muted,
            },
        });
    }
    if canvas.selected.contains(&object.id) {
        let a = to_screen([bounds.x, bounds.y]);
        let b = to_screen([bounds.right(), bounds.bottom()]);
        let x0 = a[0].min(b[0]);
        let y0 = a[1].min(b[1]);
        let x1 = a[0].max(b[0]);
        let y1 = a[1].max(b[1]);
        if x1 > x0 && y1 > y0 {
            output.push(DrawItem {
                clip,
                z: z.saturating_add(2),
                primitive: Primitive::RoundedRect {
                    rect: ClipRect {
                        x: x0,
                        y: y0,
                        width: x1 - x0,
                        height: y1 - y0,
                    },
                    radius: 0.0,
                    color: theme.color(ThemeColorToken::CanvasSelectionOverlay),
                },
            });
        }
        for (from, to) in [
            ([x0, y0], [x1, y0]),
            ([x1, y0], [x1, y1]),
            ([x1, y1], [x0, y1]),
            ([x0, y1], [x0, y0]),
        ] {
            output.push(DrawItem {
                clip,
                z: z.saturating_add(2),
                primitive: Primitive::Line {
                    from,
                    to,
                    width: 1.5,
                    color: theme.selection,
                },
            });
        }
    }
}

#[derive(Debug)]
struct CanvasDrag {
    target: Entity,
    button: UiPointerButton,
    last: [f32; 2],
    before: String,
    moving_objects: bool,
}

#[derive(Resource, Debug, Default)]
pub struct CanvasInputState(HashMap<Entity, CanvasDrag>);

/// Applies routed canvas gestures before the generic widget dispatcher. Screen
/// positions are converted to canvas-local coordinates using the same layout
/// result that clips painting.
pub fn process_canvas_input(world: &mut World) {
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|queue| queue.events.clone())
        .unwrap_or_default();
    let annotated_downs: HashSet<_> = events
        .iter()
        .filter_map(|event| {
            if let UiRoutedEvent::PointerDownWithModifiers {
                window,
                target,
                button,
                ..
            } = event
            {
                Some((*window, *target, *button))
            } else {
                None
            }
        })
        .collect();
    for event in events {
        match event {
            UiRoutedEvent::PointerDown {
                window,
                target,
                button,
                position,
            } if !annotated_downs.contains(&(window, target, button)) => {
                begin_canvas_drag(
                    world,
                    window,
                    target,
                    button,
                    position,
                    UiModifiers::default(),
                );
            }
            UiRoutedEvent::PointerDownWithModifiers {
                window,
                target,
                button,
                position,
                modifiers,
            } => {
                begin_canvas_drag(world, window, target, button, position, modifiers);
            }
            UiRoutedEvent::PointerMoved {
                window, position, ..
            } => {
                let Some(mut drag) = world.resource_mut::<CanvasInputState>().0.remove(&window)
                else {
                    continue;
                };
                if let Some((local, _)) = local_point(world, drag.target, position) {
                    let delta = [local[0] - drag.last[0], local[1] - drag.last[1]];
                    if drag.button == UiPointerButton::Middle {
                        ensure_canvas_spatial_index(world, drag.target);
                        let changed = world
                            .get_mut::<Canvas>(drag.target)
                            .is_some_and(|mut canvas| canvas.pan_by(delta));
                        if changed {
                            note_canvas_metadata_edit(world, drag.target);
                        }
                    } else if drag.moving_objects {
                        let zoom = world
                            .get::<Canvas>(drag.target)
                            .map_or(1.0, |canvas| canvas.zoom);
                        move_selected_objects(
                            world,
                            drag.target,
                            [delta[0] / zoom, delta[1] / zoom],
                        );
                    }
                    drag.last = local;
                }
                world
                    .resource_mut::<CanvasInputState>()
                    .0
                    .insert(window, drag);
            }
            UiRoutedEvent::PointerUp {
                window,
                button,
                cancelled,
                ..
            } => {
                let Some(drag) = world.resource_mut::<CanvasInputState>().0.remove(&window) else {
                    continue;
                };
                if drag.button != button {
                    world
                        .resource_mut::<CanvasInputState>()
                        .0
                        .insert(window, drag);
                    continue;
                }
                finish_canvas_drag(world, drag, cancelled);
            }
            UiRoutedEvent::PointerCancel { window, .. } => {
                if let Some(drag) = world.resource_mut::<CanvasInputState>().0.remove(&window) {
                    finish_canvas_drag(world, drag, true);
                }
            }
            UiRoutedEvent::Wheel {
                target,
                position,
                delta,
                unit,
                ..
            } => {
                let Some(target) = canvas_owner(world, target) else {
                    continue;
                };
                let Some((local, _)) = local_point(world, target, position) else {
                    continue;
                };
                let amount = match unit {
                    UiWheelUnit::Lines => delta[1],
                    UiWheelUnit::Pixels => delta[1] / 48.0,
                };
                edit_canvas(world, target, |canvas| {
                    canvas.zoom_at(local, 1.1_f32.powf(amount))
                });
            }
            UiRoutedEvent::Key {
                target,
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let Some(target) = canvas_owner(world, target) else {
                    continue;
                };
                if key == UiKeyCode::Delete || key == UiKeyCode::Backspace {
                    edit_canvas_structure(world, target, |canvas| canvas.remove_selected() > 0);
                } else if matches!(
                    key,
                    UiKeyCode::ArrowLeft
                        | UiKeyCode::ArrowRight
                        | UiKeyCode::ArrowUp
                        | UiKeyCode::ArrowDown
                ) {
                    let step = if modifiers.shift {
                        10.0
                    } else if modifiers.control || modifiers.super_key {
                        0.1
                    } else {
                        1.0
                    };
                    let delta = match key {
                        UiKeyCode::ArrowLeft => [-step, 0.0],
                        UiKeyCode::ArrowRight => [step, 0.0],
                        UiKeyCode::ArrowUp => [0.0, -step],
                        _ => [0.0, step],
                    };
                    edit_canvas_move_selected(world, target, delta);
                } else if matches!(key, UiKeyCode::PageUp | UiKeyCode::PageDown) {
                    if let Some(layout) = world.get::<UiLayoutResult>(target) {
                        let center = [layout.rect.width * 0.5, layout.rect.height * 0.5];
                        let factor = if key == UiKeyCode::PageUp {
                            1.2
                        } else {
                            1.0 / 1.2
                        };
                        edit_canvas(world, target, |canvas| canvas.zoom_at(center, factor));
                    }
                } else if key == UiKeyCode::Home {
                    edit_canvas(world, target, |canvas| {
                        let changed = canvas.pan != [0.0, 0.0] || canvas.zoom != 1.0;
                        canvas.pan = [0.0, 0.0];
                        canvas.zoom = 1.0;
                        changed
                    });
                } else if key == UiKeyCode::A && (modifiers.control || modifiers.super_key) {
                    edit_canvas(world, target, |canvas| {
                        let before = canvas.selected.len();
                        canvas.selected.extend(
                            canvas
                                .objects
                                .iter()
                                .filter(|object| !object.locked)
                                .map(|object| object.id),
                        );
                        canvas.selected.len() != before
                    });
                } else if key == UiKeyCode::Escape {
                    let windows: Vec<_> = world
                        .resource::<CanvasInputState>()
                        .0
                        .iter()
                        .filter_map(|(window, drag)| (drag.target == target).then_some(*window))
                        .collect();
                    for window in windows {
                        if let Some(drag) =
                            world.resource_mut::<CanvasInputState>().0.remove(&window)
                        {
                            finish_canvas_drag(world, drag, true);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn begin_canvas_drag(
    world: &mut World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
    modifiers: UiModifiers,
) {
    if !matches!(button, UiPointerButton::Primary | UiPointerButton::Middle) {
        return;
    }
    let Some(target) = canvas_owner(world, target) else {
        return;
    };
    let Some((local, viewport)) = local_point(world, target, position) else {
        return;
    };
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let moving_objects = if button == UiPointerButton::Primary {
        let hit = indexed_canvas_hit_test(world, target, local, viewport);
        let additive = modifiers.control || modifiers.super_key || modifiers.shift;
        let (selected, selection_changed) = {
            let Some(mut canvas) = world.get_mut::<Canvas>(target) else {
                return;
            };
            let selection_changed =
                if additive || !hit.is_some_and(|id| canvas.selected.contains(&id)) {
                    canvas.select_hit(hit, additive)
                } else {
                    false
                };
            (
                hit.is_some_and(|id| canvas.selected.contains(&id)),
                selection_changed,
            )
        };
        if selection_changed {
            note_canvas_metadata_edit(world, target);
        }
        selected
    } else {
        false
    };
    world.resource_mut::<CanvasInputState>().0.insert(
        window,
        CanvasDrag {
            target,
            button,
            last: local,
            before,
            moving_objects,
        },
    );
}

fn canvas_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<Canvas>(target).is_some() {
        Some(target)
    } else {
        world
            .get::<CanvasObjectControl>(target)
            .map(|control| control.owner)
    }
}

fn local_point(world: &World, target: Entity, screen: [f32; 2]) -> Option<([f32; 2], UiRect)> {
    world.get::<Canvas>(target)?;
    let layout = world.get::<UiLayoutResult>(target)?;
    let viewport = UiRect::new(0.0, 0.0, layout.rect.width, layout.rect.height);
    Some((
        [screen[0] - layout.rect.x, screen[1] - layout.rect.y],
        viewport,
    ))
}

fn edit_canvas(world: &mut World, target: Entity, edit: impl FnOnce(&mut Canvas) -> bool) {
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<Canvas>(target)
        .is_some_and(|mut canvas| edit(&mut canvas));
    if changed {
        note_canvas_metadata_edit(world, target);
        emit_canvas_edit(world, target, before);
    }
}

fn edit_canvas_structure(
    world: &mut World,
    target: Entity,
    edit: impl FnOnce(&mut Canvas) -> bool,
) {
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<Canvas>(target)
        .is_some_and(|mut canvas| edit(&mut canvas));
    if changed {
        emit_canvas_edit(world, target, before);
    }
}

fn edit_canvas_move_selected(world: &mut World, target: Entity, delta: [f32; 2]) {
    ensure_canvas_spatial_index(world, target);
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = move_selected_objects(world, target, delta);
    if changed {
        emit_canvas_edit(world, target, before);
    }
}

fn selected_canvas_object_positions(world: &World, target: Entity) -> Option<Vec<usize>> {
    let canvas = world.get::<Canvas>(target)?;
    let indexes = world.get_resource::<CanvasSpatialIndexes>()?;
    let mut positions = Vec::with_capacity(canvas.selected.len());
    for id in &canvas.selected {
        positions.push(indexes.position(target, *id)?);
    }
    Some(positions)
}

fn move_selected_objects(world: &mut World, target: Entity, delta: [f32; 2]) -> bool {
    ensure_canvas_spatial_index(world, target);
    let Some(positions) = selected_canvas_object_positions(world, target) else {
        let changed = world
            .get_mut::<Canvas>(target)
            .is_some_and(|mut canvas| canvas.move_selected(delta));
        return changed;
    };
    let changed = world
        .get_mut::<Canvas>(target)
        .is_some_and(|mut canvas| canvas.move_selected_at_indices(&positions, delta));
    if changed {
        note_canvas_object_edits(world, target, &positions);
    }
    changed
}

fn indexed_canvas_hit_test(
    world: &World,
    target: Entity,
    position: [f32; 2],
    viewport: UiRect,
) -> Option<u64> {
    let canvas = world.get::<Canvas>(target)?;
    let indexes = world.get_resource::<CanvasSpatialIndexes>()?;
    let index = indexes.0.get(&target)?;
    Some(canvas.hit_test_indexed(index, position, viewport)).flatten()
}

fn finish_canvas_drag(world: &mut World, drag: CanvasDrag, cancelled: bool) {
    if cancelled {
        if let Ok(previous) = Canvas::from_json(&drag.before)
            && let Some(mut canvas) = world.get_mut::<Canvas>(drag.target)
        {
            *canvas = previous;
        }
    } else {
        emit_canvas_edit(world, drag.target, drag.before);
    }
}

fn emit_canvas_edit(world: &mut World, target: Entity, before: String) {
    let Some(after) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    if before != after {
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::CanvasCommitted {
                target,
                before,
                after,
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipped_hit_testing_respects_z_order_and_transform() {
        let mut canvas = Canvas::default();
        let back = canvas
            .add(
                [10.0, 10.0],
                CanvasShape::Rectangle { size: [40.0, 40.0] },
                [1.0; 4],
            )
            .unwrap();
        let front = canvas
            .add(
                [20.0, 20.0],
                CanvasShape::Ellipse { size: [40.0, 40.0] },
                [1.0; 4],
            )
            .unwrap();
        assert!(canvas.zoom_at([50.0, 50.0], 2.0));
        let point = canvas.world_to_screen([40.0, 40.0]);
        let viewport = UiRect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(canvas.hit_test(point, viewport), Some(front));
        assert!(canvas.select_at(point, viewport, false));
        assert_eq!(canvas.selected, BTreeSet::from([front]));
        assert_eq!(
            canvas.hit_test(point, UiRect::new(0.0, 0.0, 10.0, 10.0)),
            None
        );
        canvas.objects[1].locked = true;
        assert_eq!(canvas.hit_test(point, viewport), Some(back));
    }

    #[test]
    fn spatial_index_paints_visible_objects_and_tracks_one_object_move() {
        let mut canvas = Canvas::default();
        for index in 0..100_000 {
            let (x, y) = if index < 100 {
                ((index % 10) as f32 * 10.0, (index / 10) as f32 * 10.0)
            } else {
                (1_000.0 + index as f32 * 12.0, 1_000.0)
            };
            canvas
                .add(
                    [x, y],
                    CanvasShape::Rectangle { size: [8.0, 8.0] },
                    [1.0; 4],
                )
                .unwrap();
        }
        let moved = canvas.objects[0].id;
        canvas.selected.insert(moved);
        let viewport = UiRect::new(0.0, 0.0, 100.0, 100.0);
        let clip = ClipRect::from(viewport);
        let theme = Theme::default();
        let index = CanvasSpatialIndex::new(&canvas);
        let visible = index.visible_indices(&canvas, viewport, clip, 8.0).unwrap();
        assert_eq!(visible.len(), 100);

        let mut full = Vec::new();
        let mut indexed = Vec::new();
        paint_canvas(&canvas, viewport, clip, 0, &theme, None, &mut full);
        paint_canvas_indexed(
            &canvas,
            &visible,
            viewport,
            clip,
            0,
            &theme,
            None,
            &mut indexed,
        );
        assert_eq!(indexed, full);
        assert_eq!(indexed.len(), 106);

        let mut world = World::new();
        let window = world.spawn_empty().id();
        let owner = world
            .spawn((
                UiNode,
                canvas,
                UiLayoutResult {
                    window,
                    rect: viewport,
                    clip: None,
                    z: 0,
                    content_size: [viewport.width, viewport.height],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        world.insert_resource(CanvasSpatialIndexes::default());
        world.insert_resource(CanvasObjectItems::default());
        world.insert_resource(UiDirty::default());
        assert!(ensure_canvas_spatial_index(&mut world, owner));
        sync_canvas_object_views(&mut world);
        assert_eq!(world.resource::<CanvasObjectItems>().0[&owner].len(), 100);
        let moved_entity = world.resource::<CanvasObjectItems>().0[&owner][&moved];
        assert_eq!(
            world
                .resource_mut::<UiDirty>()
                .take(UiDirtyDomain::Paint)
                .entities
                .len(),
            100
        );

        assert!(move_selected_objects(&mut world, owner, [10.0, 0.0]));
        sync_canvas_object_views(&mut world);
        let moved_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
        assert_eq!(moved_batch.entities, HashSet::from([moved_entity]));
        assert_eq!(world.resource::<CanvasObjectItems>().0[&owner].len(), 100);

        assert!(move_selected_objects(&mut world, owner, [200.0, 0.0]));
        sync_canvas_object_views(&mut world);
        let removed_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
        assert_eq!(removed_batch.entities, HashSet::from([moved_entity]));
        assert!(world.get_entity(moved_entity).is_err());
        let directly_edited_id = world.get::<Canvas>(owner).unwrap().objects[1].id;
        let directly_edited_entity =
            world.resource::<CanvasObjectItems>().0[&owner][&directly_edited_id];
        assert!(edit_canvas_object(
            &mut world,
            owner,
            directly_edited_id,
            |object| object.color[0] = 0.5,
        ));
        sync_canvas_object_views(&mut world);
        let tracked_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
        assert_eq!(
            tracked_batch.entities,
            HashSet::from([directly_edited_entity])
        );
        assert!(!edit_canvas_object(
            &mut world,
            owner,
            directly_edited_id,
            |object| object.id = 0,
        ));
        assert_eq!(
            world.get::<Canvas>(owner).unwrap().objects[1].id,
            directly_edited_id
        );
        world.get_mut::<Canvas>(owner).unwrap().objects[1].color[0] = 0.25;
        sync_canvas_object_views(&mut world);
        let direct_batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
        assert_eq!(
            direct_batch.entities,
            HashSet::from([directly_edited_entity])
        );
        let canvas = world.get::<Canvas>(owner).unwrap();
        assert_eq!(canvas.objects[0].origin, [210.0, 0.0]);
        let indexes = world.resource::<CanvasSpatialIndexes>();
        let visible_after = indexes
            .0
            .get(&owner)
            .unwrap()
            .visible_indices(canvas, viewport, clip, 8.0)
            .unwrap();
        assert_eq!(visible_after.len(), 99);
    }

    #[test]
    fn line_move_zoom_anchor_and_serialization_are_stable() {
        let mut canvas = Canvas::default();
        let id = canvas
            .add(
                [0.0, 0.0],
                CanvasShape::Line { end: [100.0, 0.0] },
                [0.5, 0.4, 0.3, 1.0],
            )
            .unwrap();
        canvas.selected.insert(id);
        assert!(canvas.move_selected([10.0, 20.0]));
        assert_eq!(
            canvas.hit_test([50.0, 24.0], UiRect::new(0.0, 0.0, 200.0, 100.0)),
            Some(id)
        );
        let anchor = canvas.screen_to_world([50.0, 40.0]);
        assert!(canvas.zoom_at([50.0, 40.0], 1.5));
        let after = canvas.screen_to_world([50.0, 40.0]);
        assert!((anchor[0] - after[0]).abs() < 0.0001);
        assert!((anchor[1] - after[1]).abs() < 0.0001);
        let restored = Canvas::from_json(&canvas.to_json().unwrap()).unwrap();
        assert_eq!(restored, canvas);
        assert_eq!(
            restored.objects[0].bounds(),
            UiRect::new(10.0, 20.0, 100.0, 0.0)
        );
    }

    #[test]
    fn invalid_edits_and_snapshots_do_not_corrupt_canvas() {
        let mut canvas = Canvas::default();
        assert_eq!(
            canvas.add(
                [0.0, 0.0],
                CanvasShape::Image {
                    size: [0.0, 2.0],
                    asset: "a.png".to_owned()
                },
                [1.0; 4]
            ),
            None
        );
        assert_eq!(
            canvas.add(
                [0.0, 0.0],
                CanvasShape::Image {
                    size: [10.0, 10.0],
                    asset: "../outside.png".to_owned(),
                },
                [1.0; 4]
            ),
            None
        );
        assert!(!canvas.pan_by([f32::NAN, 1.0]));
        assert!(!canvas.zoom_at([0.0, 0.0], -1.0));
        assert_eq!(
            canvas.add(
                [f32::MAX * 0.75, 0.0],
                CanvasShape::Rectangle {
                    size: [f32::MAX * 0.75, 10.0],
                },
                [1.0; 4],
            ),
            None
        );
        assert_eq!(canvas, Canvas::default());
        let id = canvas
            .add(
                [0.0, 0.0],
                CanvasShape::Image {
                    size: [10.0, 10.0],
                    asset: "a.png".to_owned(),
                },
                [1.0; 4],
            )
            .unwrap();
        canvas.selected.insert(id);
        canvas.objects[0].locked = true;
        assert_eq!(canvas.remove_selected(), 0);
        assert_eq!(canvas.objects.len(), 1);
        canvas.objects[0].locked = false;
        canvas.objects[0].origin = [f32::MAX * 0.75, 0.0];
        assert!(!canvas.move_selected([f32::MAX * 0.75, 0.0]));
        assert_eq!(canvas.objects[0].origin, [f32::MAX * 0.75, 0.0]);
        let mut saved: serde_json::Value =
            serde_json::from_str(&canvas.to_json().unwrap()).unwrap();
        saved["objects"][0]["id"] = serde_json::json!(999);
        assert!(Canvas::from_json(&saved.to_string()).is_err());
    }

    #[test]
    fn pointer_move_commits_once_and_cancel_restores_original() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<WidgetEventQueue>();
        world.init_resource::<CanvasInputState>();
        let window = world.spawn_empty().id();
        let mut canvas = Canvas::default();
        let id = canvas
            .add(
                [10.0, 10.0],
                CanvasShape::Rectangle { size: [40.0, 40.0] },
                [1.0; 4],
            )
            .unwrap();
        let target = world
            .spawn((
                canvas,
                UiLayoutResult {
                    window,
                    rect: UiRect::new(100.0, 200.0, 100.0, 100.0),
                    clip: None,
                    z: 0,
                    content_size: [100.0, 100.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        world.resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [120.0, 220.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [130.0, 225.0],
                delta: [10.0, 5.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [130.0, 225.0],
                cancelled: false,
            },
        ];
        process_canvas_input(&mut world);
        let canvas = world.get::<Canvas>(target).unwrap();
        assert_eq!(canvas.objects[0].origin, [20.0, 15.0]);
        assert_eq!(canvas.selected, BTreeSet::from([id]));
        assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 1);

        world.resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [130.0, 225.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [140.0, 240.0],
                delta: [10.0, 15.0],
                dragging: true,
            },
            UiRoutedEvent::PointerCancel { window, target },
        ];
        process_canvas_input(&mut world);
        assert_eq!(
            world.get::<Canvas>(target).unwrap().objects[0].origin,
            [20.0, 15.0]
        );
        assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 1);
        world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target,
            position: [150.0, 250.0],
            delta: [0.0, 2.0],
            unit: UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        }];
        process_canvas_input(&mut world);
        assert!(world.get::<Canvas>(target).unwrap().zoom > 1.0);
        assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 2);
        world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target,
            key: UiKeyCode::Home,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        process_canvas_input(&mut world);
        assert_eq!(world.get::<Canvas>(target).unwrap().zoom, 1.0);
        assert_eq!(world.get::<Canvas>(target).unwrap().pan, [0.0, 0.0]);
        assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 3);
    }

    #[test]
    fn routed_keyboard_selects_moves_zooms_resets_deletes_and_cancels_drag() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<WidgetEventQueue>();
        world.init_resource::<CanvasInputState>();
        let window = world.spawn_empty().id();
        let mut canvas = Canvas::default();
        let movable = canvas
            .add(
                [10.0, 10.0],
                CanvasShape::Rectangle { size: [20.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        let locked = canvas
            .add(
                [60.0, 10.0],
                CanvasShape::Rectangle { size: [20.0, 20.0] },
                [0.5; 4],
            )
            .unwrap();
        canvas.objects[1].locked = true;
        let target = world
            .spawn((
                canvas,
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z: 0,
                    content_size: [100.0, 100.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        let key = |key, modifiers| UiRoutedEvent::Key {
            window,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        };

        world.resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..Default::default()
            },
        )];
        process_canvas_input(&mut world);
        assert_eq!(
            world.get::<Canvas>(target).unwrap().selected,
            BTreeSet::from([movable]),
            "Ctrl+A selects unlocked objects and skips locked objects"
        );

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::ArrowRight, UiModifiers::default())];
        process_canvas_input(&mut world);
        world.resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::ArrowDown,
            UiModifiers {
                shift: true,
                ..Default::default()
            },
        )];
        process_canvas_input(&mut world);
        world.resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::ArrowLeft,
            UiModifiers {
                control: true,
                ..Default::default()
            },
        )];
        process_canvas_input(&mut world);
        assert_eq!(
            world.get::<Canvas>(target).unwrap().objects[0].origin,
            [10.9, 20.0]
        );

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::PageUp, UiModifiers::default())];
        process_canvas_input(&mut world);
        assert!(world.get::<Canvas>(target).unwrap().zoom > 1.0);
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::PageDown, UiModifiers::default())];
        process_canvas_input(&mut world);
        assert!((world.get::<Canvas>(target).unwrap().zoom - 1.0).abs() < 0.0001);

        {
            let mut canvas = world.get_mut::<Canvas>(target).unwrap();
            canvas.pan = [12.0, -4.0];
            canvas.zoom = 2.0;
        }
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::Home, UiModifiers::default())];
        process_canvas_input(&mut world);
        assert_eq!(world.get::<Canvas>(target).unwrap().pan, [0.0, 0.0]);
        assert_eq!(world.get::<Canvas>(target).unwrap().zoom, 1.0);

        let original = world.get::<Canvas>(target).unwrap().objects[0].origin;
        world.resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [original[0] + 2.0, original[1] + 2.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [original[0] + 12.0, original[1] + 2.0],
                delta: [10.0, 0.0],
                dragging: true,
            },
            key(UiKeyCode::Escape, UiModifiers::default()),
        ];
        process_canvas_input(&mut world);
        assert_eq!(
            world.get::<Canvas>(target).unwrap().objects[0].origin,
            original
        );
        assert!(!world.resource::<CanvasInputState>().0.contains_key(&window));

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::Delete, UiModifiers::default())];
        process_canvas_input(&mut world);
        let canvas = world.get::<Canvas>(target).unwrap();
        assert_eq!(canvas.objects.len(), 1);
        assert_eq!(canvas.objects[0].id, locked);
        assert!(canvas.selected.is_empty());
    }

    #[test]
    fn paint_uses_the_same_zoomed_coordinates_and_clip_as_hit_testing() {
        let mut canvas = Canvas::default();
        let id = canvas
            .add(
                [10.0, 20.0],
                CanvasShape::Ellipse { size: [30.0, 40.0] },
                [0.2, 0.4, 0.6, 1.0],
            )
            .unwrap();
        canvas.zoom_at([0.0, 0.0], 2.0);
        canvas.selected.insert(id);
        let rect = UiRect::new(100.0, 200.0, 200.0, 150.0);
        let clip = ClipRect::from(UiRect::new(100.0, 200.0, 150.0, 100.0));
        let mut items = Vec::new();
        paint_canvas(&canvas, rect, clip, 5, &Theme::default(), None, &mut items);
        assert!(items.iter().all(|item| item.clip == clip));
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::Ellipse { rect: shape, .. }
                if shape.x == 120.0 && shape.y == 240.0 && shape.width == 60.0 && shape.height == 80.0
        )));
        assert_eq!(
            canvas.hit_test([40.0, 60.0], UiRect::new(0.0, 0.0, 200.0, 150.0)),
            Some(id)
        );
        canvas
            .add(
                [50.0, 10.0],
                CanvasShape::Image {
                    size: [20.0, 30.0],
                    asset: "images/sample.png".to_owned(),
                },
                [1.0; 4],
            )
            .unwrap();
        let mut images = CanvasImageAssets::default();
        images.ids.insert("images/sample.png".to_owned(), 57);
        images
            .states
            .insert("images/sample.png".to_owned(), CanvasImageState::Ready);
        items.clear();
        paint_canvas(
            &canvas,
            rect,
            clip,
            5,
            &Theme::default(),
            Some(&images),
            &mut items,
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Icon { asset_id: 57, .. }))
        );
        images.states.insert(
            "images/sample.png".to_owned(),
            CanvasImageState::Failed("missing file".to_owned()),
        );
        items.clear();
        paint_canvas(
            &canvas,
            rect,
            clip,
            5,
            &Theme::default(),
            Some(&images),
            &mut items,
        );
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::Text { text, .. } if text.contains("Image error: missing file")
        )));
    }

    #[test]
    fn paint_culls_objects_outside_the_clipped_viewport() {
        let mut canvas = Canvas::default();
        let visible = canvas
            .add(
                [5.0, 5.0],
                CanvasShape::Rectangle { size: [10.0, 10.0] },
                [1.0; 4],
            )
            .unwrap();
        for x in 0..10_000 {
            let id = canvas
                .add(
                    [200.0 + x as f32 * 20.0, 5.0],
                    CanvasShape::Rectangle { size: [10.0, 10.0] },
                    [1.0; 4],
                )
                .unwrap();
            canvas.selected.insert(id);
        }
        canvas.selected.insert(visible);
        let mut items = Vec::new();
        paint_canvas(
            &canvas,
            UiRect::new(0.0, 0.0, 100.0, 100.0),
            ClipRect::from(UiRect::new(0.0, 0.0, 50.0, 50.0)),
            0,
            &Theme::default(),
            None,
            &mut items,
        );
        assert_eq!(items.len(), 7); // background, object, selected fill, and four edges
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { color, .. }
                if color == Theme::default().color(ThemeColorToken::CanvasSelectionOverlay)
        )));
    }

    #[test]
    fn paint_preserves_a_line_touching_the_viewport_edge() {
        let mut canvas = Canvas::default();
        canvas
            .add(
                [-10.0, 20.0],
                CanvasShape::Line { end: [0.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        let mut items = Vec::new();
        paint_canvas(
            &canvas,
            UiRect::new(0.0, 0.0, 100.0, 100.0),
            ClipRect::from(UiRect::new(0.0, 0.0, 50.0, 50.0)),
            0,
            &Theme::default(),
            None,
            &mut items,
        );
        assert_eq!(items.len(), 2);
        assert!(matches!(items[1].primitive, Primitive::Line { .. }));
    }

    #[test]
    fn paint_skips_finite_geometry_that_overflows_the_view_transform() {
        let mut canvas = Canvas::default();
        canvas
            .add(
                [f32::MAX / 4.0, 0.0],
                CanvasShape::Rectangle { size: [10.0, 10.0] },
                [1.0; 4],
            )
            .unwrap();
        assert!(canvas.zoom_at([0.0, 0.0], 32.0));
        let mut items = Vec::new();
        paint_canvas(
            &canvas,
            UiRect::new(0.0, 0.0, 100.0, 100.0),
            ClipRect::from(UiRect::new(0.0, 0.0, 50.0, 50.0)),
            0,
            &Theme::default(),
            None,
            &mut items,
        );
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn image_assets_follow_viewport_and_visibility() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut canvas = Canvas::default();
        for (asset, origin) in [
            ("visible.png", [10.0, 10.0]),
            ("offscreen.png", [300.0, 300.0]),
        ] {
            canvas
                .add(
                    origin,
                    CanvasShape::Image {
                        size: [40.0, 40.0],
                        asset: asset.to_owned(),
                    },
                    [1.0; 4],
                )
                .unwrap();
        }
        let entity = world
            .spawn((
                canvas,
                UiVisible(true),
                UiLayoutResult {
                    window,
                    rect: UiRect::new(100.0, 200.0, 100.0, 100.0),
                    clip: Some(UiRect::new(100.0, 200.0, 80.0, 80.0)),
                    z: 0,
                    content_size: [100.0, 100.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        assert_eq!(
            visible_canvas_image_paths(&mut world),
            HashSet::from(["visible.png".to_owned()])
        );
        world.get_mut::<UiVisible>(entity).unwrap().0 = false;
        assert!(visible_canvas_image_paths(&mut world).is_empty());
    }

    #[test]
    fn modifier_annotated_click_adds_to_canvas_selection_once() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<WidgetEventQueue>();
        world.init_resource::<CanvasInputState>();
        let window = world.spawn_empty().id();
        let mut canvas = Canvas::default();
        let first = canvas
            .add(
                [0.0, 0.0],
                CanvasShape::Rectangle { size: [20.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        let second = canvas
            .add(
                [40.0, 0.0],
                CanvasShape::Rectangle { size: [20.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        let target = world
            .spawn((
                canvas,
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z: 0,
                    content_size: [100.0, 100.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        for (position, modifiers) in [
            ([10.0, 10.0], UiModifiers::default()),
            (
                [50.0, 10.0],
                UiModifiers {
                    control: true,
                    ..Default::default()
                },
            ),
        ] {
            world.resource_mut::<UiRoutedEvents>().events = vec![
                UiRoutedEvent::PointerDown {
                    window,
                    target,
                    button: UiPointerButton::Primary,
                    position,
                },
                UiRoutedEvent::PointerDownWithModifiers {
                    window,
                    target,
                    button: UiPointerButton::Primary,
                    position,
                    modifiers,
                },
                UiRoutedEvent::PointerUp {
                    window,
                    target,
                    button: UiPointerButton::Primary,
                    position,
                    cancelled: false,
                },
            ];
            process_canvas_input(&mut world);
        }
        assert_eq!(
            world.get::<Canvas>(target).unwrap().selected,
            BTreeSet::from([first, second])
        );
        assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 2);
        world.resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [10.0, 10.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [20.0, 15.0],
                delta: [10.0, 5.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [20.0, 15.0],
                cancelled: false,
            },
        ];
        process_canvas_input(&mut world);
        let canvas = world.get::<Canvas>(target).unwrap();
        assert_eq!(canvas.selected, BTreeSet::from([first, second]));
        assert_eq!(canvas.objects[0].origin, [10.0, 5.0]);
        assert_eq!(canvas.objects[1].origin, [50.0, 5.0]);
        assert_eq!(world.resource::<WidgetEventQueue>().0.len(), 3);
    }

    #[test]
    fn object_controls_keep_identity_and_virtualize_offscreen_content() {
        let mut world = World::new();
        world.init_resource::<CanvasObjectItems>();
        let window = world.spawn_empty().id();
        let mut canvas = Canvas::default();
        let visible = canvas
            .add(
                [10.0, 10.0],
                CanvasShape::Rectangle { size: [20.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        canvas
            .add(
                [1000.0, 1000.0],
                CanvasShape::Ellipse { size: [20.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        let owner = world
            .spawn((
                UiNode,
                canvas,
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z: 3,
                    content_size: [100.0, 100.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        sync_canvas_object_views(&mut world);
        let controls = &world.resource::<CanvasObjectItems>().0[&owner];
        assert_eq!(controls.len(), 1);
        let entity = controls[&visible];
        assert_eq!(
            world.get::<CanvasObjectControl>(entity).unwrap().owner,
            owner
        );
        assert_eq!(world.get::<UiParent>(entity).unwrap().0, owner);
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<WidgetEventQueue>();
        world.init_resource::<CanvasInputState>();
        world.resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: [20.0, 20.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: [25.0, 20.0],
                delta: [5.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: [25.0, 20.0],
                cancelled: false,
            },
        ];
        process_canvas_input(&mut world);
        assert_eq!(
            world.get::<Canvas>(owner).unwrap().objects[0].origin,
            [15.0, 10.0]
        );
        world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: entity,
            key: UiKeyCode::ArrowRight,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        process_canvas_input(&mut world);
        assert_eq!(
            world.get::<Canvas>(owner).unwrap().objects[0].origin,
            [16.0, 10.0]
        );
        sync_canvas_object_views(&mut world);
        assert_eq!(
            world.resource::<CanvasObjectItems>().0[&owner][&visible],
            entity
        );
        assert!(world.get::<CanvasObjectControl>(entity).unwrap().selected);
        world.get_mut::<Canvas>(owner).unwrap().pan = [-500.0, -500.0];
        sync_canvas_object_views(&mut world);
        assert!(world.resource::<CanvasObjectItems>().0[&owner].is_empty());
        assert!(world.get_entity(entity).is_err());
    }

    #[test]
    fn spatial_indexes_are_pruned_when_canvas_owners_are_despawned() {
        let mut world = World::new();
        world.init_resource::<CanvasSpatialIndexes>();
        world.init_resource::<CanvasObjectItems>();
        let window = world.spawn_empty().id();
        let owner = world
            .spawn((
                UiNode,
                Canvas::default(),
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, 0.0, 100.0, 100.0),
                    clip: None,
                    z: 0,
                    content_size: [100.0, 100.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();

        sync_canvas_object_views(&mut world);
        assert!(
            world
                .resource::<CanvasSpatialIndexes>()
                .0
                .contains_key(&owner)
        );

        world.despawn(owner);
        sync_canvas_object_views(&mut world);

        assert!(
            !world
                .resource::<CanvasSpatialIndexes>()
                .0
                .contains_key(&owner)
        );
        assert!(!world.resource::<CanvasObjectItems>().0.contains_key(&owner));
    }
}

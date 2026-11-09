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

mod model;
pub use model::{Canvas, CanvasObject, CanvasShape};

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

mod edit;
pub(crate) use edit::note_canvas_metadata_edit;
use edit::*;
pub use edit::{edit_canvas_object, sync_canvas_object_views};

mod images;
use images::*;
pub use images::{CanvasImageState, sync_canvas_images};

mod paint;
pub use paint::{paint_canvas, paint_canvas_indexed};
pub(crate) use paint::{paint_canvas_background, paint_canvas_object_fragment};

mod input;
#[cfg(test)]
use input::move_selected_objects;
pub use input::{CanvasInputState, process_canvas_input};

#[cfg(test)]
#[path = "canvas/tests.rs"]
mod tests;

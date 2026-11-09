//! Shared measured layout geometry for rendering and pointer hit testing.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::lifecycle::RemovedComponentReader;
use bevy_ecs::prelude::*;

use bui_core::{
    UiChildren, UiDirty, UiDirtyDomain, UiHitBox, UiKeyCode, UiNode, UiParent, UiRoutedEvent,
    UiRoutedEvents, UiSystems, UiWheelUnit, UiZIndex,
};
use std::collections::{HashMap, HashSet};

pub use bui_core::UiRect as Rect;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum UiLength {
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
    /// Share available space with sibling `Fill` lengths by weight.
    Fill(f32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiSize {
    pub width: UiLength,
    pub height: UiLength,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiEdges {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl UiEdges {
    pub const ZERO: Self = Self {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub const fn horizontal_vertical(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    fn sanitized(self) -> Self {
        Self {
            left: nonnegative(self.left),
            top: nonnegative(self.top),
            right: nonnegative(self.right),
            bottom: nonnegative(self.bottom),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiFlexDirection {
    #[default]
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiLayoutMode {
    Leaf,
    Row,
    #[default]
    Column,
    Overlay,
    Grid {
        columns: usize,
    },
    /// Flex layout with optional wrapping along the main axis.
    Flex {
        direction: UiFlexDirection,
        wrap: bool,
    },
    /// Grid chooses as many equal-width columns as fit while keeping each cell
    /// at least `min_cell_width` logical pixels wide, up to `max_columns`.
    ResponsiveGrid {
        min_cell_width: u32,
        max_columns: usize,
    },
    /// Horizontal flow that wraps children onto additional lines.
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiAlign {
    Start,
    #[default]
    Stretch,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiJustify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Layout rules for one node. `Auto` sizes from its intrinsic size or children;
/// row/column containers distribute `Fill` lengths after fixed sizes and gaps.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiLayoutNode {
    pub mode: UiLayoutMode,
    pub size: UiSize,
    pub min_size: [Option<f32>; 2],
    pub max_size: [Option<f32>; 2],
    pub padding: UiEdges,
    pub margin: UiEdges,
    pub gap: f32,
    pub align_items: UiAlign,
    pub justify_content: UiJustify,
    pub clip_contents: bool,
    pub z_index: i32,
}

impl Default for UiLayoutNode {
    fn default() -> Self {
        Self {
            mode: UiLayoutMode::Column,
            size: UiSize::default(),
            min_size: [None, None],
            max_size: [None, None],
            padding: UiEdges::ZERO,
            margin: UiEdges::ZERO,
            gap: 0.0,
            align_items: UiAlign::Stretch,
            justify_content: UiJustify::Start,
            clip_contents: false,
            z_index: 0,
        }
    }
}

impl UiLayoutNode {
    pub fn row(gap: f32) -> Self {
        Self {
            mode: UiLayoutMode::Row,
            gap,
            ..Self::default()
        }
    }

    pub fn column(gap: f32) -> Self {
        Self {
            mode: UiLayoutMode::Column,
            gap,
            ..Self::default()
        }
    }

    pub fn flex(direction: UiFlexDirection, wrap: bool, gap: f32) -> Self {
        Self {
            mode: UiLayoutMode::Flex { direction, wrap },
            gap,
            ..Self::default()
        }
    }

    pub fn responsive_grid(min_cell_width: u32, max_columns: usize, gap: f32) -> Self {
        Self {
            mode: UiLayoutMode::ResponsiveGrid {
                min_cell_width,
                max_columns,
            },
            gap,
            ..Self::default()
        }
    }

    pub fn leaf(width: UiLength, height: UiLength) -> Self {
        Self {
            mode: UiLayoutMode::Leaf,
            size: UiSize { width, height },
            ..Self::default()
        }
    }
}

/// Optional per-item flex sizing. `basis` overrides the item's main-axis size;
/// positive `grow` and `shrink` weights distribute free and overflowing space.
/// The parent must use `UiLayoutMode::Flex` for these values to apply.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiFlexItem {
    pub basis: Option<UiLength>,
    pub grow: f32,
    pub shrink: f32,
    pub align_self: Option<UiAlign>,
}

impl Default for UiFlexItem {
    fn default() -> Self {
        Self {
            basis: None,
            grow: 0.0,
            shrink: 1.0,
            align_self: None,
        }
    }
}

/// Content-provided natural size, for example measured text or an image.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct UiIntrinsicSize(pub [f32; 2]);

/// Logical-pixel scroll position for a clipped layout container.
///
/// The layout pass clamps the requested offset to the measured content extent,
/// translates descendants by the effective offset, and uses the container rect
/// as their inherited clip. The same resulting rect and clip are written to
/// `UiHitBox`, so hit testing follows the scrolled content.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct UiScroll {
    pub offset: [f32; 2],
}

/// Marks a wheel target whose widget behavior owns wheel input, such as a
/// curve editor that zooms instead of scrolling its parent container.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UiWheelHandled;

impl UiScroll {
    pub const fn new() -> Self {
        Self { offset: [0.0, 0.0] }
    }

    pub const fn at(x: f32, y: f32) -> Self {
        Self { offset: [x, y] }
    }

    pub fn set_offset(&mut self, x: f32, y: f32) {
        self.offset = [x, y];
    }

    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        if dx.is_finite() {
            self.offset[0] += dx;
        }
        if dy.is_finite() {
            self.offset[1] += dy;
        }
    }

    fn clamp_to(&mut self, content_size: [f32; 2], viewport_size: [f32; 2]) -> bool {
        let previous = self.offset;
        for axis in 0..2 {
            let maximum =
                (nonnegative(content_size[axis]) - nonnegative(viewport_size[axis])).max(0.0);
            self.offset[axis] = if self.offset[axis].is_finite() {
                self.offset[axis].clamp(0.0, maximum)
            } else {
                0.0
            };
        }
        self.offset != previous
    }
}

/// A layout root provides logical viewport bounds and the current DPI scale.
/// Resize and DPI changes are picked up on the next `Layout` phase.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiLayoutRoot {
    pub window: Entity,
    pub viewport: Rect,
    pub dpi_scale: f32,
}

impl UiLayoutRoot {
    pub fn new(window: Entity, viewport: Rect, dpi_scale: f32) -> Self {
        Self {
            window,
            viewport,
            dpi_scale: positive_or_one(dpi_scale),
        }
    }
}

/// Shared result consumed by hit testing and the renderer.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiLayoutResult {
    /// Window or camera surface used as the hit-test coordinate space.
    pub window: Entity,
    pub rect: Rect,
    /// Inherited clip for painting and hit testing; `None` means unbounded.
    pub clip: Option<Rect>,
    pub z: i32,
    pub content_size: [f32; 2],
    pub dpi_scale: f32,
}

/// A viewport registry is convenient when roots represent Bevy windows.
#[derive(Resource, Debug, Default)]
pub struct UiViewports(pub std::collections::HashMap<Entity, UiViewport>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiViewport {
    pub rect: Rect,
    pub dpi_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLayoutIssue {
    MissingNode,
    InvalidViewport,
    Cycle,
}

#[derive(Resource, Debug, Default)]
pub struct UiLayoutIssues(pub Vec<(Entity, UiLayoutIssue)>);

/// Mark an entity for layout invalidation when its change is not represented
/// by an ECS component. The owning root is found during the next layout pass.
pub fn invalidate_ui_layout(world: &mut World, entity: Entity) {
    world.init_resource::<UiDirty>();
    world
        .resource_mut::<UiDirty>()
        .mark(UiDirtyDomain::Layout, entity);
}

/// Work performed by the most recent layout phase and cumulative totals.
/// `nodes_processed == 0` means the phase performed no geometry traversal.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UiLayoutStats {
    pub roots_processed: usize,
    pub nodes_processed: usize,
    pub frames: u64,
    pub idle_frames: u64,
    pub total_roots_processed: u64,
    pub total_nodes_processed: u64,
}

#[derive(Resource, Default)]
struct UiLayoutCache {
    initialized: bool,
    root_configs: HashMap<Entity, UiLayoutRoot>,
    viewports: HashMap<Entity, UiViewport>,
    root_members: HashMap<Entity, HashSet<Entity>>,
    entity_roots: HashMap<Entity, HashSet<Entity>>,
    input_snapshots: HashMap<Entity, LayoutInputs>,
    root_issues: HashMap<Entity, Vec<(Entity, UiLayoutIssue)>>,
    removed_nodes: RemovedComponentReader<UiNode>,
    removed_layout_nodes: RemovedComponentReader<UiLayoutNode>,
    removed_intrinsics: RemovedComponentReader<UiIntrinsicSize>,
    removed_parents: RemovedComponentReader<UiParent>,
    removed_children: RemovedComponentReader<UiChildren>,
    removed_z_indices: RemovedComponentReader<UiZIndex>,
    removed_scrolls: RemovedComponentReader<UiScroll>,
    removed_flex_items: RemovedComponentReader<UiFlexItem>,
    removed_roots: RemovedComponentReader<UiLayoutRoot>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LayoutInputs {
    is_node: bool,
    root: Option<UiLayoutRoot>,
    layout: Option<UiLayoutNode>,
    intrinsic: Option<UiIntrinsicSize>,
    parent: Option<UiParent>,
    children: Option<UiChildren>,
    z_index: Option<UiZIndex>,
    scroll: Option<UiScroll>,
    flex_item: Option<UiFlexItem>,
}

#[derive(Clone)]
struct LayoutInputChange {
    entity: Entity,
    previous: Option<LayoutInputs>,
    current: Option<LayoutInputs>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MeasureKind {
    Node,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MeasureKey {
    available: [u32; 2],
    kind: MeasureKind,
}

#[derive(Debug, Clone, Copy)]
struct MeasurementEntry {
    key: MeasureKey,
    size: [f32; 2],
}

const MAX_MEASUREMENT_ENTRIES: usize = 8;

#[derive(Default)]
struct MeasurementBucket {
    entries: [Option<MeasurementEntry>; MAX_MEASUREMENT_ENTRIES],
    next_replace: usize,
}

impl MeasurementBucket {
    fn get(&self, key: MeasureKey) -> Option<[f32; 2]> {
        self.entries
            .iter()
            .flatten()
            .find(|entry| entry.key == key)
            .map(|entry| entry.size)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    fn insert(&mut self, key: MeasureKey, size: [f32; 2]) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .flatten()
            .find(|entry| entry.key == key)
        {
            entry.size = size;
            return;
        }
        let entry = MeasurementEntry { key, size };
        if let Some(slot) = self.entries.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(entry);
        } else {
            self.entries[self.next_replace] = Some(entry);
            self.next_replace = (self.next_replace + 1) % MAX_MEASUREMENT_ENTRIES;
        }
    }
}

type MeasurementMap = HashMap<Entity, MeasurementBucket>;

fn measure_key(available: [f32; 2], kind: MeasureKind) -> MeasureKey {
    MeasureKey {
        available: [available[0].to_bits(), available[1].to_bits()],
        kind,
    }
}

#[derive(Resource, Default)]
struct UiMeasurementCache(MeasurementMap);

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiViewports>()
            .init_resource::<UiLayoutIssues>()
            .init_resource::<UiDirty>()
            .init_resource::<UiLayoutStats>()
            .init_resource::<UiLayoutCache>()
            .init_resource::<UiRoutedEvents>()
            .add_systems(Update, compute_layout.in_set(UiSystems::Layout))
            .add_systems(
                Update,
                apply_routed_scroll_input.in_set(UiSystems::LayoutAfterInput),
            );
    }
}

const WHEEL_LINE_PIXELS: f32 = 40.0;

/// Apply routed wheel and scroll-navigation keys after input routing. Wheel
/// deltas bubble from the deepest scroll ancestor outward; each ancestor
/// consumes only the distance available before its boundary. When an offset
/// changes, layout and stationary-pointer hover are refreshed before style,
/// text, and widget paint phases.
pub fn apply_routed_scroll_input(world: &mut World) {
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    let mut changed = false;
    for event in events {
        match event {
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                modifiers,
                ..
            } if !modifiers.control
                && !modifiers.super_key
                && world.get::<UiWheelHandled>(target).is_none() =>
            {
                let scale = match unit {
                    UiWheelUnit::Lines => WHEEL_LINE_PIXELS,
                    UiWheelUnit::Pixels => 1.0,
                };
                changed |= scroll_ancestors(world, target, [delta[0] * scale, -delta[1] * scale]);
            }
            UiRoutedEvent::Key {
                target,
                key,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.control && !modifiers.super_key => match key {
                UiKeyCode::PageUp => {
                    let extent = scroll_page_extent(world, target);
                    changed |= scroll_ancestors(world, target, [0.0, -extent]);
                }
                UiKeyCode::PageDown => {
                    let extent = scroll_page_extent(world, target);
                    changed |= scroll_ancestors(world, target, [0.0, extent]);
                }
                // Home and End are also caret keys. They affect a scroll view
                // only when that scroll view itself owns keyboard focus.
                UiKeyCode::Home if world.get::<UiScroll>(target).is_some() => {
                    changed |= set_scroll_vertical_edge(world, target, false);
                }
                UiKeyCode::End if world.get::<UiScroll>(target).is_some() => {
                    changed |= set_scroll_vertical_edge(world, target, true);
                }
                _ => {}
            },
            _ => {}
        }
    }
    if changed {
        compute_layout(world);
        bui_core::refresh_ui_hover(world);
    }
}

fn scroll_page_extent(world: &World, target: Entity) -> f32 {
    let mut current = Some(target);
    let mut visited = HashSet::new();
    while let Some(entity) = current {
        if !visited.insert(entity) {
            break;
        }
        if world.get::<UiScroll>(entity).is_some() {
            return world
                .get::<UiLayoutResult>(entity)
                .map_or(0.0, |layout| layout.rect.height);
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    0.0
}

fn scroll_ancestors(world: &mut World, target: Entity, mut remaining: [f32; 2]) -> bool {
    let mut current = Some(target);
    let mut visited = HashSet::new();
    let mut changed = false;
    while let Some(entity) = current {
        if !visited.insert(entity) || remaining.iter().all(|delta| delta.abs() < 0.001) {
            break;
        }
        if world.get::<UiWheelHandled>(entity).is_some() {
            break;
        }
        if world.get::<UiScroll>(entity).is_some() {
            let (remaining_delta, entity_changed) = consume_scroll_delta(world, entity, remaining);
            remaining = remaining_delta;
            changed |= entity_changed;
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    changed
}

fn consume_scroll_delta(world: &mut World, entity: Entity, delta: [f32; 2]) -> ([f32; 2], bool) {
    let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
        if let Some(mut scroll) = world.get_mut::<UiScroll>(entity) {
            let before = scroll.offset;
            scroll.scroll_by(delta[0], delta[1]);
            return ([0.0, 0.0], scroll.offset != before);
        }
        return (delta, false);
    };
    let maximum = [
        (layout.content_size[0] - layout.rect.width).max(0.0),
        (layout.content_size[1] - layout.rect.height).max(0.0),
    ];
    let Some(current) = world.get::<UiScroll>(entity).copied() else {
        return (delta, false);
    };
    let mut next = current.offset;
    let mut remaining = delta;
    for axis in 0..2 {
        let before = if next[axis].is_finite() {
            next[axis].clamp(0.0, maximum[axis])
        } else {
            0.0
        };
        let requested = if delta[axis].is_finite() {
            before + delta[axis]
        } else {
            before
        };
        next[axis] = requested.clamp(0.0, maximum[axis]);
        let consumed = next[axis] - before;
        remaining[axis] -= consumed;
    }
    if next != current.offset
        && let Some(mut scroll) = world.get_mut::<UiScroll>(entity)
    {
        scroll.offset = next;
    }
    (remaining, next != current.offset)
}

fn set_scroll_vertical_edge(world: &mut World, entity: Entity, end: bool) -> bool {
    let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
        return false;
    };
    let maximum = (layout.content_size[1] - layout.rect.height).max(0.0);
    if let Some(mut scroll) = world.get_mut::<UiScroll>(entity) {
        let target = if end { maximum } else { 0.0 };
        let changed = scroll.offset[1] != target;
        if changed {
            scroll.offset[1] = target;
        }
        changed
    } else {
        false
    }
}

/// Recompute invalidated layout paths and propagate geometry changes to affected
/// siblings and descendants. Measurements are memoized so unchanged branches
/// can supply cached intrinsic sizes without walking their subtrees.
pub fn compute_layout(world: &mut World) {
    ensure_layout_resources(world);
    let input_changes = collect_layout_changes(world);
    let batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Layout);
    let entities = batch.entities;
    let invalidate_all = batch.all;
    let initialized = world.resource::<UiLayoutCache>().initialized;
    let viewports_changed =
        world.resource::<UiViewports>().0 != world.resource::<UiLayoutCache>().viewports;
    let root_structure_changed = input_changes.iter().any(|change| {
        let previous = change.previous.as_ref();
        let current = change.current.as_ref();
        previous.and_then(|inputs| inputs.root) != current.and_then(|inputs| inputs.root)
            || previous.map(|inputs| inputs.is_node) != current.map(|inputs| inputs.is_node)
    });
    let rediscover_roots =
        !initialized || invalidate_all || viewports_changed || root_structure_changed;
    let previous_root_configs =
        std::mem::take(&mut world.resource_mut::<UiLayoutCache>().root_configs);
    let mut changed_root_configs = HashSet::new();
    let root_configs: HashMap<Entity, UiLayoutRoot> = if rediscover_roots {
        let current: HashMap<Entity, UiLayoutRoot> = effective_roots(world).into_iter().collect();
        for (root, info) in &current {
            if previous_root_configs.get(root) != Some(info) {
                changed_root_configs.insert(*root);
            }
        }
        for root in previous_root_configs.keys() {
            if !current.contains_key(root) {
                changed_root_configs.insert(*root);
            }
        }
        current
    } else {
        previous_root_configs
    };
    let mut dirty_roots = HashSet::new();
    let mut full_roots = HashSet::new();
    let mut local_only_entities = HashSet::new();
    let mut local_only_by_root: HashMap<Entity, Vec<Entity>> = HashMap::new();
    if !invalidate_all && input_changes.len() == 1 && entities.len() == 1 {
        let change = &input_changes[0];
        let memberships = world
            .resource::<UiLayoutCache>()
            .entity_roots
            .get(&change.entity);
        if memberships.is_some_and(|roots| roots.len() == 1)
            && is_independent_intrinsic_change(world, change)
        {
            let root = *memberships.unwrap().iter().next().unwrap();
            local_only_entities.insert(change.entity);
            local_only_by_root
                .entry(root)
                .or_default()
                .push(change.entity);
        }
    }
    if invalidate_all || !initialized {
        dirty_roots.extend(root_configs.keys().copied());
        full_roots.extend(dirty_roots.iter().copied());
    }
    dirty_roots.extend(changed_root_configs.iter().copied());
    full_roots.extend(changed_root_configs.iter().copied());

    for entity in &entities {
        collect_entity_roots(world, *entity, &root_configs, &mut dirty_roots);
    }
    for change in &input_changes {
        collect_entity_roots(world, change.entity, &root_configs, &mut dirty_roots);
        for parent in [
            change.previous.as_ref().and_then(|inputs| inputs.parent),
            change.current.as_ref().and_then(|inputs| inputs.parent),
        ]
        .into_iter()
        .flatten()
        {
            collect_entity_roots(world, parent.0, &root_configs, &mut dirty_roots);
        }
        let structural = match (&change.previous, &change.current) {
            (Some(previous), Some(current)) => {
                previous.is_node != current.is_node
                    || previous.parent != current.parent
                    || previous.children != current.children
            }
            _ => true,
        };
        if structural {
            collect_entity_roots(world, change.entity, &root_configs, &mut full_roots);
            for parent in [
                change.previous.as_ref().and_then(|inputs| inputs.parent),
                change.current.as_ref().and_then(|inputs| inputs.parent),
            ]
            .into_iter()
            .flatten()
            {
                collect_entity_roots(world, parent.0, &root_configs, &mut full_roots);
            }
        }
    }

    let mut measurements = std::mem::take(&mut world.resource_mut::<UiMeasurementCache>().0);
    if invalidate_all {
        measurements.clear();
    } else {
        let changed_entities: HashSet<Entity> =
            input_changes.iter().map(|change| change.entity).collect();
        for change in &input_changes {
            if local_only_entities.contains(&change.entity) {
                measurements.remove(&change.entity);
            } else {
                invalidate_measurement_path(
                    world,
                    change.entity,
                    change
                        .previous
                        .as_ref()
                        .and_then(|inputs| inputs.parent)
                        .map(|parent| parent.0),
                    &mut measurements,
                );
            }
        }
        for entity in &entities {
            if !changed_entities.contains(entity) {
                invalidate_measurement_path(world, *entity, None, &mut measurements);
            }
        }
        for root in &full_roots {
            let members = world
                .resource::<UiLayoutCache>()
                .root_members
                .get(root)
                .cloned()
                .unwrap_or_default();
            for entity in members {
                measurements.remove(&entity);
            }
        }
    }

    let mut required_by_root: HashMap<Entity, HashSet<Entity>> = HashMap::new();
    let incremental_roots: Vec<_> = dirty_roots
        .iter()
        .filter(|root| !full_roots.contains(root))
        .copied()
        .collect();
    for root in incremental_roots {
        let required = required_by_root.entry(root).or_default();
        for entity in &entities {
            if !local_only_entities.contains(entity) {
                add_path_to_root(world, *entity, root, required);
            }
        }
        for change in &input_changes {
            if !local_only_entities.contains(&change.entity) {
                add_path_to_root(world, change.entity, root, required);
                if let Some(parent) = change.current.as_ref().and_then(|inputs| inputs.parent) {
                    add_path_to_root(world, parent.0, root, required);
                }
            }
        }
        if required.is_empty() && !local_only_by_root.contains_key(&root) {
            full_roots.insert(root);
        }
    }

    let mut processed_roots = 0;
    let mut processed_nodes = 0;
    let mut stale_entities = HashSet::new();
    let mut clamped_scrolls = HashSet::new();
    let mut ordered_dirty: Vec<Entity> = dirty_roots.into_iter().collect();
    ordered_dirty.sort_unstable();

    for root in ordered_dirty {
        processed_roots += 1;
        let info = root_configs.get(&root).copied();
        let mut new_members = HashSet::new();
        let mut root_issues = Vec::new();
        if let Some(info) = info {
            if !info.viewport.is_finite() || info.viewport.is_empty() {
                root_issues.push((root, UiLayoutIssue::InvalidViewport));
            } else if world.get::<UiNode>(root).is_none() {
                root_issues.push((root, UiLayoutIssue::MissingNode));
            } else {
                let mut pass = LayoutPass {
                    window: info.window,
                    dpi_scale: positive_or_one(info.dpi_scale),
                    visiting: Vec::new(),
                    visited: HashSet::new(),
                    issues: Vec::new(),
                    clamped_scrolls: HashSet::new(),
                };
                let is_full = full_roots.contains(&root);
                let required = required_by_root.get(&root);
                if is_full || required.is_some_and(|required| !required.is_empty()) {
                    pass.layout_subtree(
                        world,
                        root,
                        AssignedRect::Raw(info.viewport),
                        None,
                        (!is_full).then_some(required).flatten(),
                        &mut measurements,
                    );
                }
                if !is_full && let Some(local_entities) = local_only_by_root.get(&root) {
                    for entity in local_entities {
                        if let Some(previous) = world.get::<UiLayoutResult>(*entity).copied() {
                            let local_required = HashSet::from([*entity]);
                            pass.layout_subtree(
                                world,
                                *entity,
                                AssignedRect::Resolved(previous.rect),
                                previous.clip,
                                Some(&local_required),
                                &mut measurements,
                            );
                        }
                    }
                }
                processed_nodes += pass.visited.len();
                if is_full {
                    new_members = pass.visited.clone();
                    root_issues = pass.issues;
                } else {
                    let previous_issues = world
                        .resource::<UiLayoutCache>()
                        .root_issues
                        .get(&root)
                        .cloned()
                        .unwrap_or_default();
                    root_issues = previous_issues
                        .into_iter()
                        .filter(|(entity, _)| !pass.visited.contains(entity))
                        .chain(pass.issues)
                        .collect();
                    // Membership is stable for non-structural invalidations.
                }
                clamped_scrolls.extend(pass.clamped_scrolls);
            }
        }

        let old_members = if full_roots.contains(&root) {
            world
                .resource::<UiLayoutCache>()
                .root_members
                .get(&root)
                .cloned()
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        {
            let mut cache = world.resource_mut::<UiLayoutCache>();
            if full_roots.contains(&root) {
                for entity in &old_members {
                    if let Some(roots) = cache.entity_roots.get_mut(entity) {
                        roots.remove(&root);
                        if roots.is_empty() {
                            cache.entity_roots.remove(entity);
                            stale_entities.insert(*entity);
                        }
                    }
                }
                for entity in &new_members {
                    cache.entity_roots.entry(*entity).or_default().insert(root);
                }
                if info.is_some() {
                    cache.root_members.insert(root, new_members);
                    cache.root_issues.insert(root, root_issues);
                } else {
                    cache.root_members.remove(&root);
                    cache.root_issues.remove(&root);
                }
            } else if info.is_some() {
                cache.root_issues.insert(root, root_issues);
            }
        }
    }

    for entity in stale_entities {
        if world.get_entity(entity).is_ok()
            && !world
                .resource::<UiLayoutCache>()
                .entity_roots
                .contains_key(&entity)
            && let Ok(mut entity_mut) = world.get_entity_mut(entity)
        {
            entity_mut.remove::<UiLayoutResult>();
            entity_mut.remove::<UiHitBox>();
        }
    }

    if !clamped_scrolls.is_empty() {
        let snapshots: Vec<_> = clamped_scrolls
            .into_iter()
            .filter(|entity| world.get_entity(*entity).is_ok())
            .map(|entity| (entity, layout_inputs(world, entity)))
            .collect();
        let mut cache = world.resource_mut::<UiLayoutCache>();
        for (entity, snapshot) in snapshots {
            cache.input_snapshots.insert(entity, snapshot);
        }
    }

    let issues = {
        let mut issues: Vec<_> = world
            .resource::<UiLayoutCache>()
            .root_issues
            .values()
            .flatten()
            .copied()
            .collect();
        issues.sort_unstable_by_key(|(entity, issue)| (*entity, *issue as u8));
        issues
    };
    if world.resource::<UiLayoutIssues>().0 != issues {
        world.resource_mut::<UiLayoutIssues>().0 = issues;
    }
    let viewports_snapshot = rediscover_roots.then(|| world.resource::<UiViewports>().0.clone());
    {
        let mut cache = world.resource_mut::<UiLayoutCache>();
        cache.root_configs = root_configs;
        if let Some(viewports) = viewports_snapshot {
            cache.viewports = viewports;
        }
        cache.initialized = true;
    }
    world.resource_mut::<UiMeasurementCache>().0 = measurements;
    {
        let mut stats = world.resource_mut::<UiLayoutStats>();
        stats.roots_processed = processed_roots;
        stats.nodes_processed = processed_nodes;
        stats.frames += 1;
        stats.total_roots_processed += processed_roots as u64;
        stats.total_nodes_processed += processed_nodes as u64;
        if processed_roots == 0 {
            stats.idle_frames += 1;
        }
    }
}

fn ensure_layout_resources(world: &mut World) {
    world.init_resource::<UiLayoutIssues>();
    world.init_resource::<UiDirty>();
    world.init_resource::<UiLayoutStats>();
    world.init_resource::<UiLayoutCache>();
    world.init_resource::<UiMeasurementCache>();
    world.init_resource::<UiViewports>();
}

fn effective_roots(world: &mut World) -> Vec<(Entity, UiLayoutRoot)> {
    let registered: Vec<(Entity, UiLayoutRoot)> = {
        let mut query = world.query::<(Entity, &UiLayoutRoot)>();
        query
            .iter(world)
            .map(|(entity, root)| (entity, *root))
            .collect()
    };
    let viewports = world.resource::<UiViewports>().0.clone();
    let mut roots = registered;
    for (root, viewport) in viewports {
        if let Some(existing) = roots.iter_mut().find(|(entity, _)| *entity == root) {
            existing.1.viewport = viewport.rect;
            existing.1.dpi_scale = positive_or_one(viewport.dpi_scale);
        } else if world.get::<UiNode>(root).is_some() {
            roots.push((
                root,
                UiLayoutRoot::new(root, viewport.rect, viewport.dpi_scale),
            ));
        }
    }
    roots.sort_unstable_by_key(|(entity, _)| *entity);
    roots
}

fn collect_layout_changes(world: &mut World) -> Vec<LayoutInputChange> {
    let changed: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, Or<(
            Changed<UiNode>,
            Changed<UiLayoutRoot>,
            Changed<UiLayoutNode>,
            Changed<UiIntrinsicSize>,
            Changed<UiParent>,
            Changed<UiChildren>,
            Changed<UiZIndex>,
            Changed<UiScroll>,
            Changed<UiFlexItem>,
        )>>();
        query.iter(world).collect()
    };
    let mut cache = world.remove_resource::<UiLayoutCache>().unwrap_or_default();
    let mut removed = read_removed::<UiNode>(world, &mut cache.removed_nodes);
    removed.extend(read_removed::<UiLayoutRoot>(
        world,
        &mut cache.removed_roots,
    ));
    removed.extend(read_removed::<UiLayoutNode>(
        world,
        &mut cache.removed_layout_nodes,
    ));
    removed.extend(read_removed::<UiIntrinsicSize>(
        world,
        &mut cache.removed_intrinsics,
    ));
    removed.extend(read_removed::<UiParent>(world, &mut cache.removed_parents));
    removed.extend(read_removed::<UiChildren>(
        world,
        &mut cache.removed_children,
    ));
    removed.extend(read_removed::<UiZIndex>(
        world,
        &mut cache.removed_z_indices,
    ));
    removed.extend(read_removed::<UiScroll>(world, &mut cache.removed_scrolls));
    removed.extend(read_removed::<UiFlexItem>(
        world,
        &mut cache.removed_flex_items,
    ));
    let mut dirty_entities = Vec::new();
    let mut changes = Vec::new();
    for entity in changed.into_iter().chain(removed) {
        if world.get_entity(entity).is_ok() {
            let current = layout_inputs(world, entity);
            let previous = cache.input_snapshots.get(&entity).cloned();
            if previous.as_ref() != Some(&current) {
                cache.input_snapshots.insert(entity, current.clone());
                dirty_entities.push(entity);
                changes.push(LayoutInputChange {
                    entity,
                    previous,
                    current: Some(current),
                });
            }
        } else if let Some(previous) = cache.input_snapshots.remove(&entity) {
            dirty_entities.push(entity);
            changes.push(LayoutInputChange {
                entity,
                previous: Some(previous),
                current: None,
            });
        }
    }
    world.insert_resource(cache);
    let mut dirty = world.resource_mut::<UiDirty>();
    for entity in dirty_entities {
        dirty.mark(UiDirtyDomain::Layout, entity);
    }
    changes
}

fn layout_inputs(world: &World, entity: Entity) -> LayoutInputs {
    LayoutInputs {
        is_node: world.get::<UiNode>(entity).is_some(),
        root: world.get::<UiLayoutRoot>(entity).copied(),
        layout: world.get::<UiLayoutNode>(entity).copied(),
        intrinsic: world.get::<UiIntrinsicSize>(entity).copied(),
        parent: world.get::<UiParent>(entity).copied(),
        children: world.get::<UiChildren>(entity).cloned(),
        z_index: world.get::<UiZIndex>(entity).copied(),
        scroll: world.get::<UiScroll>(entity).copied(),
        flex_item: world.get::<UiFlexItem>(entity).copied(),
    }
}

fn read_removed<C: Component>(
    world: &World,
    reader: &mut RemovedComponentReader<C>,
) -> Vec<Entity> {
    let Some(component_id) = world.component_id::<C>() else {
        return Vec::new();
    };
    let Some(messages) = world.removed_components().get(component_id) else {
        return Vec::new();
    };
    reader
        .read(messages)
        .map(|removed| removed.clone().into())
        .collect()
}

fn collect_entity_roots(
    world: &World,
    entity: Entity,
    current_roots: &HashMap<Entity, UiLayoutRoot>,
    roots: &mut HashSet<Entity>,
) {
    let previous = world
        .resource::<UiLayoutCache>()
        .entity_roots
        .get(&entity)
        .cloned()
        .unwrap_or_default();
    roots.extend(previous);
    let mut at = Some(entity);
    let mut visited = HashSet::new();
    while let Some(current) = at {
        if !visited.insert(current) {
            break;
        }
        if current_roots.contains_key(&current) {
            roots.insert(current);
        }
        at = world.get::<UiParent>(current).map(|parent| parent.0);
    }
}

fn add_path_to_root(world: &World, entity: Entity, root: Entity, required: &mut HashSet<Entity>) {
    let mut at = Some(entity);
    let mut visited = HashSet::new();
    while let Some(current) = at {
        if !visited.insert(current) {
            break;
        }
        required.insert(current);
        if current == root {
            break;
        }
        at = world.get::<UiParent>(current).map(|parent| parent.0);
    }
}

fn invalidate_measurement_path(
    world: &World,
    entity: Entity,
    previous_parent: Option<Entity>,
    measurements: &mut MeasurementMap,
) {
    let mut pending = vec![entity];
    pending.extend(previous_parent);
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        measurements.remove(&current);
        pending.extend(world.get::<UiParent>(current).map(|parent| parent.0));
    }
}

fn is_independent_intrinsic_change(world: &World, change: &LayoutInputChange) -> bool {
    let (Some(previous), Some(current)) = (&change.previous, &change.current) else {
        return false;
    };
    if previous.intrinsic == current.intrinsic {
        return false;
    }
    let mut previous_without_intrinsic = previous.clone();
    previous_without_intrinsic.intrinsic = current.intrinsic;
    if previous_without_intrinsic != *current {
        return false;
    }
    let Some(parent) = current.parent.map(|parent| parent.0) else {
        return false;
    };
    if world.get::<UiNode>(change.entity).is_none()
        || world.get::<UiLayoutResult>(change.entity).is_none()
    {
        return false;
    }
    let child_style = world
        .get::<UiLayoutNode>(change.entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    if child_style.mode != UiLayoutMode::Leaf
        && world
            .get::<UiChildren>(change.entity)
            .is_some_and(|children| !children.0.is_empty())
    {
        return false;
    }
    let old_size = previous.intrinsic.map_or([0.0, 0.0], |size| size.0);
    let new_size = current.intrinsic.map_or([0.0, 0.0], |size| size.0);
    let changed = [old_size[0] != new_size[0], old_size[1] != new_size[1]];
    if !changed.into_iter().any(|axis_changed| axis_changed) {
        return false;
    }
    let parent_style = world
        .get::<UiLayoutNode>(parent)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let size_is_fixed = |axis: usize| {
        matches!(
            axis_length(child_style.size, axis),
            UiLength::Px(_) | UiLength::Percent(_)
        )
    };
    match parent_style.mode {
        UiLayoutMode::Row => changed[0] && !changed[1] && size_is_fixed(0),
        UiLayoutMode::Column => changed[1] && !changed[0] && size_is_fixed(1),
        UiLayoutMode::Flex { direction, .. } => {
            let main_axis = flex_axes(direction).0;
            let cross_axis = flex_axes(direction).1;
            let basis = world
                .get::<UiFlexItem>(change.entity)
                .and_then(|item| item.basis)
                .unwrap_or_else(|| axis_length(child_style.size, main_axis));
            changed[main_axis]
                && !changed[cross_axis]
                && matches!(
                    basis,
                    UiLength::Px(_) | UiLength::Percent(_) | UiLength::Fill(_)
                )
        }
        UiLayoutMode::Overlay | UiLayoutMode::Grid { .. } | UiLayoutMode::ResponsiveGrid { .. } => {
            (0..2).all(|axis| !changed[axis] || size_is_fixed(axis))
        }
        UiLayoutMode::Leaf | UiLayoutMode::Inline => false,
    }
}

struct LayoutPass {
    window: Entity,
    dpi_scale: f32,
    visiting: Vec<Entity>,
    visited: HashSet<Entity>,
    issues: Vec<(Entity, UiLayoutIssue)>,
    clamped_scrolls: HashSet<Entity>,
}

enum AssignedRect {
    Raw(Rect),
    Resolved(Rect),
}

impl LayoutPass {
    fn layout_subtree(
        &mut self,
        world: &mut World,
        entity: Entity,
        assigned: AssignedRect,
        inherited_clip: Option<Rect>,
        required: Option<&HashSet<Entity>>,
        measurements: &mut MeasurementMap,
    ) {
        if self.visiting.contains(&entity) {
            self.issues.push((entity, UiLayoutIssue::Cycle));
            return;
        }
        if world.get::<UiNode>(entity).is_none() {
            self.issues.push((entity, UiLayoutIssue::MissingNode));
            return;
        }

        self.visiting.push(entity);
        self.visited.insert(entity);
        let style = world
            .get::<UiLayoutNode>(entity)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let rect = match assigned {
            AssignedRect::Raw(assigned) => {
                apply_margins_and_constraints(assigned, style.margin, style)
            }
            AssignedRect::Resolved(rect) => rect,
        };
        let scrollable = world.get::<UiScroll>(entity).is_some();
        let own_clip = if style.clip_contents || scrollable {
            Some(inherited_clip.map_or(rect, |parent| {
                parent.intersection(rect).unwrap_or(Rect::ZERO)
            }))
        } else {
            inherited_clip
        };
        let z = style
            .z_index
            .saturating_add(world.get::<UiZIndex>(entity).map_or(0, |z| z.0));
        let children = world
            .get::<bui_core::UiChildren>(entity)
            .map(|children| children.0.clone())
            .unwrap_or_default();
        let content_size = measure_content_with_available(
            world,
            entity,
            style,
            &children,
            [rect.width, rect.height],
            measurements,
        );
        let scroll_offset = if let Some(mut scroll) = world.get_mut::<UiScroll>(entity) {
            if scroll.clamp_to(content_size, [rect.width, rect.height]) {
                self.clamped_scrolls.insert(entity);
            }
            scroll.offset
        } else {
            [0.0, 0.0]
        };
        let result = UiLayoutResult {
            window: self.window,
            rect,
            clip: inherited_clip,
            z,
            content_size,
            dpi_scale: self.dpi_scale,
        };
        let hit_box = UiHitBox {
            window: Some(self.window),
            rect,
            clip: inherited_clip,
            z_index: z,
        };
        if world.get::<UiLayoutResult>(entity).copied() != Some(result) {
            world.entity_mut(entity).insert(result);
        }
        if world.get::<UiHitBox>(entity).copied() != Some(hit_box) {
            world.entity_mut(entity).insert(hit_box);
        }

        if !children.is_empty() && style.mode != UiLayoutMode::Leaf {
            let inner = rect.inset(
                style.padding.left,
                style.padding.top,
                style.padding.right,
                style.padding.bottom,
            );
            let child_rects = distribute_children(world, &children, style, inner, measurements);
            for (child, child_rect) in children.into_iter().zip(child_rects) {
                let child_rect = Rect::new(
                    child_rect.x - scroll_offset[0],
                    child_rect.y - scroll_offset[1],
                    child_rect.width,
                    child_rect.height,
                );
                if required.is_none_or(|required| required.contains(&child))
                    || layout_context_changed(
                        world,
                        child,
                        child_rect,
                        own_clip,
                        self.window,
                        self.dpi_scale,
                    )
                {
                    self.layout_subtree(
                        world,
                        child,
                        AssignedRect::Raw(child_rect),
                        own_clip,
                        required,
                        measurements,
                    );
                }
            }
        }
        self.visiting.pop();
    }
}

fn layout_context_changed(
    world: &World,
    entity: Entity,
    assigned: Rect,
    inherited_clip: Option<Rect>,
    window: Entity,
    dpi_scale: f32,
) -> bool {
    let Some(previous) = world.get::<UiLayoutResult>(entity) else {
        return true;
    };
    let style = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let rect = apply_margins_and_constraints(assigned, style.margin, style);
    previous.rect != rect
        || previous.clip != inherited_clip
        || previous.window != window
        || previous.dpi_scale != dpi_scale
}

impl UiLayoutNode {
    fn sanitized(mut self) -> Self {
        self.padding = self.padding.sanitized();
        self.margin = self.margin.sanitized();
        self.gap = nonnegative(self.gap);
        self.min_size = self
            .min_size
            .map(|value| value.filter(|v| v.is_finite()).map(nonnegative));
        self.max_size = self
            .max_size
            .map(|value| value.filter(|v| v.is_finite()).map(nonnegative));
        self.size.width = sanitize_length(self.size.width);
        self.size.height = sanitize_length(self.size.height);
        self
    }
}

fn measure_content_with_available(
    world: &World,
    entity: Entity,
    style: UiLayoutNode,
    children: &[Entity],
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let key = measure_key(available, MeasureKind::Content);
    if let Some(size) = measurements.get(&entity).and_then(|bucket| bucket.get(key)) {
        return size;
    }
    let size = measure_content_with_available_uncached(
        world,
        entity,
        style,
        children,
        available,
        measurements,
    );
    measurements.entry(entity).or_default().insert(key, size);
    size
}

fn measure_content_with_available_uncached(
    world: &World,
    entity: Entity,
    style: UiLayoutNode,
    children: &[Entity],
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    if let Some(size) = world.get::<UiIntrinsicSize>(entity)
        && (style.mode == UiLayoutMode::Leaf || children.is_empty())
    {
        return [nonnegative(size.0[0]), nonnegative(size.0[1])];
    }
    if children.is_empty() || style.mode == UiLayoutMode::Leaf {
        let intrinsic = world
            .get::<UiIntrinsicSize>(entity)
            .map_or([0.0, 0.0], |size| size.0);
        return [
            nonnegative(intrinsic[0]) + style.padding.left + style.padding.right,
            nonnegative(intrinsic[1]) + style.padding.top + style.padding.bottom,
        ];
    }
    let child_available = [
        (available[0] - style.padding.left - style.padding.right).max(0.0),
        (available[1] - style.padding.top - style.padding.bottom).max(0.0),
    ];
    // These modes compute their own child measurements while deciding line or
    // cell assignments. Building the generic `sizes` vector first measured
    // every child a second time and allocated a vector that was never read.
    match style.mode {
        UiLayoutMode::Flex { direction, wrap } => {
            return measure_flex_content(
                world,
                children,
                style,
                available,
                direction,
                wrap,
                measurements,
            );
        }
        UiLayoutMode::ResponsiveGrid {
            min_cell_width,
            max_columns,
        } => {
            return measure_responsive_grid_content(
                world,
                children,
                style,
                available,
                min_cell_width,
                max_columns,
                measurements,
            );
        }
        _ => {}
    }
    let sizes: Vec<[f32; 2]> = children
        .iter()
        .map(|child| measured_node_with_available(world, *child, child_available, measurements))
        .collect();
    let gap = style.gap * children.len().saturating_sub(1) as f32;
    match style.mode {
        UiLayoutMode::Row => [
            sizes.iter().map(|size| size[0]).sum::<f32>()
                + gap
                + style.padding.left
                + style.padding.right,
            sizes.iter().map(|size| size[1]).fold(0.0, f32::max)
                + style.padding.top
                + style.padding.bottom,
        ],
        UiLayoutMode::Flex { direction, wrap } => measure_flex_content(
            world,
            children,
            style,
            available,
            direction,
            wrap,
            measurements,
        ),
        UiLayoutMode::Grid { columns } => {
            let columns = columns.max(1);
            let rows = children.len().div_ceil(columns);
            let width = sizes.iter().map(|size| size[0]).fold(0.0, f32::max) * columns as f32
                + style.gap * columns.saturating_sub(1) as f32;
            let height = sizes.iter().map(|size| size[1]).fold(0.0, f32::max) * rows as f32
                + style.gap * rows.saturating_sub(1) as f32;
            [
                width + style.padding.left + style.padding.right,
                height + style.padding.top + style.padding.bottom,
            ]
        }
        UiLayoutMode::ResponsiveGrid {
            min_cell_width,
            max_columns,
        } => measure_responsive_grid_content(
            world,
            children,
            style,
            available,
            min_cell_width,
            max_columns,
            measurements,
        ),
        UiLayoutMode::Overlay => [
            sizes.iter().map(|size| size[0]).fold(0.0, f32::max)
                + style.padding.left
                + style.padding.right,
            sizes.iter().map(|size| size[1]).fold(0.0, f32::max)
                + style.padding.top
                + style.padding.bottom,
        ],
        UiLayoutMode::Inline => inline_measure(&sizes, style.gap, f32::INFINITY, style.padding),
        UiLayoutMode::Column | UiLayoutMode::Leaf => [
            sizes.iter().map(|size| size[0]).fold(0.0, f32::max)
                + style.padding.left
                + style.padding.right,
            sizes.iter().map(|size| size[1]).sum::<f32>()
                + gap
                + style.padding.top
                + style.padding.bottom,
        ],
    }
}

fn measured_node_with_available(
    world: &World,
    entity: Entity,
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let key = measure_key(available, MeasureKind::Node);
    if let Some(size) = measurements.get(&entity).and_then(|bucket| bucket.get(key)) {
        return size;
    }
    let size = measured_node_with_available_uncached(world, entity, available, measurements);
    measurements.entry(entity).or_default().insert(key, size);
    size
}

fn measured_node_with_available_uncached(
    world: &World,
    entity: Entity,
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let style = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    if let (Some(width), Some(height)) = (
        resolve_exact_length(style.size.width, available[0]),
        resolve_exact_length(style.size.height, available[1]),
    ) {
        return constrain_size([width, height], style);
    }
    let mut size = if let Some(children) = world.get::<bui_core::UiChildren>(entity) {
        measure_content_with_available(world, entity, style, &children.0, available, measurements)
    } else {
        measure_content_with_available(world, entity, style, &[], available, measurements)
    };
    size[0] = resolve_measured_length(style.size.width, available[0], size[0]);
    size[1] = resolve_measured_length(style.size.height, available[1], size[1]);
    constrain_size(size, style)
}

fn resolve_exact_length(length: UiLength, available: f32) -> Option<f32> {
    match length {
        UiLength::Px(value) if value.is_finite() => Some(nonnegative(value)),
        UiLength::Percent(value) if available.is_finite() => {
            Some(nonnegative(available * nonnegative(value)))
        }
        _ => None,
    }
}

fn resolve_measured_length(length: UiLength, available: f32, measured: f32) -> f32 {
    match length {
        UiLength::Px(value) => nonnegative(value),
        UiLength::Percent(value) if available.is_finite() => available * nonnegative(value),
        _ => measured,
    }
}

fn resolve_flex_basis(length: UiLength, available: f32, measured: f32) -> f32 {
    match length {
        UiLength::Fill(_) => 0.0,
        UiLength::Percent(value) if available.is_finite() => available * nonnegative(value),
        UiLength::Percent(_) | UiLength::Auto => measured,
        UiLength::Px(value) => nonnegative(value),
    }
}

fn responsive_grid_columns_with_gap(
    width: f32,
    min_cell_width: u32,
    max_columns: usize,
    gap: f32,
) -> usize {
    let max_columns = max_columns.max(1);
    if !width.is_finite() {
        return max_columns;
    }
    let minimum = min_cell_width.max(1) as f32;
    (((nonnegative(width) + nonnegative(gap)) / (minimum + nonnegative(gap))).floor() as usize)
        .max(1)
        .min(max_columns)
}

fn measure_flex_content(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    available: [f32; 2],
    direction: UiFlexDirection,
    wrap: bool,
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let (main_axis, cross_axis) = flex_axes(direction);
    let inner_available = [
        (available[0] - style.padding.left - style.padding.right).max(0.0),
        (available[1] - style.padding.top - style.padding.bottom).max(0.0),
    ];
    let main_available = inner_available[main_axis];
    let mut lines: Vec<(f32, f32)> = Vec::new();
    let (mut line_main, mut line_cross, mut line_items) = (0.0_f32, 0.0_f32, 0usize);
    for child in children {
        let child_style = world
            .get::<UiLayoutNode>(*child)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let measured = measured_node_with_available(world, *child, inner_available, measurements);
        let margins = child_style.margin;
        let item = world.get::<UiFlexItem>(*child).copied();
        let main_length = item
            .and_then(|item| item.basis)
            .unwrap_or(axis_length(child_style.size, main_axis));
        let main_size = resolve_flex_basis(main_length, main_available, measured[main_axis]);
        let outer_main = main_size + axis_margin_sum(margins, main_axis);
        let outer_cross = measured[cross_axis] + axis_margin_sum(margins, cross_axis);
        let next_main = line_main + if line_items == 0 { 0.0 } else { style.gap } + outer_main;
        if wrap && main_available.is_finite() && line_items > 0 && next_main > main_available {
            lines.push((line_main, line_cross));
            line_main = outer_main;
            line_cross = outer_cross;
            line_items = 1;
        } else {
            line_main = next_main;
            line_cross = line_cross.max(outer_cross);
            line_items += 1;
        }
    }
    if line_items > 0 {
        lines.push((line_main, line_cross));
    }
    let natural_main = if wrap {
        lines.iter().map(|line| line.0).fold(0.0, f32::max)
    } else {
        lines.first().map_or(0.0, |line| line.0)
    };
    let natural_cross = if wrap {
        lines.iter().map(|line| line.1).sum::<f32>()
            + style.gap * lines.len().saturating_sub(1) as f32
    } else {
        lines.first().map_or(0.0, |line| line.1)
    };
    let mut result = [0.0; 2];
    result[main_axis] = natural_main;
    result[cross_axis] = natural_cross;
    result[0] += style.padding.left + style.padding.right;
    result[1] += style.padding.top + style.padding.bottom;
    result
}

fn measure_responsive_grid_content(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    available: [f32; 2],
    min_cell_width: u32,
    max_columns: usize,
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let max_columns = max_columns.max(1).min(children.len().max(1));
    let inner_width = if available[0].is_finite() {
        (available[0] - style.padding.left - style.padding.right).max(0.0)
    } else {
        f32::INFINITY
    };
    let columns =
        responsive_grid_columns_with_gap(inner_width, min_cell_width, max_columns, style.gap)
            .min(children.len().max(1));
    let rows = children.len().div_ceil(columns);
    let cell_width = if inner_width.is_finite() {
        (inner_width - style.gap * columns.saturating_sub(1) as f32).max(0.0) / columns as f32
    } else {
        let intrinsic_width = children
            .iter()
            .map(|child| {
                measured_node_with_available(
                    world,
                    *child,
                    [f32::INFINITY, available[1]],
                    measurements,
                )[0]
            })
            .fold(0.0, f32::max);
        (min_cell_width.max(1) as f32).max(intrinsic_width)
    };
    let mut row_heights = vec![0.0_f32; rows];
    for (index, child) in children.iter().enumerate() {
        let inner_height = if available[1].is_finite() {
            (available[1] - style.padding.top - style.padding.bottom).max(0.0)
        } else {
            available[1]
        };
        let measured =
            measured_node_with_available(world, *child, [cell_width, inner_height], measurements);
        let margin = world
            .get::<UiLayoutNode>(*child)
            .map_or(UiEdges::ZERO, |node| node.margin.sanitized());
        row_heights[index / columns] =
            row_heights[index / columns].max(measured[1] + margin.top + margin.bottom);
    }
    let row_gap = style.gap * rows.saturating_sub(1) as f32;
    let natural_width = cell_width * columns as f32
        + style.gap * columns.saturating_sub(1) as f32
        + style.padding.left
        + style.padding.right;
    [
        if available[0].is_finite() {
            available[0]
        } else {
            natural_width
        },
        row_heights.iter().sum::<f32>() + row_gap + style.padding.top + style.padding.bottom,
    ]
}

fn distribute_children(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    if children.is_empty() {
        return Vec::new();
    }
    match style.mode {
        UiLayoutMode::Row | UiLayoutMode::Column => {
            distribute_linear(world, children, style, inner, measurements)
        }
        UiLayoutMode::Flex { direction, wrap } => {
            distribute_flex(world, children, style, inner, direction, wrap, measurements)
        }
        UiLayoutMode::Overlay => children
            .iter()
            .map(|child| align_child(world, *child, style, inner, measurements))
            .collect(),
        UiLayoutMode::Grid { columns } => {
            distribute_grid(world, children, style, inner, columns.max(1), measurements)
        }
        UiLayoutMode::ResponsiveGrid {
            min_cell_width,
            max_columns,
        } => distribute_responsive_grid(
            world,
            children,
            style,
            inner,
            min_cell_width,
            max_columns,
            measurements,
        ),
        UiLayoutMode::Inline => distribute_inline(world, children, style, inner, measurements),
        UiLayoutMode::Leaf => Vec::new(),
    }
}

#[derive(Clone, Copy)]
struct FlexItemLayout {
    style: UiLayoutNode,
    margins: UiEdges,
    measured: [f32; 2],
    main_axis: usize,
    min_outer_main: f32,
    max_outer_main: f32,
    allocated_outer_main: f32,
    grow: f32,
    shrink: f32,
    align_self: Option<UiAlign>,
}

fn distribute_flex(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    direction: UiFlexDirection,
    wrap: bool,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let (main_axis, cross_axis) = flex_axes(direction);
    let main_extent = axis_extent(inner, main_axis);
    let available = [inner.width, inner.height];
    let mut items = Vec::with_capacity(children.len());
    for entity in children {
        let child_style = world
            .get::<UiLayoutNode>(*entity)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let item = world.get::<UiFlexItem>(*entity).copied();
        let margins = child_style.margin;
        let measured = measured_node_with_available(world, *entity, available, measurements);
        let main_length = item
            .and_then(|value| value.basis)
            .unwrap_or(axis_length(child_style.size, main_axis));
        let (basis, fill_weight) = resolve_length(main_length, main_extent, measured[main_axis]);
        let basis = constrain_dimension(
            basis.max(child_style.min_size[main_axis].unwrap_or(0.0)),
            child_style.min_size[main_axis],
            child_style.max_size[main_axis],
        );
        let margin_main = axis_margin_sum(margins, main_axis);
        let min_outer_main = child_style.min_size[main_axis].unwrap_or(0.0) + margin_main;
        let max_outer_main = child_style.max_size[main_axis].unwrap_or(f32::INFINITY) + margin_main;
        items.push(FlexItemLayout {
            style: child_style,
            margins,
            measured,
            main_axis,
            min_outer_main,
            max_outer_main,
            allocated_outer_main: basis + margin_main,
            grow: item.map_or(fill_weight, |value| nonnegative(value.grow)),
            shrink: item.map_or(1.0, |value| nonnegative(value.shrink)),
            align_self: item.and_then(|value| value.align_self),
        });
    }

    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current = Vec::new();
    let mut used = 0.0;
    for (index, item) in items.iter().enumerate() {
        let gap = if current.is_empty() { 0.0 } else { style.gap };
        let next = used + gap + item.allocated_outer_main;
        if wrap && main_extent.is_finite() && !current.is_empty() && next > main_extent {
            lines.push(std::mem::take(&mut current));
            current.push(index);
            used = item.allocated_outer_main;
        } else {
            current.push(index);
            used = next;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }

    let mut line_cross_sizes = Vec::with_capacity(lines.len());
    for line in &lines {
        let gap_total = style.gap * line.len().saturating_sub(1) as f32;
        distribute_flex_line(&mut items, line, (main_extent - gap_total).max(0.0));
        line_cross_sizes.push(
            line.iter()
                .map(|index| {
                    let item = items[*index];
                    item.measured[cross_axis] + axis_margin_sum(item.margins, cross_axis)
                })
                .fold(0.0, f32::max),
        );
    }

    let mut output = vec![Rect::ZERO; children.len()];
    let mut cross_cursor = axis_origin(inner, cross_axis);
    for (line_index, line) in lines.iter().enumerate() {
        let line_cross = line_cross_sizes[line_index];
        let line_gap_total = style.gap * line.len().saturating_sub(1) as f32;
        let line_used = line
            .iter()
            .map(|index| items[*index].allocated_outer_main)
            .sum::<f32>()
            + line_gap_total;
        let free_main = (main_extent - line_used).max(0.0);
        let (main_offset, extra_gap) =
            justify_offsets(style.justify_content, free_main, line.len());
        let mut main_cursor = axis_origin(inner, main_axis) + main_offset;
        for index in line {
            let item = items[*index];
            let margin_cross = axis_margin_sum(item.margins, cross_axis);
            let available_cross = (line_cross - margin_cross).max(0.0);
            let cross_length = axis_length(item.style.size, cross_axis);
            let mut cross_size =
                resolve_length(cross_length, available_cross, item.measured[cross_axis]).0;
            let align = item.align_self.unwrap_or(style.align_items);
            if align == UiAlign::Stretch
                && matches!(cross_length, UiLength::Auto | UiLength::Fill(_))
            {
                cross_size = available_cross;
            }
            cross_size = constrain_dimension(
                cross_size,
                item.style.min_size[cross_axis],
                item.style.max_size[cross_axis],
            );
            let outer_cross = cross_size + margin_cross;
            let cross_free = line_cross - outer_cross;
            let cross_offset = match align {
                UiAlign::Start | UiAlign::Stretch => 0.0,
                UiAlign::Center => cross_free * 0.5,
                UiAlign::End => cross_free,
            };
            let outer_main = item.allocated_outer_main;
            let rect = if main_axis == 0 {
                Rect::new(
                    main_cursor,
                    cross_cursor + cross_offset,
                    outer_main,
                    outer_cross,
                )
            } else {
                Rect::new(
                    cross_cursor + cross_offset,
                    main_cursor,
                    outer_cross,
                    outer_main,
                )
            };
            output[*index] = rect;
            main_cursor += outer_main + style.gap + extra_gap;
        }
        cross_cursor += line_cross + style.gap;
    }
    output
}

fn distribute_flex_line(items: &mut [FlexItemLayout], line: &[usize], available: f32) {
    let used = line
        .iter()
        .map(|index| items[*index].allocated_outer_main)
        .sum::<f32>();
    if used < available {
        let mut remaining = available - used;
        for _ in 0..line.len() {
            if remaining <= 0.001 {
                break;
            }
            let weight: f32 = line
                .iter()
                .map(|index| {
                    let item = items[*index];
                    if item.allocated_outer_main < item.max_outer_main {
                        item.grow
                    } else {
                        0.0
                    }
                })
                .sum();
            if weight <= 0.0 {
                break;
            }
            let mut consumed = 0.0;
            for index in line {
                let item = &mut items[*index];
                if item.grow <= 0.0 || item.allocated_outer_main >= item.max_outer_main {
                    continue;
                }
                let share = remaining * item.grow / weight;
                let delta = share.min(item.max_outer_main - item.allocated_outer_main);
                item.allocated_outer_main += delta;
                consumed += delta;
            }
            if consumed <= 0.001 {
                break;
            }
            remaining -= consumed;
        }
    } else if used > available {
        let mut remaining = used - available;
        for _ in 0..line.len() {
            if remaining <= 0.001 {
                break;
            }
            let weight: f32 = line
                .iter()
                .map(|index| {
                    let item = items[*index];
                    if item.allocated_outer_main > item.min_outer_main {
                        item.shrink
                            * (item.allocated_outer_main
                                - axis_margin_sum(item.margins, item.main_axis))
                            .max(1.0)
                    } else {
                        0.0
                    }
                })
                .sum();
            if weight <= 0.0 {
                break;
            }
            let mut consumed = 0.0;
            for index in line {
                let item = &mut items[*index];
                if item.shrink <= 0.0 || item.allocated_outer_main <= item.min_outer_main {
                    continue;
                }
                let basis = (item.allocated_outer_main
                    - axis_margin_sum(item.margins, item.main_axis))
                .max(1.0);
                let share = remaining * item.shrink * basis / weight;
                let delta = share.min(item.allocated_outer_main - item.min_outer_main);
                item.allocated_outer_main -= delta;
                consumed += delta;
            }
            if consumed <= 0.001 {
                break;
            }
            remaining -= consumed;
        }
    }
}

fn distribute_responsive_grid(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    min_cell_width: u32,
    max_columns: usize,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let columns =
        responsive_grid_columns_with_gap(inner.width, min_cell_width, max_columns, style.gap)
            .min(children.len().max(1));
    let rows = children.len().div_ceil(columns);
    let cell_width =
        (inner.width - style.gap * columns.saturating_sub(1) as f32).max(0.0) / columns as f32;
    let mut row_heights = vec![0.0_f32; rows];
    for (index, child) in children.iter().enumerate() {
        let measured =
            measured_node_with_available(world, *child, [cell_width, inner.height], measurements);
        let margin = world
            .get::<UiLayoutNode>(*child)
            .map_or(UiEdges::ZERO, |node| node.margin.sanitized());
        row_heights[index / columns] =
            row_heights[index / columns].max(measured[1] + margin.top + margin.bottom);
    }
    let row_gap_total = style.gap * rows.saturating_sub(1) as f32;
    let used_height = row_heights.iter().sum::<f32>() + row_gap_total;
    let free_height = (inner.height - used_height).max(0.0);
    let (start_y, extra_gap) = justify_offsets(style.justify_content, free_height, rows);
    let mut row_y = inner.y + start_y;
    let mut result = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        let row = index / columns;
        let col = index % columns;
        let cell = Rect::new(
            inner.x + col as f32 * (cell_width + style.gap),
            row_y,
            cell_width,
            row_heights[row],
        );
        result.push(align_child(world, *child, style, cell, measurements));
        if col + 1 == columns || index + 1 == children.len() {
            row_y += row_heights[row] + style.gap + extra_gap;
        }
    }
    result
}

fn flex_axes(direction: UiFlexDirection) -> (usize, usize) {
    match direction {
        UiFlexDirection::Row => (0, 1),
        UiFlexDirection::Column => (1, 0),
    }
}

fn axis_length(size: UiSize, axis: usize) -> UiLength {
    if axis == 0 { size.width } else { size.height }
}

fn axis_extent(rect: Rect, axis: usize) -> f32 {
    if axis == 0 { rect.width } else { rect.height }
}

fn axis_origin(rect: Rect, axis: usize) -> f32 {
    if axis == 0 { rect.x } else { rect.y }
}

fn axis_margin_sum(margins: UiEdges, axis: usize) -> f32 {
    if axis == 0 {
        margins.left + margins.right
    } else {
        margins.top + margins.bottom
    }
}

fn justify_offsets(justify: UiJustify, free: f32, count: usize) -> (f32, f32) {
    match justify {
        UiJustify::Start => (0.0, 0.0),
        UiJustify::Center => (free * 0.5, 0.0),
        UiJustify::End => (free, 0.0),
        UiJustify::SpaceBetween if count > 1 => (0.0, free / (count - 1) as f32),
        UiJustify::SpaceBetween => (0.0, 0.0),
    }
}

fn distribute_linear(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let horizontal = style.mode == UiLayoutMode::Row;
    let main_extent = if horizontal {
        inner.width
    } else {
        inner.height
    };
    let cross_extent = if horizontal {
        inner.height
    } else {
        inner.width
    };
    let gaps = style.gap * children.len().saturating_sub(1) as f32;
    let mut main_sizes = Vec::with_capacity(children.len());
    let mut fill_weight = 0.0;
    for child in children {
        let child_style = world
            .get::<UiLayoutNode>(*child)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let length = if horizontal {
            child_style.size.width
        } else {
            child_style.size.height
        };
        let measured =
            measured_node_with_available(world, *child, [inner.width, inner.height], measurements);
        let basis = if horizontal { measured[0] } else { measured[1] };
        let (value, weight) = resolve_length(length, main_extent, basis);
        main_sizes.push(value);
        fill_weight += weight;
    }
    let fixed_total: f32 = main_sizes.iter().sum::<f32>() + gaps;
    let remaining = (main_extent - fixed_total).max(0.0);
    if fill_weight > 0.0 {
        for (index, child) in children.iter().enumerate() {
            let child_style = world
                .get::<UiLayoutNode>(*child)
                .copied()
                .unwrap_or_default();
            let length = if horizontal {
                child_style.size.width
            } else {
                child_style.size.height
            };
            if let UiLength::Fill(weight) = length {
                main_sizes[index] += remaining * nonnegative(weight) / fill_weight;
            }
        }
    }
    let used = main_sizes.iter().sum::<f32>() + gaps;
    let free = (main_extent - used).max(0.0);
    let (start_offset, extra_gap) = match style.justify_content {
        UiJustify::Start => (0.0, 0.0),
        UiJustify::Center => (free * 0.5, 0.0),
        UiJustify::End => (free, 0.0),
        UiJustify::SpaceBetween if children.len() > 1 => (0.0, free / (children.len() - 1) as f32),
        UiJustify::SpaceBetween => (0.0, 0.0),
    };
    let mut cursor = if horizontal { inner.x } else { inner.y };
    cursor += start_offset;
    let mut output = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        let child_style = world
            .get::<UiLayoutNode>(*child)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let measured =
            measured_node_with_available(world, *child, [inner.width, inner.height], measurements);
        let main = main_sizes[index];
        let cross_length = if horizontal {
            child_style.size.height
        } else {
            child_style.size.width
        };
        let cross_basis = if horizontal { measured[1] } else { measured[0] };
        let mut cross = resolve_length(cross_length, cross_extent, cross_basis).0;
        if style.align_items == UiAlign::Stretch
            && matches!(cross_length, UiLength::Auto | UiLength::Fill(_))
        {
            cross = cross_extent;
        }
        cross = cross.min(cross_extent).max(0.0);
        let cross_offset = match style.align_items {
            UiAlign::Start | UiAlign::Stretch => 0.0,
            UiAlign::Center => (cross_extent - cross) * 0.5,
            UiAlign::End => cross_extent - cross,
        };
        let rect = if horizontal {
            Rect::new(cursor, inner.y + cross_offset, main, cross)
        } else {
            Rect::new(inner.x + cross_offset, cursor, cross, main)
        };
        output.push(rect);
        cursor += main + style.gap + extra_gap;
    }
    output
}

fn distribute_grid(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    columns: usize,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let rows = children.len().div_ceil(columns);
    let col_gap = style.gap;
    let row_gap = style.gap;
    let cell_w =
        ((inner.width - col_gap * columns.saturating_sub(1) as f32).max(0.0)) / columns as f32;
    let row_height =
        (inner.height - row_gap * rows.saturating_sub(1) as f32).max(0.0) / rows as f32;
    children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let col = index % columns;
            let row = index / columns;
            let cell = Rect::new(
                inner.x + col as f32 * (cell_w + col_gap),
                inner.y + row as f32 * (row_height + row_gap),
                cell_w,
                row_height,
            );
            align_child(world, *child, style, cell, measurements)
        })
        .collect()
}

fn distribute_inline(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let mut output = Vec::with_capacity(children.len());
    let mut x = inner.x;
    let mut y = inner.y;
    let mut line_height: f32 = 0.0;
    for child in children {
        let size =
            measured_node_with_available(world, *child, [inner.width, inner.height], measurements);
        let margin = world
            .get::<UiLayoutNode>(*child)
            .map_or(UiEdges::ZERO, |node| node.margin.sanitized());
        let width = (size[0] + margin.left + margin.right).min(inner.width);
        let height = size[1] + margin.top + margin.bottom;
        if x > inner.x && x + width > inner.right() {
            x = inner.x;
            y += line_height + style.gap;
            line_height = 0.0;
        }
        output.push(Rect::new(
            x + margin.left,
            y + margin.top,
            (width - margin.left - margin.right).max(0.0),
            size[1],
        ));
        x += width + style.gap;
        line_height = line_height.max(height);
    }
    output
}

fn align_child(
    world: &World,
    entity: Entity,
    parent: UiLayoutNode,
    slot: Rect,
    measurements: &mut MeasurementMap,
) -> Rect {
    let style = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let measured =
        measured_node_with_available(world, entity, [slot.width, slot.height], measurements);
    let width = resolve_length(style.size.width, slot.width, measured[0]).0;
    let height = resolve_length(style.size.height, slot.height, measured[1]).0;
    let align_x = match parent.align_items {
        UiAlign::Start | UiAlign::Stretch => 0.0,
        UiAlign::Center => (slot.width - width) * 0.5,
        UiAlign::End => slot.width - width,
    };
    let align_y = match parent.align_items {
        UiAlign::Start | UiAlign::Stretch => 0.0,
        UiAlign::Center => (slot.height - height) * 0.5,
        UiAlign::End => slot.height - height,
    };
    let width = if parent.align_items == UiAlign::Stretch
        && matches!(style.size.width, UiLength::Auto | UiLength::Fill(_))
    {
        slot.width
    } else {
        width
    };
    let height = if parent.align_items == UiAlign::Stretch
        && matches!(style.size.height, UiLength::Auto | UiLength::Fill(_))
    {
        slot.height
    } else {
        height
    };
    Rect::new(
        slot.x + align_x,
        slot.y + align_y,
        width.max(0.0),
        height.max(0.0),
    )
}

fn inline_measure(sizes: &[[f32; 2]], gap: f32, max_width: f32, padding: UiEdges) -> [f32; 2] {
    [
        sizes.iter().map(|size| size[0]).sum::<f32>()
            + gap * sizes.len().saturating_sub(1) as f32
            + padding.left
            + padding.right,
        sizes.iter().map(|size| size[1]).fold(0.0, f32::max)
            + padding.top
            + padding.bottom
            + max_width.min(0.0),
    ]
}

fn apply_margins_and_constraints(rect: Rect, margin: UiEdges, style: UiLayoutNode) -> Rect {
    let rect = Rect::new(
        rect.x + margin.left,
        rect.y + margin.top,
        (rect.width - margin.left - margin.right).max(0.0),
        (rect.height - margin.top - margin.bottom).max(0.0),
    );
    let size = constrain_size([rect.width, rect.height], style);
    Rect::new(rect.x, rect.y, size[0], size[1])
}

fn constrain_size(size: [f32; 2], style: UiLayoutNode) -> [f32; 2] {
    [
        constrain_dimension(size[0], style.min_size[0], style.max_size[0]),
        constrain_dimension(size[1], style.min_size[1], style.max_size[1]),
    ]
}

fn constrain_dimension(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut value = nonnegative(value);
    if let Some(min) = min {
        value = value.max(min);
    }
    if let Some(max) = max {
        value = value.min(max);
    }
    value
}

fn resolve_length(length: UiLength, available: f32, measured: f32) -> (f32, f32) {
    match length {
        UiLength::Auto => (measured, 0.0),
        UiLength::Px(value) => (nonnegative(value), 0.0),
        UiLength::Percent(value) => (available * nonnegative(value), 0.0),
        UiLength::Fill(weight) => (0.0, nonnegative(weight)),
    }
}

fn sanitize_length(length: UiLength) -> UiLength {
    match length {
        UiLength::Auto => UiLength::Auto,
        UiLength::Px(value) => UiLength::Px(nonnegative(value)),
        UiLength::Percent(value) => UiLength::Percent(nonnegative(value)),
        UiLength::Fill(value) => UiLength::Fill(nonnegative(value)),
    }
}

fn nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn positive_or_one(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bui_core::{
        CorePlugin, UiFocus, UiFocusable, UiInputEvent, UiInputQueue, UiPointerButton,
        UiRoutedEvent, UiRoutedEvents, UiSystems, UiVisible, set_parent,
    };

    #[derive(Resource, Default)]
    struct ScrollPhaseObservation {
        target: Option<Entity>,
        text_rect: Option<Rect>,
        widget_hit_box: Option<UiHitBox>,
    }

    fn observe_text_phase(
        layouts: Query<&UiLayoutResult>,
        mut observation: ResMut<ScrollPhaseObservation>,
    ) {
        if let Some(target) = observation.target
            && let Ok(layout) = layouts.get(target)
        {
            observation.text_rect = Some(layout.rect);
        }
    }

    fn observe_widget_phase(
        hit_boxes: Query<&UiHitBox>,
        mut observation: ResMut<ScrollPhaseObservation>,
    ) {
        if let Some(target) = observation.target
            && let Ok(hit_box) = hit_boxes.get(target)
        {
            observation.widget_hit_box = Some(*hit_box);
        }
    }

    fn root(world: &mut World, viewport: Rect) -> (Entity, Entity) {
        let window = world.spawn_empty().id();
        let root = world
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, viewport, 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Row,
                    padding: UiEdges::all(10.0),
                    gap: 10.0,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        (window, root)
    }

    #[test]
    fn row_distributes_fill_children_inside_padding_and_gap() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 300.0, 100.0));
        let fixed = world
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(50.0), UiLength::Fill(1.0)),
            ))
            .id();
        let fill = world
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Fill(1.0), UiLength::Fill(1.0)),
            ))
            .id();
        set_parent(&mut world, fixed, parent).unwrap();
        set_parent(&mut world, fill, parent).unwrap();
        compute_layout(&mut world);
        assert_eq!(
            world.get::<UiLayoutResult>(fixed).unwrap().rect,
            Rect::new(10.0, 10.0, 50.0, 80.0)
        );
        assert_eq!(
            world.get::<UiLayoutResult>(fill).unwrap().rect,
            Rect::new(70.0, 10.0, 220.0, 80.0)
        );
        assert_eq!(
            world.get::<UiHitBox>(fill).unwrap().rect,
            world.get::<UiLayoutResult>(fill).unwrap().rect
        );
    }

    #[test]
    fn nested_clipping_is_shared_with_hit_testing_and_dpi_is_reported() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let parent = world
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(5.0, 7.0, 80.0, 50.0), 2.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    clip_contents: true,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let child = world
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(120.0), UiLength::Px(40.0)),
            ))
            .id();
        set_parent(&mut world, child, parent).unwrap();
        compute_layout(&mut world);
        let result = world.get::<UiLayoutResult>(child).unwrap();
        assert_eq!(result.rect, Rect::new(5.0, 7.0, 120.0, 40.0));
        assert_eq!(result.clip, Some(Rect::new(5.0, 7.0, 80.0, 50.0)));
        assert_eq!(result.dpi_scale, 2.0);
        assert_eq!(world.get::<UiHitBox>(child).unwrap().clip, result.clip);
    }

    #[test]
    fn scroll_offsets_clamp_to_content_and_viewport_resize_without_idle_relayout() {
        let mut app = App::new();
        app.add_plugins(LayoutPlugin);
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let scroll = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::at(0.0, 500.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Percent(0.5),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let children: Vec<_> = (0..2)
            .map(|_| {
                app.world_mut()
                    .spawn((
                        UiNode,
                        UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(40.0)),
                    ))
                    .id()
            })
            .collect();
        set_parent(app.world_mut(), scroll, root).unwrap();
        for child in &children {
            set_parent(app.world_mut(), *child, scroll).unwrap();
        }

        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 30.0]
        );
        assert_eq!(
            app.world().get::<UiLayoutResult>(children[0]).unwrap().rect,
            Rect::new(0.0, -30.0, 80.0, 40.0)
        );
        let viewport_clip = Some(Rect::new(0.0, 0.0, 80.0, 50.0));
        assert_eq!(
            app.world().get::<UiLayoutResult>(children[0]).unwrap().clip,
            viewport_clip
        );
        assert_eq!(
            app.world().get::<UiHitBox>(children[0]).unwrap().clip,
            viewport_clip
        );

        app.update();
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 0);

        app.world_mut()
            .get_mut::<UiLayoutRoot>(root)
            .unwrap()
            .viewport = Rect::new(0.0, 0.0, 100.0, 120.0);
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 20.0]
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[0])
                .unwrap()
                .rect
                .y,
            -20.0
        );
        assert_eq!(
            app.world().get::<UiLayoutResult>(children[0]).unwrap().clip,
            Some(Rect::new(0.0, 0.0, 80.0, 60.0))
        );
        app.update();
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 0);

        app.world_mut()
            .get_mut::<UiScroll>(scroll)
            .unwrap()
            .set_offset(-5.0, f32::NAN);
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 0.0]
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[0])
                .unwrap()
                .rect
                .y,
            0.0
        );
    }

    #[test]
    fn nested_scroll_clips_translate_descendant_hitboxes_together() {
        let mut app = App::new();
        app.add_plugins(LayoutPlugin);
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 100.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let outer = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::new(),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(70.0),
                        height: UiLength::Px(40.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let inner = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::at(0.0, 15.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(60.0),
                        height: UiLength::Px(30.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let content = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(60.0), UiLength::Px(60.0)),
            ))
            .id();
        set_parent(app.world_mut(), outer, root).unwrap();
        set_parent(app.world_mut(), inner, outer).unwrap();
        set_parent(app.world_mut(), content, inner).unwrap();

        app.update();
        let inner_rect = app.world().get::<UiLayoutResult>(inner).unwrap().rect;
        let content_layout = app.world().get::<UiLayoutResult>(content).unwrap();
        let content_hit = app.world().get::<UiHitBox>(content).unwrap();
        assert_eq!(inner_rect, Rect::new(0.0, 0.0, 60.0, 30.0));
        assert_eq!(content_layout.rect, Rect::new(0.0, -15.0, 60.0, 60.0));
        assert_eq!(content_layout.clip, Some(inner_rect));
        assert_eq!(content_hit.rect, content_layout.rect);
        assert_eq!(content_hit.clip, content_layout.clip);
        assert!(content_hit.contains([5.0, 5.0]));
        assert!(!content_hit.contains([5.0, 35.0]));
    }

    #[test]
    fn pointer_routing_hits_scrolled_descendants_and_rejects_clipped_regions() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let scroll = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::at(0.0, 40.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(30.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let spacer = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(40.0)),
            ))
            .id();
        let visible_button = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            ))
            .id();
        let clipped_button = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            ))
            .id();
        set_parent(app.world_mut(), scroll, root).unwrap();
        set_parent(app.world_mut(), spacer, scroll).unwrap();
        set_parent(app.world_mut(), visible_button, scroll).unwrap();
        set_parent(app.world_mut(), clipped_button, scroll).unwrap();
        app.update();

        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(visible_button)
                .unwrap()
                .rect,
            Rect::new(0.0, 0.0, 80.0, 20.0)
        );
        let clipped_bounds = app.world().get::<UiHitBox>(clipped_button).unwrap();
        assert!(clipped_bounds.rect.contains(10.0, 35.0));
        assert!(!clipped_bounds.contains([10.0, 35.0]));

        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [10.0, 10.0],
                time_seconds: 0.0,
            },
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position: [10.0, 10.0],
                time_seconds: 0.1,
            },
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [10.0, 35.0],
                time_seconds: 0.2,
            },
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position: [10.0, 35.0],
                time_seconds: 0.3,
            },
        ]);
        app.update();
        let clicks: Vec<_> = app
            .world()
            .resource::<UiRoutedEvents>()
            .events
            .iter()
            .filter_map(|event| match event {
                UiRoutedEvent::Click { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        assert_eq!(clicks.first(), Some(&visible_button));
        assert_eq!(clicks.len(), 2);
        assert!(!clicks.contains(&clipped_button));
    }

    #[test]
    fn routed_wheel_and_keyboard_navigation_update_layout_in_the_same_frame() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin))
            .init_resource::<ScrollPhaseObservation>()
            .add_systems(Update, observe_text_phase.in_set(UiSystems::Text))
            .add_systems(Update, observe_widget_phase.in_set(UiSystems::Widgets));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let scroll = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiScroll::new(),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(40.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let children: Vec<_> = (0..6)
            .map(|index| {
                let mut entity = app.world_mut().spawn((
                    UiNode,
                    UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
                ));
                if index == 1 {
                    entity.insert(UiFocusable);
                }
                entity.id()
            })
            .collect();
        set_parent(app.world_mut(), scroll, root).unwrap();
        for child in &children {
            set_parent(app.world_mut(), *child, scroll).unwrap();
        }
        app.world_mut()
            .resource_mut::<ScrollPhaseObservation>()
            .target = Some(children[0]);
        app.update();

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Wheel {
                window,
                position: [10.0, 10.0],
                delta: [0.0, -1.0],
                unit: UiWheelUnit::Lines,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 40.0]
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[0])
                .unwrap()
                .rect
                .y,
            -40.0
        );
        assert_eq!(
            app.world().get::<UiHitBox>(children[0]).unwrap().clip,
            Some(Rect::new(0.0, 0.0, 80.0, 40.0))
        );
        let same_frame = app.world().resource::<ScrollPhaseObservation>();
        assert_eq!(same_frame.text_rect.unwrap().y, -40.0);
        assert_eq!(same_frame.widget_hit_box.unwrap().rect.y, -40.0);

        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(children[1]);
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::PageDown,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 80.0]
        );
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::PageUp,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 40.0]
        );

        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(scroll);
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::End,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 80.0]
        );
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Home,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 0.0]
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[0])
                .unwrap()
                .rect
                .y,
            0.0
        );
    }

    #[test]
    fn wheel_delta_propagates_from_inner_scroll_at_its_boundary_to_parent() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 120.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let outer = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::new(),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(100.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let inner = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::at(0.0, 40.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(50.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let inner_children: Vec<_> = (0..2)
            .map(|_| {
                app.world_mut()
                    .spawn((
                        UiNode,
                        UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(50.0)),
                    ))
                    .id()
            })
            .collect();
        let outer_filler = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(100.0)),
            ))
            .id();
        set_parent(app.world_mut(), outer, root).unwrap();
        set_parent(app.world_mut(), inner, outer).unwrap();
        for child in &inner_children {
            set_parent(app.world_mut(), *child, inner).unwrap();
        }
        set_parent(app.world_mut(), outer_filler, outer).unwrap();
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(inner).unwrap().offset,
            [0.0, 40.0]
        );
        assert_eq!(
            app.world().get::<UiScroll>(outer).unwrap().offset,
            [0.0, 0.0]
        );

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Wheel {
                window,
                position: [10.0, 5.0],
                delta: [0.0, -1.0],
                unit: UiWheelUnit::Lines,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(inner).unwrap().offset,
            [0.0, 50.0]
        );
        assert_eq!(
            app.world().get::<UiScroll>(outer).unwrap().offset,
            [0.0, 30.0]
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(inner_children[1])
                .unwrap()
                .rect
                .y,
            -30.0
        );

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Wheel {
                window,
                position: [10.0, 5.0],
                delta: [0.0, -1.0],
                unit: UiWheelUnit::Lines,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(inner).unwrap().offset,
            [0.0, 50.0]
        );
        assert_eq!(
            app.world().get::<UiScroll>(outer).unwrap().offset,
            [0.0, 50.0]
        );
    }

    #[test]
    fn widget_handled_wheel_does_not_scroll_its_ancestor() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let scroll = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::new(),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(30.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiWheelHandled,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(80.0)),
            ))
            .id();
        set_parent(app.world_mut(), scroll, root).unwrap();
        set_parent(app.world_mut(), child, scroll).unwrap();
        app.update();
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Wheel {
                window,
                position: [10.0, 10.0],
                delta: [0.0, -1.0],
                unit: UiWheelUnit::Lines,
                modifiers: Default::default(),
            });
        app.update();
        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 0.0]
        );
    }

    #[test]
    fn grid_and_inline_layout_produce_bounded_children() {
        let mut world = World::new();
        let (_, grid) = root(&mut world, Rect::new(0.0, 0.0, 120.0, 90.0));
        world.entity_mut(grid).insert(UiLayoutNode {
            mode: UiLayoutMode::Grid { columns: 2 },
            padding: UiEdges::ZERO,
            gap: 4.0,
            ..UiLayoutNode::default()
        });
        let cells: Vec<Entity> = (0..3)
            .map(|_| world.spawn((UiNode, UiIntrinsicSize([20.0, 15.0]))).id())
            .collect();
        for child in &cells {
            set_parent(&mut world, *child, grid).unwrap();
        }
        compute_layout(&mut world);
        let first = world.get::<UiLayoutResult>(cells[0]).unwrap().rect;
        let second = world.get::<UiLayoutResult>(cells[1]).unwrap().rect;
        let third = world.get::<UiLayoutResult>(cells[2]).unwrap().rect;
        assert!(first.x < second.x);
        assert!(third.y > first.y);
    }

    #[test]
    fn generated_flex_grid_and_scroll_bounds_match_paint_and_hit_clips() {
        fn check_bounds(world: &World, children: &[Entity], clip: Rect) -> (usize, usize) {
            let mut visible = 0;
            let mut clipped = 0;
            for &child in children {
                let layout = world.get::<UiLayoutResult>(child).unwrap();
                let hit_box = world.get::<UiHitBox>(child).unwrap();
                let rect = layout.rect;
                assert!(
                    [rect.x, rect.y, rect.width, rect.height]
                        .into_iter()
                        .all(f32::is_finite)
                );
                assert!(rect.width >= 0.0 && rect.height >= 0.0);
                assert_eq!(layout.clip, Some(clip));
                assert_eq!(hit_box.rect, rect);
                assert_eq!(hit_box.clip, layout.clip);

                let left = rect.x.max(clip.x);
                let top = rect.y.max(clip.y);
                let right = (rect.x + rect.width).min(clip.x + clip.width);
                let bottom = (rect.y + rect.height).min(clip.y + clip.height);
                if right > left && bottom > top {
                    visible += 1;
                    assert!(hit_box.contains([(left + right) * 0.5, (top + bottom) * 0.5]));
                } else {
                    clipped += 1;
                    assert!(
                        !hit_box.contains([rect.x + rect.width * 0.5, rect.y + rect.height * 0.5])
                    );
                }
                assert!(!hit_box.contains([clip.x - 0.5, clip.y + clip.height * 0.5]));
                assert!(
                    !hit_box.contains([clip.x + clip.width + 0.5, clip.y + clip.height * 0.5,])
                );
            }
            (visible, clipped)
        }

        let mut world = World::new();
        let window = world.spawn_empty().id();
        let flex_clip = Rect::new(10.0, 10.0, 220.0, 80.0);
        let flex = world
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, flex_clip, 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Flex {
                        direction: UiFlexDirection::Row,
                        wrap: true,
                    },
                    clip_contents: true,
                    gap: 4.0,
                    padding: UiEdges::all(2.0),
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let flex_children: Vec<_> = (0..30)
            .map(|index| {
                let width = 20 + (index * 17 % 50);
                let height = 8 + (index * 11 % 24);
                world
                    .spawn((
                        UiNode,
                        UiLayoutNode::leaf(UiLength::Px(width as f32), UiLength::Px(height as f32)),
                    ))
                    .id()
            })
            .collect();
        for child in &flex_children {
            set_parent(&mut world, *child, flex).unwrap();
        }

        let grid_clip = Rect::new(250.0, 10.0, 140.0, 80.0);
        let grid = world
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, grid_clip, 2.0),
                UiLayoutNode {
                    mode: UiLayoutMode::ResponsiveGrid {
                        min_cell_width: 40,
                        max_columns: 3,
                    },
                    clip_contents: true,
                    gap: 4.0,
                    padding: UiEdges::all(2.0),
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let grid_children: Vec<_> = (0..33)
            .map(|index| {
                let width = 12 + (index * 13 % 24);
                let height = 8 + (index * 7 % 20);
                world
                    .spawn((
                        UiNode,
                        UiLayoutNode::leaf(UiLength::Px(width as f32), UiLength::Px(height as f32)),
                    ))
                    .id()
            })
            .collect();
        for child in &grid_children {
            set_parent(&mut world, *child, grid).unwrap();
        }

        let scroll_clip = Rect::new(10.0, 120.0, 220.0, 80.0);
        let scroll = world
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, scroll_clip, 1.5),
                UiScroll::at(0.0, 10_000.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    clip_contents: true,
                    gap: 3.0,
                    padding: UiEdges::all(2.0),
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let scroll_children: Vec<_> = (0..24)
            .map(|index| {
                let height = 10 + (index * 7 % 18);
                world
                    .spawn((
                        UiNode,
                        UiLayoutNode::leaf(
                            UiLength::Px(40.0 + (index * 5 % 60) as f32),
                            UiLength::Px(height as f32),
                        ),
                    ))
                    .id()
            })
            .collect();
        for child in &scroll_children {
            set_parent(&mut world, *child, scroll).unwrap();
        }

        compute_layout(&mut world);

        let (flex_visible, flex_clipped) = check_bounds(&world, &flex_children, flex_clip);
        let (grid_visible, grid_clipped) = check_bounds(&world, &grid_children, grid_clip);
        let (scroll_visible, scroll_clipped) = check_bounds(&world, &scroll_children, scroll_clip);
        assert!(flex_visible > 0 && flex_clipped > 0);
        assert!(grid_visible > 0 && grid_clipped > 0);
        assert!(scroll_visible > 0 && scroll_clipped > 0);
        assert_eq!(world.get::<UiScroll>(scroll).unwrap().offset[0], 0.0);
        assert!(world.get::<UiScroll>(scroll).unwrap().offset[1] > 0.0);
        assert_eq!(
            world
                .get::<UiLayoutResult>(grid_children[0])
                .unwrap()
                .dpi_scale,
            2.0
        );
        assert_eq!(
            world
                .get::<UiLayoutResult>(scroll_children[0])
                .unwrap()
                .dpi_scale,
            1.5
        );
    }

    #[test]
    fn flex_wrap_uses_intrinsic_sizes_gaps_and_cross_axis_alignment() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 150.0, 120.0));
        world.entity_mut(parent).insert(UiLayoutNode {
            mode: UiLayoutMode::Flex {
                direction: UiFlexDirection::Row,
                wrap: true,
            },
            gap: 10.0,
            align_items: UiAlign::Center,
            padding: UiEdges::ZERO,
            ..UiLayoutNode::default()
        });
        let children: Vec<Entity> = [[60.0, 20.0], [60.0, 40.0], [40.0, 10.0]]
            .into_iter()
            .map(|size| {
                world
                    .spawn((UiNode, UiIntrinsicSize(size), UiLayoutNode::default()))
                    .id()
            })
            .collect();
        for child in &children {
            set_parent(&mut world, *child, parent).unwrap();
        }

        compute_layout(&mut world);

        let first = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
        let second = world.get::<UiLayoutResult>(children[1]).unwrap().rect;
        let third = world.get::<UiLayoutResult>(children[2]).unwrap().rect;
        assert_eq!(first, Rect::new(0.0, 10.0, 60.0, 20.0));
        assert_eq!(second, Rect::new(70.0, 0.0, 60.0, 40.0));
        assert_eq!(third, Rect::new(0.0, 50.0, 40.0, 10.0));
        assert_eq!(
            world.get::<UiLayoutResult>(parent).unwrap().content_size,
            [130.0, 60.0]
        );
    }

    #[test]
    fn exact_child_lengths_measure_without_expanding_nested_content() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 100.0));
        let fixed_container = world
            .spawn((
                UiNode,
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(40.0),
                        height: UiLength::Px(30.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let oversized_descendant = world
            .spawn((
                UiNode,
                UiIntrinsicSize([500.0, 400.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
            ))
            .id();
        set_parent(&mut world, fixed_container, parent).unwrap();
        set_parent(&mut world, oversized_descendant, fixed_container).unwrap();

        compute_layout(&mut world);

        assert_eq!(
            world.get::<UiLayoutResult>(parent).unwrap().content_size,
            [60.0, 50.0]
        );
        assert_eq!(
            world
                .get::<UiLayoutResult>(fixed_container)
                .unwrap()
                .content_size,
            [500.0, 400.0]
        );
        assert_eq!(
            world.get::<UiLayoutResult>(fixed_container).unwrap().rect,
            Rect::new(10.0, 10.0, 40.0, 30.0)
        );
    }

    #[test]
    fn flex_grow_respects_weights_and_maximum_constraints() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 200.0, 50.0));
        world.entity_mut(parent).insert(UiLayoutNode {
            mode: UiLayoutMode::Flex {
                direction: UiFlexDirection::Row,
                wrap: false,
            },
            gap: 10.0,
            padding: UiEdges::ZERO,
            ..UiLayoutNode::default()
        });
        let first = world
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                UiFlexItem {
                    basis: Some(UiLength::Px(40.0)),
                    grow: 1.0,
                    ..UiFlexItem::default()
                },
            ))
            .id();
        let second = world
            .spawn((
                UiNode,
                UiLayoutNode {
                    mode: UiLayoutMode::Leaf,
                    size: UiSize {
                        width: UiLength::Auto,
                        height: UiLength::Auto,
                    },
                    max_size: [Some(100.0), None],
                    ..UiLayoutNode::default()
                },
                UiFlexItem {
                    basis: Some(UiLength::Px(40.0)),
                    grow: 3.0,
                    ..UiFlexItem::default()
                },
            ))
            .id();
        set_parent(&mut world, first, parent).unwrap();
        set_parent(&mut world, second, parent).unwrap();

        compute_layout(&mut world);

        let first_rect = world.get::<UiLayoutResult>(first).unwrap().rect;
        let second_rect = world.get::<UiLayoutResult>(second).unwrap().rect;
        assert_eq!(first_rect.width, 90.0);
        assert_eq!(second_rect.width, 100.0);
        assert_eq!(second_rect.x, 100.0);

        let mut root_config = *world.get::<UiLayoutRoot>(parent).unwrap();
        root_config.viewport.width = 90.0;
        world.entity_mut(parent).insert(root_config);
        for child in [first, second] {
            let mut child_style = *world.get::<UiLayoutNode>(child).unwrap();
            child_style.min_size[0] = Some(50.0);
            world.entity_mut(child).insert(child_style);
        }
        compute_layout(&mut world);
        assert_eq!(world.get::<UiLayoutResult>(first).unwrap().rect.width, 50.0);
        assert_eq!(
            world.get::<UiLayoutResult>(second).unwrap().rect.width,
            50.0
        );
        assert_eq!(world.get::<UiLayoutResult>(second).unwrap().rect.x, 60.0);
    }

    #[test]
    fn flex_fill_basis_uses_same_wrapping_measurement_and_distribution() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 50.0));
        world.entity_mut(parent).insert(UiLayoutNode {
            mode: UiLayoutMode::Flex {
                direction: UiFlexDirection::Row,
                wrap: true,
            },
            gap: 10.0,
            padding: UiEdges::ZERO,
            ..UiLayoutNode::default()
        });
        let fill = world
            .spawn((
                UiNode,
                UiIntrinsicSize([60.0, 20.0]),
                UiLayoutNode {
                    mode: UiLayoutMode::Leaf,
                    size: UiSize {
                        width: UiLength::Fill(1.0),
                        height: UiLength::Auto,
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let fixed = world
            .spawn((
                UiNode,
                UiIntrinsicSize([40.0, 20.0]),
                UiLayoutNode::leaf(UiLength::Px(40.0), UiLength::Auto),
            ))
            .id();
        set_parent(&mut world, fill, parent).unwrap();
        set_parent(&mut world, fixed, parent).unwrap();

        compute_layout(&mut world);

        let fill_rect = world.get::<UiLayoutResult>(fill).unwrap().rect;
        let fixed_rect = world.get::<UiLayoutResult>(fixed).unwrap().rect;
        assert_eq!(fill_rect.width, 50.0);
        assert_eq!(fixed_rect.x, 60.0);
        assert_eq!(fixed_rect.y, 0.0);
        assert_eq!(
            world.get::<UiLayoutResult>(parent).unwrap().content_size,
            [50.0, 20.0]
        );
    }

    #[test]
    fn column_flex_wraps_into_intrinsic_columns() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 100.0));
        world.entity_mut(parent).insert(UiLayoutNode {
            mode: UiLayoutMode::Flex {
                direction: UiFlexDirection::Column,
                wrap: true,
            },
            gap: 10.0,
            padding: UiEdges::ZERO,
            ..UiLayoutNode::default()
        });
        let children: Vec<Entity> = [[20.0, 40.0], [40.0, 40.0], [30.0, 30.0]]
            .into_iter()
            .map(|size| {
                world
                    .spawn((UiNode, UiIntrinsicSize(size), UiLayoutNode::default()))
                    .id()
            })
            .collect();
        for child in &children {
            set_parent(&mut world, *child, parent).unwrap();
        }

        compute_layout(&mut world);

        let first = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
        let second = world.get::<UiLayoutResult>(children[1]).unwrap().rect;
        let third = world.get::<UiLayoutResult>(children[2]).unwrap().rect;
        assert_eq!(first, Rect::new(0.0, 0.0, 40.0, 40.0));
        assert_eq!(second, Rect::new(0.0, 50.0, 40.0, 40.0));
        assert_eq!(third, Rect::new(50.0, 0.0, 30.0, 30.0));
        assert_eq!(
            world.get::<UiLayoutResult>(parent).unwrap().content_size,
            [80.0, 90.0]
        );
    }

    #[test]
    fn flex_item_change_invalidates_and_reflows_its_layout_root() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let (_, parent) = root(app.world_mut(), Rect::new(0.0, 0.0, 200.0, 60.0));
        app.world_mut().entity_mut(parent).insert(UiLayoutNode {
            mode: UiLayoutMode::Flex {
                direction: UiFlexDirection::Row,
                wrap: false,
            },
            gap: 10.0,
            padding: UiEdges::ZERO,
            ..UiLayoutNode::default()
        });
        let first = app
            .world_mut()
            .spawn((
                UiNode,
                UiFlexItem {
                    basis: Some(UiLength::Px(50.0)),
                    ..UiFlexItem::default()
                },
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                UiNode,
                UiFlexItem {
                    basis: Some(UiLength::Px(50.0)),
                    ..UiFlexItem::default()
                },
            ))
            .id();
        set_parent(app.world_mut(), first, parent).unwrap();
        set_parent(app.world_mut(), second, parent).unwrap();
        app.update();
        assert_eq!(
            app.world().get::<UiLayoutResult>(second).unwrap().rect.x,
            60.0
        );

        app.world_mut().entity_mut(first).insert(UiFlexItem {
            basis: Some(UiLength::Px(80.0)),
            ..UiFlexItem::default()
        });
        app.update();
        assert_eq!(
            app.world().get::<UiLayoutResult>(second).unwrap().rect.x,
            90.0
        );
    }

    #[test]
    fn responsive_grid_reflows_at_width_threshold_and_uses_logical_pixels() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let parent = world
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 320.0, 200.0), 2.0),
                UiLayoutNode {
                    mode: UiLayoutMode::ResponsiveGrid {
                        min_cell_width: 100,
                        max_columns: 3,
                    },
                    gap: 10.0,
                    align_items: UiAlign::Start,
                    padding: UiEdges::ZERO,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let children: Vec<Entity> = [
            [20.0, 20.0],
            [30.0, 30.0],
            [40.0, 40.0],
            [50.0, 50.0],
            [60.0, 60.0],
        ]
        .into_iter()
        .map(|size| world.spawn((UiNode, UiIntrinsicSize(size))).id())
        .collect();
        for child in &children {
            set_parent(&mut world, *child, parent).unwrap();
        }

        compute_layout(&mut world);

        let first_row = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
        let third_cell = world.get::<UiLayoutResult>(children[2]).unwrap().rect;
        let second_row = world.get::<UiLayoutResult>(children[3]).unwrap().rect;
        assert_eq!(first_row.x, 0.0);
        assert_eq!(third_cell.x, 220.0);
        assert_eq!(second_row.y, 50.0);
        assert_eq!(world.get::<UiLayoutResult>(parent).unwrap().dpi_scale, 2.0);
        assert_eq!(
            world.get::<UiLayoutResult>(parent).unwrap().content_size,
            [320.0, 110.0]
        );

        let mut root_config = *world.get::<UiLayoutRoot>(parent).unwrap();
        root_config.viewport.width = 190.0;
        world.entity_mut(parent).insert(root_config);
        compute_layout(&mut world);

        let first = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
        let second = world.get::<UiLayoutResult>(children[1]).unwrap().rect;
        assert_eq!(first.x, 0.0);
        assert_eq!(second.x, 0.0);
        assert_eq!(second.y, 30.0);
        assert_eq!(world.get::<UiLayoutResult>(parent).unwrap().dpi_scale, 2.0);
    }

    #[test]
    fn layout_skips_non_nodes_and_sanitizes_invalid_dimensions() {
        let mut world = World::new();
        let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 50.0));
        let child = world
            .spawn((
                UiNode,
                UiVisible(true),
                UiLayoutNode::leaf(UiLength::Px(f32::NAN), UiLength::Px(-4.0)),
            ))
            .id();
        set_parent(&mut world, child, parent).unwrap();
        compute_layout(&mut world);
        assert_eq!(world.get::<UiLayoutResult>(child).unwrap().rect.width, 0.0);
        assert_eq!(world.get::<UiLayoutResult>(child).unwrap().rect.height, 0.0);
    }

    #[test]
    fn layout_phase_populates_hit_boxes_before_input_phase_in_the_same_frame() {
        let mut app = App::new();
        app.add_plugins(CorePlugin);
        app.add_plugins(LayoutPlugin);
        let (window, parent) = root(app.world_mut(), Rect::new(0.0, 0.0, 100.0, 60.0));
        let button = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiLayoutNode::leaf(UiLength::Px(40.0), UiLength::Px(24.0)),
            ))
            .id();
        set_parent(app.world_mut(), button, parent).unwrap();
        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [10.0, 10.0],
                time_seconds: 0.0,
            },
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position: [10.0, 10.0],
                time_seconds: 0.1,
            },
        ]);
        app.update();
        let result = app.world().get::<UiLayoutResult>(button).unwrap();
        assert_eq!(result.window, window);
        assert!(app.world().resource::<UiRoutedEvents>().events.iter().any(
            |event| matches!(event, UiRoutedEvent::Click { target, .. } if *target == button)
        ));
    }

    #[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
    struct LayoutChanges {
        results: usize,
        hit_boxes: usize,
        issues: usize,
    }

    fn count_layout_changes(
        results: Query<Ref<UiLayoutResult>>,
        hit_boxes: Query<Ref<UiHitBox>>,
        issues: Res<UiLayoutIssues>,
        mut changes: ResMut<LayoutChanges>,
    ) {
        changes.results += results.iter().filter(|result| result.is_changed()).count();
        changes.hit_boxes += hit_boxes
            .iter()
            .filter(|hit_box| hit_box.is_changed())
            .count();
        changes.issues += usize::from(issues.is_changed());
    }

    #[test]
    fn unchanged_layout_results_and_hit_boxes_do_not_trigger_change_detection() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin))
            .init_resource::<LayoutChanges>()
            .add_systems(Update, count_layout_changes.in_set(UiSystems::Render));
        let (window, parent) = root(app.world_mut(), Rect::new(0.0, 0.0, 100.0, 50.0));
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([1.0, 1.0]),
                UiLayoutNode::leaf(UiLength::Px(30.0), UiLength::Px(20.0)),
            ))
            .id();
        set_parent(app.world_mut(), child, parent).unwrap();

        app.update();
        let first = *app.world().resource::<LayoutChanges>();
        let first_work = *app.world().resource::<UiLayoutStats>();
        assert_eq!(first.results, 2);
        assert_eq!(first.hit_boxes, 2);
        assert_eq!(first_work.roots_processed, 1);
        assert_eq!(first_work.nodes_processed, 2);
        app.update();
        assert_eq!(*app.world().resource::<LayoutChanges>(), first);
        let idle_work = *app.world().resource::<UiLayoutStats>();
        assert_eq!(idle_work.roots_processed, 0);
        assert_eq!(idle_work.nodes_processed, 0);
        assert_eq!(idle_work.idle_frames, first_work.idle_frames + 1);

        let layout = *app.world().get::<UiLayoutNode>(child).unwrap();
        app.world_mut()
            .entity_mut(child)
            .insert((layout, UiIntrinsicSize([1.0, 1.0])));
        app.update();
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 0);
        assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 0);

        invalidate_ui_layout(app.world_mut(), child);
        app.update();
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
        assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 2);

        app.world_mut()
            .get_mut::<UiLayoutRoot>(parent)
            .unwrap()
            .viewport = Rect::new(0.0, 0.0, 120.0, 50.0);
        app.update();
        let resized = *app.world().resource::<LayoutChanges>();
        let resized_work = *app.world().resource::<UiLayoutStats>();
        assert!(resized.results > first.results);
        assert!(resized.hit_boxes > first.hit_boxes);
        assert_eq!(resized_work.roots_processed, 1);
        assert_eq!(resized_work.nodes_processed, 2);
        assert_eq!(resized.issues, first.issues);
        assert_eq!(
            app.world().get::<UiLayoutResult>(child).unwrap().window,
            window
        );
    }

    #[test]
    fn intrinsic_and_ancestor_style_changes_refresh_descendant_geometry_and_clip() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 100.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(60.0),
                    },
                    align_items: UiAlign::Start,
                    clip_contents: true,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let text = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([120.0, 24.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
            ))
            .id();
        set_parent(app.world_mut(), parent, root).unwrap();
        set_parent(app.world_mut(), text, parent).unwrap();

        app.update();
        let initial = *app.world().get::<UiLayoutResult>(text).unwrap();
        assert_eq!(initial.rect.width, 120.0);
        assert_eq!(initial.clip, Some(Rect::new(0.0, 0.0, 80.0, 60.0)));

        app.world_mut()
            .entity_mut(text)
            .insert(UiIntrinsicSize([150.0, 24.0]));
        app.update();
        let intrinsic = *app.world().get::<UiLayoutResult>(text).unwrap();
        assert_eq!(intrinsic.rect.width, 150.0);
        assert_eq!(intrinsic.clip, initial.clip);
        assert_eq!(
            app.world().get::<UiHitBox>(text).unwrap().clip,
            intrinsic.clip
        );
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
        assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 3);

        let mut parent_style = app.world_mut().get_mut::<UiLayoutNode>(parent).unwrap();
        parent_style.size.width = UiLength::Px(50.0);
        app.update();
        let changed_ancestor = *app.world().get::<UiLayoutResult>(text).unwrap();
        assert_eq!(changed_ancestor.rect.width, 150.0);
        assert_eq!(changed_ancestor.clip, Some(Rect::new(0.0, 0.0, 50.0, 60.0)));
        assert_eq!(
            app.world().get::<UiHitBox>(text).unwrap().clip,
            changed_ancestor.clip
        );
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
        assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 3);
    }

    #[test]
    fn reparent_and_detach_invalidate_old_and_new_roots_and_clear_stale_hitboxes() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window_a = app.world_mut().spawn_empty().id();
        let window_b = app.world_mut().spawn_empty().id();
        let root_a = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window_a, Rect::new(0.0, 0.0, 100.0, 50.0), 1.0),
                UiLayoutNode::default(),
            ))
            .id();
        let root_b = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window_b, Rect::new(200.0, 0.0, 100.0, 50.0), 2.0),
                UiLayoutNode::default(),
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(30.0), UiLength::Px(20.0)),
            ))
            .id();
        set_parent(app.world_mut(), child, root_a).unwrap();
        app.update();
        assert_eq!(
            app.world().get::<UiLayoutResult>(child).unwrap().window,
            window_a
        );

        set_parent(app.world_mut(), child, root_b).unwrap();
        app.update();
        let moved = app.world().get::<UiLayoutResult>(child).unwrap();
        assert_eq!(moved.window, window_b);
        assert_eq!(moved.rect.x, 200.0);
        assert_eq!(moved.dpi_scale, 2.0);
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 2);

        bui_core::detach_from_parent(app.world_mut(), child);
        app.update();
        assert!(app.world().get::<UiLayoutResult>(child).is_none());
        assert!(app.world().get::<UiHitBox>(child).is_none());
        assert_eq!(app.world().resource::<UiLayoutStats>().roots_processed, 1);
    }

    #[test]
    fn viewport_registry_rect_and_dpi_changes_are_detected_without_tree_mutation() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let root = app.world_mut().spawn(UiNode).id();
        app.world_mut().resource_mut::<UiViewports>().0.insert(
            root,
            UiViewport {
                rect: Rect::new(3.0, 4.0, 80.0, 50.0),
                dpi_scale: 1.0,
            },
        );
        app.update();
        let first = *app.world().get::<UiLayoutResult>(root).unwrap();
        assert_eq!(first.rect, Rect::new(3.0, 4.0, 80.0, 50.0));
        assert_eq!(first.dpi_scale, 1.0);

        app.world_mut().resource_mut::<UiViewports>().0.insert(
            root,
            UiViewport {
                rect: Rect::new(3.0, 4.0, 90.0, 55.0),
                dpi_scale: 2.0,
            },
        );
        app.update();
        let resized = app.world().get::<UiLayoutResult>(root).unwrap();
        assert_eq!(resized.rect, Rect::new(3.0, 4.0, 90.0, 55.0));
        assert_eq!(resized.dpi_scale, 2.0);
        assert_eq!(app.world().resource::<UiLayoutStats>().nodes_processed, 1);
    }

    #[test]
    fn intrinsic_change_recomputes_only_its_node_when_parent_allocation_is_independent() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 300.0, 120.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let branch = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(60.0),
                    },
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let sibling_branch = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    size: UiSize {
                        width: UiLength::Px(50.0),
                        height: UiLength::Px(50.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let dirty_leaf = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([2.0, 3.0]),
                UiLayoutNode::leaf(UiLength::Px(20.0), UiLength::Px(10.0)),
            ))
            .id();
        let sibling_leaf = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([4.0, 5.0]),
                UiLayoutNode::leaf(UiLength::Px(15.0), UiLength::Px(12.0)),
            ))
            .id();
        set_parent(app.world_mut(), branch, root).unwrap();
        set_parent(app.world_mut(), sibling_branch, root).unwrap();
        set_parent(app.world_mut(), dirty_leaf, branch).unwrap();
        set_parent(app.world_mut(), sibling_leaf, sibling_branch).unwrap();

        app.update();
        let sibling_before = *app.world().get::<UiLayoutResult>(sibling_leaf).unwrap();
        let sibling_hit_box_before = *app.world().get::<UiHitBox>(sibling_leaf).unwrap();
        let dirty_rect_before = app.world().get::<UiLayoutResult>(dirty_leaf).unwrap().rect;
        let dirty_hit_box_before = *app.world().get::<UiHitBox>(dirty_leaf).unwrap();
        app.world_mut()
            .entity_mut(dirty_leaf)
            .insert(UiIntrinsicSize([9.0, 7.0]));
        app.update();

        let dirty = app.world().get::<UiLayoutResult>(dirty_leaf).unwrap();
        assert_eq!(dirty.content_size, [9.0, 7.0]);
        assert_eq!(dirty.rect, dirty_rect_before);
        assert_eq!(
            *app.world().get::<UiHitBox>(dirty_leaf).unwrap(),
            dirty_hit_box_before
        );
        assert_eq!(
            *app.world().get::<UiLayoutResult>(sibling_leaf).unwrap(),
            sibling_before
        );
        assert_eq!(
            *app.world().get::<UiHitBox>(sibling_leaf).unwrap(),
            sibling_hit_box_before
        );
        let work = app.world().resource::<UiLayoutStats>();
        assert_eq!(work.roots_processed, 1);
        assert_eq!(
            work.nodes_processed, 1,
            "only the changed leaf should be visited"
        );
    }

    #[test]
    fn flex_intrinsic_change_reflows_and_visits_only_geometry_affected_siblings() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 60.0), 1.0),
                UiLayoutNode::flex(UiFlexDirection::Row, false, 0.0),
            ))
            .id();
        let children: Vec<_> = (0..3)
            .map(|_| {
                app.world_mut()
                    .spawn((
                        UiNode,
                        UiIntrinsicSize([40.0, 20.0]),
                        UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                        UiFlexItem {
                            grow: 0.0,
                            shrink: 0.0,
                            ..UiFlexItem::default()
                        },
                    ))
                    .id()
            })
            .collect();
        for child in &children {
            set_parent(app.world_mut(), *child, root).unwrap();
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[2])
                .unwrap()
                .rect
                .x,
            80.0
        );

        app.world_mut()
            .entity_mut(children[0])
            .insert(UiIntrinsicSize([60.0, 20.0]));
        app.update();

        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[1])
                .unwrap()
                .rect
                .x,
            60.0
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[2])
                .unwrap()
                .rect
                .x,
            100.0
        );
        let work = app.world().resource::<UiLayoutStats>();
        assert_eq!(work.roots_processed, 1);
        assert_eq!(
            work.nodes_processed, 4,
            "the dirty item and two shifted siblings should be visited with the root"
        );
    }

    #[test]
    fn responsive_grid_intrinsic_change_updates_row_height_and_following_rows() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 100.0), 1.0),
                UiLayoutNode::responsive_grid(80, 2, 0.0),
            ))
            .id();
        let children: Vec<_> = (0..4)
            .map(|_| {
                app.world_mut()
                    .spawn((
                        UiNode,
                        UiIntrinsicSize([80.0, 10.0]),
                        UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                    ))
                    .id()
            })
            .collect();
        for child in &children {
            set_parent(app.world_mut(), *child, root).unwrap();
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[2])
                .unwrap()
                .rect
                .y,
            10.0
        );

        app.world_mut()
            .entity_mut(children[0])
            .insert(UiIntrinsicSize([80.0, 40.0]));
        app.update();

        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[0])
                .unwrap()
                .rect
                .height,
            40.0
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[1])
                .unwrap()
                .rect
                .height,
            40.0
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[2])
                .unwrap()
                .rect
                .y,
            40.0
        );
        assert_eq!(
            app.world()
                .get::<UiLayoutResult>(children[3])
                .unwrap()
                .rect
                .y,
            40.0
        );
        let work = app.world().resource::<UiLayoutStats>();
        assert_eq!(work.roots_processed, 1);
        assert_eq!(
            work.nodes_processed, 5,
            "all cells whose row geometry changed should be visited"
        );
    }

    #[test]
    fn ten_thousand_fixed_basis_flex_siblings_recompute_only_the_changed_leaf() {
        const NODE_COUNT: usize = 10_000;
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
                UiLayoutNode::flex(UiFlexDirection::Row, true, 3.0),
            ))
            .id();
        let mut first = None;
        for _ in 0..NODE_COUNT {
            let child = app
                .world_mut()
                .spawn((
                    UiNode,
                    UiIntrinsicSize([44.0, 22.0]),
                    UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                    UiFlexItem {
                        basis: Some(UiLength::Px(44.0)),
                        grow: 0.25,
                        ..UiFlexItem::default()
                    },
                ))
                .id();
            set_parent(app.world_mut(), child, root).unwrap();
            first.get_or_insert(child);
        }
        let first = first.unwrap();
        app.update();
        let result_before = *app.world().get::<UiLayoutResult>(first).unwrap();
        let sibling = app.world().get::<UiChildren>(root).unwrap().0[1];
        let sibling_before = *app.world().get::<UiLayoutResult>(sibling).unwrap();

        app.world_mut()
            .entity_mut(first)
            .insert(UiIntrinsicSize([45.0, 22.0]));
        app.update();

        assert_eq!(
            app.world().get::<UiLayoutResult>(first).unwrap().rect,
            result_before.rect
        );
        assert_eq!(
            *app.world().get::<UiLayoutResult>(sibling).unwrap(),
            sibling_before
        );
        let work = app.world().resource::<UiLayoutStats>();
        assert_eq!(work.roots_processed, 1);
        assert_eq!(work.nodes_processed, 1);
    }

    #[test]
    fn measurement_cache_stays_bounded_when_ancestor_width_changes_repeatedly() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 400.0, 200.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    size: UiSize {
                        width: UiLength::Px(100.0),
                        height: UiLength::Px(80.0),
                    },
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let leaf = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([20.0, 10.0]),
                UiLayoutNode::leaf(UiLength::Percent(0.5), UiLength::Px(10.0)),
            ))
            .id();
        set_parent(app.world_mut(), parent, root).unwrap();
        set_parent(app.world_mut(), leaf, parent).unwrap();
        app.update();

        for frame in 0..32 {
            let width = 80.0 + frame as f32 * 3.5;
            app.world_mut()
                .get_mut::<UiLayoutNode>(parent)
                .unwrap()
                .size
                .width = UiLength::Px(width);
            app.update();

            assert_eq!(
                app.world().get::<UiLayoutResult>(leaf).unwrap().rect.width,
                width * 0.5
            );
            let cache = app.world().resource::<UiMeasurementCache>();
            let leaf_measurements = cache
                .0
                .get(&leaf)
                .expect("leaf should have cached measurements")
                .len();
            assert!(
                leaf_measurements <= MAX_MEASUREMENT_ENTRIES,
                "leaf cache grew to {leaf_measurements} entries"
            );
        }

        let leaf_measurements = app
            .world()
            .resource::<UiMeasurementCache>()
            .0
            .get(&leaf)
            .unwrap()
            .len();
        assert_eq!(leaf_measurements, MAX_MEASUREMENT_ENTRIES);
    }
}

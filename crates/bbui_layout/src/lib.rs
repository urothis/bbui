//! Shared measured layout geometry for rendering and pointer hit testing.

use bevy_app::{App, Plugin, Update};
use bevy_ecs::lifecycle::RemovedComponentReader;
use bevy_ecs::prelude::*;
use bevy_ecs::{change_detection::Tick, component::ComponentId};

use bbui_core::{
    UiChildren, UiDirty, UiDirtyDomain, UiHitBox, UiKeyCode, UiNode, UiParent, UiRoutedEvent,
    UiRoutedEvents, UiSystems, UiVisible, UiWheelUnit, UiZIndex, ui_components_changed_since,
};
use std::collections::{HashMap, HashSet};

pub use bbui_core::UiRect as Rect;

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
#[component(summary_tick)]
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

/// Reveals a container's measured content height while keeping its vertical padding.
/// `progress` is clamped to `0..=1`; descendants keep their natural sizes while
/// following siblings reflow around the visible height. Set `clip_contents` on
/// the owner when its descendants must not paint outside that height.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[component(summary_tick)]
pub struct UiContentReveal {
    pub progress: f32,
}

impl UiContentReveal {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: if progress.is_finite() {
                progress.clamp(0.0, 1.0)
            } else {
                0.0
            },
        }
    }
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
#[component(summary_tick)]
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
#[component(summary_tick)]
pub struct UiIntrinsicSize(pub [f32; 2]);

/// Logical-pixel scroll position for a clipped layout container.
///
/// The layout pass clamps the requested offset to the measured content extent,
/// translates descendants by the effective offset, and uses the container rect
/// as their inherited clip. The same resulting rect and clip are written to
/// `UiHitBox`, so hit testing follows the scrolled content.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
#[component(summary_tick)]
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
#[component(summary_tick)]
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
#[component(summary_tick)]
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
    removed_visibles: RemovedComponentReader<UiVisible>,
    removed_content_reveals: RemovedComponentReader<UiContentReveal>,
    removed_roots: RemovedComponentReader<UiLayoutRoot>,
}

#[derive(Resource, Default)]
struct UiLayoutCollectorGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LayoutInputs {
    is_node: bool,
    root: Option<UiLayoutRoot>,
    layout: Option<UiLayoutNode>,
    content_reveal: Option<UiContentReveal>,
    intrinsic: Option<UiIntrinsicSize>,
    parent: Option<UiParent>,
    children: Option<UiChildren>,
    z_index: Option<UiZIndex>,
    scroll: Option<UiScroll>,
    flex_item: Option<UiFlexItem>,
    visible: bool,
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

mod scroll;
pub use scroll::apply_routed_scroll_input;

mod compute;
pub use compute::compute_layout;

mod changes;
use changes::*;

mod pass;
use pass::*;

mod measure;
use measure::*;

mod flex;
use flex::*;

mod place;
use place::*;

mod math;
use math::*;

#[cfg(test)]
#[path = "layout/tests.rs"]
mod tests;

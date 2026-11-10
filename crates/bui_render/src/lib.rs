//! Renderer-facing draw data and a Bevy UI GPU backend.
//!
//! [`BuiRenderPlugin`] translates [`DisplayList`] items into Bevy UI nodes. Bevy's UI render
//! pipeline draws those nodes on the GPU; this crate does not rasterize widgets on the CPU.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::OnceLock,
};

use bevy::{
    asset::{Asset, Assets, Handle, RenderAssetUsages},
    camera::Camera,
    ecs::{
        change_detection::DetectChanges,
        hierarchy::ChildSpawnerCommands,
        prelude::{Changed, Or, Query, RemovedComponents},
        schedule::IntoScheduleConfigs,
        system::SystemParam,
    },
    math::Rot2,
    prelude::{
        App, BackgroundColor, BorderColor, BorderRadius, Color, Commands, Component, Entity,
        GlobalZIndex, Image, ImageNode, Node, Overflow, Plugin, PositionType, Res, ResMut,
        Resource, Text, Update, Val, ZIndex,
    },
    text::{
        Justify, LetterSpacing, LineBreak, LineHeight, TextBounds, TextColor, TextFont, TextLayout,
    },
    ui::{BoxShadow, UiRect, UiTargetCamera, UiTransform},
};

use bevy::{
    math::{UVec2, Vec2, Vec4},
    reflect::TypePath,
    render::{RenderApp, render_resource::AsBindGroup, storage::ShaderBuffer},
    shader::ShaderRef,
    ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin},
};
use bui_core::{UiDirty, UiDirtyDomain, UiParent, UiSystems};
use bui_layout::{Rect, UiLayoutResult};
use bui_style::{Rgba, Theme};
use bui_text::{register_default_ui_font, ui_default_text_font};

mod primitives;
pub use primitives::*;

mod display_list;
pub use display_list::*;

mod image_registry;
pub use image_registry::*;

mod surface;
pub use surface::*;

/// Adds the Bevy UI nodes that Bevy's GPU UI pipeline renders from [`DisplayList`].
pub struct BuiRenderPlugin;

#[derive(Asset, AsBindGroup, Clone, Debug, TypePath)]
struct HsvWheelMaterial {
    #[uniform(0)]
    parameters: Vec4,
}

#[derive(Asset, AsBindGroup, Clone, Debug, TypePath)]
struct BezierMaterial {
    #[uniform(0)]
    points_01: Vec4,
    #[uniform(1)]
    points_23: Vec4,
    #[uniform(2)]
    color: Vec4,
    #[uniform(3)]
    style: Vec4,
}

#[derive(Asset, AsBindGroup, Clone, Debug, TypePath)]
struct BezierBatchMaterial {
    #[storage(0, read_only)]
    segments: Handle<ShaderBuffer>,
    #[storage(1, read_only)]
    curves: Handle<ShaderBuffer>,
    #[storage(2, read_only)]
    tile_indices: Handle<ShaderBuffer>,
    #[uniform(3)]
    parameters: Vec4,
}

impl UiMaterial for HsvWheelMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://bui_render/hsv_wheel.wesl".into()
    }
}

impl UiMaterial for BezierMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://bui_render/bezier.wesl".into()
    }
}

impl UiMaterial for BezierBatchMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://bui_render/bezier_batch.wesl".into()
    }
}

impl Plugin for BuiRenderPlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(RenderApp).is_some() {
            bevy::asset::embedded_asset!(app, "hsv_wheel.wesl");
            bevy::asset::embedded_asset!(app, "bezier.wesl");
            app.add_plugins(UiMaterialPlugin::<HsvWheelMaterial>::default());
            app.add_plugins(UiMaterialPlugin::<BezierMaterial>::default());
            // WebGL2 has no storage buffers. The renderer already falls back to
            // per-curve uniform materials when the batch material is absent.
            #[cfg(not(target_arch = "wasm32"))]
            bevy::asset::embedded_asset!(app, "bezier_batch.wesl");
            #[cfg(not(target_arch = "wasm32"))]
            app.add_plugins(UiMaterialPlugin::<BezierBatchMaterial>::default());
        }
        app.init_resource::<DisplayList>()
            .init_resource::<UiDirty>()
            .init_resource::<ImageRegistry>()
            .init_resource::<RenderTargetCamera>()
            .init_resource::<RenderCache>()
            .init_resource::<RenderSyncStats>()
            .add_systems(
                Update,
                register_default_ui_font
                    .in_set(UiSystems::Render)
                    .before(collect_render_changes),
            )
            .add_systems(
                Update,
                (collect_render_changes, sync_display_list)
                    .chain()
                    .in_set(UiSystems::Render),
            );
        register_default_ui_font(app.world_mut());
    }
}

#[derive(Component)]
struct RenderedItem;

#[derive(Component)]
struct RenderedClipGroup;

#[derive(Default, Resource)]
struct RenderCache {
    items: HashMap<RenderKey, CachedItem>,
    clip_groups: HashMap<ClipGroupKey, CachedClipGroup>,
    clip_group_users: HashMap<ClipGroupKey, usize>,
    bezier_batches: HashMap<BezierBatchKey, CachedBezierBatch>,
    desired: Vec<DesiredItem>,
    fragment_keys: HashMap<Entity, Vec<RenderKey>>,
    fragment_order_keys: HashMap<Entity, Vec<WidgetOrderKey>>,
    widget_order_labels: BTreeMap<WidgetOrderKey, i32>,
    fragment_batches: HashMap<Entity, Vec<BezierBatchKey>>,
    bezier_batch_for_key: HashMap<RenderKey, BezierBatchKey>,
    layout_order: HashMap<Entity, usize>,
    layout_entity_order_keys: HashMap<Entity, LayoutOrderKey>,
    layout_order_labels: BTreeMap<LayoutOrderKey, i32>,
    image_users: HashMap<u64, HashSet<RenderKey>>,
    parents_by_entity: HashMap<Entity, Entity>,
    children_by_parent: HashMap<Entity, Vec<Entity>>,
    layout_entities_by_window: HashMap<Entity, HashSet<Entity>>,
    layout_window_by_entity: HashMap<Entity, Entity>,
    widget_fragments_by_camera: HashMap<Entity, HashSet<Entity>>,
    widget_fragment_camera: HashMap<Entity, Entity>,
    generation: u64,
    target_camera: Option<Entity>,
}

/// Counters for verifying that a renderer update stays scoped to the changed paint fragments.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RenderSyncStats {
    /// Draw keys resolved or compared during the most recent update.
    pub scanned_draw_keys: usize,
    /// Render cache entries whose GPU/UI nodes were replaced or removed.
    pub updated_render_entries: usize,
    /// Number of complete display-list synchronization passes so far.
    pub full_syncs: u64,
    /// Number of entity-scoped synchronization passes so far.
    pub incremental_syncs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BezierBatchKey {
    first: RenderKey,
    last: RenderKey,
    clip: ClipGroupKey,
}

#[derive(Clone)]
struct CachedBezierBatch {
    entity: Entity,
    material: Handle<BezierBatchMaterial>,
    segments: Handle<ShaderBuffer>,
    curves: Handle<ShaderBuffer>,
    tile_indices: Handle<ShaderBuffer>,
    signature: Vec<DrawItem>,
    keys: Vec<RenderKey>,
    rect: ClipRect,
    first_z: i32,
    local_z: i32,
    generation: u64,
}

struct BezierBatchSyncContext<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    cache: &'a mut RenderCache,
    materials: &'a mut Assets<BezierBatchMaterial>,
    buffers: &'a mut Assets<ShaderBuffer>,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum RenderKey {
    ManualDisplayListIndex(usize),
    WidgetDisplayListIndex(usize),
    OverlayWidgetDisplayListIndex(usize),
    CameraDisplayListIndex(Entity, usize),
    OverlayCameraDisplayListIndex(Entity, usize),
    WidgetFragment(Entity, usize),
    LayoutEntity(Entity),
}

impl RenderKey {
    fn stable_order(
        self,
        fragment_order: Option<u64>,
        hierarchy_depth: u32,
        target_camera: Option<Entity>,
    ) -> RenderOrder {
        match self {
            Self::ManualDisplayListIndex(index) => RenderOrder::Manual(index as u64),
            // Entity-attached UI paints include root backgrounds. At equal z they must be
            // created before display-list widget content, otherwise a root-sized background can
            // cover its same-z labels and controls.
            Self::LayoutEntity(entity) => RenderOrder::Layout(hierarchy_depth, entity.to_bits()),
            Self::WidgetFragment(entity, index) => match target_camera {
                None => RenderOrder::WidgetFragment(
                    fragment_order.unwrap_or_default(),
                    entity.to_bits(),
                    index as u64,
                ),
                Some(camera) => RenderOrder::CameraFragment(
                    camera.to_bits(),
                    fragment_order.unwrap_or_default(),
                    entity.to_bits(),
                    index as u64,
                ),
            },
            Self::WidgetDisplayListIndex(index) => RenderOrder::WidgetList(index as u64),
            Self::OverlayWidgetDisplayListIndex(index) => RenderOrder::OverlayWidget(index as u64),
            Self::CameraDisplayListIndex(camera, index) => {
                RenderOrder::Camera(camera.to_bits(), index as u64)
            }
            Self::OverlayCameraDisplayListIndex(camera, index) => {
                RenderOrder::OverlayCamera(camera.to_bits(), index as u64)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RenderOrder {
    Manual(u64),
    Layout(u32, u64),
    WidgetFragment(u64, u64, u64),
    WidgetList(u64),
    OverlayWidget(u64),
    Camera(u64, u64),
    CameraFragment(u64, u64, u64, u64),
    OverlayCamera(u64, u64),
}

const LOCAL_Z_LAYER_STRIDE: i32 = 300_000_000;
const WIDGET_FRAGMENT_LOCAL_Z_START: i32 = LOCAL_Z_LAYER_STRIDE * 2;
const WIDGET_FRAGMENT_LOCAL_Z_END: i32 = LOCAL_Z_LAYER_STRIDE * 3;
const CAMERA_FRAGMENT_LOCAL_Z_START: i32 = LOCAL_Z_LAYER_STRIDE * 5 + 100_000_000;
const CAMERA_FRAGMENT_LOCAL_Z_END: i32 = LOCAL_Z_LAYER_STRIDE * 6;

fn desired_local_z(
    desired: &DesiredItem,
    layout_order: &HashMap<Entity, usize>,
    layout_order_labels: &BTreeMap<LayoutOrderKey, i32>,
    widget_order_labels: &BTreeMap<WidgetOrderKey, i32>,
) -> i32 {
    let (layer, order) = match desired.key {
        RenderKey::ManualDisplayListIndex(index) => (0, index as u64),
        RenderKey::LayoutEntity(entity) => {
            let _ = layout_order;
            return layout_order_labels
                .get(&LayoutOrderKey {
                    depth: desired.hierarchy_depth,
                    entity_bits: entity.to_bits(),
                })
                .copied()
                .or_else(|| {
                    layout_order
                        .get(&entity)
                        .map(|rank| LOCAL_Z_LAYER_STRIDE + *rank as i32)
                })
                .unwrap_or(LOCAL_Z_LAYER_STRIDE);
        }
        RenderKey::WidgetFragment(entity, index) => {
            let key = WidgetOrderKey {
                order: desired.fragment_order.unwrap_or_default(),
                entity_bits: entity.to_bits(),
                item_index: index,
            };
            let label = widget_order_labels
                .get(&key)
                .copied()
                .unwrap_or(WIDGET_FRAGMENT_LOCAL_Z_START);
            return if desired.target_camera.is_some() {
                let relative = i64::from(label - WIDGET_FRAGMENT_LOCAL_Z_START);
                let scaled = relative
                    * i64::from(CAMERA_FRAGMENT_LOCAL_Z_END - CAMERA_FRAGMENT_LOCAL_Z_START)
                    / i64::from(WIDGET_FRAGMENT_LOCAL_Z_END - WIDGET_FRAGMENT_LOCAL_Z_START);
                CAMERA_FRAGMENT_LOCAL_Z_START + scaled as i32
            } else {
                label
            };
        }
        RenderKey::WidgetDisplayListIndex(index) => (3, index as u64),
        RenderKey::OverlayWidgetDisplayListIndex(index) => (4, index as u64),
        RenderKey::CameraDisplayListIndex(_, index) => (5, index as u64),
        RenderKey::OverlayCameraDisplayListIndex(_, index) => (6, index as u64),
    };
    let rank = i32::try_from(order.min((LOCAL_Z_LAYER_STRIDE - 1) as u64))
        .unwrap_or(LOCAL_Z_LAYER_STRIDE - 1);
    layer * LOCAL_Z_LAYER_STRIDE + rank
}

fn rebuild_widget_order_labels(cache: &mut RenderCache, desired: &[DesiredItem]) {
    cache.fragment_order_keys.clear();
    cache.widget_order_labels.clear();
    let mut keys: Vec<_> = desired
        .iter()
        .filter_map(|desired| {
            let RenderKey::WidgetFragment(entity, item_index) = desired.key else {
                return None;
            };
            Some((
                entity,
                WidgetOrderKey {
                    order: desired.fragment_order.unwrap_or_default(),
                    entity_bits: entity.to_bits(),
                    item_index,
                },
            ))
        })
        .collect();
    keys.sort_unstable_by_key(|(_, key)| *key);
    if keys.is_empty() {
        return;
    }
    let width = i64::from(WIDGET_FRAGMENT_LOCAL_Z_END - WIDGET_FRAGMENT_LOCAL_Z_START);
    let step = (width / (keys.len() as i64 + 1)).max(1);
    for (rank, (entity, key)) in keys.into_iter().enumerate() {
        let rank = i64::try_from(rank + 1).unwrap_or(i64::MAX);
        let label = i64::from(WIDGET_FRAGMENT_LOCAL_Z_START)
            .saturating_add(step.saturating_mul(rank))
            .min(i64::from(WIDGET_FRAGMENT_LOCAL_Z_END - 1)) as i32;
        cache.widget_order_labels.insert(key, label);
        cache
            .fragment_order_keys
            .entry(entity)
            .or_default()
            .push(key);
    }
}

fn update_widget_order_labels(
    cache: &mut RenderCache,
    entity: Entity,
    fragment: Option<&WidgetRenderFragment>,
) -> bool {
    if let Some(previous) = cache.fragment_order_keys.remove(&entity) {
        for key in previous {
            cache.widget_order_labels.remove(&key);
        }
    }
    let Some(fragment) = fragment else {
        return true;
    };
    if fragment.items.is_empty() {
        cache.fragment_order_keys.insert(entity, Vec::new());
        return true;
    }
    let keys: Vec<_> = (0..fragment.items.len())
        .map(|item_index| WidgetOrderKey {
            order: fragment.order,
            entity_bits: entity.to_bits(),
            item_index,
        })
        .collect();
    let Some((&first, &last)) = keys.first().zip(keys.last()) else {
        return true;
    };
    let previous = cache
        .widget_order_labels
        .range(..first)
        .next_back()
        .map(|(_, label)| *label)
        .unwrap_or(WIDGET_FRAGMENT_LOCAL_Z_START);
    let next = cache
        .widget_order_labels
        .range(last..)
        .next()
        .map(|(_, label)| *label)
        .unwrap_or(WIDGET_FRAGMENT_LOCAL_Z_END);
    let gap = i64::from(next) - i64::from(previous);
    if gap <= keys.len() as i64 {
        return false;
    }
    let step = gap / (keys.len() as i64 + 1);
    let mut labels = Vec::with_capacity(keys.len());
    for (index, key) in keys.into_iter().enumerate() {
        let offset = step.saturating_mul(index as i64 + 1);
        let label = (i64::from(previous) + offset) as i32;
        cache.widget_order_labels.insert(key, label);
        labels.push(key);
    }
    cache.fragment_order_keys.insert(entity, labels);
    true
}

fn rebuild_layout_order_labels(cache: &mut RenderCache, desired: &[DesiredItem]) {
    cache.layout_entity_order_keys.clear();
    cache.layout_order_labels.clear();
    let mut keys: Vec<_> = desired
        .iter()
        .filter_map(|desired| match desired.key {
            RenderKey::LayoutEntity(entity) => Some((
                entity,
                LayoutOrderKey {
                    depth: desired.hierarchy_depth,
                    entity_bits: entity.to_bits(),
                },
            )),
            _ => None,
        })
        .collect();
    keys.sort_unstable_by_key(|(_, key)| *key);
    let width = i64::from(LOCAL_Z_LAYER_STRIDE);
    let step = (width / (keys.len() as i64 + 1)).max(1);
    for (index, (entity, key)) in keys.into_iter().enumerate() {
        let offset = step.saturating_mul(index as i64 + 1);
        let label = (i64::from(LOCAL_Z_LAYER_STRIDE) + offset)
            .min(i64::from(LOCAL_Z_LAYER_STRIDE * 2 - 1)) as i32;
        cache.layout_entity_order_keys.insert(entity, key);
        cache.layout_order_labels.insert(key, label);
    }
}

fn insert_layout_order_label(cache: &mut RenderCache, entity: Entity, key: LayoutOrderKey) -> bool {
    let previous = cache
        .layout_order_labels
        .range(..key)
        .next_back()
        .map(|(_, label)| *label)
        .unwrap_or(LOCAL_Z_LAYER_STRIDE);
    let next = cache
        .layout_order_labels
        .range(key..)
        .next()
        .map(|(_, label)| *label)
        .unwrap_or(LOCAL_Z_LAYER_STRIDE * 2);
    if i64::from(next) - i64::from(previous) <= 1 {
        return false;
    }
    let label = previous + (next - previous) / 2;
    cache.layout_entity_order_keys.insert(entity, key);
    cache.layout_order_labels.insert(key, label);
    true
}

fn update_layout_order_labels_for_entities(
    cache: &mut RenderCache,
    entities: impl IntoIterator<Item = Entity>,
) -> bool {
    let entities: HashSet<_> = entities
        .into_iter()
        .filter(|entity| cache.layout_order.contains_key(entity))
        .collect();
    let mut keys = Vec::with_capacity(entities.len());
    for entity in entities {
        let key = LayoutOrderKey {
            depth: hierarchy_depth(entity, &cache.parents_by_entity),
            entity_bits: entity.to_bits(),
        };
        if cache.layout_entity_order_keys.get(&entity) == Some(&key) {
            continue;
        }
        if let Some(previous) = cache.layout_entity_order_keys.remove(&entity) {
            cache.layout_order_labels.remove(&previous);
        }
        keys.push((entity, key));
    }
    keys.sort_unstable_by_key(|(_, key)| *key);
    for (entity, key) in keys {
        if !insert_layout_order_label(cache, entity, key) {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
struct DesiredItem {
    key: RenderKey,
    z: i32,
    target_camera: Option<Entity>,
    hierarchy_depth: u32,
    fragment_order: Option<u64>,
}

struct CurrentBezierEntry {
    desired: DesiredItem,
    item: Option<DrawItem>,
    order: RenderOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct WidgetOrderKey {
    order: u64,
    entity_bits: u64,
    item_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct LayoutOrderKey {
    depth: u32,
    entity_bits: u64,
}

fn hierarchy_depth(entity: Entity, parents: &HashMap<Entity, Entity>) -> u32 {
    let mut current = entity;
    let mut depth = 0u32;
    // The bound also makes this safe if a malformed hierarchy somehow contains a cycle.
    for _ in 0..=parents.len() {
        let Some(parent) = parents.get(&current).copied() else {
            break;
        };
        depth = depth.saturating_add(1);
        current = parent;
    }
    depth
}

fn display_list_item(display_list: &DisplayList, key: RenderKey) -> Option<&DrawItem> {
    match key {
        RenderKey::ManualDisplayListIndex(index) => display_list.manual_items.get(index),
        RenderKey::WidgetDisplayListIndex(index) => display_list.widget_items.get(index),
        RenderKey::OverlayWidgetDisplayListIndex(index) => {
            display_list.overlay_widget_items.get(index)
        }
        RenderKey::CameraDisplayListIndex(camera, index) => {
            display_list.camera_items.get(&camera)?.get(index)
        }
        RenderKey::OverlayCameraDisplayListIndex(camera, index) => {
            display_list.overlay_camera_items.get(&camera)?.get(index)
        }
        RenderKey::WidgetFragment(entity, index) => {
            display_list.widget_fragments.get(&entity)?.items.get(index)
        }
        RenderKey::LayoutEntity(_) => None,
    }
}

fn order_for_desired(desired: &DesiredItem) -> RenderOrder {
    desired.key.stable_order(
        desired.fragment_order,
        desired.hierarchy_depth,
        desired.target_camera,
    )
}

fn update_cached_parent(cache: &mut RenderCache, entity: Entity, parent: Option<Entity>) {
    if let Some(previous) = cache.parents_by_entity.remove(&entity)
        && let Some(children) = cache.children_by_parent.get_mut(&previous)
    {
        children.retain(|child| *child != entity);
        if children.is_empty() {
            cache.children_by_parent.remove(&previous);
        }
    }
    if let Some(parent) = parent {
        cache.parents_by_entity.insert(entity, parent);
        let children = cache.children_by_parent.entry(parent).or_default();
        if !children.contains(&entity) {
            children.push(entity);
        }
    }
}

fn rebuild_cached_hierarchy(
    cache: &mut RenderCache,
    hierarchy: &Query<(Entity, Option<&UiParent>)>,
) {
    cache.parents_by_entity.clear();
    cache.children_by_parent.clear();
    for (entity, parent) in hierarchy.iter() {
        if let Some(parent) = parent {
            cache.parents_by_entity.insert(entity, parent.0);
            cache
                .children_by_parent
                .entry(parent.0)
                .or_default()
                .push(entity);
        }
    }
}

fn update_layout_window(cache: &mut RenderCache, entity: Entity, window: Option<Entity>) {
    if let Some(previous) = cache.layout_window_by_entity.remove(&entity)
        && let Some(entities) = cache.layout_entities_by_window.get_mut(&previous)
    {
        entities.remove(&entity);
        if entities.is_empty() {
            cache.layout_entities_by_window.remove(&previous);
        }
    }
    if let Some(window) = window {
        cache.layout_window_by_entity.insert(entity, window);
        cache
            .layout_entities_by_window
            .entry(window)
            .or_default()
            .insert(entity);
    }
}

fn update_widget_fragment_camera(cache: &mut RenderCache, entity: Entity, camera: Option<Entity>) {
    if let Some(previous) = cache.widget_fragment_camera.remove(&entity)
        && let Some(entities) = cache.widget_fragments_by_camera.get_mut(&previous)
    {
        entities.remove(&entity);
        if entities.is_empty() {
            cache.widget_fragments_by_camera.remove(&previous);
        }
    }
    if let Some(camera) = camera {
        cache.widget_fragment_camera.insert(entity, camera);
        cache
            .widget_fragments_by_camera
            .entry(camera)
            .or_default()
            .insert(entity);
    }
}

fn cached_subtree(cache: &RenderCache, root: Entity) -> Vec<Entity> {
    let mut stack = vec![root];
    let mut visited = HashSet::new();
    let mut subtree = Vec::new();
    while let Some(entity) = stack.pop() {
        if !visited.insert(entity) {
            continue;
        }
        subtree.push(entity);
        if let Some(children) = cache.children_by_parent.get(&entity) {
            stack.extend(children.iter().copied());
        }
    }
    subtree
}

#[derive(Default)]
enum RenderedNodes {
    #[default]
    Empty,
    Single(Entity),
    Multiple(Vec<Entity>),
}

impl RenderedNodes {
    fn push(&mut self, entity: Entity) {
        match self {
            Self::Empty => *self = Self::Single(entity),
            Self::Single(first) => {
                *self = Self::Multiple(vec![*first, entity]);
            }
            Self::Multiple(entities) => entities.push(entity),
        }
    }

    fn first(&self) -> Option<Entity> {
        match self {
            Self::Empty => None,
            Self::Single(entity) => Some(*entity),
            Self::Multiple(entities) => entities.first().copied(),
        }
    }

    fn for_each(&self, mut f: impl FnMut(Entity)) {
        match self {
            Self::Empty => {}
            Self::Single(entity) => f(*entity),
            Self::Multiple(entities) => entities.iter().copied().for_each(f),
        }
    }

    fn despawn(self, commands: &mut Commands) {
        match self {
            Self::Empty => {}
            Self::Single(entity) => {
                commands.entity(entity).despawn();
            }
            Self::Multiple(entities) => {
                for entity in entities {
                    commands.entity(entity).despawn();
                }
            }
        }
    }

    fn set_local_z(&self, commands: &mut Commands, local_z: i32) {
        self.for_each(|entity| {
            commands.entity(entity).insert(ZIndex(local_z));
        });
    }
}

#[derive(Clone, Copy)]
struct RenderNodeOrder {
    global_z: i32,
    local_z: i32,
    mark_item: bool,
}

struct PrimitiveRenderContext<'a> {
    text_line_height: f32,
    default_text_font: TextFont,
    hsv_material: Option<&'a Handle<HsvWheelMaterial>>,
    bezier_material: Option<&'a Handle<BezierMaterial>>,
    local_z: i32,
}

struct CachedItem {
    item: DrawItem,
    nodes: RenderedNodes,
    local_z: i32,
    clip_group: Option<ClipGroupKey>,
    target_camera: Option<Entity>,
    text_font_size: Option<f32>,
    text_line_height: Option<f32>,
    image: Option<Handle<Image>>,
    image_asset_id: Option<u64>,
    hsv_wheel_values: Option<(f32, f32, f32)>,
    hsv_material: Option<Handle<HsvWheelMaterial>>,
    bezier_values: Option<BezierMaterialValues>,
    bezier_material: Option<Handle<BezierMaterial>>,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BezierMaterialValues {
    rect: ClipRect,
    points_01: Vec4,
    points_23: Vec4,
    color: Vec4,
    style: Vec4,
}

struct BezierBatchValues {
    rect: ClipRect,
    segments: Vec<f32>,
    curves: Vec<f32>,
    tile_indices: Vec<u32>,
    parameters: Vec4,
}

const BEZIER_BATCH_TILE_SIZE: f32 = 16.0;
const MAX_BEZIER_BATCH_CURVES: usize = 128;
const MAX_BEZIER_BATCH_TILES: usize = 262_144;
const MAX_BEZIER_BATCH_TILE_REFERENCES: usize = 8_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClipGroupKey {
    x_bits: u32,
    y_bits: u32,
    width_bits: u32,
    height_bits: u32,
    target_camera: Option<Entity>,
}

impl ClipGroupKey {
    fn new(clip: ClipRect, target_camera: Option<Entity>) -> Self {
        Self {
            x_bits: clip.x.to_bits(),
            y_bits: clip.y.to_bits(),
            width_bits: clip.width.to_bits(),
            height_bits: clip.height.to_bits(),
            target_camera,
        }
    }

    fn clip(self) -> ClipRect {
        ClipRect {
            x: f32::from_bits(self.x_bits),
            y: f32::from_bits(self.y_bits),
            width: f32::from_bits(self.width_bits),
            height: f32::from_bits(self.height_bits),
        }
    }
}

struct CachedClipGroup {
    entity: Entity,
    generation: u64,
}

#[derive(SystemParam)]
struct RendererSync<'w, 's> {
    commands: Commands<'w, 's>,
    display_list: ResMut<'w, DisplayList>,
    images: ResMut<'w, ImageRegistry>,
    hsv_wheel_materials: Option<ResMut<'w, Assets<HsvWheelMaterial>>>,
    bezier_materials: Option<ResMut<'w, Assets<BezierMaterial>>>,
    bezier_batch_materials: Option<ResMut<'w, Assets<BezierBatchMaterial>>>,
    shader_buffers: Option<ResMut<'w, Assets<ShaderBuffer>>>,
    target_camera: Res<'w, RenderTargetCamera>,
    theme: Option<Res<'w, Theme>>,
    camera_query: bevy::ecs::system::Query<'w, 's, (), bevy::ecs::query::With<Camera>>,
    paint_query: bevy::ecs::system::Query<
        'w,
        's,
        (
            Entity,
            bevy::ecs::change_detection::Ref<'static, UiLayoutResult>,
            bevy::ecs::change_detection::Ref<'static, UiPaint>,
        ),
    >,
    hierarchy_query: bevy::ecs::system::Query<'w, 's, (Entity, Option<&'static UiParent>)>,
    dirty: ResMut<'w, UiDirty>,
    cache: ResMut<'w, RenderCache>,
    stats: ResMut<'w, RenderSyncStats>,
}

mod collect;
use collect::*;

mod sync;
use sync::*;

mod bezier_incremental;
use bezier_incremental::*;

mod fragments;
use fragments::*;

mod full_sync;
use full_sync::*;

mod bezier;
use bezier::*;

mod spawn;
use spawn::*;

#[cfg(test)]
#[path = "render/tests.rs"]
mod tests;

//! Renderer-facing draw data and a Bevy UI GPU backend.
//!
//! [`BuiRenderPlugin`] translates [`DisplayList`] items into Bevy UI nodes. Bevy's UI render
//! pipeline draws those nodes on the GPU; this crate does not rasterize widgets on the CPU.

use std::collections::{HashMap, HashSet};

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
        App, BackgroundColor, BorderRadius, Color, Commands, Component, Entity, GlobalZIndex,
        Image, ImageNode, Node, Overflow, Plugin, PositionType, Res, ResMut, Resource, Text,
        Update, Val, ZIndex,
    },
    text::{
        Justify, LetterSpacing, LineBreak, LineHeight, TextBounds, TextColor, TextFont, TextLayout,
    },
    ui::{BoxShadow, UiTargetCamera, UiTransform},
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Optional per-control text metrics. Unset font and line height inherit the active theme.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRenderStyle {
    pub font: Option<TextFont>,
    pub line_height: Option<LineHeight>,
    pub letter_spacing: LetterSpacing,
    pub justify: Justify,
    pub line_break: LineBreak,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    RoundedRect {
        rect: ClipRect,
        radius: f32,
        color: Rgba,
    },
    /// A GPU-rendered filled ellipse with the supplied bounds and sRGB color.
    Ellipse {
        rect: ClipRect,
        color: Rgba,
    },
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: Rgba,
    },
    Text {
        origin: [f32; 2],
        text: String,
        color: Rgba,
    },
    /// Text with an explicit shaping width and wrapping rule, used by editable controls.
    TextWrapped {
        origin: [f32; 2],
        text: String,
        color: Rgba,
        max_width: f32,
        wrap: bool,
        style: Option<TextRenderStyle>,
    },
    Image {
        rect: ClipRect,
        asset_id: u64,
    },
    /// A rounded fill with a native Bevy UI box shadow.
    Shadow {
        rect: ClipRect,
        radius: f32,
        color: Rgba,
        shadow: Rgba,
        offset: [f32; 2],
        blur: f32,
        spread: f32,
    },
    /// A tintable registered image for an icon or small UI symbol.
    Icon {
        rect: ClipRect,
        asset_id: u64,
        tint: Rgba,
    },
    /// A cubic Bezier path, drawn by one shader-backed Bevy UI node.
    Bezier {
        points: [[f32; 2]; 4],
        width: f32,
        color: Rgba,
    },
    /// GPU-shaded hue ring and saturation/value disk with hue and saturation selectors.
    HsvWheel {
        rect: ClipRect,
        hue_degrees: f32,
        saturation: f32,
        value: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawItem {
    pub clip: ClipRect,
    pub z: i32,
    pub primitive: Primitive,
}

/// A paint description attached to a BUI entity. `UiLayoutResult` supplies its final position,
/// clipping region, and z order in the renderer phase.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiPaint(pub Primitive);

impl UiPaint {
    pub fn draw_item(&self, layout: &UiLayoutResult) -> DrawItem {
        let rect = ClipRect::from(layout.rect);
        let clip = layout
            .clip
            .and_then(|clip| clip.intersection(layout.rect))
            .map(ClipRect::from)
            .unwrap_or(rect);
        let primitive = match &self.0 {
            Primitive::RoundedRect { radius, color, .. } => Primitive::RoundedRect {
                rect,
                radius: *radius,
                color: *color,
            },
            Primitive::Ellipse { color, .. } => Primitive::Ellipse {
                rect,
                color: *color,
            },
            Primitive::Line {
                from,
                to,
                width,
                color,
            } => Primitive::Line {
                from: *from,
                to: *to,
                width: *width,
                color: *color,
            },
            Primitive::Text { text, color, .. } => Primitive::Text {
                origin: [layout.rect.x, layout.rect.y],
                text: text.clone(),
                color: *color,
            },
            Primitive::TextWrapped {
                text,
                color,
                max_width,
                wrap,
                style,
                ..
            } => Primitive::TextWrapped {
                origin: [layout.rect.x, layout.rect.y],
                text: text.clone(),
                color: *color,
                max_width: *max_width,
                wrap: *wrap,
                style: style.clone(),
            },
            Primitive::Image { asset_id, .. } => Primitive::Image {
                rect,
                asset_id: *asset_id,
            },
            Primitive::Shadow {
                radius,
                color,
                shadow,
                offset,
                blur,
                spread,
                ..
            } => Primitive::Shadow {
                rect,
                radius: *radius,
                color: *color,
                shadow: *shadow,
                offset: *offset,
                blur: *blur,
                spread: *spread,
            },
            Primitive::Icon { asset_id, tint, .. } => Primitive::Icon {
                rect,
                asset_id: *asset_id,
                tint: *tint,
            },
            Primitive::Bezier {
                points,
                width,
                color,
            } => Primitive::Bezier {
                points: *points,
                width: *width,
                color: *color,
            },
            Primitive::HsvWheel {
                hue_degrees,
                saturation,
                value,
                ..
            } => Primitive::HsvWheel {
                rect,
                hue_degrees: *hue_degrees,
                saturation: *saturation,
                value: *value,
            },
        };
        DrawItem {
            clip,
            z: layout.z,
            primitive,
        }
    }
}

impl From<Rect> for ClipRect {
    fn from(rect: Rect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[derive(Default, Debug, Resource)]
pub struct DisplayList {
    manual_items: Vec<DrawItem>,
    widget_items: Vec<DrawItem>,
    overlay_widget_items: Vec<DrawItem>,
    camera_items: HashMap<Entity, Vec<DrawItem>>,
    overlay_camera_items: HashMap<Entity, Vec<DrawItem>>,
    camera_order: Vec<Entity>,
    items: Vec<DrawItem>,
}

impl DisplayList {
    pub fn push(&mut self, item: DrawItem) {
        self.manual_items.push(item);
        self.rebuild_merged_items();
    }

    /// Replace the widget-generated layer while preserving manually pushed draw items.
    pub fn replace_widget_items(&mut self, items: Vec<DrawItem>) {
        if self.widget_items != items {
            self.widget_items = items;
            self.rebuild_merged_items();
        }
    }

    /// Replace a post-widget overlay for the screen UI layer without overwriting base or manual
    /// items. Overlay items sort after the base widget layer at equal z.
    pub fn replace_overlay_widget_items(&mut self, items: Vec<DrawItem>) {
        if self.overlay_widget_items != items {
            self.overlay_widget_items = items;
            self.rebuild_merged_items();
        }
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
                if items.is_empty() {
                    self.camera_items.remove(&camera);
                } else if self.camera_items.get(&camera) != Some(&items) {
                    self.camera_items.insert(camera, items);
                }
                self.rebuild_merged_items();
            }
        }
    }

    /// Replace an auxiliary layer for one camera without overwriting its widget-generated items.
    ///
    /// This is intended for systems that add renderer-backed content after the main widget paint
    /// pass, such as thumbnails owned by a virtualized browser. Its items join the same stable z
    /// ordering as camera items, after the base camera layer at equal z.
    pub fn replace_overlay_camera_items(&mut self, camera: Entity, items: Vec<DrawItem>) {
        if items.is_empty() {
            self.overlay_camera_items.remove(&camera);
        } else if self.overlay_camera_items.get(&camera) != Some(&items) {
            self.overlay_camera_items.insert(camera, items);
        }
        self.rebuild_merged_items();
    }

    /// Read the widget-generated layer before z-order merging.
    pub fn widget_items(&self) -> &[DrawItem] {
        &self.widget_items
    }

    /// Read the screen UI overlay layer before z-order merging.
    pub fn overlay_widget_items(&self) -> &[DrawItem] {
        &self.overlay_widget_items
    }

    pub fn camera_items(&self, camera: Entity) -> Option<&[DrawItem]> {
        self.camera_items.get(&camera).map(Vec::as_slice)
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
        self.items.sort_by_key(|item| item.z);
        &self.items
    }

    /// Read the draw items in ascending z order.
    pub fn items(&self) -> &[DrawItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.manual_items.clear();
        self.widget_items.clear();
        self.overlay_widget_items.clear();
        self.camera_items.clear();
        self.overlay_camera_items.clear();
        self.camera_order.clear();
        self.items.clear();
    }

    fn rebuild_merged_items(&mut self) {
        self.items.clear();
        self.items.extend(self.manual_items.iter().cloned());
        self.items.extend(self.widget_items.iter().cloned());
        self.items.extend(self.overlay_widget_items.iter().cloned());
        self.camera_order.clear();
        self.camera_order.extend(self.camera_items.keys().copied());
        self.camera_order
            .extend(self.overlay_camera_items.keys().copied());
        self.camera_order
            .sort_unstable_by_key(|camera| camera.to_bits());
        self.camera_order.dedup();
        for camera in &self.camera_order {
            if let Some(items) = self.camera_items.get(camera) {
                self.items.extend(items.iter().cloned());
            }
            if let Some(items) = self.overlay_camera_items.get(camera) {
                self.items.extend(items.iter().cloned());
            }
        }
        self.items.sort_by_key(|item| item.z);
    }
}

/// Associates numeric image IDs in [`Primitive::Image`] with Bevy image assets.
#[derive(Resource)]
pub struct ImageRegistry {
    images: HashMap<u64, Handle<Image>>,
    next_id: u64,
}

impl Default for ImageRegistry {
    fn default() -> Self {
        Self {
            images: HashMap::new(),
            next_id: 1,
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
        self.images.insert(asset_id, image)
    }

    pub fn remove(&mut self, asset_id: u64) -> Option<Handle<Image>> {
        self.images.remove(&asset_id)
    }

    pub fn get(&self, asset_id: u64) -> Option<&Handle<Image>> {
        self.images.get(&asset_id)
    }
}

/// Routes generated UI to a specific camera.
///
/// Leave the camera unset to use Bevy's primary UI camera. To make a world-space BUI surface,
/// target a camera whose render target is an [`Image`], then display that
/// image on a Bevy 3D mesh. Camera and render-target creation remain under the host application's
/// control.
#[derive(Default, Resource)]
pub struct RenderTargetCamera(pub Option<Entity>);

/// Describes the mapping between a world-space mesh surface and its offscreen BUI camera.
///
/// Attach this component to a mesh whose local plane lies on XY at `z = 0`, centered at the
/// entity origin. The renderer does not create the camera, render-target image, or mesh. Set
/// [`RenderTargetCamera`] to `target_camera`, render it to an image, and use that image as the
/// mesh texture. Register the same texture handle under `texture_asset_id` when it should also
/// appear in a BUI [`Primitive::Image`].
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiWorldSurface {
    pub target_camera: Entity,
    pub texture_asset_id: u64,
    /// Full width and height of the plane in local world units.
    pub world_size: Vec2,
    /// Render-target extent in pixels.
    pub resolution: UVec2,
}

impl UiWorldSurface {
    /// Map a point in the surface's local XY plane to top-left-origin UI pixels.
    ///
    /// The left and top edges are included; the right and bottom edges are excluded, matching
    /// the half-open rectangle semantics used by BUI hit testing.
    pub fn local_to_ui(self, local_xy: Vec2) -> Option<[f32; 2]> {
        let [width, height] = [self.world_size.x, self.world_size.y];
        if !local_xy.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || self.resolution.x == 0
            || self.resolution.y == 0
        {
            return None;
        }

        let half_width = width * 0.5;
        let half_height = height * 0.5;
        if local_xy.x < -half_width
            || local_xy.x >= half_width
            || local_xy.y > half_height
            || local_xy.y <= -half_height
        {
            return None;
        }

        Some([
            (local_xy.x + half_width) / width * self.resolution.x as f32,
            (half_height - local_xy.y) / height * self.resolution.y as f32,
        ])
    }
}

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
            bevy::asset::embedded_asset!(app, "bezier_batch.wesl");
            app.add_plugins(UiMaterialPlugin::<HsvWheelMaterial>::default());
            app.add_plugins(UiMaterialPlugin::<BezierMaterial>::default());
            app.add_plugins(UiMaterialPlugin::<BezierBatchMaterial>::default());
        }
        app.init_resource::<DisplayList>()
            .init_resource::<UiDirty>()
            .init_resource::<ImageRegistry>()
            .init_resource::<RenderTargetCamera>()
            .init_resource::<RenderCache>()
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
    bezier_batches: HashMap<BezierBatchKey, CachedBezierBatch>,
    desired: Vec<DesiredItem>,
    generation: u64,
    target_camera: Option<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BezierBatchKey {
    first: RenderKey,
    last: RenderKey,
    clip: ClipGroupKey,
}

struct CachedBezierBatch {
    entity: Entity,
    material: Handle<BezierBatchMaterial>,
    segments: Handle<ShaderBuffer>,
    curves: Handle<ShaderBuffer>,
    tile_indices: Handle<ShaderBuffer>,
    signature: Vec<DrawItem>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RenderKey {
    ManualDisplayListIndex(usize),
    WidgetDisplayListIndex(usize),
    OverlayWidgetDisplayListIndex(usize),
    CameraDisplayListIndex(Entity, usize),
    OverlayCameraDisplayListIndex(Entity, usize),
    LayoutEntity(Entity),
}

impl RenderKey {
    fn stable_order(self) -> (u8, u64, u64) {
        match self {
            Self::ManualDisplayListIndex(index) => (0, index as u64, 0),
            // Entity-attached UI paints include root backgrounds. At equal z they must be
            // created before display-list widget content, otherwise a root-sized background can
            // cover its same-z labels and controls.
            Self::LayoutEntity(entity) => (1, entity.to_bits(), 0),
            Self::WidgetDisplayListIndex(index) => (2, index as u64, 0),
            Self::OverlayWidgetDisplayListIndex(index) => (3, index as u64, 0),
            Self::CameraDisplayListIndex(camera, index) => (4, camera.to_bits(), index as u64),
            Self::OverlayCameraDisplayListIndex(camera, index) => {
                (5, camera.to_bits(), index as u64)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct DesiredItem {
    key: RenderKey,
    z: i32,
    target_camera: Option<Entity>,
    hierarchy_depth: u32,
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
    display_list: Res<'w, DisplayList>,
    images: Res<'w, ImageRegistry>,
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
}

#[expect(
    clippy::type_complexity,
    reason = "Bevy change query covers render inputs"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects each tracked resource separately"
)]
fn collect_render_changes(
    display_list: Res<DisplayList>,
    images: Res<ImageRegistry>,
    target_camera: Res<RenderTargetCamera>,
    theme: Option<Res<Theme>>,
    changed_paints: Query<
        Entity,
        Or<(
            Changed<UiLayoutResult>,
            Changed<UiPaint>,
            Changed<UiParent>,
            Changed<Camera>,
        )>,
    >,
    mut removed_paints: RemovedComponents<UiPaint>,
    mut removed_layouts: RemovedComponents<UiLayoutResult>,
    mut removed_parents: RemovedComponents<UiParent>,
    mut removed_cameras: RemovedComponents<Camera>,
    mut dirty: ResMut<UiDirty>,
) {
    if display_list.is_changed()
        || images.is_changed()
        || target_camera.is_changed()
        || theme.as_ref().is_some_and(DetectChanges::is_changed)
        || changed_paints.iter().next().is_some()
        || removed_paints.read().next().is_some()
        || removed_layouts.read().next().is_some()
        || removed_parents.read().next().is_some()
        || removed_cameras.read().next().is_some()
    {
        dirty.mark_all(UiDirtyDomain::Render);
    }
}

fn sync_display_list(sync: RendererSync) {
    let RendererSync {
        mut commands,
        display_list,
        images,
        mut hsv_wheel_materials,
        mut bezier_materials,
        mut bezier_batch_materials,
        mut shader_buffers,
        target_camera,
        theme,
        camera_query,
        paint_query,
        hierarchy_query,
        mut dirty,
        mut cache,
    } = sync;
    let changes = dirty.take(UiDirtyDomain::Render);
    if changes.is_empty() {
        return;
    }
    let images_changed = images.is_changed();
    let text_font_size = theme
        .as_ref()
        .map(|theme| theme.font_size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .unwrap_or(14.0);
    let text_line_height = theme
        .as_ref()
        .map(|theme| theme.line_height)
        .filter(|height| height.is_finite() && *height > 0.0)
        .unwrap_or(1.25);
    let default_text_font = ui_default_text_font(text_font_size);
    let mut desired = std::mem::take(&mut cache.desired);
    desired.clear();
    desired.reserve(display_list.items.len());
    let hierarchy_parents: HashMap<_, _> = hierarchy_query
        .iter()
        .filter_map(|(entity, parent)| parent.map(|parent| (entity, parent.0)))
        .collect();
    for (index, item) in display_list.manual_items.iter().enumerate() {
        desired.push(DesiredItem {
            key: RenderKey::ManualDisplayListIndex(index),
            z: item.z,
            target_camera: target_camera.0,
            hierarchy_depth: 0,
        });
    }
    for (index, item) in display_list.widget_items.iter().enumerate() {
        desired.push(DesiredItem {
            key: RenderKey::WidgetDisplayListIndex(index),
            z: item.z,
            target_camera: None,
            hierarchy_depth: 0,
        });
    }
    for (index, item) in display_list.overlay_widget_items.iter().enumerate() {
        desired.push(DesiredItem {
            key: RenderKey::OverlayWidgetDisplayListIndex(index),
            z: item.z,
            target_camera: None,
            hierarchy_depth: 0,
        });
    }
    for (camera, items) in &display_list.camera_items {
        for (index, item) in items.iter().enumerate() {
            desired.push(DesiredItem {
                key: RenderKey::CameraDisplayListIndex(*camera, index),
                z: item.z,
                target_camera: Some(*camera),
                hierarchy_depth: 0,
            });
        }
    }
    for (camera, items) in &display_list.overlay_camera_items {
        for (index, item) in items.iter().enumerate() {
            desired.push(DesiredItem {
                key: RenderKey::OverlayCameraDisplayListIndex(*camera, index),
                z: item.z,
                target_camera: Some(*camera),
                hierarchy_depth: 0,
            });
        }
    }
    for (entity, layout, _) in &paint_query {
        let camera = camera_query.get(layout.window).ok().map(|_| layout.window);
        desired.push(DesiredItem {
            key: RenderKey::LayoutEntity(entity),
            z: layout.z,
            target_camera: camera,
            hierarchy_depth: hierarchy_depth(entity, &hierarchy_parents),
        });
    }
    desired.sort_unstable_by(|first, second| {
        first.z.cmp(&second.z).then_with(|| {
            let first_order = first.key.stable_order();
            let second_order = second.key.stable_order();
            if first_order.0 == 1 && second_order.0 == 1 {
                first
                    .hierarchy_depth
                    .cmp(&second.hierarchy_depth)
                    .then_with(|| first_order.cmp(&second_order))
            } else {
                first_order.cmp(&second_order)
            }
        })
    });
    cache.generation = cache.generation.wrapping_add(1);
    let generation = cache.generation;
    let mut retired_hsv_materials = Vec::new();
    let mut retired_bezier_materials = Vec::new();
    let mut retired_bezier_batches = Vec::new();
    let resolved_items: Vec<Option<DrawItem>> = desired
        .iter()
        .map(|desired_item| match desired_item.key {
            RenderKey::ManualDisplayListIndex(index) => {
                display_list.manual_items.get(index).cloned()
            }
            RenderKey::WidgetDisplayListIndex(index) => {
                display_list.widget_items.get(index).cloned()
            }
            RenderKey::OverlayWidgetDisplayListIndex(index) => {
                display_list.overlay_widget_items.get(index).cloned()
            }
            RenderKey::CameraDisplayListIndex(camera, index) => display_list
                .camera_items
                .get(&camera)
                .and_then(|items| items.get(index))
                .cloned(),
            RenderKey::OverlayCameraDisplayListIndex(camera, index) => display_list
                .overlay_camera_items
                .get(&camera)
                .and_then(|items| items.get(index))
                .cloned(),
            RenderKey::LayoutEntity(entity) => paint_query
                .get(entity)
                .ok()
                .map(|(_, layout, paint)| paint.draw_item(&layout)),
        })
        .collect();
    let mut batched_keys = HashSet::new();
    if let (Some(materials), Some(buffers)) =
        (bezier_batch_materials.as_mut(), shader_buffers.as_mut())
    {
        let mut start = 0;
        while start < desired.len() {
            let Some(item) = resolved_items[start].as_ref() else {
                start += 1;
                continue;
            };
            let Some(clip) = valid_clip(item.clip)
                .then(|| ClipGroupKey::new(item.clip, desired[start].target_camera))
            else {
                start += 1;
                continue;
            };
            if !matches!(&item.primitive, Primitive::Bezier { .. }) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < desired.len() {
                let Some(next_item) = resolved_items[end].as_ref() else {
                    break;
                };
                if desired[end].target_camera != desired[start].target_camera
                    || !matches!(&next_item.primitive, Primitive::Bezier { .. })
                    || !valid_clip(next_item.clip)
                    || ClipGroupKey::new(next_item.clip, desired[end].target_camera) != clip
                {
                    break;
                }
                end += 1;
            }
            if end - start >= 2 {
                let key = BezierBatchKey {
                    first: desired[start].key,
                    last: desired[end - 1].key,
                    clip,
                };
                let signature_unchanged = cache.bezier_batches.get(&key).is_some_and(|cached| {
                    cached.signature.len() == end - start
                        && resolved_items[start..end]
                            .iter()
                            .zip(&cached.signature)
                            .all(|(item, previous)| item.as_ref() == Some(previous))
                });
                let local_z = i32::try_from(start).unwrap_or(i32::MAX);
                if signature_unchanged {
                    if let Some(cached) = cache.bezier_batches.get_mut(&key) {
                        if cached.local_z != local_z {
                            commands.entity(cached.entity).insert(ZIndex(local_z));
                            cached.local_z = local_z;
                        }
                        cached.generation = generation;
                    }
                    if let Some(group) = cache.clip_groups.get_mut(&clip) {
                        group.generation = generation;
                    }
                    batched_keys.extend(desired[start..end].iter().map(|item| item.key));
                } else {
                    let signature: Vec<_> = resolved_items[start..end]
                        .iter()
                        .filter_map(Clone::clone)
                        .collect();
                    if let Some(values) = build_bezier_batch_values(&signature, clip.clip()) {
                        BezierBatchSyncContext {
                            commands: &mut commands,
                            cache: &mut cache,
                            materials,
                            buffers,
                            generation,
                        }
                        .sync(
                            key,
                            signature,
                            values,
                            desired[start].z,
                            local_z,
                        );
                        batched_keys.extend(desired[start..end].iter().map(|item| item.key));
                    }
                }
                start = end;
            } else {
                start += 1;
            }
        }
    }
    for (order, desired_item) in desired.drain(..).enumerate() {
        let DesiredItem {
            key,
            target_camera: target,
            z: _,
            hierarchy_depth: _,
        } = desired_item;
        if batched_keys.contains(&key) {
            continue;
        }
        let Some(item) = resolved_items[order].as_ref() else {
            continue;
        };
        let local_z = i32::try_from(order).unwrap_or(i32::MAX);
        let clip_group = valid_clip(item.clip).then(|| ClipGroupKey::new(item.clip, target));
        let clip_group_entity = if let Some(group_key) = clip_group {
            if let Some(group) = cache.clip_groups.get_mut(&group_key) {
                group.generation = generation;
                Some(group.entity)
            } else {
                let entity = spawn_clip_group(&mut commands, group_key);
                cache
                    .clip_groups
                    .insert(group_key, CachedClipGroup { entity, generation });
                Some(entity)
            }
        } else {
            None
        };

        let resolved_image = image_handle(&images, &item.primitive).cloned();
        let wheel_values = match (&item.primitive, clip_group_entity) {
            (Primitive::HsvWheel { rect, .. }, Some(_)) if valid_clip(*rect) => {
                hsv_wheel_parameters(&item.primitive)
            }
            _ => None,
        };
        let bezier_values = match (&item.primitive, clip_group_entity) {
            (Primitive::Bezier { .. }, Some(_)) => bezier_material_values(item),
            _ => None,
        };
        let item_font_size = uses_theme_font_size(&item.primitive).then_some(text_font_size);
        let item_line_height = uses_theme_line_height(&item.primitive).then_some(text_line_height);
        let image_changed = images_changed
            && cache
                .items
                .get(&key)
                .is_some_and(|cached| cached.image != resolved_image);
        let unchanged = !image_changed
            && cache.items.get(&key).is_some_and(|cached| {
                cached.item == *item
                    && cached.target_camera == target
                    && cached.clip_group == clip_group
                    && cached.text_font_size == item_font_size
                    && cached.text_line_height == item_line_height
                    && cached.image == resolved_image
                    && cached.hsv_wheel_values == wheel_values
                    && cached.bezier_values == bezier_values
            });
        if unchanged {
            if let Some(cached) = cache.items.get_mut(&key) {
                if cached.local_z != local_z {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                }
                cached.generation = generation;
            }
            continue;
        }

        let reusable_bezier_material = cache.items.get(&key).and_then(|cached| {
            (bezier_values.is_some()
                && cached.target_camera == target
                && cached.clip_group == clip_group
                && cached.image == resolved_image)
                .then(|| cached.bezier_material.clone())
                .flatten()
        });
        let bezier_updated_in_place = reusable_bezier_material
            .as_ref()
            .zip(bezier_values)
            .is_some_and(|(handle, values)| {
                bezier_materials.as_mut().is_some_and(|materials| {
                    materials.get_mut(handle).is_some_and(|mut material| {
                        material.points_01 = values.points_01;
                        material.points_23 = values.points_23;
                        material.color = values.color;
                        material.style = values.style;
                        true
                    })
                })
            });
        if bezier_updated_in_place {
            if let Some(cached) = cache.items.get_mut(&key) {
                cached.item = item.clone();
                cached.bezier_values = bezier_values;
                if let Some(values) = bezier_values
                    && let Some(node) = cached.nodes.first()
                {
                    commands
                        .entity(node)
                        .insert(positioned_node(values.rect, item.clip));
                }
                if cached.local_z != local_z {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                }
                cached.generation = generation;
            }
            continue;
        }

        let reusable_wheel_material = cache.items.get(&key).and_then(|cached| {
            (wheel_values.is_some()
                && same_hsv_wheel_geometry(&cached.item, item)
                && cached.target_camera == target
                && cached.clip_group == clip_group
                && cached.image == resolved_image
                && cached.text_font_size == item_font_size
                && cached.text_line_height == item_line_height)
                .then(|| cached.hsv_material.clone())
                .flatten()
        });
        let wheel_updated_in_place = reusable_wheel_material
            .as_ref()
            .zip(wheel_values)
            .is_some_and(|(handle, (hue_degrees, saturation, value))| {
                hsv_wheel_materials.as_mut().is_some_and(|materials| {
                    materials.get_mut(handle).is_some_and(|mut material| {
                        material.parameters = Vec4::new(hue_degrees, saturation, value, 0.0);
                        true
                    })
                })
            });
        if wheel_updated_in_place {
            if let Some(cached) = cache.items.get_mut(&key) {
                cached.item = item.clone();
                cached.hsv_wheel_values = wheel_values;
                if cached.local_z != local_z {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                }
                cached.generation = generation;
            }
            continue;
        }

        let mut previous_hsv_material = None;
        let mut previous_bezier_material = None;
        if let Some(previous) = cache.items.remove(&key) {
            previous_hsv_material = previous.hsv_material.clone();
            previous_bezier_material = previous.bezier_material.clone();
            previous.nodes.despawn(&mut commands);
        }

        let mut nodes = RenderedNodes::default();
        let hsv_material = wheel_values.and_then(|(hue_degrees, saturation, value)| {
            let parameters = Vec4::new(hue_degrees, saturation, value, 0.0);
            if let Some(handle) = previous_hsv_material.as_ref() {
                let materials = hsv_wheel_materials.as_mut()?;
                materials.get_mut(handle)?.parameters = parameters;
                Some(handle.clone())
            } else {
                hsv_wheel_materials
                    .as_mut()
                    .map(|materials| materials.add(HsvWheelMaterial { parameters }))
            }
        });
        if let Some(handle) = previous_hsv_material.filter(|_| hsv_material.is_none()) {
            retired_hsv_materials.push(handle);
        }
        let bezier_material = bezier_values.and_then(|values| {
            bezier_materials.as_mut().map(|materials| {
                materials.add(BezierMaterial {
                    points_01: values.points_01,
                    points_23: values.points_23,
                    color: values.color,
                    style: values.style,
                })
            })
        });
        if let Some(handle) = previous_bezier_material {
            retired_bezier_materials.push(handle);
        }
        if let Some(group) = clip_group_entity {
            commands.entity(group).with_children(|parent| {
                spawn_primitive(
                    parent,
                    item,
                    images.as_ref(),
                    PrimitiveRenderContext {
                        text_line_height,
                        default_text_font: default_text_font.clone(),
                        hsv_material: hsv_material.as_ref(),
                        bezier_material: bezier_material.as_ref(),
                        local_z,
                    },
                    &mut nodes,
                );
            });
        }
        cache.items.insert(
            key,
            CachedItem {
                item: item.clone(),
                nodes,
                local_z,
                clip_group,
                target_camera: target,
                text_font_size: item_font_size,
                text_line_height: item_line_height,
                image: resolved_image,
                hsv_wheel_values: wheel_values,
                hsv_material,
                bezier_values,
                bezier_material,
                generation,
            },
        );
    }

    cache.items.retain(|_, cached| {
        if cached.generation == generation {
            return true;
        }
        cached.nodes.for_each(|node| {
            commands.entity(node).despawn();
        });
        if let Some(material) = &cached.hsv_material {
            retired_hsv_materials.push(material.clone());
        }
        if let Some(material) = &cached.bezier_material {
            retired_bezier_materials.push(material.clone());
        }
        false
    });
    cache.bezier_batches.retain(|_, batch| {
        if batch.generation == generation {
            return true;
        }
        commands.entity(batch.entity).despawn();
        retired_bezier_batches.push((
            batch.material.clone(),
            batch.segments.clone(),
            batch.curves.clone(),
            batch.tile_indices.clone(),
        ));
        false
    });
    cache.clip_groups.retain(|_, group| {
        if group.generation == generation {
            return true;
        }
        commands.entity(group.entity).despawn();
        false
    });
    if let Some(materials) = hsv_wheel_materials.as_mut() {
        for handle in retired_hsv_materials {
            materials.remove(handle.id());
        }
    }
    if let Some(materials) = bezier_materials.as_mut() {
        for handle in retired_bezier_materials {
            materials.remove(handle.id());
        }
    }
    if let Some(materials) = bezier_batch_materials.as_mut() {
        for (material, _, _, _) in &retired_bezier_batches {
            materials.remove(material.id());
        }
    }
    if let Some(buffers) = shader_buffers.as_mut() {
        for (_, segments, curves, tile_indices) in retired_bezier_batches {
            buffers.remove(segments.id());
            buffers.remove(curves.id());
            buffers.remove(tile_indices.id());
        }
    }
    cache.desired = desired;
    cache.target_camera = target_camera.0;
}

fn spawn_clip_group(commands: &mut Commands, key: ClipGroupKey) -> Entity {
    let mut group = commands.spawn((RenderedClipGroup, clip_node(key.clip())));
    if let Some(camera) = key.target_camera {
        group.insert(UiTargetCamera(camera));
    }
    group.id()
}

fn image_handle<'a>(images: &'a ImageRegistry, primitive: &Primitive) -> Option<&'a Handle<Image>> {
    match primitive {
        Primitive::Image { asset_id, .. } | Primitive::Icon { asset_id, .. } => {
            images.get(*asset_id)
        }
        _ => None,
    }
}

fn hsv_wheel_parameters(primitive: &Primitive) -> Option<(f32, f32, f32)> {
    let Primitive::HsvWheel {
        hue_degrees,
        saturation,
        value,
        ..
    } = primitive
    else {
        return None;
    };
    if !hue_degrees.is_finite() || !saturation.is_finite() || !value.is_finite() {
        return None;
    }
    Some((
        hue_degrees.rem_euclid(360.0),
        saturation.clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    ))
}

fn bezier_material_values(item: &DrawItem) -> Option<BezierMaterialValues> {
    let Primitive::Bezier {
        points,
        width,
        color,
    } = &item.primitive
    else {
        return None;
    };
    if !points.iter().flatten().all(|value| value.is_finite())
        || !width.is_finite()
        || *width <= 0.0
        || ![color.0, color.1, color.2, color.3]
            .iter()
            .all(|channel| channel.is_finite())
    {
        return None;
    }

    let padding = width * 0.5 + 1.25;
    let min_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min)
        - padding;
    let min_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min)
        - padding;
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max)
        + padding;
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max)
        + padding;
    let rect = ClipRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    if !valid_clip(rect) {
        return None;
    }

    let local = points.map(|point| [point[0] - rect.x, point[1] - rect.y]);
    Some(BezierMaterialValues {
        rect,
        points_01: Vec4::new(local[0][0], local[0][1], local[1][0], local[1][1]),
        points_23: Vec4::new(local[2][0], local[2][1], local[3][0], local[3][1]),
        color: Vec4::new(
            color.0.clamp(0.0, 1.0),
            color.1.clamp(0.0, 1.0),
            color.2.clamp(0.0, 1.0),
            color.3.clamp(0.0, 1.0),
        ),
        style: Vec4::new(*width, 0.0, 0.0, 0.0),
    })
}

fn intersect_clip(first: ClipRect, second: ClipRect) -> Option<ClipRect> {
    let left = first.x.max(second.x);
    let top = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    let rect = ClipRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    valid_clip(rect).then_some(rect)
}

fn cubic_bezier_point(points: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let inverse = 1.0 - t;
    let weights = [
        inverse * inverse * inverse,
        3.0 * inverse * inverse * t,
        3.0 * inverse * t * t,
        t * t * t,
    ];
    [
        points
            .iter()
            .zip(weights)
            .map(|(point, weight)| point[0] * weight)
            .sum(),
        points
            .iter()
            .zip(weights)
            .map(|(point, weight)| point[1] * weight)
            .sum(),
    ]
}

fn build_bezier_batch_values(items: &[DrawItem], clip: ClipRect) -> Option<BezierBatchValues> {
    let mut bounded_curves = Vec::with_capacity(items.len());
    let mut bounds: Option<ClipRect> = None;
    for item in items {
        let values = bezier_material_values(item)?;
        let Some(clipped) = intersect_clip(values.rect, clip) else {
            continue;
        };
        let next = if let Some(bounds) = bounds {
            let right = (bounds.x + bounds.width).max(clipped.x + clipped.width);
            let bottom = (bounds.y + bounds.height).max(clipped.y + clipped.height);
            ClipRect {
                x: bounds.x.min(clipped.x),
                y: bounds.y.min(clipped.y),
                width: right - bounds.x.min(clipped.x),
                height: bottom - bounds.y.min(clipped.y),
            }
        } else {
            clipped
        };
        bounds = Some(next);
        bounded_curves.push((item, values));
    }
    let rect = bounds?;
    if !valid_clip(rect) {
        return None;
    }

    let columns = (rect.width / BEZIER_BATCH_TILE_SIZE).ceil().max(1.0) as usize;
    let rows = (rect.height / BEZIER_BATCH_TILE_SIZE).ceil().max(1.0) as usize;
    let tile_count = columns.checked_mul(rows)?;
    if tile_count > MAX_BEZIER_BATCH_TILES {
        return None;
    }
    let mut tile_segments: Vec<Vec<u32>> = (0..tile_count).map(|_| Vec::new()).collect();
    let mut segments = Vec::with_capacity(bounded_curves.len().saturating_mul(32 * 4));
    let mut curves = Vec::with_capacity(bounded_curves.len().saturating_mul(8));
    for (curve_index, (item, values)) in bounded_curves.iter().enumerate() {
        let Primitive::Bezier {
            points,
            width,
            color: _,
        } = &item.primitive
        else {
            return None;
        };
        let curve_index = u32::try_from(curve_index).ok()?;
        curves.extend([
            values.color.x,
            values.color.y,
            values.color.z,
            values.color.w,
            *width,
            0.0,
            0.0,
            0.0,
        ]);
        let mut previous = cubic_bezier_point(*points, 0.0);
        for segment in 0..32u32 {
            let current = cubic_bezier_point(*points, (segment + 1) as f32 / 32.0);
            let segment_index = curve_index.checked_mul(32)?.checked_add(segment)?;
            segments.extend([
                previous[0] - rect.x,
                previous[1] - rect.y,
                current[0] - rect.x,
                current[1] - rect.y,
            ]);
            let padding = *width * 0.5 + 1.25;
            let segment_bounds = ClipRect {
                x: previous[0].min(current[0]) - padding,
                y: previous[1].min(current[1]) - padding,
                width: (previous[0] - current[0]).abs() + padding * 2.0,
                height: (previous[1] - current[1]).abs() + padding * 2.0,
            };
            if let Some(segment_bounds) = intersect_clip(segment_bounds, rect) {
                let first_x = (((segment_bounds.x - rect.x) / BEZIER_BATCH_TILE_SIZE).floor()
                    as usize)
                    .min(columns - 1);
                let first_y = (((segment_bounds.y - rect.y) / BEZIER_BATCH_TILE_SIZE).floor()
                    as usize)
                    .min(rows - 1);
                let last_x = (((segment_bounds.x + segment_bounds.width - rect.x - f32::EPSILON)
                    / BEZIER_BATCH_TILE_SIZE)
                    .floor() as usize)
                    .min(columns - 1);
                let last_y = (((segment_bounds.y + segment_bounds.height - rect.y - f32::EPSILON)
                    / BEZIER_BATCH_TILE_SIZE)
                    .floor() as usize)
                    .min(rows - 1);
                for y in first_y..=last_y {
                    for x in first_x..=last_x {
                        tile_segments[y * columns + x].push(segment_index);
                    }
                }
            }
            previous = current;
        }
    }

    let metadata_count = tile_count.checked_mul(2)?;
    let reference_count: usize = tile_segments.iter().map(Vec::len).sum();
    if reference_count > MAX_BEZIER_BATCH_TILE_REFERENCES || metadata_count > u32::MAX as usize {
        return None;
    }
    let mut tile_indices = Vec::with_capacity(metadata_count + reference_count);
    tile_indices.resize(metadata_count, 0);
    for (tile, segment_indices) in tile_segments.into_iter().enumerate() {
        let header = tile * 2;
        tile_indices[header] = u32::try_from(tile_indices.len() - metadata_count).ok()?;
        tile_indices[header + 1] = u32::try_from(segment_indices.len()).ok()?;
        tile_indices.extend(segment_indices);
    }

    Some(BezierBatchValues {
        rect,
        segments,
        curves,
        tile_indices,
        parameters: Vec4::new(
            BEZIER_BATCH_TILE_SIZE,
            columns as f32,
            rows as f32,
            metadata_count as f32,
        ),
    })
}

impl BezierBatchSyncContext<'_, '_, '_> {
    fn sync(
        &mut self,
        key: BezierBatchKey,
        signature: Vec<DrawItem>,
        values: BezierBatchValues,
        first_z: i32,
        local_z: i32,
    ) {
        let Self {
            commands,
            cache,
            materials,
            buffers,
            generation,
        } = self;
        let generation = *generation;
        let clip = key.clip.clip();
        let group_entity = if let Some(group) = cache.clip_groups.get_mut(&key.clip) {
            group.generation = generation;
            group.entity
        } else {
            let entity = spawn_clip_group(commands, key.clip);
            cache
                .clip_groups
                .insert(key.clip, CachedClipGroup { entity, generation });
            entity
        };

        if let Some(mut cached) = cache.bezier_batches.remove(&key) {
            if cached.signature == signature {
                if cached.local_z != local_z {
                    commands
                        .entity(cached.entity)
                        .insert((GlobalZIndex(first_z), ZIndex(local_z)));
                }
                cached.first_z = first_z;
                cached.local_z = local_z;
                cached.generation = generation;
                cache.bezier_batches.insert(key, cached);
                return;
            }
            let segments_updated = if let Some(mut buffer) = buffers.get_mut(&cached.segments) {
                buffer.clear();
                buffer.extend_from_slice(&values.segments);
                true
            } else {
                false
            };
            let curves_updated = if let Some(mut buffer) = buffers.get_mut(&cached.curves) {
                buffer.clear();
                buffer.extend_from_slice(&values.curves);
                true
            } else {
                false
            };
            let tiles_updated = if let Some(mut buffer) = buffers.get_mut(&cached.tile_indices) {
                buffer.clear();
                buffer.extend_from_slice(&values.tile_indices);
                true
            } else {
                false
            };
            let material_updated =
                materials
                    .get_mut(&cached.material)
                    .is_some_and(|mut material| {
                        material.parameters = values.parameters;
                        true
                    });
            if segments_updated && curves_updated && tiles_updated && material_updated {
                if cached.rect != values.rect
                    || cached.first_z != first_z
                    || cached.local_z != local_z
                {
                    commands.entity(cached.entity).insert((
                        positioned_node(values.rect, clip),
                        GlobalZIndex(first_z),
                        ZIndex(local_z),
                    ));
                }
                cached.signature = signature;
                cached.rect = values.rect;
                cached.first_z = first_z;
                cached.local_z = local_z;
                cached.generation = generation;
                cache.bezier_batches.insert(key, cached);
                return;
            }
            commands.entity(cached.entity).despawn();
            materials.remove(cached.material.id());
            buffers.remove(cached.segments.id());
            buffers.remove(cached.curves.id());
            buffers.remove(cached.tile_indices.id());
        }

        let segments = buffers.add(ShaderBuffer::new(
            values.segments,
            RenderAssetUsages::default(),
        ));
        let curves = buffers.add(ShaderBuffer::new(
            values.curves,
            RenderAssetUsages::default(),
        ));
        let tile_indices = buffers.add(ShaderBuffer::new(
            values.tile_indices,
            RenderAssetUsages::default(),
        ));
        let material = materials.add(BezierBatchMaterial {
            segments: segments.clone(),
            curves: curves.clone(),
            tile_indices: tile_indices.clone(),
            parameters: values.parameters,
        });
        let mut entity = None;
        commands.entity(group_entity).with_children(|parent| {
            entity = Some(spawn_item_node(
                parent,
                (
                    positioned_node(values.rect, clip),
                    MaterialNode(material.clone()),
                ),
                first_z,
                local_z,
                true,
            ));
        });
        let entity = entity.expect("clip-group child should spawn");
        cache.bezier_batches.insert(
            key,
            CachedBezierBatch {
                entity,
                material,
                segments,
                curves,
                tile_indices,
                signature,
                rect: values.rect,
                first_z,
                local_z,
                generation,
            },
        );
    }
}

fn same_hsv_wheel_geometry(first: &DrawItem, second: &DrawItem) -> bool {
    if first.clip != second.clip || first.z != second.z {
        return false;
    }
    matches!(
        (&first.primitive, &second.primitive),
        (
            Primitive::HsvWheel { rect: first, .. },
            Primitive::HsvWheel { rect: second, .. }
        ) if first == second
    )
}

fn uses_theme_font_size(primitive: &Primitive) -> bool {
    match primitive {
        Primitive::Text { .. } => true,
        Primitive::TextWrapped { style, .. } => style
            .as_ref()
            .and_then(|style| style.font.as_ref())
            .is_none(),
        _ => false,
    }
}

fn uses_theme_line_height(primitive: &Primitive) -> bool {
    match primitive {
        Primitive::Text { .. } => true,
        Primitive::TextWrapped { style, .. } => {
            style.as_ref().and_then(|style| style.line_height).is_none()
        }
        _ => false,
    }
}

fn spawn_item_node(
    parent: &mut ChildSpawnerCommands<'_>,
    bundle: impl bevy::ecs::bundle::Bundle,
    z: i32,
    local_z: i32,
    mark_item: bool,
) -> Entity {
    if mark_item {
        parent
            .spawn((bundle, GlobalZIndex(z), ZIndex(local_z), RenderedItem))
            .id()
    } else {
        parent
            .spawn((bundle, GlobalZIndex(z), ZIndex(local_z)))
            .id()
    }
}

fn spawn_primitive(
    parent: &mut ChildSpawnerCommands<'_>,
    item: &DrawItem,
    images: &ImageRegistry,
    context: PrimitiveRenderContext<'_>,
    output: &mut RenderedNodes,
) {
    match &item.primitive {
        Primitive::RoundedRect {
            rect,
            radius,
            color,
        } if valid_clip(*rect) => {
            let mut node = positioned_node(*rect, item.clip);
            node.border_radius = BorderRadius::all(Val::Px(radius.max(0.0)));
            output.push(spawn_item_node(
                parent,
                (node, BackgroundColor(color_to_bevy(*color))),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Ellipse { rect, color } if valid_clip(*rect) => {
            let mut node = positioned_node(*rect, item.clip);
            node.border_radius = BorderRadius::all(Val::Percent(50.0));
            output.push(spawn_item_node(
                parent,
                (node, BackgroundColor(color_to_bevy(*color))),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Line {
            from,
            to,
            width,
            color,
        } if from.iter().chain(to).all(|value| value.is_finite())
            && width.is_finite()
            && *width > 0.0 =>
        {
            if let Some(entity) = spawn_line(
                parent,
                item.clip,
                *from,
                *to,
                *width,
                *color,
                RenderNodeOrder {
                    global_z: item.z,
                    local_z: context.local_z,
                    mark_item: true,
                },
            ) {
                output.push(entity);
            }
        }
        Primitive::Text {
            origin,
            text,
            color,
        } if origin.iter().all(|value| value.is_finite()) && !text.is_empty() => {
            output.push(spawn_item_node(
                parent,
                (
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(origin[0] - item.clip.x),
                        top: Val::Px(origin[1] - item.clip.y),
                        width: Val::Px((item.clip.width - (origin[0] - item.clip.x)).max(0.0)),
                        ..Default::default()
                    },
                    Text::new(text.clone()),
                    context.default_text_font.clone(),
                    LineHeight::RelativeToFont(context.text_line_height),
                    TextColor(color_to_bevy(*color)),
                ),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::TextWrapped {
            origin,
            text,
            color,
            max_width,
            wrap,
            style,
        } if origin.iter().all(|value| value.is_finite())
            && max_width.is_finite()
            && *max_width > 0.0
            && !text.is_empty() =>
        {
            output.push(spawn_item_node(
                parent,
                (
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(origin[0] - item.clip.x),
                        top: Val::Px(origin[1] - item.clip.y),
                        width: Val::Px(*max_width),
                        ..Default::default()
                    },
                    Text::new(text.clone()),
                    style
                        .as_ref()
                        .and_then(|style| style.font.clone())
                        .unwrap_or_else(|| context.default_text_font.clone()),
                    style
                        .as_ref()
                        .and_then(|style| style.line_height)
                        .unwrap_or(LineHeight::RelativeToFont(context.text_line_height)),
                    style
                        .as_ref()
                        .map_or(LetterSpacing::default(), |style| style.letter_spacing),
                    TextColor(color_to_bevy(*color)),
                    TextBounds::new_horizontal(*max_width),
                    TextLayout::new(
                        style
                            .as_ref()
                            .map_or(Justify::default(), |style| style.justify),
                        style.as_ref().map_or(
                            if *wrap {
                                LineBreak::WordBoundary
                            } else {
                                LineBreak::NoWrap
                            },
                            |style| style.line_break,
                        ),
                    ),
                ),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Image { rect, asset_id } if valid_clip(*rect) => {
            if let Some(image) = images.get(*asset_id) {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(*rect, item.clip),
                        ImageNode::new(image.clone()),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        Primitive::Shadow {
            rect,
            radius,
            color,
            shadow,
            offset,
            blur,
            spread,
        } if valid_clip(*rect)
            && radius.is_finite()
            && offset.iter().all(|value| value.is_finite())
            && blur.is_finite()
            && *blur >= 0.0
            && spread.is_finite() =>
        {
            let mut node = positioned_node(*rect, item.clip);
            node.border_radius = BorderRadius::all(Val::Px(radius.max(0.0)));
            output.push(spawn_item_node(
                parent,
                (
                    node,
                    BackgroundColor(color_to_bevy(*color)),
                    BoxShadow::new(
                        color_to_bevy(*shadow),
                        Val::Px(offset[0]),
                        Val::Px(offset[1]),
                        Val::Px(*spread),
                        Val::Px(*blur),
                    ),
                ),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Icon {
            rect,
            asset_id,
            tint,
        } if valid_clip(*rect) => {
            if let Some(image) = images.get(*asset_id) {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(*rect, item.clip),
                        ImageNode::new(image.clone()).with_color(color_to_bevy(*tint)),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        Primitive::Bezier { .. } => {
            if let (Some(values), Some(material)) =
                (bezier_material_values(item), context.bezier_material)
            {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(values.rect, item.clip),
                        MaterialNode(material.clone()),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        Primitive::HsvWheel { rect, .. } if valid_clip(*rect) => {
            if let Some(material) = context.hsv_material {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(*rect, item.clip),
                        MaterialNode(material.clone()),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        _ => {}
    }
}

fn spawn_line(
    parent: &mut ChildSpawnerCommands<'_>,
    clip: ClipRect,
    from: [f32; 2],
    to: [f32; 2],
    width: f32,
    color: Rgba,
    order: RenderNodeOrder,
) -> Option<Entity> {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return None;
    }
    let color = BackgroundColor(color_to_bevy(color));
    if dx.abs() <= f32::EPSILON {
        let rect = ClipRect {
            x: (from[0] - width * 0.5).round(),
            y: from[1].min(to[1]),
            width,
            height: length,
        };
        Some(spawn_item_node(
            parent,
            (positioned_node(rect, clip), color),
            order.global_z,
            order.local_z,
            order.mark_item,
        ))
    } else if dy.abs() <= f32::EPSILON {
        let rect = ClipRect {
            x: from[0].min(to[0]),
            y: (from[1] - width * 0.5).round(),
            width: length,
            height: width,
        };
        Some(spawn_item_node(
            parent,
            (positioned_node(rect, clip), color),
            order.global_z,
            order.local_z,
            order.mark_item,
        ))
    } else {
        let rect = ClipRect {
            x: (from[0] + to[0]) * 0.5 - length * 0.5,
            y: (from[1] + to[1]) * 0.5 - width * 0.5,
            width: length,
            height: width,
        };
        Some(spawn_item_node(
            parent,
            (
                positioned_node(rect, clip),
                UiTransform::from_rotation(Rot2::radians(dy.atan2(dx))),
                color,
            ),
            order.global_z,
            order.local_z,
            order.mark_item,
        ))
    }
}

fn clip_node(clip: ClipRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(clip.x),
        top: Val::Px(clip.y),
        width: Val::Px(clip.width),
        height: Val::Px(clip.height),
        overflow: Overflow::clip(),
        ..Default::default()
    }
}

fn positioned_node(rect: ClipRect, clip: ClipRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect.x - clip.x),
        top: Val::Px(rect.y - clip.y),
        width: Val::Px(rect.width),
        height: Val::Px(rect.height),
        ..Default::default()
    }
}

fn valid_clip(rect: ClipRect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|value| value.is_finite())
        && rect.width > 0.0
        && rect.height > 0.0
}

fn color_to_bevy(color: Rgba) -> Color {
    Color::srgba(color.0, color.1, color.2, color.3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::prelude::With;
    use bui_text::UI_DEFAULT_FONT_FAMILY;

    fn rect_item(z: i32, x: f32) -> DrawItem {
        DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 80.0,
            },
            z,
            primitive: Primitive::RoundedRect {
                rect: ClipRect {
                    x,
                    y: 4.0,
                    width: 30.0,
                    height: 18.0,
                },
                radius: 4.0,
                color: Rgba(0.2, 0.4, 0.8, 1.0),
            },
        }
    }

    #[test]
    fn display_list_is_stably_ordered_by_z() {
        let mut list = DisplayList::default();
        list.push(rect_item(4, 1.0));
        list.push(rect_item(2, 2.0));
        list.push(rect_item(4, 3.0));
        assert_eq!(
            list.items().iter().map(|item| item.z).collect::<Vec<_>>(),
            [2, 4, 4]
        );
        assert_eq!(list.items()[1].primitive, rect_item(4, 1.0).primitive);
        assert_eq!(list.items()[2].primitive, rect_item(4, 3.0).primitive);
    }

    #[test]
    fn replacing_widget_layer_preserves_manual_items_and_merged_order() {
        let mut list = DisplayList::default();
        list.push(rect_item(5, 1.0));
        list.replace_widget_items(vec![rect_item(2, 2.0), rect_item(8, 3.0)]);
        assert_eq!(
            list.items().iter().map(|item| item.z).collect::<Vec<_>>(),
            [2, 5, 8]
        );
        assert_eq!(list.manual_items().len(), 1);
        assert_eq!(list.widget_items().len(), 2);

        list.replace_widget_items(vec![rect_item(3, 4.0)]);
        assert_eq!(
            list.items().iter().map(|item| item.z).collect::<Vec<_>>(),
            [3, 5]
        );
        assert_eq!(
            list.manual_items()[0].primitive,
            rect_item(5, 1.0).primitive
        );
    }

    #[test]
    fn camera_overlay_replacement_preserves_base_camera_items() {
        let camera = Entity::PLACEHOLDER;
        let mut list = DisplayList::default();
        list.replace_camera_items(Some(camera), vec![rect_item(2, 2.0)]);
        list.replace_overlay_camera_items(
            camera,
            vec![DrawItem {
                clip: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 120.0,
                    height: 80.0,
                },
                z: 2,
                primitive: Primitive::Ellipse {
                    rect: ClipRect {
                        x: 8.0,
                        y: 8.0,
                        width: 20.0,
                        height: 20.0,
                    },
                    color: Rgba(0.9, 0.2, 0.1, 1.0),
                },
            }],
        );
        assert_eq!(list.camera_items(camera).unwrap().len(), 1);
        assert_eq!(list.items().len(), 2);
        assert!(matches!(
            list.items()[1].primitive,
            Primitive::Ellipse { .. }
        ));

        list.replace_overlay_camera_items(camera, Vec::new());
        assert_eq!(list.items().len(), 1);
        assert_eq!(list.camera_items(camera).unwrap().len(), 1);
    }

    #[test]
    fn screen_widget_overlay_preserves_base_layers_and_sorts_after_equal_z() {
        let mut list = DisplayList::default();
        list.push(rect_item(3, 1.0));
        list.replace_widget_items(vec![rect_item(3, 2.0)]);
        list.replace_overlay_widget_items(vec![rect_item(3, 3.0)]);

        assert_eq!(list.manual_items().len(), 1);
        assert_eq!(list.widget_items().len(), 1);
        assert_eq!(list.overlay_widget_items().len(), 1);
        assert_eq!(
            list.items()
                .iter()
                .map(|item| match &item.primitive {
                    Primitive::RoundedRect { rect, .. } => rect.x,
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>(),
            [1.0, 2.0, 3.0]
        );

        list.replace_overlay_widget_items(vec![rect_item(4, 4.0)]);
        assert_eq!(
            list.manual_items()[0].primitive,
            rect_item(3, 1.0).primitive
        );
        assert_eq!(
            list.widget_items()[0].primitive,
            rect_item(3, 2.0).primitive
        );
        assert_eq!(
            list.overlay_widget_items()[0].primitive,
            rect_item(4, 4.0).primitive
        );
    }

    #[test]
    fn image_registry_allocates_unique_ids_and_skips_explicit_registrations() {
        let mut registry = ImageRegistry::default();
        assert_eq!(registry.allocate_id(), Some(1));
        registry.insert(2, Handle::default());
        assert_eq!(registry.allocate_id(), Some(3));
        assert_eq!(registry.allocate_id(), Some(4));
    }

    #[test]
    fn ui_paint_uses_layout_geometry_clipping_and_z() {
        let paint = UiPaint(Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            radius: 6.0,
            color: Rgba(0.3, 0.4, 0.5, 1.0),
        });
        let layout = UiLayoutResult {
            window: Entity::PLACEHOLDER,
            rect: Rect::new(10.0, 20.0, 50.0, 40.0),
            clip: Some(Rect::new(0.0, 30.0, 25.0, 60.0)),
            z: 12,
            content_size: [50.0, 40.0],
            dpi_scale: 2.0,
        };
        let item = paint.draw_item(&layout);
        assert_eq!(item.z, 12);
        assert_eq!(
            item.clip,
            ClipRect {
                x: 10.0,
                y: 30.0,
                width: 15.0,
                height: 30.0,
            }
        );
        let Primitive::RoundedRect { rect, .. } = item.primitive else {
            panic!("expected rounded rectangle");
        };
        assert_eq!(rect, ClipRect::from(layout.rect));

        let ellipse_color = Rgba(0.9, 0.2, 0.1, 1.0);
        let ellipse = UiPaint(Primitive::Ellipse {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            color: ellipse_color,
        })
        .draw_item(&layout);
        assert_eq!(
            ellipse.primitive,
            Primitive::Ellipse {
                rect: ClipRect::from(layout.rect),
                color: ellipse_color,
            }
        );
    }

    #[test]
    fn gpu_ui_nodes_are_created_and_unchanged_entities_persist() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        app.world_mut()
            .resource_mut::<DisplayList>()
            .push(rect_item(3, 10.0));
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<RenderedItem>>();
        let first = query.single(app.world()).unwrap();
        let node = app.world().get::<Node>(first).unwrap();
        assert_ne!(node.overflow, Overflow::clip());
        assert_eq!(
            app.world().get::<GlobalZIndex>(first),
            Some(&GlobalZIndex(3))
        );
        let cache = app.world().resource::<RenderCache>();
        let group_key = cache.items[&RenderKey::ManualDisplayListIndex(0)]
            .clip_group
            .unwrap();
        let group = cache.clip_groups[&group_key].entity;
        assert_eq!(
            app.world().get::<Node>(group).unwrap().overflow,
            Overflow::clip()
        );

        app.update();
        assert_eq!(query.single(app.world()).unwrap(), first);

        app.world_mut()
            .resource_mut::<DisplayList>()
            .push(rect_item(4, 50.0));
        app.update();
        assert!(app.world().entities().contains(first));
        assert_eq!(query.iter(app.world()).count(), 2);
    }

    #[test]
    fn changed_items_reuse_clip_groups_and_retire_unused_groups() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        let mut second_clip = rect_item(1, 40.0);
        second_clip.clip = ClipRect {
            x: 30.0,
            y: 0.0,
            width: 80.0,
            height: 80.0,
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![
                rect_item(0, 10.0),
                second_clip.clone(),
                rect_item(2, 70.0),
            ]);
        app.update();

        let first_group_key = ClipGroupKey::new(rect_item(0, 10.0).clip, None);
        let second_group_key = ClipGroupKey::new(second_clip.clip, None);
        let (first_group, second_group) = {
            let cache = app.world().resource::<RenderCache>();
            assert_eq!(cache.clip_groups.len(), 2);
            (
                cache.clip_groups[&first_group_key].entity,
                cache.clip_groups[&second_group_key].entity,
            )
        };

        let mut changed_second = second_clip;
        if let Primitive::RoundedRect { color, .. } = &mut changed_second.primitive {
            *color = Rgba(0.9, 0.1, 0.2, 1.0);
        }
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![rect_item(0, 10.0), changed_second]);
        app.update();
        {
            let cache = app.world().resource::<RenderCache>();
            assert_eq!(cache.clip_groups.len(), 2);
            assert_eq!(cache.clip_groups[&first_group_key].entity, first_group);
            assert_eq!(cache.clip_groups[&second_group_key].entity, second_group);
        }

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![rect_item(1, 20.0)]);
        app.update();
        let cache = app.world().resource::<RenderCache>();
        assert_eq!(cache.clip_groups.len(), 1);
        assert!(cache.clip_groups.contains_key(&first_group_key));
        assert!(app.world().entities().contains(first_group));
        assert!(!app.world().entities().contains(second_group));
    }

    #[test]
    fn z_order_updates_reuse_unchanged_item_entities() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        let first = rect_item(1, 10.0);
        let second = rect_item(2, 50.0);
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![first.clone(), second]);
        app.update();
        let (second_entity, group) = {
            let cache = app.world().resource::<RenderCache>();
            let key = RenderKey::WidgetDisplayListIndex(1);
            (
                cache.items[&key].nodes.first().unwrap(),
                cache.clip_groups[&cache.items[&key].clip_group.unwrap()].entity,
            )
        };
        assert_eq!(app.world().get::<ZIndex>(second_entity), Some(&ZIndex(1)));

        let mut moved_first = first;
        moved_first.z = 3;
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![moved_first, rect_item(2, 50.0)]);
        app.update();
        assert!(app.world().entities().contains(second_entity));
        assert_eq!(app.world().get::<ZIndex>(second_entity), Some(&ZIndex(0)));
        assert!(app.world().entities().contains(group));
    }

    #[test]
    fn hsv_wheel_uses_one_material_node_and_updates_without_entity_churn() {
        let mut app = App::new();
        app.insert_resource(Assets::<HsvWheelMaterial>::default());
        app.add_plugins(BuiRenderPlugin);
        let make_item = |hue_degrees, saturation, value| DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z: 0,
            primitive: Primitive::HsvWheel {
                rect: ClipRect {
                    x: 10.0,
                    y: 10.0,
                    width: 68.0,
                    height: 68.0,
                },
                hue_degrees,
                saturation,
                value,
            },
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![make_item(20.0, 0.4, 0.8)]);
        app.update();

        let (first_entity, handle, first_parameters) = {
            let mut query = app
                .world_mut()
                .query_filtered::<(Entity, &MaterialNode<HsvWheelMaterial>), With<RenderedItem>>();
            let (entity, material_node) = query.single(app.world()).unwrap();
            let parameters = app
                .world()
                .resource::<Assets<HsvWheelMaterial>>()
                .get(&material_node.0)
                .unwrap()
                .parameters;
            (entity, material_node.0.clone(), parameters)
        };
        assert_eq!(first_parameters, Vec4::new(20.0, 0.4, 0.8, 0.0));

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![make_item(200.0, 0.75, 0.45)]);
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &MaterialNode<HsvWheelMaterial>), With<RenderedItem>>();
        let (second_entity, material_node) = query.single(app.world()).unwrap();
        assert_eq!(second_entity, first_entity);
        assert_eq!(material_node.0, handle);
        assert_eq!(
            app.world()
                .resource::<Assets<HsvWheelMaterial>>()
                .get(&handle)
                .unwrap()
                .parameters,
            Vec4::new(200.0, 0.75, 0.45, 0.0)
        );
    }

    #[test]
    fn hsv_wheel_material_assets_retire_when_items_leave_the_scene() {
        let mut app = App::new();
        app.insert_resource(Assets::<HsvWheelMaterial>::default());
        app.add_plugins(BuiRenderPlugin);
        let wheel = DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z: 0,
            primitive: Primitive::HsvWheel {
                rect: ClipRect {
                    x: 10.0,
                    y: 10.0,
                    width: 68.0,
                    height: 68.0,
                },
                hue_degrees: 20.0,
                saturation: 0.4,
                value: 0.8,
            },
        };

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![wheel.clone()]);
        app.update();
        let initial_handle = app.world().resource::<RenderCache>().items
            [&RenderKey::WidgetDisplayListIndex(0)]
            .hsv_material
            .as_ref()
            .unwrap()
            .clone();
        assert!(
            app.world()
                .resource::<Assets<HsvWheelMaterial>>()
                .get(initial_handle.id())
                .is_some()
        );

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![rect_item(0, 12.0)]);
        app.update();
        assert!(
            app.world()
                .resource::<Assets<HsvWheelMaterial>>()
                .get(initial_handle.id())
                .is_none()
        );

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![wheel]);
        app.update();
        let restored_handle = app.world().resource::<RenderCache>().items
            [&RenderKey::WidgetDisplayListIndex(0)]
            .hsv_material
            .as_ref()
            .unwrap()
            .clone();
        assert!(
            app.world()
                .resource::<Assets<HsvWheelMaterial>>()
                .get(restored_handle.id())
                .is_some()
        );

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(Vec::new());
        app.update();
        assert!(
            app.world()
                .resource::<Assets<HsvWheelMaterial>>()
                .get(restored_handle.id())
                .is_none()
        );
        assert!(app.world().resource::<RenderCache>().items.is_empty());
    }

    #[test]
    fn image_registry_replacement_updates_nodes_without_taking_asset_ownership() {
        let mut app = App::new();
        app.insert_resource(Assets::<Image>::default());
        app.add_plugins(BuiRenderPlugin);
        let (first_image, second_image) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (images.add(Image::default()), images.add(Image::default()))
        };
        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(7, first_image.clone());
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        };
        let image_item = DrawItem {
            clip,
            z: 0,
            primitive: Primitive::Image {
                rect: ClipRect {
                    x: 10.0,
                    y: 10.0,
                    width: 20.0,
                    height: 20.0,
                },
                asset_id: 7,
            },
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![image_item]);
        app.update();

        let (first_entity, first_node_image) = {
            let mut query = app
                .world_mut()
                .query_filtered::<(Entity, &ImageNode), With<RenderedItem>>();
            let (entity, node) = query.single(app.world()).unwrap();
            (entity, node.image.id())
        };
        assert_eq!(first_node_image, first_image.id());

        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(7, second_image.clone());
        app.update();
        let (second_entity, second_node_image) = {
            let mut query = app
                .world_mut()
                .query_filtered::<(Entity, &ImageNode), With<RenderedItem>>();
            let (entity, node) = query.single(app.world()).unwrap();
            (entity, node.image.id())
        };
        assert_ne!(second_entity, first_entity);
        assert_eq!(second_node_image, second_image.id());
        assert_eq!(
            app.world().resource::<RenderCache>().items[&RenderKey::WidgetDisplayListIndex(0)]
                .image
                .as_ref()
                .unwrap()
                .id(),
            second_image.id()
        );

        app.world_mut().resource_mut::<ImageRegistry>().remove(7);
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<RenderedItem>>();
        assert_eq!(query.iter(app.world()).count(), 0);
        assert!(
            app.world().resource::<RenderCache>().items[&RenderKey::WidgetDisplayListIndex(0)]
                .image
                .is_none()
        );
        let images = app.world().resource::<Assets<Image>>();
        assert!(images.get(first_image.id()).is_some());
        assert!(images.get(second_image.id()).is_some());
    }

    #[test]
    fn repeated_theme_and_clip_changes_keep_render_cache_bounded() {
        let mut app = App::new();
        app.insert_resource(Theme::default());
        app.add_plugins(BuiRenderPlugin);
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![DrawItem {
                clip: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 120.0,
                    height: 80.0,
                },
                z: 0,
                primitive: Primitive::Text {
                    origin: [4.0, 4.0],
                    text: "resize and theme".to_owned(),
                    color: Rgba(1.0, 1.0, 1.0, 1.0),
                },
            }]);
        app.update();

        for step in 0..24 {
            let width = 100.0 + step as f32;
            app.world_mut()
                .resource_mut::<DisplayList>()
                .replace_widget_items(vec![DrawItem {
                    clip: ClipRect {
                        x: step as f32,
                        y: 0.0,
                        width,
                        height: 70.0 + step as f32,
                    },
                    z: 0,
                    primitive: Primitive::Text {
                        origin: [4.0, 4.0],
                        text: "resize and theme".to_owned(),
                        color: Rgba(1.0, 1.0, 1.0, 1.0),
                    },
                }]);
            {
                let mut theme = app.world_mut().resource_mut::<Theme>();
                theme.font_size = 14.0 + (step % 3) as f32;
                theme.line_height = 1.25 + (step % 2) as f32 * 0.1;
            }
            app.update();

            let cache = app.world().resource::<RenderCache>();
            assert_eq!(cache.items.len(), 1);
            assert_eq!(cache.clip_groups.len(), 1);
            let rendered_items = {
                let mut query = app
                    .world_mut()
                    .query_filtered::<Entity, With<RenderedItem>>();
                query.iter(app.world()).count()
            };
            let rendered_clip_groups = {
                let mut query = app
                    .world_mut()
                    .query_filtered::<Entity, With<RenderedClipGroup>>();
                query.iter(app.world()).count()
            };
            assert_eq!(rendered_items, 1);
            assert_eq!(rendered_clip_groups, 1);
        }
    }

    #[test]
    fn theme_font_metrics_drive_text_nodes_and_invalidate_changed_text() {
        let mut app = App::new();
        app.insert_resource(Theme::default());
        app.add_plugins(BuiRenderPlugin);
        app.world_mut()
            .resource_mut::<DisplayList>()
            .push(DrawItem {
                clip: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 120.0,
                    height: 80.0,
                },
                z: 1,
                primitive: Primitive::Text {
                    origin: [4.0, 4.0],
                    text: "theme size".to_owned(),
                    color: Rgba(1.0, 1.0, 1.0, 1.0),
                },
            });
        app.update();

        let (first_entity, first_size, first_line_height) = {
            let mut query = app
                .world_mut()
                .query_filtered::<(Entity, &TextFont, &LineHeight), With<Text>>();
            let (entity, font, line_height) = query.single(app.world()).unwrap();
            (entity, font.font_size, *line_height)
        };
        assert_eq!(first_size, bevy::text::FontSize::Px(14.0));
        assert_eq!(first_line_height, LineHeight::RelativeToFont(1.25));
        let default_font = app.world().get::<TextFont>(first_entity).unwrap();
        assert!(matches!(
            default_font.font,
            bevy::text::FontSource::Family(ref family) if family == UI_DEFAULT_FONT_FAMILY
        ));

        {
            let mut theme = app.world_mut().resource_mut::<Theme>();
            theme.font_size = 20.0;
            theme.line_height = 1.5;
        }
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &TextFont, &LineHeight), With<Text>>();
        let (second_entity, second_font, second_line_height) = query.single(app.world()).unwrap();
        assert_eq!(second_font.font_size, bevy::text::FontSize::Px(20.0));
        assert_eq!(*second_line_height, LineHeight::RelativeToFont(1.5));
        assert_ne!(first_entity, second_entity);
        app.world_mut().resource_mut::<Theme>().line_height = 1.7;
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &LineHeight), With<Text>>();
        let (third_entity, third_line_height) = query.single(app.world()).unwrap();
        assert_eq!(*third_line_height, LineHeight::RelativeToFont(1.7));
        assert_ne!(second_entity, third_entity);
    }

    #[test]
    fn same_z_parent_background_is_ordered_under_nested_widget_text() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        let rect = Rect::new(0.0, 0.0, 180.0, 32.0);
        let parent = app
            .world_mut()
            .spawn((
                bui_core::UiNode,
                UiPaint(Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: 0.0,
                    color: Rgba(0.02, 0.03, 0.04, 1.0),
                }),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect,
                    clip: None,
                    z: 0,
                    content_size: [rect.width, rect.height],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        let child_rect = Rect::new(8.0, 4.0, 164.0, 24.0);
        let child = app
            .world_mut()
            .spawn((
                bui_core::UiNode,
                UiPaint(Primitive::RoundedRect {
                    rect: ClipRect::from(child_rect),
                    radius: 2.0,
                    color: Rgba(0.08, 0.09, 0.10, 1.0),
                }),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: child_rect,
                    clip: None,
                    z: 0,
                    content_size: [child_rect.width, child_rect.height],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        bui_core::set_parent(app.world_mut(), child, parent)
            .expect("the label paint belongs to the background container");
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![DrawItem {
                clip: ClipRect::from(rect),
                z: 0,
                primitive: Primitive::TextWrapped {
                    origin: [4.0, 4.0],
                    text: "same layer label".to_owned(),
                    color: Rgba(0.95, 0.95, 0.95, 1.0),
                    max_width: rect.width - 8.0,
                    wrap: false,
                    style: None,
                },
            }]);
        app.update();

        let cache = app.world().resource::<RenderCache>();
        let rendered_entity = |key| {
            let cached = &cache.items[&key];
            match &cached.nodes {
                RenderedNodes::Single(entity) => *entity,
                RenderedNodes::Multiple(entities) => {
                    *entities.first().expect("rendered item has a Bevy entity")
                }
                RenderedNodes::Empty => panic!("rendered item has a Bevy entity"),
            }
        };
        let parent_paint = rendered_entity(RenderKey::LayoutEntity(parent));
        let child_paint = rendered_entity(RenderKey::LayoutEntity(child));
        let widget_text = rendered_entity(RenderKey::WidgetDisplayListIndex(0));
        let parent_z = (
            *app.world().get::<GlobalZIndex>(parent_paint).unwrap(),
            *app.world().get::<ZIndex>(parent_paint).unwrap(),
        );
        let child_z = (
            *app.world().get::<GlobalZIndex>(child_paint).unwrap(),
            *app.world().get::<ZIndex>(child_paint).unwrap(),
        );
        let text_z = (
            *app.world().get::<GlobalZIndex>(widget_text).unwrap(),
            *app.world().get::<ZIndex>(widget_text).unwrap(),
        );
        assert_eq!(parent_z.0, child_z.0);
        assert_eq!(child_z.0, text_z.0);
        assert!(
            parent_z.1.0 < child_z.1.0,
            "parent paint must precede nested child paint, got parent={:?}, child={:?}",
            parent_z.1,
            child_z.1
        );
        assert!(
            child_z.1.0 < text_z.1.0,
            "nested paint must precede child widget text, got child={:?}, text={:?}",
            child_z.1,
            text_z.1
        );
    }

    #[test]
    fn wrapped_text_uses_explicit_width_and_linebreak_rule() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![DrawItem {
                clip,
                z: 0,
                primitive: Primitive::TextWrapped {
                    origin: [5.0, 6.0],
                    text: "wrapped text content".to_owned(),
                    color: Rgba(1.0, 1.0, 1.0, 1.0),
                    max_width: 83.0,
                    wrap: true,
                    style: None,
                },
            }]);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(&Node, &TextBounds, &TextLayout), With<Text>>();
        let (node, bounds, layout) = query.single(app.world()).unwrap();
        assert_eq!(node.width, Val::Px(83.0));
        assert_eq!(bounds.width, Some(83.0));
        assert_eq!(bounds.height, None);
        assert_eq!(layout.linebreak, TextLayout::default().linebreak);

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![DrawItem {
                clip,
                z: 0,
                primitive: Primitive::TextWrapped {
                    origin: [5.0, 6.0],
                    text: "single line".to_owned(),
                    color: Rgba(1.0, 1.0, 1.0, 1.0),
                    max_width: 83.0,
                    wrap: false,
                    style: Some(TextRenderStyle {
                        font: Some(TextFont::from_font_size(22.0)),
                        line_height: Some(LineHeight::RelativeToFont(1.8)),
                        letter_spacing: LetterSpacing::Px(2.0),
                        justify: Justify::Right,
                        line_break: LineBreak::AnyCharacter,
                    }),
                },
            }]);
        app.update();
        let (_, _, layout) = query.single(app.world()).unwrap();
        assert_eq!(layout.linebreak, LineBreak::AnyCharacter);
        assert_eq!(layout.justify, Justify::Right);
        let mut style_query = app
            .world_mut()
            .query_filtered::<(&TextFont, &LineHeight, &LetterSpacing), With<Text>>();
        let (font, line_height, spacing) = style_query.single(app.world()).unwrap();
        assert_eq!(font.font_size, bevy::text::FontSize::Px(22.0));
        assert_eq!(*line_height, LineHeight::RelativeToFont(1.8));
        assert_eq!(*spacing, LetterSpacing::Px(2.0));
    }

    #[test]
    fn shadow_icon_and_bezier_use_bevy_gpu_ui_components() {
        let mut app = App::new();
        app.insert_resource(Assets::<BezierMaterial>::default());
        app.add_plugins(BuiRenderPlugin);
        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(5, Handle::default());
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![
                DrawItem {
                    clip,
                    z: 0,
                    primitive: Primitive::Shadow {
                        rect: ClipRect {
                            x: 8.0,
                            y: 8.0,
                            width: 30.0,
                            height: 20.0,
                        },
                        radius: 3.0,
                        color: Rgba(0.2, 0.25, 0.3, 1.0),
                        shadow: Rgba(0.0, 0.0, 0.0, 0.5),
                        offset: [1.0, 2.0],
                        blur: 4.0,
                        spread: 1.0,
                    },
                },
                DrawItem {
                    clip,
                    z: 1,
                    primitive: Primitive::Icon {
                        rect: ClipRect {
                            x: 45.0,
                            y: 8.0,
                            width: 16.0,
                            height: 16.0,
                        },
                        asset_id: 5,
                        tint: Rgba(0.8, 0.9, 1.0, 1.0),
                    },
                },
                DrawItem {
                    clip,
                    z: 2,
                    primitive: Primitive::Bezier {
                        points: [[10.0, 60.0], [30.0, 35.0], [60.0, 75.0], [90.0, 45.0]],
                        width: 2.0,
                        color: Rgba(0.3, 0.8, 0.7, 1.0),
                    },
                },
            ]);
        app.update();

        let mut shadow_query = app.world_mut().query::<&BoxShadow>();
        assert_eq!(shadow_query.iter(app.world()).count(), 1);
        let mut icon_query = app.world_mut().query::<&ImageNode>();
        assert_eq!(icon_query.iter(app.world()).count(), 1);
        let mut curve_query = app
            .world_mut()
            .query_filtered::<(Entity, &MaterialNode<BezierMaterial>), With<RenderedItem>>();
        let (curve_entity, curve_material) = curve_query.single(app.world()).unwrap();
        let cached_curve =
            &app.world().resource::<RenderCache>().items[&RenderKey::WidgetDisplayListIndex(2)];
        assert_eq!(cached_curve.nodes.first(), Some(curve_entity));
        assert_eq!(
            app.world()
                .resource::<Assets<BezierMaterial>>()
                .get(&curve_material.0)
                .unwrap()
                .style
                .x,
            2.0
        );
        assert_eq!(app.world().resource::<Assets<BezierMaterial>>().len(), 1);

        let mut items = app
            .world_mut()
            .query_filtered::<Entity, With<RenderedItem>>();
        assert_eq!(items.iter(app.world()).count(), 3);
        let mut groups = app
            .world_mut()
            .query_filtered::<&Node, With<RenderedClipGroup>>();
        assert_eq!(groups.iter(app.world()).count(), 1);
        assert!(
            groups
                .iter(app.world())
                .all(|node| node.overflow == Overflow::clip())
        );
    }

    #[test]
    fn degenerate_bezier_controls_produce_finite_shader_uniforms() {
        let item = DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z: 0,
            primitive: Primitive::Bezier {
                points: [[25.0, 25.0]; 4],
                width: 2.0,
                color: Rgba(0.9, 0.4, 0.2, 1.0),
            },
        };
        let values = bezier_material_values(&item).unwrap();
        assert!(
            [
                values.points_01.x,
                values.points_01.y,
                values.points_01.z,
                values.points_01.w,
                values.points_23.x,
                values.points_23.y,
                values.points_23.z,
                values.points_23.w,
                values.style.x,
            ]
            .iter()
            .all(|value| value.is_finite())
        );
        assert_eq!(
            (values.points_01.x, values.points_01.y),
            (values.points_01.z, values.points_01.w)
        );
        assert_eq!(
            (values.points_23.x, values.points_23.y),
            (values.points_23.z, values.points_23.w)
        );
    }

    #[test]
    fn changed_bezier_updates_shader_and_bounds_without_node_churn() {
        let mut app = App::new();
        app.insert_resource(Assets::<BezierMaterial>::default());
        app.add_plugins(BuiRenderPlugin);
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            width: 160.0,
            height: 120.0,
        };
        let item = |points, color| DrawItem {
            clip,
            z: 0,
            primitive: Primitive::Bezier {
                points,
                width: 2.0,
                color,
            },
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![item(
                [[10.0, 20.0], [20.0, 15.0], [35.0, 25.0], [45.0, 20.0]],
                Rgba(0.2, 0.6, 0.9, 1.0),
            )]);
        app.update();

        let (entity, handle) = {
            let mut query = app
                .world_mut()
                .query_filtered::<(Entity, &MaterialNode<BezierMaterial>), With<RenderedItem>>();
            let (entity, node) = query.single(app.world()).unwrap();
            (entity, node.0.clone())
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![item(
                [[30.0, 25.0], [42.0, 38.0], [55.0, 10.0], [70.0, 30.0]],
                Rgba(0.9, 0.3, 0.1, 1.0),
            )]);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &MaterialNode<BezierMaterial>), With<RenderedItem>>();
        let (updated_entity, updated_node) = query.single(app.world()).unwrap();
        assert_eq!(updated_entity, entity);
        assert_eq!(updated_node.0, handle);
        let material = app
            .world()
            .resource::<Assets<BezierMaterial>>()
            .get(&handle)
            .unwrap();
        assert_eq!(material.color, Vec4::new(0.9, 0.3, 0.1, 1.0));
        let new_bounds = app.world().resource::<RenderCache>().items
            [&RenderKey::WidgetDisplayListIndex(0)]
            .bezier_values
            .unwrap()
            .rect;
        assert_eq!(material.points_01.x, 30.0 - new_bounds.x);
        let node = app.world().get::<Node>(updated_entity).unwrap();
        assert_eq!(node.left, Val::Px(new_bounds.x - clip.x));
        assert_eq!(node.top, Val::Px(new_bounds.y - clip.y));
        assert_eq!(app.world().resource::<Assets<BezierMaterial>>().len(), 1);
    }

    #[test]
    fn four_thousand_beziers_share_batched_render_and_release_gpu_assets() {
        const CURVE_COUNT: usize = 4_000;
        let mut app = App::new();
        app.insert_resource(Assets::<BezierMaterial>::default());
        app.insert_resource(Assets::<BezierBatchMaterial>::default());
        app.insert_resource(Assets::<ShaderBuffer>::default());
        app.add_plugins(BuiRenderPlugin);
        let clip = ClipRect {
            x: 0.0,
            y: 0.0,
            width: 1200.0,
            height: 800.0,
        };
        let items: Vec<DrawItem> = (0..CURVE_COUNT)
            .map(|index| {
                let x = (index % 100) as f32 * 10.0;
                let y = (index / 100) as f32 * 18.0;
                DrawItem {
                    clip,
                    z: index as i32,
                    primitive: Primitive::Bezier {
                        points: [
                            [x, y],
                            [x + 30.0, y + 18.0],
                            [x + 50.0, y - 12.0],
                            [x + 80.0, y + 2.0],
                        ],
                        width: 1.5,
                        color: Rgba(0.2, 0.75, 0.9, 1.0),
                    },
                }
            })
            .collect();
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(items.clone());
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
        let first_batch_entity = query.single(app.world()).unwrap();
        assert_eq!(query.iter(app.world()).count(), 1);
        assert_eq!(
            app.world().resource::<RenderCache>().items.len(),
            0,
            "batched curves are owned by the batch cache"
        );
        assert_eq!(
            app.world().resource::<RenderCache>().bezier_batches.len(),
            1
        );
        assert_eq!(
            app.world().resource::<Assets<BezierBatchMaterial>>().len(),
            1
        );
        assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 3);

        let mut updated_items = items;
        updated_items[CURVE_COUNT / 2].primitive =
            match updated_items[CURVE_COUNT / 2].primitive.clone() {
                Primitive::Bezier { points, width, .. } => Primitive::Bezier {
                    points,
                    width,
                    color: Rgba(0.9, 0.1, 0.2, 1.0),
                },
                _ => unreachable!(),
            };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(updated_items);
        app.update();
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
        assert_eq!(query.single(app.world()).unwrap(), first_batch_entity);
        assert_eq!(
            app.world().resource::<Assets<BezierBatchMaterial>>().len(),
            1
        );
        assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 3);

        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(Vec::new());
        app.update();
        assert!(
            app.world()
                .resource::<RenderCache>()
                .bezier_batches
                .is_empty()
        );
        assert_eq!(
            app.world().resource::<Assets<BezierBatchMaterial>>().len(),
            0
        );
        assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 0);
    }

    #[test]
    fn widget_layer_updates_do_not_respawn_manual_gpu_nodes() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        app.world_mut()
            .resource_mut::<DisplayList>()
            .push(rect_item(0, 12.0));
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<RenderedItem>>();
        let manual = query.single(app.world()).unwrap();
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![rect_item(1, 44.0)]);
        app.update();
        assert!(app.world().entities().contains(manual));
        assert_eq!(query.iter(app.world()).count(), 2);
    }

    #[test]
    fn screen_and_world_paints_route_to_separate_ui_cameras() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        let screen_window = app.world_mut().spawn_empty().id();
        let world_camera = app.world_mut().spawn(Camera::default()).id();
        let screen_entity = app
            .world_mut()
            .spawn((
                UiPaint(Primitive::RoundedRect {
                    rect: ClipRect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    radius: 0.0,
                    color: Rgba(1.0, 0.0, 0.0, 1.0),
                }),
                UiLayoutResult {
                    window: screen_window,
                    rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                    clip: None,
                    z: 0,
                    content_size: [20.0, 20.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        let world_entity = app
            .world_mut()
            .spawn((
                UiPaint(Primitive::RoundedRect {
                    rect: ClipRect {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    radius: 0.0,
                    color: Rgba(0.0, 1.0, 0.0, 1.0),
                }),
                UiLayoutResult {
                    window: world_camera,
                    rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                    clip: None,
                    z: 0,
                    content_size: [20.0, 20.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.update();

        let cache = app.world().resource::<RenderCache>();
        let screen_group_key = cache.items[&RenderKey::LayoutEntity(screen_entity)]
            .clip_group
            .unwrap();
        let world_group_key = cache.items[&RenderKey::LayoutEntity(world_entity)]
            .clip_group
            .unwrap();
        let screen_group = cache.clip_groups[&screen_group_key].entity;
        let world_group = cache.clip_groups[&world_group_key].entity;
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, Option<&UiTargetCamera>), With<RenderedClipGroup>>();
        let targets: HashMap<_, _> = query
            .iter(app.world())
            .map(|(entity, target)| (entity, target.map(|target| target.0)))
            .collect();
        assert_eq!(targets.get(&screen_group), Some(&None));
        assert_eq!(targets.get(&world_group), Some(&Some(world_camera)));
    }

    #[test]
    fn explicit_camera_display_items_attach_the_requested_camera() {
        let mut app = App::new();
        app.add_plugins(BuiRenderPlugin);
        let camera = app.world_mut().spawn(Camera::default()).id();
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_camera_items(Some(camera), vec![rect_item(7, 20.0)]);
        app.update();

        let cache = app.world().resource::<RenderCache>();
        let group_key = cache.items[&RenderKey::CameraDisplayListIndex(camera, 0)]
            .clip_group
            .unwrap();
        let group = cache.clip_groups[&group_key].entity;
        assert_eq!(app.world().get::<UiTargetCamera>(group).unwrap().0, camera);
    }

    #[test]
    fn world_surface_maps_center_and_rejects_half_open_edges() {
        let surface = UiWorldSurface {
            target_camera: Entity::PLACEHOLDER,
            texture_asset_id: 9,
            world_size: Vec2::new(2.0, 1.0),
            resolution: UVec2::new(400, 100),
        };
        assert_eq!(surface.local_to_ui(Vec2::ZERO), Some([200.0, 50.0]));
        assert_eq!(surface.local_to_ui(Vec2::new(-1.0, 0.5)), Some([0.0, 0.0]));
        assert_eq!(surface.local_to_ui(Vec2::new(1.0, 0.0)), None);
        assert_eq!(surface.local_to_ui(Vec2::new(0.0, -0.5)), None);
    }
}

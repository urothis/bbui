//! Visibility-aware image and video poster thumbnails for the virtualized file browser.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use bevy_app::{App, Update};
use bevy_asset::Assets;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::prelude::{
    Changed, Entity, Or, Query, RemovedComponents, Res, ResMut, Resource, With, World,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_image::Image;
use bevy_tasks::{AsyncComputeTaskPool, Task, futures_lite::future::block_on, poll_once};
use bevy_time::Time;
use bui_core::{UiDirty, UiDirtyDomain, UiNode, UiParent, UiSystems, UiVisible};
use bui_layout::{Rect, UiLayoutResult};
use bui_media::{
    RgbaFrame, decode_gif_thumbnail, decode_image_thumbnail, decode_ivf_vp8_thumbnail,
    decode_mp4_h264_thumbnail_cancellable, decode_svg_thumbnail, decode_webm_vp8_thumbnail,
    rgba_frame_to_image,
};
use bui_render::{ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive};
use bui_style::Theme;

use crate::{
    FileBrowser, FileView, PreviewKind,
    declarative::{FileBrowserItem, FileBrowserViewState},
};

const MAX_PREVIEW_ENCODED_BYTES: usize = 32 * 1024 * 1024;
const MAX_PREVIEW_DIMENSION: u32 = 128;
const MAX_CONCURRENT_PREVIEWS: usize = 4;
const MAX_CACHED_PREVIEWS: usize = 256;
const MAX_CACHED_PREVIEW_BYTES: usize = 16 * 1024 * 1024;
const PREVIEW_CACHE_IDLE_TIME: Duration = Duration::from_secs(5);

type PreviewTask = Task<Result<RgbaFrame, String>>;

enum PreviewState {
    Loading {
        task: PreviewTask,
        cancelled: Arc<AtomicBool>,
    },
    Ready {
        asset_id: u64,
        width: u32,
        height: u32,
        byte_len: usize,
    },
    Failed(String),
}

struct PreviewEntry {
    state: PreviewState,
    visible_owners: HashSet<Entity>,
    last_used: Duration,
}

/// Path-keyed, bounded cache for visible file-browser thumbnails.
///
/// The cache owns only IDs in [`ImageRegistry`]. Removing an idle entry drops BUI's strong image
/// handle, allowing Bevy to retire the corresponding GPU image after renderer extraction.
#[derive(Resource, Default)]
pub struct FileBrowserPreviewCache {
    entries: HashMap<PathBuf, PreviewEntry>,
    cached_bytes: usize,
    overlay_cameras: HashSet<Entity>,
    completed_decodes: usize,
    prepared: bool,
    sync_passes: u64,
}

impl FileBrowserPreviewCache {
    /// Number of thumbnails currently decoded and registered for GPU rendering.
    pub fn ready_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| matches!(&entry.state, PreviewState::Ready { .. }))
            .count()
    }

    /// Number of asynchronous preview jobs in flight.
    pub fn loading_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| matches!(&entry.state, PreviewState::Loading { .. }))
            .count()
    }

    /// Decoded thumbnail bytes held in the cache.
    pub const fn cached_bytes(&self) -> usize {
        self.cached_bytes
    }

    /// Number of successful decodes since the cache was created.
    pub const fn completed_decodes(&self) -> usize {
        self.completed_decodes
    }

    /// Number of preview synchronization passes that inspected visible browser items.
    pub const fn sync_passes(&self) -> u64 {
        self.sync_passes
    }

    /// Returns an error recorded for an item, if its latest visible load failed.
    pub fn failure(&self, path: &std::path::Path) -> Option<&str> {
        self.entries.get(path).and_then(|entry| match &entry.state {
            PreviewState::Failed(error) => Some(error.as_str()),
            _ => None,
        })
    }
}

/// Install the thumbnail cache after the file browser has mounted its visible virtual items.
pub fn install(app: &mut App) {
    app.init_resource::<FileBrowserPreviewCache>()
        .init_resource::<UiDirty>()
        .add_systems(
            Update,
            (collect_preview_changes, sync_file_browser_previews)
                .chain()
                .in_set(UiSystems::Widgets)
                .after(crate::actions::collect_widget_paint),
        );
    app.world_mut()
        .resource_mut::<UiDirty>()
        .mark_all(UiDirtyDomain::Preview);
}

#[expect(
    clippy::too_many_arguments,
    reason = "Independent Bevy change streams feed one preview cache"
)]
#[expect(
    clippy::type_complexity,
    reason = "Preview updates track independent Bevy change filters"
)]
fn collect_preview_changes(
    browsers: Query<Entity, Changed<FileBrowser>>,
    items: Query<Entity, Changed<FileBrowserItem>>,
    views: Query<Entity, Changed<FileBrowserViewState>>,
    layouts: Query<
        Entity,
        (
            Changed<UiLayoutResult>,
            Or<(With<FileBrowser>, With<FileBrowserItem>)>,
        ),
    >,
    visibility: Query<Entity, Or<(Changed<UiVisible>, Changed<UiParent>, Changed<UiNode>)>>,
    preview_entities: Query<Entity, Or<(With<FileBrowser>, With<FileBrowserItem>)>>,
    parents: Query<&UiParent>,
    mut removed_items: RemovedComponents<FileBrowserItem>,
    mut removed_browsers: RemovedComponents<FileBrowser>,
    mut removed_layouts: RemovedComponents<UiLayoutResult>,
    mut removed_visibility: RemovedComponents<UiVisible>,
    mut removed_parents: RemovedComponents<UiParent>,
    mut removed_nodes: RemovedComponents<UiNode>,
    theme: Option<Res<Theme>>,
    mut cache: ResMut<FileBrowserPreviewCache>,
    mut dirty: ResMut<UiDirty>,
) {
    cache.prepared = true;
    let removed_items_any = removed_items.read().count() > 0;
    let removed_browsers_any = removed_browsers.read().count() > 0;
    let removed_layouts_any = removed_layouts.read().count() > 0;
    let removed_visibility_any = removed_visibility.read().count() > 0;
    let removed_parents_any = removed_parents.read().count() > 0;
    let removed_nodes_any = removed_nodes.read().count() > 0;
    let removed = removed_items_any
        || removed_browsers_any
        || removed_layouts_any
        || removed_visibility_any
        || removed_parents_any
        || removed_nodes_any;
    let visibility_affects_previews = if visibility.is_empty() {
        false
    } else {
        let mut related = HashSet::new();
        for entity in &preview_entities {
            let mut current = Some(entity);
            while let Some(at) = current {
                if !related.insert(at) {
                    break;
                }
                current = parents.get(at).ok().map(|parent| parent.0);
            }
        }
        visibility.iter().any(|entity| related.contains(&entity))
    };
    if cache.entries.is_empty()
        && cache.overlay_cameras.is_empty()
        && browsers.is_empty()
        && items.is_empty()
        && views.is_empty()
    {
        return;
    }
    if !browsers.is_empty()
        || !items.is_empty()
        || !views.is_empty()
        || !layouts.is_empty()
        || visibility_affects_previews
        || removed
        || theme.as_ref().is_some_and(|theme| theme.is_changed())
    {
        dirty.mark_all(UiDirtyDomain::Preview);
    }
}

#[derive(Clone)]
struct VisiblePreview {
    entity: Entity,
    owner: Entity,
    path: PathBuf,
    kind: PreviewKind,
    layout: UiLayoutResult,
    clip: Rect,
}

/// Start, cancel, poll, evict, and paint thumbnails for currently visible grid items.
pub fn sync_file_browser_previews(world: &mut World) {
    // Tests and applications that install only WidgetPlugin need no task pool or image resources.
    if !world.contains_resource::<Assets<Image>>() || !world.contains_resource::<ImageRegistry>() {
        clear_preview_overlays(world);
        return;
    }

    let now = world
        .get_resource::<Time>()
        .map_or(Duration::ZERO, |time| time.elapsed());
    let prepared = std::mem::take(&mut world.resource_mut::<FileBrowserPreviewCache>().prepared);
    let dirty = world
        .get_resource_mut::<UiDirty>()
        .map(|mut dirty| dirty.take(UiDirtyDomain::Preview));
    let needs_sync = {
        let cache = world.resource::<FileBrowserPreviewCache>();
        !prepared
            || dirty.as_ref().is_some_and(|dirty| !dirty.is_empty())
            || cache.entries.values().any(|entry| {
                matches!(entry.state, PreviewState::Loading { .. })
                    || (entry.visible_owners.is_empty()
                        && now.saturating_sub(entry.last_used) >= PREVIEW_CACHE_IDLE_TIME)
            })
    };
    if !needs_sync {
        return;
    }
    {
        let mut cache = world.resource_mut::<FileBrowserPreviewCache>();
        cache.sync_passes = cache.sync_passes.saturating_add(1);
    }
    let visible_items = collect_visible_items(world);

    let mut evicted_ids = Vec::new();
    let mut completions = Vec::new();
    {
        let mut cache = world.resource_mut::<FileBrowserPreviewCache>();
        for entry in cache.entries.values_mut() {
            if !entry.visible_owners.is_empty() {
                entry.last_used = now;
            }
            entry.visible_owners.clear();
        }
        for item in &visible_items {
            if let Some(entry) = cache.entries.get_mut(&item.path) {
                entry.visible_owners.insert(item.entity);
                entry.last_used = now;
            }
        }

        let unowned_loading: Vec<_> = cache
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.visible_owners.is_empty()
                    && matches!(&entry.state, PreviewState::Loading { .. })
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in unowned_loading {
            if let Some(entry) = cache.entries.remove(&path)
                && let PreviewState::Loading { cancelled, .. } = entry.state
            {
                cancelled.store(true, Ordering::Release);
            }
        }

        let idle_paths: Vec<_> = cache
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.visible_owners.is_empty()
                    && now.saturating_sub(entry.last_used) >= PREVIEW_CACHE_IDLE_TIME
            })
            .map(|(path, _)| path.clone())
            .collect();
        for path in idle_paths {
            if let Some(entry) = cache.entries.remove(&path)
                && let PreviewState::Ready {
                    asset_id, byte_len, ..
                } = entry.state
            {
                cache.cached_bytes = cache.cached_bytes.saturating_sub(byte_len);
                evicted_ids.push(asset_id);
            }
        }

        for item in &visible_items {
            if cache.entries.contains_key(&item.path)
                || cache.entries.len() >= MAX_CACHED_PREVIEWS
                || cache.loading_count() >= MAX_CONCURRENT_PREVIEWS
            {
                continue;
            }
            let cancelled = Arc::new(AtomicBool::new(false));
            let task_cancelled = Arc::clone(&cancelled);
            let path = item.path.clone();
            let kind = item.kind;
            let task = AsyncComputeTaskPool::get()
                .spawn(async move { decode_preview_file(path, kind, task_cancelled) });
            cache.entries.insert(
                item.path.clone(),
                PreviewEntry {
                    state: PreviewState::Loading { task, cancelled },
                    visible_owners: HashSet::from([item.entity]),
                    last_used: now,
                },
            );
        }

        for (path, entry) in &mut cache.entries {
            if let PreviewState::Loading { task, .. } = &mut entry.state
                && let Some(result) = block_on(poll_once(task))
            {
                completions.push((path.clone(), result));
            }
        }
    }

    for asset_id in evicted_ids {
        world.resource_mut::<ImageRegistry>().remove(asset_id);
    }

    for (path, result) in completions {
        match result {
            Ok(frame) => admit_thumbnail(world, path, frame, now),
            Err(error) => {
                if let Some(entry) = world
                    .resource_mut::<FileBrowserPreviewCache>()
                    .entries
                    .get_mut(&path)
                {
                    entry.state = PreviewState::Failed(error);
                }
            }
        }
    }

    publish_visible_overlays(world, &visible_items);
}

fn collect_visible_items(world: &mut World) -> Vec<VisiblePreview> {
    let mut query = world.query::<(Entity, &FileBrowserItem, &UiLayoutResult)>();
    query
        .iter(world)
        .filter_map(|(entity, item, layout)| {
            let browser = world.get::<FileBrowser>(item.owner)?;
            let supported = matches!(
                item.preview,
                PreviewKind::Image
                    | PreviewKind::Svg
                    | PreviewKind::AnimatedImage
                    | PreviewKind::Video
            );
            (browser.view == FileView::Grid
                && supported
                && !item.directory
                && ui_tree_is_visible(world, entity))
            .then(|| VisiblePreview {
                entity,
                owner: item.owner,
                path: item.path.clone(),
                kind: item.preview,
                layout: *layout,
                clip: layout.rect,
            })
            .and_then(|mut preview| {
                preview.clip = file_browser_preview_clip(world, &preview)?;
                Some(preview)
            })
        })
        .collect()
}

fn ui_tree_is_visible(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut visited = HashSet::new();
    while let Some(at) = current {
        if !visited.insert(at) || world.get::<UiNode>(at).is_none() {
            return false;
        }
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    true
}

fn decode_preview_file(
    path: PathBuf,
    kind: PreviewKind,
    cancelled: Arc<AtomicBool>,
) -> Result<RgbaFrame, String> {
    if cancelled.load(Ordering::Acquire) {
        return Err("preview load was cancelled".to_owned());
    }
    let video_extension = if kind == PreviewKind::Video {
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
    } else {
        None
    };
    if kind == PreviewKind::Video {
        let Some(extension) = video_extension.as_deref() else {
            return Err("video file has no extension".to_owned());
        };
        if !matches!(extension, "ivf" | "webm" | "mkv" | "mp4" | "mov") {
            return Err(format!(
                "video poster previews are not supported for .{extension}; supported formats are VP8 in IVF, WebM, and Matroska, and H.264/AVC in MP4 and QuickTime MOV"
            ));
        }
    }
    let file = File::open(&path).map_err(|error| format!("open preview: {error}"))?;
    let length = file
        .metadata()
        .map_err(|error| format!("read preview metadata: {error}"))?
        .len();
    if length > MAX_PREVIEW_ENCODED_BYTES as u64 {
        return Err(format!(
            "preview file exceeds the {} byte limit",
            MAX_PREVIEW_ENCODED_BYTES
        ));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take((MAX_PREVIEW_ENCODED_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read preview: {error}"))?;
    if bytes.len() > MAX_PREVIEW_ENCODED_BYTES {
        return Err(format!(
            "preview file exceeds the {} byte limit",
            MAX_PREVIEW_ENCODED_BYTES
        ));
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("preview load was cancelled".to_owned());
    }
    let decoded = match kind {
        PreviewKind::Image => {
            decode_image_thumbnail(&bytes, MAX_PREVIEW_DIMENSION, MAX_PREVIEW_DIMENSION)
                .map_err(|error| format!("decode preview: {error}"))?
        }
        PreviewKind::Svg => {
            decode_svg_thumbnail(&bytes, MAX_PREVIEW_DIMENSION, MAX_PREVIEW_DIMENSION)
                .map_err(|error| format!("decode preview: {error}"))?
        }
        PreviewKind::AnimatedImage => {
            decode_gif_thumbnail(&bytes, MAX_PREVIEW_DIMENSION, MAX_PREVIEW_DIMENSION)
                .map_err(|error| format!("decode preview: {error}"))?
        }
        PreviewKind::Video => match video_extension.as_deref() {
            Some("ivf") => {
                decode_ivf_vp8_thumbnail(&bytes, MAX_PREVIEW_DIMENSION, MAX_PREVIEW_DIMENSION)
                    .map_err(|error| format!("decode preview: {error}"))?
            }
            Some("webm" | "mkv") => {
                decode_webm_vp8_thumbnail(&bytes, MAX_PREVIEW_DIMENSION, MAX_PREVIEW_DIMENSION)
                    .map_err(|error| format!("decode preview: {error}"))?
            }
            Some("mp4" | "mov") => decode_mp4_h264_thumbnail_cancellable(
                bytes,
                MAX_PREVIEW_DIMENSION,
                MAX_PREVIEW_DIMENSION,
                &cancelled,
            )
            .map_err(|error| format!("decode preview: {error}"))?
            .ok_or_else(|| "preview load was cancelled".to_owned())?,
            _ => unreachable!("unsupported video extensions return before file reads"),
        },
        _ => return Err("file type does not support image previews".to_owned()),
    };
    if cancelled.load(Ordering::Acquire) {
        return Err("preview load was cancelled".to_owned());
    }
    Ok(decoded)
}

fn admit_thumbnail(world: &mut World, path: PathBuf, frame: RgbaFrame, now: Duration) {
    let byte_len = frame.pixels.len();
    let (width, height) = (frame.width, frame.height);
    if byte_len == 0 || byte_len > MAX_CACHED_PREVIEW_BYTES {
        set_failure(
            world,
            &path,
            "decoded preview exceeds the cache limit".to_owned(),
        );
        return;
    }

    let mut evicted_ids = Vec::new();
    let fits = {
        let mut cache = world.resource_mut::<FileBrowserPreviewCache>();
        while cache.cached_bytes.saturating_add(byte_len) > MAX_CACHED_PREVIEW_BYTES {
            let candidate = cache
                .entries
                .iter()
                .filter(|(candidate_path, entry)| {
                    **candidate_path != path && entry.visible_owners.is_empty()
                })
                .filter_map(|(candidate_path, entry)| match &entry.state {
                    PreviewState::Ready { .. } | PreviewState::Failed(_) => {
                        Some((candidate_path.clone(), entry.last_used))
                    }
                    PreviewState::Loading { .. } => None,
                })
                .min_by_key(|(_, last_used)| *last_used)
                .map(|(candidate_path, _)| candidate_path);
            let Some(candidate) = candidate else {
                break;
            };
            if let Some(entry) = cache.entries.remove(&candidate)
                && let PreviewState::Ready {
                    asset_id, byte_len, ..
                } = entry.state
            {
                cache.cached_bytes = cache.cached_bytes.saturating_sub(byte_len);
                evicted_ids.push(asset_id);
            }
        }
        cache.cached_bytes.saturating_add(byte_len) <= MAX_CACHED_PREVIEW_BYTES
    };
    for asset_id in evicted_ids {
        world.resource_mut::<ImageRegistry>().remove(asset_id);
    }
    if !fits {
        set_failure(
            world,
            &path,
            "all decoded preview cache space is held by visible items".to_owned(),
        );
        return;
    }

    let image = match rgba_frame_to_image(frame) {
        Ok(image) => image,
        Err(error) => {
            set_failure(world, &path, format!("create preview image: {error}"));
            return;
        }
    };
    let Some(asset_id) = world.resource_mut::<ImageRegistry>().allocate_id() else {
        set_failure(
            world,
            &path,
            "image registry ID space is exhausted".to_owned(),
        );
        return;
    };
    let handle = world.resource_mut::<Assets<Image>>().add(image);
    world
        .resource_mut::<ImageRegistry>()
        .insert(asset_id, handle);

    let cache_entry_exists = {
        let mut cache = world.resource_mut::<FileBrowserPreviewCache>();
        if cache.entries.contains_key(&path) {
            cache.cached_bytes = cache.cached_bytes.saturating_add(byte_len);
            cache.completed_decodes = cache.completed_decodes.saturating_add(1);
            let entry = cache
                .entries
                .get_mut(&path)
                .expect("preview entry was checked above");
            entry.state = PreviewState::Ready {
                asset_id,
                width,
                height,
                byte_len,
            };
            entry.last_used = now;
            true
        } else {
            false
        }
    };
    if !cache_entry_exists {
        // The row went offscreen after its worker finished. Drop the registry reference rather
        // than turning a stale completion into an unowned thumbnail.
        world.resource_mut::<ImageRegistry>().remove(asset_id);
    }
}

fn set_failure(world: &mut World, path: &std::path::Path, error: String) {
    if let Some(entry) = world
        .resource_mut::<FileBrowserPreviewCache>()
        .entries
        .get_mut(path)
    {
        entry.state = PreviewState::Failed(error);
    }
}

fn publish_visible_overlays(world: &mut World, visible_items: &[VisiblePreview]) {
    let cache = world.resource::<FileBrowserPreviewCache>();
    let muted_text = world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().text_muted, |theme| theme.text_muted);
    let mut overlays = HashMap::<Entity, Vec<DrawItem>>::new();
    for item in visible_items {
        let Some(entry) = cache.entries.get(&item.path) else {
            continue;
        };
        let bounds = Rect::new(
            item.layout.rect.x + 8.0,
            item.layout.rect.y + 20.0,
            (item.layout.rect.width - 16.0).max(0.0),
            (item.layout.rect.height - 28.0).max(0.0),
        );
        let clip = item.clip;
        if clip.is_empty() || bounds.intersection(clip).is_none() {
            continue;
        }
        let items = overlays.entry(item.layout.window).or_default();
        match &entry.state {
            PreviewState::Ready {
                asset_id,
                width,
                height,
                ..
            } => {
                let Some(rect) = aspect_fit(bounds, *width, *height) else {
                    continue;
                };
                if rect.intersection(clip).is_some() {
                    items.push(DrawItem {
                        clip: ClipRect::from(clip),
                        z: item.layout.z,
                        primitive: Primitive::Image {
                            rect: ClipRect::from(rect),
                            asset_id: *asset_id,
                        },
                    });
                }
            }
            PreviewState::Loading { .. } => items.push(DrawItem {
                clip: ClipRect::from(clip),
                z: item.layout.z,
                primitive: Primitive::Text {
                    origin: [bounds.x, bounds.y],
                    text: "Loading preview…".to_owned(),
                    color: muted_text,
                },
            }),
            PreviewState::Failed(_) => items.push(DrawItem {
                clip: ClipRect::from(clip),
                z: item.layout.z,
                primitive: Primitive::Text {
                    origin: [bounds.x, bounds.y],
                    text: "Preview unavailable".to_owned(),
                    color: muted_text,
                },
            }),
        }
    }

    let old_cameras = cache.overlay_cameras.clone();
    let new_cameras: HashSet<_> = overlays.keys().copied().collect();
    let cameras: HashSet<_> = old_cameras.union(&new_cameras).copied().collect();
    let changed = cameras.iter().any(|camera| {
        let current = overlays.get(camera).map(Vec::as_slice).unwrap_or_default();
        world
            .resource::<DisplayList>()
            .overlay_camera_items(*camera)
            != if current.is_empty() {
                None
            } else {
                Some(current)
            }
    });
    if changed {
        let mut display_list = world.resource_mut::<DisplayList>();
        for camera in cameras {
            display_list
                .replace_overlay_camera_items(camera, overlays.remove(&camera).unwrap_or_default());
        }
    }
    world
        .resource_mut::<FileBrowserPreviewCache>()
        .overlay_cameras = new_cameras;
}

fn file_browser_preview_clip(world: &World, item: &VisiblePreview) -> Option<Rect> {
    let mut clip = item.layout.rect;
    if let Some(item_clip) = item.layout.clip {
        clip = clip.intersection(item_clip)?;
    }

    let Some(browser_layout) = world.get::<UiLayoutResult>(item.owner).copied() else {
        return Some(clip);
    };
    clip = clip.intersection(browser_layout.rect)?;
    if let Some(parent_clip) = browser_layout.clip {
        clip = clip.intersection(parent_clip)?;
    }

    // FileBrowserItem entities are virtualized directly under their browser. Their layout
    // result carries the browser's inherited clip, not the browser's own `clip_contents`
    // region. Derive the content viewport below the header and intersect it explicitly.
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let view = world
        .get::<FileBrowserViewState>(item.owner)
        .copied()
        .unwrap_or_default();
    let header_height = if view.header_height == 48.0 {
        theme.spacing.file_browser.header_height
    } else {
        view.header_height.max(1.0)
    }
    .min(browser_layout.rect.height);
    let content = Rect::new(
        browser_layout.rect.x,
        browser_layout.rect.y + header_height,
        browser_layout.rect.width,
        (browser_layout.rect.height - header_height).max(0.0),
    );
    clip.intersection(content)
}

fn clear_preview_overlays(world: &mut World) {
    let cameras = std::mem::take(
        &mut world
            .resource_mut::<FileBrowserPreviewCache>()
            .overlay_cameras,
    );
    if cameras.is_empty() || !world.contains_resource::<DisplayList>() {
        return;
    }
    let mut display_list = world.resource_mut::<DisplayList>();
    for camera in cameras {
        display_list.replace_overlay_camera_items(camera, Vec::new());
    }
}

fn aspect_fit(bounds: Rect, width: u32, height: u32) -> Option<Rect> {
    if bounds.is_empty() || width == 0 || height == 0 {
        return None;
    }
    let scale = (bounds.width / width as f32).min(bounds.height / height as f32);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let fitted_width = width as f32 * scale;
    let fitted_height = height as f32 * scale;
    Some(Rect::new(
        bounds.x + (bounds.width - fitted_width) * 0.5,
        bounds.y + (bounds.height - fitted_height) * 0.5,
        fitted_width,
        fitted_height,
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::Ordering,
        time::{SystemTime, UNIX_EPOCH},
    };

    use bevy_app::App;
    use bevy_asset::Assets;
    use bevy_ecs::prelude::World;
    use bevy_tasks::{AsyncComputeTaskPool, TaskPoolBuilder};

    use super::*;

    fn preview_world() -> World {
        let mut world = World::new();
        world.insert_resource(FileBrowserPreviewCache::default());
        world.insert_resource(Assets::<Image>::default());
        world.insert_resource(ImageRegistry::default());
        world.insert_resource(DisplayList::default());
        world
    }

    #[test]
    fn settled_preview_pass_skips_work_until_browser_input_changes() {
        let mut app = App::new();
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(ImageRegistry::default());
        app.insert_resource(DisplayList::default());
        install(&mut app);

        app.update();
        let first = app
            .world()
            .resource::<FileBrowserPreviewCache>()
            .sync_passes();
        assert_eq!(first, 1);
        app.update();
        assert_eq!(
            app.world()
                .resource::<FileBrowserPreviewCache>()
                .sync_passes(),
            first
        );

        app.world_mut()
            .resource_mut::<FileBrowserPreviewCache>()
            .entries
            .insert(
                PathBuf::from("cached-preview.png"),
                PreviewEntry {
                    state: PreviewState::Failed("fixture".to_owned()),
                    visible_owners: HashSet::new(),
                    last_used: Duration::ZERO,
                },
            );
        app.world_mut().spawn((UiNode, UiVisible(true)));
        app.update();
        assert_eq!(
            app.world()
                .resource::<FileBrowserPreviewCache>()
                .sync_passes(),
            first,
            "an unrelated visibility change does not rescan browser previews"
        );

        app.world_mut().spawn(FileBrowserItem {
            owner: Entity::PLACEHOLDER,
            path: PathBuf::from("/unused-preview.png"),
            name: "unused-preview.png".to_owned(),
            index: 0,
            directory: false,
            size: 0,
            selected: false,
            preview: PreviewKind::Image,
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<FileBrowserPreviewCache>()
                .sync_passes(),
            first + 1
        );
    }

    #[test]
    fn hidden_file_browser_items_do_not_start_preview_decoding() {
        let temp = std::env::temp_dir().join(format!(
            "bui-hidden-preview-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp).unwrap();
        let path = temp.join("preview.png");
        fs::write(&path, b"fixture").unwrap();

        let mut world = preview_world();
        let mut browser = FileBrowser::new(&temp).unwrap();
        browser.view = FileView::Grid;
        let owner = world.spawn((UiNode, UiVisible(false), browser)).id();
        let item = world
            .spawn((
                UiNode,
                UiVisible(true),
                UiParent(owner),
                FileBrowserItem {
                    owner,
                    path: path.clone(),
                    name: "preview.png".to_owned(),
                    index: 0,
                    directory: false,
                    size: 7,
                    selected: false,
                    preview: PreviewKind::Image,
                },
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: Rect::new(0.0, 48.0, 120.0, 78.0),
                    clip: Some(Rect::new(0.0, 0.0, 120.0, 126.0)),
                    z: 1,
                    content_size: [120.0, 78.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();

        sync_file_browser_previews(&mut world);
        assert_eq!(
            world.resource::<FileBrowserPreviewCache>().loading_count(),
            0
        );
        assert_eq!(world.resource::<FileBrowserPreviewCache>().ready_count(), 0);
        assert!(world.get_entity(item).is_ok());

        world.get_mut::<UiVisible>(owner).unwrap().0 = true;
        world.get_mut::<UiLayoutResult>(item).unwrap().rect.y = 500.0;
        sync_file_browser_previews(&mut world);
        assert_eq!(
            world.resource::<FileBrowserPreviewCache>().loading_count(),
            0,
            "a mounted item outside its inherited clip must not own a decode"
        );

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn unowned_pending_preview_work_is_cancelled_and_released() {
        let _ = AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
        let cancelled = Arc::new(AtomicBool::new(false));
        let task = AsyncComputeTaskPool::get()
            .spawn(async { std::future::pending::<Result<RgbaFrame, String>>().await });
        let path = PathBuf::from("unmounted-preview.png");
        let mut world = preview_world();
        world
            .resource_mut::<FileBrowserPreviewCache>()
            .entries
            .insert(
                path,
                PreviewEntry {
                    state: PreviewState::Loading {
                        task,
                        cancelled: Arc::clone(&cancelled),
                    },
                    visible_owners: HashSet::new(),
                    last_used: Duration::ZERO,
                },
            );

        sync_file_browser_previews(&mut world);
        assert!(cancelled.load(Ordering::Acquire));
        assert_eq!(
            world.resource::<FileBrowserPreviewCache>().loading_count(),
            0
        );
    }

    #[test]
    fn preview_expiry_starts_when_the_last_visible_owner_disappears() {
        let mut world = preview_world();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs(10));
        world.insert_resource(time);
        let path = PathBuf::from("cached-preview.png");
        world
            .resource_mut::<FileBrowserPreviewCache>()
            .entries
            .insert(
                path.clone(),
                PreviewEntry {
                    state: PreviewState::Failed("fixture".to_owned()),
                    visible_owners: HashSet::from([Entity::from_bits(1)]),
                    last_used: Duration::ZERO,
                },
            );

        sync_file_browser_previews(&mut world);
        assert!(
            world
                .resource::<FileBrowserPreviewCache>()
                .entries
                .contains_key(&path)
        );
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(4));
        sync_file_browser_previews(&mut world);
        assert!(
            world
                .resource::<FileBrowserPreviewCache>()
                .entries
                .contains_key(&path)
        );
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(2));
        sync_file_browser_previews(&mut world);
        assert!(
            !world
                .resource::<FileBrowserPreviewCache>()
                .entries
                .contains_key(&path)
        );
    }

    #[test]
    fn vp8_and_h264_video_posters_decode_and_unsupported_video_has_an_explicit_error() {
        let root = std::env::temp_dir().join(format!(
            "bui-video-preview-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();

        for extension in ["webm", "mkv"] {
            let path = root.join(format!("sample.{extension}"));
            fs::write(&path, include_bytes!("../../bui_media/assets/sample.webm")).unwrap();
            let poster =
                decode_preview_file(path, PreviewKind::Video, Arc::new(AtomicBool::new(false)))
                    .unwrap();
            assert_eq!((poster.width, poster.height), (16, 16));
            assert!(poster.pixels[0] > poster.pixels[1]);
        }

        for extension in ["mp4", "mov"] {
            let path = root.join(format!("sample.{extension}"));
            let mut fixture = include_bytes!("../../bui_media/assets/sample-h264.mp4").to_vec();
            if extension == "mov" {
                fixture[8..12].copy_from_slice(b"qt  ");
            }
            fs::write(&path, fixture).unwrap();
            let poster =
                decode_preview_file(path, PreviewKind::Video, Arc::new(AtomicBool::new(false)))
                    .unwrap();
            assert_eq!((poster.width, poster.height), (32, 18));
            assert!(
                poster
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[0] != pixel[1])
            );
        }

        let unsupported = root.join("sample.avi");
        fs::write(&unsupported, b"unsupported video fixture").unwrap();
        let error = decode_preview_file(
            unsupported,
            PreviewKind::Video,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
        assert!(error.contains("not supported for .avi"), "{error}");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn loading_and_failed_preview_states_publish_visible_status_text() {
        let _ = AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
        let root = std::env::temp_dir().join(format!(
            "bui-preview-status-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();

        let mut world = preview_world();
        world.insert_resource(Theme::default());
        let mut browser = FileBrowser::new(&root).unwrap();
        browser.view = FileView::Grid;
        let owner = world.spawn((UiNode, browser)).id();
        let paths = [root.join("loading.webm"), root.join("broken.mp4")];
        let mut entities = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let entity = world
                .spawn((
                    UiNode,
                    UiParent(owner),
                    FileBrowserItem {
                        owner,
                        path: path.clone(),
                        name: path.file_name().unwrap().to_string_lossy().into_owned(),
                        index,
                        directory: false,
                        size: 0,
                        selected: false,
                        preview: PreviewKind::Video,
                    },
                    UiLayoutResult {
                        window: Entity::PLACEHOLDER,
                        rect: Rect::new(0.0, (index as f32) * 80.0, 120.0, 78.0),
                        clip: Some(Rect::new(0.0, 0.0, 120.0, 160.0)),
                        z: 1,
                        content_size: [120.0, 78.0],
                        dpi_scale: 1.0,
                    },
                ))
                .id();
            entities.push(entity);
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        let task = AsyncComputeTaskPool::get()
            .spawn(async { std::future::pending::<Result<RgbaFrame, String>>().await });
        {
            let mut cache = world.resource_mut::<FileBrowserPreviewCache>();
            cache.entries.insert(
                paths[0].clone(),
                PreviewEntry {
                    state: PreviewState::Loading {
                        task,
                        cancelled: Arc::clone(&cancelled),
                    },
                    visible_owners: HashSet::from([entities[0]]),
                    last_used: Duration::ZERO,
                },
            );
            cache.entries.insert(
                paths[1].clone(),
                PreviewEntry {
                    state: PreviewState::Failed("unsupported MP4 container".to_owned()),
                    visible_owners: HashSet::from([entities[1]]),
                    last_used: Duration::ZERO,
                },
            );
        }

        let visible = collect_visible_items(&mut world);
        publish_visible_overlays(&mut world, &visible);
        let status_texts: Vec<_> = world
            .resource::<DisplayList>()
            .overlay_camera_items(Entity::PLACEHOLDER)
            .unwrap()
            .iter()
            .filter_map(|item| match &item.primitive {
                Primitive::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(status_texts.contains(&"Loading preview…"));
        assert!(status_texts.contains(&"Preview unavailable"));
        cancelled.store(true, Ordering::Release);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn thumbnail_bounds_preserve_source_aspect_ratio_without_upscaling() {
        assert_eq!(
            aspect_fit(Rect::new(0.0, 0.0, 104.0, 50.0), 1, 1),
            Some(Rect::new(27.0, 0.0, 50.0, 50.0))
        );
        assert_eq!(
            aspect_fit(Rect::new(0.0, 0.0, 104.0, 50.0), 2, 1),
            Some(Rect::new(2.0, 0.0, 100.0, 50.0))
        );
    }
}

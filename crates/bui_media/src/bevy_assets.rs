//! Bevy asset loading and playback for BUI image media.
//!
//! Bevy's `ImageLoader` handles ordinary raster image extensions. This module adds SVG images,
//! animated GIFs, IVF/VP8, WebM/VP8, and MP4/MOV H.264 as Bevy assets. Every decoded frame is
//! uploaded through a normal Bevy `Image`, so Bevy owns its GPU allocation and release lifetime.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    error::Error,
    fmt, io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    },
    time::Duration,
};

use bevy::{
    asset::{
        Asset, AssetApp, AssetEvent, AssetId, AssetLoader, AssetServer, AsyncReadExt, Handle,
        LoadContext, RenderAssetUsages, io::Reader,
    },
    camera::visibility::VisibilitySystems,
    ecs::message::MessageReader,
    ecs::schedule::IntoScheduleConfigs,
    ecs::system::SystemParam,
    ecs::world::FromWorld,
    prelude::{
        App, Assets, Component, Entity, Image, ImageNode, InheritedVisibility, Plugin, PostUpdate,
        Query, Res, ResMut, Resource, Time, Visibility,
    },
    reflect::TypePath,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    MediaDecodeError, RgbaFrame, decode_gif_stream_cancellable, decode_image_with_max_bytes,
    decode_ivf_vp8_stream_cancellable, decode_mp4_h264_stream_cancellable,
    decode_svg_with_max_bytes, decode_webm_vp8_stream_cancellable,
};

const MAX_ASSET_BYTES: usize = 128 * 1024 * 1024;
const MAX_SEQUENCE_FRAMES: usize = 1_000_000;
const MAX_SEQUENCE_BYTES: usize = 256 * 1024 * 1024;
const MIN_FRAME_DURATION: Duration = Duration::from_millis(10);
const DEFAULT_VIDEO_FRAME_DURATION: Duration = Duration::from_nanos(33_333_333);
const DEFAULT_GLOBAL_DECODED_BYTES: usize = MAX_SEQUENCE_BYTES * 2;
const MAX_STREAMED_VIDEO_CACHE_FRAMES: usize = 8;
const STREAMED_VIDEO_CHANNEL_FRAMES: usize = 2;

/// Configurable caps for encoded media and the decoded frame cache.
///
/// Encoded animation/video sequences stay in their Bevy asset as bounded source bytes. Frames are
/// decoded only for visible players and held in an eight-frame rolling window per player. The
/// global budget accounts for cached decoded RGBA bytes; it excludes transient decoder scratch
/// memory, encoded sources, still images, and GPU allocator overhead.
#[derive(Resource, Debug, Clone, Copy)]
pub struct BuiMediaLimits {
    /// Maximum encoded file size accepted by a custom media loader.
    pub max_encoded_bytes: usize,
    /// Maximum RGBA bytes retained in one source-backed sequence's rolling frame cache.
    pub max_sequence_bytes: usize,
    /// Maximum decoded frame count accepted from one source-backed sequence.
    pub max_sequence_frames: usize,
    /// Maximum decoded RGBA bytes retained by all cached sequences and still strongly owned frame Images.
    pub max_global_decoded_bytes: usize,
}

impl Default for BuiMediaLimits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: MAX_ASSET_BYTES,
            max_sequence_bytes: MAX_SEQUENCE_BYTES,
            max_sequence_frames: MAX_SEQUENCE_FRAMES,
            max_global_decoded_bytes: DEFAULT_GLOBAL_DECODED_BYTES,
        }
    }
}

#[derive(Debug, Default)]
struct DecodedBudgetState {
    used_bytes: AtomicUsize,
}

/// Shared decoded-frame memory budget for visible and recently visible animation/video sequences.
///
/// Reservations follow Bevy frame Image assets until their last strong handle is dropped. Hiding
/// a player cancels an in-flight decode; rolling frames are dropped as playback advances, and Bevy
/// then releases their GPU images after render extraction. Configure
/// [`BuiMediaLimits::max_global_decoded_bytes`] before adding [`BuiMediaPlugin`].
#[derive(Resource, Debug, Clone)]
pub struct BuiMediaDecodedBudget {
    max_bytes: usize,
    state: Arc<DecodedBudgetState>,
}

#[derive(Resource, Default)]
struct DecodedImageBudgetReservations {
    // A reservation follows the Bevy Image asset until its last strong UI/asset handle is dropped.
    by_image: HashMap<AssetId<Image>, DecodedFrameReservation>,
}

impl BuiMediaDecodedBudget {
    /// Create a budget with a fixed decoded-frame byte limit.
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            state: Arc::default(),
        }
    }

    /// Maximum number of cached decoded RGBA frame bytes.
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Current reservation total for decoded frame Images still held by cache or strong handles.
    pub fn used_bytes(&self) -> usize {
        self.state.used_bytes.load(Ordering::Acquire)
    }

    /// Remaining capacity available for another sequence.
    pub fn available_bytes(&self) -> usize {
        self.max_bytes.saturating_sub(self.used_bytes())
    }

    fn try_reserve(&self, bytes: usize) -> Option<DecodedFrameReservation> {
        if bytes == 0 {
            return Some(DecodedFrameReservation {
                state: Arc::clone(&self.state),
                bytes,
            });
        }

        let mut current = self.state.used_bytes.load(Ordering::Acquire);
        loop {
            let next = current.checked_add(bytes)?;
            if next > self.max_bytes {
                return None;
            }
            match self.state.used_bytes.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(DecodedFrameReservation {
                        state: Arc::clone(&self.state),
                        bytes,
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }
}

impl FromWorld for BuiMediaDecodedBudget {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        let max_bytes = world
            .get_resource::<BuiMediaLimits>()
            .copied()
            .unwrap_or_default()
            .max_global_decoded_bytes;
        Self::new(max_bytes)
    }
}

#[derive(Debug)]
struct DecodedFrameReservation {
    state: Arc<DecodedBudgetState>,
    bytes: usize,
}

impl Drop for DecodedFrameReservation {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.state
                .used_bytes
                .fetch_sub(self.bytes, Ordering::AcqRel);
        }
    }
}

/// Kind of media stored in a [`BuiMediaSequence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSequenceKind {
    Gif,
    IvfVp8,
    WebmVp8,
    Mp4H264,
}

/// Metadata for an animation/video sequence.
///
/// Caller-created sequences keep their explicit Bevy `Image` frame handles in `frames`. BUI's
/// loaders instead retain bounded encoded source bytes here and leave `frames` empty; visible
/// players decode on demand into [`BuiMediaFrameCache`]. The sequence asset can therefore remain
/// strongly referenced while its decoded CPU/GPU frame cache is evicted.
#[derive(Asset, TypePath, Debug)]
pub struct BuiMediaSequence {
    /// Explicit frame images in presentation order for caller-created sequences.
    ///
    /// Sequences loaded by BUI keep encoded source bytes and leave this vector empty; their cached
    /// images are available on the visible player's [`ImageNode`] and are owned by
    /// [`BuiMediaFrameCache`].
    #[dependency]
    pub frames: Vec<Handle<Image>>,
    /// Durations for explicit caller-provided frame handles. Missing entries use the default
    /// video cadence; zero-length entries are clamped to 10 ms when the timeline is evaluated.
    /// Loader-created sequences keep decoded durations in the frame cache until eviction.
    pub frame_durations: Vec<Duration>,
    /// Source format and codec.
    pub kind: MediaSequenceKind,
    encoded_source: Option<Arc<[u8]>>,
}

impl BuiMediaSequence {
    /// Create a sequence from frame handles managed by the caller.
    ///
    /// Caller-created sequences are not charged to [`BuiMediaDecodedBudget`]; only sequences
    /// admitted through BUI's GIF and video loaders own decoded-frame reservations.
    pub fn new(
        frames: Vec<Handle<Image>>,
        frame_durations: Vec<Duration>,
        kind: MediaSequenceKind,
    ) -> Self {
        Self {
            frames,
            frame_durations,
            kind,
            encoded_source: None,
        }
    }

    /// Create an on-demand sequence from encoded GIF, VP8, or H.264 video bytes.
    ///
    /// The source remains in this Bevy asset while handles exist. Visible players decode it into
    /// the bounded frame cache; hidden players do not start a decode. The configured encoded and
    /// decoded limits are enforced when the visible decode request runs.
    pub fn from_encoded_bytes(source: Vec<u8>, kind: MediaSequenceKind) -> Self {
        Self {
            frames: Vec::new(),
            frame_durations: Vec::new(),
            kind,
            encoded_source: Some(Arc::from(source)),
        }
    }

    /// Current RGBA storage owned by this sequence.
    ///
    /// Loaded assets keep encoded data; the decoded cache reservation is reported by
    /// [`BuiMediaDecodedBudget::used_bytes`]. Caller-created sequences report zero.
    pub fn decoded_bytes(&self) -> usize {
        0
    }

    /// Number of explicit frame handles in this sequence.
    ///
    /// Loader-created sequences return zero here until frames are decoded into the media cache.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether the sequence contains no frames.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.encoded_source.is_none()
    }

    /// Timeline length for explicit frame handles, defaulting missing durations to the 30 fps
    /// video cadence and clamping zero-length durations to 10 ms. Loader-created sequences return
    /// zero here until their visible player has decoded the timeline.
    pub fn duration(&self) -> Duration {
        (0..self.frames.len()).fold(Duration::ZERO, |total, index| {
            total.saturating_add(normalize_duration(self.frame_duration(index)))
        })
    }

    fn frame_at(&self, position: Duration) -> usize {
        let mut end = Duration::ZERO;
        for index in 0..self.frames.len() {
            end = end.saturating_add(normalize_duration(self.frame_duration(index)));
            if position < end {
                return index;
            }
        }
        self.frames.len().saturating_sub(1)
    }

    fn frame_duration(&self, index: usize) -> Duration {
        self.frame_durations
            .get(index)
            .copied()
            .unwrap_or(DEFAULT_VIDEO_FRAME_DURATION)
    }
}

/// Playback and on-demand decode state for a [`BuiMediaPlayer`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BuiMediaPlayerState {
    /// The sequence handle has not resolved to an asset yet.
    #[default]
    WaitingForAsset,
    /// The player is hidden; an in-flight decode is cancelled and its image handle is released.
    Hidden,
    /// The visible player's encoded source is being decoded by a bounded-frame worker thread.
    Decoding,
    /// A decoded frame image is ready and displayed.
    Ready,
    /// The decode failed or the active visible set exceeds the decoded-frame budget.
    Failed(String),
}

struct CachedVideoFrame {
    index: usize,
    timestamp: Duration,
    duration: Duration,
    byte_len: usize,
    image: Handle<Image>,
}

enum VideoStreamMessage {
    Frame(crate::VideoFrame),
    Finished(Result<(), String>),
}

struct StreamingVideo {
    sequence_id: AssetId<BuiMediaSequence>,
    receiver: Mutex<Receiver<VideoStreamMessage>>,
    task: Option<std::thread::JoinHandle<()>>,
    cancellation: Arc<AtomicBool>,
    frames: VecDeque<CachedVideoFrame>,
    pending_frame: Option<crate::VideoFrame>,
    next_index: usize,
    frame_capacity: usize,
    cached_bytes: usize,
    display_index: usize,
    eof: bool,
    duration: Option<Duration>,
    failure: Option<String>,
    last_used: Duration,
    pending_budget_waits: u8,
}

/// Bounded rolling caches for visible GIF and video players.
///
/// Hiding a player cancels its decoder and drops cached frame handles. Any image still strongly
/// held elsewhere remains a normal Bevy asset, with its decoded-byte reservation held until the
/// final strong handle is dropped. Bevy then releases its GPU representation through normal render
/// asset cleanup. Encoded sequences remain owned by AssetServer while strong sequence handles exist.
#[derive(Resource, Default)]
pub struct BuiMediaFrameCache {
    video_streams: HashMap<Entity, StreamingVideo>,
    cancelled_jobs: usize,
}

impl BuiMediaFrameCache {
    /// Number of decoded frame images currently owned by the cache.
    pub fn cached_image_count(&self) -> usize {
        self.video_streams
            .values()
            .map(|stream| stream.frames.len())
            .sum::<usize>()
    }

    /// Number of visible source decoders still producing frames.
    pub fn pending_decode_count(&self) -> usize {
        self.video_streams
            .values()
            .filter(|stream| stream.task.is_some() && !stream.eof)
            .count()
    }

    /// Number of decode jobs cancelled after their visible owners disappeared.
    pub fn cancelled_job_count(&self) -> usize {
        self.cancelled_jobs
    }

    /// Number of source sequences currently represented in the cache.
    pub fn sequence_count(&self) -> usize {
        self.video_streams
            .values()
            .map(|stream| stream.sequence_id)
            .collect::<HashSet<_>>()
            .len()
    }
}

/// Player for a [`BuiMediaSequence`], intended for an entity with a Bevy [`ImageNode`].
///
/// The clock advances only when the entity is not hidden by `Visibility` or
/// `InheritedVisibility`, and is visible to at least one camera when Bevy supplies
/// `ViewVisibility`. A player can be paused by setting `playing` to `false`; its last frame remains
/// displayed.
#[derive(Component, Debug, Clone)]
pub struct BuiMediaPlayer {
    /// Sequence asset handle.
    pub sequence: Handle<BuiMediaSequence>,
    /// Whether playback advances while visible.
    pub playing: bool,
    /// Whether playback wraps from the last frame to the first.
    pub looping: bool,
    /// Current frame index, updated by the playback system.
    pub current_frame: usize,
    /// Position in the sequence timeline.
    pub position: Duration,
    /// Current asset/decode state. The system updates this field.
    pub state: BuiMediaPlayerState,
    active_sequence: Option<bevy::asset::AssetId<BuiMediaSequence>>,
}

impl BuiMediaPlayer {
    pub fn new(sequence: Handle<BuiMediaSequence>) -> Self {
        Self {
            sequence,
            playing: true,
            looping: true,
            current_frame: 0,
            position: Duration::ZERO,
            state: BuiMediaPlayerState::WaitingForAsset,
            active_sequence: None,
        }
    }

    /// Stop playback and return the clock to the first frame.
    pub fn restart(&mut self) {
        self.position = Duration::ZERO;
        self.current_frame = 0;
        self.playing = true;
    }

    /// Seek to a position in the sequence timeline. The player system selects the matching frame
    /// on its next update.
    pub fn seek_to(&mut self, position: Duration) {
        self.position = position;
    }
}

/// Registers BUI media asset loaders and the visibility-aware [`BuiMediaPlayer`] system.
///
/// Add this after Bevy's `AssetPlugin` and `ImagePlugin` (normally through `DefaultPlugins`).
/// Bevy's built-in `ImageLoader` owns PNG/JPEG/WebP and other enabled raster extensions; this
/// plugin adds `.svg`, `.gif`, `.ivf`, `.webm`, `.mkv`, `.mp4`, and `.mov`.
pub struct BuiMediaPlugin;

impl Plugin for BuiMediaPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AssetServer>() {
            panic!("BuiMediaPlugin requires Bevy's AssetPlugin");
        }
        app.init_resource::<BuiMediaLimits>()
            .init_resource::<BuiMediaDecodedBudget>()
            .init_resource::<DecodedImageBudgetReservations>()
            .init_resource::<BuiMediaFrameCache>()
            .init_asset::<BuiMediaSequence>()
            .init_asset_loader::<RasterSvgImageLoader>()
            .init_asset_loader::<GifSequenceLoader>()
            .init_asset_loader::<IvfSequenceLoader>()
            .init_asset_loader::<WebmSequenceLoader>()
            .init_asset_loader::<Mp4H264SequenceLoader>()
            .add_systems(
                PostUpdate,
                update_media_players.after(VisibilitySystems::CheckVisibility),
            );
    }
}

/// Loader for SVG images, with `.buiimg` as an opt-in alias that accepts any format supported by
/// `image`'s raster decoder. Common raster extensions use Bevy's built-in `ImageLoader`.
#[derive(Debug, Clone, TypePath)]
pub struct RasterSvgImageLoader {
    limits: BuiMediaLimits,
}

impl FromWorld for RasterSvgImageLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for RasterSvgImageLoader {
    type Asset = Image;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        let extension = load_context
            .path()
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        let frame = if extension.eq_ignore_ascii_case("svg") {
            decode_svg_with_max_bytes(&bytes, self.limits.max_sequence_bytes)
        } else {
            decode_image_with_max_bytes(&bytes, self.limits.max_sequence_bytes)
        }
        .map_err(configured_decode_error)?;
        if frame.pixels.len() > self.limits.max_sequence_bytes {
            return Err(MediaAssetLoadError::DecodedLimit);
        }
        image_from_frame(frame)
    }

    fn extensions(&self) -> &[&str] {
        &["svg", "buiimg"]
    }
}

#[derive(Debug, Clone, TypePath)]
struct GifSequenceLoader {
    limits: BuiMediaLimits,
}

impl FromWorld for GifSequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for GifSequenceLoader {
    type Asset = BuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::Gif,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["gif"]
    }
}

#[derive(Debug, Clone, TypePath)]
struct IvfSequenceLoader {
    limits: BuiMediaLimits,
}

impl FromWorld for IvfSequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for IvfSequenceLoader {
    type Asset = BuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::IvfVp8,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["ivf"]
    }
}

#[derive(Debug, Clone, TypePath)]
struct WebmSequenceLoader {
    limits: BuiMediaLimits,
}

#[derive(Debug, Clone, TypePath)]
struct Mp4H264SequenceLoader {
    limits: BuiMediaLimits,
}

impl FromWorld for Mp4H264SequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for Mp4H264SequenceLoader {
    type Asset = BuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::Mp4H264,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["mp4", "mov"]
    }
}

impl FromWorld for WebmSequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for WebmSequenceLoader {
    type Asset = BuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::WebmVp8,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["webm", "mkv"]
    }
}

fn configured_decode_error(error: MediaDecodeError) -> MediaAssetLoadError {
    match error {
        MediaDecodeError::FrameLimit => MediaAssetLoadError::FrameLimit,
        MediaDecodeError::SequenceLimit => MediaAssetLoadError::DecodedLimit,
        other => MediaAssetLoadError::Decode(other),
    }
}

fn image_from_frame(frame: RgbaFrame) -> Result<Image, MediaAssetLoadError> {
    let expected = usize::try_from(u64::from(frame.width) * u64::from(frame.height) * 4)
        .map_err(|_| MediaAssetLoadError::DecodedLimit)?;
    if frame.pixels.len() != expected {
        return Err(MediaAssetLoadError::Decode(MediaDecodeError::InvalidData(
            "decoded frame buffer length differs from dimensions",
        )));
    }
    Ok(Image::new(
        Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        frame.pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Convert a validated decoded frame into a Bevy image that can be uploaded and sampled by UI.
pub fn rgba_frame_to_image(frame: RgbaFrame) -> Result<Image, MediaAssetLoadError> {
    image_from_frame(frame)
}

async fn read_bounded(
    reader: &mut dyn Reader,
    max_bytes: usize,
) -> Result<Vec<u8>, MediaAssetLoadError> {
    let read_limit = max_bytes
        .checked_add(1)
        .ok_or(MediaAssetLoadError::EncodedLimit)?;
    let mut bytes = Vec::with_capacity(read_limit.min(1024 * 1024));
    reader
        .take(read_limit as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(MediaAssetLoadError::Io)?;
    if bytes.len() > max_bytes {
        return Err(MediaAssetLoadError::EncodedLimit);
    }
    Ok(bytes)
}

fn normalize_duration(duration: Duration) -> Duration {
    duration.max(MIN_FRAME_DURATION)
}

type MediaPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut BuiMediaPlayer,
        &'static mut ImageNode,
        Option<&'static Visibility>,
        Option<&'static InheritedVisibility>,
        Option<&'static bevy::camera::visibility::ViewVisibility>,
    ),
>;

#[derive(SystemParam)]
struct MediaPlayerUpdate<'w, 's> {
    time: Res<'w, Time>,
    sequences: Res<'w, Assets<BuiMediaSequence>>,
    limits: Res<'w, BuiMediaLimits>,
    budget: Res<'w, BuiMediaDecodedBudget>,
    image_events: MessageReader<'w, 's, AssetEvent<Image>>,
    image_reservations: ResMut<'w, DecodedImageBudgetReservations>,
    cache: ResMut<'w, BuiMediaFrameCache>,
    images: ResMut<'w, Assets<Image>>,
}

fn update_media_players(update: MediaPlayerUpdate, mut players: MediaPlayerQuery) {
    let MediaPlayerUpdate {
        time,
        sequences,
        limits,
        budget,
        mut image_events,
        mut image_reservations,
        mut cache,
        mut images,
    } = update;
    for event in image_events.read() {
        match event {
            AssetEvent::Unused { id } | AssetEvent::Removed { id } => {
                image_reservations.by_image.remove(id);
            }
            AssetEvent::Added { .. }
            | AssetEvent::Modified { .. }
            | AssetEvent::LoadedWithDependencies { .. } => {}
        }
    }
    image_reservations
        .by_image
        .retain(|id, _| images.get(*id).is_some());
    let now = time.elapsed();
    let mut visible_video_players = HashSet::new();

    let mut restart_video_players = HashSet::new();
    for (entity, mut player, mut image_node, visibility, inherited, view_visibility) in &mut players
    {
        let sequence_id = player.sequence.id();
        if player.active_sequence != Some(sequence_id) {
            remove_video_stream(&mut cache, entity);
            player.active_sequence = Some(sequence_id);
            player.current_frame = 0;
            player.position = Duration::ZERO;
            player.state = BuiMediaPlayerState::WaitingForAsset;
            image_node.image = ImageNode::default().image;
        }
        let Some(sequence) = sequences.get(&player.sequence) else {
            player.state = BuiMediaPlayerState::WaitingForAsset;
            image_node.image = ImageNode::default().image;
            continue;
        };
        let is_visible = visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
            && inherited.is_none_or(|visibility| visibility.get())
            && view_visibility.is_none_or(|visibility| visibility.get());

        if let Some(source) = sequence.encoded_source.as_ref() {
            if !is_visible {
                player.state = BuiMediaPlayerState::Hidden;
                image_node.image = ImageNode::default().image;
                continue;
            }
            visible_video_players.insert(entity);
            if let std::collections::hash_map::Entry::Vacant(entry) =
                cache.video_streams.entry(entity)
            {
                let stream = start_source_media_stream(
                    sequence_id,
                    Arc::clone(source),
                    sequence.kind,
                    *limits,
                    now,
                );
                entry.insert(stream);
            }
            if let Some(stream) = cache.video_streams.get_mut(&entity) {
                stream.last_used = now;
            }
            continue;
        }

        if sequence.frames.is_empty() {
            player.state = BuiMediaPlayerState::Failed("sequence has no frames".into());
            continue;
        }
        player.state = BuiMediaPlayerState::Ready;
        if player.playing && is_visible {
            advance_player(&mut player, sequence.duration(), time.delta());
        }
        let frame_index = sequence.frame_at(player.position);
        player.current_frame = frame_index;
        let frame = &sequence.frames[frame_index];
        if image_node.image != *frame {
            image_node.image = frame.clone();
        }
    }

    let hidden_video_players = cache
        .video_streams
        .keys()
        .filter(|entity| !visible_video_players.contains(entity))
        .copied()
        .collect::<Vec<_>>();
    for entity in hidden_video_players {
        remove_video_stream(&mut cache, entity);
    }

    poll_h264_video_streams(
        &mut cache,
        &mut images,
        &budget,
        &mut image_reservations,
        *limits,
        now,
    );

    for (entity, mut player, mut image_node, visibility, inherited, view_visibility) in &mut players
    {
        let Some(sequence) = sequences.get(&player.sequence) else {
            continue;
        };
        let Some(_) = sequence.encoded_source else {
            continue;
        };
        let is_visible = visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
            && inherited.is_none_or(|visibility| visibility.get())
            && view_visibility.is_none_or(|visibility| visibility.get());
        if !is_visible {
            continue;
        }
        let Some(stream) = cache.video_streams.get_mut(&entity) else {
            player.state = BuiMediaPlayerState::Decoding;
            continue;
        };
        if let Some(error) = stream.failure.as_ref() {
            player.state = BuiMediaPlayerState::Failed(error.clone());
            image_node.image = ImageNode::default().image;
            continue;
        }
        if stream.frames.is_empty() {
            player.state = BuiMediaPlayerState::Decoding;
            continue;
        }
        if stream.eof
            && let Some(duration) = stream.duration
        {
            let previous_position = player.position;
            if player.playing {
                advance_player(&mut player, duration, time.delta());
            }
            if player.looping && player.playing && player.position < previous_position {
                player.state = BuiMediaPlayerState::Decoding;
                image_node.image = ImageNode::default().image;
                restart_video_players.insert(entity);
                continue;
            }
        } else if player.playing {
            player.position = player.position.saturating_add(time.delta());
        }

        let desired_ns = player.position.as_nanos().min(u128::from(u64::MAX)) as u64;
        let oldest_timestamp = stream.frames.front().map(|frame| frame.timestamp);
        if player.position.is_zero() && stream.frames.front().is_some_and(|frame| frame.index > 0) {
            player.state = BuiMediaPlayerState::Decoding;
            image_node.image = ImageNode::default().image;
            restart_video_players.insert(entity);
            continue;
        }
        if oldest_timestamp.is_some_and(|timestamp| player.position < timestamp)
            && stream.frames.front().is_some_and(|frame| frame.index > 0)
        {
            player.state = BuiMediaPlayerState::Decoding;
            image_node.image = ImageNode::default().image;
            restart_video_players.insert(entity);
            continue;
        }
        let selected = stream
            .frames
            .iter()
            .rev()
            .find(|frame| frame.timestamp.as_nanos() <= u128::from(desired_ns))
            .or_else(|| stream.frames.front())
            .expect("non-empty video frame cache was checked above");
        player.current_frame = selected.index;
        stream.display_index = selected.index;
        if image_node.image != selected.image {
            image_node.image = selected.image.clone();
        }
        player.state = BuiMediaPlayerState::Ready;
        stream.last_used = now;
        let current_index = selected.index;
        while stream
            .frames
            .front()
            .is_some_and(|frame| frame.index < current_index)
        {
            if let Some(frame) = stream.frames.pop_front() {
                stream.cached_bytes = stream.cached_bytes.saturating_sub(frame.byte_len);
                drop(frame.image);
            }
        }
    }
    for entity in restart_video_players {
        remove_video_stream(&mut cache, entity);
    }
}

fn start_source_media_stream(
    sequence_id: AssetId<BuiMediaSequence>,
    source: Arc<[u8]>,
    kind: MediaSequenceKind,
    limits: BuiMediaLimits,
    now: Duration,
) -> StreamingVideo {
    let (sender, receiver) = sync_channel(STREAMED_VIDEO_CHANNEL_FRAMES);
    let cancellation = Arc::new(AtomicBool::new(false));
    let task_cancellation = Arc::clone(&cancellation);
    let task = std::thread::Builder::new()
        .name("bui-media-decode".to_owned())
        .spawn(move || {
            let mut emit = |frame| send_stream_frame(&sender, &task_cancellation, frame);
            let result = match kind {
                MediaSequenceKind::Gif => decode_gif_stream_cancellable(
                    &source,
                    &task_cancellation,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
                MediaSequenceKind::IvfVp8 => decode_ivf_vp8_stream_cancellable(
                    &source,
                    &task_cancellation,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
                MediaSequenceKind::WebmVp8 => decode_webm_vp8_stream_cancellable(
                    &source,
                    &task_cancellation,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
                MediaSequenceKind::Mp4H264 => decode_mp4_h264_stream_cancellable(
                    Arc::clone(&source),
                    &task_cancellation,
                    limits.max_encoded_bytes,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
            };
            let completion = result
                .map(|_| ())
                .map_err(|error| configured_decode_error(error).to_string());
            let _ = sender.send(VideoStreamMessage::Finished(completion));
        })
        .expect("failed to spawn H.264 decode worker");
    StreamingVideo {
        sequence_id,
        receiver: Mutex::new(receiver),
        task: Some(task),
        cancellation,
        frames: VecDeque::new(),
        pending_frame: None,
        next_index: 0,
        frame_capacity: 0,
        cached_bytes: 0,
        display_index: 0,
        eof: false,
        duration: None,
        failure: None,
        last_used: now,
        pending_budget_waits: 0,
    }
}

fn send_stream_frame(
    sender: &SyncSender<VideoStreamMessage>,
    cancellation: &AtomicBool,
    frame: crate::VideoFrame,
) -> bool {
    match sender.send(VideoStreamMessage::Frame(frame)) {
        Ok(()) => true,
        Err(_) => {
            cancellation.store(true, Ordering::Release);
            false
        }
    }
}

fn remove_video_stream(cache: &mut BuiMediaFrameCache, owner: Entity) {
    let Some(mut stream) = cache.video_streams.remove(&owner) else {
        return;
    };
    stream.cancellation.store(true, Ordering::Release);
    if stream.task.take().is_some() && !stream.eof {
        cache.cancelled_jobs = cache.cancelled_jobs.saturating_add(1);
    }
    stream.frames.clear();
}

fn poll_h264_video_streams(
    cache: &mut BuiMediaFrameCache,
    images: &mut Assets<Image>,
    budget: &BuiMediaDecodedBudget,
    image_reservations: &mut DecodedImageBudgetReservations,
    limits: BuiMediaLimits,
    now: Duration,
) {
    let mut messages = Vec::new();
    for (owner, stream) in &mut cache.video_streams {
        if stream.failure.is_some() || stream.eof {
            continue;
        }
        if let Some(frame) = stream.pending_frame.take() {
            let byte_len = frame.frame.pixels.len();
            if stream.frames.len() < stream.frame_capacity
                && stream.cached_bytes.saturating_add(byte_len) <= limits.max_sequence_bytes
            {
                messages.push((*owner, VideoStreamMessage::Frame(frame)));
            } else {
                stream.pending_frame = Some(frame);
            }
            continue;
        }
        let message = stream
            .receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_recv();
        match message {
            Ok(message) => messages.push((*owner, message)),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
    }

    for (owner, message) in messages {
        match message {
            VideoStreamMessage::Frame(frame) => {
                let byte_len = frame.frame.pixels.len();
                if byte_len > limits.max_sequence_bytes {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.failure = Some(MediaAssetLoadError::DecodedLimit.to_string());
                        stream.cancellation.store(true, Ordering::Release);
                    }
                    continue;
                }
                let frame_capacity = (limits.max_sequence_bytes / byte_len.max(1))
                    .min(MAX_STREAMED_VIDEO_CACHE_FRAMES);
                if frame_capacity < 2 {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.failure = Some(
                            "configured decoded byte limit must hold at least two video frames"
                                .to_owned(),
                        );
                        stream.cancellation.store(true, Ordering::Release);
                    }
                    continue;
                }
                let at_capacity = cache.video_streams.get(&owner).is_some_and(|stream| {
                    stream.frames.len() >= frame_capacity
                        || stream.cached_bytes.saturating_add(byte_len) > limits.max_sequence_bytes
                });
                if at_capacity {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.frame_capacity = frame_capacity;
                        stream.pending_frame = Some(frame);
                    }
                    continue;
                }
                if byte_len > budget.max_bytes() {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.failure = Some(MediaAssetLoadError::GlobalDecodedLimit.to_string());
                        stream.cancellation.store(true, Ordering::Release);
                    }
                    continue;
                }
                if !make_room_for_video_frame(cache, budget, byte_len, owner) {
                    defer_video_frame_for_budget(cache, owner, frame);
                    continue;
                }
                let Some(reservation) = budget.try_reserve(byte_len) else {
                    defer_video_frame_for_budget(cache, owner, frame);
                    continue;
                };
                match image_from_frame(frame.frame) {
                    Ok(image) => {
                        let image = images.add(image);
                        image_reservations.by_image.insert(image.id(), reservation);
                        if let Some(stream) = cache.video_streams.get_mut(&owner) {
                            stream.frame_capacity = frame_capacity;
                            stream.cached_bytes = stream.cached_bytes.saturating_add(byte_len);
                            stream.frames.push_back(CachedVideoFrame {
                                index: stream.next_index,
                                timestamp: Duration::from_nanos(frame.timestamp_ns),
                                duration: Duration::from_nanos(
                                    frame
                                        .duration_ns
                                        .unwrap_or(DEFAULT_VIDEO_FRAME_DURATION.as_nanos() as u64),
                                ),
                                byte_len,
                                image,
                            });
                            stream.next_index = stream.next_index.saturating_add(1);
                            stream.pending_budget_waits = 0;
                            stream.last_used = now;
                        }
                    }
                    Err(error) => {
                        if let Some(stream) = cache.video_streams.get_mut(&owner) {
                            stream.failure = Some(error.to_string());
                            stream.cancellation.store(true, Ordering::Release);
                        }
                    }
                }
            }
            VideoStreamMessage::Finished(result) => {
                if let Some(stream) = cache.video_streams.get_mut(&owner) {
                    match result {
                        Ok(()) => {
                            stream.eof = true;
                            stream.duration = stream
                                .frames
                                .back()
                                .map(|frame| frame.timestamp.saturating_add(frame.duration));
                        }
                        Err(error) => {
                            stream.failure = Some(error);
                        }
                    }
                }
            }
        }
    }

    for stream in cache.video_streams.values_mut() {
        if stream
            .task
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(task) = stream.task.take()
        {
            let _ = task.join();
        }
    }
}

fn defer_video_frame_for_budget(
    cache: &mut BuiMediaFrameCache,
    owner: Entity,
    frame: crate::VideoFrame,
) {
    let Some(stream) = cache.video_streams.get_mut(&owner) else {
        return;
    };
    let waits = stream.pending_budget_waits.saturating_add(1);
    if waits > 8 {
        stream.failure = Some(MediaAssetLoadError::GlobalDecodedLimit.to_string());
        stream.cancellation.store(true, Ordering::Release);
    } else {
        stream.pending_budget_waits = waits;
        stream.pending_frame = Some(frame);
    }
}

fn make_room_for_video_frame(
    cache: &mut BuiMediaFrameCache,
    budget: &BuiMediaDecodedBudget,
    bytes: usize,
    protected_owner: Entity,
) -> bool {
    while budget.available_bytes() < bytes {
        let inactive_video_frame = cache
            .video_streams
            .iter()
            .filter(|(owner, stream)| {
                **owner != protected_owner
                    && stream
                        .frames
                        .front()
                        .is_some_and(|frame| frame.index < stream.display_index)
            })
            .min_by_key(|(_, stream)| stream.last_used)
            .map(|(owner, _)| *owner);
        let Some(owner) = inactive_video_frame else {
            return false;
        };
        if let Some(stream) = cache.video_streams.get_mut(&owner)
            && let Some(frame) = stream.frames.pop_front()
        {
            stream.cached_bytes = stream.cached_bytes.saturating_sub(frame.byte_len);
            drop(frame.image);
        }
    }
    true
}

fn advance_player(player: &mut BuiMediaPlayer, duration: Duration, delta: Duration) {
    if duration.is_zero() {
        return;
    }
    player.position = player.position.saturating_add(delta);
    if player.position >= duration {
        if player.looping {
            let position_ns = player.position.as_nanos() % duration.as_nanos();
            player.position = Duration::new(
                (position_ns / 1_000_000_000) as u64,
                (position_ns % 1_000_000_000) as u32,
            );
        } else {
            player.position = duration.saturating_sub(MIN_FRAME_DURATION);
            player.playing = false;
        }
    }
}

#[derive(Debug)]
pub enum MediaAssetLoadError {
    Io(io::Error),
    Decode(MediaDecodeError),
    EncodedLimit,
    DecodedLimit,
    FrameLimit,
    GlobalDecodedLimit,
}

impl fmt::Display for MediaAssetLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read media asset: {error}"),
            Self::Decode(error) => error.fmt(f),
            Self::EncodedLimit => write!(f, "encoded asset exceeds the configured byte limit"),
            Self::DecodedLimit => write!(f, "decoded media exceeds the configured byte limit"),
            Self::FrameLimit => write!(f, "media exceeds the configured frame count limit"),
            Self::GlobalDecodedLimit => {
                write!(
                    f,
                    "loaded media exceeds the global decoded-frame memory budget"
                )
            }
        }
    }
}

impl Error for MediaAssetLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::EncodedLimit
            | Self::DecodedLimit
            | Self::FrameLimit
            | Self::GlobalDecodedLimit => None,
        }
    }
}

impl From<MediaDecodeError> for MediaAssetLoadError {
    fn from(error: MediaDecodeError) -> Self {
        Self::Decode(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_gif;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::{App, AssetPlugin, ImagePlugin, MinimalPlugins};
    use image::ImageEncoder;
    use std::{
        fs, thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    const VP8_KEYFRAME_16X16: &[u8] = &[
        48, 1, 0, 157, 1, 42, 16, 0, 16, 0, 1, 64, 38, 37, 160, 0, 3, 112, 0, 254, 240, 83, 31,
        255, 226, 223, 180, 127, 123, 101, 7, 255, 253, 251, 75, 255, 240, 39, 127, 255, 2, 119,
        253, 12, 0, 0,
    ];

    fn long_ivf_vp8_fixture(frame_count: usize) -> Vec<u8> {
        let mut ivf = Vec::with_capacity(32 + frame_count * (12 + VP8_KEYFRAME_16X16.len()));
        ivf.extend_from_slice(b"DKIF");
        ivf.extend_from_slice(&0u16.to_le_bytes());
        ivf.extend_from_slice(&32u16.to_le_bytes());
        ivf.extend_from_slice(b"VP80");
        ivf.extend_from_slice(&16u16.to_le_bytes());
        ivf.extend_from_slice(&16u16.to_le_bytes());
        ivf.extend_from_slice(&30u32.to_le_bytes());
        ivf.extend_from_slice(&1u32.to_le_bytes());
        ivf.extend_from_slice(
            &u32::try_from(frame_count)
                .expect("fixture frame count fits in IVF header")
                .to_le_bytes(),
        );
        ivf.extend_from_slice(&0u32.to_le_bytes());
        for timestamp in 0..frame_count {
            ivf.extend_from_slice(&(VP8_KEYFRAME_16X16.len() as u32).to_le_bytes());
            ivf.extend_from_slice(
                &u64::try_from(timestamp)
                    .expect("fixture timestamp fits in IVF packet")
                    .to_le_bytes(),
            );
            ivf.extend_from_slice(VP8_KEYFRAME_16X16);
        }
        ivf
    }

    fn long_gif_fixture(frame_count: usize) -> Vec<u8> {
        let frames = (0..frame_count).map(|index| {
            let color = image::Rgba([
                (index % 255) as u8,
                ((index * 17) % 255) as u8,
                ((index * 43) % 255) as u8,
                255,
            ]);
            image::Frame::from_parts(
                image::RgbaImage::from_pixel(2, 2, color),
                0,
                0,
                image::Delay::from_numer_denom_ms(10, 1),
            )
        });
        let mut bytes = Vec::new();
        image::codecs::gif::GifEncoder::new(&mut bytes)
            .encode_frames(frames)
            .expect("long GIF fixture encodes");
        bytes
    }

    fn assert_source_stream_is_bounded(
        kind: MediaSequenceKind,
        source: Vec<u8>,
        frame_count: usize,
        frame_bytes: usize,
    ) {
        let max_cache_bytes = frame_bytes * MAX_STREAMED_VIDEO_CACHE_FRAMES;
        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            max_sequence_bytes: max_cache_bytes,
            max_global_decoded_bytes: max_cache_bytes,
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            BuiMediaPlugin,
        ));
        let sequence = app
            .world_mut()
            .resource_mut::<Assets<BuiMediaSequence>>()
            .add(BuiMediaSequence::from_encoded_bytes(source, kind));
        let player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer {
                    playing: false,
                    looping: false,
                    ..BuiMediaPlayer::new(sequence)
                },
                ImageNode::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::VISIBLE,
            ))
            .id();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if app
                .world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .get(&player)
                .is_some_and(|stream| !stream.frames.is_empty())
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{kind:?} stream did not produce its first frame"
            );
            thread::sleep(Duration::from_millis(1));
        }
        app.world_mut()
            .get_mut::<BuiMediaPlayer>(player)
            .unwrap()
            .seek_to(Duration::from_secs(30));

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            app.update();
            let stream = app
                .world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .get(&player)
                .unwrap();
            if stream.eof && stream.next_index == frame_count {
                assert!(stream.frames.len() <= MAX_STREAMED_VIDEO_CACHE_FRAMES);
                assert!(stream.cached_bytes <= max_cache_bytes);
                assert_eq!(
                    app.world()
                        .get::<BuiMediaPlayer>(player)
                        .unwrap()
                        .current_frame,
                    frame_count - 1
                );
                break;
            }
            assert!(
                !matches!(
                    app.world().get::<BuiMediaPlayer>(player).unwrap().state,
                    BuiMediaPlayerState::Failed(_)
                ),
                "{kind:?} stream failed before frame {frame_count}: {:?}",
                app.world().get::<BuiMediaPlayer>(player).unwrap().state
            );
            assert!(
                std::time::Instant::now() < deadline,
                "{kind:?} stream did not finish: decoded={}, cached={}",
                stream.next_index,
                stream.frames.len()
            );
            thread::sleep(Duration::from_millis(1));
        }
        assert!(app.world().resource::<BuiMediaDecodedBudget>().used_bytes() <= max_cache_bytes);
        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
        app.update();
        assert!(
            !app.world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .contains_key(&player)
        );
        wait_for_decoded_budget(&mut app, 0);
    }

    fn wait_for_decoded_budget(app: &mut App, expected_bytes: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.world().resource::<BuiMediaDecodedBudget>().used_bytes() != expected_bytes {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "decoded budget did not settle at {expected_bytes} bytes; currently {}",
                app.world().resource::<BuiMediaDecodedBudget>().used_bytes()
            );
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn frame_conversion_creates_gpu_uploadable_bevy_image() {
        let image = image_from_frame(RgbaFrame::new(1, 1, vec![10, 20, 30, 255]).unwrap()).unwrap();
        assert_eq!(image.width(), 1);
        assert_eq!(image.height(), 1);
        assert_eq!(image.data.as_deref(), Some(&[10, 20, 30, 255][..]));
    }

    #[test]
    fn bundled_loop_gif_is_a_valid_two_frame_animation() {
        let frames = decode_gif(include_bytes!("../assets/loop.gif")).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].frame.width, 2);
        assert_eq!(frames[0].frame.height, 1);
    }

    #[test]
    fn plugin_registers_custom_media_assets_and_loaders() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        assert!(app.world().contains_resource::<Assets<BuiMediaSequence>>());
    }

    #[test]
    fn svg_and_raster_assets_report_load_errors_and_enforce_configured_limits() {
        let root = std::env::temp_dir().join(format!(
            "bui-still-image-loader-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();

        let svg = |width: u32, height: u32| {
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"100%\" height=\"100%\" fill=\"#336699\"/></svg>"
            )
        };
        fs::write(root.join("small.svg"), svg(1, 1)).unwrap();
        fs::write(root.join("oversized.svg"), svg(2, 2)).unwrap();
        fs::write(root.join("broken.svg"), "<svg><broken").unwrap();
        let mut encoded_over_limit = svg(1, 1).into_bytes();
        encoded_over_limit.resize(513, b' ');
        fs::write(root.join("encoded-over-limit.svg"), encoded_over_limit).unwrap();

        let encode_png = |width: u32, height: u32| {
            let mut bytes = Vec::new();
            image::codecs::png::PngEncoder::new(&mut bytes)
                .write_image(
                    &vec![90; (width * height * 4) as usize],
                    width,
                    height,
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap();
            bytes
        };
        fs::write(root.join("small.buiimg"), encode_png(1, 1)).unwrap();
        fs::write(root.join("oversized.buiimg"), encode_png(2, 2)).unwrap();

        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            max_encoded_bytes: 512,
            max_sequence_bytes: 4,
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                ..Default::default()
            },
            ImagePlugin::default(),
            BuiMediaPlugin,
        ));
        app.finish();
        app.cleanup();

        let (small_svg, small_raster, oversized_svg, oversized_raster, broken_svg, encoded_limit) = {
            let server = app.world().resource::<AssetServer>();
            (
                server.load::<Image>("small.svg"),
                server.load::<Image>("small.buiimg"),
                server.load::<Image>("oversized.svg"),
                server.load::<Image>("oversized.buiimg"),
                server.load::<Image>("broken.svg"),
                server.load::<Image>("encoded-over-limit.svg"),
            )
        };
        let ids = [
            small_svg.id(),
            small_raster.id(),
            oversized_svg.id(),
            oversized_raster.id(),
            broken_svg.id(),
            encoded_limit.id(),
        ];
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let states = ids.map(|id| app.world().resource::<AssetServer>().get_load_state(id));
            if states.iter().all(|state| {
                state.as_ref().is_some_and(|state| {
                    matches!(
                        state,
                        bevy::asset::LoadState::Loaded | bevy::asset::LoadState::Failed(_)
                    )
                })
            }) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "bounded SVG/raster assets did not finish loading"
            );
            thread::sleep(Duration::from_millis(2));
        }

        for image in [&small_svg, &small_raster] {
            let image = app
                .world()
                .resource::<Assets<Image>>()
                .get(image)
                .expect("small image should load within the configured byte limit");
            assert_eq!((image.width(), image.height()), (1, 1));
        }
        let asset_server = app.world().resource::<AssetServer>();
        for image in [&oversized_svg, &oversized_raster] {
            let Some(bevy::asset::LoadState::Failed(error)) =
                asset_server.get_load_state(image.id())
            else {
                panic!("oversized decoded image should fail to load");
            };
            assert!(
                error
                    .to_string()
                    .contains("decoded media exceeds the configured byte limit"),
                "unexpected decoded-image error: {error}"
            );
        }
        for (image, expected) in [
            (&broken_svg, "SVG decode failed"),
            (
                &encoded_limit,
                "encoded asset exceeds the configured byte limit",
            ),
        ] {
            let Some(bevy::asset::LoadState::Failed(error)) =
                asset_server.get_load_state(image.id())
            else {
                panic!("invalid/oversized source should fail to load");
            };
            assert!(
                error.to_string().contains(expected),
                "unexpected load error: {error}"
            );
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn h264_mp4_mov_assets_play_frames_pause_offscreen_and_release_cache() {
        let root = std::env::temp_dir().join(format!(
            "bui-h264-sequence-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let mp4_path = root.join("sample.mp4");
        fs::write(&mp4_path, include_bytes!("../assets/sample-h264.mp4")).unwrap();
        let mut mov_bytes = include_bytes!("../assets/sample-h264.mp4").to_vec();
        mov_bytes[8..12].copy_from_slice(b"qt  ");
        fs::write(root.join("sample.mov"), mov_bytes).unwrap();

        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                ..Default::default()
            },
            ImagePlugin::default(),
            BuiMediaPlugin,
        ));
        app.finish();
        app.cleanup();

        let (mp4, mov) = {
            let server = app.world().resource::<AssetServer>();
            (
                server.load::<BuiMediaSequence>("sample.mp4"),
                server.load::<BuiMediaSequence>("sample.mov"),
            )
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !app
            .world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(mp4.id())
            || !app
                .world()
                .resource::<AssetServer>()
                .is_loaded_with_dependencies(mov.id())
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "MP4/MOV sequence assets did not finish loading"
            );
            thread::sleep(Duration::from_millis(5));
        }
        for sequence in [&mp4, &mov] {
            let sequence_asset = app
                .world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(sequence)
                .unwrap();
            assert_eq!(sequence_asset.kind, MediaSequenceKind::Mp4H264);
            assert!(sequence_asset.frames.is_empty());
            assert!(sequence_asset.encoded_source.is_some());
        }

        let sequence_id = mp4.id();
        let baseline_image_count = app.world().resource::<Assets<Image>>().len();
        let player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer {
                    playing: false,
                    ..BuiMediaPlayer::new(mp4.clone())
                },
                ImageNode::default(),
                Visibility::Hidden,
                InheritedVisibility::HIDDEN,
                bevy::camera::visibility::ViewVisibility::HIDDEN,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().state,
            BuiMediaPlayerState::Hidden
        );
        assert_eq!(
            app.world()
                .resource::<BuiMediaFrameCache>()
                .pending_decode_count(),
            0
        );
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            baseline_image_count
        );

        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::VISIBLE;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let state = app
                .world()
                .get::<BuiMediaPlayer>(player)
                .unwrap()
                .state
                .clone();
            if state == BuiMediaPlayerState::Ready {
                break;
            }
            assert!(
                !matches!(state, BuiMediaPlayerState::Failed(_)),
                "visible MP4 failed to decode: {state:?}"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "visible MP4 did not decode: {state:?}"
            );
            thread::sleep(Duration::from_millis(2));
        }

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let stream = app
                .world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .get(&player);
            if stream.is_some_and(|stream| stream.eof && stream.frames.len() == 2) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "two-frame MP4 stream did not reach EOF"
            );
            thread::sleep(Duration::from_millis(2));
        }

        let (frame_ids, second_timestamp) = {
            let cache = app.world().resource::<BuiMediaFrameCache>();
            let stream = &cache.video_streams[&player];
            assert_eq!(stream.sequence_id, sequence_id);
            assert_eq!(stream.frames.len(), 2);
            (
                stream
                    .frames
                    .iter()
                    .map(|frame| frame.image.id())
                    .collect::<Vec<_>>(),
                stream.frames[1].timestamp,
            )
        };
        assert_eq!(
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
            2 * 32 * 18 * 4
        );
        assert_eq!(
            app.world().get::<ImageNode>(player).unwrap().image.id(),
            frame_ids[0]
        );
        {
            let mut playback = app.world_mut().get_mut::<BuiMediaPlayer>(player).unwrap();
            playback.seek_to(second_timestamp + Duration::from_nanos(1));
        }
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        assert_eq!(
            app.world()
                .get::<BuiMediaPlayer>(player)
                .unwrap()
                .current_frame,
            1
        );
        assert_eq!(
            app.world().get::<ImageNode>(player).unwrap().image.id(),
            frame_ids[1]
        );

        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::HIDDEN;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::HIDDEN;
        app.update();
        wait_for_decoded_budget(&mut app, 0);
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().state,
            BuiMediaPlayerState::Hidden
        );
        assert_eq!(
            app.world().get::<ImageNode>(player).unwrap().image,
            ImageNode::default().image
        );
        assert_eq!(
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
            0
        );
        assert!(
            app.world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(sequence_id)
                .is_some(),
            "the encoded sequence must remain AssetServer-owned while handles are strong"
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while frame_ids
            .iter()
            .any(|id| app.world().resource::<Assets<Image>>().get(*id).is_some())
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "hidden H.264 cache retained Bevy Image assets"
            );
            thread::sleep(Duration::from_millis(2));
        }

        app.world_mut()
            .get_mut::<BuiMediaPlayer>(player)
            .unwrap()
            .sequence = mov.clone();
        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::VISIBLE;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let state = app
                .world()
                .get::<BuiMediaPlayer>(player)
                .unwrap()
                .state
                .clone();
            if state == BuiMediaPlayerState::Ready {
                break;
            }
            assert!(
                !matches!(state, BuiMediaPlayerState::Failed(_)),
                "visible MOV failed to decode: {state:?}"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "visible MOV did not decode: {state:?}"
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().state,
            BuiMediaPlayerState::Ready
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn h264_asset_streams_long_video_with_bounded_cache_seek_loop_and_cancel() {
        let root = std::env::temp_dir().join(format!(
            "bui-h264-long-stream-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("long.mp4"),
            include_bytes!("../assets/sample-h264-long.mp4"),
        )
        .unwrap();

        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            ..Default::default()
        });
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(20),
        ));
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: root.to_string_lossy().into_owned(),
                ..Default::default()
            },
            ImagePlugin::default(),
            BuiMediaPlugin,
        ));
        app.finish();
        app.cleanup();

        let sequence = app
            .world()
            .resource::<AssetServer>()
            .load::<BuiMediaSequence>("long.mp4");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !app
            .world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(sequence.id())
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "long H.264 sequence asset did not load"
            );
            thread::sleep(Duration::from_millis(2));
        }

        let player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer {
                    playing: false,
                    looping: false,
                    ..BuiMediaPlayer::new(sequence.clone())
                },
                ImageNode::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::VISIBLE,
            ))
            .id();

        app.update();
        let ready_deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let state = &app.world().get::<BuiMediaPlayer>(player).unwrap().state;
            if *state == BuiMediaPlayerState::Ready {
                break;
            }
            assert!(
                !matches!(state, BuiMediaPlayerState::Failed(_)),
                "long H.264 stream failed before its first frame: {state:?}"
            );
            assert!(
                std::time::Instant::now() < ready_deadline,
                "long H.264 stream did not produce its first frame"
            );
            thread::sleep(Duration::from_millis(1));
        }

        // Seeking past the end makes the player keep consuming the stream as frames arrive. The
        // decoder must reach all 601 frames without collecting them into one sequence allocation.
        app.world_mut()
            .get_mut::<BuiMediaPlayer>(player)
            .unwrap()
            .seek_to(Duration::from_secs(30));
        let decode_deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            app.update();
            let cache = app.world().resource::<BuiMediaFrameCache>();
            let stream = cache.video_streams.get(&player).unwrap();
            if stream.eof && stream.next_index == 601 {
                assert_eq!(
                    app.world()
                        .get::<BuiMediaPlayer>(player)
                        .unwrap()
                        .current_frame,
                    600
                );
                assert!(stream.frames.len() <= MAX_STREAMED_VIDEO_CACHE_FRAMES);
                assert!(stream.cached_bytes <= MAX_STREAMED_VIDEO_CACHE_FRAMES * 32 * 18 * 4);
                assert!(cache.cached_image_count() <= MAX_STREAMED_VIDEO_CACHE_FRAMES);
                assert!(
                    app.world().resource::<BuiMediaDecodedBudget>().used_bytes()
                        <= MAX_STREAMED_VIDEO_CACHE_FRAMES * 32 * 18 * 4
                );
                break;
            }
            assert!(
                !matches!(
                    app.world().get::<BuiMediaPlayer>(player).unwrap().state,
                    BuiMediaPlayerState::Failed(_)
                ),
                "long H.264 stream failed before EOF"
            );
            assert!(
                std::time::Instant::now() < decode_deadline,
                "long H.264 stream did not decode all frames; next={}, cached={}",
                stream.next_index,
                stream.frames.len()
            );
            thread::sleep(Duration::from_millis(1));
        }

        let duration = app.world().resource::<BuiMediaFrameCache>().video_streams[&player]
            .duration
            .unwrap();
        {
            let mut playback = app.world_mut().get_mut::<BuiMediaPlayer>(player).unwrap();
            playback.looping = true;
            playback.playing = true;
            playback.seek_to(duration.saturating_sub(Duration::from_millis(10)));
        }
        app.update();
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().position,
            Duration::from_millis(10),
            "looping preserves the wrapped timeline position"
        );
        assert!(
            !app.world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .contains_key(&player),
            "looping must restart the source-backed stream"
        );

        // The loop restart begins at the source start. Advance into the restarted stream, then
        // seek backward and verify that the decoder is cancelled/recreated at frame zero.
        app.world_mut()
            .get_mut::<BuiMediaPlayer>(player)
            .unwrap()
            .playing = false;
        let restart_deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let state = &app.world().get::<BuiMediaPlayer>(player).unwrap().state;
            if *state == BuiMediaPlayerState::Ready {
                break;
            }
            assert!(
                !matches!(state, BuiMediaPlayerState::Failed(_)),
                "loop restart failed: {state:?}"
            );
            assert!(std::time::Instant::now() < restart_deadline);
            thread::sleep(Duration::from_millis(1));
        }
        app.world_mut()
            .get_mut::<BuiMediaPlayer>(player)
            .unwrap()
            .seek_to(Duration::from_secs(1));
        let forward_deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if app
                .world()
                .get::<BuiMediaPlayer>(player)
                .unwrap()
                .current_frame
                > 0
            {
                break;
            }
            assert!(std::time::Instant::now() < forward_deadline);
            thread::sleep(Duration::from_millis(1));
        }
        app.world_mut()
            .get_mut::<BuiMediaPlayer>(player)
            .unwrap()
            .seek_to(Duration::ZERO);
        app.update();
        assert!(
            !app.world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .contains_key(&player),
            "backward seek outside the rolling cache must restart decoding"
        );
        let backward_deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let playback = app.world().get::<BuiMediaPlayer>(player).unwrap();
            if playback.state == BuiMediaPlayerState::Ready && playback.current_frame == 0 {
                break;
            }
            assert!(
                !matches!(playback.state, BuiMediaPlayerState::Failed(_)),
                "backward seek restart failed: {:?}",
                playback.state
            );
            assert!(std::time::Instant::now() < backward_deadline);
            thread::sleep(Duration::from_millis(1));
        }

        // Hiding cancels the active stream and drops its Bevy images while preserving the strong
        // sequence asset. Revealing starts a fresh stream at the paused timeline position.
        {
            let mut playback = app.world_mut().get_mut::<BuiMediaPlayer>(player).unwrap();
            playback.playing = false;
            playback.seek_to(Duration::from_millis(500));
        }
        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::HIDDEN;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::HIDDEN;
        app.update();
        wait_for_decoded_budget(&mut app, 0);
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().state,
            BuiMediaPlayerState::Hidden
        );
        assert_eq!(
            app.world()
                .resource::<BuiMediaFrameCache>()
                .cached_image_count(),
            0
        );
        assert_eq!(
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
            0
        );
        assert!(
            app.world()
                .resource::<BuiMediaFrameCache>()
                .cancelled_job_count()
                > 0
        );
        assert!(
            app.world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(&sequence)
                .is_some()
        );

        let hidden_position = app.world().get::<BuiMediaPlayer>(player).unwrap().position;
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().position,
            hidden_position,
            "hidden playback does not advance"
        );
        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::VISIBLE;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
        let reveal_deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let playback = app.world().get::<BuiMediaPlayer>(player).unwrap();
            if playback.state == BuiMediaPlayerState::Ready {
                assert_eq!(playback.position, hidden_position);
                break;
            }
            assert!(
                !matches!(playback.state, BuiMediaPlayerState::Failed(_)),
                "revealed H.264 stream failed: {:?}",
                playback.state
            );
            assert!(std::time::Instant::now() < reveal_deadline);
            thread::sleep(Duration::from_millis(1));
        }

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hidden_encoded_player_does_not_decode_and_releases_images_while_handle_stays_strong() {
        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            BuiMediaPlugin,
        ));
        let sequence = app
            .world_mut()
            .resource_mut::<Assets<BuiMediaSequence>>()
            .add(BuiMediaSequence::from_encoded_bytes(
                include_bytes!("../assets/loop.gif").to_vec(),
                MediaSequenceKind::Gif,
            ));
        let sequence_id = sequence.id();
        let baseline_image_count = app.world().resource::<Assets<Image>>().len();
        let player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer::new(sequence.clone()),
                ImageNode::default(),
                Visibility::Hidden,
                InheritedVisibility::HIDDEN,
                bevy::camera::visibility::ViewVisibility::HIDDEN,
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().state,
            BuiMediaPlayerState::Hidden
        );
        assert_eq!(
            app.world()
                .resource::<BuiMediaFrameCache>()
                .pending_decode_count(),
            0
        );
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            baseline_image_count
        );

        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::VISIBLE;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !app
            .world()
            .resource::<BuiMediaFrameCache>()
            .video_streams
            .get(&player)
            .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "visible GIF did not decode"
            );
            thread::sleep(Duration::from_millis(2));
        }
        let frame_ids = app.world().resource::<BuiMediaFrameCache>().video_streams[&player]
            .frames
            .iter()
            .map(|frame| frame.image.id())
            .collect::<Vec<_>>();
        assert_eq!(frame_ids.len(), 2);
        let external_image_handle = app.world().get::<ImageNode>(player).unwrap().image.clone();
        let externally_held_bytes = app.world().resource::<BuiMediaFrameCache>().video_streams
            [&player]
            .frames
            .iter()
            .find(|frame| frame.image.id() == external_image_handle.id())
            .unwrap()
            .byte_len;
        assert!(app.world().resource::<BuiMediaDecodedBudget>().used_bytes() > 0);

        *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(player)
            .unwrap() = InheritedVisibility::HIDDEN;
        app.update();
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(player).unwrap().state,
            BuiMediaPlayerState::Hidden
        );
        assert_eq!(
            app.world().get::<ImageNode>(player).unwrap().image,
            ImageNode::default().image
        );
        wait_for_decoded_budget(&mut app, externally_held_bytes);
        assert!(
            app.world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(sequence_id)
                .is_some()
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(external_image_handle.id())
                .is_some()
        );
        drop(external_image_handle);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while frame_ids
            .iter()
            .any(|id| app.world().resource::<Assets<Image>>().get(*id).is_some())
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "Bevy did not release hidden cache-owned frame Images"
            );
            thread::sleep(Duration::from_millis(2));
        }
        wait_for_decoded_budget(&mut app, 0);
    }

    #[test]
    fn hidden_video_releases_cached_frames_before_the_next_visible_sequence_decodes() {
        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            max_global_decoded_bytes: 16,
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            BuiMediaPlugin,
        ));
        let make_sequence = || {
            BuiMediaSequence::from_encoded_bytes(
                include_bytes!("../assets/loop.gif").to_vec(),
                MediaSequenceKind::Gif,
            )
        };
        let (first, second) = {
            let mut sequences = app.world_mut().resource_mut::<Assets<BuiMediaSequence>>();
            (
                sequences.add(make_sequence()),
                sequences.add(make_sequence()),
            )
        };
        let first_id = first.id();
        let first_player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer {
                    playing: false,
                    ..BuiMediaPlayer::new(first.clone())
                },
                ImageNode::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::VISIBLE,
            ))
            .id();
        let second_player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer {
                    playing: false,
                    ..BuiMediaPlayer::new(second)
                },
                ImageNode::default(),
                Visibility::Hidden,
                InheritedVisibility::HIDDEN,
                bevy::camera::visibility::ViewVisibility::HIDDEN,
            ))
            .id();

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !app
            .world()
            .resource::<BuiMediaFrameCache>()
            .video_streams
            .get(&first_player)
            .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "first GIF did not decode: state={:?}, cache={}, budget={}",
                app.world()
                    .get::<BuiMediaPlayer>(first_player)
                    .unwrap()
                    .state,
                app.world()
                    .resource::<BuiMediaFrameCache>()
                    .sequence_count(),
                app.world().resource::<BuiMediaDecodedBudget>().used_bytes()
            );
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
            16
        );

        *app.world_mut().get_mut::<Visibility>(first_player).unwrap() = Visibility::Hidden;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(first_player)
            .unwrap() = InheritedVisibility::HIDDEN;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(first_player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::HIDDEN;
        wait_for_decoded_budget(&mut app, 0);
        *app.world_mut()
            .get_mut::<Visibility>(second_player)
            .unwrap() = Visibility::Visible;
        *app.world_mut()
            .get_mut::<InheritedVisibility>(second_player)
            .unwrap() = InheritedVisibility::VISIBLE;
        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(second_player)
            .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;

        while !app
            .world()
            .resource::<BuiMediaFrameCache>()
            .video_streams
            .get(&second_player)
            .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
        {
            app.update();
            assert!(
                std::time::Instant::now() < deadline,
                "second GIF did not decode"
            );
            thread::sleep(Duration::from_millis(2));
        }
        let cache = app.world().resource::<BuiMediaFrameCache>();
        assert_eq!(cache.cached_image_count(), 2);
        assert_eq!(cache.sequence_count(), 1);
        assert_eq!(
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
            16
        );
        assert!(
            app.world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(first_id)
                .is_some()
        );
        assert_eq!(
            app.world()
                .get::<BuiMediaPlayer>(first_player)
                .unwrap()
                .state,
            BuiMediaPlayerState::Hidden
        );
    }

    #[test]
    fn gif_decode_honors_configured_frame_and_byte_limits() {
        let cancelled = AtomicBool::new(false);
        let source = include_bytes!("../assets/loop.gif");
        let mut discard = |_| true;
        let frame_limited =
            decode_gif_stream_cancellable(source, &cancelled, 1, usize::MAX, &mut discard)
                .unwrap_err();
        assert!(matches!(frame_limited, MediaDecodeError::FrameLimit));

        let byte_limited =
            decode_gif_stream_cancellable(source, &cancelled, usize::MAX, 8, &mut discard)
                .unwrap_err();
        assert!(matches!(byte_limited, MediaDecodeError::SequenceLimit));
    }

    #[test]
    fn hidden_player_keeps_its_clock_still_and_shows_current_frame() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        let (first, second) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (images.add(Image::default()), images.add(Image::default()))
        };
        let sequence = app
            .world_mut()
            .resource_mut::<Assets<BuiMediaSequence>>()
            .add(BuiMediaSequence::new(
                vec![first.clone(), second],
                vec![Duration::from_millis(100); 2],
                MediaSequenceKind::Gif,
            ));
        let entity = app
            .world_mut()
            .spawn((
                BuiMediaPlayer::new(sequence),
                ImageNode::default(),
                Visibility::Hidden,
                InheritedVisibility::HIDDEN,
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(150));
        app.update();
        let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert_eq!(player.position, Duration::ZERO);
        assert_eq!(player.current_frame, 0);
        assert_eq!(image.image, first);
    }

    #[test]
    fn player_pauses_for_view_visibility_and_manual_pause_then_resumes() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        let (first, second) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (images.add(Image::default()), images.add(Image::default()))
        };
        let sequence = app
            .world_mut()
            .resource_mut::<Assets<BuiMediaSequence>>()
            .add(BuiMediaSequence::new(
                vec![first.clone(), second.clone()],
                vec![Duration::from_millis(100); 2],
                MediaSequenceKind::Gif,
            ));
        let entity = app
            .world_mut()
            .spawn((
                BuiMediaPlayer::new(sequence),
                ImageNode::default(),
                Visibility::Inherited,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::HIDDEN,
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(150));
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        {
            let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
            let image = app.world().get::<ImageNode>(entity).unwrap();
            assert_eq!(player.position, Duration::ZERO);
            assert_eq!(player.current_frame, 0);
            assert_eq!(image.image, first);
        }

        *app.world_mut()
            .get_mut::<bevy::camera::visibility::ViewVisibility>(entity)
            .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
        app.world_mut()
            .get_mut::<BuiMediaPlayer>(entity)
            .unwrap()
            .playing = false;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(150));
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(entity).unwrap().position,
            Duration::ZERO
        );

        app.world_mut()
            .get_mut::<BuiMediaPlayer>(entity)
            .unwrap()
            .playing = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(150));
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        {
            let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
            let image = app.world().get::<ImageNode>(entity).unwrap();
            assert_eq!(player.position, Duration::from_millis(150));
            assert_eq!(player.current_frame, 1);
            assert_eq!(image.image, second);
        }

        *app.world_mut()
            .get_mut::<InheritedVisibility>(entity)
            .unwrap() = InheritedVisibility::HIDDEN;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        assert_eq!(
            app.world().get::<BuiMediaPlayer>(entity).unwrap().position,
            Duration::from_millis(150)
        );

        *app.world_mut()
            .get_mut::<InheritedVisibility>(entity)
            .unwrap() = InheritedVisibility::VISIBLE;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert_eq!(player.position, Duration::from_millis(50));
        assert_eq!(player.current_frame, 0);
        assert_eq!(image.image, first);
    }

    #[test]
    fn sequence_timeline_clamps_zero_delays_and_selects_frame() {
        let sequence = BuiMediaSequence::new(
            vec![Handle::default(), Handle::default()],
            vec![Duration::ZERO, Duration::from_millis(20)],
            MediaSequenceKind::Gif,
        );
        assert_eq!(sequence.duration(), Duration::from_millis(30));
        assert_eq!(sequence.frame_at(Duration::from_millis(5)), 0);
        assert_eq!(sequence.frame_at(Duration::from_millis(15)), 1);

        let missing_durations = BuiMediaSequence::new(
            vec![Handle::default(), Handle::default()],
            Vec::new(),
            MediaSequenceKind::Gif,
        );
        assert_eq!(
            missing_durations.duration(),
            DEFAULT_VIDEO_FRAME_DURATION.saturating_mul(2)
        );
        assert_eq!(missing_durations.frame_at(Duration::from_millis(20)), 0);
        assert_eq!(missing_durations.frame_at(Duration::from_millis(40)), 1);

        let partial_durations = BuiMediaSequence::new(
            vec![Handle::default(), Handle::default()],
            vec![Duration::ZERO],
            MediaSequenceKind::Gif,
        );
        assert_eq!(
            partial_durations.duration(),
            MIN_FRAME_DURATION.saturating_add(DEFAULT_VIDEO_FRAME_DURATION)
        );
        assert_eq!(partial_durations.frame_at(Duration::from_millis(15)), 1);
    }

    #[test]
    fn player_with_no_frame_durations_advances_without_zero_modulo() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        let (first, second) = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            (images.add(Image::default()), images.add(Image::default()))
        };
        let sequence = app
            .world_mut()
            .resource_mut::<Assets<BuiMediaSequence>>()
            .add(BuiMediaSequence::new(
                vec![first, second.clone()],
                Vec::new(),
                MediaSequenceKind::Gif,
            ));
        let entity = app
            .world_mut()
            .spawn((
                BuiMediaPlayer::new(sequence),
                ImageNode::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::VISIBLE,
            ))
            .id();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(40));
        app.world_mut()
            .run_system_once(update_media_players)
            .unwrap();
        let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert_eq!(player.position, Duration::from_millis(40));
        assert_eq!(player.current_frame, 1);
        assert_eq!(image.image, second);
    }

    #[test]
    fn asset_server_releases_sequence_frames_and_budget_after_last_reference() {
        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .to_string_lossy()
                    .into_owned(),
                ..Default::default()
            },
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        app.finish();
        app.cleanup();

        let sequence: Handle<BuiMediaSequence> =
            app.world().resource::<AssetServer>().load("loop.gif");
        let sequence_id = sequence.id();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let sequence_asset = app
                .world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(&sequence);
            if let Some(sequence_asset) = sequence_asset
                && app
                    .world()
                    .resource::<AssetServer>()
                    .is_loaded_with_dependencies(sequence_id)
            {
                assert!(sequence_asset.frames.is_empty());
                assert!(sequence_asset.encoded_source.is_some());
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "GIF sequence and frame dependencies did not load before deadline"
            );
            thread::sleep(Duration::from_millis(5));
        }
        wait_for_decoded_budget(&mut app, 0);

        let player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer::new(sequence.clone()),
                ImageNode::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::VISIBLE,
            ))
            .id();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if app
                .world()
                .resource::<BuiMediaFrameCache>()
                .video_streams
                .get(&player)
                .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "visible GIF did not decode"
            );
            thread::sleep(Duration::from_millis(2));
        }
        let frame_ids = app.world().resource::<BuiMediaFrameCache>().video_streams[&player]
            .frames
            .iter()
            .map(|frame| frame.image.id())
            .collect::<Vec<_>>();
        assert_eq!(frame_ids.len(), 2);
        assert!(
            frame_ids
                .iter()
                .all(|id| app.world().resource::<Assets<Image>>().get(*id).is_some())
        );
        assert!(app.world().resource::<BuiMediaDecodedBudget>().used_bytes() > 0);

        app.world_mut().entity_mut(player).despawn();
        drop(sequence);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let sequence_gone = app
                .world()
                .resource::<Assets<BuiMediaSequence>>()
                .get(sequence_id)
                .is_none();
            let frames_gone = frame_ids
                .iter()
                .all(|id| app.world().resource::<Assets<Image>>().get(*id).is_none());
            let budget_released = app.world().resource::<BuiMediaDecodedBudget>().used_bytes() == 0;
            if sequence_gone && frames_gone && budget_released {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "dropped media sequence retained sequence={sequence_gone}, frames={frames_gone}, budget={budget_released}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn gif_player_streams_beyond_eager_limit_with_bounded_cache() {
        const FRAME_COUNT: usize = 601;
        assert_source_stream_is_bounded(
            MediaSequenceKind::Gif,
            long_gif_fixture(FRAME_COUNT),
            FRAME_COUNT,
            2 * 2 * 4,
        );
    }

    #[test]
    fn ivf_vp8_player_streams_beyond_eager_limit_with_bounded_cache() {
        const FRAME_COUNT: usize = 601;
        assert_source_stream_is_bounded(
            MediaSequenceKind::IvfVp8,
            long_ivf_vp8_fixture(FRAME_COUNT),
            FRAME_COUNT,
            16 * 16 * 4,
        );
    }

    #[test]
    fn dropped_unneeded_sequence_load_does_not_retain_decoded_assets() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .to_string_lossy()
                    .into_owned(),
                ..Default::default()
            },
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        app.finish();
        app.cleanup();

        let initial_image_count = app.world().resource::<Assets<Image>>().len();
        let initial_sequence_count = app.world().resource::<Assets<BuiMediaSequence>>().len();
        let sequence: Handle<BuiMediaSequence> =
            app.world().resource::<AssetServer>().load("loop.gif");
        let sequence_id = sequence.id();
        drop(sequence);

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            let load_cancelled = app
                .world()
                .resource::<AssetServer>()
                .get_load_state(sequence_id)
                .is_none();
            let sequences_gone =
                app.world().resource::<Assets<BuiMediaSequence>>().len() == initial_sequence_count;
            let images_gone = app.world().resource::<Assets<Image>>().len() == initial_image_count;
            let budget_empty = app.world().resource::<BuiMediaDecodedBudget>().used_bytes() == 0;
            if load_cancelled && sequences_gone && images_gone && budget_empty {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "dropped sequence load retained load={load_cancelled}, sequences={sequences_gone}, images={images_gone}, budget={budget_empty}"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn decoded_frame_budget_is_atomic_across_concurrent_sequence_loads() {
        const THREADS: usize = 8;
        const RESERVATION_BYTES: usize = 32;
        let budget = BuiMediaDecodedBudget::new(THREADS * RESERVATION_BYTES);
        let (status_send, status_receive) = std::sync::mpsc::channel();

        thread::scope(|scope| {
            let mut releases = Vec::with_capacity(THREADS);
            for _ in 0..THREADS {
                let budget = budget.clone();
                let status_send = status_send.clone();
                let (release_send, release_receive) = std::sync::mpsc::channel();
                releases.push(release_send);
                scope.spawn(move || {
                    let reservation = budget.try_reserve(RESERVATION_BYTES);
                    status_send.send(reservation.is_some()).unwrap();
                    let _ = release_receive.recv_timeout(Duration::from_secs(2));
                    drop(reservation);
                });
            }

            drop(status_send);
            let successful_reservations = (0..THREADS)
                .filter(|_| {
                    status_receive
                        .recv_timeout(Duration::from_secs(3))
                        .expect("each worker reports one reservation within three seconds")
                })
                .count();
            if successful_reservations != THREADS {
                for release in &releases {
                    let _ = release.send(());
                }
                assert_eq!(successful_reservations, THREADS);
            }
            assert_eq!(budget.used_bytes(), THREADS * RESERVATION_BYTES);
            assert!(budget.try_reserve(1).is_none());
            for release in releases {
                let _ = release.send(());
            }
        });

        assert_eq!(budget.used_bytes(), 0);
    }

    #[test]
    fn gif_loader_reports_global_budget_failure_and_releases_reservation() {
        let mut app = App::new();
        app.insert_resource(BuiMediaLimits {
            max_global_decoded_bytes: 1,
            ..Default::default()
        });
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .to_string_lossy()
                    .into_owned(),
                ..Default::default()
            },
            ImagePlugin::default(),
        ));
        app.add_plugins(BuiMediaPlugin);
        app.finish();
        app.cleanup();

        let sequence: Handle<BuiMediaSequence> =
            app.world().resource::<AssetServer>().load("loop.gif");
        let player = app
            .world_mut()
            .spawn((
                BuiMediaPlayer::new(sequence.clone()),
                ImageNode::default(),
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                bevy::camera::visibility::ViewVisibility::VISIBLE,
            ))
            .id();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let failure = loop {
            app.update();
            let state = &app.world().get::<BuiMediaPlayer>(player).unwrap().state;
            if let BuiMediaPlayerState::Failed(message) = state {
                break message.clone();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "visible GIF did not report its decoded-frame budget failure"
            );
            thread::sleep(Duration::from_millis(2));
        };

        assert!(
            failure.contains("global decoded-frame memory budget"),
            "{failure}"
        );
        assert!(matches!(
            app.world()
                .resource::<AssetServer>()
                .get_load_state(sequence.id()),
            Some(bevy::asset::LoadState::Loaded)
        ));
        assert_eq!(
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
            0
        );
    }
}

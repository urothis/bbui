//! Bevy asset loading and playback for BBUI image media.
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
pub struct BbuiMediaLimits {
    /// Maximum encoded file size accepted by a custom media loader.
    pub max_encoded_bytes: usize,
    /// Maximum RGBA bytes retained in one source-backed sequence's rolling frame cache.
    pub max_sequence_bytes: usize,
    /// Maximum decoded frame count accepted from one source-backed sequence.
    pub max_sequence_frames: usize,
    /// Maximum decoded RGBA bytes retained by all cached sequences and still strongly owned frame Images.
    pub max_global_decoded_bytes: usize,
}

impl Default for BbuiMediaLimits {
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
/// [`BbuiMediaLimits::max_global_decoded_bytes`] before adding [`BbuiMediaPlugin`].
#[derive(Resource, Debug, Clone)]
pub struct BbuiMediaDecodedBudget {
    max_bytes: usize,
    state: Arc<DecodedBudgetState>,
}

#[derive(Resource, Default)]
struct DecodedImageBudgetReservations {
    // A reservation follows the Bevy Image asset until its last strong UI/asset handle is dropped.
    by_image: HashMap<AssetId<Image>, DecodedFrameReservation>,
}

impl BbuiMediaDecodedBudget {
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

impl FromWorld for BbuiMediaDecodedBudget {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        let max_bytes = world
            .get_resource::<BbuiMediaLimits>()
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

/// Kind of media stored in a [`BbuiMediaSequence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSequenceKind {
    Gif,
    IvfVp8,
    WebmVp8,
    Mp4H264,
}

/// Metadata for an animation/video sequence.
///
/// Caller-created sequences keep their explicit Bevy `Image` frame handles in `frames`. BBUI's
/// loaders instead retain bounded encoded source bytes here and leave `frames` empty; visible
/// players decode on demand into [`BbuiMediaFrameCache`]. The sequence asset can therefore remain
/// strongly referenced while its decoded CPU/GPU frame cache is evicted.
#[derive(Asset, TypePath, Debug)]
pub struct BbuiMediaSequence {
    /// Explicit frame images in presentation order for caller-created sequences.
    ///
    /// Sequences loaded by BBUI keep encoded source bytes and leave this vector empty; their cached
    /// images are available on the visible player's [`ImageNode`] and are owned by
    /// [`BbuiMediaFrameCache`].
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

impl BbuiMediaSequence {
    /// Create a sequence from frame handles managed by the caller.
    ///
    /// Caller-created sequences are not charged to [`BbuiMediaDecodedBudget`]; only sequences
    /// admitted through BBUI's GIF and video loaders own decoded-frame reservations.
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
    /// [`BbuiMediaDecodedBudget::used_bytes`]. Caller-created sequences report zero.
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

/// Playback and on-demand decode state for a [`BbuiMediaPlayer`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BbuiMediaPlayerState {
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
    sequence_id: AssetId<BbuiMediaSequence>,
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
pub struct BbuiMediaFrameCache {
    video_streams: HashMap<Entity, StreamingVideo>,
    cancelled_jobs: usize,
}

impl BbuiMediaFrameCache {
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

/// Player for a [`BbuiMediaSequence`], intended for an entity with a Bevy [`ImageNode`].
///
/// The clock advances only when the entity is not hidden by `Visibility` or
/// `InheritedVisibility`, and is visible to at least one camera when Bevy supplies
/// `ViewVisibility`. A player can be paused by setting `playing` to `false`; its last frame remains
/// displayed.
#[derive(Component, Debug, Clone)]
pub struct BbuiMediaPlayer {
    /// Sequence asset handle.
    pub sequence: Handle<BbuiMediaSequence>,
    /// Whether playback advances while visible.
    pub playing: bool,
    /// Whether playback wraps from the last frame to the first.
    pub looping: bool,
    /// Current frame index, updated by the playback system.
    pub current_frame: usize,
    /// Position in the sequence timeline.
    pub position: Duration,
    /// Current asset/decode state. The system updates this field.
    pub state: BbuiMediaPlayerState,
    active_sequence: Option<bevy::asset::AssetId<BbuiMediaSequence>>,
}

impl BbuiMediaPlayer {
    pub fn new(sequence: Handle<BbuiMediaSequence>) -> Self {
        Self {
            sequence,
            playing: true,
            looping: true,
            current_frame: 0,
            position: Duration::ZERO,
            state: BbuiMediaPlayerState::WaitingForAsset,
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

/// Registers BBUI media asset loaders and the visibility-aware [`BbuiMediaPlayer`] system.
///
/// Add this after Bevy's `AssetPlugin` and `ImagePlugin` (normally through `DefaultPlugins`).
/// Bevy's built-in `ImageLoader` owns PNG/JPEG/WebP and other enabled raster extensions; this
/// plugin adds `.svg`, `.gif`, `.ivf`, `.webm`, `.mkv`, `.mp4`, and `.mov`.
pub struct BbuiMediaPlugin;

impl Plugin for BbuiMediaPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AssetServer>() {
            bevy::log::error!("BbuiMediaPlugin requires Bevy's AssetPlugin");
            return;
        }
        app.init_resource::<BbuiMediaLimits>()
            .init_resource::<BbuiMediaDecodedBudget>()
            .init_resource::<DecodedImageBudgetReservations>()
            .init_resource::<BbuiMediaFrameCache>()
            .init_asset::<BbuiMediaSequence>()
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

#[path = "bevy_assets_loaders.rs"]
mod loaders;
use loaders::*;
pub use loaders::{RasterSvgImageLoader, rgba_frame_to_image};

#[path = "bevy_assets_players.rs"]
mod players;
use players::*;

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
#[path = "bevy_assets/tests.rs"]
mod tests;

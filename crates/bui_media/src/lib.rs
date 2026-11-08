//! Pure-Rust image, SVG, VP8, and H.264 decoding for BUI assets.
//!
//! Raster still images use the selected `image` crate codecs. SVGs are parsed by usvg and
//! rasterized by resvg into an RGBA frame. IVF and WebM video APIs decode VP8 frames with
//! `vp8-decoder`; MP4 and QuickTime MOV use `oxideav-mp4` plus `rust_h264` for AVC/H.264. Bevy
//! sequence assets stream decoded frames through a bounded rolling cache; poster APIs are also
//! available for file previews. Video is decoded on the CPU and uploaded as normal Bevy images.
//! UI drawing itself remains on Bevy's GPU render pipeline.

use std::{
    convert::TryFrom,
    error::Error,
    fmt,
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use image::{
    AnimationDecoder, GenericImageView, ImageDecoder, ImageReader, Limits, codecs::gif::GifDecoder,
};
use oxideav_core::{Demuxer, MediaType, NullCodecResolver};
use rust_h264::{
    decoder::{ChromaFormat as H264ChromaFormat, Frame as H264Frame, OrderedDecoder},
    nal::{NalUnitType, parse_avcc, parse_avcc_config, parse_sps_info},
};
use vp8_decoder::{Decoder as Vp8Decoder, Picture};
use webm_iterable::{
    WebmIterator,
    matroska_spec::{Block, EbmlTag, Master, MatroskaSpec, SimpleBlock},
};

pub mod bevy_assets;
pub use bevy_assets::{
    BuiMediaDecodedBudget, BuiMediaFrameCache, BuiMediaLimits, BuiMediaPlayer, BuiMediaPlayerState,
    BuiMediaPlugin, BuiMediaSequence, MediaAssetLoadError, MediaSequenceKind, RasterSvgImageLoader,
    rgba_frame_to_image,
};

/// Upper bound for a single decoded image or video frame: 64 megapixels.
pub const MAX_DECODED_PIXELS: u64 = 64 * 1024 * 1024;
/// Maximum decoded frames returned by one animation or video decode call.
pub const MAX_DECODED_FRAMES: usize = 600;
/// Maximum combined decoded RGBA storage returned by one animation or video decode call.
pub const MAX_DECODED_SEQUENCE_BYTES: usize = 256 * 1024 * 1024;
const MAX_THUMBNAIL_SOURCE_DIMENSION: u32 = 4096;
const MAX_THUMBNAIL_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_H264_PARAMETER_BYTES: usize = 64 * 1024;
const MAX_H264_SAMPLE_BYTES: usize = 8 * 1024 * 1024;
const MAX_H264_SAMPLE_NALS: usize = 4096;
const MAX_H264_DEMUX_PACKETS: usize = 512;
const MAX_H264_VIDEO_SAMPLES: usize = 64;
const MAX_H264_SEQUENCE_PACKETS: usize = MAX_DECODED_FRAMES * 4 + 32;
const MAX_H264_STREAM_PACKETS: usize = 1_000_000;
const MAX_STREAMED_MEDIA_FRAMES: usize = 1_000_000;
const MIN_STREAM_FRAME_DURATION: Duration = Duration::from_millis(10);
const DEFAULT_H264_FRAME_DURATION_NS: u64 = 33_333_333;
const MAX_H264_REORDER_DEPTH: u32 = 16;
const MAX_H264_WORKING_LUMA_SAMPLES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    /// Straight-alpha, row-major RGBA8 pixels.
    pub pixels: Vec<u8>,
}

impl RgbaFrame {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, &'static str> {
        let expected = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or("image dimensions overflow")?;
        if pixels.len() != expected {
            return Err("RGBA buffer length differs from dimensions");
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    fn check_dimensions(width: u32, height: u32) -> Result<(), MediaDecodeError> {
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(MediaDecodeError::DimensionsOverflow)?;
        if width == 0 || height == 0 || pixels > MAX_DECODED_PIXELS {
            return Err(MediaDecodeError::PixelLimit);
        }
        Ok(())
    }
}

/// A decoded animation frame and the delay to the next frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationFrame {
    pub frame: RgbaFrame,
    pub delay: Duration,
}

/// A video frame with a presentation timestamp in nanoseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub timestamp_ns: u64,
    pub duration_ns: Option<u64>,
    pub frame: RgbaFrame,
}

#[derive(Debug)]
pub enum MediaDecodeError {
    Raster(image::ImageError),
    RasterIo(std::io::Error),
    Svg(String),
    Video(String),
    Vp8(vp8_decoder::DecodeError),
    InvalidData(&'static str),
    DimensionsOverflow,
    PixelLimit,
    FrameLimit,
    SequenceLimit,
}

impl fmt::Display for MediaDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raster(error) => write!(f, "raster image decode failed: {error}"),
            Self::RasterIo(error) => write!(f, "could not inspect raster image: {error}"),
            Self::Svg(error) => write!(f, "SVG decode failed: {error}"),
            Self::Video(error) => write!(f, "video decode failed: {error}"),
            Self::Vp8(error) => write!(f, "VP8 frame decode failed: {error}"),
            Self::InvalidData(reason) => write!(f, "invalid media data: {reason}"),
            Self::DimensionsOverflow => f.write_str("media dimensions overflow"),
            Self::PixelLimit => write!(f, "media dimensions exceed {MAX_DECODED_PIXELS} pixels"),
            Self::FrameLimit => write!(f, "media exceeds {MAX_DECODED_FRAMES} decoded frames"),
            Self::SequenceLimit => write!(
                f,
                "decoded media exceeds {MAX_DECODED_SEQUENCE_BYTES} bytes"
            ),
        }
    }
}

impl Error for MediaDecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Raster(error) => Some(error),
            Self::RasterIo(error) => Some(error),
            Self::Vp8(error) => Some(error),
            _ => None,
        }
    }
}

/// Decode the first frame of a supported raster image (PNG, JPEG, WebP, BMP, TGA, QOI, HDR, or PNM).
pub fn decode_image(bytes: &[u8]) -> Result<RgbaFrame, MediaDecodeError> {
    decode_image_with_max_bytes(bytes, MAX_DECODED_SEQUENCE_BYTES)
}

pub(crate) fn decode_image_with_max_bytes(
    bytes: &[u8],
    max_decoded_bytes: usize,
) -> Result<RgbaFrame, MediaDecodeError> {
    let max_decoded_bytes = max_decoded_bytes.min(MAX_DECODED_SEQUENCE_BYTES);
    let dimensions = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(MediaDecodeError::RasterIo)?
        .into_dimensions()
        .map_err(MediaDecodeError::Raster)?;
    RgbaFrame::check_dimensions(dimensions.0, dimensions.1)?;
    if rgba_frame_byte_len(dimensions.0, dimensions.1)? > max_decoded_bytes {
        return Err(MediaDecodeError::SequenceLimit);
    }

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(MediaDecodeError::RasterIo)?;
    reader.limits(raster_limits(max_decoded_bytes as u64));
    let image = reader.decode().map_err(raster_decode_error)?;
    let (width, height) = image.dimensions();
    RgbaFrame::check_dimensions(width, height)?;
    if rgba_frame_byte_len(width, height)? > max_decoded_bytes {
        return Err(MediaDecodeError::SequenceLimit);
    }
    RgbaFrame::new(width, height, image.into_rgba8().into_raw())
        .map_err(MediaDecodeError::InvalidData)
}

/// Decode and downsample a raster image for a UI thumbnail without allocating an unbounded source
/// image. Source dimensions are capped at 4096×4096 and decoded working memory at 64 MiB.
pub fn decode_image_thumbnail(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    validate_thumbnail_size(max_width, max_height)?;
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(MediaDecodeError::RasterIo)?;
    let mut limits = raster_limits(MAX_THUMBNAIL_SOURCE_BYTES);
    limits.max_image_width = Some(MAX_THUMBNAIL_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_THUMBNAIL_SOURCE_DIMENSION);
    reader.limits(limits);
    let image = reader.decode().map_err(MediaDecodeError::Raster)?;
    let thumbnail = resize_thumbnail(image, max_width, max_height);
    let (width, height) = thumbnail.dimensions();
    RgbaFrame::new(width, height, thumbnail.into_raw()).map_err(MediaDecodeError::InvalidData)
}

/// Decode all GIF frames, preserving the decoder-composited canvas and per-frame delay.
pub fn decode_gif(bytes: &[u8]) -> Result<Vec<AnimationFrame>, MediaDecodeError> {
    decode_gif_inner(bytes, None, MAX_DECODED_FRAMES, MAX_DECODED_SEQUENCE_BYTES)?
        .ok_or(MediaDecodeError::InvalidData("GIF decode was cancelled"))
}

/// Decode only the first composited GIF frame and downsample it for a thumbnail.
pub fn decode_gif_thumbnail(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    validate_thumbnail_size(max_width, max_height)?;
    let mut decoder = GifDecoder::new(Cursor::new(bytes)).map_err(MediaDecodeError::Raster)?;
    let (width, height) = decoder.dimensions();
    RgbaFrame::check_dimensions(width, height)?;
    if width > MAX_THUMBNAIL_SOURCE_DIMENSION || height > MAX_THUMBNAIL_SOURCE_DIMENSION {
        return Err(MediaDecodeError::PixelLimit);
    }
    decoder
        .set_limits(raster_limits(MAX_THUMBNAIL_SOURCE_BYTES))
        .map_err(raster_decode_error)?;
    let frame = decoder
        .into_frames()
        .next()
        .ok_or(MediaDecodeError::InvalidData("GIF contains no frames"))?
        .map_err(MediaDecodeError::Raster)?
        .into_buffer();
    let thumbnail = resize_thumbnail(
        image::DynamicImage::ImageRgba8(frame),
        max_width,
        max_height,
    );
    let (width, height) = thumbnail.dimensions();
    RgbaFrame::new(width, height, thumbnail.into_raw()).map_err(MediaDecodeError::InvalidData)
}

/// Decode only the first IVF/VP8 frame and downsample it for a file-browser poster preview.
pub fn decode_ivf_vp8_thumbnail(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    validate_thumbnail_size(max_width, max_height)?;
    let frames = decode_ivf_vp8_inner(
        bytes,
        None,
        true,
        MAX_DECODED_FRAMES,
        MAX_DECODED_SEQUENCE_BYTES,
    )?
    .ok_or(MediaDecodeError::InvalidData("IVF decode was cancelled"))?;
    let frame = frames
        .into_iter()
        .next()
        .ok_or(MediaDecodeError::InvalidData(
            "IVF contains no decoded frames",
        ))?;
    video_frame_thumbnail(frame.frame, max_width, max_height)
}

/// Decode only the first WebM/Matroska VP8 frame for a file-browser poster preview.
pub fn decode_webm_vp8_thumbnail(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    validate_thumbnail_size(max_width, max_height)?;
    let frames = decode_webm_vp8_inner(
        bytes,
        None,
        true,
        MAX_DECODED_FRAMES,
        MAX_DECODED_SEQUENCE_BYTES,
    )?
    .ok_or(MediaDecodeError::InvalidData("WebM decode was cancelled"))?;
    let frame = frames
        .into_iter()
        .next()
        .ok_or(MediaDecodeError::InvalidData(
            "WebM contains no decoded frames",
        ))?;
    video_frame_thumbnail(frame.frame, max_width, max_height)
}

/// Decode a bounded H.264/AVC MP4 or QuickTime MOV sequence into full-size RGBA frames.
///
/// The returned frames are in display order with normalized presentation timestamps and per-frame
/// durations. The default limits are [`MAX_DECODED_FRAMES`] frames and
/// [`MAX_DECODED_SEQUENCE_BYTES`] decoded bytes. Encoded sources, demux packets, H.264 samples,
/// decoder working dimensions, reorder depth, and reference pictures are also bounded. This
/// supports AVC formats implemented by `rust_h264`; unsupported codecs, protected tracks, and
/// unsupported H.264 profiles or pixel formats return explicit errors.
pub fn decode_mp4_h264(bytes: impl Into<Vec<u8>>) -> Result<Vec<VideoFrame>, MediaDecodeError> {
    decode_mp4_h264_sequence_inner(
        Arc::from(bytes.into()),
        None,
        MAX_DECODED_FRAMES,
        MAX_DECODED_SEQUENCE_BYTES,
        MAX_THUMBNAIL_SOURCE_BYTES as usize * 2,
    )?
    .ok_or(MediaDecodeError::InvalidData(
        "MP4/MOV H.264 decode was cancelled",
    ))
}

#[cfg(test)]
pub(crate) fn decode_mp4_h264_sequence_cancellable(
    source: Arc<[u8]>,
    cancelled: &AtomicBool,
    max_frames: usize,
    max_sequence_bytes: usize,
    max_encoded_bytes: usize,
) -> Result<Option<Vec<VideoFrame>>, MediaDecodeError> {
    decode_mp4_h264_sequence_inner(
        source,
        Some(cancelled),
        max_frames,
        max_sequence_bytes,
        max_encoded_bytes,
    )
}

/// Decode the first displayable H.264/AVC picture from an MP4 or QuickTime MOV file and make a
/// bounded RGBA poster thumbnail.
///
/// This CPU decoder supports the Baseline, Main, High, High 10, and High 4:2:2 AVC families that
/// `rust_h264` implements: 8- or 10-bit 4:2:0, 4:2:2, and monochrome. 4:4:4, higher bit depths,
/// unsupported slice extensions, protected/encrypted tracks, and non-H.264 video tracks return
/// explicit errors. Inputs are limited to 64 MiB, video samples to 8 MiB, and decoded working
/// dimensions/references are bounded before picture allocation. Unspecified/unknown matrix
/// coefficients are converted using BT.601; BT.709 and BT.2020 coefficients are honored.
pub fn decode_mp4_h264_thumbnail(
    bytes: impl Into<Vec<u8>>,
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    decode_mp4_h264_thumbnail_inner(bytes.into(), max_width, max_height, None)?.ok_or(
        MediaDecodeError::InvalidData("MP4/MOV H.264 decode was cancelled"),
    )
}

/// Cancellable version of [`decode_mp4_h264_thumbnail`]. Returns `Ok(None)` when cancelled.
pub fn decode_mp4_h264_thumbnail_cancellable(
    bytes: impl Into<Vec<u8>>,
    max_width: u32,
    max_height: u32,
    cancelled: &AtomicBool,
) -> Result<Option<RgbaFrame>, MediaDecodeError> {
    decode_mp4_h264_thumbnail_inner(bytes.into(), max_width, max_height, Some(cancelled))
}

fn decode_mp4_h264_thumbnail_inner(
    bytes: Vec<u8>,
    max_width: u32,
    max_height: u32,
    cancelled: Option<&AtomicBool>,
) -> Result<Option<RgbaFrame>, MediaDecodeError> {
    validate_thumbnail_size(max_width, max_height)?;
    if bytes.len() > MAX_THUMBNAIL_SOURCE_BYTES as usize {
        return Err(MediaDecodeError::Video(format!(
            "MP4/MOV poster source exceeds the {} byte limit",
            MAX_THUMBNAIL_SOURCE_BYTES
        )));
    }
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(None);
    }

    let mut demuxer =
        oxideav_mp4::demux::open_typed(Box::new(Cursor::new(bytes)), &NullCodecResolver)
            .map_err(|error| MediaDecodeError::Video(format!("MP4/MOV demux failed: {error}")))?;
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(None);
    }

    let (video_stream_index, params) = demuxer
        .streams()
        .iter()
        .find(|stream| stream.params.media_type == MediaType::Video)
        .map(|stream| (stream.index, stream.params.clone()))
        .ok_or(MediaDecodeError::InvalidData(
            "MP4/MOV contains no video stream",
        ))?;
    if params.codec_id.as_str() != "h264" {
        return Err(MediaDecodeError::Video(format!(
            "MP4/MOV video codec '{}' is unsupported; poster previews currently support H.264/AVC",
            params.codec_id
        )));
    }
    if let Some(scheme) = params.options.get("protection_scheme") {
        return Err(MediaDecodeError::Video(format!(
            "encrypted MP4/MOV video (protection scheme '{scheme}') cannot be previewed"
        )));
    }
    if params.extradata.is_empty() || params.extradata.len() > MAX_H264_PARAMETER_BYTES {
        return Err(MediaDecodeError::Video(
            "H.264 AVC configuration is empty or exceeds the 64 KiB limit".to_owned(),
        ));
    }
    let avcc = parse_avcc_config(&params.extradata).map_err(|error| {
        MediaDecodeError::Video(format!("invalid H.264 AVC configuration: {error}"))
    })?;
    if avcc.sps_nals.is_empty() || avcc.pps_nals.is_empty() {
        return Err(MediaDecodeError::Video(
            "H.264 AVC configuration must contain SPS and PPS parameter sets".to_owned(),
        ));
    }

    let mut color_info = None;
    let mut reorder_depth = MAX_H264_REORDER_DEPTH;
    for sps in &avcc.sps_nals {
        let info = parse_sps_info(sps)
            .map_err(|error| MediaDecodeError::Video(format!("invalid H.264 SPS: {error}")))?;
        if info.width > MAX_THUMBNAIL_SOURCE_DIMENSION
            || info.height > MAX_THUMBNAIL_SOURCE_DIMENSION
            || info.width == 0
            || info.height == 0
        {
            return Err(MediaDecodeError::PixelLimit);
        }
        if !matches!(info.chroma_format, 0..=2)
            || !matches!(info.bit_depth_luma, 8 | 10)
            || !matches!(info.bit_depth_chroma, 8 | 10)
            || info.bit_depth_luma != info.bit_depth_chroma
        {
            return Err(MediaDecodeError::Video(format!(
                "unsupported H.264 pixel format: chroma_format={}, luma_depth={}, chroma_depth={}; supported formats are 8/10-bit 4:2:0, 4:2:2, and monochrome",
                info.chroma_format, info.bit_depth_luma, info.bit_depth_chroma
            )));
        }
        let references = u64::from(info.reference_frames.max(1)) + 1;
        let working_samples = u64::from(info.coded_width)
            .checked_mul(u64::from(info.coded_height))
            .and_then(|samples| samples.checked_mul(references))
            .ok_or(MediaDecodeError::DimensionsOverflow)?;
        if working_samples > MAX_H264_WORKING_LUMA_SAMPLES {
            return Err(MediaDecodeError::Video(format!(
                "H.264 reference working set ({working_samples} coded luma samples) exceeds the {MAX_H264_WORKING_LUMA_SAMPLES} sample limit"
            )));
        }
        if let Some(previous) = color_info {
            let previous: rust_h264::nal::SpsInfo = previous;
            if (
                previous.width,
                previous.height,
                previous.chroma_format,
                previous.bit_depth_luma,
            ) != (
                info.width,
                info.height,
                info.chroma_format,
                info.bit_depth_luma,
            ) {
                return Err(MediaDecodeError::Video(
                    "H.264 streams that switch resolution or pixel format are unsupported for poster previews".to_owned(),
                ));
            }
        } else {
            color_info = Some(info);
            let reorder = info.max_reorder_frames.unwrap_or(MAX_H264_REORDER_DEPTH);
            if reorder > MAX_H264_REORDER_DEPTH {
                return Err(MediaDecodeError::Video(format!(
                    "H.264 display reordering depth {reorder} exceeds the {MAX_H264_REORDER_DEPTH} frame poster limit"
                )));
            }
            reorder_depth = reorder.max(1);
        }
    }
    let color_info = color_info.expect("the avcC SPS set was checked as non-empty");

    let mut decoder = OrderedDecoder::<bool>::with_max_depth(reorder_depth as usize);
    for parameter_set in avcc.sps_nals.iter().chain(&avcc.pps_nals) {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Ok(None);
        }
        decoder
            .decode_nal_with_meta(parameter_set, false)
            .map_err(|error| {
                MediaDecodeError::Video(format!("H.264 parameter decode failed: {error}"))
            })?;
    }

    let mut started_at_keyframe = false;
    let mut video_samples = 0usize;
    let mut decoded_thumbnail = None;
    for _ in 0..MAX_H264_DEMUX_PACKETS {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Ok(None);
        }
        let packet = match demuxer.next_packet() {
            Ok(packet) => packet,
            Err(oxideav_core::Error::Eof) => break,
            Err(error) => {
                return Err(MediaDecodeError::Video(format!(
                    "MP4/MOV sample read failed: {error}"
                )));
            }
        };
        if packet.stream_index != video_stream_index {
            continue;
        }
        video_samples += 1;
        if video_samples > MAX_H264_VIDEO_SAMPLES {
            return Err(MediaDecodeError::Video(format!(
                "no displayable H.264 picture found within the {MAX_H264_VIDEO_SAMPLES} video sample poster limit"
            )));
        }
        if packet.data.len() > MAX_H264_SAMPLE_BYTES {
            return Err(MediaDecodeError::Video(format!(
                "H.264 sample exceeds the {} byte poster limit",
                MAX_H264_SAMPLE_BYTES
            )));
        }
        let nal_count = checked_avcc_nal_count(&packet.data, avcc.length_size)?;
        let nals = parse_avcc(&packet.data, avcc.length_size);
        if nals.len() != nal_count {
            return Err(MediaDecodeError::Video(
                "H.264 sample contains an invalid AVCC NAL unit".to_owned(),
            ));
        }
        if !started_at_keyframe
            && !packet.flags.keyframe
            && !nals
                .iter()
                .any(|nal| nal.nal_unit_type == NalUnitType::SliceIdr)
        {
            continue;
        }
        started_at_keyframe = true;
        for nal in nals {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                return Ok(None);
            }
            for (frame, discard) in decoder
                .decode_nal_with_meta(&nal, packet.flags.discard)
                .map_err(|error| {
                    MediaDecodeError::Video(format!("H.264 frame decode failed: {error}"))
                })?
            {
                if !discard {
                    decoded_thumbnail = Some(h264_frame_thumbnail(
                        &frame, color_info, max_width, max_height,
                    )?);
                    break;
                }
            }
            if decoded_thumbnail.is_some() {
                break;
            }
        }
        if decoded_thumbnail.is_some() {
            break;
        }
    }
    if !started_at_keyframe {
        return Err(MediaDecodeError::Video(
            "MP4/MOV H.264 track contains no decodable keyframe in the poster search window"
                .to_owned(),
        ));
    }
    if decoded_thumbnail.is_none() {
        for (frame, discard) in decoder.flush_with_meta() {
            if !discard {
                decoded_thumbnail = Some(h264_frame_thumbnail(
                    &frame, color_info, max_width, max_height,
                )?);
                break;
            }
        }
    }
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(None);
    }
    decoded_thumbnail.map(Some).ok_or_else(|| {
        MediaDecodeError::Video("H.264 track produced no displayable picture".to_owned())
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct H264Timing {
    timestamp_ns: Option<u64>,
    duration_ns: Option<u64>,
    discard: bool,
}

struct H264DecodedFrame {
    timing: H264Timing,
    frame: RgbaFrame,
}

fn decode_mp4_h264_sequence_inner(
    source: Arc<[u8]>,
    cancelled: Option<&AtomicBool>,
    max_frames: usize,
    max_sequence_bytes: usize,
    max_encoded_bytes: usize,
) -> Result<Option<Vec<VideoFrame>>, MediaDecodeError> {
    let options = H264DecodeOptions {
        max_frames: Some(max_frames.min(MAX_DECODED_FRAMES)),
        max_sequence_bytes: Some(max_sequence_bytes.min(MAX_DECODED_SEQUENCE_BYTES)),
        max_frame_bytes: max_sequence_bytes.min(MAX_DECODED_SEQUENCE_BYTES),
        max_encoded_bytes,
    };
    let mut decoded_frames = Vec::new();
    if !decode_mp4_h264_sequence_with_sink(source, cancelled, options, |frame| {
        decoded_frames.push(frame);
    })? {
        return Ok(None);
    }
    Ok(Some(normalize_h264_timeline(decoded_frames)))
}

#[derive(Clone, Copy)]
struct H264DecodeOptions {
    max_frames: Option<usize>,
    max_sequence_bytes: Option<usize>,
    max_frame_bytes: usize,
    max_encoded_bytes: usize,
}

pub(crate) fn decode_mp4_h264_stream_cancellable(
    source: Arc<[u8]>,
    cancelled: &AtomicBool,
    max_encoded_bytes: usize,
    max_frames: usize,
    max_frame_bytes: usize,
    mut emit: impl FnMut(VideoFrame) -> bool,
) -> Result<bool, MediaDecodeError> {
    let mut timeline = H264StreamTimeline::new(&mut emit);
    let completed = decode_mp4_h264_sequence_with_sink(
        source,
        Some(cancelled),
        H264DecodeOptions {
            max_frames: Some(max_frames.min(MAX_STREAMED_MEDIA_FRAMES)),
            max_sequence_bytes: None,
            max_frame_bytes: max_frame_bytes.min(MAX_DECODED_SEQUENCE_BYTES),
            max_encoded_bytes,
        },
        |frame| {
            if !timeline.push(frame) {
                cancelled.store(true, Ordering::Release);
            }
        },
    )?;
    if completed && !cancelled.load(Ordering::Acquire) && !timeline.finish() {
        cancelled.store(true, Ordering::Release);
    }
    Ok(completed && !cancelled.load(Ordering::Acquire))
}

struct H264StreamTimeline<'a> {
    emit: &'a mut dyn FnMut(VideoFrame) -> bool,
    first_timestamp_ns: Option<u64>,
    fallback_timestamp_ns: u64,
    pending: Option<VideoFrame>,
}

impl<'a> H264StreamTimeline<'a> {
    fn new(emit: &'a mut dyn FnMut(VideoFrame) -> bool) -> Self {
        Self {
            emit,
            first_timestamp_ns: None,
            fallback_timestamp_ns: 0,
            pending: None,
        }
    }

    fn push(&mut self, decoded: H264DecodedFrame) -> bool {
        let raw_timestamp = decoded
            .timing
            .timestamp_ns
            .unwrap_or(self.fallback_timestamp_ns);
        let first_timestamp = *self.first_timestamp_ns.get_or_insert(raw_timestamp);
        let timestamp_ns = raw_timestamp.saturating_sub(first_timestamp);
        let duration_ns = decoded.timing.duration_ns.filter(|duration| *duration > 0);
        let current = VideoFrame {
            timestamp_ns,
            duration_ns,
            frame: decoded.frame,
        };
        if let Some(mut previous) = self.pending.take() {
            let next_delta = timestamp_ns
                .checked_sub(previous.timestamp_ns)
                .filter(|delta| *delta > 0);
            previous.duration_ns = previous
                .duration_ns
                .filter(|duration| *duration > 0)
                .or(next_delta)
                .or(Some(DEFAULT_H264_FRAME_DURATION_NS));
            if !(self.emit)(previous) {
                return false;
            }
        }
        self.fallback_timestamp_ns =
            raw_timestamp.saturating_add(duration_ns.unwrap_or(DEFAULT_H264_FRAME_DURATION_NS));
        self.pending = Some(current);
        true
    }

    fn finish(&mut self) -> bool {
        let Some(mut previous) = self.pending.take() else {
            return true;
        };
        if previous.duration_ns.is_none_or(|duration| duration == 0) {
            previous.duration_ns = Some(DEFAULT_H264_FRAME_DURATION_NS);
        }
        (self.emit)(previous)
    }
}

fn decode_mp4_h264_sequence_with_sink(
    source: Arc<[u8]>,
    cancelled: Option<&AtomicBool>,
    options: H264DecodeOptions,
    mut emit: impl FnMut(H264DecodedFrame),
) -> Result<bool, MediaDecodeError> {
    let H264DecodeOptions {
        max_frames,
        max_sequence_bytes,
        max_frame_bytes,
        max_encoded_bytes,
    } = options;
    if source.len() > max_encoded_bytes {
        return Err(MediaDecodeError::Video(format!(
            "MP4/MOV H.264 source exceeds the {max_encoded_bytes} byte sequence limit"
        )));
    }
    if max_frames == Some(0) {
        return Err(MediaDecodeError::FrameLimit);
    }
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(false);
    }

    let mut demuxer = oxideav_mp4::demux::open_typed(
        Box::new(Cursor::new(Arc::clone(&source))),
        &NullCodecResolver,
    )
    .map_err(|error| MediaDecodeError::Video(format!("MP4/MOV demux failed: {error}")))?;
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(false);
    }

    let (video_stream_index, params) = demuxer
        .streams()
        .iter()
        .find(|stream| stream.params.media_type == MediaType::Video)
        .map(|stream| (stream.index, stream.params.clone()))
        .ok_or(MediaDecodeError::InvalidData(
            "MP4/MOV contains no video stream",
        ))?;
    if params.codec_id.as_str() != "h264" {
        return Err(MediaDecodeError::Video(format!(
            "MP4/MOV video codec '{}' is unsupported; sequence playback currently supports H.264/AVC",
            params.codec_id
        )));
    }
    if let Some(scheme) = params.options.get("protection_scheme") {
        return Err(MediaDecodeError::Video(format!(
            "encrypted MP4/MOV video (protection scheme '{scheme}') cannot be decoded"
        )));
    }
    if params.extradata.is_empty() || params.extradata.len() > MAX_H264_PARAMETER_BYTES {
        return Err(MediaDecodeError::Video(
            "H.264 AVC configuration is empty or exceeds the 64 KiB limit".to_owned(),
        ));
    }
    let avcc = parse_avcc_config(&params.extradata).map_err(|error| {
        MediaDecodeError::Video(format!("invalid H.264 AVC configuration: {error}"))
    })?;
    if avcc.sps_nals.is_empty() || avcc.pps_nals.is_empty() {
        return Err(MediaDecodeError::Video(
            "H.264 AVC configuration must contain SPS and PPS parameter sets".to_owned(),
        ));
    }

    let mut color_info = None;
    let mut reorder_depth = MAX_H264_REORDER_DEPTH;
    for sps in &avcc.sps_nals {
        let info = parse_sps_info(sps)
            .map_err(|error| MediaDecodeError::Video(format!("invalid H.264 SPS: {error}")))?;
        if info.width > MAX_THUMBNAIL_SOURCE_DIMENSION
            || info.height > MAX_THUMBNAIL_SOURCE_DIMENSION
            || info.width == 0
            || info.height == 0
        {
            return Err(MediaDecodeError::PixelLimit);
        }
        if !matches!(info.chroma_format, 0..=2)
            || !matches!(info.bit_depth_luma, 8 | 10)
            || !matches!(info.bit_depth_chroma, 8 | 10)
            || info.bit_depth_luma != info.bit_depth_chroma
        {
            return Err(MediaDecodeError::Video(format!(
                "unsupported H.264 pixel format: chroma_format={}, luma_depth={}, chroma_depth={}; supported formats are 8/10-bit 4:2:0, 4:2:2, and monochrome",
                info.chroma_format, info.bit_depth_luma, info.bit_depth_chroma
            )));
        }
        let references = u64::from(info.reference_frames.max(1)) + 1;
        let working_samples = u64::from(info.coded_width)
            .checked_mul(u64::from(info.coded_height))
            .and_then(|samples| samples.checked_mul(references))
            .ok_or(MediaDecodeError::DimensionsOverflow)?;
        if working_samples > MAX_H264_WORKING_LUMA_SAMPLES {
            return Err(MediaDecodeError::Video(format!(
                "H.264 reference working set ({working_samples} coded luma samples) exceeds the {MAX_H264_WORKING_LUMA_SAMPLES} sample limit"
            )));
        }
        if let Some(previous) = color_info {
            let previous: rust_h264::nal::SpsInfo = previous;
            if (
                previous.width,
                previous.height,
                previous.chroma_format,
                previous.bit_depth_luma,
            ) != (
                info.width,
                info.height,
                info.chroma_format,
                info.bit_depth_luma,
            ) {
                return Err(MediaDecodeError::Video(
                    "H.264 streams that switch resolution or pixel format are unsupported for sequence playback".to_owned(),
                ));
            }
        } else {
            color_info = Some(info);
            let reorder = info.max_reorder_frames.unwrap_or(MAX_H264_REORDER_DEPTH);
            if reorder > MAX_H264_REORDER_DEPTH {
                return Err(MediaDecodeError::Video(format!(
                    "H.264 display reordering depth {reorder} exceeds the {MAX_H264_REORDER_DEPTH} frame limit"
                )));
            }
            reorder_depth = reorder.max(1);
        }
    }
    let color_info = color_info.expect("the avcC SPS set was checked as non-empty");

    let mut decoder = OrderedDecoder::<H264Timing>::with_max_depth(reorder_depth as usize);
    for parameter_set in avcc.sps_nals.iter().chain(&avcc.pps_nals) {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Ok(false);
        }
        decoder
            .decode_nal_with_meta(parameter_set, H264Timing::default())
            .map_err(|error| {
                MediaDecodeError::Video(format!("H.264 parameter decode failed: {error}"))
            })?;
    }

    let packet_limit = max_frames.map_or(MAX_H264_STREAM_PACKETS, |max_frames| {
        max_frames
            .saturating_mul(4)
            .saturating_add(reorder_depth as usize)
            .min(MAX_H264_SEQUENCE_PACKETS)
    });
    let mut started_at_keyframe = false;
    let mut decoded_frames = Vec::<H264DecodedFrame>::new();
    let mut decoded_bytes = 0usize;
    let mut decoded_frame_count = 0usize;
    let mut reached_eof = false;
    for _ in 0..packet_limit {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Ok(false);
        }
        let packet = match demuxer.next_packet() {
            Ok(packet) => packet,
            Err(oxideav_core::Error::Eof) => {
                reached_eof = true;
                break;
            }
            Err(error) => {
                return Err(MediaDecodeError::Video(format!(
                    "MP4/MOV sample read failed: {error}"
                )));
            }
        };
        if packet.stream_index != video_stream_index {
            continue;
        }
        if packet.data.len() > MAX_H264_SAMPLE_BYTES {
            return Err(MediaDecodeError::Video(format!(
                "H.264 sample exceeds the {} byte sequence limit",
                MAX_H264_SAMPLE_BYTES
            )));
        }
        let nal_count = checked_avcc_nal_count(&packet.data, avcc.length_size)?;
        let nals = parse_avcc(&packet.data, avcc.length_size);
        if nals.len() != nal_count {
            return Err(MediaDecodeError::Video(
                "H.264 sample contains an invalid AVCC NAL unit".to_owned(),
            ));
        }
        if !started_at_keyframe
            && !packet.flags.keyframe
            && !nals
                .iter()
                .any(|nal| nal.nal_unit_type == NalUnitType::SliceIdr)
        {
            continue;
        }
        started_at_keyframe = true;
        let timing = H264Timing {
            timestamp_ns: packet
                .pts
                .and_then(|pts| media_ticks_to_ns(pts, packet.time_base)),
            duration_ns: packet
                .duration
                .and_then(|ticks| media_ticks_to_ns(ticks, packet.time_base)),
            discard: packet.flags.discard,
        };
        for nal in nals {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                return Ok(false);
            }
            let outputs = decoder
                .decode_nal_with_meta(&nal, timing)
                .map_err(|error| {
                    MediaDecodeError::Video(format!("H.264 frame decode failed: {error}"))
                })?;
            for (frame, timing) in outputs {
                if timing.discard {
                    continue;
                }
                if max_frames.is_some_and(|max_frames| decoded_frame_count >= max_frames) {
                    return Err(MediaDecodeError::FrameLimit);
                }
                let frame_bytes = rgba_frame_byte_len(frame.width, frame.height)?;
                if frame_bytes > max_frame_bytes {
                    return Err(MediaDecodeError::SequenceLimit);
                }
                if let Some(max_sequence_bytes) = max_sequence_bytes {
                    decoded_bytes = decoded_bytes
                        .checked_add(frame_bytes)
                        .ok_or(MediaDecodeError::SequenceLimit)?;
                    if decoded_bytes > max_sequence_bytes {
                        return Err(MediaDecodeError::SequenceLimit);
                    }
                }
                let rgba = h264_frame_to_rgba(&frame, color_info)?;
                let decoded = H264DecodedFrame {
                    timing,
                    frame: rgba,
                };
                if max_frames.is_some() {
                    decoded_frames.push(decoded);
                } else {
                    emit(decoded);
                    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                        return Ok(false);
                    }
                }
                decoded_frame_count = decoded_frame_count.saturating_add(1);
            }
        }
    }
    if !reached_eof {
        return Err(MediaDecodeError::Video(format!(
            "MP4/MOV H.264 sequence exceeds the {packet_limit} demux packet limit"
        )));
    }
    if !started_at_keyframe {
        return Err(MediaDecodeError::Video(
            "MP4/MOV H.264 track contains no decodable keyframe".to_owned(),
        ));
    }
    for (frame, timing) in decoder.flush_with_meta() {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            return Ok(false);
        }
        if timing.discard {
            continue;
        }
        if max_frames.is_some_and(|max_frames| decoded_frame_count >= max_frames) {
            return Err(MediaDecodeError::FrameLimit);
        }
        let frame_bytes = rgba_frame_byte_len(frame.width, frame.height)?;
        if frame_bytes > max_frame_bytes {
            return Err(MediaDecodeError::SequenceLimit);
        }
        if let Some(max_sequence_bytes) = max_sequence_bytes {
            decoded_bytes = decoded_bytes
                .checked_add(frame_bytes)
                .ok_or(MediaDecodeError::SequenceLimit)?;
            if decoded_bytes > max_sequence_bytes {
                return Err(MediaDecodeError::SequenceLimit);
            }
        }
        let decoded = H264DecodedFrame {
            timing,
            frame: h264_frame_to_rgba(&frame, color_info)?,
        };
        if max_frames.is_some() {
            decoded_frames.push(decoded);
        } else {
            emit(decoded);
            if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                return Ok(false);
            }
        }
        decoded_frame_count = decoded_frame_count.saturating_add(1);
    }
    if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
        return Ok(false);
    }
    if decoded_frame_count == 0 {
        return Err(MediaDecodeError::Video(
            "H.264 track produced no displayable pictures".to_owned(),
        ));
    }
    if max_frames.is_some() {
        for frame in decoded_frames {
            emit(frame);
        }
    }
    Ok(true)
}

fn media_ticks_to_ns(ticks: i64, time_base: oxideav_core::TimeBase) -> Option<u64> {
    if ticks < 0 || time_base.num() <= 0 || time_base.den() <= 0 {
        return None;
    }
    let nanoseconds = i128::from(ticks)
        .checked_mul(i128::from(time_base.num()))?
        .checked_mul(1_000_000_000)?
        / i128::from(time_base.den());
    u64::try_from(nanoseconds).ok()
}

fn normalize_h264_timeline(frames: Vec<H264DecodedFrame>) -> Vec<VideoFrame> {
    let first_timestamp = frames
        .iter()
        .find_map(|frame| frame.timing.timestamp_ns)
        .unwrap_or(0);
    let timestamps: Vec<_> = frames
        .iter()
        .map(|frame| {
            frame
                .timing
                .timestamp_ns
                .map(|timestamp| timestamp.saturating_sub(first_timestamp))
        })
        .collect();
    let mut fallback_timestamp = 0u64;
    frames
        .into_iter()
        .enumerate()
        .map(|(index, decoded)| {
            let timestamp_ns = timestamps[index].unwrap_or(fallback_timestamp);
            let next_delta = timestamps
                .get(index + 1)
                .and_then(|next| next.map(|next| next.checked_sub(timestamp_ns)))
                .flatten()
                .filter(|delta| *delta > 0);
            let duration_ns = decoded
                .timing
                .duration_ns
                .filter(|duration| *duration > 0)
                .or(next_delta)
                .unwrap_or(DEFAULT_H264_FRAME_DURATION_NS);
            fallback_timestamp = timestamp_ns.saturating_add(duration_ns);
            VideoFrame {
                timestamp_ns,
                duration_ns: Some(duration_ns),
                frame: decoded.frame,
            }
        })
        .collect()
}

fn rgba_frame_byte_len(width: u32, height: u32) -> Result<usize, MediaDecodeError> {
    RgbaFrame::check_dimensions(width, height)?;
    usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| MediaDecodeError::DimensionsOverflow)
}

fn checked_avcc_nal_count(sample: &[u8], length_size: usize) -> Result<usize, MediaDecodeError> {
    if !matches!(length_size, 1 | 2 | 4) || sample.is_empty() {
        return Err(MediaDecodeError::Video(
            "H.264 sample has invalid AVCC length fields".to_owned(),
        ));
    }
    let mut offset = 0usize;
    let mut count = 0usize;
    while offset < sample.len() {
        let prefix_end = offset
            .checked_add(length_size)
            .filter(|end| *end <= sample.len())
            .ok_or_else(|| {
                MediaDecodeError::Video("truncated H.264 AVCC length field".to_owned())
            })?;
        let length = match length_size {
            1 => usize::from(sample[offset]),
            2 => usize::from(u16::from_be_bytes([sample[offset], sample[offset + 1]])),
            4 => usize::try_from(u32::from_be_bytes([
                sample[offset],
                sample[offset + 1],
                sample[offset + 2],
                sample[offset + 3],
            ]))
            .map_err(|_| MediaDecodeError::DimensionsOverflow)?,
            _ => unreachable!("AVCC length size checked above"),
        };
        if length == 0 {
            return Err(MediaDecodeError::Video(
                "H.264 AVCC sample contains an empty NAL unit".to_owned(),
            ));
        }
        offset = prefix_end
            .checked_add(length)
            .filter(|end| *end <= sample.len())
            .ok_or_else(|| MediaDecodeError::Video("truncated H.264 AVCC NAL unit".to_owned()))?;
        count += 1;
        if count > MAX_H264_SAMPLE_NALS {
            return Err(MediaDecodeError::Video(format!(
                "H.264 sample exceeds the {MAX_H264_SAMPLE_NALS} NAL unit limit"
            )));
        }
    }
    Ok(count)
}

fn h264_frame_thumbnail(
    frame: &H264Frame,
    color: rust_h264::nal::SpsInfo,
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    RgbaFrame::check_dimensions(frame.width, frame.height)?;
    if frame.width > MAX_THUMBNAIL_SOURCE_DIMENSION || frame.height > MAX_THUMBNAIL_SOURCE_DIMENSION
    {
        return Err(MediaDecodeError::PixelLimit);
    }
    if frame.bit_depth != color.bit_depth_luma as u8
        || !matches!(
            frame.chroma_format,
            H264ChromaFormat::Yuv420 | H264ChromaFormat::Yuv422
        )
    {
        return Err(MediaDecodeError::Video(
            "H.264 decoder output changed pixel format after SPS validation".to_owned(),
        ));
    }
    let bytes_per_sample = frame.bytes_per_sample();
    let luma_samples = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    let chroma_samples = (frame.chroma_width() as usize)
        .checked_mul(frame.chroma_height() as usize)
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    if frame.y.len() < luma_samples * bytes_per_sample
        || frame.u.len() < chroma_samples * bytes_per_sample
        || frame.v.len() < chroma_samples * bytes_per_sample
    {
        return Err(MediaDecodeError::Video(
            "H.264 decoder returned incomplete YUV planes".to_owned(),
        ));
    }

    let scale = (max_width as f64 / f64::from(frame.width))
        .min(max_height as f64 / f64::from(frame.height))
        .min(1.0);
    let width = (f64::from(frame.width) * scale)
        .round()
        .clamp(1.0, f64::from(max_width)) as u32;
    let height = (f64::from(frame.height) * scale)
        .round()
        .clamp(1.0, f64::from(max_height)) as u32;
    let byte_count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    let mut pixels = vec![0; byte_count];
    let chroma_width = frame.chroma_width() as usize;
    let chroma_height = frame.chroma_height() as usize;
    for y in 0..height as usize {
        let source_y = ((2 * y + 1) * frame.height as usize / (2 * height as usize))
            .min(frame.height as usize - 1);
        for x in 0..width as usize {
            let source_x = ((2 * x + 1) * frame.width as usize / (2 * width as usize))
                .min(frame.width as usize - 1);
            let chroma_x = (source_x / 2).min(chroma_width - 1);
            let chroma_y = match frame.chroma_format {
                H264ChromaFormat::Yuv420 => source_y / 2,
                H264ChromaFormat::Yuv422 => source_y,
            }
            .min(chroma_height - 1);
            let y_sample = frame.sample(&frame.y, source_y * frame.width as usize + source_x);
            let chroma_index = chroma_y * chroma_width + chroma_x;
            let u_sample = frame.sample(&frame.u, chroma_index);
            let v_sample = frame.sample(&frame.v, chroma_index);
            let rgb = h264_yuv_to_rgb(
                y_sample,
                u_sample,
                v_sample,
                frame.bit_depth,
                color.full_range,
                color.matrix,
            );
            let output = (y * width as usize + x) * 4;
            pixels[output..output + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    RgbaFrame::new(width, height, pixels).map_err(MediaDecodeError::InvalidData)
}

fn h264_frame_to_rgba(
    frame: &H264Frame,
    color: rust_h264::nal::SpsInfo,
) -> Result<RgbaFrame, MediaDecodeError> {
    RgbaFrame::check_dimensions(frame.width, frame.height)?;
    if frame.width > MAX_THUMBNAIL_SOURCE_DIMENSION || frame.height > MAX_THUMBNAIL_SOURCE_DIMENSION
    {
        return Err(MediaDecodeError::PixelLimit);
    }
    if frame.bit_depth != color.bit_depth_luma as u8
        || !matches!(
            frame.chroma_format,
            H264ChromaFormat::Yuv420 | H264ChromaFormat::Yuv422
        )
    {
        return Err(MediaDecodeError::Video(
            "H.264 decoder output changed pixel format after SPS validation".to_owned(),
        ));
    }
    let bytes_per_sample = frame.bytes_per_sample();
    let luma_samples = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    let chroma_samples = (frame.chroma_width() as usize)
        .checked_mul(frame.chroma_height() as usize)
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    if frame.y.len() < luma_samples * bytes_per_sample
        || frame.u.len() < chroma_samples * bytes_per_sample
        || frame.v.len() < chroma_samples * bytes_per_sample
    {
        return Err(MediaDecodeError::Video(
            "H.264 decoder returned incomplete YUV planes".to_owned(),
        ));
    }

    let pixel_count = luma_samples;
    let mut pixels = vec![0; pixel_count * 4];
    let chroma_width = frame.chroma_width() as usize;
    let chroma_height = frame.chroma_height() as usize;
    for y in 0..frame.height as usize {
        for x in 0..frame.width as usize {
            let chroma_x = (x / 2).min(chroma_width - 1);
            let chroma_y = match frame.chroma_format {
                H264ChromaFormat::Yuv420 => y / 2,
                H264ChromaFormat::Yuv422 => y,
            }
            .min(chroma_height - 1);
            let y_sample = frame.sample(&frame.y, y * frame.width as usize + x);
            let chroma_index = chroma_y * chroma_width + chroma_x;
            let u_sample = frame.sample(&frame.u, chroma_index);
            let v_sample = frame.sample(&frame.v, chroma_index);
            let rgb = h264_yuv_to_rgb(
                y_sample,
                u_sample,
                v_sample,
                frame.bit_depth,
                color.full_range,
                color.matrix,
            );
            let output = (y * frame.width as usize + x) * 4;
            pixels[output..output + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    RgbaFrame::new(frame.width, frame.height, pixels).map_err(MediaDecodeError::InvalidData)
}

fn h264_yuv_to_rgb(
    y_sample: u16,
    u_sample: u16,
    v_sample: u16,
    bit_depth: u8,
    full_range: bool,
    matrix: u8,
) -> [u8; 3] {
    let (kr, kb) = match matrix {
        1 => (0.2126, 0.0722), // BT.709
        9 => (0.2627, 0.0593), // BT.2020 non-constant luminance
        _ => (0.299, 0.114),   // BT.601 / unspecified / legacy matrix codes
    };
    let kg = 1.0 - kr - kb;
    let (y, cb, cr) = if full_range {
        let maximum = f64::from((1u16 << bit_depth) - 1);
        (
            f64::from(y_sample) / maximum,
            f64::from(u_sample) / maximum - 0.5,
            f64::from(v_sample) / maximum - 0.5,
        )
    } else {
        let depth_scale = f64::from(1u16 << (bit_depth - 8));
        (
            (f64::from(y_sample) - 16.0 * depth_scale) / (219.0 * depth_scale),
            (f64::from(u_sample) - 128.0 * depth_scale) / (224.0 * depth_scale),
            (f64::from(v_sample) - 128.0 * depth_scale) / (224.0 * depth_scale),
        )
    };
    let rgb = [
        y + 2.0 * (1.0 - kr) * cr,
        y - 2.0 * kb * (1.0 - kb) / kg * cb - 2.0 * kr * (1.0 - kr) / kg * cr,
        y + 2.0 * (1.0 - kb) * cb,
    ];
    rgb.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn video_frame_thumbnail(
    frame: RgbaFrame,
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    if frame.width > MAX_THUMBNAIL_SOURCE_DIMENSION || frame.height > MAX_THUMBNAIL_SOURCE_DIMENSION
    {
        return Err(MediaDecodeError::PixelLimit);
    }
    let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.pixels).ok_or(
        MediaDecodeError::InvalidData("video frame has invalid RGBA storage"),
    )?;
    let thumbnail = resize_thumbnail(
        image::DynamicImage::ImageRgba8(image),
        max_width,
        max_height,
    );
    let (width, height) = thumbnail.dimensions();
    RgbaFrame::new(width, height, thumbnail.into_raw()).map_err(MediaDecodeError::InvalidData)
}

/// Decode a GIF frame-by-frame into a bounded consumer without retaining its full timeline.
pub(crate) fn decode_gif_stream_cancellable(
    bytes: &[u8],
    cancelled: &AtomicBool,
    max_frames: usize,
    max_frame_bytes: usize,
    mut emit: impl FnMut(VideoFrame) -> bool,
) -> Result<bool, MediaDecodeError> {
    let mut decoder = GifDecoder::new(Cursor::new(bytes)).map_err(MediaDecodeError::Raster)?;
    let (canvas_width, canvas_height) = decoder.dimensions();
    RgbaFrame::check_dimensions(canvas_width, canvas_height)?;
    decoder
        .set_limits(raster_limits(
            max_frame_bytes.min(MAX_DECODED_SEQUENCE_BYTES) as u64,
        ))
        .map_err(raster_decode_error)?;

    let max_frames = max_frames.min(MAX_STREAMED_MEDIA_FRAMES);
    let mut frame_count = 0usize;
    let mut timestamp_ns = 0u64;
    for frame in decoder.into_frames() {
        if is_cancelled(Some(cancelled)) {
            return Ok(false);
        }
        if frame_count >= max_frames {
            return Err(MediaDecodeError::FrameLimit);
        }
        let frame = frame.map_err(raster_decode_error)?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay_ns = (u128::from(numerator) * 1_000_000 / u128::from(denominator.max(1)))
            .min(u128::from(u64::MAX)) as u64;
        let delay = Duration::from_nanos(delay_ns).max(MIN_STREAM_FRAME_DURATION);
        let image = frame.into_buffer();
        let (width, height) = image.dimensions();
        RgbaFrame::check_dimensions(width, height)?;
        let frame_bytes = rgba_frame_byte_len(width, height)?;
        if frame_bytes > max_frame_bytes.min(MAX_DECODED_SEQUENCE_BYTES) {
            return Err(MediaDecodeError::SequenceLimit);
        }
        let frame = RgbaFrame::new(width, height, image.into_raw())
            .map_err(MediaDecodeError::InvalidData)?;
        if !emit(VideoFrame {
            timestamp_ns,
            duration_ns: Some(delay.as_nanos().min(u128::from(u64::MAX)) as u64),
            frame,
        }) {
            cancelled.store(true, Ordering::Release);
            return Ok(false);
        }
        timestamp_ns =
            timestamp_ns.saturating_add(delay.as_nanos().min(u128::from(u64::MAX)) as u64);
        frame_count = frame_count.saturating_add(1);
    }
    if frame_count == 0 {
        return Err(MediaDecodeError::InvalidData(
            "GIF contains no decodable frames",
        ));
    }
    Ok(true)
}

fn decode_gif_inner(
    bytes: &[u8],
    cancelled: Option<&AtomicBool>,
    max_frames: usize,
    max_sequence_bytes: usize,
) -> Result<Option<Vec<AnimationFrame>>, MediaDecodeError> {
    let mut decoder = GifDecoder::new(Cursor::new(bytes)).map_err(MediaDecodeError::Raster)?;
    let (canvas_width, canvas_height) = decoder.dimensions();
    RgbaFrame::check_dimensions(canvas_width, canvas_height)?;
    decoder
        .set_limits(raster_limits(max_sequence_bytes as u64))
        .map_err(raster_decode_error)?;
    let mut frames = Vec::new();
    let mut total_bytes = 0usize;
    for frame in decoder.into_frames() {
        if is_cancelled(cancelled) {
            return Ok(None);
        }
        if frames.len() >= max_frames {
            return Err(MediaDecodeError::FrameLimit);
        }
        let frame = frame.map_err(raster_decode_error)?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay = Duration::from_secs_f64(numerator as f64 / denominator.max(1) as f64 / 1_000.0);
        let image = frame.into_buffer();
        let (width, height) = image.dimensions();
        RgbaFrame::check_dimensions(width, height)?;
        let frame = RgbaFrame::new(width, height, image.into_raw())
            .map_err(MediaDecodeError::InvalidData)?;
        total_bytes = total_bytes
            .checked_add(frame.pixels.len())
            .ok_or(MediaDecodeError::SequenceLimit)?;
        if total_bytes > max_sequence_bytes {
            return Err(MediaDecodeError::SequenceLimit);
        }
        frames.push(AnimationFrame { frame, delay });
    }
    Ok(Some(frames))
}

fn raster_limits(max_alloc: u64) -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_PIXELS as u32);
    limits.max_image_height = Some(MAX_DECODED_PIXELS as u32);
    limits.max_alloc = Some(max_alloc);
    limits
}

fn raster_decode_error(error: image::ImageError) -> MediaDecodeError {
    match error {
        image::ImageError::Limits(limit) => match limit.kind() {
            image::error::LimitErrorKind::InsufficientMemory => MediaDecodeError::SequenceLimit,
            image::error::LimitErrorKind::DimensionError => MediaDecodeError::PixelLimit,
            _ => MediaDecodeError::Raster(image::ImageError::Limits(limit)),
        },
        other => MediaDecodeError::Raster(other),
    }
}

/// Decode SVG bytes with usvg/resvg. The resulting raster is straight-alpha RGBA8.
pub fn decode_svg(bytes: &[u8]) -> Result<RgbaFrame, MediaDecodeError> {
    decode_svg_with_max_bytes(bytes, MAX_DECODED_SEQUENCE_BYTES)
}

pub(crate) fn decode_svg_with_max_bytes(
    bytes: &[u8],
    max_decoded_bytes: usize,
) -> Result<RgbaFrame, MediaDecodeError> {
    let max_decoded_bytes = max_decoded_bytes.min(MAX_DECODED_SEQUENCE_BYTES);
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options)
        .map_err(|error| MediaDecodeError::Svg(error.to_string()))?;
    let size = tree.size();
    let width = size.width().ceil() as u32;
    let height = size.height().ceil() as u32;
    RgbaFrame::check_dimensions(width, height)?;
    if rgba_frame_byte_len(width, height)? > max_decoded_bytes {
        return Err(MediaDecodeError::SequenceLimit);
    }
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or(MediaDecodeError::DimensionsOverflow)?;
    let scale = resvg::tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, scale, &mut pixmap.as_mut());

    let mut pixels = pixmap.data().to_vec();
    unpremultiply_rgba(&mut pixels);
    RgbaFrame::new(width, height, pixels).map_err(MediaDecodeError::InvalidData)
}

/// Rasterize an SVG at its intrinsic aspect ratio within a bounded thumbnail size.
pub fn decode_svg_thumbnail(
    bytes: &[u8],
    max_width: u32,
    max_height: u32,
) -> Result<RgbaFrame, MediaDecodeError> {
    validate_thumbnail_size(max_width, max_height)?;
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options)
        .map_err(|error| MediaDecodeError::Svg(error.to_string()))?;
    let size = tree.size();
    let scale = (max_width as f32 / size.width())
        .min(max_height as f32 / size.height())
        .min(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(MediaDecodeError::DimensionsOverflow);
    }
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or(MediaDecodeError::DimensionsOverflow)?;
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut pixels = pixmap.data().to_vec();
    unpremultiply_rgba(&mut pixels);
    RgbaFrame::new(width, height, pixels).map_err(MediaDecodeError::InvalidData)
}

fn validate_thumbnail_size(width: u32, height: u32) -> Result<(), MediaDecodeError> {
    if width == 0 || height == 0 {
        Err(MediaDecodeError::InvalidData(
            "thumbnail dimensions must be positive",
        ))
    } else {
        Ok(())
    }
}

fn resize_thumbnail(
    image: image::DynamicImage,
    max_width: u32,
    max_height: u32,
) -> image::RgbaImage {
    let (source_width, source_height) = image.dimensions();
    let scale = (max_width as f32 / source_width as f32)
        .min(max_height as f32 / source_height as f32)
        .min(1.0);
    let width = (source_width as f32 * scale)
        .round()
        .clamp(1.0, max_width as f32) as u32;
    let height = (source_height as f32 * scale)
        .round()
        .clamp(1.0, max_height as f32) as u32;
    if (width, height) == (source_width, source_height) {
        image.into_rgba8()
    } else {
        image
            .resize_exact(width, height, image::imageops::FilterType::Triangle)
            .into_rgba8()
    }
}

/// Decode an IVF container carrying a single VP8 stream (`VP80` FourCC).
pub fn decode_ivf_vp8(bytes: &[u8]) -> Result<Vec<VideoFrame>, MediaDecodeError> {
    decode_ivf_vp8_inner(
        bytes,
        None,
        false,
        MAX_DECODED_FRAMES,
        MAX_DECODED_SEQUENCE_BYTES,
    )?
    .ok_or(MediaDecodeError::InvalidData("IVF decode was cancelled"))
}

#[cfg(test)]
pub(crate) fn decode_ivf_vp8_cancellable_with_limits(
    bytes: &[u8],
    cancelled: &AtomicBool,
    max_frames: usize,
    max_sequence_bytes: usize,
) -> Result<Option<Vec<VideoFrame>>, MediaDecodeError> {
    decode_ivf_vp8_inner(
        bytes,
        Some(cancelled),
        false,
        max_frames.min(MAX_DECODED_FRAMES),
        max_sequence_bytes.min(MAX_DECODED_SEQUENCE_BYTES),
    )
}

fn decode_ivf_vp8_inner(
    bytes: &[u8],
    cancelled: Option<&AtomicBool>,
    first_frame_only: bool,
    max_frames: usize,
    max_sequence_bytes: usize,
) -> Result<Option<Vec<VideoFrame>>, MediaDecodeError> {
    let reader = vp8_decoder::ivf::IvfReader::new(bytes)
        .map_err(|error| MediaDecodeError::Video(error.to_string()))?;
    let header = reader.header();
    if header.fourcc != *b"VP80" {
        return Err(MediaDecodeError::InvalidData("IVF codec is not VP8"));
    }
    let (scale, rate) = header.timebase;
    if scale == 0 || rate == 0 {
        return Err(MediaDecodeError::InvalidData("IVF time base is zero"));
    }

    let mut decoder = Vp8Decoder::new();
    let mut frames = Vec::new();
    let mut decoded_bytes = 0usize;
    for packet in reader {
        if is_cancelled(cancelled) {
            return Ok(None);
        }
        let packet = packet.map_err(|error| MediaDecodeError::Video(error.to_string()))?;
        let timestamp_ns = timebase_to_ns(packet.timestamp, u64::from(scale), u64::from(rate))?;
        if let Some(picture) = decoder.decode(packet.data).map_err(MediaDecodeError::Vp8)? {
            check_thumbnail_video_dimensions(picture.width, picture.height, first_frame_only)?;
            push_video_picture(
                &mut frames,
                &mut decoded_bytes,
                picture,
                timestamp_ns,
                None,
                max_frames,
                max_sequence_bytes,
            )?;
            if first_frame_only {
                return Ok(Some(frames));
            }
        }
    }
    Ok(Some(frames))
}

/// Decode an IVF/VP8 sequence one frame at a time without allocating a full decoded timeline.
pub(crate) fn decode_ivf_vp8_stream_cancellable(
    bytes: &[u8],
    cancelled: &AtomicBool,
    max_frames: usize,
    max_frame_bytes: usize,
    mut emit: impl FnMut(VideoFrame) -> bool,
) -> Result<bool, MediaDecodeError> {
    let reader = vp8_decoder::ivf::IvfReader::new(bytes)
        .map_err(|error| MediaDecodeError::Video(error.to_string()))?;
    let header = reader.header();
    if header.fourcc != *b"VP80" {
        return Err(MediaDecodeError::InvalidData("IVF codec is not VP8"));
    }
    let (scale, rate) = header.timebase;
    if scale == 0 || rate == 0 {
        return Err(MediaDecodeError::InvalidData("IVF time base is zero"));
    }

    let max_frames = max_frames.min(MAX_STREAMED_MEDIA_FRAMES);
    let frame_duration_ns = timebase_to_ns(1, u64::from(scale), u64::from(rate))?;
    let max_frame_bytes = max_frame_bytes.min(MAX_DECODED_SEQUENCE_BYTES);
    let mut decoder = Vp8Decoder::new();
    let mut frame_count = 0usize;
    for packet in reader {
        if cancelled.load(Ordering::Acquire) {
            return Ok(false);
        }
        let packet = packet.map_err(|error| MediaDecodeError::Video(error.to_string()))?;
        let timestamp_ns = timebase_to_ns(packet.timestamp, u64::from(scale), u64::from(rate))?;
        if let Some(picture) = decoder.decode(packet.data).map_err(MediaDecodeError::Vp8)? {
            if frame_count >= max_frames {
                return Err(MediaDecodeError::FrameLimit);
            }
            RgbaFrame::check_dimensions(picture.width, picture.height)?;
            if rgba_frame_byte_len(picture.width, picture.height)? > max_frame_bytes {
                return Err(MediaDecodeError::SequenceLimit);
            }
            let frame = picture_to_rgba(picture)?;
            if !emit(VideoFrame {
                timestamp_ns,
                duration_ns: Some(frame_duration_ns),
                frame,
            }) {
                cancelled.store(true, Ordering::Release);
                return Ok(false);
            }
            frame_count = frame_count.saturating_add(1);
        }
    }
    if frame_count == 0 {
        return Err(MediaDecodeError::InvalidData(
            "IVF contains no decoded frames",
        ));
    }
    Ok(true)
}

/// Decode the VP8 video track in a WebM/Matroska file.
///
/// The decoder selects the first enabled video track whose codec is `V_VP8`. It supports standard
/// `SimpleBlock` and `BlockGroup` packets, all block-lacing modes handled by `webm-iterable`, and
/// presentation timestamps. Other codecs, encrypted tracks, and unsupported/malformed packets
/// return an explicit error. Audio tracks are ignored.
pub fn decode_webm_vp8(bytes: &[u8]) -> Result<Vec<VideoFrame>, MediaDecodeError> {
    decode_webm_vp8_inner(
        bytes,
        None,
        false,
        MAX_DECODED_FRAMES,
        MAX_DECODED_SEQUENCE_BYTES,
    )?
    .ok_or(MediaDecodeError::InvalidData("WebM decode was cancelled"))
}

#[cfg(test)]
pub(crate) fn decode_webm_vp8_cancellable_with_limits(
    bytes: &[u8],
    cancelled: &AtomicBool,
    max_frames: usize,
    max_sequence_bytes: usize,
) -> Result<Option<Vec<VideoFrame>>, MediaDecodeError> {
    decode_webm_vp8_inner(
        bytes,
        Some(cancelled),
        false,
        max_frames.min(MAX_DECODED_FRAMES),
        max_sequence_bytes.min(MAX_DECODED_SEQUENCE_BYTES),
    )
}

fn decode_webm_vp8_inner(
    bytes: &[u8],
    cancelled: Option<&AtomicBool>,
    first_frame_only: bool,
    max_frames: usize,
    max_sequence_bytes: usize,
) -> Result<Option<Vec<VideoFrame>>, MediaDecodeError> {
    if bytes.is_empty() {
        return Err(MediaDecodeError::InvalidData("empty WebM file"));
    }

    let buffer_full_track_entries = [MatroskaSpec::TrackEntry(Master::Full(Vec::new()))];
    let mut source = Cursor::new(bytes);
    let tags = WebmIterator::new(&mut source, &buffer_full_track_entries);
    let mut timestamp_scale_ns = 1_000_000_u64;
    let mut track = None;
    let mut saw_video_track = false;
    let mut in_cluster = false;
    let mut cluster_timestamp = 0_u64;
    let mut in_block_group = false;
    let mut pending_block: Option<PendingVideoBlock> = None;
    let mut decoder = Vp8Decoder::new();
    let mut decoded = Vec::new();
    let mut decoded_bytes = 0usize;

    for tag in tags {
        if is_cancelled(cancelled) {
            return Ok(None);
        }
        let tag = tag.map_err(|error| MediaDecodeError::Video(error.to_string()))?;
        match tag {
            MatroskaSpec::TimestampScale(value) => {
                timestamp_scale_ns = value;
                if timestamp_scale_ns == 0 {
                    return Err(MediaDecodeError::InvalidData("WebM TimestampScale is zero"));
                }
            }
            MatroskaSpec::TrackEntry(Master::Full(children)) => {
                let track_type =
                    child_unsigned(&children, |tag| matches!(tag, MatroskaSpec::TrackType(_)));
                if track_type == Some(1) {
                    saw_video_track = true;
                    let codec = children
                        .iter()
                        .find(|tag| matches!(tag, MatroskaSpec::CodecID(_)))
                        .and_then(EbmlTag::as_utf8);
                    if codec == Some("V_VP8") && track.is_none() {
                        let number = child_unsigned(&children, |tag| {
                            matches!(tag, MatroskaSpec::TrackNumber(_))
                        })
                        .ok_or(MediaDecodeError::InvalidData(
                            "VP8 track has no TrackNumber",
                        ))?;
                        let default_duration_ns = child_unsigned(&children, |tag| {
                            matches!(tag, MatroskaSpec::DefaultDuration(_))
                        });
                        let timestamp_scale = children
                            .iter()
                            .find(|tag| matches!(tag, MatroskaSpec::TrackTimestampScale(_)))
                            .and_then(EbmlTag::as_float)
                            .copied()
                            .unwrap_or(1.0);
                        if !timestamp_scale.is_finite() || timestamp_scale <= 0.0 {
                            return Err(MediaDecodeError::InvalidData(
                                "invalid TrackTimestampScale",
                            ));
                        }
                        track = Some(VideoTrack {
                            number,
                            default_duration_ns,
                            timestamp_scale,
                        });
                    }
                }
            }
            MatroskaSpec::Cluster(Master::Start) => {
                in_cluster = true;
                cluster_timestamp = 0;
            }
            MatroskaSpec::Cluster(Master::End) => {
                in_cluster = false;
            }
            MatroskaSpec::Timestamp(value) if in_cluster => cluster_timestamp = value,
            MatroskaSpec::BlockGroup(Master::Start) => {
                in_block_group = true;
                pending_block = None;
            }
            MatroskaSpec::BlockGroup(Master::End) => {
                in_block_group = false;
                if let Some(block) = pending_block.take()
                    && let Some(track) = track.filter(|track| track.number == block.track)
                {
                    decode_video_block(
                        &mut decoder,
                        &mut decoded,
                        &mut decoded_bytes,
                        block,
                        DecodeBlockContext {
                            cluster_timestamp,
                            timestamp_scale_ns,
                            track,
                            cancelled,
                            first_frame_only,
                            max_frames,
                            max_sequence_bytes,
                        },
                    )?;
                    if first_frame_only && !decoded.is_empty() {
                        return Ok(Some(decoded));
                    }
                }
            }
            MatroskaSpec::BlockDuration(value) if in_block_group => {
                if let Some(block) = pending_block.as_mut() {
                    block.duration_ticks = Some(value);
                }
            }
            MatroskaSpec::SimpleBlock(data) if in_cluster => {
                let block = SimpleBlock::try_from(&data[..])
                    .map_err(|error| MediaDecodeError::Video(error.to_string()))?;
                let packet = PendingVideoBlock {
                    track: block.track,
                    relative_timestamp: block.timestamp,
                    frames: block
                        .read_frame_data()
                        .map_err(|error| MediaDecodeError::Video(error.to_string()))?
                        .into_iter()
                        .map(|frame| frame.data.to_vec())
                        .collect(),
                    duration_ticks: None,
                };
                if !in_block_group && track.is_some_and(|track| track.number == packet.track) {
                    decode_video_block(
                        &mut decoder,
                        &mut decoded,
                        &mut decoded_bytes,
                        packet,
                        DecodeBlockContext {
                            cluster_timestamp,
                            timestamp_scale_ns,
                            track: track.expect("checked above"),
                            cancelled,
                            first_frame_only,
                            max_frames,
                            max_sequence_bytes,
                        },
                    )?;
                    if first_frame_only && !decoded.is_empty() {
                        return Ok(Some(decoded));
                    }
                }
            }
            MatroskaSpec::Block(data) if in_block_group => {
                let block = Block::try_from(&data[..])
                    .map_err(|error| MediaDecodeError::Video(error.to_string()))?;
                pending_block = Some(PendingVideoBlock {
                    track: block.track,
                    relative_timestamp: block.timestamp,
                    frames: block
                        .read_frame_data()
                        .map_err(|error| MediaDecodeError::Video(error.to_string()))?
                        .into_iter()
                        .map(|frame| frame.data.to_vec())
                        .collect(),
                    duration_ticks: None,
                });
            }
            _ => {}
        }
    }

    if track.is_none() {
        return Err(if saw_video_track {
            MediaDecodeError::InvalidData("WebM has no supported V_VP8 video track")
        } else {
            MediaDecodeError::InvalidData("WebM has no video track")
        });
    }
    if in_block_group {
        return Err(MediaDecodeError::InvalidData(
            "unterminated WebM BlockGroup",
        ));
    }
    Ok(Some(decoded))
}

/// Decode a WebM/Matroska VP8 sequence a frame at a time without retaining the full timeline.
pub(crate) fn decode_webm_vp8_stream_cancellable(
    bytes: &[u8],
    cancelled: &AtomicBool,
    max_frames: usize,
    max_frame_bytes: usize,
    mut emit: impl FnMut(VideoFrame) -> bool,
) -> Result<bool, MediaDecodeError> {
    if bytes.is_empty() {
        return Err(MediaDecodeError::InvalidData("empty WebM file"));
    }

    let buffer_full_track_entries = [MatroskaSpec::TrackEntry(Master::Full(Vec::new()))];
    let mut source = Cursor::new(bytes);
    let tags = WebmIterator::new(&mut source, &buffer_full_track_entries);
    let mut timestamp_scale_ns = 1_000_000_u64;
    let mut track = None;
    let mut saw_video_track = false;
    let mut in_cluster = false;
    let mut cluster_timestamp = 0_u64;
    let mut in_block_group = false;
    let mut pending_block: Option<PendingVideoBlock> = None;
    let mut decoder = Vp8Decoder::new();
    let mut frame_count = 0usize;
    let block_context = |cluster_timestamp, timestamp_scale_ns, track| StreamBlockContext {
        cluster_timestamp,
        timestamp_scale_ns,
        track,
        cancelled,
        max_frames: max_frames.min(MAX_STREAMED_MEDIA_FRAMES),
        max_frame_bytes: max_frame_bytes.min(MAX_DECODED_SEQUENCE_BYTES),
    };

    for tag in tags {
        if cancelled.load(Ordering::Acquire) {
            return Ok(false);
        }
        let tag = tag.map_err(|error| MediaDecodeError::Video(error.to_string()))?;
        match tag {
            MatroskaSpec::TimestampScale(value) => {
                timestamp_scale_ns = value;
                if timestamp_scale_ns == 0 {
                    return Err(MediaDecodeError::InvalidData("WebM TimestampScale is zero"));
                }
            }
            MatroskaSpec::TrackEntry(Master::Full(children)) => {
                let track_type =
                    child_unsigned(&children, |tag| matches!(tag, MatroskaSpec::TrackType(_)));
                if track_type == Some(1) {
                    saw_video_track = true;
                    let codec = children
                        .iter()
                        .find(|tag| matches!(tag, MatroskaSpec::CodecID(_)))
                        .and_then(EbmlTag::as_utf8);
                    if codec == Some("V_VP8") && track.is_none() {
                        let number = child_unsigned(&children, |tag| {
                            matches!(tag, MatroskaSpec::TrackNumber(_))
                        })
                        .ok_or(MediaDecodeError::InvalidData(
                            "VP8 track has no TrackNumber",
                        ))?;
                        let default_duration_ns = child_unsigned(&children, |tag| {
                            matches!(tag, MatroskaSpec::DefaultDuration(_))
                        });
                        let timestamp_scale = children
                            .iter()
                            .find(|tag| matches!(tag, MatroskaSpec::TrackTimestampScale(_)))
                            .and_then(EbmlTag::as_float)
                            .copied()
                            .unwrap_or(1.0);
                        if !timestamp_scale.is_finite() || timestamp_scale <= 0.0 {
                            return Err(MediaDecodeError::InvalidData(
                                "invalid TrackTimestampScale",
                            ));
                        }
                        track = Some(VideoTrack {
                            number,
                            default_duration_ns,
                            timestamp_scale,
                        });
                    }
                }
            }
            MatroskaSpec::Cluster(Master::Start) => {
                in_cluster = true;
                cluster_timestamp = 0;
            }
            MatroskaSpec::Cluster(Master::End) => in_cluster = false,
            MatroskaSpec::Timestamp(value) if in_cluster => cluster_timestamp = value,
            MatroskaSpec::BlockGroup(Master::Start) => {
                in_block_group = true;
                pending_block = None;
            }
            MatroskaSpec::BlockGroup(Master::End) => {
                in_block_group = false;
                if let Some(block) = pending_block.take()
                    && let Some(track) = track.filter(|track| track.number == block.track)
                    && !decode_video_block_stream(
                        &mut decoder,
                        &mut frame_count,
                        block,
                        block_context(cluster_timestamp, timestamp_scale_ns, track),
                        &mut emit,
                    )?
                {
                    return Ok(false);
                }
            }
            MatroskaSpec::BlockDuration(value) if in_block_group => {
                if let Some(block) = pending_block.as_mut() {
                    block.duration_ticks = Some(value);
                }
            }
            MatroskaSpec::SimpleBlock(data) if in_cluster => {
                let block = SimpleBlock::try_from(&data[..])
                    .map_err(|error| MediaDecodeError::Video(error.to_string()))?;
                let packet = PendingVideoBlock {
                    track: block.track,
                    relative_timestamp: block.timestamp,
                    frames: block
                        .read_frame_data()
                        .map_err(|error| MediaDecodeError::Video(error.to_string()))?
                        .into_iter()
                        .map(|frame| frame.data.to_vec())
                        .collect(),
                    duration_ticks: None,
                };
                if !in_block_group
                    && let Some(track) = track.filter(|track| track.number == packet.track)
                    && !decode_video_block_stream(
                        &mut decoder,
                        &mut frame_count,
                        packet,
                        block_context(cluster_timestamp, timestamp_scale_ns, track),
                        &mut emit,
                    )?
                {
                    return Ok(false);
                }
            }
            MatroskaSpec::Block(data) if in_block_group => {
                let block = Block::try_from(&data[..])
                    .map_err(|error| MediaDecodeError::Video(error.to_string()))?;
                pending_block = Some(PendingVideoBlock {
                    track: block.track,
                    relative_timestamp: block.timestamp,
                    frames: block
                        .read_frame_data()
                        .map_err(|error| MediaDecodeError::Video(error.to_string()))?
                        .into_iter()
                        .map(|frame| frame.data.to_vec())
                        .collect(),
                    duration_ticks: None,
                });
            }
            _ => {}
        }
    }

    if track.is_none() {
        return Err(if saw_video_track {
            MediaDecodeError::InvalidData("WebM has no supported V_VP8 video track")
        } else {
            MediaDecodeError::InvalidData("WebM has no video track")
        });
    }
    if in_block_group {
        return Err(MediaDecodeError::InvalidData(
            "unterminated WebM BlockGroup",
        ));
    }
    if frame_count == 0 {
        return Err(MediaDecodeError::InvalidData(
            "WebM contains no decoded frames",
        ));
    }
    Ok(true)
}

#[derive(Clone, Copy)]
struct StreamBlockContext<'a> {
    cluster_timestamp: u64,
    timestamp_scale_ns: u64,
    track: VideoTrack,
    cancelled: &'a AtomicBool,
    max_frames: usize,
    max_frame_bytes: usize,
}

fn decode_video_block_stream(
    decoder: &mut Vp8Decoder,
    frame_count: &mut usize,
    block: PendingVideoBlock,
    context: StreamBlockContext<'_>,
    emit: &mut impl FnMut(VideoFrame) -> bool,
) -> Result<bool, MediaDecodeError> {
    if block.frames.is_empty() {
        return Err(MediaDecodeError::InvalidData(
            "WebM video block has no frames",
        ));
    }
    let timestamp_ticks =
        i128::from(context.cluster_timestamp) + i128::from(block.relative_timestamp);
    if timestamp_ticks < 0 || timestamp_ticks > i128::from(u64::MAX) {
        return Err(MediaDecodeError::InvalidData(
            "WebM block timestamp is out of range",
        ));
    }
    let timestamp_ns = webm_time_to_ns(
        timestamp_ticks as u64,
        context.timestamp_scale_ns,
        context.track.timestamp_scale,
    )?;
    let block_duration_ns = block
        .duration_ticks
        .map(|duration| {
            webm_time_to_ns(
                duration,
                context.timestamp_scale_ns,
                context.track.timestamp_scale,
            )
        })
        .transpose()?
        .or(context.track.default_duration_ns);
    let block_frame_count = block.frames.len() as u64;
    let duration_per_frame = block_duration_ns.map(|duration| duration / block_frame_count);

    for (index, compressed) in block.frames.iter().enumerate() {
        if context.cancelled.load(Ordering::Acquire) {
            return Ok(false);
        }
        let frame_timestamp_ns = duration_per_frame
            .and_then(|step| step.checked_mul(index as u64))
            .and_then(|offset| timestamp_ns.checked_add(offset))
            .unwrap_or(timestamp_ns);
        if let Some(picture) = decoder.decode(compressed).map_err(MediaDecodeError::Vp8)? {
            if *frame_count >= context.max_frames {
                return Err(MediaDecodeError::FrameLimit);
            }
            RgbaFrame::check_dimensions(picture.width, picture.height)?;
            if rgba_frame_byte_len(picture.width, picture.height)? > context.max_frame_bytes {
                return Err(MediaDecodeError::SequenceLimit);
            }
            let frame = picture_to_rgba(picture)?;
            if !emit(VideoFrame {
                timestamp_ns: frame_timestamp_ns,
                duration_ns: duration_per_frame,
                frame,
            }) {
                context.cancelled.store(true, Ordering::Release);
                return Ok(false);
            }
            *frame_count = frame_count.saturating_add(1);
        }
    }
    Ok(true)
}

fn is_cancelled(cancelled: Option<&AtomicBool>) -> bool {
    cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
}

#[derive(Clone, Copy)]
struct VideoTrack {
    number: u64,
    default_duration_ns: Option<u64>,
    timestamp_scale: f64,
}

struct PendingVideoBlock {
    track: u64,
    relative_timestamp: i16,
    frames: Vec<Vec<u8>>,
    duration_ticks: Option<u64>,
}

#[derive(Clone, Copy)]
struct DecodeBlockContext<'a> {
    cluster_timestamp: u64,
    timestamp_scale_ns: u64,
    track: VideoTrack,
    cancelled: Option<&'a AtomicBool>,
    first_frame_only: bool,
    max_frames: usize,
    max_sequence_bytes: usize,
}

fn child_unsigned(
    children: &[MatroskaSpec],
    matches: impl Fn(&MatroskaSpec) -> bool,
) -> Option<u64> {
    children
        .iter()
        .find(|tag| matches(tag))
        .and_then(EbmlTag::as_unsigned_int)
        .copied()
}

fn decode_video_block(
    decoder: &mut Vp8Decoder,
    output: &mut Vec<VideoFrame>,
    decoded_bytes: &mut usize,
    block: PendingVideoBlock,
    context: DecodeBlockContext<'_>,
) -> Result<(), MediaDecodeError> {
    if block.frames.is_empty() {
        return Err(MediaDecodeError::InvalidData(
            "WebM video block has no frames",
        ));
    }
    let timestamp_ticks =
        i128::from(context.cluster_timestamp) + i128::from(block.relative_timestamp);
    if timestamp_ticks < 0 || timestamp_ticks > i128::from(u64::MAX) {
        return Err(MediaDecodeError::InvalidData(
            "WebM block timestamp is out of range",
        ));
    }
    let timestamp_ns = webm_time_to_ns(
        timestamp_ticks as u64,
        context.timestamp_scale_ns,
        context.track.timestamp_scale,
    )?;
    let block_duration_ns = block
        .duration_ticks
        .map(|duration| {
            webm_time_to_ns(
                duration,
                context.timestamp_scale_ns,
                context.track.timestamp_scale,
            )
        })
        .transpose()?
        .or(context.track.default_duration_ns);
    let frame_count = block.frames.len() as u64;
    let duration_per_frame = block_duration_ns.map(|duration| duration / frame_count);

    for (index, compressed) in block.frames.iter().enumerate() {
        if is_cancelled(context.cancelled) {
            break;
        }
        let timestamp_ns = duration_per_frame
            .and_then(|step| step.checked_mul(index as u64))
            .and_then(|offset| timestamp_ns.checked_add(offset))
            .unwrap_or(timestamp_ns);
        if let Some(picture) = decoder.decode(compressed).map_err(MediaDecodeError::Vp8)? {
            check_thumbnail_video_dimensions(
                picture.width,
                picture.height,
                context.first_frame_only,
            )?;
            push_video_picture(
                output,
                decoded_bytes,
                picture,
                timestamp_ns,
                duration_per_frame,
                context.max_frames,
                context.max_sequence_bytes,
            )?;
            if context.first_frame_only {
                break;
            }
        }
    }
    Ok(())
}

fn check_thumbnail_video_dimensions(
    width: u32,
    height: u32,
    first_frame_only: bool,
) -> Result<(), MediaDecodeError> {
    if first_frame_only
        && (width > MAX_THUMBNAIL_SOURCE_DIMENSION || height > MAX_THUMBNAIL_SOURCE_DIMENSION)
    {
        return Err(MediaDecodeError::PixelLimit);
    }
    Ok(())
}

fn push_video_picture(
    output: &mut Vec<VideoFrame>,
    decoded_bytes: &mut usize,
    picture: Picture<'_>,
    timestamp_ns: u64,
    duration_ns: Option<u64>,
    max_frames: usize,
    max_sequence_bytes: usize,
) -> Result<(), MediaDecodeError> {
    if output.len() >= max_frames {
        return Err(MediaDecodeError::FrameLimit);
    }
    RgbaFrame::check_dimensions(picture.width, picture.height)?;
    let frame_bytes = u64::from(picture.width)
        .checked_mul(u64::from(picture.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    let total_bytes = decoded_bytes
        .checked_add(frame_bytes)
        .ok_or(MediaDecodeError::SequenceLimit)?;
    if total_bytes > max_sequence_bytes {
        return Err(MediaDecodeError::SequenceLimit);
    }
    let frame = picture_to_rgba(picture)?;
    *decoded_bytes = total_bytes;
    output.push(VideoFrame {
        timestamp_ns,
        duration_ns,
        frame,
    });
    Ok(())
}

fn webm_time_to_ns(
    ticks: u64,
    timestamp_scale_ns: u64,
    track_scale: f64,
) -> Result<u64, MediaDecodeError> {
    let nanoseconds = ticks as f64 * timestamp_scale_ns as f64 * track_scale;
    if !nanoseconds.is_finite() || nanoseconds < 0.0 || nanoseconds > u64::MAX as f64 {
        return Err(MediaDecodeError::DimensionsOverflow);
    }
    Ok(nanoseconds.round() as u64)
}

fn picture_to_rgba(picture: Picture<'_>) -> Result<RgbaFrame, MediaDecodeError> {
    RgbaFrame::check_dimensions(picture.width, picture.height)?;
    let width = usize::try_from(picture.width).map_err(|_| MediaDecodeError::DimensionsOverflow)?;
    let height =
        usize::try_from(picture.height).map_err(|_| MediaDecodeError::DimensionsOverflow)?;
    let byte_count = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(MediaDecodeError::DimensionsOverflow)?;
    let chroma_width = picture.chroma_width() as usize;
    let mut rgba = vec![0; byte_count];

    for y in 0..height {
        for x in 0..width {
            let luma = i32::from(picture.y[y * picture.y_stride + x]);
            let chroma_index = (y / 2) * picture.uv_stride + (x / 2).min(chroma_width - 1);
            let cb = i32::from(picture.u[chroma_index]) - 128;
            let cr = i32::from(picture.v[chroma_index]) - 128;
            let c = (luma - 16).max(0);
            let out = (y * width + x) * 4;
            rgba[out] = clamp_byte((298 * c + 409 * cr + 128) >> 8);
            rgba[out + 1] = clamp_byte((298 * c - 100 * cb - 208 * cr + 128) >> 8);
            rgba[out + 2] = clamp_byte((298 * c + 516 * cb + 128) >> 8);
            rgba[out + 3] = 255;
        }
    }

    RgbaFrame::new(picture.width, picture.height, rgba).map_err(MediaDecodeError::InvalidData)
}

fn clamp_byte(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

fn unpremultiply_rgba(pixels: &mut [u8]) {
    for rgba in pixels.as_chunks_mut::<4>().0 {
        let alpha = u16::from(rgba[3]);
        if alpha == 0 {
            rgba[..3].fill(0);
        } else if alpha < 255 {
            for channel in &mut rgba[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
}

fn timebase_to_ns(timestamp: u64, scale: u64, rate: u64) -> Result<u64, MediaDecodeError> {
    let nanos = u128::from(timestamp)
        .checked_mul(u128::from(scale))
        .and_then(|value| value.checked_mul(1_000_000_000))
        .ok_or(MediaDecodeError::DimensionsOverflow)?
        / u128::from(rate);
    u64::try_from(nanos).map_err(|_| MediaDecodeError::DimensionsOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H264_MP4_FIXTURE: &[u8] = include_bytes!("../assets/sample-h264.mp4");
    const H264_10BIT_MP4_FIXTURE: &[u8] = include_bytes!("../assets/sample-h264-10bit.mp4");
    const H264_422_MP4_FIXTURE: &[u8] = include_bytes!("../assets/sample-h264-422.mp4");
    const H264_444_MP4_FIXTURE: &[u8] = include_bytes!("../assets/sample-h264-444.mp4");
    const H264_LONG_MP4_FIXTURE: &[u8] = include_bytes!("../assets/sample-h264-long.mp4");

    const VP8_KEYFRAME_16X16: &[u8] = &[
        48, 1, 0, 157, 1, 42, 16, 0, 16, 0, 1, 64, 38, 37, 160, 0, 3, 112, 0, 254, 240, 83, 31,
        255, 226, 223, 180, 127, 123, 101, 7, 255, 253, 251, 75, 255, 240, 39, 127, 255, 2, 119,
        253, 12, 0, 0,
    ];

    const TWO_FRAME_GIF: &[u8] = &[
        71, 73, 70, 56, 57, 97, 2, 0, 1, 0, 129, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33,
        255, 11, 78, 69, 84, 83, 67, 65, 80, 69, 50, 46, 48, 3, 1, 0, 0, 0, 33, 249, 4, 8, 10, 0,
        0, 0, 44, 0, 0, 0, 0, 2, 0, 1, 0, 0, 8, 5, 0, 1, 0, 8, 8, 0, 33, 249, 4, 9, 25, 0, 2, 0,
        44, 0, 0, 0, 0, 2, 0, 1, 0, 129, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 5, 0, 1, 0, 8, 8,
        0, 59,
    ];

    fn vp8_ivf_fixture(frame: &[u8]) -> Vec<u8> {
        let mut ivf = Vec::new();
        ivf.extend_from_slice(b"DKIF");
        ivf.extend_from_slice(&0u16.to_le_bytes());
        ivf.extend_from_slice(&32u16.to_le_bytes());
        ivf.extend_from_slice(b"VP80");
        ivf.extend_from_slice(&16u16.to_le_bytes());
        ivf.extend_from_slice(&16u16.to_le_bytes());
        ivf.extend_from_slice(&30u32.to_le_bytes());
        ivf.extend_from_slice(&1u32.to_le_bytes());
        ivf.extend_from_slice(&1u32.to_le_bytes());
        ivf.extend_from_slice(&0u32.to_le_bytes());
        ivf.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        ivf.extend_from_slice(&0u64.to_le_bytes());
        ivf.extend_from_slice(frame);
        ivf
    }

    #[test]
    fn rgba_frame_rejects_wrong_lengths_and_overflow() {
        assert!(RgbaFrame::new(2, 2, vec![0; 15]).is_err());
        assert_eq!(
            RgbaFrame::new(u32::MAX, 2, vec![]),
            Err("image dimensions overflow")
        );
        assert_eq!(RgbaFrame::new(0, 1, vec![]).unwrap().pixels.len(), 0);
    }

    #[test]
    fn decodes_png_pixels_to_straight_rgba() {
        const PNG_1X1: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        let frame = decode_image(PNG_1X1).unwrap();
        assert_eq!((frame.width, frame.height), (1, 1));
        assert_eq!(frame.pixels.len(), 4);
        assert_eq!(frame.pixels[3], 255);
    }

    #[test]
    fn raster_svg_and_gif_thumbnails_are_bounded_and_keep_aspect_ratio() {
        let source = image::RgbaImage::from_pixel(16, 8, image::Rgba([200, 100, 50, 255]));
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let raster = decode_image_thumbnail(png.get_ref(), 4, 4).unwrap();
        assert_eq!((raster.width, raster.height), (4, 2));
        assert_eq!(&raster.pixels[..4], &[200, 100, 50, 255]);

        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#40c0e0"/></svg>"##;
        let vector = decode_svg_thumbnail(svg, 5, 5).unwrap();
        assert_eq!((vector.width, vector.height), (5, 3));
        assert_eq!(&vector.pixels[..3], &[64, 192, 224]);

        let animated = decode_gif_thumbnail(TWO_FRAME_GIF, 1, 1).unwrap();
        assert_eq!((animated.width, animated.height), (1, 1));
        assert_eq!(animated.pixels[0], 255);
        assert_eq!(animated.pixels[1], 0);
    }

    #[test]
    fn decodes_and_unpremultiplies_svg() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="1" height="1" fill="#ff0000" fill-opacity="0.5"/></svg>"##;
        let frame = decode_svg(svg).unwrap();
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.pixels[0], 255);
        assert_eq!(frame.pixels[3], 128);
        assert_eq!(frame.pixels[7], 0);
    }

    #[test]
    fn decodes_composited_gif_frames_and_delays() {
        let frames = decode_gif(TWO_FRAME_GIF).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!((frames[0].frame.width, frames[0].frame.height), (2, 1));
        assert_eq!(frames[0].delay, Duration::from_millis(100));
        assert_eq!(frames[1].delay, Duration::from_millis(250));
        assert_eq!(frames[0].frame.pixels[0..3], [255, 0, 0]);
        assert_eq!(frames[1].frame.pixels[0..3], [0, 0, 255]);
    }

    #[test]
    fn rejects_bad_ivf_and_unknown_fourcc() {
        assert!(decode_ivf_vp8(b"not an IVF file").is_err());
        let mut bytes = vec![0; 32];
        bytes[..4].copy_from_slice(b"DKIF");
        bytes[6..8].copy_from_slice(&32u16.to_le_bytes());
        bytes[8..12].copy_from_slice(b"AV01");
        bytes[16..20].copy_from_slice(&30u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(
            decode_ivf_vp8(&bytes),
            Err(MediaDecodeError::InvalidData(_))
        ));
    }

    #[test]
    fn yuv420_conversion_uses_neutral_chroma_and_checked_timestamp_math() {
        let i420 = [235, 128, 128];
        let picture = Picture::from_i420(&i420, 1, 1).unwrap();
        let frame = picture_to_rgba(picture).unwrap();
        assert_eq!(frame.pixels, [255, 255, 255, 255]);
        assert_eq!(timebase_to_ns(3, 1, 30).unwrap(), 100_000_000);
        assert!(timebase_to_ns(u64::MAX, u64::MAX, 1).is_err());
    }

    #[test]
    fn decodes_real_vp8_frame_from_ivf() {
        let ivf = vp8_ivf_fixture(VP8_KEYFRAME_16X16);

        let frames = decode_ivf_vp8(&ivf).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].timestamp_ns, 0);
        assert_eq!((frames[0].frame.width, frames[0].frame.height), (16, 16));
        assert!(frames[0].frame.pixels[0] > frames[0].frame.pixels[1]);
        assert!(frames[0].frame.pixels[0] > frames[0].frame.pixels[2]);

        let thumbnail = decode_ivf_vp8_thumbnail(&ivf, 8, 8).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (8, 8));
        assert!(thumbnail.pixels[0] > thumbnail.pixels[1]);
    }

    #[test]
    fn vp8_sequence_decoders_stop_at_configured_frame_and_byte_limits() {
        let cancelled = AtomicBool::new(false);
        let ivf = vp8_ivf_fixture(VP8_KEYFRAME_16X16);
        let frame_limit =
            decode_ivf_vp8_cancellable_with_limits(&ivf, &cancelled, 0, usize::MAX).unwrap_err();
        assert!(matches!(frame_limit, MediaDecodeError::FrameLimit));
        let byte_limit =
            decode_ivf_vp8_cancellable_with_limits(&ivf, &cancelled, 1, 0).unwrap_err();
        assert!(matches!(byte_limit, MediaDecodeError::SequenceLimit));

        let webm = include_bytes!("../assets/sample.webm");
        let frame_limit =
            decode_webm_vp8_cancellable_with_limits(webm, &cancelled, 0, usize::MAX).unwrap_err();
        assert!(matches!(frame_limit, MediaDecodeError::FrameLimit));
        let byte_limit =
            decode_webm_vp8_cancellable_with_limits(webm, &cancelled, usize::MAX, 0).unwrap_err();
        assert!(matches!(byte_limit, MediaDecodeError::SequenceLimit));
    }

    #[test]
    fn decodes_bounded_h264_mp4_poster_and_cancellation() {
        let poster = decode_mp4_h264_thumbnail(H264_MP4_FIXTURE, 16, 16).unwrap();
        assert_eq!((poster.width, poster.height), (16, 9));
        assert_eq!(poster.pixels.len(), 16 * 9 * 4);
        assert!(
            poster
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 255)
        );
        let colors: std::collections::HashSet<[u8; 3]> = poster
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();
        assert!(
            colors.len() > 8,
            "synthetic H.264 pattern lost color detail"
        );

        let cancelled = AtomicBool::new(true);
        assert!(
            decode_mp4_h264_thumbnail_cancellable(H264_MP4_FIXTURE.to_vec(), 16, 16, &cancelled,)
                .unwrap()
                .is_none()
        );

        let mut mov = H264_MP4_FIXTURE.to_vec();
        mov[8..12].copy_from_slice(b"qt  ");
        let mov_poster = decode_mp4_h264_thumbnail(mov, 16, 16).unwrap();
        assert_eq!((mov_poster.width, mov_poster.height), (16, 9));
    }

    #[test]
    fn h264_mp4_mov_sequences_decode_all_display_frames_with_bounded_timing() {
        let frames = decode_mp4_h264(H264_MP4_FIXTURE).unwrap();
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|frame| {
            (frame.frame.width, frame.frame.height) == (32, 18)
                && frame.frame.pixels.len() == 32 * 18 * 4
                && frame
                    .frame
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == 255)
        }));
        assert_eq!(frames[0].timestamp_ns, 0);
        assert!(frames[0].duration_ns.unwrap() > 0);
        assert!(frames[1].timestamp_ns > frames[0].timestamp_ns);

        let mut mov = H264_MP4_FIXTURE.to_vec();
        mov[8..12].copy_from_slice(b"qt  ");
        let mov_frames = decode_mp4_h264(mov).unwrap();
        assert_eq!(mov_frames.len(), 2);
        assert_eq!(mov_frames[0].frame.pixels, frames[0].frame.pixels);

        for fixture in [H264_10BIT_MP4_FIXTURE, H264_422_MP4_FIXTURE] {
            let decoded = decode_mp4_h264(fixture).unwrap();
            assert_eq!(decoded.len(), 2);
            assert!(decoded.iter().all(|frame| {
                (frame.frame.width, frame.frame.height) == (32, 18)
                    && frame
                        .frame
                        .pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|pixel| pixel[3] == 255)
            }));
        }

        let cancelled = AtomicBool::new(true);
        assert!(
            decode_mp4_h264_sequence_cancellable(
                Arc::from(H264_MP4_FIXTURE),
                &cancelled,
                MAX_DECODED_FRAMES,
                MAX_DECODED_SEQUENCE_BYTES,
                H264_MP4_FIXTURE.len(),
            )
            .unwrap()
            .is_none()
        );
        let error = decode_mp4_h264_sequence_inner(
            Arc::from(H264_MP4_FIXTURE),
            None,
            1,
            MAX_DECODED_SEQUENCE_BYTES,
            H264_MP4_FIXTURE.len(),
        )
        .unwrap_err();
        assert!(matches!(error, MediaDecodeError::FrameLimit));
        let error = decode_mp4_h264_sequence_inner(
            Arc::from(H264_MP4_FIXTURE),
            None,
            MAX_DECODED_FRAMES,
            32 * 18 * 4 - 1,
            H264_MP4_FIXTURE.len(),
        )
        .unwrap_err();
        assert!(matches!(error, MediaDecodeError::SequenceLimit));

        let error = decode_mp4_h264_sequence_inner(
            Arc::from(H264_444_MP4_FIXTURE),
            None,
            MAX_DECODED_FRAMES,
            MAX_DECODED_SEQUENCE_BYTES,
            H264_444_MP4_FIXTURE.len(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("unsupported H.264 pixel format"));
    }

    #[test]
    fn h264_stream_decodes_beyond_eager_limit_without_collecting_the_full_sequence() {
        assert!(matches!(
            decode_mp4_h264(H264_LONG_MP4_FIXTURE),
            Err(MediaDecodeError::FrameLimit)
        ));

        let cancelled = AtomicBool::new(false);
        let mut frame_count = 0usize;
        let mut previous_timestamp = None;
        let completed = decode_mp4_h264_stream_cancellable(
            Arc::from(H264_LONG_MP4_FIXTURE),
            &cancelled,
            H264_LONG_MP4_FIXTURE.len(),
            MAX_STREAMED_MEDIA_FRAMES,
            MAX_DECODED_SEQUENCE_BYTES,
            |frame| {
                assert_eq!((frame.frame.width, frame.frame.height), (32, 18));
                assert!(frame.duration_ns.unwrap() > 0);
                if let Some(previous_timestamp) = previous_timestamp {
                    assert!(frame.timestamp_ns > previous_timestamp);
                }
                previous_timestamp = Some(frame.timestamp_ns);
                frame_count += 1;
                true
            },
        )
        .unwrap();
        assert!(completed);
        assert_eq!(frame_count, 601);
    }

    #[test]
    fn h264_thumbnail_supports_ten_bit_420_and_eight_bit_422() {
        for fixture in [H264_10BIT_MP4_FIXTURE, H264_422_MP4_FIXTURE] {
            let poster = decode_mp4_h264_thumbnail(fixture, 16, 16).unwrap();
            assert_eq!((poster.width, poster.height), (16, 9));
            assert!(
                poster
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] == 255)
            );
        }
    }

    #[test]
    fn h264_mp4_poster_reports_unsupported_444_and_invalid_containers() {
        let error = decode_mp4_h264_thumbnail(H264_444_MP4_FIXTURE, 16, 16).unwrap_err();
        assert!(error.to_string().contains("unsupported H.264 pixel format"));

        let error =
            decode_mp4_h264_thumbnail(b"not an MP4 or MOV file".as_slice(), 16, 16).unwrap_err();
        assert!(error.to_string().contains("MP4/MOV demux failed"));
    }

    #[test]
    fn avcc_thumbnail_sample_parser_rejects_truncated_and_excessive_nal_lists() {
        assert_eq!(checked_avcc_nal_count(&[0, 0, 0, 1, 0x65], 4).unwrap(), 1);
        assert!(checked_avcc_nal_count(&[0, 0, 0], 4).is_err());
        assert!(checked_avcc_nal_count(&[0, 0, 0, 0], 4).is_err());
    }

    #[test]
    fn demuxes_and_decodes_webm_vp8_video() {
        let webm = tiny_webm_vp8(VP8_KEYFRAME_16X16);
        let frames = decode_webm_vp8(&webm).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].timestamp_ns, 10_000_000);
        assert_eq!(frames[0].duration_ns, Some(33_000_000));
        assert_eq!((frames[0].frame.width, frames[0].frame.height), (16, 16));
        assert!(frames[0].frame.pixels[0] > frames[0].frame.pixels[1]);

        let thumbnail = decode_webm_vp8_thumbnail(&webm, 8, 8).unwrap();
        assert_eq!((thumbnail.width, thumbnail.height), (8, 8));
        assert!(thumbnail.pixels[0] > thumbnail.pixels[1]);
    }

    #[test]
    fn webm_vp8_stream_decodes_beyond_eager_limit_without_collecting_frames() {
        const FRAME_COUNT: usize = 601;
        let webm = tiny_webm_vp8_frames(VP8_KEYFRAME_16X16, FRAME_COUNT);
        let cancelled = AtomicBool::new(false);
        let mut decoded = 0usize;
        let completed = decode_webm_vp8_stream_cancellable(
            &webm,
            &cancelled,
            FRAME_COUNT,
            16 * 16 * 4,
            |frame| {
                assert_eq!((frame.frame.width, frame.frame.height), (16, 16));
                assert!(frame.duration_ns.unwrap() > 0);
                decoded += 1;
                true
            },
        )
        .unwrap();
        assert!(completed);
        assert_eq!(decoded, FRAME_COUNT);
    }

    fn tiny_webm_vp8(frame: &[u8]) -> Vec<u8> {
        tiny_webm_vp8_frames(frame, 1)
    }

    fn tiny_webm_vp8_frames(frame: &[u8], frame_count: usize) -> Vec<u8> {
        fn element(id: &[u8], payload: &[u8]) -> Vec<u8> {
            let mut result = id.to_vec();
            result.extend(ebml_size(payload.len()));
            result.extend_from_slice(payload);
            result
        }

        fn ebml_size(size: usize) -> Vec<u8> {
            if size < 0x7f {
                vec![0x80 | size as u8]
            } else if size < 0x3fff {
                (0x4000u16 | size as u16).to_be_bytes().to_vec()
            } else if size < 0x0fff_ffff {
                (0x1000_0000u32 | size as u32).to_be_bytes().to_vec()
            } else {
                panic!("test EBML fixture too large")
            }
        }

        fn uint(value: u64) -> Vec<u8> {
            let bytes = value.to_be_bytes();
            let first = bytes.iter().position(|byte| *byte != 0).unwrap_or(7);
            bytes[first..].to_vec()
        }

        fn unsigned_element(id: &[u8], value: u64) -> Vec<u8> {
            element(id, &uint(value))
        }

        let mut ebml_header = Vec::new();
        ebml_header.extend(unsigned_element(&[0x42, 0x86], 1));
        ebml_header.extend(unsigned_element(&[0x42, 0xf7], 1));
        ebml_header.extend(unsigned_element(&[0x42, 0xf2], 4));
        ebml_header.extend(unsigned_element(&[0x42, 0xf3], 8));
        ebml_header.extend(element(&[0x42, 0x82], b"webm"));
        ebml_header.extend(unsigned_element(&[0x42, 0x87], 4));
        ebml_header.extend(unsigned_element(&[0x42, 0x85], 2));
        let ebml_header = element(&[0x1a, 0x45, 0xdf, 0xa3], &ebml_header);

        let info = element(
            &[0x15, 0x49, 0xa9, 0x66],
            &unsigned_element(&[0x2a, 0xd7, 0xb1], 1_000_000),
        );
        let mut track_entry = Vec::new();
        track_entry.extend(unsigned_element(&[0xd7], 1));
        track_entry.extend(unsigned_element(&[0x83], 1));
        track_entry.extend(element(&[0x86], b"V_VP8"));
        track_entry.extend(unsigned_element(&[0x23, 0xe3, 0x83], 33_000_000));
        let tracks = element(&[0x16, 0x54, 0xae, 0x6b], &element(&[0xae], &track_entry));

        let mut cluster = unsigned_element(&[0xe7], 10);
        for index in 0..frame_count {
            let relative_time = i16::try_from(index)
                .expect("test WebM timestamp fits in a signed relative block time")
                .to_be_bytes();
            let mut block = vec![0x81, relative_time[0], relative_time[1], 0x80];
            block.extend_from_slice(frame);
            cluster.extend(element(&[0xa3], &block));
        }
        let cluster = element(&[0x1f, 0x43, 0xb6, 0x75], &cluster);

        let mut segment = Vec::new();
        segment.extend(info);
        segment.extend(tracks);
        segment.extend(cluster);
        let mut webm = ebml_header;
        webm.extend(element(&[0x18, 0x53, 0x80, 0x67], &segment));
        webm
    }
}

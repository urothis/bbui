//! Pure-Rust image, SVG, VP8, and H.264 decoding for BBUI assets.
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
    BbuiMediaDecodedBudget, BbuiMediaFrameCache, BbuiMediaLimits, BbuiMediaPlayer,
    BbuiMediaPlayerState, BbuiMediaPlugin, BbuiMediaSequence, MediaAssetLoadError,
    MediaSequenceKind, RasterSvgImageLoader, rgba_frame_to_image,
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

mod h264_pixels;
use h264_pixels::*;

mod h264;
#[cfg(test)]
pub(crate) use h264::decode_mp4_h264_sequence_cancellable;
pub(crate) use h264::decode_mp4_h264_stream_cancellable;
use h264::*;
pub use h264::{decode_mp4_h264, decode_mp4_h264_thumbnail, decode_mp4_h264_thumbnail_cancellable};

mod gif;
pub(crate) use gif::decode_gif_stream_cancellable;
use gif::*;

mod svg;
pub(crate) use svg::decode_svg_with_max_bytes;
use svg::*;
pub use svg::{decode_svg, decode_svg_thumbnail};

mod ivf;
pub use ivf::decode_ivf_vp8;
#[cfg(test)]
pub(crate) use ivf::decode_ivf_vp8_cancellable_with_limits;
pub(crate) use ivf::decode_ivf_vp8_stream_cancellable;
use ivf::*;

mod webm;
pub use webm::decode_webm_vp8;
#[cfg(test)]
pub(crate) use webm::decode_webm_vp8_cancellable_with_limits;
pub(crate) use webm::decode_webm_vp8_stream_cancellable;
use webm::*;

#[cfg(test)]
#[path = "media/tests.rs"]
mod tests;

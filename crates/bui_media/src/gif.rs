use super::*;

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

pub(super) fn decode_gif_inner(
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

pub(super) fn raster_limits(max_alloc: u64) -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_PIXELS as u32);
    limits.max_image_height = Some(MAX_DECODED_PIXELS as u32);
    limits.max_alloc = Some(max_alloc);
    limits
}

pub(super) fn raster_decode_error(error: image::ImageError) -> MediaDecodeError {
    match error {
        image::ImageError::Limits(limit) => match limit.kind() {
            image::error::LimitErrorKind::InsufficientMemory => MediaDecodeError::SequenceLimit,
            image::error::LimitErrorKind::DimensionError => MediaDecodeError::PixelLimit,
            _ => MediaDecodeError::Raster(image::ImageError::Limits(limit)),
        },
        other => MediaDecodeError::Raster(other),
    }
}

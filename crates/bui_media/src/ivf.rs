use super::*;

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

pub(super) fn decode_ivf_vp8_inner(
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

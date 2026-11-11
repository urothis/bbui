use super::*;

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

pub(super) fn decode_webm_vp8_inner(
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
                if let Some(track) = track
                    && !in_block_group
                    && track.number == packet.track
                {
                    decode_video_block(
                        &mut decoder,
                        &mut decoded,
                        &mut decoded_bytes,
                        packet,
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
pub(super) struct StreamBlockContext<'a> {
    cluster_timestamp: u64,
    timestamp_scale_ns: u64,
    track: VideoTrack,
    cancelled: &'a AtomicBool,
    max_frames: usize,
    max_frame_bytes: usize,
}

pub(super) fn decode_video_block_stream(
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

pub(super) fn is_cancelled(cancelled: Option<&AtomicBool>) -> bool {
    cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
}

#[derive(Clone, Copy)]
pub(super) struct VideoTrack {
    number: u64,
    default_duration_ns: Option<u64>,
    timestamp_scale: f64,
}

pub(super) struct PendingVideoBlock {
    track: u64,
    relative_timestamp: i16,
    frames: Vec<Vec<u8>>,
    duration_ticks: Option<u64>,
}

#[derive(Clone, Copy)]
pub(super) struct DecodeBlockContext<'a> {
    cluster_timestamp: u64,
    timestamp_scale_ns: u64,
    track: VideoTrack,
    cancelled: Option<&'a AtomicBool>,
    first_frame_only: bool,
    max_frames: usize,
    max_sequence_bytes: usize,
}

pub(super) fn child_unsigned(
    children: &[MatroskaSpec],
    matches: impl Fn(&MatroskaSpec) -> bool,
) -> Option<u64> {
    children
        .iter()
        .find(|tag| matches(tag))
        .and_then(EbmlTag::as_unsigned_int)
        .copied()
}

pub(super) fn decode_video_block(
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

pub(super) fn check_thumbnail_video_dimensions(
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

pub(super) fn push_video_picture(
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

pub(super) fn webm_time_to_ns(
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

pub(super) fn picture_to_rgba(picture: Picture<'_>) -> Result<RgbaFrame, MediaDecodeError> {
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

pub(super) fn clamp_byte(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

pub(super) fn unpremultiply_rgba(pixels: &mut [u8]) {
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

pub(super) fn timebase_to_ns(
    timestamp: u64,
    scale: u64,
    rate: u64,
) -> Result<u64, MediaDecodeError> {
    let nanos = u128::from(timestamp)
        .checked_mul(u128::from(scale))
        .and_then(|value| value.checked_mul(1_000_000_000))
        .ok_or(MediaDecodeError::DimensionsOverflow)?
        / u128::from(rate);
    u64::try_from(nanos).map_err(|_| MediaDecodeError::DimensionsOverflow)
}

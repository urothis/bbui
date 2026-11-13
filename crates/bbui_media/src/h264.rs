use super::*;

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

mod thumbnail;
use thumbnail::decode_mp4_h264_thumbnail_inner;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct H264Timing {
    timestamp_ns: Option<u64>,
    duration_ns: Option<u64>,
    discard: bool,
}

pub(super) struct H264DecodedFrame {
    timing: H264Timing,
    frame: RgbaFrame,
}

pub(super) fn decode_mp4_h264_sequence_inner(
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
pub(super) struct H264DecodeOptions {
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

pub(super) struct H264StreamTimeline<'a> {
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

pub(super) fn decode_mp4_h264_sequence_with_sink(
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
    let color_info = color_info.ok_or_else(|| {
        MediaDecodeError::Video("H.264 stream has no usable SPS color information".to_owned())
    })?;

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

pub(super) fn media_ticks_to_ns(ticks: i64, time_base: oxideav_core::TimeBase) -> Option<u64> {
    if ticks < 0 || time_base.num() <= 0 || time_base.den() <= 0 {
        return None;
    }
    let nanoseconds = i128::from(ticks)
        .checked_mul(i128::from(time_base.num()))?
        .checked_mul(1_000_000_000)?
        / i128::from(time_base.den());
    u64::try_from(nanoseconds).ok()
}

pub(super) fn normalize_h264_timeline(frames: Vec<H264DecodedFrame>) -> Vec<VideoFrame> {
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

pub(super) fn rgba_frame_byte_len(width: u32, height: u32) -> Result<usize, MediaDecodeError> {
    RgbaFrame::check_dimensions(width, height)?;
    usize::try_from(u64::from(width) * u64::from(height) * 4)
        .map_err(|_| MediaDecodeError::DimensionsOverflow)
}

pub(super) fn checked_avcc_nal_count(
    sample: &[u8],
    length_size: usize,
) -> Result<usize, MediaDecodeError> {
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
            _ => {
                return Err(MediaDecodeError::Video(
                    "unsupported AVCC length size".to_owned(),
                ));
            }
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

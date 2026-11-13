use super::*;

pub(super) fn decode_mp4_h264_thumbnail_inner(
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
    let color_info = color_info.ok_or_else(|| {
        MediaDecodeError::Video("H.264 stream has no usable SPS color information".to_owned())
    })?;

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

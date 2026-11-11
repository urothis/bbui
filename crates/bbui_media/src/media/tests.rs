use super::*;

const H264_MP4_FIXTURE: &[u8] = include_bytes!("../../assets/sample-h264.mp4");
const H264_10BIT_MP4_FIXTURE: &[u8] = include_bytes!("../../assets/sample-h264-10bit.mp4");
const H264_422_MP4_FIXTURE: &[u8] = include_bytes!("../../assets/sample-h264-422.mp4");
const H264_444_MP4_FIXTURE: &[u8] = include_bytes!("../../assets/sample-h264-444.mp4");
const H264_LONG_MP4_FIXTURE: &[u8] = include_bytes!("../../assets/sample-h264-long.mp4");

const VP8_KEYFRAME_16X16: &[u8] = &[
    48, 1, 0, 157, 1, 42, 16, 0, 16, 0, 1, 64, 38, 37, 160, 0, 3, 112, 0, 254, 240, 83, 31, 255,
    226, 223, 180, 127, 123, 101, 7, 255, 253, 251, 75, 255, 240, 39, 127, 255, 2, 119, 253, 12, 0,
    0,
];

const TWO_FRAME_GIF: &[u8] = &[
    71, 73, 70, 56, 57, 97, 2, 0, 1, 0, 129, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 255,
    11, 78, 69, 84, 83, 67, 65, 80, 69, 50, 46, 48, 3, 1, 0, 0, 0, 33, 249, 4, 8, 10, 0, 0, 0, 44,
    0, 0, 0, 0, 2, 0, 1, 0, 0, 8, 5, 0, 1, 0, 8, 8, 0, 33, 249, 4, 9, 25, 0, 2, 0, 44, 0, 0, 0, 0,
    2, 0, 1, 0, 129, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 5, 0, 1, 0, 8, 8, 0, 59,
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
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
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
    let byte_limit = decode_ivf_vp8_cancellable_with_limits(&ivf, &cancelled, 1, 0).unwrap_err();
    assert!(matches!(byte_limit, MediaDecodeError::SequenceLimit));

    let webm = include_bytes!("../../assets/sample.webm");
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
    let completed =
        decode_webm_vp8_stream_cancellable(&webm, &cancelled, FRAME_COUNT, 16 * 16 * 4, |frame| {
            assert_eq!((frame.frame.width, frame.frame.height), (16, 16));
            assert!(frame.duration_ns.unwrap() > 0);
            decoded += 1;
            true
        })
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

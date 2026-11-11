use super::*;

pub(super) fn h264_frame_thumbnail(
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

pub(super) fn h264_frame_to_rgba(
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

pub(super) fn h264_yuv_to_rgb(
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

pub(super) fn video_frame_thumbnail(
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

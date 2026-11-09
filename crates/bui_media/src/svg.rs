use super::*;

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

pub(super) fn validate_thumbnail_size(width: u32, height: u32) -> Result<(), MediaDecodeError> {
    if width == 0 || height == 0 {
        Err(MediaDecodeError::InvalidData(
            "thumbnail dimensions must be positive",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn resize_thumbnail(
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

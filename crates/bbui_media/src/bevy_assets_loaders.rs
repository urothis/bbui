use super::*;

/// Loader for SVG images, with `.bbuiimg` as an opt-in alias that accepts any format supported by
/// `image`'s raster decoder. Common raster extensions use Bevy's built-in `ImageLoader`.
#[derive(Debug, Clone, TypePath)]
pub struct RasterSvgImageLoader {
    limits: BbuiMediaLimits,
}

impl FromWorld for RasterSvgImageLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BbuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for RasterSvgImageLoader {
    type Asset = Image;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        let extension = load_context
            .path()
            .path()
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        let frame = if extension.eq_ignore_ascii_case("svg") {
            decode_svg_with_max_bytes(&bytes, self.limits.max_sequence_bytes)
        } else {
            decode_image_with_max_bytes(&bytes, self.limits.max_sequence_bytes)
        }
        .map_err(configured_decode_error)?;
        if frame.pixels.len() > self.limits.max_sequence_bytes {
            return Err(MediaAssetLoadError::DecodedLimit);
        }
        image_from_frame(frame)
    }

    fn extensions(&self) -> &[&str] {
        &["svg", "bbuiimg"]
    }
}

#[derive(Debug, Clone, TypePath)]
pub(super) struct GifSequenceLoader {
    limits: BbuiMediaLimits,
}

impl FromWorld for GifSequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BbuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for GifSequenceLoader {
    type Asset = BbuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BbuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::Gif,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["gif"]
    }
}

#[derive(Debug, Clone, TypePath)]
pub(super) struct IvfSequenceLoader {
    limits: BbuiMediaLimits,
}

impl FromWorld for IvfSequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BbuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for IvfSequenceLoader {
    type Asset = BbuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BbuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::IvfVp8,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["ivf"]
    }
}

#[derive(Debug, Clone, TypePath)]
pub(super) struct WebmSequenceLoader {
    limits: BbuiMediaLimits,
}

#[derive(Debug, Clone, TypePath)]
pub(super) struct Mp4H264SequenceLoader {
    limits: BbuiMediaLimits,
}

impl FromWorld for Mp4H264SequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BbuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for Mp4H264SequenceLoader {
    type Asset = BbuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BbuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::Mp4H264,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["mp4", "mov"]
    }
}

impl FromWorld for WebmSequenceLoader {
    fn from_world(world: &mut bevy::prelude::World) -> Self {
        Self {
            limits: world
                .get_resource::<BbuiMediaLimits>()
                .copied()
                .unwrap_or_default(),
        }
    }
}

impl AssetLoader for WebmSequenceLoader {
    type Asset = BbuiMediaSequence;
    type Settings = ();
    type Error = MediaAssetLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let bytes = read_bounded(reader, self.limits.max_encoded_bytes).await?;
        Ok(BbuiMediaSequence::from_encoded_bytes(
            bytes,
            MediaSequenceKind::WebmVp8,
        ))
    }

    fn extensions(&self) -> &[&str] {
        &["webm", "mkv"]
    }
}

pub(super) fn configured_decode_error(error: MediaDecodeError) -> MediaAssetLoadError {
    match error {
        MediaDecodeError::FrameLimit => MediaAssetLoadError::FrameLimit,
        MediaDecodeError::SequenceLimit => MediaAssetLoadError::DecodedLimit,
        other => MediaAssetLoadError::Decode(other),
    }
}

pub(super) fn image_from_frame(frame: RgbaFrame) -> Result<Image, MediaAssetLoadError> {
    let expected = usize::try_from(u64::from(frame.width) * u64::from(frame.height) * 4)
        .map_err(|_| MediaAssetLoadError::DecodedLimit)?;
    if frame.pixels.len() != expected {
        return Err(MediaAssetLoadError::Decode(MediaDecodeError::InvalidData(
            "decoded frame buffer length differs from dimensions",
        )));
    }
    Ok(Image::new(
        Extent3d {
            width: frame.width,
            height: frame.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        frame.pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Convert a validated decoded frame into a Bevy image that can be uploaded and sampled by UI.
pub fn rgba_frame_to_image(frame: RgbaFrame) -> Result<Image, MediaAssetLoadError> {
    image_from_frame(frame)
}

async fn read_bounded(
    reader: &mut dyn Reader,
    max_bytes: usize,
) -> Result<Vec<u8>, MediaAssetLoadError> {
    let read_limit = max_bytes
        .checked_add(1)
        .ok_or(MediaAssetLoadError::EncodedLimit)?;
    let mut bytes = Vec::with_capacity(read_limit.min(1024 * 1024));
    reader
        .take(read_limit as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(MediaAssetLoadError::Io)?;
    if bytes.len() > max_bytes {
        return Err(MediaAssetLoadError::EncodedLimit);
    }
    Ok(bytes)
}

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Optional per-control text metrics. Unset font and line height inherit the active theme.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRenderStyle {
    pub font: Option<TextFont>,
    pub line_height: Option<LineHeight>,
    pub letter_spacing: LetterSpacing,
    pub justify: Justify,
    pub line_break: LineBreak,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    RoundedRect {
        rect: ClipRect,
        radius: f32,
        color: Rgba,
    },
    /// A GPU-interpolated horizontal color strip; stop positions are in 0..=1.
    LinearGradient {
        rect: ClipRect,
        stops: Vec<(f32, Rgba)>,
    },
    /// A transparent rounded outline, including curved corners.
    RoundedBorder {
        rect: ClipRect,
        radius: f32,
        width: f32,
        color: Rgba,
    },
    /// A GPU-rendered filled ellipse with the supplied bounds and sRGB color.
    Ellipse {
        rect: ClipRect,
        color: Rgba,
    },
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: Rgba,
    },
    Text {
        origin: [f32; 2],
        text: String,
        color: Rgba,
    },
    /// Text with an explicit shaping width and wrapping rule, used by editable controls.
    TextWrapped {
        origin: [f32; 2],
        text: String,
        color: Rgba,
        max_width: f32,
        wrap: bool,
        style: Option<TextRenderStyle>,
    },
    Image {
        rect: ClipRect,
        asset_id: u64,
    },
    /// A rounded fill with a native Bevy UI box shadow.
    Shadow {
        rect: ClipRect,
        radius: f32,
        color: Rgba,
        shadow: Rgba,
        offset: [f32; 2],
        blur: f32,
        spread: f32,
    },
    /// A tintable registered image for an icon or small UI symbol.
    Icon {
        rect: ClipRect,
        asset_id: u64,
        tint: Rgba,
    },
    /// A cubic Bezier path, drawn by one shader-backed Bevy UI node.
    Bezier {
        points: [[f32; 2]; 4],
        width: f32,
        color: Rgba,
    },
    /// A cubic Bezier path shaded from its first endpoint color to its last.
    BezierGradient {
        points: [[f32; 2]; 4],
        width: f32,
        start_color: Rgba,
        end_color: Rgba,
    },
    /// GPU-shaded hue ring and saturation/value disk with hue and saturation selectors.
    HsvWheel {
        rect: ClipRect,
        hue_degrees: f32,
        saturation: f32,
        value: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawItem {
    pub clip: ClipRect,
    pub z: i32,
    pub primitive: Primitive,
}

/// Incremental widget paint output keyed by the widget's stable ECS entity.
///
/// `order` preserves the widget painter's stable traversal rank for equal-z items. Items within
/// one fragment retain their vector order. `target_camera` selects the independent screen or
/// camera render layer.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetRenderFragment {
    pub entity: Entity,
    pub order: u64,
    pub target_camera: Option<Entity>,
    pub items: Vec<DrawItem>,
}

/// Places fragments into a bounded cross-fragment Bezier batch.
///
/// The owner identifies the logical canvas and `chunk` keeps updates local. Fragments in one
/// group must contain only compatible Bezier items and there must be at most 128 of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetFragmentBatchGroup {
    pub owner: Entity,
    pub chunk: u32,
}

/// A paint description attached to a BBUI entity. `UiLayoutResult` supplies its final position,
/// clipping region, and z order in the renderer phase.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct UiPaint(pub Primitive);

impl UiPaint {
    pub fn draw_item(&self, layout: &UiLayoutResult) -> DrawItem {
        let rect = ClipRect::from(layout.rect);
        let clip = layout
            .clip
            .and_then(|clip| clip.intersection(layout.rect))
            .map(ClipRect::from)
            .unwrap_or(rect);
        let primitive = match &self.0 {
            Primitive::RoundedRect { radius, color, .. } => Primitive::RoundedRect {
                rect,
                radius: *radius,
                color: *color,
            },
            Primitive::LinearGradient { stops, .. } => Primitive::LinearGradient {
                rect,
                stops: stops.clone(),
            },
            Primitive::RoundedBorder {
                radius,
                width,
                color,
                ..
            } => Primitive::RoundedBorder {
                rect,
                radius: *radius,
                width: *width,
                color: *color,
            },
            Primitive::Ellipse { color, .. } => Primitive::Ellipse {
                rect,
                color: *color,
            },
            Primitive::Line {
                from,
                to,
                width,
                color,
            } => Primitive::Line {
                from: *from,
                to: *to,
                width: *width,
                color: *color,
            },
            Primitive::Text { text, color, .. } => Primitive::Text {
                origin: [layout.rect.x, layout.rect.y],
                text: text.clone(),
                color: *color,
            },
            Primitive::TextWrapped {
                text,
                color,
                max_width,
                wrap,
                style,
                ..
            } => Primitive::TextWrapped {
                origin: [layout.rect.x, layout.rect.y],
                text: text.clone(),
                color: *color,
                max_width: *max_width,
                wrap: *wrap,
                style: style.clone(),
            },
            Primitive::Image { asset_id, .. } => Primitive::Image {
                rect,
                asset_id: *asset_id,
            },
            Primitive::Shadow {
                radius,
                color,
                shadow,
                offset,
                blur,
                spread,
                ..
            } => Primitive::Shadow {
                rect,
                radius: *radius,
                color: *color,
                shadow: *shadow,
                offset: *offset,
                blur: *blur,
                spread: *spread,
            },
            Primitive::Icon { asset_id, tint, .. } => Primitive::Icon {
                rect,
                asset_id: *asset_id,
                tint: *tint,
            },
            Primitive::Bezier {
                points,
                width,
                color,
            } => Primitive::Bezier {
                points: *points,
                width: *width,
                color: *color,
            },
            Primitive::BezierGradient {
                points,
                width,
                start_color,
                end_color,
            } => Primitive::BezierGradient {
                points: *points,
                width: *width,
                start_color: *start_color,
                end_color: *end_color,
            },
            Primitive::HsvWheel {
                hue_degrees,
                saturation,
                value,
                ..
            } => Primitive::HsvWheel {
                rect,
                hue_degrees: *hue_degrees,
                saturation: *saturation,
                value: *value,
            },
        };
        DrawItem {
            clip,
            z: layout.z,
            primitive,
        }
    }
}

impl From<Rect> for ClipRect {
    fn from(rect: Rect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

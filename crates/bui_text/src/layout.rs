//! Shared glyph placement used by text measurement, selection, and hit testing.
//!
//! [`TextLayout::shape_with_bevy`] uses the same Bevy text pipeline and Parley
//! shaping data as Bevy's text renderer. [`TextLayout::shape`] remains useful
//! for deterministic layouts with a small custom measurer.

use crate::text_buffer::{CaretAffinity, TextCaretPosition};
use bevy_asset::Assets;
use bevy_math::Vec2;
use bevy_text::{
    ComputedTextBlock, Font, FontCx, Justify as BevyJustify, LayoutCx,
    LetterSpacing as BevyLetterSpacing, LineBreak as BevyLineBreak, LineHeight as BevyLineHeight,
    RemSize, TextBounds as BevyTextBounds, TextBrush, TextElement, TextFont as BevyTextFont,
    TextPipeline,
};
use parley::{Affinity, Cursor, Layout as ShapedLayout, Selection};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphPlacement {
    pub character: char,
    pub byte_range: Range<usize>,
    /// Horizontal position in the layout's coordinate space.
    pub x: f32,
    /// Vertical position in the layout's coordinate space.
    pub y: f32,
    /// Advance in the layout's coordinate space.
    pub advance: f32,
    pub line: usize,
}

pub trait TextMeasurer {
    fn advance(&self, character: char) -> f32;
    fn line_height(&self) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonospaceMeasurer {
    pub advance_width: f32,
    pub line_height_value: f32,
}

impl Default for MonospaceMeasurer {
    fn default() -> Self {
        Self {
            advance_width: 8.0,
            line_height_value: 16.0,
        }
    }
}

impl TextMeasurer for MonospaceMeasurer {
    fn advance(&self, character: char) -> f32 {
        if character == '\t' {
            self.advance_width * 4.0
        } else {
            self.advance_width
        }
    }

    fn line_height(&self) -> f32 {
        self.line_height_value
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextBounds {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub struct TextLayout {
    text_len: usize,
    glyphs: Vec<GlyphPlacement>,
    line_starts: Vec<usize>,
    line_widths: Vec<f32>,
    bounds: TextBounds,
    line_height: f32,
    bevy_layout: Option<ShapedLayout<TextBrush>>,
}

/// Text settings used when shaping through Bevy's text pipeline.
///
/// Pass the same font and layout properties used on the rendered Bevy `Text`
/// entity. The default values match Bevy's standard `Text` components.
#[derive(Debug, Clone, Copy)]
pub struct BevyTextLayoutConfig<'a> {
    pub font: &'a BevyTextFont,
    pub line_height: BevyLineHeight,
    pub letter_spacing: BevyLetterSpacing,
    pub line_break: BevyLineBreak,
    pub justify: BevyJustify,
    /// Window/UI scale used by Bevy when resolving text font metrics.
    pub scale_factor: f32,
    pub logical_viewport_size: Vec2,
    pub rem_size: RemSize,
}

impl<'a> BevyTextLayoutConfig<'a> {
    /// Create a configuration matching Bevy's default text line height,
    /// letter spacing, line wrapping, alignment, viewport, and rem size.
    pub fn default_for(font: &'a BevyTextFont) -> Self {
        Self {
            font,
            line_height: BevyLineHeight::default(),
            letter_spacing: BevyLetterSpacing::default(),
            line_break: BevyLineBreak::default(),
            justify: BevyJustify::default(),
            scale_factor: 1.0,
            logical_viewport_size: Vec2::ZERO,
            rem_size: RemSize::default(),
        }
    }
}

impl PartialEq for TextLayout {
    fn eq(&self, other: &Self) -> bool {
        self.text_len == other.text_len
            && self.glyphs == other.glyphs
            && self.line_starts == other.line_starts
            && self.line_widths == other.line_widths
            && self.bounds == other.bounds
            && self.line_height == other.line_height
    }
}

impl TextLayout {
    /// Lay out text in logical pixels, wrapping only between Unicode scalar
    /// values. Advanced shaping engines can provide the measured advance for
    /// each scalar and consume the resulting positions for drawing.
    pub fn shape(text: &str, max_width: f32, measurer: &impl TextMeasurer) -> Self {
        let line_height = positive(measurer.line_height());
        let max_width = if max_width.is_finite() {
            max_width.max(0.0)
        } else {
            f32::INFINITY
        };
        let mut glyphs = Vec::new();
        let mut line_starts = vec![0];
        let mut line_widths = Vec::new();
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut line = 0usize;
        for (byte, character) in text.char_indices() {
            let next_byte = byte + character.len_utf8();
            if character == '\n' {
                glyphs.push(GlyphPlacement {
                    character,
                    byte_range: byte..next_byte,
                    x,
                    y,
                    advance: 0.0,
                    line,
                });
                line_widths.push(x);
                x = 0.0;
                y += line_height;
                line += 1;
                line_starts.push(next_byte);
                continue;
            }
            let advance = positive(measurer.advance(character));
            if x > 0.0 && x + advance > max_width {
                line_widths.push(x);
                x = 0.0;
                y += line_height;
                line += 1;
                line_starts.push(byte);
            }
            glyphs.push(GlyphPlacement {
                character,
                byte_range: byte..next_byte,
                x,
                y,
                advance,
                line,
            });
            x += advance;
        }
        line_widths.push(x);
        let bounds = TextBounds {
            width: line_widths.iter().copied().fold(0.0, f32::max),
            height: line_height * line_starts.len() as f32,
        };
        Self {
            text_len: text.len(),
            glyphs,
            line_starts,
            line_widths,
            bounds,
            line_height,
            bevy_layout: None,
        }
    }

    /// Shape text through Bevy's text pipeline with its standard line height,
    /// letter spacing, wrapping, and alignment settings.
    ///
    /// `font` should be the same `TextFont` inserted on the rendered text
    /// entity. This convenience uses the defaults of Bevy's `TextLayout` and
    /// `TextLineHeight` components.
    pub fn shape_with_bevy(
        text: &str,
        max_width: f32,
        font: &BevyTextFont,
        pipeline: &mut TextPipeline,
        fonts: &Assets<Font>,
        font_cx: &mut FontCx,
        layout_cx: &mut LayoutCx,
    ) -> Result<Self, bevy_text::TextError> {
        Self::shape_with_bevy_config(
            text,
            max_width,
            BevyTextLayoutConfig::default_for(font),
            pipeline,
            fonts,
            font_cx,
            layout_cx,
        )
    }

    /// Shape text through Bevy's text pipeline with explicit rendering
    /// settings. Hit testing, caret geometry, and selection rectangles use the
    /// returned Parley layout, the same shaped data consumed by Bevy drawing.
    pub fn shape_with_bevy_config(
        text: &str,
        max_width: f32,
        config: BevyTextLayoutConfig<'_>,
        pipeline: &mut TextPipeline,
        fonts: &Assets<Font>,
        font_cx: &mut FontCx,
        layout_cx: &mut LayoutCx,
    ) -> Result<Self, bevy_text::TextError> {
        let element = TextElement::Text {
            text,
            font: config.font,
            color: bevy_text::TextColor::WHITE.0,
            line_height: config.line_height,
            letter_spacing: config.letter_spacing,
        };
        let mut computed = ComputedTextBlock::default();
        let scale_factor = positive(config.scale_factor);
        let bounds = if max_width.is_finite() {
            BevyTextBounds::new_horizontal((max_width.max(0.0) * scale_factor).max(0.0))
        } else {
            BevyTextBounds::UNBOUNDED
        };
        pipeline.update_buffer(
            fonts,
            std::iter::once((bevy_ecs::entity::Entity::PLACEHOLDER, 0, element)),
            config.line_break,
            config.justify,
            bounds,
            scale_factor,
            &mut computed,
            font_cx,
            layout_cx,
            config.logical_viewport_size,
            config.rem_size,
        )?;

        Ok(Self::from_bevy_layout(text, computed.buffer().clone()))
    }

    fn from_bevy_layout(text: &str, bevy_layout: ShapedLayout<TextBrush>) -> Self {
        let mut glyphs = Vec::new();
        let mut line_starts = Vec::new();
        let mut line_widths = Vec::new();
        let mut line_height = 0.0;

        for (line_index, line) in bevy_layout.lines().enumerate() {
            let metrics = line.metrics();
            line_starts.push(line.text_range().start);
            line_widths.push(metrics.advance);
            if line_index == 0 {
                line_height = metrics.line_height;
            }

            let mut x = metrics.inline_min_coord + metrics.offset;
            let y = metrics.block_min_coord;
            for run in line.runs() {
                for cluster in run.visual_clusters() {
                    let byte_range = cluster.text_range();
                    let advance = cluster.advance().max(0.0);
                    let character = text
                        .get(byte_range.clone())
                        .and_then(|cluster_text| cluster_text.chars().next())
                        .unwrap_or('\0');
                    glyphs.push(GlyphPlacement {
                        character,
                        byte_range,
                        x,
                        y,
                        advance,
                        line: line_index,
                    });
                    x += advance;
                }
            }
        }

        let bounds = TextBounds {
            width: bevy_layout.full_width(),
            height: bevy_layout.height(),
        };
        if line_starts.is_empty() {
            line_starts.push(0);
            line_widths.push(0.0);
        }

        Self {
            text_len: text.len(),
            glyphs,
            line_starts,
            line_widths,
            bounds,
            line_height,
            bevy_layout: Some(bevy_layout),
        }
    }

    pub fn glyphs(&self) -> &[GlyphPlacement] {
        &self.glyphs
    }

    /// Bounds in the layout's coordinate space.
    ///
    /// [`TextLayout::shape`] uses logical pixels. Bevy-shaped layouts use the
    /// scaled coordinates produced by Bevy's text pipeline (logical pixels
    /// multiplied by [`BevyTextLayoutConfig::scale_factor`]).
    pub fn bounds(&self) -> TextBounds {
        self.bounds
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Map a point to the closest insertion byte offset.
    ///
    /// Coordinates must use the same space as [`Self::bounds`] and
    /// [`GlyphPlacement`].
    pub fn hit_test(&self, x: f32, y: f32) -> usize {
        self.hit_test_position(x, y).byte_index
    }

    /// Map a point to an insertion position, including which visual side of a
    /// soft-wrap boundary the caret occupies.
    pub fn hit_test_position(&self, x: f32, y: f32) -> TextCaretPosition {
        if let Some(layout) = &self.bevy_layout {
            let cursor = Cursor::from_point(layout, x, y);
            return TextCaretPosition {
                byte_index: cursor.index().min(self.text_len),
                affinity: from_parley_affinity(cursor.affinity()),
            };
        }
        let line = if self.line_height > 0.0 {
            (y.max(0.0) / self.line_height).floor() as usize
        } else {
            0
        }
        .min(self.line_starts.len().saturating_sub(1));
        let start = self.line_starts.get(line).copied().unwrap_or(0);
        let mut end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text_len);
        if end > start
            && self
                .glyphs
                .iter()
                .any(|glyph| glyph.byte_range.end == end && glyph.character == '\n')
        {
            end = self
                .glyphs
                .iter()
                .find(|glyph| glyph.byte_range.end == end)
                .map_or(end, |glyph| glyph.byte_range.start);
        }
        let glyphs = self
            .glyphs
            .iter()
            .filter(|glyph| glyph.line == line && glyph.character != '\n');
        for glyph in glyphs {
            if x < glyph.x + glyph.advance * 0.5 {
                return TextCaretPosition {
                    byte_index: glyph.byte_range.start,
                    affinity: CaretAffinity::Downstream,
                };
            }
            if x < glyph.x + glyph.advance {
                return TextCaretPosition {
                    byte_index: glyph.byte_range.end,
                    affinity: CaretAffinity::Upstream,
                };
            }
        }
        TextCaretPosition {
            byte_index: end,
            affinity: CaretAffinity::Upstream,
        }
    }

    /// Caret geometry in the layout's coordinate space.
    pub fn caret_rect(&self, byte_offset: usize) -> TextRect {
        self.caret_rect_with_affinity(byte_offset, CaretAffinity::Downstream)
    }

    /// Caret geometry at a byte offset and visual affinity.
    pub fn caret_rect_with_affinity(
        &self,
        byte_offset: usize,
        affinity: CaretAffinity,
    ) -> TextRect {
        let offset = byte_offset.min(self.text_len);
        if let Some(layout) = &self.bevy_layout {
            let cursor = Cursor::from_byte_index(layout, offset, to_parley_affinity(affinity));
            return from_parley_rect(cursor.geometry(layout, 1.0));
        }
        if affinity == CaretAffinity::Upstream
            && let Some(glyph) = self
                .glyphs
                .iter()
                .find(|glyph| glyph.byte_range.end == offset && glyph.character != '\n')
        {
            return TextRect {
                x: glyph.x + glyph.advance,
                y: glyph.y,
                width: 1.0,
                height: self.line_height,
            };
        }
        if let Some(glyph) = self
            .glyphs
            .iter()
            .find(|glyph| glyph.byte_range.start == offset)
        {
            return TextRect {
                x: glyph.x,
                y: glyph.y,
                width: 1.0,
                height: self.line_height,
            };
        }
        if let Some(glyph) = self
            .glyphs
            .iter()
            .find(|glyph| glyph.byte_range.end == offset)
        {
            if glyph.character == '\n' {
                return TextRect {
                    x: glyph.x,
                    y: glyph.y,
                    width: 1.0,
                    height: self.line_height,
                };
            }
            return TextRect {
                x: glyph.x + glyph.advance,
                y: glyph.y,
                width: 1.0,
                height: self.line_height,
            };
        }
        let line = self
            .line_starts
            .iter()
            .rposition(|start| *start <= offset)
            .unwrap_or(0);
        TextRect {
            x: self.line_widths.get(line).copied().unwrap_or(0.0),
            y: line as f32 * self.line_height,
            width: 1.0,
            height: self.line_height,
        }
    }

    /// Move a shaped cursor in visual order. `None` indicates a deterministic
    /// custom-measurer layout, where callers may use logical grapheme steps.
    pub fn move_visual_caret(
        &self,
        position: TextCaretPosition,
        right: bool,
    ) -> Option<TextCaretPosition> {
        let layout = self.bevy_layout.as_ref()?;
        let cursor = Cursor::from_byte_index(
            layout,
            position.byte_index.min(self.text_len),
            to_parley_affinity(position.affinity),
        );
        let moved = if right {
            cursor.next_visual(layout)
        } else {
            cursor.previous_visual(layout)
        };
        Some(TextCaretPosition {
            byte_index: moved.index().min(self.text_len),
            affinity: from_parley_affinity(moved.affinity()),
        })
    }

    /// Return one highlight rectangle per contiguous selection run on a line,
    /// in the layout's coordinate space.
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<TextRect> {
        let start = range.start.min(range.end).min(self.text_len);
        let end = range.start.max(range.end).min(self.text_len);
        if start == end {
            return Vec::new();
        }
        if let Some(layout) = &self.bevy_layout {
            let selection = Selection::new(
                Cursor::from_byte_index(layout, start, Affinity::Downstream),
                Cursor::from_byte_index(layout, end, Affinity::Downstream),
            );
            return selection
                .geometry(layout)
                .into_iter()
                .map(|(rect, _line)| from_parley_rect(rect))
                .collect();
        }
        let mut rects = Vec::<TextRect>::new();
        for glyph in &self.glyphs {
            if glyph.character == '\n'
                || glyph.byte_range.end <= start
                || glyph.byte_range.start >= end
            {
                continue;
            }
            if let Some(last) = rects.last_mut()
                && (last.y - glyph.y).abs() < f32::EPSILON
                && (last.x + last.width - glyph.x).abs() < f32::EPSILON
            {
                last.width += glyph.advance;
            } else {
                rects.push(TextRect {
                    x: glyph.x,
                    y: glyph.y,
                    width: glyph.advance,
                    height: self.line_height,
                });
            }
        }
        rects
    }
}

fn from_parley_rect(rect: parley::BoundingBox) -> TextRect {
    TextRect {
        x: rect.x0 as f32,
        y: rect.y0 as f32,
        width: rect.width() as f32,
        height: rect.height() as f32,
    }
}

fn to_parley_affinity(affinity: CaretAffinity) -> Affinity {
    match affinity {
        CaretAffinity::Downstream => Affinity::Downstream,
        CaretAffinity::Upstream => Affinity::Upstream,
    }
}

fn from_parley_affinity(affinity: Affinity) -> CaretAffinity {
    match affinity {
        Affinity::Downstream => CaretAffinity::Downstream,
        Affinity::Upstream => CaretAffinity::Upstream,
    }
}

fn positive(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_wrapping_hit_testing_and_caret_share_positions() {
        let layout = TextLayout::shape("abcd", 16.0, &MonospaceMeasurer::default());
        assert_eq!(layout.line_count(), 2);
        assert_eq!(
            layout.bounds(),
            TextBounds {
                width: 16.0,
                height: 32.0
            }
        );
        assert_eq!(layout.hit_test(12.0, 0.0), 2);
        assert_eq!(layout.hit_test(0.0, 16.0), 2);
        assert_eq!(layout.caret_rect(3).y, 16.0);
        assert_eq!(
            layout.selection_rects(1..3),
            vec![
                TextRect {
                    x: 8.0,
                    y: 0.0,
                    width: 8.0,
                    height: 16.0
                },
                TextRect {
                    x: 0.0,
                    y: 16.0,
                    width: 8.0,
                    height: 16.0
                },
            ]
        );
    }

    #[test]
    fn explicit_newlines_and_utf8_offsets_are_supported() {
        let layout = TextLayout::shape("a🙂\nb", 64.0, &MonospaceMeasurer::default());
        assert_eq!(layout.line_count(), 2);
        assert_eq!(layout.hit_test(100.0, 0.0), "a🙂".len());
        assert_eq!(layout.hit_test(0.0, 16.0), "a🙂\n".len());
        assert_eq!(layout.glyphs()[1].byte_range, 1..5);
    }

    #[test]
    fn fallback_font_runs_share_cluster_geometry_with_caret_and_selection() {
        use bevy_app::{App, Update};
        use bevy_ecs::system::{Res, ResMut, SystemState};
        use bevy_text::FontSource;

        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .init_resource::<LayoutCx>()
            .init_resource::<TextPipeline>()
            .add_systems(Update, bevy_text::load_font_assets_into_font_collection);
        let primary = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(bevy_text::DEFAULT_FONT_DATA.to_vec()));
        let fallback = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(
                include_bytes!("../tests/assets/NotoSansSymbols2-U+1F7D5.ttf").to_vec(),
            ));
        app.update();

        let fallback_char = '\u{1F7D5}';
        let text = format!("A{fallback_char}B");
        let fallback_start = "A".len();
        let fallback_end = fallback_start + fallback_char.len_utf8();
        let mut font = BevyTextFont::from_font_size(20.0);
        font.font = FontSource::list([
            FontSource::Handle(primary),
            FontSource::Handle(fallback.clone()),
        ]);

        let mut state = SystemState::<(
            ResMut<TextPipeline>,
            Res<Assets<Font>>,
            ResMut<FontCx>,
            ResMut<LayoutCx>,
        )>::new(app.world_mut());
        let (mut pipeline, fonts, mut font_cx, mut layout_cx) = state
            .get_mut(app.world_mut())
            .expect("the text pipeline resources are initialized");
        let fallback_data = fonts
            .get(fallback.id())
            .expect("the fallback test font asset remains loaded")
            .data
            .clone();
        let layout = TextLayout::shape_with_bevy(
            &text,
            120.0,
            &font,
            &mut pipeline,
            &fonts,
            &mut font_cx,
            &mut layout_cx,
        )
        .expect("the requested and fallback fonts should shape successfully");

        let fallback_run = layout
            .bevy_layout
            .as_ref()
            .expect("Bevy-shaped layout is retained for editing geometry")
            .lines()
            .flat_map(|line| line.runs())
            .find(|run| {
                let range = run.text_range();
                range.start <= fallback_start && range.end >= fallback_end
            })
            .expect("the fallback glyph belongs to a shaped text run");
        assert_eq!(fallback_run.font().data, fallback_data);

        let fallback_cluster = layout
            .glyphs()
            .iter()
            .find(|glyph| {
                glyph.byte_range.start <= fallback_start && glyph.byte_range.end >= fallback_end
            })
            .expect("the fallback glyph has a cluster placement");
        let selection = layout.selection_rects(fallback_start..fallback_end);
        assert!(!selection.is_empty());
        assert!(selection.iter().any(|rect| rect.width > 0.0));
        let cluster_hit = layout.hit_test(
            fallback_cluster.x + fallback_cluster.advance * 0.5,
            fallback_cluster.y + layout.caret_rect(fallback_start).height * 0.5,
        );
        assert!(
            cluster_hit == fallback_start || cluster_hit == fallback_end,
            "fallback cluster hit should land at one of its byte boundaries: {cluster_hit}"
        );
        assert!(layout.caret_rect(fallback_start).height > 0.0);
        assert!(layout.caret_rect(fallback_end).height > 0.0);
        state.apply(app.world_mut());
    }
}

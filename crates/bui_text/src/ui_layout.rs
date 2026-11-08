//! Bevy component and systems that connect text metrics to BUI layout and widgets.

use bevy_asset::Assets;
use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use bevy_text::{
    Font, FontCx, Justify, LayoutCx, LetterSpacing, LineBreak, LineHeight, RemSize, TextFont,
    TextPipeline,
};
use bui_core::{UiFocus, UiParent};
use bui_layout::{
    UiIntrinsicSize, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength, UiViewports,
};
use bui_style::{Theme, UiComputedStyle, UiStyleProperty};
use std::ops::Range;

use crate::{
    BevyTextLayoutConfig, TextBuffer, TextCaretPosition, TextLayout, TextRange, TextRect,
    resolve_text_font,
};

/// Text drawing and editing state shared by layout, widgets, and paint.
///
/// Add this to an entity with a [`TextBuffer`]. BUI shapes the visible text
/// using Bevy's text pipeline, stores the shared [`TextLayout`], and publishes
/// absolute logical-pixel caret/selection geometry. `prefix` supports widgets
/// that paint a label before their editable value; `placeholder` is drawn only
/// while the buffer is empty and is excluded from caret/selection positions.
#[derive(Component, Debug, Clone)]
pub struct UiTextLayout {
    /// Optional explicit Bevy font. If absent, the current theme's font size
    /// and Bevy's default font handle are used.
    pub font: Option<TextFont>,
    /// Optional explicit Bevy line height. Defaults to the theme's relative
    /// line height, matching BUI renderer text.
    pub line_height: Option<LineHeight>,
    pub letter_spacing: LetterSpacing,
    /// Optional explicit wrapping rule. Defaults to no wrapping for a single
    /// line field and word wrapping for a multiline area.
    pub line_break: Option<LineBreak>,
    pub justify: Justify,
    /// Optional explicit DPI scale; by default the owning layout root's scale
    /// is used, matching Bevy's UI text pipeline.
    pub scale_factor: Option<f32>,
    /// Insets from the widget's content box to the rendered text origin.
    pub inset: [f32; 2],
    pub multiline: bool,
    pub wrap: bool,
    pub prefix: String,
    pub placeholder: String,
    /// Logical pixels scrolled away from the text origin. Positive values
    /// move the visible content up and to the left.
    pub scroll_offset: [f32; 2],
    text_layout: Option<TextLayout>,
    display_text: String,
    text_origin: Option<[f32; 2]>,
    prefix_bytes: usize,
    value_len: usize,
    preferred_visual_x: Option<f32>,
    preferred_visual_position: Option<TextCaretPosition>,
    replacement_range: Option<TextRange>,
    preedit_range: Option<Range<usize>>,
    preedit_caret: Option<usize>,
    buffer_selection: Option<TextRange>,
    scroll_viewport: Option<[f32; 2]>,
    last_focused: Option<bool>,
    shape_key: Option<ShapeKey>,
    intrinsic_key: Option<ShapeKey>,
    intrinsic_bounds: Option<crate::TextBounds>,
    last_error: Option<String>,
}

impl Default for UiTextLayout {
    fn default() -> Self {
        Self {
            font: None,
            line_height: None,
            letter_spacing: LetterSpacing::default(),
            line_break: None,
            justify: Justify::default(),
            scale_factor: None,
            inset: [6.0, 3.0],
            multiline: false,
            wrap: false,
            prefix: String::new(),
            placeholder: String::new(),
            scroll_offset: [0.0, 0.0],
            text_layout: None,
            display_text: String::new(),
            text_origin: None,
            prefix_bytes: 0,
            value_len: 0,
            preferred_visual_x: None,
            preferred_visual_position: None,
            replacement_range: None,
            preedit_range: None,
            preedit_caret: None,
            buffer_selection: None,
            scroll_viewport: None,
            last_focused: None,
            shape_key: None,
            intrinsic_key: None,
            intrinsic_bounds: None,
            last_error: None,
        }
    }
}

impl UiTextLayout {
    pub fn text_field(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            prefix: if label.is_empty() {
                String::new()
            } else {
                format!("{label}: ")
            },
            ..Self::default()
        }
    }

    pub fn text_area(label: impl Into<String>, wrap: bool) -> Self {
        let mut layout = Self::text_field(label);
        layout.multiline = true;
        layout.wrap = wrap;
        layout
    }

    pub fn layout(&self) -> Option<&TextLayout> {
        self.text_layout.as_ref()
    }

    pub fn display_text(&self) -> Option<&str> {
        self.text_layout
            .as_ref()
            .map(|_| self.display_text.as_str())
    }

    pub fn text_origin(&self) -> Option<[f32; 2]> {
        self.text_origin
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Byte range of the visible IME preedit in [`Self::display_text`].
    /// The range includes any widget prefix in its offsets.
    pub fn preedit_range(&self) -> Option<Range<usize>> {
        self.preedit_range.clone()
    }

    /// Return the editable buffer byte offset under a logical window point.
    pub fn hit_test(&self, position: [f32; 2]) -> Option<usize> {
        self.hit_test_caret(position)
            .map(|position| position.byte_index)
    }

    /// Return the editable insertion point under a logical window point,
    /// including which side of a soft-wrap boundary the caret occupies.
    pub fn hit_test_caret(&self, position: [f32; 2]) -> Option<TextCaretPosition> {
        let layout = self.text_layout.as_ref()?;
        let origin = self.text_origin?;
        let scale = self.layout_scale_factor();
        let position = layout.hit_test_position(
            (position[0] - origin[0]) * scale,
            (position[1] - origin[1]) * scale,
        );
        Some(self.display_position_to_buffer(position))
    }

    /// Place or extend a buffer selection at a logical window point.
    pub fn place_caret(&self, buffer: &mut TextBuffer, position: [f32; 2], extend: bool) -> bool {
        let Some(position) = self.hit_test_caret(position) else {
            return false;
        };
        set_buffer_caret(buffer, position, extend)
    }

    /// Move the caret to the adjacent shaped visual line while retaining its
    /// preferred horizontal position across repeated Up/Down keys. Returns
    /// `None` when the text has not been shaped yet, allowing callers to use
    /// logical newline navigation until geometry is available.
    pub fn move_caret_visual_line(
        &mut self,
        buffer: &mut TextBuffer,
        down: bool,
        extend: bool,
    ) -> Option<bool> {
        if buffer.composition().is_some() {
            return Some(false);
        }
        let layout = self.text_layout.as_ref()?;
        let caret = layout.caret_rect_with_affinity(buffer.caret(), buffer.caret_affinity());
        let current_position = TextCaretPosition {
            byte_index: buffer.caret(),
            affinity: buffer.caret_affinity(),
        };
        let preferred_x = if self.preferred_visual_position == Some(current_position) {
            self.preferred_visual_x.unwrap_or(caret.x)
        } else {
            self.preferred_visual_x = Some(caret.x);
            caret.x
        };
        let line_height = caret.height.max(1.0);
        let target_y = if down {
            caret.y + line_height * 1.5
        } else {
            caret.y - line_height * 0.5
        };
        let display_position = layout.hit_test_position(preferred_x, target_y);
        let target_caret =
            layout.caret_rect_with_affinity(display_position.byte_index, display_position.affinity);
        if (down && target_caret.y <= caret.y + 0.01) || (!down && target_caret.y >= caret.y - 0.01)
        {
            self.preferred_visual_position = Some(current_position);
            return Some(false);
        }
        let target = self.display_position_to_buffer(display_position);
        if set_buffer_caret(buffer, target, extend) {
            self.preferred_visual_position = Some(TextCaretPosition {
                byte_index: buffer.caret(),
                affinity: buffer.caret_affinity(),
            });
            Some(true)
        } else {
            self.preferred_visual_position = Some(TextCaretPosition {
                byte_index: buffer.caret(),
                affinity: buffer.caret_affinity(),
            });
            Some(false)
        }
    }

    /// Move to the start or end of the shaped visual line containing the
    /// caret. This respects soft wrapping as well as explicit newlines.
    pub fn move_caret_visual_line_edge(
        &mut self,
        buffer: &mut TextBuffer,
        end: bool,
        extend: bool,
    ) -> Option<bool> {
        if buffer.composition().is_some() {
            return Some(false);
        }
        let layout = self.text_layout.as_ref()?;
        let caret = layout.caret_rect_with_affinity(buffer.caret(), buffer.caret_affinity());
        let x = if end {
            layout.bounds().width + 1.0
        } else {
            -1.0
        };
        let display_position = layout.hit_test_position(x, caret.y + caret.height * 0.5);
        let target = self.display_position_to_buffer(display_position);
        self.reset_visual_navigation();
        Some(set_buffer_caret(buffer, target, extend))
    }

    /// Move left or right through the shaped visual caret order, including the
    /// two visual positions at a soft-wrap boundary.
    pub fn move_caret_visual_horizontal(
        &mut self,
        buffer: &mut TextBuffer,
        right: bool,
        extend: bool,
    ) -> Option<bool> {
        if buffer.composition().is_some() {
            return Some(false);
        }
        let layout = self.text_layout.as_ref()?;
        let selection = buffer.selection();
        if !extend && !selection.is_empty() {
            let target = if right {
                selection.end
            } else {
                selection.start
            };
            let affinity = if buffer.caret() == target {
                buffer.caret_affinity()
            } else {
                buffer.anchor_affinity()
            };
            self.reset_visual_navigation();
            return Some(set_buffer_caret(
                buffer,
                TextCaretPosition {
                    byte_index: target,
                    affinity,
                },
                false,
            ));
        }
        let position = TextCaretPosition {
            byte_index: buffer.caret(),
            affinity: buffer.caret_affinity(),
        };
        let target = layout.move_visual_caret(position, right)?;
        self.reset_visual_navigation();
        Some(set_buffer_caret(
            buffer,
            self.display_position_to_buffer(target),
            extend,
        ))
    }

    /// Clear the retained x coordinate used by visual Up/Down navigation.
    pub fn reset_visual_navigation(&mut self) -> bool {
        let x_changed = self.preferred_visual_x.take().is_some();
        let caret_changed = self.preferred_visual_position.take().is_some();
        x_changed || caret_changed
    }

    /// Caret geometry in absolute logical window pixels.
    pub fn caret_rect(&self, buffer: &TextBuffer) -> Option<TextRect> {
        let layout = self.text_layout.as_ref()?;
        let origin = self.text_origin?;
        let mut rect = layout.caret_rect_with_affinity(
            self.display_caret(buffer),
            self.display_caret_affinity(buffer),
        );
        scale_rect(&mut rect, 1.0 / self.layout_scale_factor());
        translate(&mut rect, origin);
        Some(rect)
    }

    /// Selection geometry in absolute logical window pixels.
    pub fn selection_rects(&self, buffer: &TextBuffer) -> Vec<TextRect> {
        let (Some(layout), Some(origin)) = (&self.text_layout, self.text_origin) else {
            return Vec::new();
        };
        if self.preedit_range.is_some() {
            return Vec::new();
        }
        let range = buffer.selection();
        if range.is_empty() {
            return Vec::new();
        }
        let start = self.prefix_bytes + range.start.min(self.value_len);
        let end = self.prefix_bytes + range.end.min(self.value_len);
        let mut rects = layout.selection_rects(start..end);
        for rect in &mut rects {
            scale_rect(rect, 1.0 / self.layout_scale_factor());
            translate(rect, origin);
        }
        rects
    }

    /// Logical window rectangles occupied by the active IME preedit text.
    /// Paint these as underlines to distinguish uncommitted text from the
    /// surrounding value.
    pub fn preedit_rects(&self) -> Vec<TextRect> {
        let (Some(layout), Some(origin), Some(range)) =
            (&self.text_layout, self.text_origin, &self.preedit_range)
        else {
            return Vec::new();
        };
        let mut rects = layout.selection_rects(range.clone());
        for rect in &mut rects {
            scale_rect(rect, 1.0 / self.layout_scale_factor());
            translate(rect, origin);
        }
        rects
    }

    fn display_position_to_buffer(&self, mut position: TextCaretPosition) -> TextCaretPosition {
        let display_byte = position.byte_index.saturating_sub(self.prefix_bytes);
        let Some(replacement) = self.replacement_range else {
            position.byte_index = display_byte.min(self.value_len);
            return position;
        };
        let Some(preedit) = &self.preedit_range else {
            position.byte_index = display_byte.min(self.value_len);
            return position;
        };
        let start = preedit.start.saturating_sub(self.prefix_bytes);
        let end = preedit.end.saturating_sub(self.prefix_bytes);
        let mapped = if display_byte < start {
            display_byte.min(replacement.start)
        } else if display_byte <= end {
            let preedit_offset = display_byte.saturating_sub(start);
            if preedit_offset.saturating_mul(2) < end.saturating_sub(start) {
                replacement.start
            } else {
                replacement.end
            }
        } else {
            display_byte
                .saturating_sub(end.saturating_sub(start))
                .saturating_add(replacement.len())
        };
        position.byte_index = mapped.min(self.value_len);
        position
    }

    fn layout_scale_factor(&self) -> f32 {
        self.shape_key
            .as_ref()
            .map_or(1.0, |key| positive(key.scale_factor, 1.0))
    }

    fn visible_text(&self, buffer: &TextBuffer) -> VisibleText {
        let mut text = String::with_capacity(
            self.prefix.len()
                + buffer.len_bytes()
                + buffer.composition().map_or(0, |ime| ime.text.len()),
        );
        text.push_str(&self.prefix);
        if let Some(composition) = buffer.composition() {
            let replacement = buffer.selection();
            text.push_str(&buffer.as_str()[..replacement.start]);
            let preedit_start = text.len();
            text.push_str(&composition.text);
            let preedit_end = text.len();
            text.push_str(&buffer.as_str()[replacement.end..]);
            let preedit_caret = self.prefix.len()
                + replacement.start
                + composition
                    .cursor_range
                    .map_or(composition.text.len(), |range| range.end);
            VisibleText {
                text,
                replacement_range: Some(replacement),
                preedit_range: Some(preedit_start..preedit_end),
                preedit_caret: Some(preedit_caret),
            }
        } else {
            let value = if buffer.is_empty() {
                &self.placeholder
            } else {
                buffer.as_str()
            };
            text.push_str(value);
            VisibleText {
                text,
                replacement_range: None,
                preedit_range: None,
                preedit_caret: None,
            }
        }
    }

    fn display_caret(&self, buffer: &TextBuffer) -> usize {
        self.preedit_caret
            .unwrap_or(self.prefix_bytes + buffer.caret().min(self.value_len))
    }

    fn display_caret_affinity(&self, buffer: &TextBuffer) -> crate::CaretAffinity {
        if self.preedit_caret.is_some() {
            crate::CaretAffinity::Downstream
        } else {
            buffer.caret_affinity()
        }
    }

    fn font(&self, theme: &Theme, computed: Option<&UiComputedStyle>) -> TextFont {
        self.font.clone().unwrap_or_else(|| {
            let size = computed
                .and_then(|style| style.scalar(UiStyleProperty::FontSize))
                .unwrap_or(theme.font_size);
            TextFont::from_font_size(positive(size, 14.0))
        })
    }

    fn line_height(&self, theme: &Theme, computed: Option<&UiComputedStyle>) -> LineHeight {
        self.line_height.unwrap_or_else(|| {
            let relative = computed
                .and_then(|style| style.scalar(UiStyleProperty::LineHeight))
                .unwrap_or(theme.line_height);
            LineHeight::RelativeToFont(positive(relative, 1.2))
        })
    }

    fn line_break(&self) -> LineBreak {
        self.line_break.unwrap_or(if self.multiline && self.wrap {
            LineBreak::WordBoundary
        } else {
            LineBreak::NoWrap
        })
    }

    fn cache_key(&self, text: &str, width: f32, context: ShapeContext) -> ShapeKey {
        ShapeKey {
            text: text.to_owned(),
            width,
            font: context.font,
            line_height: context.line_height,
            letter_spacing: self.letter_spacing,
            line_break: self.line_break(),
            justify: self.justify,
            scale_factor: positive(self.scale_factor.unwrap_or(context.scale_factor), 1.0),
            viewport_size: context.viewport_size,
            rem_size: RemSize::default(),
        }
    }

    fn config<'a>(key: &'a ShapeKey) -> BevyTextLayoutConfig<'a> {
        BevyTextLayoutConfig {
            font: &key.font,
            line_height: key.line_height,
            letter_spacing: key.letter_spacing,
            line_break: key.line_break,
            justify: key.justify,
            scale_factor: key.scale_factor,
            logical_viewport_size: key.viewport_size,
            rem_size: key.rem_size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ShapeKey {
    text: String,
    width: f32,
    font: TextFont,
    line_height: LineHeight,
    letter_spacing: LetterSpacing,
    line_break: LineBreak,
    justify: Justify,
    scale_factor: f32,
    viewport_size: Vec2,
    rem_size: RemSize,
}

struct ShapeContext {
    font: TextFont,
    line_height: LineHeight,
    scale_factor: f32,
    viewport_size: Vec2,
}

struct VisibleText {
    text: String,
    replacement_range: Option<TextRange>,
    preedit_range: Option<Range<usize>>,
    preedit_caret: Option<usize>,
}

/// Shape text without a resolved rectangle and update `UiIntrinsicSize` before
/// the layout pass. Explicit pixel widths are used for wrapped text areas;
/// auto/fill widths use the unbounded preferred text width.
pub fn measure_text_intrinsics(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let entities = text_entities(world, false);
    if entities.is_empty() {
        return;
    }

    world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
        world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
            world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                world.resource_scope(|world, mut layout_cx: Mut<LayoutCx>| {
                    for target in entities {
                        let Some(mut text_layout) =
                            world.get::<UiTextLayout>(target.entity).cloned()
                        else {
                            continue;
                        };
                        let visible_text = text_layout.visible_text(&target.buffer);
                        let display_text = visible_text.text;
                        let max_width = intrinsic_width(&text_layout, target.node);
                        let computed_style = world.get::<UiComputedStyle>(target.entity).cloned();
                        let (viewport, scale_factor) =
                            logical_viewport_metrics(world, target.entity, None);
                        let font = resolve_text_font(
                            world,
                            target.entity,
                            text_layout.font(&theme, computed_style.as_ref()),
                        );
                        let key = text_layout.cache_key(
                            &display_text,
                            max_width,
                            ShapeContext {
                                font,
                                line_height: text_layout
                                    .line_height(&theme, computed_style.as_ref()),
                                scale_factor,
                                viewport_size: viewport,
                            },
                        );
                        if text_layout.intrinsic_key.as_ref() == Some(&key)
                            && text_layout.intrinsic_bounds.is_some()
                        {
                            continue;
                        }
                        let shaped = TextLayout::shape_with_bevy_config(
                            &display_text,
                            max_width,
                            UiTextLayout::config(&key),
                            &mut pipeline,
                            &fonts,
                            &mut font_cx,
                            &mut layout_cx,
                        );
                        match shaped {
                            Ok(shaped) => {
                                let bounds = shaped.bounds();
                                let intrinsic = intrinsic_size(
                                    bounds,
                                    &text_layout,
                                    target.node,
                                    key.scale_factor,
                                );
                                text_layout.intrinsic_key = Some(key);
                                text_layout.intrinsic_bounds = Some(bounds);
                                text_layout.last_error = None;
                                world
                                    .entity_mut(target.entity)
                                    .insert((text_layout, UiIntrinsicSize(intrinsic)));
                            }
                            Err(error) => {
                                text_layout.intrinsic_key = Some(key);
                                text_layout.intrinsic_bounds = None;
                                text_layout.last_error = Some(format!("{error:?}"));
                                world.entity_mut(target.entity).insert(text_layout);
                            }
                        }
                    }
                });
            });
        });
    });
}

/// Shape text against final BUI bounds after layout and before widget paint.
pub fn update_text_layouts(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let entities = text_entities(world, true);
    if entities.is_empty() {
        return;
    }

    world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
        world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
            world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                world.resource_scope(|world, mut layout_cx: Mut<LayoutCx>| {
                    for target in entities {
                        let Some(result) = target.result else {
                            continue;
                        };
                        let Some(mut text_layout) =
                            world.get::<UiTextLayout>(target.entity).cloned()
                        else {
                            continue;
                        };
                        let visible_text = text_layout.visible_text(&target.buffer);
                        let display_text = visible_text.text;
                        let prefix_bytes = text_layout.prefix.len();
                        let value_len = target.buffer.len_bytes();
                        let padding = target.node.map_or([0.0; 2], |node| {
                            [
                                node.padding.left + node.padding.right,
                                node.padding.top + node.padding.bottom,
                            ]
                        });
                        let content_width =
                            (result.rect.width - padding[0] - text_layout.inset[0].max(0.0) * 2.0)
                                .max(0.0);
                        let content_height =
                            (result.rect.height - padding[1] - text_layout.inset[1].max(0.0) * 2.0)
                                .max(0.0);
                        let content_viewport = [content_width, content_height];
                        let computed_style = world.get::<UiComputedStyle>(target.entity).cloned();
                        let (viewport, scale_factor) =
                            logical_viewport_metrics(world, target.entity, Some(result));
                        let font = resolve_text_font(
                            world,
                            target.entity,
                            text_layout.font(&theme, computed_style.as_ref()),
                        );
                        let key = text_layout.cache_key(
                            &display_text,
                            content_width,
                            ShapeContext {
                                font,
                                line_height: text_layout
                                    .line_height(&theme, computed_style.as_ref()),
                                scale_factor,
                                viewport_size: viewport,
                            },
                        );
                        let focused = world
                            .get_resource::<UiFocus>()
                            .and_then(|focus| focus.state(result.window))
                            .is_some_and(|state| state.focused == Some(target.entity));
                        let geometry_changed = text_layout.display_text != display_text
                            || text_layout.prefix_bytes != prefix_bytes
                            || text_layout.value_len != value_len
                            || text_layout.replacement_range != visible_text.replacement_range
                            || text_layout.preedit_range != visible_text.preedit_range
                            || text_layout.preedit_caret != visible_text.preedit_caret
                            || text_layout.buffer_selection != Some(target.buffer.selection())
                            || text_layout.scroll_viewport != Some(content_viewport)
                            || text_layout.last_focused != Some(focused);
                        text_layout.display_text.clone_from(&display_text);
                        text_layout.prefix_bytes = prefix_bytes;
                        text_layout.value_len = value_len;
                        text_layout.replacement_range = visible_text.replacement_range;
                        text_layout.preedit_range = visible_text.preedit_range;
                        text_layout.preedit_caret = visible_text.preedit_caret;
                        text_layout.buffer_selection = Some(target.buffer.selection());
                        text_layout.scroll_viewport = Some(content_viewport);
                        text_layout.last_focused = Some(focused);
                        let shape_is_current = text_layout.shape_key.as_ref() == Some(&key)
                            && text_layout.text_layout.is_some();
                        if !shape_is_current {
                            text_layout.reset_visual_navigation();
                            let shaped = TextLayout::shape_with_bevy_config(
                                &display_text,
                                content_width,
                                UiTextLayout::config(&key),
                                &mut pipeline,
                                &fonts,
                                &mut font_cx,
                                &mut layout_cx,
                            );
                            match shaped {
                                Ok(shaped) => {
                                    text_layout.text_layout = Some(shaped);
                                    text_layout.shape_key = Some(key);
                                    text_layout.last_error = None;
                                }
                                Err(error) => {
                                    text_layout.text_layout = None;
                                    text_layout.shape_key = Some(key);
                                    text_layout.text_origin =
                                        Some(text_origin(&text_layout, result, target.node));
                                    text_layout.last_error = Some(format!("{error:?}"));
                                    world.entity_mut(target.entity).insert(text_layout);
                                    continue;
                                }
                            }
                        }

                        let scroll_changed = if focused && (geometry_changed || !shape_is_current) {
                            ensure_caret_visible(&mut text_layout, &target.buffer, content_viewport)
                        } else {
                            clamp_scroll_offset(&mut text_layout, &target.buffer, content_viewport)
                        };
                        let origin = text_origin(&text_layout, result, target.node);
                        let origin_changed = text_layout.text_origin != Some(origin);
                        text_layout.text_origin = Some(origin);
                        if geometry_changed || scroll_changed || origin_changed || !shape_is_current
                        {
                            world.entity_mut(target.entity).insert(text_layout);
                        }
                    }
                });
            });
        });
    });
}

#[derive(Clone)]
struct TextEntity {
    entity: Entity,
    buffer: TextBuffer,
    node: Option<UiLayoutNode>,
    result: Option<UiLayoutResult>,
}

fn text_entities(world: &mut World, require_layout: bool) -> Vec<TextEntity> {
    let mut query = world.query::<(Entity, &TextBuffer, &UiTextLayout)>();
    query
        .iter(world)
        .filter_map(|(entity, buffer, _)| {
            let result = world.get::<UiLayoutResult>(entity).copied();
            if require_layout && result.is_none() {
                return None;
            }
            Some(TextEntity {
                entity,
                buffer: buffer.clone(),
                node: world.get::<UiLayoutNode>(entity).copied(),
                result,
            })
        })
        .collect()
}

fn intrinsic_width(layout: &UiTextLayout, node: Option<UiLayoutNode>) -> f32 {
    let Some(node) = node else {
        return f32::INFINITY;
    };
    match node.size.width {
        UiLength::Px(width) if layout.multiline && layout.wrap => {
            (width - node.padding.left - node.padding.right - layout.inset[0].max(0.0) * 2.0)
                .max(0.0)
        }
        _ => f32::INFINITY,
    }
}

fn intrinsic_size(
    bounds: crate::TextBounds,
    layout: &UiTextLayout,
    node: Option<UiLayoutNode>,
    scale_factor: f32,
) -> [f32; 2] {
    let scale_factor = positive(scale_factor, 1.0);
    let padding = node.map_or([0.0; 2], |node| {
        [
            node.padding.left + node.padding.right,
            node.padding.top + node.padding.bottom,
        ]
    });
    [
        bounds.width / scale_factor + padding[0] + layout.inset[0].max(0.0) * 2.0,
        bounds.height / scale_factor + padding[1] + layout.inset[1].max(0.0) * 2.0,
    ]
}

fn ensure_caret_visible(
    layout: &mut UiTextLayout,
    buffer: &TextBuffer,
    viewport: [f32; 2],
) -> bool {
    let Some(shaped) = layout.text_layout.as_ref() else {
        return false;
    };
    let scale = layout.layout_scale_factor();
    let mut caret = shaped.caret_rect_with_affinity(
        layout.display_caret(buffer),
        layout.display_caret_affinity(buffer),
    );
    scale_rect(&mut caret, 1.0 / scale);
    let bounds = shaped.bounds();
    let max_x = (bounds.width / scale.max(0.001)).max(caret.x + caret.width) - viewport[0];
    let max_y = (bounds.height / scale.max(0.001)).max(caret.y + caret.height) - viewport[1];
    let max_x = max_x.max(0.0);
    let max_y = max_y.max(0.0);
    let old = layout.scroll_offset;
    let mut x = finite_nonnegative(old[0]).min(max_x);
    let mut y = finite_nonnegative(old[1]).min(max_y);
    if viewport[0] > 0.0 {
        if caret.x < x {
            x = caret.x.max(0.0);
        } else if caret.x + caret.width > x + viewport[0] {
            x = (caret.x + caret.width - viewport[0]).max(0.0);
        }
    }
    if viewport[1] > 0.0 {
        if caret.y < y {
            y = caret.y.max(0.0);
        } else if caret.y + caret.height > y + viewport[1] {
            y = (caret.y + caret.height - viewport[1]).max(0.0);
        }
    }
    layout.scroll_offset = [x.min(max_x), y.min(max_y)];
    (layout.scroll_offset[0] - old[0]).abs() > 0.01
        || (layout.scroll_offset[1] - old[1]).abs() > 0.01
}

fn clamp_scroll_offset(layout: &mut UiTextLayout, buffer: &TextBuffer, viewport: [f32; 2]) -> bool {
    let Some(shaped) = layout.text_layout.as_ref() else {
        return false;
    };
    let scale = layout.layout_scale_factor();
    let mut caret = shaped.caret_rect_with_affinity(
        layout.display_caret(buffer),
        layout.display_caret_affinity(buffer),
    );
    scale_rect(&mut caret, 1.0 / scale);
    let bounds = shaped.bounds();
    let max_x =
        ((bounds.width / scale.max(0.001)).max(caret.x + caret.width) - viewport[0]).max(0.0);
    let max_y =
        ((bounds.height / scale.max(0.001)).max(caret.y + caret.height) - viewport[1]).max(0.0);
    let old = layout.scroll_offset;
    layout.scroll_offset = [
        finite_nonnegative(old[0]).min(max_x),
        finite_nonnegative(old[1]).min(max_y),
    ];
    (layout.scroll_offset[0] - finite_nonnegative(old[0])).abs() > 0.01
        || (layout.scroll_offset[1] - finite_nonnegative(old[1])).abs() > 0.01
}

fn logical_viewport_metrics(
    world: &World,
    entity: Entity,
    result: Option<UiLayoutResult>,
) -> (Vec2, f32) {
    let viewports = world.get_resource::<UiViewports>();
    if let Some(result) = result {
        if let Some(viewport) = viewports.and_then(|viewports| viewports.0.get(&result.window)) {
            return (
                Vec2::new(viewport.rect.width, viewport.rect.height),
                positive(result.dpi_scale, 1.0),
            );
        }
        if let Some(root) = world.get::<UiLayoutRoot>(result.window) {
            return (
                Vec2::new(root.viewport.width, root.viewport.height),
                positive(result.dpi_scale, root.dpi_scale),
            );
        }
        return (Vec2::ZERO, positive(result.dpi_scale, 1.0));
    }
    let mut current = Some(entity);
    while let Some(node) = current {
        if let Some(root) = world.get::<UiLayoutRoot>(node) {
            if let Some(viewport) = viewports.and_then(|viewports| viewports.0.get(&root.window)) {
                return (
                    Vec2::new(viewport.rect.width, viewport.rect.height),
                    positive(viewport.dpi_scale, root.dpi_scale),
                );
            }
            return (
                Vec2::new(root.viewport.width, root.viewport.height),
                positive(root.dpi_scale, 1.0),
            );
        }
        current = world.get::<UiParent>(node).map(|parent| parent.0);
    }
    (Vec2::ZERO, 1.0)
}

fn translate(rect: &mut TextRect, origin: [f32; 2]) {
    rect.x += origin[0];
    rect.y += origin[1];
}

fn text_origin(
    layout: &UiTextLayout,
    result: UiLayoutResult,
    node: Option<UiLayoutNode>,
) -> [f32; 2] {
    [
        result.rect.x + node.map_or(0.0, |node| node.padding.left) + layout.inset[0].max(0.0)
            - layout.scroll_offset[0].max(0.0),
        result.rect.y + node.map_or(0.0, |node| node.padding.top) + layout.inset[1].max(0.0)
            - layout.scroll_offset[1].max(0.0),
    ]
}

fn scale_rect(rect: &mut TextRect, scale: f32) {
    rect.x *= scale;
    rect.y *= scale;
    rect.width *= scale;
    rect.height *= scale;
}

fn positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn set_buffer_caret(buffer: &mut TextBuffer, target: TextCaretPosition, extend: bool) -> bool {
    let before = (buffer.anchor(), buffer.caret());
    let after = if extend {
        (buffer.anchor(), target.byte_index)
    } else {
        (target.byte_index, target.byte_index)
    };
    let affinities_after = if extend {
        (buffer.anchor_affinity(), target.affinity)
    } else {
        (target.affinity, target.affinity)
    };
    if before == after && affinities_after == (buffer.anchor_affinity(), buffer.caret_affinity()) {
        return false;
    }
    buffer.set_selection_with_affinity(after.0, after.1, affinities_after.0, affinities_after.1);
    true
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

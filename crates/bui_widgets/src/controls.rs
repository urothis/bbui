//! Reusable controls with interaction state independent of a particular input backend.

use bevy_ecs::prelude::Component;
use bui_layout::Rect;
use bui_render::{ClipRect, DrawItem, Primitive};
use bui_style::{Rgba, Theme, ThemeColorToken};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetStatus {
    #[default]
    Idle,
    Hovered,
    Pressed,
    Focused,
    Disabled,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct Button {
    pub label: String,
    pub enabled: bool,
    pub status: WidgetStatus,
    pub activations: u64,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            enabled: true,
            status: WidgetStatus::Idle,
            activations: 0,
        }
    }

    /// Activate on pointer release or Enter/Space. Disabled buttons never activate.
    pub fn activate(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        self.activations = self.activations.saturating_add(1);
        true
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct Checkbox {
    pub label: String,
    pub value: CheckboxValue,
    pub enabled: bool,
    pub read_only: bool,
    pub status: WidgetStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckboxValue {
    #[default]
    Off,
    On,
    Mixed,
}

impl Checkbox {
    pub fn new(label: impl Into<String>, checked: bool) -> Self {
        Self {
            label: label.into(),
            value: if checked {
                CheckboxValue::On
            } else {
                CheckboxValue::Off
            },
            enabled: true,
            read_only: false,
            status: WidgetStatus::Idle,
        }
    }

    pub fn is_checked(&self) -> bool {
        self.value == CheckboxValue::On
    }

    pub fn set(&mut self, value: bool) -> bool {
        if !self.enabled || self.read_only {
            return false;
        }
        let value = if value {
            CheckboxValue::On
        } else {
            CheckboxValue::Off
        };
        if self.value == value {
            return false;
        }
        self.value = value;
        true
    }

    pub fn toggle(&mut self) -> bool {
        self.set(!self.is_checked())
    }

    /// Paints a checkbox body and a check or mixed-state mark into the shared display list.
    pub fn draw_items(&self, rect: Rect, theme: &Theme, z: i32) -> Vec<DrawItem> {
        self.draw_items_clipped(rect, None, theme, z)
    }

    /// Paint using the layout clip shared by hit testing and all other widget primitives.
    pub fn draw_items_clipped(
        &self,
        rect: Rect,
        inherited_clip: Option<Rect>,
        theme: &Theme,
        z: i32,
    ) -> Vec<DrawItem> {
        let widget_clip = ClipRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let box_size = rect.width.min(rect.height).max(0.0);
        let box_rect = ClipRect {
            x: rect.x,
            y: rect.y,
            width: box_size,
            height: rect.height,
        };
        let clip = inherited_clip
            .and_then(|bounds| rect.intersection(bounds))
            .map(|bounds| ClipRect {
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
            })
            .unwrap_or(widget_clip);
        let disabled = !self.enabled;
        let color = if disabled {
            theme.color(ThemeColorToken::ControlDisabledSurface)
        } else {
            theme.surface
        };
        let mut items = vec![DrawItem {
            clip,
            z,
            primitive: Primitive::RoundedRect {
                rect: box_rect,
                radius: (box_size * 0.15).max(0.0),
                color,
            },
        }];
        let mark_color = if disabled {
            theme.color(ThemeColorToken::ControlDisabledMark)
        } else {
            theme.accent
        };
        let inset = (box_size * 0.24).max(1.0);
        match self.value {
            CheckboxValue::Off => {}
            CheckboxValue::Mixed => items.push(DrawItem {
                clip,
                z: z.saturating_add(1),
                primitive: Primitive::Line {
                    from: [rect.x + inset, rect.y + rect.height * 0.5],
                    to: [rect.x + box_size - inset, rect.y + rect.height * 0.5],
                    width: (box_size * 0.12).max(1.0),
                    color: mark_color,
                },
            }),
            CheckboxValue::On => {
                let left = [rect.x + inset, rect.y + rect.height * 0.52];
                let bend = [rect.x + box_size * 0.43, rect.y + rect.height - inset];
                let tip = [rect.x + box_size - inset * 0.7, rect.y + inset];
                items.push(DrawItem {
                    clip,
                    z: z.saturating_add(1),
                    primitive: Primitive::Line {
                        from: left,
                        to: bend,
                        width: (box_size * 0.12).max(1.0),
                        color: mark_color,
                    },
                });
                items.push(DrawItem {
                    clip,
                    z: z.saturating_add(1),
                    primitive: Primitive::Line {
                        from: bend,
                        to: tip,
                        width: (box_size * 0.12).max(1.0),
                        color: mark_color,
                    },
                });
            }
        }
        if !self.label.is_empty() {
            items.push(DrawItem {
                clip,
                z: z.saturating_add(2),
                primitive: Primitive::Text {
                    origin: [
                        rect.x + box_size + theme.spacing.control.checkbox_label_gap,
                        rect.y,
                    ],
                    text: self.label.clone(),
                    color: if disabled {
                        theme.color(ThemeColorToken::ControlDisabledText)
                    } else {
                        theme.text
                    },
                },
            });
        }
        items
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioOption {
    pub label: String,
    pub enabled: bool,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct RadioGroup {
    pub options: Vec<RadioOption>,
    pub selected: Option<usize>,
    pub enabled: bool,
}

impl RadioGroup {
    pub fn new(
        labels: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<usize>,
    ) -> Self {
        let options: Vec<_> = labels
            .into_iter()
            .map(|label| RadioOption {
                label: label.into(),
                enabled: true,
            })
            .collect();
        let selected = selected.filter(|index| *index < options.len());
        Self {
            options,
            selected,
            enabled: true,
        }
    }

    pub fn select(&mut self, index: usize) -> bool {
        if !self.enabled
            || !self.options.get(index).is_some_and(|option| option.enabled)
            || self.selected == Some(index)
        {
            return false;
        }
        self.selected = Some(index);
        true
    }

    pub fn move_selection(&mut self, delta: i32) -> Option<usize> {
        if !self.enabled || self.options.is_empty() {
            return None;
        }
        let start = self
            .selected
            .unwrap_or(if delta < 0 { 0 } else { self.options.len() - 1 });
        let direction = delta.signum();
        if direction == 0 {
            return self.selected;
        }
        let mut at = start as i64;
        for _ in 0..self.options.len() {
            at = (at + direction as i64).rem_euclid(self.options.len() as i64);
            if self.options[at as usize].enabled {
                self.selected = Some(at as usize);
                return self.selected;
            }
        }
        None
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct Dropdown {
    pub options: Vec<String>,
    pub selected: Option<usize>,
    pub highlighted: Option<usize>,
    pub open: bool,
    pub enabled: bool,
}

impl Dropdown {
    pub fn new(
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<usize>,
    ) -> Self {
        let options: Vec<_> = options.into_iter().map(Into::into).collect();
        let selected = selected.filter(|index| *index < options.len());
        Self {
            options,
            selected,
            highlighted: selected,
            open: false,
            enabled: true,
        }
    }

    pub fn open(&mut self) {
        if self.enabled {
            self.open = true;
            self.highlighted = self
                .selected
                .or_else(|| (!self.options.is_empty()).then_some(0));
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.highlighted = self.selected;
    }

    pub fn move_highlight(&mut self, delta: i32) -> Option<usize> {
        if !self.open || self.options.is_empty() || delta == 0 {
            return self.highlighted;
        }
        let start = self.highlighted.unwrap_or(0) as i64;
        self.highlighted =
            Some((start + delta as i64).rem_euclid(self.options.len() as i64) as usize);
        self.highlighted
    }

    pub fn choose(&mut self, index: usize) -> bool {
        if !self.enabled || !self.options.get(index).is_some() {
            return false;
        }
        let changed = self.selected != Some(index);
        self.selected = Some(index);
        self.close();
        changed
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct Label {
    pub text: String,
    pub selectable: bool,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            selectable: false,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct IconButton {
    pub icon: String,
    pub accessible_name: String,
    pub enabled: bool,
    pub activations: u64,
}

impl IconButton {
    pub fn new(icon: impl Into<String>, accessible_name: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            accessible_name: accessible_name.into(),
            enabled: true,
            activations: 0,
        }
    }

    pub fn activate(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        self.activations = self.activations.saturating_add(1);
        true
    }
}

/// Asset source for an image-backed icon button.
///
/// The path is resolved by Bevy's `AssetServer` while the control is visible. The widget cache
/// drops its strong handle and renderer registration when the control leaves the visible tree.
#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
pub struct IconButtonAsset {
    pub path: String,
    /// Optional sRGB tint. `None` uses the themed icon color, or the disabled icon color.
    pub tint: Option<Rgba>,
    /// Increment to explicitly retry a failed load without changing the asset path.
    pub retry_generation: u64,
}

impl IconButtonAsset {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            tint: None,
            retry_generation: 0,
        }
    }

    pub fn with_tint(mut self, tint: Rgba) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn retry(&mut self) {
        self.retry_generation = self.retry_generation.wrapping_add(1);
    }
}

/// Public load status for an image-backed icon button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconButtonAssetStatus {
    /// The owner is hidden, clipped, unmounted, or has not requested the asset yet.
    Hidden,
    Loading,
    Ready,
    Failed(String),
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct IconButtonAssetState {
    pub status: IconButtonAssetStatus,
}

impl Default for IconButtonAssetState {
    fn default() -> Self {
        Self {
            status: IconButtonAssetStatus::Hidden,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[component(summary_tick)]
pub struct Progress {
    pub value: f32,
    pub indeterminate: bool,
}

impl Progress {
    pub fn new(value: f32) -> Self {
        Self {
            value: if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            },
            indeterminate: false,
        }
    }

    pub fn set(&mut self, value: f32) {
        self.value = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            self.value
        };
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct Tooltip {
    pub text: String,
    pub delay_millis: u32,
    pub visible: bool,
}

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            delay_millis: 400,
            visible: false,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct TextField {
    pub text: String,
    /// Caret and selection use Unicode scalar-value indices, not UTF-8 byte offsets.
    pub caret: usize,
    pub selection: Option<Range<usize>>,
    pub placeholder: String,
    pub enabled: bool,
    pub read_only: bool,
    pub max_chars: Option<usize>,
}

impl TextField {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.chars().count();
        Self {
            text,
            caret,
            selection: None,
            placeholder: String::new(),
            enabled: true,
            read_only: false,
            max_chars: None,
        }
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection
            .as_ref()
            .map(|range| slice_chars(&self.text, range.clone()).to_owned())
    }

    pub fn select(&mut self, range: Range<usize>) {
        let len = self.text.chars().count();
        let start = range.start.min(len);
        let end = range.end.min(len);
        self.selection = (start != end).then_some(start.min(end)..start.max(end));
        self.caret = end;
    }

    pub fn select_all(&mut self) {
        self.select(0..self.text.chars().count());
    }

    pub fn insert(&mut self, input: &str) -> bool {
        if !self.enabled || self.read_only || input.is_empty() {
            return false;
        }
        let current_len = self.text.chars().count();
        let removed = self
            .selection
            .as_ref()
            .map_or(0, |range| range.end - range.start);
        let allowed = self.max_chars.map_or(input.chars().count(), |limit| {
            limit.saturating_sub(current_len - removed)
        });
        let inserted: String = input.chars().take(allowed).collect();
        if inserted.is_empty() {
            return false;
        }
        let range = self.selection.take().unwrap_or(self.caret..self.caret);
        let (start, end) = byte_range(&self.text, range.clone());
        self.text.replace_range(start..end, &inserted);
        self.caret = range.start + inserted.chars().count();
        true
    }

    pub fn backspace(&mut self) -> bool {
        if !self.enabled || self.read_only {
            return false;
        }
        if let Some(range) = self.selection.take() {
            if range.start == range.end {
                return false;
            }
            let (start, end) = byte_range(&self.text, range.clone());
            self.text.replace_range(start..end, "");
            self.caret = range.start;
            return true;
        }
        if self.caret == 0 {
            return false;
        }
        let range = self.caret - 1..self.caret;
        let (start, end) = byte_range(&self.text, range);
        self.text.replace_range(start..end, "");
        self.caret -= 1;
        true
    }

    pub fn move_caret(&mut self, delta: i32, extend: bool) {
        let old = self.caret;
        let len = self.text.chars().count();
        self.caret = (self.caret as i64 + delta as i64).clamp(0, len as i64) as usize;
        if extend {
            let anchor = self.selection.as_ref().map_or(old, |range| {
                if old == range.start {
                    range.end
                } else {
                    range.start
                }
            });
            self.selection =
                (anchor != self.caret).then_some(anchor.min(self.caret)..anchor.max(self.caret));
        } else {
            self.selection = None;
        }
    }

    pub fn clear(&mut self) -> bool {
        if !self.enabled || self.read_only || self.text.is_empty() {
            return false;
        }
        self.text.clear();
        self.caret = 0;
        self.selection = None;
        true
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct TextArea {
    pub field: TextField,
    pub scroll_line: usize,
    pub wrap: bool,
}

impl TextArea {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            field: TextField::new(text),
            scroll_line: 0,
            wrap: true,
        }
    }

    pub fn insert_text(&mut self, text: &str) -> bool {
        self.field.insert(text)
    }

    pub fn move_vertical(&mut self, lines: i32, columns: usize) {
        let current = self.field.caret;
        let before = slice_chars(&self.field.text, 0..current);
        let line_start = before
            .rfind('\n')
            .map_or(0, |byte| before[..byte].chars().count() + 1);
        let column = current.saturating_sub(line_start).min(columns);
        let all_lines: Vec<&str> = self.field.text.split('\n').collect();
        let current_line = before.matches('\n').count();
        let target = (current_line as i64 + lines as i64)
            .clamp(0, all_lines.len().saturating_sub(1) as i64) as usize;
        let target_offset: usize = all_lines
            .iter()
            .take(target)
            .map(|line| line.chars().count() + 1)
            .sum();
        self.field.caret = target_offset + column.min(all_lines[target].chars().count());
        self.field.selection = None;
    }
}

fn byte_range(text: &str, range: Range<usize>) -> (usize, usize) {
    let start = char_to_byte(text, range.start);
    let end = char_to_byte(text, range.end);
    (start, end)
}

fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}

fn slice_chars(text: &str, range: Range<usize>) -> &str {
    let (start, end) = byte_range(text, range);
    &text[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_paints_off_on_and_mixed_states() {
        let theme = Theme::default();
        let rect = Rect {
            x: 2.0,
            y: 3.0,
            width: 18.0,
            height: 18.0,
        };
        let mut checkbox = Checkbox::new("enabled", false);
        assert_eq!(checkbox.draw_items(rect, &theme, 4).len(), 2);
        checkbox.value = CheckboxValue::On;
        assert_eq!(checkbox.draw_items(rect, &theme, 4).len(), 4);
        checkbox.value = CheckboxValue::Mixed;
        assert_eq!(checkbox.draw_items(rect, &theme, 4).len(), 3);
    }

    #[test]
    fn checkbox_uses_theme_disabled_tokens_and_configured_label_gap() {
        let rect = Rect {
            x: 2.0,
            y: 3.0,
            width: 18.0,
            height: 18.0,
        };
        let mut theme = Theme::default();
        theme.spacing.control.checkbox_label_gap = 13.0;
        let mut checkbox = Checkbox::new("locked", true);
        checkbox.enabled = false;

        let items = checkbox.draw_items(rect, &theme, 0);
        let expected_surface = theme.color(ThemeColorToken::ControlDisabledSurface);
        let expected_mark = theme.color(ThemeColorToken::ControlDisabledMark);
        let expected_text = theme.color(ThemeColorToken::ControlDisabledText);
        assert!(matches!(
            items[0].primitive,
            Primitive::RoundedRect { color, .. } if color == expected_surface
        ));
        assert!(matches!(
            items[1].primitive,
            Primitive::Line { color, .. } if color == expected_mark
        ));
        assert!(matches!(
            items.last().unwrap().primitive,
            Primitive::Text { origin, color, .. }
                if origin == [33.0, 3.0] && color == expected_text
        ));

        theme.switch(bui_style::ThemeKind::Light);
        let light_items = checkbox.draw_items(rect, &theme, 0);
        assert!(matches!(
            light_items[0].primitive,
            Primitive::RoundedRect { color, .. }
                if color == theme.color(ThemeColorToken::ControlDisabledSurface)
        ));
        assert!(matches!(
            light_items.last().unwrap().primitive,
            Primitive::Text { color, .. }
                if color == theme.color(ThemeColorToken::ControlDisabledText)
        ));
    }

    #[test]
    fn checkbox_read_only_and_button_disabled_states_do_not_activate() {
        let mut checkbox = Checkbox::new("locked", true);
        checkbox.read_only = true;
        assert!(!checkbox.toggle());
        let mut button = Button::new("save");
        button.enabled = false;
        assert!(!button.activate());
        assert_eq!(button.activations, 0);
    }

    #[test]
    fn text_field_edits_unicode_by_character_index() {
        let mut field = TextField::new("a🧊c");
        field.select(1..2);
        assert_eq!(field.selected_text().as_deref(), Some("🧊"));
        assert!(field.insert("雪"));
        assert_eq!(field.text, "a雪c");
        assert_eq!(field.caret, 2);
        assert!(field.backspace());
        assert_eq!(field.text, "ac");
    }

    #[test]
    fn radio_navigation_skips_disabled_options_and_dropdown_wraps() {
        let mut radios = RadioGroup::new(["a", "b", "c"], Some(0));
        radios.options[1].enabled = false;
        assert_eq!(radios.move_selection(1), Some(2));
        let mut dropdown = Dropdown::new(["a", "b"], Some(0));
        dropdown.open();
        assert_eq!(dropdown.move_highlight(-1), Some(1));
        assert!(dropdown.choose(1));
        assert!(!dropdown.open);
    }
}

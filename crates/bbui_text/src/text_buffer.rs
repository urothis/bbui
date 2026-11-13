//! Unicode-safe text editing state with local undo, selection, and IME support.

use bevy_ecs::prelude::Component;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start.min(end),
            end: start.max(end),
        }
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn len(self) -> usize {
        self.end - self.start
    }
}

/// Visual side of an insertion point shared by the end of one wrapped line
/// and the start of the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CaretAffinity {
    #[default]
    Downstream,
    Upstream,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextCaretPosition {
    pub byte_index: usize,
    pub affinity: CaretAffinity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImeComposition {
    pub text: String,
    /// Selection/caret offsets in the composition string, as UTF-8 byte offsets.
    pub cursor_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    anchor: usize,
    caret: usize,
    anchor_affinity: CaretAffinity,
    caret_affinity: CaretAffinity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Insert,
    Backspace,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditRecord {
    start: usize,
    deleted: String,
    inserted: String,
    before: Snapshot,
    after: Snapshot,
    kind: EditKind,
    group: u64,
}

/// Editable text state. Caret and selection positions are UTF-8 byte offsets
/// snapped to extended grapheme cluster boundaries.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
#[component(summary_tick)]
pub struct TextBuffer {
    text: String,
    anchor: usize,
    caret: usize,
    anchor_affinity: CaretAffinity,
    caret_affinity: CaretAffinity,
    composition: Option<ImeComposition>,
    undo: Vec<EditRecord>,
    redo: Vec<EditRecord>,
    preferred_column: Option<usize>,
    edit_group: u64,
    last_edit_millis: Option<u64>,
}

impl TextBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let end = text.len();
        Self {
            text,
            anchor: end,
            caret: end,
            anchor_affinity: CaretAffinity::Downstream,
            caret_affinity: CaretAffinity::Downstream,
            ..Self::default()
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn len_bytes(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn caret_affinity(&self) -> CaretAffinity {
        self.caret_affinity
    }

    pub fn anchor_affinity(&self) -> CaretAffinity {
        self.anchor_affinity
    }

    pub fn selection(&self) -> TextRange {
        TextRange::new(self.anchor, self.caret)
    }

    pub fn selected_text(&self) -> Option<&str> {
        let selection = self.selection();
        (!selection.is_empty()).then(|| &self.text[selection.start..selection.end])
    }

    pub fn composition(&self) -> Option<&ImeComposition> {
        self.composition.as_ref()
    }

    pub fn set_selection(&mut self, anchor: usize, caret: usize) {
        self.set_selection_with_affinity(
            anchor,
            caret,
            CaretAffinity::Downstream,
            CaretAffinity::Downstream,
        );
    }

    pub fn set_selection_with_affinity(
        &mut self,
        anchor: usize,
        caret: usize,
        anchor_affinity: CaretAffinity,
        caret_affinity: CaretAffinity,
    ) {
        self.break_undo_group();
        self.anchor = floor_boundary(&self.text, anchor.min(self.text.len()));
        self.caret = floor_boundary(&self.text, caret.min(self.text.len()));
        self.anchor_affinity = anchor_affinity;
        self.caret_affinity = caret_affinity;
        self.preferred_column = None;
    }

    pub fn collapse_to(&mut self, caret: usize) {
        self.set_selection(caret, caret);
    }

    pub fn select_all(&mut self) {
        self.break_undo_group();
        self.anchor = 0;
        self.caret = self.text.len();
        self.anchor_affinity = CaretAffinity::Downstream;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
    }

    pub fn insert_text(&mut self, inserted: &str) -> bool {
        if inserted.is_empty() && self.selection().is_empty() {
            return false;
        }
        let before = self.snapshot();
        let selection = self.selection();
        let deleted = self.text[selection.start..selection.end].to_owned();
        self.text
            .replace_range(selection.start..selection.end, inserted);
        self.caret = ceil_boundary(&self.text, selection.start + inserted.len());
        self.anchor = self.caret;
        self.anchor_affinity = CaretAffinity::Downstream;
        self.caret_affinity = CaretAffinity::Downstream;
        self.composition = None;
        self.preferred_column = None;
        self.record(
            before,
            selection.start,
            deleted,
            inserted.to_owned(),
            EditKind::Insert,
        );
        true
    }

    pub fn delete_backward(&mut self) -> bool {
        let selection = self.selection();
        if selection.is_empty() {
            if self.caret == 0 {
                return false;
            }
            let start = previous_boundary(&self.text, self.caret);
            return self.delete_range(TextRange::new(start, self.caret), EditKind::Backspace);
        }
        self.delete_range(selection, EditKind::Backspace)
    }

    pub fn delete_forward(&mut self) -> bool {
        let selection = self.selection();
        if selection.is_empty() {
            if self.caret >= self.text.len() {
                return false;
            }
            let end = next_boundary(&self.text, self.caret);
            return self.delete_range(TextRange::new(self.caret, end), EditKind::Delete);
        }
        self.delete_range(selection, EditKind::Delete)
    }

    pub fn delete_word_backward(&mut self) -> bool {
        if !self.selection().is_empty() {
            return self.delete_backward();
        }
        self.delete_range(
            TextRange::new(word_left_boundary(&self.text, self.caret), self.caret),
            EditKind::Backspace,
        )
    }

    pub fn delete_word_forward(&mut self) -> bool {
        if !self.selection().is_empty() {
            return self.delete_forward();
        }
        self.delete_range(
            TextRange::new(self.caret, word_right_boundary(&self.text, self.caret)),
            EditKind::Delete,
        )
    }

    pub fn move_left(&mut self, extend: bool) {
        self.move_horizontal(false, extend);
    }

    pub fn move_right(&mut self, extend: bool) {
        self.move_horizontal(true, extend);
    }

    pub fn move_word_left(&mut self, extend: bool) {
        self.move_to(word_left_boundary(&self.text, self.caret), extend);
    }

    pub fn move_word_right(&mut self, extend: bool) {
        self.move_to(word_right_boundary(&self.text, self.caret), extend);
    }

    pub fn move_home(&mut self, extend: bool) {
        let start = self.text[..self.caret].rfind('\n').map_or(0, |at| at + 1);
        self.move_to(start, extend);
    }

    pub fn move_end(&mut self, extend: bool) {
        let end = self.text[self.caret..]
            .find('\n')
            .map_or(self.text.len(), |at| self.caret + at);
        self.move_to(end, extend);
    }

    pub fn move_document_start(&mut self, extend: bool) {
        self.move_to(0, extend);
    }

    pub fn move_document_end(&mut self, extend: bool) {
        self.move_to(self.text.len(), extend);
    }

    pub fn move_vertical(&mut self, down: bool, extend: bool) {
        let (line_start, column) = line_and_column(&self.text, self.caret);
        let desired_column = *self.preferred_column.get_or_insert(column);
        let previous_line = down.then(|| {
            self.text[line_start..]
                .find('\n')
                .map(|at| line_start + at + 1)
        });
        let target = if down {
            previous_line
                .flatten()
                .and_then(|start| position_at_column(&self.text, start, desired_column))
                .unwrap_or(self.caret)
        } else if line_start == 0 {
            0
        } else {
            let before = &self.text[..line_start - 1];
            let previous_start = before.rfind('\n').map_or(0, |at| at + 1);
            position_at_column(&self.text, previous_start, desired_column).unwrap_or(line_start - 1)
        };
        self.move_to(target, extend);
        self.preferred_column = Some(desired_column);
    }

    pub fn set_composition(&mut self, text: impl Into<String>, cursor_range: Option<TextRange>) {
        let text = text.into();
        let cursor_range = cursor_range.map(|range| {
            TextRange::new(
                floor_boundary(&text, range.start.min(text.len())),
                floor_boundary(&text, range.end.min(text.len())),
            )
        });
        self.composition = Some(ImeComposition { text, cursor_range });
    }

    pub fn commit_composition(&mut self, committed: Option<&str>) -> bool {
        let value = committed.map(str::to_owned).or_else(|| {
            self.composition
                .as_ref()
                .map(|composition| composition.text.clone())
        });
        self.composition = None;
        value.is_some_and(|value| self.insert_text(&value))
    }

    pub fn cancel_composition(&mut self) -> bool {
        self.composition.take().is_some()
    }

    pub fn copy(&self) -> Option<String> {
        self.selected_text().map(str::to_owned)
    }

    pub fn cut(&mut self) -> Option<String> {
        let copied = self.copy()?;
        self.delete_backward();
        Some(copied)
    }

    pub fn paste(&mut self, text: &str) -> bool {
        self.break_undo_group();
        self.insert_text(text)
    }

    /// Begin a new local undo step after a pause between edits.
    pub fn begin_timed_edit(&mut self, now_seconds: f64) {
        if !now_seconds.is_finite() || now_seconds < 0.0 {
            return;
        }
        let now_millis = (now_seconds * 1000.0) as u64;
        if self
            .last_edit_millis
            .is_some_and(|last| now_millis.saturating_sub(last) > 750 || now_millis < last)
        {
            self.break_undo_group();
        }
        self.last_edit_millis = Some(now_millis);
    }

    /// Separate the next edit from the previous typing, delete, or paste step.
    pub fn break_undo_group(&mut self) {
        self.edit_group = self.edit_group.wrapping_add(1);
        self.last_edit_millis = None;
    }

    pub fn undo(&mut self) -> bool {
        let Some(record) = self.undo.pop() else {
            return false;
        };
        self.text.replace_range(
            record.start..record.start + record.inserted.len(),
            &record.deleted,
        );
        self.restore_selection(&record.before);
        self.redo.push(record);
        self.break_undo_group();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(record) = self.redo.pop() else {
            return false;
        };
        self.text.replace_range(
            record.start..record.start + record.deleted.len(),
            &record.inserted,
        );
        self.restore_selection(&record.after);
        self.undo.push(record);
        self.break_undo_group();
        true
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Clear local undo and redo steps at an application-history boundary.
    ///
    /// Text widgets keep their local undo stack while focused, then commit the
    /// editing session to shared UI history when focus leaves. Clearing both
    /// stacks prevents that same edit from being applied twice by local undo.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.break_undo_group();
    }

    /// Replace committed text and its directed byte-offset selection without
    /// recording a local edit. Historical restore also cancels IME composition.
    pub fn restore_committed_text(&mut self, text: impl Into<String>, anchor: usize, caret: usize) {
        self.text = text.into();
        self.anchor = floor_boundary(&self.text, anchor.min(self.text.len()));
        self.caret = floor_boundary(&self.text, caret.min(self.text.len()));
        self.anchor_affinity = CaretAffinity::Downstream;
        self.caret_affinity = CaretAffinity::Downstream;
        self.composition = None;
        self.preferred_column = None;
        self.clear_history();
    }

    fn move_horizontal(&mut self, right: bool, extend: bool) {
        let selection = self.selection();
        let next = if !extend && !selection.is_empty() {
            if right {
                selection.end
            } else {
                selection.start
            }
        } else if right {
            next_boundary(&self.text, self.caret)
        } else {
            previous_boundary(&self.text, self.caret)
        };
        self.move_to(next, extend);
    }

    fn move_to(&mut self, position: usize, extend: bool) {
        let position = floor_boundary(&self.text, position.min(self.text.len()));
        if self.caret != position || (!extend && self.anchor != position) {
            self.break_undo_group();
        }
        if !extend {
            self.anchor = position;
            self.anchor_affinity = CaretAffinity::Downstream;
        }
        self.caret = position;
        self.caret_affinity = CaretAffinity::Downstream;
        self.preferred_column = None;
    }

    fn delete_range(&mut self, range: TextRange, kind: EditKind) -> bool {
        if range.is_empty() {
            return false;
        }
        let before = self.snapshot();
        let deleted = self.text[range.start..range.end].to_owned();
        self.text.replace_range(range.start..range.end, "");
        self.anchor = range.start;
        self.caret = range.start;
        self.anchor_affinity = CaretAffinity::Downstream;
        self.caret_affinity = CaretAffinity::Downstream;
        self.composition = None;
        self.preferred_column = None;
        self.record(before, range.start, deleted, String::new(), kind);
        true
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            anchor: self.anchor,
            caret: self.caret,
            anchor_affinity: self.anchor_affinity,
            caret_affinity: self.caret_affinity,
        }
    }

    fn restore_selection(&mut self, snapshot: &Snapshot) {
        self.anchor = snapshot.anchor;
        self.caret = snapshot.caret;
        self.anchor_affinity = snapshot.anchor_affinity;
        self.caret_affinity = snapshot.caret_affinity;
        self.composition = None;
        self.preferred_column = None;
    }

    fn record(
        &mut self,
        before: Snapshot,
        start: usize,
        deleted: String,
        inserted: String,
        kind: EditKind,
    ) {
        let after = self.snapshot();
        if let Some(last) = self.undo.last_mut()
            && last.kind == kind
            && last.after == before
            && last.group == self.edit_group
            && before.anchor == before.caret
        {
            let merged = match kind {
                EditKind::Insert
                    if last.deleted.is_empty()
                        && deleted.is_empty()
                        && start == last.start + last.inserted.len() =>
                {
                    last.inserted.push_str(&inserted);
                    true
                }
                EditKind::Backspace
                    if inserted.is_empty()
                        && last.inserted.is_empty()
                        && start + deleted.len() == last.start =>
                {
                    last.deleted.insert_str(0, &deleted);
                    last.start = start;
                    true
                }
                EditKind::Delete
                    if inserted.is_empty() && last.inserted.is_empty() && start == last.start =>
                {
                    last.deleted.push_str(&deleted);
                    true
                }
                _ => false,
            };
            if merged {
                last.after = after;
                self.redo.clear();
                return;
            }
        }
        self.undo.push(EditRecord {
            start,
            deleted,
            inserted,
            before,
            after,
            kind,
            group: self.edit_group,
        });
        self.redo.clear();
    }
}

/// Component form used by text widgets.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiTextBuffer(pub TextBuffer);

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UiMultiline;

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiReadOnly;

/// Hand clipboard shortcuts to a widget/platform bridge instead of the local
/// [`crate::UiClipboard`] resource. The routed shortcut event remains visible
/// to widget systems.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UiExternalClipboard;

fn floor_boundary(text: &str, position: usize) -> usize {
    let position = position.min(text.len());
    if position == text.len() {
        return position;
    }
    text.grapheme_indices(true)
        .take_while(|(index, _)| *index <= position)
        .last()
        .map_or(0, |(index, _)| index)
}

fn ceil_boundary(text: &str, position: usize) -> usize {
    let position = position.min(text.len());
    text.grapheme_indices(true)
        .map(|(index, _)| index)
        .find(|index| *index >= position)
        .unwrap_or(text.len())
}

fn previous_boundary(text: &str, position: usize) -> usize {
    let position = floor_boundary(text, position);
    text[..position]
        .grapheme_indices(true)
        .next_back()
        .map_or(0, |(index, _)| index)
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn word_left_boundary(text: &str, caret: usize) -> usize {
    let mut position = caret;
    while position > 0 {
        let previous = previous_boundary(text, position);
        let character = text[previous..position].chars().next().unwrap_or(' ');
        if is_word_character(character) {
            break;
        }
        position = previous;
    }
    while position > 0 {
        let previous = previous_boundary(text, position);
        let character = text[previous..position].chars().next().unwrap_or(' ');
        if !is_word_character(character) {
            break;
        }
        position = previous;
    }
    position
}

fn word_right_boundary(text: &str, caret: usize) -> usize {
    let mut position = caret;
    while position < text.len() {
        let next = next_boundary(text, position);
        let character = text[position..next].chars().next().unwrap_or(' ');
        if is_word_character(character) {
            break;
        }
        position = next;
    }
    while position < text.len() {
        let next = next_boundary(text, position);
        let character = text[position..next].chars().next().unwrap_or(' ');
        if !is_word_character(character) {
            break;
        }
        position = next;
    }
    position
}

fn next_boundary(text: &str, position: usize) -> usize {
    let position = floor_boundary(text, position);
    if position == text.len() {
        return position;
    }
    text[position..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(text.len(), |(offset, _)| position + offset)
}

fn line_and_column(text: &str, position: usize) -> (usize, usize) {
    let start = text[..position].rfind('\n').map_or(0, |at| at + 1);
    let column = text[start..position].graphemes(true).count();
    (start, column)
}

fn position_at_column(text: &str, start: usize, column: usize) -> Option<usize> {
    let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
    Some(
        text[start..end]
            .grapheme_indices(true)
            .nth(column)
            .map_or(end, |(offset, _)| start + offset),
    )
}

#[cfg(test)]
mod tests;

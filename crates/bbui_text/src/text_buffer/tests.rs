use super::*;

#[test]
fn edits_move_selection_and_undo_as_local_transactions() {
    let mut buffer = TextBuffer::new("hello");
    buffer.insert_text("!");
    buffer.insert_text("?");
    assert_eq!(buffer.as_str(), "hello!?");
    assert_eq!(buffer.undo_len(), 1, "adjacent typing is coalesced");
    assert!(buffer.undo());
    assert_eq!(buffer.as_str(), "hello");
    assert!(buffer.redo());
    assert_eq!(buffer.as_str(), "hello!?");
    buffer.set_selection(5, 7);
    assert_eq!(buffer.cut().as_deref(), Some("!?"));
    assert_eq!(buffer.as_str(), "hello");
}

#[test]
fn committed_restore_keeps_directed_selection_and_clears_local_history() {
    let mut buffer = TextBuffer::new("old");
    buffer.insert_text("!");
    assert_eq!(buffer.undo_len(), 1);
    buffer.set_composition("preedit", None);

    buffer.restore_committed_text("a🧊c", 6, 1);
    assert_eq!(buffer.as_str(), "a🧊c");
    assert_eq!(buffer.anchor(), 6);
    assert_eq!(buffer.caret(), 1);
    assert!(buffer.composition().is_none());
    assert_eq!(buffer.undo_len(), 0);
    assert!(!buffer.undo());
    assert!(!buffer.redo());

    buffer.restore_committed_text("a🧊c", 3, 2);
    assert_eq!(buffer.anchor(), 1, "partial grapheme offsets snap backward");
    assert_eq!(buffer.caret(), 1);
}

#[test]
fn navigation_and_delete_respect_unicode_boundaries() {
    let mut buffer = TextBuffer::new("a🙂b");
    buffer.move_left(false);
    assert_eq!(buffer.caret(), "a🙂".len());
    assert!(buffer.delete_backward());
    assert_eq!(buffer.as_str(), "ab");
    buffer.set_selection(1, 1);
    buffer.insert_text("ñ");
    assert_eq!(buffer.as_str(), "añb");
    assert!(buffer.as_str().is_char_boundary(buffer.caret()));
}

#[test]
fn ime_composition_can_commit_or_cancel_without_partial_text_edits() {
    let mut buffer = TextBuffer::new("a");
    buffer.set_composition("に", Some(TextRange::new(0, 3)));
    assert_eq!(buffer.as_str(), "a");
    assert!(buffer.commit_composition(None));
    assert_eq!(buffer.as_str(), "aに");
    buffer.set_composition("x", None);
    assert!(buffer.cancel_composition());
    assert_eq!(buffer.as_str(), "aに");
}

#[test]
fn vertical_navigation_keeps_a_preferred_column() {
    let mut buffer = TextBuffer::new("abcd\nef\nghij");
    buffer.set_selection(3, 3);
    buffer.move_vertical(true, false);
    assert_eq!(&buffer.as_str()[..buffer.caret()], "abcd\nef");
    buffer.move_vertical(true, false);
    assert_eq!(&buffer.as_str()[..buffer.caret()], "abcd\nef\nghi");
}

#[test]
fn word_navigation_respects_utf8_boundaries() {
    let mut buffer = TextBuffer::new("one café, two");
    buffer.move_word_left(false);
    assert_eq!(&buffer.as_str()[buffer.caret()..], "two");
    buffer.move_word_left(false);
    assert_eq!(&buffer.as_str()[buffer.caret()..], "café, two");
    buffer.move_word_right(false);
    assert_eq!(&buffer.as_str()[buffer.caret()..], ", two");
}

#[test]
fn caret_and_delete_treat_combining_marks_and_emoji_as_single_characters() {
    let mut buffer = TextBuffer::new("e\u{301}👨‍👩‍👧‍👦x");
    buffer.set_selection(1, 1);
    assert_eq!(buffer.caret(), 0, "selection cannot split a grapheme");
    buffer.move_right(false);
    assert_eq!(buffer.caret(), "e\u{301}".len());
    buffer.move_right(false);
    assert_eq!(buffer.caret(), "e\u{301}👨‍👩‍👧‍👦".len());
    assert!(buffer.delete_backward());
    assert_eq!(buffer.as_str(), "e\u{301}x");
    assert!(buffer.undo());
    assert_eq!(buffer.as_str(), "e\u{301}👨‍👩‍👧‍👦x");
}

#[test]
fn insertion_that_joins_the_next_grapheme_keeps_caret_at_a_valid_boundary() {
    let mut buffer = TextBuffer::new("\u{301}x");
    buffer.move_document_start(false);
    buffer.insert_text("e");
    assert_eq!(buffer.as_str(), "e\u{301}x");
    assert_eq!(buffer.caret(), "e\u{301}".len());
    assert!(buffer.delete_backward());
    assert_eq!(buffer.as_str(), "x");
    assert!(buffer.undo());
    assert_eq!(buffer.as_str(), "e\u{301}x");
}

#[test]
fn vertical_navigation_counts_graphemes_instead_of_code_points() {
    let mut buffer = TextBuffer::new("ae\u{301}b\nxyz");
    buffer.set_selection("ae\u{301}".len(), "ae\u{301}".len());
    buffer.move_vertical(true, false);
    assert_eq!(&buffer.as_str()[..buffer.caret()], "ae\u{301}b\nxy");
}

#[test]
fn timed_typing_and_caret_moves_split_local_undo_steps() {
    let mut buffer = TextBuffer::new("");
    buffer.begin_timed_edit(0.0);
    buffer.insert_text("a");
    buffer.begin_timed_edit(0.2);
    buffer.insert_text("b");
    buffer.begin_timed_edit(1.1);
    buffer.insert_text("c");
    assert_eq!(buffer.undo_len(), 2);
    assert!(buffer.undo());
    assert_eq!(buffer.as_str(), "ab");
    buffer.move_left(false);
    buffer.insert_text("x");
    assert!(buffer.undo());
    assert_eq!(buffer.as_str(), "ab");
    assert!(buffer.undo());
    assert_eq!(buffer.as_str(), "");
}

#[test]
fn word_deletion_and_document_navigation_preserve_grapheme_boundaries() {
    let mut buffer = TextBuffer::new("e\u{301}cho world\nlast");
    buffer.move_document_start(false);
    buffer.move_word_right(false);
    assert_eq!(&buffer.as_str()[..buffer.caret()], "e\u{301}cho");
    assert!(buffer.delete_word_backward());
    assert_eq!(buffer.as_str(), " world\nlast");
    buffer.move_document_end(false);
    buffer.move_document_start(true);
    assert_eq!(buffer.selected_text(), Some(" world\nlast"));
    buffer.move_document_start(false);
    assert!(buffer.delete_word_forward());
    assert_eq!(buffer.as_str(), "\nlast");
}

#[test]
fn undo_history_stores_changes_instead_of_whole_document_snapshots() {
    let original = "a".repeat(100_000);
    let mut buffer = TextBuffer::new(&original);
    for index in 0..100 {
        buffer.begin_timed_edit(index as f64);
        buffer.insert_text("x");
    }
    let stored_bytes: usize = buffer
        .undo
        .iter()
        .map(|record| record.deleted.len() + record.inserted.len())
        .sum();
    assert_eq!(stored_bytes, 100);
    assert_eq!(buffer.undo_len(), 100);
    for _ in 0..100 {
        assert!(buffer.undo());
    }
    assert_eq!(buffer.as_str(), original);
    for _ in 0..100 {
        assert!(buffer.redo());
    }
    assert_eq!(buffer.len_bytes(), original.len() + 100);
}

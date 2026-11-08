//! Transactional undo and redo for UI edits.

use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

/// A committed before/after edit. `before == after` is never recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit<T> {
    pub key: String,
    pub before: T,
    pub after: T,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction<T> {
    pub label: String,
    pub edits: Vec<Edit<T>>,
}

#[derive(Debug)]
pub struct History<T> {
    undo: Vec<Transaction<T>>,
    redo: Vec<Transaction<T>>,
    active: Option<Transaction<T>>,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            active: None,
        }
    }
}

impl<T: Clone + PartialEq> History<T> {
    pub fn begin(&mut self, label: impl Into<String>) -> Result<(), &'static str> {
        if self.active.is_some() {
            return Err("transaction already active");
        }
        self.active = Some(Transaction {
            label: label.into(),
            edits: Vec::new(),
        });
        Ok(())
    }

    pub fn record(
        &mut self,
        key: impl Into<String>,
        before: T,
        after: T,
    ) -> Result<(), &'static str> {
        let tx = self.active.as_mut().ok_or("no active transaction")?;
        let key = key.into();
        if let Some(edit) = tx.edits.iter_mut().find(|edit| edit.key == key) {
            edit.after = after;
        } else {
            tx.edits.push(Edit { key, before, after });
        }
        Ok(())
    }

    /// Returns the original values to restore on cancellation.
    pub fn cancel(&mut self) -> Option<Vec<(String, T)>> {
        self.active.take().map(|tx| {
            tx.edits
                .into_iter()
                .rev()
                .map(|edit| (edit.key, edit.before))
                .collect()
        })
    }

    pub fn commit(&mut self) -> Result<bool, &'static str> {
        let mut tx = self.active.take().ok_or("no active transaction")?;
        tx.edits.retain(|edit| edit.before != edit.after);
        if tx.edits.is_empty() {
            return Ok(false);
        }
        self.undo.push(tx);
        self.redo.clear();
        Ok(true)
    }

    /// Returns changes in application order; caller applies them to its model.
    pub fn undo(&mut self) -> Option<Vec<(String, T)>> {
        if self.active.is_some() {
            return None;
        }
        let tx = self.undo.pop()?;
        let changes = tx
            .edits
            .iter()
            .rev()
            .map(|edit| (edit.key.clone(), edit.before.clone()))
            .collect();
        self.redo.push(tx);
        Some(changes)
    }

    /// Preview the next undo without changing the history stacks.
    pub fn peek_undo(&self) -> Option<Vec<(String, T)>> {
        if self.active.is_some() {
            return None;
        }
        Some(
            self.undo
                .last()?
                .edits
                .iter()
                .rev()
                .map(|edit| (edit.key.clone(), edit.before.clone()))
                .collect(),
        )
    }

    pub fn redo(&mut self) -> Option<Vec<(String, T)>> {
        if self.active.is_some() {
            return None;
        }
        let tx = self.redo.pop()?;
        let changes = tx
            .edits
            .iter()
            .map(|edit| (edit.key.clone(), edit.after.clone()))
            .collect();
        self.undo.push(tx);
        Some(changes)
    }

    /// Preview the next redo without changing the history stacks.
    pub fn peek_redo(&self) -> Option<Vec<(String, T)>> {
        if self.active.is_some() {
            return None;
        }
        Some(
            self.redo
                .last()?
                .edits
                .iter()
                .map(|edit| (edit.key.clone(), edit.after.clone()))
                .collect(),
        )
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

/// Values that can be stored in the built-in UI undo service.
#[derive(Debug, Clone, PartialEq)]
pub enum UiValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Numbers(Vec<f64>),
    OptionalIndex(Option<usize>),
    Text(String),
    Color([f32; 4]),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextSelection {
    pub anchor: usize,
    pub caret: usize,
}

/// State that accompanies an undoable gesture so editors can restore their
/// keyboard focus and text/range selection when the gesture is replayed.
/// Selection endpoints retain anchor/caret direction; callers should not
/// normalize them to an unordered range.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiEditContext {
    pub focused_window: Option<bevy_ecs::entity::Entity>,
    pub focused_key: Option<String>,
    pub selections: HashMap<String, TextSelection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiHistoryRestore {
    pub changes: Vec<(String, UiValue)>,
    pub context: UiEditContext,
}

/// ECS resource wrapping value history with matching focus/selection snapshots.
#[derive(Resource, Debug, Default)]
pub struct UiHistory {
    values: History<UiValue>,
    active_context: Option<UiEditContext>,
    undo_context: Vec<(UiEditContext, UiEditContext)>,
    redo_context: Vec<(UiEditContext, UiEditContext)>,
}

impl UiHistory {
    pub fn begin(
        &mut self,
        label: impl Into<String>,
        context: UiEditContext,
    ) -> Result<(), &'static str> {
        self.values.begin(label)?;
        self.active_context = Some(context);
        Ok(())
    }

    pub fn record(
        &mut self,
        key: impl Into<String>,
        before: UiValue,
        after: UiValue,
    ) -> Result<(), &'static str> {
        self.values.record(key, before, after)
    }

    pub fn commit(&mut self, context_after: UiEditContext) -> Result<bool, &'static str> {
        let changed = self.values.commit()?;
        let before = self.active_context.take().unwrap_or_default();
        if changed {
            self.undo_context.push((before, context_after));
            self.redo_context.clear();
        }
        Ok(changed)
    }

    pub fn cancel(&mut self) -> Option<UiHistoryRestore> {
        let changes = self.values.cancel()?;
        Some(UiHistoryRestore {
            changes,
            context: self.active_context.take().unwrap_or_default(),
        })
    }

    pub fn undo(&mut self) -> Option<UiHistoryRestore> {
        let changes = self.values.undo()?;
        let context = self.undo_context.pop().unwrap_or_default();
        let restore = UiHistoryRestore {
            changes,
            context: context.0.clone(),
        };
        self.redo_context.push(context);
        Some(restore)
    }

    pub fn peek_undo(&self) -> Option<UiHistoryRestore> {
        Some(UiHistoryRestore {
            changes: self.values.peek_undo()?,
            context: self.undo_context.last()?.0.clone(),
        })
    }

    pub fn redo(&mut self) -> Option<UiHistoryRestore> {
        let changes = self.values.redo()?;
        let context = self.redo_context.pop().unwrap_or_default();
        let restore = UiHistoryRestore {
            changes,
            context: context.1.clone(),
        };
        self.undo_context.push(context);
        Some(restore)
    }

    pub fn peek_redo(&self) -> Option<UiHistoryRestore> {
        Some(UiHistoryRestore {
            changes: self.values.peek_redo()?,
            context: self.redo_context.last()?.1.clone(),
        })
    }

    pub fn undo_len(&self) -> usize {
        self.values.undo_len()
    }

    pub fn redo_len(&self) -> usize {
        self.values.redo_len()
    }

    pub fn is_active(&self) -> bool {
        self.active_context.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gesture_is_one_reversible_transaction() {
        let mut h = History::default();
        h.begin("multi drag").unwrap();
        h.record("a", 1, 2).unwrap();
        h.record("a", 2, 3).unwrap();
        h.record("b", 4, 5).unwrap();
        assert!(h.commit().unwrap());
        assert_eq!(h.undo_len(), 1);
        assert_eq!(h.undo(), Some(vec![("b".into(), 4), ("a".into(), 1)]));
        assert_eq!(h.redo(), Some(vec![("a".into(), 3), ("b".into(), 5)]));
    }

    #[test]
    fn cancellation_and_new_edit_clear_redo_correctly() {
        let mut h = History::default();
        h.begin("one").unwrap();
        h.record("a", 0, 1).unwrap();
        h.commit().unwrap();
        h.undo();
        h.begin("cancel").unwrap();
        h.record("a", 0, 9).unwrap();
        assert_eq!(h.cancel(), Some(vec![("a".into(), 0)]));
        assert_eq!(h.redo_len(), 1);
        h.begin("two").unwrap();
        h.record("a", 0, 2).unwrap();
        h.commit().unwrap();
        assert_eq!(h.redo_len(), 0);
    }

    #[test]
    fn repeated_widget_edits_coalesce_and_net_noops_are_removed() {
        let mut history = History::default();
        history.begin("mixed controls").unwrap();
        history
            .record("panel", UiValue::Bool(false), UiValue::Bool(true))
            .unwrap();
        history
            .record("amount", UiValue::Float(1.0), UiValue::Float(2.0))
            .unwrap();
        history
            .record("panel", UiValue::Bool(true), UiValue::Bool(false))
            .unwrap();
        history
            .record("amount", UiValue::Float(2.0), UiValue::Float(3.0))
            .unwrap();

        assert!(history.commit().unwrap());
        assert_eq!(history.undo_len(), 1);
        assert_eq!(
            history.undo(),
            Some(vec![("amount".into(), UiValue::Float(1.0))])
        );
        assert_eq!(
            history.redo(),
            Some(vec![("amount".into(), UiValue::Float(3.0))])
        );
    }

    #[test]
    fn cancel_restores_coalesced_values_in_reverse_order_and_preserves_redo() {
        let mut history = History::default();
        history.begin("committed").unwrap();
        history.record("a", 0, 1).unwrap();
        history.record("b", 4, 5).unwrap();
        history.commit().unwrap();
        assert_eq!(history.undo(), Some(vec![("b".into(), 4), ("a".into(), 0)]));

        history.begin("cancelled gesture").unwrap();
        history.record("a", 0, 2).unwrap();
        history.record("b", 4, 6).unwrap();
        history.record("a", 2, 3).unwrap();
        assert_eq!(
            history.cancel(),
            Some(vec![("b".into(), 4), ("a".into(), 0)])
        );
        assert_eq!(history.redo_len(), 1);
        assert_eq!(history.redo(), Some(vec![("a".into(), 1), ("b".into(), 5)]));
    }

    #[test]
    fn ui_history_restores_focus_and_selection_with_values() {
        let mut history = UiHistory::default();
        let before = UiEditContext {
            focused_window: Some(bevy_ecs::entity::Entity::PLACEHOLDER),
            focused_key: Some("field".into()),
            selections: HashMap::from([(
                "field".into(),
                TextSelection {
                    anchor: 3,
                    caret: 1,
                },
            )]),
        };
        let after = UiEditContext {
            focused_window: Some(bevy_ecs::entity::Entity::PLACEHOLDER),
            focused_key: Some("other".into()),
            selections: HashMap::from([(
                "other".into(),
                TextSelection {
                    anchor: 2,
                    caret: 2,
                },
            )]),
        };
        history.begin("edit", before.clone()).unwrap();
        history
            .record(
                "field",
                UiValue::Text("old".into()),
                UiValue::Text("new".into()),
            )
            .unwrap();
        history
            .record("checkbox", UiValue::Bool(false), UiValue::Bool(true))
            .unwrap();
        assert!(history.commit(after.clone()).unwrap());
        assert_eq!(
            history.undo().unwrap(),
            UiHistoryRestore {
                changes: vec![
                    ("checkbox".into(), UiValue::Bool(false)),
                    ("field".into(), UiValue::Text("old".into()))
                ],
                context: before,
            }
        );
        assert_eq!(
            history.redo().unwrap(),
            UiHistoryRestore {
                changes: vec![
                    ("field".into(), UiValue::Text("new".into())),
                    ("checkbox".into(), UiValue::Bool(true))
                ],
                context: after,
            }
        );
    }

    #[test]
    fn cancelled_and_empty_ui_transactions_keep_context_stacks_aligned() {
        let mut history = UiHistory::default();
        let before = UiEditContext {
            focused_window: Some(bevy_ecs::entity::Entity::PLACEHOLDER),
            focused_key: Some("field".into()),
            selections: HashMap::from([(
                "field".into(),
                TextSelection {
                    anchor: 8,
                    caret: 2,
                },
            )]),
        };
        let after = UiEditContext {
            focused_key: Some("next-field".into()),
            ..before.clone()
        };
        history.begin("committed", before.clone()).unwrap();
        history
            .record(
                "field",
                UiValue::Text("old".into()),
                UiValue::Text("new".into()),
            )
            .unwrap();
        assert!(history.commit(after.clone()).unwrap());
        assert_eq!(history.undo().unwrap().context, before);

        history.begin("cancelled", after.clone()).unwrap();
        history
            .record(
                "field",
                UiValue::Text("old".into()),
                UiValue::Text("draft".into()),
            )
            .unwrap();
        assert_eq!(
            history.cancel(),
            Some(UiHistoryRestore {
                changes: vec![("field".into(), UiValue::Text("old".into()))],
                context: after.clone(),
            })
        );
        assert_eq!(history.redo_len(), 1);

        history.begin("net no-op", before.clone()).unwrap();
        history
            .record(
                "field",
                UiValue::Text("old".into()),
                UiValue::Text("old".into()),
            )
            .unwrap();
        assert!(!history.commit(after.clone()).unwrap());
        assert_eq!(history.redo_len(), 1);
        assert_eq!(history.redo().unwrap().context, after);
    }

    #[test]
    fn generated_multi_value_transactions_round_trip_without_losing_first_values() {
        for seed in 0_u64..256 {
            let mut state = seed.wrapping_add(1);
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                (state >> 32) as i32
            };
            let mut history = History::default();
            let mut before = HashMap::new();
            let mut after = HashMap::new();
            history.begin("generated gesture").unwrap();
            for index in 0..8 {
                let key = format!("field-{}", index % 4);
                let first = *before.entry(key.clone()).or_insert_with(&mut next);
                let last = next();
                history.record(key.clone(), first, last).unwrap();
                after.insert(key, last);
            }
            let changed = before.iter().any(|(key, value)| after[key] != *value);
            assert_eq!(history.commit().unwrap(), changed);
            if changed {
                let undone: HashMap<_, _> = history.undo().unwrap().into_iter().collect();
                let redone: HashMap<_, _> = history.redo().unwrap().into_iter().collect();
                for (key, first) in before {
                    if after[&key] == first {
                        assert!(!undone.contains_key(&key));
                    } else {
                        assert_eq!(undone[&key], first);
                        assert_eq!(redone[&key], after[&key]);
                    }
                }
            }
        }
    }

    #[test]
    fn previews_match_undo_redo_and_leave_the_stacks_untouched() {
        let mut history = UiHistory::default();
        let before = UiEditContext {
            focused_key: Some("before".into()),
            ..Default::default()
        };
        let after = UiEditContext {
            focused_key: Some("after".into()),
            ..Default::default()
        };
        history.begin("preview", before).unwrap();
        history
            .record("field", UiValue::Float(1.0), UiValue::Float(2.0))
            .unwrap();
        history.commit(after).unwrap();

        let preview = history.peek_undo().unwrap();
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.redo_len(), 0);
        assert_eq!(history.undo().unwrap(), preview);
        let preview = history.peek_redo().unwrap();
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 1);
        assert_eq!(history.redo().unwrap(), preview);
    }
}

//! Transaction adapter between widget events and BUI's shared undo history.

use bevy_ecs::prelude::*;
use bui_core::{
    TextSelection, UiEditContext, UiFocus, UiFocusable, UiHistory, UiHistoryRestore, UiKey,
    UiParent, UiSelectionSnapshot, UiValue, is_interactable_in_window,
};
use bui_text::{TextBuffer, UiExternalClipboard, UiReadOnly, UiTextBuffer, UiTextUndoFallback};

use crate::actions::{WidgetEvent, WidgetEventQueue, WidgetValue};
use crate::{
    Canvas, Checkbox, CheckboxValue, ColorRamp, CurveEditor, DataTable, Dropdown, FileBrowser,
    HsvColorEditor, ListModel, NodeGraph, NumericField, Panel, PanelExpansionStore, PanelGroup,
    PreviewGrid, RadioGroup, Slider, TreeModel,
};

#[derive(Resource, Debug, Default)]
pub struct WidgetHistoryCursor {
    generation: Option<u64>,
    processed: usize,
    entities: std::collections::HashMap<String, Entity>,
}

/// Record newly appended `WidgetEventQueue` edits exactly once. Discrete edits
/// create one transaction each, while checkbox painting and numeric gestures
/// remain atomic transactions as reported by the widget event producer.
pub fn record_widget_edits(world: &mut World) {
    let (generation, queued) = {
        let queue = world.resource::<WidgetEventQueue>();
        (queue.generation(), queue.0.clone())
    };
    let start = {
        let mut cursor = world.resource_mut::<WidgetHistoryCursor>();
        if cursor.generation != Some(generation) {
            cursor.generation = Some(generation);
            cursor.processed = 0;
        }
        let start = cursor.processed.min(queued.len());
        cursor.processed = queued.len();
        start
    };

    for event in queued.into_iter().skip(start) {
        match event {
            WidgetEvent::ValueChanged {
                target,
                before,
                after,
            } => {
                if let (Some(before), Some(after)) = (widget_value(before), widget_value(after)) {
                    commit_transaction(world, "Widget value", vec![(target, before, after)]);
                }
            }
            WidgetEvent::TextCommitted {
                target,
                window,
                before,
                after,
                before_selection,
                after_selection,
            } => commit_text_transaction(
                world,
                target,
                window,
                before,
                after,
                before_selection,
                after_selection,
            ),
            WidgetEvent::CheckboxPaintCommitted { changes } => {
                let edits = changes
                    .into_iter()
                    .map(|change| {
                        (
                            change.entity,
                            checkbox_value(change.before),
                            checkbox_value(change.after),
                        )
                    })
                    .collect();
                commit_transaction(world, "Paint checkboxes", edits);
            }
            WidgetEvent::NumericCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Scrub numeric value",
                    vec![(target, UiValue::Numbers(before), UiValue::Numbers(after))],
                );
            }
            WidgetEvent::NumericGroupCommitted { changes } => {
                commit_transaction(
                    world,
                    "Scrub numeric fields",
                    changes
                        .into_iter()
                        .map(|(entity, before, after)| {
                            (entity, UiValue::Float(before), UiValue::Float(after))
                        })
                        .collect(),
                );
            }
            WidgetEvent::PanelGroupCommitted { changes } => {
                commit_transaction(
                    world,
                    "Expand panel group",
                    changes
                        .into_iter()
                        .map(|(entity, before, after)| {
                            (entity, UiValue::Bool(before), UiValue::Bool(after))
                        })
                        .collect(),
                );
            }
            WidgetEvent::PanelOrderCommitted {
                group,
                persistence_key: _,
                before,
                after,
            } => {
                if let (Ok(before), Ok(after)) =
                    (serde_json::to_vec(&before), serde_json::to_vec(&after))
                {
                    commit_transaction(
                        world,
                        "Reorder panels",
                        vec![(group, UiValue::Bytes(before), UiValue::Bytes(after))],
                    );
                }
            }
            WidgetEvent::FileBrowserCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit file browser view",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::ColorRampCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit color ramp",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::HsvColorCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit color",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::CurveEditorCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit curve",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::CollectionCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit collection",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::DataTableCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit data table",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::CanvasCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit canvas",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            WidgetEvent::NodeGraphCommitted {
                target,
                before,
                after,
            } => {
                commit_transaction(
                    world,
                    "Edit node graph",
                    vec![(
                        target,
                        UiValue::Bytes(before.into_bytes()),
                        UiValue::Bytes(after.into_bytes()),
                    )],
                );
            }
            // In-progress text edits stay in TextBuffer's local undo stack;
            // TextCommitted above records the completed session globally.
            // Cancelled gestures and non-value events enter neither history.
            _ => {}
        }
    }
}

fn commit_transaction(world: &mut World, label: &str, edits: Vec<(Entity, UiValue, UiValue)>) {
    let edits: Vec<_> = edits
        .into_iter()
        .filter(|(_, before, after)| before != after)
        .map(|(entity, before, after)| (history_key(world, entity), entity, before, after))
        .collect();
    if edits.is_empty() {
        return;
    }
    {
        let mut cursor = world.resource_mut::<WidgetHistoryCursor>();
        for (key, entity, _, _) in &edits {
            cursor.entities.insert(key.clone(), *entity);
        }
    }
    let context_before = capture_context(world);
    remember_focused_entity(world);
    let context_after = context_before.clone();
    let mut history = world.resource_mut::<UiHistory>();
    if history.begin(label, context_before).is_err() {
        return;
    }
    for (key, _, before, after) in edits {
        let _ = history.record(key, before, after);
    }
    let _ = history.commit(context_after);
}

fn commit_text_transaction(
    world: &mut World,
    target: Entity,
    window: Entity,
    before: String,
    after: String,
    before_selection: TextSelection,
    after_selection: TextSelection,
) {
    if before == after || world.get_entity(target).is_err() {
        return;
    }
    let key = history_key(world, target);
    world
        .resource_mut::<WidgetHistoryCursor>()
        .entities
        .insert(key.clone(), target);
    let mut context_before = UiEditContext {
        focused_window: Some(window),
        focused_key: Some(key.clone()),
        ..UiEditContext::default()
    };
    context_before
        .selections
        .insert(key.clone(), before_selection);
    let mut context_after = context_before.clone();
    context_after
        .selections
        .insert(key.clone(), after_selection);
    let mut history = world.resource_mut::<UiHistory>();
    if history.begin("Edit text", context_before).is_err() {
        return;
    }
    let _ = history.record(key, UiValue::Text(before), UiValue::Text(after));
    let _ = history.commit(context_after);
}

fn remember_focused_entity(world: &mut World) {
    let focused = world
        .get_resource::<UiFocus>()
        .and_then(|focus| focus.active_window.and_then(|window| focus.state(window)))
        .and_then(|state| state.focused);
    if let Some(entity) = focused {
        let key = history_key(world, entity);
        world
            .resource_mut::<WidgetHistoryCursor>()
            .entities
            .insert(key, entity);
    }
}

/// Apply focused Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y commands before widget key
/// handlers. A text component gets first refusal in `UiSystems::Text`; when its
/// local stack is empty, TextPlugin marks the target for application fallback.
pub fn process_widget_undo_shortcuts(world: &mut World) {
    let events = world
        .get_resource::<bui_core::UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    for event in events {
        let bui_core::UiRoutedEvent::Key {
            target,
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        if !(modifiers.control || modifiers.super_key)
            || !matches!(key, bui_core::UiKeyCode::Z | bui_core::UiKeyCode::Y)
        {
            continue;
        }
        let text_buffer = world.get::<TextBuffer>(target).is_some()
            || world.get::<UiTextBuffer>(target).is_some();
        if text_buffer
            && !world
                .get_resource::<UiTextUndoFallback>()
                .is_some_and(|fallback| fallback.needs_global_undo(target))
        {
            continue;
        }
        let redo = key == bui_core::UiKeyCode::Y || modifiers.shift;
        if world.get::<FileBrowser>(target).is_some() {
            let handled = if redo {
                crate::actions::redo_file_browser_filter_draft(world, target)
            } else {
                crate::actions::undo_file_browser_filter_draft(world, target)
            };
            if handled {
                continue;
            }
        }
        let preview = {
            let history = world.resource::<UiHistory>();
            if redo {
                history.peek_redo()
            } else {
                history.peek_undo()
            }
        };
        if preview
            .as_ref()
            .is_some_and(|restore| !preflight_restore(world, restore))
        {
            continue;
        }
        let restore = {
            let mut history = world.resource_mut::<UiHistory>();
            if redo { history.redo() } else { history.undo() }
        };
        if let Some(restore) = restore
            && !apply_restore(world, restore)
        {
            // A directory can disappear between the preview and the actual
            // restore. Put the transaction back on its original stack.
            let mut history = world.resource_mut::<UiHistory>();
            let _ = if redo { history.undo() } else { history.redo() };
        }
    }
}

fn preflight_restore(world: &mut World, restore: &UiHistoryRestore) -> bool {
    let known_entities = world.resource::<WidgetHistoryCursor>().entities.clone();
    for (key, value) in &restore.changes {
        let entity = known_entities
            .get(key)
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok())
            .or_else(|| find_entity_by_key(world, key));
        let Some(entity) = entity else { continue };
        if world.get::<PanelGroup>(entity).is_some() {
            let result = match value {
                UiValue::Bytes(bytes) => serde_json::from_slice::<Vec<String>>(bytes)
                    .map_err(|error| error.to_string())
                    .and_then(|order| crate::panels::validate_panel_order(world, entity, &order)),
                _ => Err("panel order history value is not a snapshot".to_owned()),
            };
            if let Err(reason) = result {
                if let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>() {
                    events.0.push(WidgetEvent::PanelOrderRestoreFailed {
                        group: entity,
                        reason,
                    });
                }
                return false;
            }
            continue;
        }
        let Some(browser) = world.get::<FileBrowser>(entity) else {
            continue;
        };
        let path = browser.directory.clone();
        let mut candidate = browser.clone();
        let result = match value {
            UiValue::Bytes(bytes) => std::str::from_utf8(bytes)
                .map_err(|error| (path.clone(), error.to_string()))
                .and_then(|json| {
                    candidate.restore_json(json).map_err(|error| {
                        let message = error.to_string();
                        (error.path, message)
                    })
                }),
            _ => Err((path, "browser history value is not a snapshot".to_owned())),
        };
        if let Err((path, message)) = result {
            if let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>() {
                events.0.push(WidgetEvent::FileBrowserRestoreFailed {
                    target: entity,
                    path,
                    message,
                });
            }
            return false;
        }
    }
    true
}

fn apply_restore(world: &mut World, restore: UiHistoryRestore) -> bool {
    let known_entities = world.resource::<WidgetHistoryCursor>().entities.clone();
    let mut panel_changed = false;
    let mut success = true;
    let mut text_targets = Vec::new();
    for (key, value) in restore.changes {
        let entity = known_entities
            .get(&key)
            .copied()
            .filter(|entity| world.get_entity(*entity).is_ok())
            .or_else(|| find_entity_by_key(world, &key));
        if let Some(entity) = entity {
            panel_changed |= world.get::<Panel>(entity).is_some();
            if matches!(&value, UiValue::Text(_)) {
                text_targets.push(entity);
            }
            success &= apply_value(world, entity, value);
        }
    }
    if !success {
        return false;
    }
    if panel_changed {
        refresh_panel_history_state(world);
    }
    restore_context(world, &restore.context, &known_entities);
    for target in text_targets.iter().copied() {
        crate::actions::refresh_text_edit_draft_after_history_restore(world, target);
    }
    if !text_targets.is_empty() {
        crate::actions::sync_text_controls(world);
    }
    true
}

fn refresh_panel_history_state(world: &mut World) {
    let panels: Vec<_> = world
        .query::<(Entity, &Panel)>()
        .iter(world)
        .map(|(entity, panel)| {
            (
                entity,
                panel.persistence_key.clone(),
                panel.title.clone(),
                panel.exclusive,
                panel.state.target_expanded,
            )
        })
        .collect();
    if let Some(mut store) = world.get_resource_mut::<PanelExpansionStore>() {
        for (_, key, _, _, expanded) in &panels {
            if let Some(key) = key {
                store.remember_key(key.clone(), *expanded);
            }
        }
    }
    let groups: Vec<_> = world
        .query::<(Entity, &PanelGroup)>()
        .iter(world)
        .map(|(entity, group)| (entity, group.panel_keys.clone()))
        .collect();
    for (group_entity, keys) in groups {
        let mut open = None;
        let mut count = 0;
        for (entity, key, title, exclusive, expanded) in &panels {
            if !exclusive || !expanded || !is_descendant_of(world, *entity, group_entity) {
                continue;
            }
            let key = key.as_ref().unwrap_or(title);
            if keys.iter().any(|candidate| candidate == key) {
                open = Some(key.clone());
                count += 1;
            }
        }
        if let Some(mut group) = world.get_mut::<PanelGroup>(group_entity) {
            group.exclusive_open = (count == 1).then_some(open).flatten();
        }
    }
}

fn is_descendant_of(world: &World, entity: Entity, ancestor: Entity) -> bool {
    let mut current = world.get::<UiParent>(entity).map(|parent| parent.0);
    for _ in 0..1024 {
        let Some(at) = current else { return false };
        if at == ancestor {
            return true;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    false
}

fn apply_value(world: &mut World, entity: Entity, value: UiValue) -> bool {
    match value {
        UiValue::Bool(value) => {
            if let Some(mut checkbox) = world.get_mut::<Checkbox>(entity) {
                checkbox.value = if value {
                    CheckboxValue::On
                } else {
                    CheckboxValue::Off
                };
            } else if let Some(mut panel) = world.get_mut::<Panel>(entity) {
                panel.state.set_expanded(value);
            }
        }
        UiValue::Integer(value) => {
            if let Some(mut checkbox) = world.get_mut::<Checkbox>(entity) {
                checkbox.value = match value {
                    1 => CheckboxValue::On,
                    2 => CheckboxValue::Mixed,
                    _ => CheckboxValue::Off,
                };
            }
        }
        UiValue::Float(value) => {
            if value.is_finite() {
                if let Some(mut field) = world.get_mut::<NumericField>(entity) {
                    let _ = field.restore_value(value);
                } else if let Some(mut slider) = world.get_mut::<Slider>(entity) {
                    let _ = slider.field.restore_value(value);
                }
            }
        }
        UiValue::Numbers(values) => {
            if let Some(value) = values.first().copied()
                && value.is_finite()
            {
                if let Some(mut field) = world.get_mut::<NumericField>(entity) {
                    let _ = field.restore_value(value);
                } else if let Some(mut slider) = world.get_mut::<Slider>(entity) {
                    let _ = slider.field.restore_value(value);
                }
            }
        }
        UiValue::OptionalIndex(value) => {
            if let Some(mut dropdown) = world.get_mut::<Dropdown>(entity) {
                dropdown.selected = value.filter(|index| *index < dropdown.options.len());
                dropdown.highlighted = dropdown.selected;
            } else if let Some(mut radio) = world.get_mut::<RadioGroup>(entity) {
                radio.selected = value.filter(|index| *index < radio.options.len());
            }
        }
        UiValue::Bytes(bytes) => {
            if world.get::<PanelGroup>(entity).is_some() {
                return serde_json::from_slice::<Vec<String>>(&bytes)
                    .ok()
                    .is_some_and(|order| {
                        crate::panels::restore_panel_order(world, entity, &order)
                    });
            }
            if let Ok(json) = String::from_utf8(bytes) {
                if let Some(mut browser) = world.get_mut::<FileBrowser>(entity) {
                    let restored = browser.restore_json(&json);
                    let success = restored.is_ok();
                    if let Err(error) = restored
                        && let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>()
                    {
                        let message = error.to_string();
                        events.0.push(WidgetEvent::FileBrowserRestoreFailed {
                            target: entity,
                            path: error.path,
                            message,
                        });
                    }
                    return success;
                }
                if let Ok(ramp) = ColorRamp::from_json(&json)
                    && let Some(mut current) = world.get_mut::<ColorRamp>(entity)
                {
                    *current = ramp;
                } else if let Ok(color) = HsvColorEditor::from_json(&json)
                    && let Some(mut current) = world.get_mut::<HsvColorEditor>(entity)
                {
                    *current = color;
                } else if let Ok(curve) = CurveEditor::from_json(&json)
                    && let Some(mut current) = world.get_mut::<CurveEditor>(entity)
                {
                    *current = curve;
                } else if let Ok(list) = ListModel::<String>::from_json(&json)
                    && let Some(mut current) = world.get_mut::<ListModel<String>>(entity)
                {
                    *current = list;
                } else if let Ok(tree) = TreeModel::<String>::from_json(&json)
                    && let Some(mut current) = world.get_mut::<TreeModel<String>>(entity)
                {
                    *current = tree;
                } else if let Ok(grid) = PreviewGrid::from_json(&json)
                    && let Some(mut current) = world.get_mut::<PreviewGrid>(entity)
                {
                    *current = grid;
                } else if let Ok(table) = DataTable::from_json(&json)
                    && let Some(mut current) = world.get_mut::<DataTable>(entity)
                {
                    *current = table;
                } else if let Ok(canvas) = Canvas::from_json(&json)
                    && let Some(mut current) = world.get_mut::<Canvas>(entity)
                {
                    *current = canvas;
                } else if let Ok(graph) = NodeGraph::from_json(&json)
                    && let Some(mut current) = world.get_mut::<NodeGraph>(entity)
                {
                    *current = graph;
                }
            }
        }
        UiValue::Text(text) => {
            if let Some(mut buffer) = world.get_mut::<TextBuffer>(entity) {
                let (anchor, caret) = (buffer.anchor(), buffer.caret());
                buffer.restore_committed_text(text, anchor, caret);
            } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(entity) {
                let (anchor, caret) = (buffer.0.anchor(), buffer.0.caret());
                buffer.0.restore_committed_text(text, anchor, caret);
            } else {
                return false;
            }
        }
        UiValue::Color(_) => {}
    }
    true
}

fn restore_context(
    world: &mut World,
    context: &UiEditContext,
    known_entities: &std::collections::HashMap<String, Entity>,
) {
    let Some(window) = context.focused_window else {
        return;
    };
    if world.get_entity(window).is_err() {
        return;
    }
    let focused = context
        .focused_key
        .as_deref()
        .and_then(|key| {
            known_entities
                .get(key)
                .copied()
                .or_else(|| find_entity_by_key(world, key))
        })
        .filter(|entity| {
            world.get_entity(*entity).is_ok()
                && world.get::<UiFocusable>(*entity).is_some()
                && is_interactable_in_window(world, *entity, window)
        });
    let selection = focused
        .and(context.focused_key.as_ref())
        .and_then(|key| context.selections.get(key))
        .copied();
    if let Some(mut focus) = world.get_resource_mut::<UiFocus>() {
        let state = focus.state_mut(window);
        state.focused = focused;
        state.selection = selection.map(|selection| UiSelectionSnapshot {
            anchor: selection.anchor,
            active: selection.caret,
        });
        focus.set_active(window);
    }
    if let (Some(entity), Some(selection)) = (focused, selection) {
        if let Some(mut buffer) = world.get_mut::<TextBuffer>(entity) {
            buffer.set_selection(selection.anchor, selection.caret);
        } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(entity) {
            buffer.0.set_selection(selection.anchor, selection.caret);
        }
    }
}

fn capture_context(world: &World) -> UiEditContext {
    let Some(focus) = world.get_resource::<UiFocus>() else {
        return UiEditContext::default();
    };
    let window = focus.active_window;
    let focused = window
        .and_then(|window| focus.state(window))
        .and_then(|state| state.focused);
    let focused_key = focused.map(|entity| history_key(world, entity));
    let mut context = UiEditContext {
        focused_window: window,
        focused_key: focused_key.clone(),
        selections: Default::default(),
    };
    if let Some(entity) = focused
        && let Some(key) = focused_key
    {
        let selection = if let Some(buffer) = world.get::<TextBuffer>(entity) {
            Some(TextSelection {
                anchor: buffer.anchor(),
                caret: buffer.caret(),
            })
        } else if let Some(buffer) = world.get::<UiTextBuffer>(entity) {
            Some(TextSelection {
                anchor: buffer.0.anchor(),
                caret: buffer.0.caret(),
            })
        } else {
            window
                .and_then(|window| focus.state(window))
                .and_then(|state| state.selection)
                .map(|selection| TextSelection {
                    anchor: selection.anchor,
                    caret: selection.active,
                })
        };
        if let Some(selection) = selection {
            context.selections.insert(key, selection);
        }
    }
    context
}

fn history_key(world: &World, entity: Entity) -> String {
    world
        .get::<UiKey>(entity)
        .map(|key| format!("ui-key:{}", key.0))
        .unwrap_or_else(|| format!("entity:{:016x}", entity.to_bits()))
}

fn find_entity_by_key(world: &mut World, key: &str) -> Option<Entity> {
    let wanted = key.strip_prefix("ui-key:")?;
    let mut query = world.query::<(Entity, &UiKey)>();
    query
        .iter(world)
        .find_map(|(entity, key)| (key.0 == wanted).then_some(entity))
}

fn widget_value(value: WidgetValue) -> Option<UiValue> {
    match value {
        WidgetValue::Bool(value) => Some(UiValue::Bool(value)),
        WidgetValue::Number(value) => Some(UiValue::Float(value)),
        WidgetValue::Option(value) => Some(UiValue::OptionalIndex(value)),
        WidgetValue::Text => None,
    }
}

fn checkbox_value(value: CheckboxValue) -> UiValue {
    UiValue::Integer(match value {
        CheckboxValue::Off => 0,
        CheckboxValue::On => 1,
        CheckboxValue::Mixed => 2,
    })
}

/// Used by tests and applications to attach a stable identity to an edit.
pub fn widget_history_key(world: &World, entity: Entity) -> String {
    history_key(world, entity)
}

/// Attach marker components used to decide whether local text history or
/// application history receives the next undo command.
pub fn is_text_edit_target(world: &World, entity: Entity) -> bool {
    world.get::<TextBuffer>(entity).is_some()
        || world.get::<UiTextBuffer>(entity).is_some()
        || world.get::<UiExternalClipboard>(entity).is_some()
        || world.get::<UiReadOnly>(entity).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileView;
    use crate::actions::CheckboxPaintChange;
    use bui_core::{
        UiChildren, UiFocusable, UiKeyCode, UiModifiers, UiNode, UiRoutedEvents, set_parent,
    };
    use bui_text::{UiTextUndoFallback, process_text_events};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "bui-history-{label}-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        // Keep expectations aligned with FileBrowser's canonical directory.
        fs::canonicalize(path).unwrap()
    }

    fn setup() -> World {
        let mut world = World::new();
        world.init_resource::<UiHistory>();
        world.init_resource::<WidgetHistoryCursor>();
        world.init_resource::<WidgetEventQueue>();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<UiFocus>();
        world.init_resource::<UiTextUndoFallback>();
        world.init_resource::<PanelExpansionStore>();
        world.init_resource::<crate::PanelOrderStore>();
        world
    }

    fn key_event(
        window: Entity,
        target: Entity,
        key: bui_core::UiKeyCode,
        shift: bool,
    ) -> bui_core::UiRoutedEvent {
        bui_core::UiRoutedEvent::Key {
            window,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                shift,
                ..UiModifiers::default()
            },
        }
    }

    #[test]
    fn discrete_widget_edits_are_committed_and_undo_redo_applies_values() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let target = world
            .spawn((
                UiNode,
                UiFocusable,
                Checkbox::new("enabled", false),
                UiKey("enabled".into()),
            ))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
        world.resource_mut::<UiFocus>().set_active(window);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Bool(false),
                after: WidgetValue::Bool(true),
            });

        record_widget_edits(&mut world);
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(
            world.get::<Checkbox>(target).unwrap().value,
            CheckboxValue::Off
        );
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().focused,
            Some(target)
        );

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(
            world.get::<Checkbox>(target).unwrap().value,
            CheckboxValue::On
        );
    }

    #[test]
    fn exclusive_panel_expansion_undoes_as_one_atomic_group() {
        let mut world = setup();
        let group = world
            .spawn(PanelGroup::with_panels(["open", "closed"]))
            .id();
        let mut first = Panel::new("Open", true);
        first.exclusive = true;
        first.persistence_key = Some("open".into());
        let open = world.spawn((first, UiParent(group))).id();
        let mut second = Panel::new("Closed", false);
        second.exclusive = true;
        second.persistence_key = Some("closed".into());
        let closed = world.spawn((second, UiParent(group))).id();
        world
            .get_mut::<Panel>(open)
            .unwrap()
            .state
            .set_expanded(false);
        world
            .get_mut::<Panel>(closed)
            .unwrap()
            .state
            .set_expanded(true);
        world.get_mut::<PanelGroup>(group).unwrap().exclusive_open = Some("closed".into());
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::PanelGroupCommitted {
                changes: vec![(open, true, false), (closed, false, true)],
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        let undo = world.resource_mut::<UiHistory>().undo().unwrap();
        apply_restore(&mut world, undo);
        assert!(world.get::<Panel>(open).unwrap().state.target_expanded);
        assert!(!world.get::<Panel>(closed).unwrap().state.target_expanded);
        assert_eq!(
            world
                .get::<PanelGroup>(group)
                .unwrap()
                .exclusive_open
                .as_deref(),
            Some("open")
        );
        assert_eq!(
            world.resource::<PanelExpansionStore>().get("open"),
            Some(true)
        );
        assert_eq!(
            world.resource::<PanelExpansionStore>().get("closed"),
            Some(false)
        );

        let redo = world.resource_mut::<UiHistory>().redo().unwrap();
        apply_restore(&mut world, redo);
        assert!(!world.get::<Panel>(open).unwrap().state.target_expanded);
        assert!(world.get::<Panel>(closed).unwrap().state.target_expanded);
        assert_eq!(
            world
                .get::<PanelGroup>(group)
                .unwrap()
                .exclusive_open
                .as_deref(),
            Some("closed")
        );
    }

    #[test]
    fn panel_order_history_restores_sibling_order_and_keeps_invalid_step() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let group = world
            .spawn((
                UiNode,
                UiKey("settings".into()),
                PanelGroup::with_panels(["alpha", "beta"]).with_reordering("settings"),
            ))
            .id();
        let mut alpha = Panel::new("Alpha", true);
        alpha.persistence_key = Some("alpha".into());
        let alpha = world.spawn((UiNode, alpha)).id();
        let mut beta = Panel::new("Beta", true);
        beta.persistence_key = Some("beta".into());
        let beta = world.spawn((UiNode, beta)).id();
        set_parent(&mut world, alpha, group).unwrap();
        set_parent(&mut world, beta, group).unwrap();
        let before = vec!["alpha".to_owned(), "beta".to_owned()];
        let after = vec!["beta".to_owned(), "alpha".to_owned()];
        assert!(crate::panels::restore_panel_order(
            &mut world, group, &after
        ));
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::PanelOrderCommitted {
                group,
                persistence_key: "settings".into(),
                before: before.clone(),
                after: after.clone(),
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, alpha, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<PanelGroup>(group).unwrap().panel_keys, before);
        assert_eq!(world.get::<UiChildren>(group).unwrap().0, [alpha, beta]);
        assert_eq!(
            world.resource::<crate::PanelOrderStore>().get("settings"),
            Some(before.as_slice())
        );

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, alpha, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<PanelGroup>(group).unwrap().panel_keys, after);
        assert_eq!(world.get::<UiChildren>(group).unwrap().0, [beta, alpha]);

        world.get_mut::<Panel>(beta).unwrap().persistence_key = Some("changed".into());
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, alpha, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
        assert_eq!(world.get::<UiChildren>(group).unwrap().0, [beta, alpha]);
        assert!(
            world
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .any(|event| matches!(
                    event,
                    WidgetEvent::PanelOrderRestoreFailed { group: target, .. } if *target == group
                ))
        );
    }

    #[test]
    fn browser_navigation_bookmark_and_view_restore_in_one_transaction() {
        let root = temporary_directory("browser-roundtrip");
        let child = root.join("child");
        fs::create_dir(&child).unwrap();
        let mut browser = FileBrowser::new(&root).unwrap();
        let before = browser.snapshot_json().unwrap();
        browser.bookmark(&root).unwrap();
        browser.navigate(&child).unwrap();
        browser.view = FileView::Grid;
        let after = browser.snapshot_json().unwrap();

        let mut world = setup();
        let target = world.spawn(browser).id();
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::FileBrowserCommitted {
                target,
                before,
                after,
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        let undo = world.resource_mut::<UiHistory>().undo().unwrap();
        apply_restore(&mut world, undo);
        let browser = world.get::<FileBrowser>(target).unwrap();
        assert_eq!(browser.directory, root);
        assert!(browser.bookmarks.is_empty());
        assert_eq!(browser.view, FileView::List);

        let redo = world.resource_mut::<UiHistory>().redo().unwrap();
        apply_restore(&mut world, redo);
        let browser = world.get::<FileBrowser>(target).unwrap();
        assert_eq!(browser.directory, child);
        assert_eq!(browser.bookmarks, vec![root.clone()]);
        assert_eq!(browser.view, FileView::Grid);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn browser_restore_failure_preserves_current_state_and_reports_the_path() {
        let root = temporary_directory("browser-missing");
        let original = root.join("original");
        let current = root.join("current");
        fs::create_dir(&original).unwrap();
        fs::create_dir(&current).unwrap();
        let mut browser = FileBrowser::new(&original).unwrap();
        let before = browser.snapshot_json().unwrap();
        browser.navigate(&current).unwrap();
        let after = browser.snapshot_json().unwrap();

        let mut world = setup();
        let target = world.spawn(browser).id();
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::FileBrowserCommitted {
                target,
                before,
                after,
            });
        record_widget_edits(&mut world);
        fs::remove_dir(&original).unwrap();
        let window = world.spawn_empty().id();
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<FileBrowser>(target).unwrap().directory, current);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
        assert_eq!(world.resource::<UiHistory>().redo_len(), 0);
        assert!(world.resource::<WidgetEventQueue>().0.iter().any(|event| {
            matches!(event, WidgetEvent::FileBrowserRestoreFailed { target: reported, path, .. }
                if *reported == target && *path == original)
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn undo_redo_preserves_backward_text_selection_direction() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let checkbox = world
            .spawn((Checkbox::new("enabled", true), UiKey("enabled".into())))
            .id();
        let text = world
            .spawn((
                UiNode,
                UiFocusable,
                bui_core::UiHitBox {
                    window: Some(window),
                    rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                    clip: None,
                    z_index: 0,
                },
                TextBuffer::new("abcdef"),
                UiKey("text".into()),
            ))
            .id();
        world
            .get_mut::<TextBuffer>(text)
            .unwrap()
            .set_selection(5, 2);
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(text);
        world.resource_mut::<UiFocus>().set_active(window);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::ValueChanged {
                target: checkbox,
                before: WidgetValue::Bool(false),
                after: WidgetValue::Bool(true),
            });
        record_widget_edits(&mut world);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, checkbox, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        let buffer = world.get::<TextBuffer>(text).unwrap();
        assert_eq!((buffer.anchor(), buffer.caret()), (5, 2));

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, checkbox, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        let buffer = world.get::<TextBuffer>(text).unwrap();
        assert_eq!((buffer.anchor(), buffer.caret()), (5, 2));
    }

    #[test]
    fn committed_text_undoes_globally_after_focus_departure_and_restores_selection() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let text = world
            .spawn((
                UiNode,
                UiFocusable,
                bui_core::UiHitBox {
                    window: Some(window),
                    rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                    clip: None,
                    z_index: 0,
                },
                TextBuffer::new("hello!"),
                crate::TextField::new("hello!"),
                UiKey("name".into()),
            ))
            .id();
        let other = world
            .spawn((UiNode, UiFocusable, crate::Button::new("Apply")))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(other);
        world.resource_mut::<UiFocus>().set_active(window);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::TextCommitted {
                target: text,
                window,
                before: "hello".into(),
                after: "hello!".into(),
                before_selection: TextSelection {
                    anchor: 5,
                    caret: 2,
                },
                after_selection: TextSelection {
                    anchor: 6,
                    caret: 6,
                },
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, other, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        let buffer = world.get::<TextBuffer>(text).unwrap();
        assert_eq!(buffer.as_str(), "hello");
        assert_eq!((buffer.anchor(), buffer.caret()), (5, 2));
        assert_eq!(buffer.undo_len(), 0);
        assert_eq!(world.get::<crate::TextField>(text).unwrap().text, "hello");
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().focused,
            Some(text)
        );

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, text, UiKeyCode::Z, true)];
        process_text_events(&mut world);
        process_widget_undo_shortcuts(&mut world);
        let buffer = world.get::<TextBuffer>(text).unwrap();
        assert_eq!(buffer.as_str(), "hello!");
        assert_eq!((buffer.anchor(), buffer.caret()), (6, 6));
        assert_eq!(world.get::<crate::TextField>(text).unwrap().text, "hello!");
    }

    #[test]
    fn undo_applies_surviving_values_without_restoring_hidden_focus() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let checkbox = world
            .spawn((
                UiNode,
                UiFocusable,
                Checkbox::new("enabled", true),
                UiKey("enabled".into()),
            ))
            .id();
        let text = world
            .spawn((
                UiNode,
                UiFocusable,
                bui_core::UiHitBox {
                    window: Some(window),
                    rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                    clip: None,
                    z_index: 0,
                },
                TextBuffer::new("selection"),
                UiKey("text".into()),
            ))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(text);
        world.resource_mut::<UiFocus>().set_active(window);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::ValueChanged {
                target: checkbox,
                before: WidgetValue::Bool(false),
                after: WidgetValue::Bool(true),
            });
        record_widget_edits(&mut world);
        world.entity_mut(text).insert(bui_core::UiVisible(false));

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, checkbox, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);

        assert_eq!(
            world.get::<Checkbox>(checkbox).unwrap().value,
            CheckboxValue::Off
        );
        let focus = world.resource::<UiFocus>().state(window).unwrap();
        assert_eq!(focus.focused, None);
        assert_eq!(focus.selection, None);
    }

    #[test]
    fn undo_does_not_restore_focus_to_disabled_or_reassigned_controls() {
        for reassign_window in [false, true] {
            let mut world = setup();
            let window = world.spawn_empty().id();
            let other_window = world.spawn_empty().id();
            let checkbox = world
                .spawn((Checkbox::new("enabled", true), UiKey("enabled".into())))
                .id();
            let text = world
                .spawn((
                    UiNode,
                    UiFocusable,
                    bui_core::UiHitBox {
                        window: Some(window),
                        rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                        clip: None,
                        z_index: 0,
                    },
                    TextBuffer::new("focus"),
                    UiKey("text".into()),
                ))
                .id();
            world.resource_mut::<UiFocus>().state_mut(window).focused = Some(text);
            world.resource_mut::<UiFocus>().set_active(window);
            world
                .resource_mut::<WidgetEventQueue>()
                .0
                .push(WidgetEvent::ValueChanged {
                    target: checkbox,
                    before: WidgetValue::Bool(false),
                    after: WidgetValue::Bool(true),
                });
            record_widget_edits(&mut world);

            if reassign_window {
                world.get_mut::<bui_core::UiHitBox>(text).unwrap().window = Some(other_window);
            } else {
                world.entity_mut(text).insert(bui_core::UiEnabled(false));
            }
            let undo = world.resource_mut::<UiHistory>().undo().unwrap();
            assert!(apply_restore(&mut world, undo));
            assert_eq!(
                world.resource::<UiFocus>().state(window).unwrap().focused,
                None
            );
            assert_eq!(
                world.get::<Checkbox>(checkbox).unwrap().value,
                CheckboxValue::Off
            );
        }
    }

    #[test]
    fn data_table_edits_restore_sort_selection_and_width_as_one_transaction() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let mut before = DataTable {
            columns: vec![crate::TableColumn::new("Name", 100.0)],
            rows: vec![
                crate::TableRow {
                    id: 1,
                    cells: vec!["zeta".into()],
                },
                crate::TableRow {
                    id: 2,
                    cells: vec!["alpha".into()],
                },
            ],
            ..DataTable::default()
        };
        let target = world
            .spawn((UiNode, UiFocusable, UiKey("table".into()), before.clone()))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
        world.resource_mut::<UiFocus>().set_active(window);
        let before_json = before.to_json().unwrap();
        before.resize_column(0, 180.0);
        before.sort_by(0, true);
        before.select_cell(2, 0, false, false);
        let after_json = before.to_json().unwrap();
        world.entity_mut(target).insert(before);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::DataTableCommitted {
                target,
                before: before_json,
                after: after_json,
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        let restored = world.get::<DataTable>(target).unwrap();
        assert_eq!(restored.columns[0].width, 100.0);
        assert_eq!(
            restored.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(restored.selection.active, None);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        let restored = world.get::<DataTable>(target).unwrap();
        assert_eq!(restored.columns[0].width, 180.0);
        assert_eq!(
            restored.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [2, 1]
        );
        assert_eq!(restored.selection.active, Some(2));
    }

    #[test]
    fn canvas_move_and_selection_restore_as_one_transaction() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let mut canvas = Canvas::default();
        let id = canvas
            .add(
                [10.0, 20.0],
                crate::CanvasShape::Rectangle { size: [30.0, 40.0] },
                [1.0; 4],
            )
            .unwrap();
        let target = world
            .spawn((UiNode, UiFocusable, UiKey("canvas".into()), canvas.clone()))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
        world.resource_mut::<UiFocus>().set_active(window);
        let before = canvas.to_json().unwrap();
        canvas.selected.insert(id);
        assert!(canvas.move_selected([7.0, -4.0]));
        let after = canvas.to_json().unwrap();
        world.entity_mut(target).insert(canvas);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::CanvasCommitted {
                target,
                before,
                after,
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        let restored = world.get::<Canvas>(target).unwrap();
        assert_eq!(restored.objects[0].origin, [10.0, 20.0]);
        assert!(restored.selected.is_empty());

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        let restored = world.get::<Canvas>(target).unwrap();
        assert_eq!(restored.objects[0].origin, [17.0, 16.0]);
        assert_eq!(restored.selected, std::collections::BTreeSet::from([id]));
    }

    #[test]
    fn node_graph_move_restores_one_transaction() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let mut graph = NodeGraph::default();
        let id = graph.add_node("Source", [10.0, 20.0], vec![]);
        let target = world
            .spawn((UiNode, UiFocusable, UiKey("graph".into()), graph.clone()))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
        world.resource_mut::<UiFocus>().set_active(window);
        let before = graph.to_json().unwrap();
        assert!(graph.select(id, false));
        assert!(graph.move_selected_by([15.0, -5.0]));
        let after = graph.to_json().unwrap();
        world.entity_mut(target).insert(graph);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::NodeGraphCommitted {
                target,
                before,
                after,
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        let graph = world.get::<NodeGraph>(target).unwrap();
        assert_eq!(graph.nodes[0].position, [10.0, 20.0]);
        assert!(graph.selected.is_empty());

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, target, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        let graph = world.get::<NodeGraph>(target).unwrap();
        assert_eq!(graph.nodes[0].position, [25.0, 15.0]);
        assert!(graph.selected.contains(&id));
    }

    #[test]
    fn painted_checkboxes_and_numeric_scrubs_each_restore_as_one_gesture() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let first = world.spawn(Checkbox::new("a", true)).id();
        let second = world.spawn(Checkbox::new("b", true)).id();
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::CheckboxPaintCommitted {
                changes: vec![
                    CheckboxPaintChange {
                        entity: first,
                        before: CheckboxValue::Off,
                        after: CheckboxValue::On,
                    },
                    CheckboxPaintChange {
                        entity: second,
                        before: CheckboxValue::Off,
                        after: CheckboxValue::On,
                    },
                ],
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, first, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(
            world.get::<Checkbox>(first).unwrap().value,
            CheckboxValue::Off
        );
        assert_eq!(
            world.get::<Checkbox>(second).unwrap().value,
            CheckboxValue::Off
        );

        let numeric = world.spawn(NumericField::float(4.0).unwrap()).id();
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::NumericCommitted {
                target: numeric,
                before: vec![1.0, 2.0],
                after: vec![4.0, 5.0],
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, numeric, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<NumericField>(numeric).unwrap().value, 1.0);
    }

    #[test]
    fn numeric_group_scrub_restores_all_fields_in_one_transaction() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let first = world.spawn(NumericField::float(4.0).unwrap()).id();
        let second = world.spawn(NumericField::float(12.0).unwrap()).id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(first);
        world.resource_mut::<UiFocus>().set_active(window);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::NumericGroupCommitted {
                changes: vec![(first, 1.0, 4.0), (second, 10.0, 12.0)],
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, first, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<NumericField>(first).unwrap().value, 1.0);
        assert_eq!(world.get::<NumericField>(second).unwrap().value, 10.0);

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, first, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<NumericField>(first).unwrap().value, 4.0);
        assert_eq!(world.get::<NumericField>(second).unwrap().value, 12.0);
    }

    #[test]
    fn slider_drag_undo_redo_restores_value_even_after_becoming_read_only() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let slider = world.spawn(Slider::new(7.0, 0.0, 10.0, 0.5).unwrap()).id();
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::NumericCommitted {
                target: slider,
                before: vec![4.0],
                after: vec![7.0],
            });
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
        world.get_mut::<Slider>(slider).unwrap().read_only = true;
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, slider, UiKeyCode::Z, false)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<Slider>(slider).unwrap().value, 4.0);
        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, slider, UiKeyCode::Z, true)];
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<Slider>(slider).unwrap().value, 7.0);
    }

    #[test]
    fn draining_the_widget_event_queue_does_not_lose_the_next_equal_sized_batch() {
        let mut world = setup();
        let target = world.spawn(Checkbox::new("enabled", false)).id();
        let event = WidgetEvent::ValueChanged {
            target,
            before: WidgetValue::Bool(false),
            after: WidgetValue::Bool(true),
        };
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(event.clone());
        record_widget_edits(&mut world);
        assert_eq!(world.resource_mut::<WidgetEventQueue>().take().len(), 1);
        world.resource_mut::<WidgetEventQueue>().0.push(event);
        record_widget_edits(&mut world);
        assert_eq!(world.resource::<UiHistory>().undo_len(), 2);
    }

    #[test]
    fn a_text_buffer_undoes_locally_then_falls_back_to_widget_history() {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let checkbox = world.spawn(Checkbox::new("enabled", true)).id();
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::ValueChanged {
                target: checkbox,
                before: WidgetValue::Bool(false),
                after: WidgetValue::Bool(true),
            });
        record_widget_edits(&mut world);
        let text = world.spawn(TextBuffer::new("a")).id();
        world.get_mut::<TextBuffer>(text).unwrap().insert_text("b");

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, text, UiKeyCode::Z, false)];
        process_text_events(&mut world);
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(world.get::<TextBuffer>(text).unwrap().as_str(), "a");
        assert_eq!(
            world.get::<Checkbox>(checkbox).unwrap().value,
            CheckboxValue::On
        );

        world.resource_mut::<UiRoutedEvents>().events =
            vec![key_event(window, text, UiKeyCode::Z, false)];
        process_text_events(&mut world);
        assert!(
            world
                .resource::<UiTextUndoFallback>()
                .needs_global_undo(text)
        );
        process_widget_undo_shortcuts(&mut world);
        assert_eq!(
            world.get::<Checkbox>(checkbox).unwrap().value,
            CheckboxValue::Off
        );
    }
}

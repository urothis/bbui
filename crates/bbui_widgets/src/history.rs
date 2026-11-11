//! Transaction adapter between widget events and BBUI's shared undo history.

use bbui_core::{
    TextSelection, UiEditContext, UiFocus, UiFocusable, UiHistory, UiHistoryRestore, UiKey,
    UiParent, UiSelectionSnapshot, UiValue, is_interactable_in_window,
};
use bbui_text::{TextBuffer, UiExternalClipboard, UiReadOnly, UiTextBuffer, UiTextUndoFallback};
use bevy_ecs::prelude::*;

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
        .get_resource::<bbui_core::UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    for event in events {
        let bbui_core::UiRoutedEvent::Key {
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
            || !matches!(key, bbui_core::UiKeyCode::Z | bbui_core::UiKeyCode::Y)
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
        let redo = key == bbui_core::UiKeyCode::Y || modifiers.shift;
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
#[path = "history/tests.rs"]
mod tests;

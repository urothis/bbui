use super::*;

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

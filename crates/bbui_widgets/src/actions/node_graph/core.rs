use super::*;

pub(in crate::actions) fn edit_node_graph(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut NodeGraph) -> bool,
) {
    let Some(owner) = node_graph_owner(world, target) else {
        return;
    };
    let Some(before) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<NodeGraph>(owner)
        .is_some_and(|mut graph| edit(&mut graph));
    if !changed {
        return;
    }
    let Some(after) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    emit_node_graph_commit(owner, before, after, emitted);
}

pub(in crate::actions) fn edit_node_graph_result(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut NodeGraph) -> Result<bool, NodeGraphError>,
) {
    let Some(owner) = node_graph_owner(world, target) else {
        return;
    };
    let Some(before) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    let result = world
        .get_mut::<NodeGraph>(owner)
        .map(|mut graph| edit(&mut graph));
    match result {
        Some(Ok(true)) => {
            if let Some(after) = world
                .get::<NodeGraph>(owner)
                .and_then(|graph| graph.to_json().ok())
            {
                emit_node_graph_commit(owner, before, after, emitted);
            }
        }
        Some(Err(error)) => emitted.push(WidgetEvent::NodeGraphLinkFailed {
            target: owner,
            error,
        }),
        Some(Ok(false)) | None => {}
    }
}

pub(in crate::actions) fn emit_node_graph_commit(
    target: Entity,
    before: String,
    after: String,
    emitted: &mut Vec<WidgetEvent>,
) {
    if before != after {
        emitted.push(WidgetEvent::NodeGraphCommitted {
            target,
            before,
            after,
        });
    }
}

pub(in crate::actions) fn has_annotated_node_graph_down(
    world: &World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
) -> bool {
    world
        .get_resource::<UiRoutedEvents>()
        .is_some_and(|events| {
            events.events.iter().any(|event| {
                matches!(event, UiRoutedEvent::PointerDownWithModifiers {
                    window: annotated_window,
                    target: annotated_target,
                    button: annotated_button,
                    position: annotated_position,
                    ..
                } if *annotated_window == window
                    && *annotated_target == target
                    && *annotated_button == button
                    && *annotated_position == position)
            })
        })
}

pub(in crate::actions) fn process_node_graph_event(
    world: &mut World,
    event: &UiRoutedEvent,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    match event {
        UiRoutedEvent::PointerDown {
            window,
            target,
            button,
            position,
        } if let Some(owner) = node_graph_owner(world, *target) => {
            if !has_annotated_node_graph_down(world, *window, *target, *button, *position) {
                begin_node_graph_gesture(world, *window, owner, *button, *position, None, emitted);
            }
            true
        }
        UiRoutedEvent::PointerDownWithModifiers {
            window,
            target,
            button,
            position,
            modifiers,
        } if let Some(owner) = node_graph_owner(world, *target) => {
            begin_node_graph_gesture(
                world,
                *window,
                owner,
                *button,
                *position,
                Some(*modifiers),
                emitted,
            );
            true
        }
        UiRoutedEvent::PointerMoved {
            target,
            position,
            dragging: true,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            update_node_graph_gesture(world, owner, *position);
            true
        }
        UiRoutedEvent::PointerUp {
            target,
            position,
            cancelled,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            finish_node_graph_gesture(world, owner, *position, *cancelled, emitted);
            true
        }
        UiRoutedEvent::PointerCancel { target, .. }
            if let Some(owner) = node_graph_owner(world, *target) =>
        {
            finish_node_graph_gesture(world, owner, [0.0, 0.0], true, emitted);
            true
        }
        UiRoutedEvent::Click { target, .. } if node_graph_owner(world, *target).is_some() => true,
        UiRoutedEvent::ClickWithModifiers { target, .. }
            if node_graph_owner(world, *target).is_some() =>
        {
            true
        }
        UiRoutedEvent::Wheel {
            target,
            position,
            delta,
            unit,
            modifiers,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            node_graph_wheel(world, owner, *position, *delta, *unit, *modifiers, emitted);
            true
        }
        UiRoutedEvent::Key {
            target,
            key,
            pressed: true,
            modifiers,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            node_graph_key(world, owner, *key, *modifiers, emitted);
            true
        }
        _ => false,
    }
}

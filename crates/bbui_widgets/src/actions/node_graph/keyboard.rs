use super::*;

pub(in crate::actions) fn node_graph_wheel(
    world: &mut World,
    target: Entity,
    pointer: [f32; 2],
    delta: [f32; 2],
    unit: bbui_core::UiWheelUnit,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    let tracked_pan = !modifiers.control && !modifiers.super_key;
    let trusted = tracked_pan && node_graph_pan_tracking_is_current(world, target);
    edit_node_graph(world, target, emitted, |graph| {
        if modifiers.control || modifiers.super_key {
            let anchor = node_graph_data_position(graph, rect, pointer, spacing);
            let scale = if unit == bbui_core::UiWheelUnit::Lines {
                1.0
            } else {
                0.25
            };
            let before = graph.zoom;
            graph.zoom_by((delta[1] * scale * 0.12).exp());
            if graph.zoom == before {
                return false;
            }
            graph.pan = [
                pointer[0] - rect.x - anchor[0] * graph.zoom,
                pointer[1] - rect.y - spacing.header_height - anchor[1] * graph.zoom,
            ];
        } else {
            let scale = if unit == bbui_core::UiWheelUnit::Lines {
                36.0
            } else {
                1.0
            };
            graph.pan_by([delta[0] * scale, delta[1] * scale]);
        }
        true
    });
    if tracked_pan {
        record_node_graph_pan(world, target, trusted);
    }
}

pub(in crate::actions) fn node_graph_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    match key {
        UiKeyCode::Escape => {
            finish_node_graph_gesture(world, target, [0.0, 0.0], true, emitted);
        }
        UiKeyCode::A if modifiers.control || modifiers.super_key => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphSelectAll),
                emitted,
            );
        }
        UiKeyCode::Delete | UiKeyCode::Backspace | UiKeyCode::X => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphDeleteSelection),
                emitted,
            );
        }
        UiKeyCode::C => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphToggleCollapse),
                emitted,
            );
        }
        UiKeyCode::N => {
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let position = world
                .get::<UiLayoutResult>(target)
                .zip(world.get::<NodeGraph>(target))
                .map(|(layout, graph)| {
                    node_graph_data_position(
                        graph,
                        layout.rect,
                        [
                            layout.rect.x + layout.rect.width * 0.5,
                            layout.rect.y + spacing.header_height + 80.0,
                        ],
                        spacing,
                    )
                })
                .unwrap_or([0.0, 0.0]);
            apply_action(
                world,
                WidgetAction::new(
                    target,
                    WidgetActionKind::NodeGraphAddNode {
                        title: "Node".to_owned(),
                        position,
                        sockets: vec![
                            NodeSocket::input("Input", crate::SocketType::Any),
                            NodeSocket::output("Output", crate::SocketType::Any),
                        ],
                    },
                ),
                emitted,
            );
        }
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowDown => {
            if modifiers.control || modifiers.super_key {
                let factor = match key {
                    UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => 1.15,
                    UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => 1.0 / 1.15,
                    _ => return,
                };
                apply_action(
                    world,
                    WidgetAction::new(target, WidgetActionKind::NodeGraphZoomBy(factor)),
                    emitted,
                );
            } else {
                let step = if modifiers.shift { 40.0 } else { 10.0 };
                let delta = match key {
                    UiKeyCode::ArrowLeft => [-step, 0.0],
                    UiKeyCode::ArrowRight => [step, 0.0],
                    UiKeyCode::ArrowUp => [0.0, -step],
                    UiKeyCode::ArrowDown => [0.0, step],
                    _ => return,
                };
                apply_action(
                    world,
                    WidgetAction::new(target, WidgetActionKind::NodeGraphMoveSelected(delta)),
                    emitted,
                );
            }
        }
        UiKeyCode::Home => {
            edit_node_graph(world, target, emitted, |graph| {
                let changed = graph.pan != [0.0, 0.0] || graph.zoom != 1.0;
                graph.pan = [0.0, 0.0];
                graph.zoom = 1.0;
                changed
            });
        }
        _ => {}
    }
}

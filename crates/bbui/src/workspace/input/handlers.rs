use super::*;

struct DropRequest {
    pane: PaneId,
    source: String,
    destination: Option<DropLocation>,
    window: Entity,
    position: [f32; 2],
    grid: bool,
}

fn commit_drop(world: &World, workspace: &mut Workspace, request: DropRequest) {
    let operation = match request.destination {
        Some(location) if location.edge == DropEdge::Center && request.grid => {
            WorkspaceOperation::Grid {
                pane: request.pane,
                target: location.group,
                id: workspace.next_key("grid"),
                group: workspace.next_key("group"),
                columns: 2,
            }
        }
        Some(location) if location.edge == DropEdge::Center => {
            let index = insertion_index(
                world,
                workspace,
                request.window,
                request.position,
                &location.group,
            );
            WorkspaceOperation::Move {
                pane: request.pane,
                group: location.group,
                index,
            }
        }
        Some(location) => {
            if location.group == request.source
                && workspace
                    .layout
                    .group(&request.source)
                    .is_some_and(|group| group.panes.len() == 1)
            {
                return;
            }
            let (axis, after) = match location.edge {
                DropEdge::Left => (SplitAxis::Horizontal, false),
                DropEdge::Right => (SplitAxis::Horizontal, true),
                DropEdge::Top => (SplitAxis::Vertical, false),
                DropEdge::Bottom => (SplitAxis::Vertical, true),
                DropEdge::Center => return,
            };
            WorkspaceOperation::Split {
                pane: request.pane,
                target: location.group,
                id: workspace.next_key("split"),
                group: workspace.next_key("group"),
                axis,
                after,
            }
        }
        None => WorkspaceOperation::Detach {
            pane: request.pane,
            window: workspace.next_key("window"),
            group: workspace.next_key("group"),
        },
    };
    let _ = workspace.apply(operation);
}

pub(crate) fn process_workspace_input(world: &mut World) {
    if !world.contains_resource::<Workspace>() {
        return;
    }
    let mut events = world
        .get_resource::<UiRoutedEvents>()
        .map(|routed| routed.events.clone())
        .unwrap_or_default();
    let cross_events =
        std::mem::take(&mut world.resource_mut::<WorkspaceGesture>().cross_window_events);
    events.extend(cross_events);
    let grid_drop = events.iter().any(|event| matches!(event, UiRoutedEvent::PointerUpWithModifiers { modifiers, .. } if modifiers.shift));
    world.resource_scope(|world, mut gesture: Mut<WorkspaceGesture>| {
        world.resource_scope(|world, mut workspace: Mut<Workspace>| {
            for event in events {
                match event {
                    UiRoutedEvent::PointerDown {
                        window,
                        target,
                        button: UiPointerButton::Primary,
                        position,
                        ..
                    } => {
                        gesture.active = pointer_down(world, &workspace, window, target, position);
                        gesture.hover = None;
                    }
                    UiRoutedEvent::PointerMoved {
                        window, position, ..
                    } => {
                        if let Some(active) = &mut gesture.active {
                            match active {
                                Gesture::Tab {
                                    start, dragging, ..
                                } => {
                                    if (position[0] - start[0]).hypot(position[1] - start[1]) > 6.0
                                    {
                                        *dragging = true;
                                    }
                                    if *dragging {
                                        let next =
                                            drop_location(world, &workspace, window, position);
                                        if gesture
                                            .hover
                                            .as_ref()
                                            .map(|value| (&value.group, value.edge))
                                            != next.as_ref().map(|value| (&value.group, value.edge))
                                        {
                                            gesture.hover = next;
                                            workspace.dirty = true;
                                        }
                                    }
                                }
                                Gesture::Splitter(splitter) => {
                                    resize_split(world, &mut workspace, splitter, position);
                                }
                            }
                        }
                    }
                    UiRoutedEvent::PointerUp {
                        window,
                        position,
                        button: UiPointerButton::Primary,
                        cancelled,
                        ..
                    } => {
                        if let Some(active) = gesture.active.take() {
                            if cancelled {
                                if let Gesture::Splitter(splitter) = active {
                                    workspace.cancel_preview(splitter.before);
                                }
                            } else {
                                match active {
                                    Gesture::Tab {
                                        pane,
                                        group,
                                        dragging,
                                        ..
                                    } => {
                                        if dragging {
                                            let destination =
                                                drop_location(world, &workspace, window, position);
                                            commit_drop(
                                                world,
                                                &mut workspace,
                                                DropRequest {
                                                    pane,
                                                    source: group,
                                                    destination,
                                                    window,
                                                    position,
                                                    grid: grid_drop,
                                                },
                                            );
                                        } else {
                                            let _ = workspace
                                                .apply(WorkspaceOperation::Select { group, pane });
                                        }
                                    }
                                    Gesture::Splitter(splitter) => {
                                        workspace.finish_preview(splitter.before)
                                    }
                                }
                            }
                        }
                        gesture.hover = None;
                        workspace.dirty = true;
                    }
                    UiRoutedEvent::Click {
                        target,
                        button: UiPointerButton::Primary,
                        ..
                    } => {
                        if let Some(WorkspaceTarget::Close { pane }) =
                            workspace.targets.get(&target).cloned()
                        {
                            let _ = workspace.apply(WorkspaceOperation::Close { pane });
                        }
                    }
                    UiRoutedEvent::Key {
                        target,
                        key: UiKeyCode::Escape,
                        pressed: true,
                        ..
                    } => {
                        let _ = target;
                        if let Some(Gesture::Splitter(splitter)) = gesture.active.take() {
                            workspace.cancel_preview(splitter.before);
                        }
                        gesture.hover = None;
                        workspace.dirty = true;
                    }
                    UiRoutedEvent::Key {
                        target,
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if (modifiers.control || modifiers.super_key)
                        && matches!(key, UiKeyCode::Z | UiKeyCode::Y)
                        && workspace.targets.contains_key(&target) =>
                    {
                        if key == UiKeyCode::Y || modifiers.shift {
                            workspace.redo();
                        } else {
                            workspace.undo();
                        }
                    }
                    UiRoutedEvent::Key {
                        target,
                        key,
                        pressed: true,
                        ..
                    } => match workspace.targets.get(&target).cloned() {
                        Some(WorkspaceTarget::Tab { group, pane }) => {
                            if matches!(key, UiKeyCode::Enter | UiKeyCode::Space) {
                                let _ = workspace.apply(WorkspaceOperation::Select { group, pane });
                            } else if matches!(key, UiKeyCode::Delete) {
                                let _ = workspace.apply(WorkspaceOperation::Close { pane });
                            } else if matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
                                && let Some(group_view) = workspace.layout.group(&group)
                            {
                                let current = group_view
                                    .panes
                                    .iter()
                                    .position(|id| id == &pane)
                                    .unwrap_or(0);
                                let next = if key == UiKeyCode::ArrowLeft {
                                    current.saturating_sub(1)
                                } else {
                                    (current + 1).min(group_view.panes.len() - 1)
                                };
                                let pane = group_view.panes[next].clone();
                                let _ = workspace.apply(WorkspaceOperation::Select { group, pane });
                            }
                        }
                        Some(WorkspaceTarget::Close { pane })
                            if matches!(key, UiKeyCode::Enter | UiKeyCode::Space) =>
                        {
                            let _ = workspace.apply(WorkspaceOperation::Close { pane });
                        }
                        Some(WorkspaceTarget::Splitter { id, index })
                            if matches!(
                                key,
                                UiKeyCode::ArrowLeft
                                    | UiKeyCode::ArrowRight
                                    | UiKeyCode::ArrowUp
                                    | UiKeyCode::ArrowDown
                            ) =>
                        {
                            if let Some((_, shares)) = find_split(&workspace.layout, &id)
                                && index + 1 < shares.len()
                            {
                                let mut next = shares.to_vec();
                                let signed: f32 =
                                    if matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowUp) {
                                        -0.1
                                    } else {
                                        0.1
                                    };
                                let change = signed.clamp(0.1 - next[index], next[index + 1] - 0.1);
                                next[index] += change;
                                next[index + 1] -= change;
                                let _ = workspace.apply(WorkspaceOperation::Resize {
                                    split: id,
                                    shares: next,
                                });
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        });
    });
}

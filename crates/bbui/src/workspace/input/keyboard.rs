//! Keyboard docking uses the same validated operations as pointer docking.

use super::*;
use bbui_core::{UiDirty, UiDirtyDomain};

#[derive(Clone)]
enum KeyboardTarget {
    Reorder { group: String, index: usize },
    Drop(DropLocation),
}

pub(super) struct KeyboardDock {
    pane: PaneId,
    tab: Entity,
    targets: Vec<KeyboardTarget>,
    selected: usize,
}

fn groups<'a>(node: &'a DockNode, result: &mut Vec<&'a bbui_widgets::TabGroup>) {
    match node {
        DockNode::Tabs(group) => result.push(group),
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
            for child in children {
                groups(child, result);
            }
        }
    }
}

fn candidates(workspace: &Workspace, source: &str, pane: &PaneId) -> Vec<KeyboardTarget> {
    let Some(source_group) = workspace.layout.group(source) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    if source_group.options.reorderable {
        let current = source_group.panes.iter().position(|id| id == pane);
        for index in 0..source_group.panes.len() {
            if Some(index) != current {
                targets.push(KeyboardTarget::Reorder {
                    group: source.to_owned(),
                    index,
                });
            }
        }
    }
    if !source_group.options.movable {
        return targets;
    }
    let mut all_groups = Vec::new();
    groups(&workspace.layout.root, &mut all_groups);
    for group in &all_groups {
        if !group.options.accepts_drops {
            continue;
        }
        if group.id != source {
            targets.push(KeyboardTarget::Drop(DropLocation {
                group: group.id.clone(),
                edge: DropEdge::Center,
            }));
        }
    }
    for group in all_groups {
        if !group.options.accepts_drops || (group.id == source && group.panes.len() == 1) {
            continue;
        }
        for edge in [
            DropEdge::Left,
            DropEdge::Right,
            DropEdge::Top,
            DropEdge::Bottom,
        ] {
            targets.push(KeyboardTarget::Drop(DropLocation {
                group: group.id.clone(),
                edge,
            }));
        }
    }
    targets
}

fn description(target: &KeyboardTarget, selected: usize, total: usize) -> String {
    let destination = match target {
        KeyboardTarget::Reorder { group, index } => {
            format!("position {} in {group}", index + 1)
        }
        KeyboardTarget::Drop(DropLocation { group, edge }) => {
            let edge = match edge {
                DropEdge::Center => "inside",
                DropEdge::Left => "left of",
                DropEdge::Right => "right of",
                DropEdge::Top => "above",
                DropEdge::Bottom => "below",
            };
            format!("{edge} group {group}")
        }
    };
    format!(
        "Move target {} of {}: {}. Arrows choose, Enter docks, Escape cancels.",
        selected + 1,
        total,
        destination
    )
}

fn show_target(world: &mut World, workspace: &mut Workspace, gesture: &mut WorkspaceGesture) {
    let Some(dock) = gesture.keyboard.as_ref() else {
        return;
    };
    let target = &dock.targets[dock.selected];
    gesture.hover = match target {
        KeyboardTarget::Reorder { group, .. } => Some(DropLocation {
            group: group.clone(),
            edge: DropEdge::Center,
        }),
        KeyboardTarget::Drop(location) => Some(location.clone()),
    };
    if world.get_entity(dock.tab).is_ok() {
        world
            .entity_mut(dock.tab)
            .insert(WorkspaceDockSemantic(description(
                target,
                dock.selected,
                dock.targets.len(),
            )));
        world
            .resource_mut::<UiDirty>()
            .mark(UiDirtyDomain::Accessibility, dock.tab);
    }
    workspace.dirty = true;
}

pub(super) fn cancel(world: &mut World, workspace: &mut Workspace, gesture: &mut WorkspaceGesture) {
    if let Some(dock) = gesture.keyboard.take()
        && world.get_entity(dock.tab).is_ok()
    {
        world.entity_mut(dock.tab).insert(WorkspaceDockSemantic(
            "Ctrl+Space to move this tab".to_owned(),
        ));
        world
            .resource_mut::<UiDirty>()
            .mark(UiDirtyDomain::Accessibility, dock.tab);
    }
    gesture.hover = None;
    workspace.dirty = true;
}

fn commit(world: &mut World, workspace: &mut Workspace, gesture: &mut WorkspaceGesture) {
    let Some(dock) = gesture.keyboard.as_ref() else {
        return;
    };
    let target = dock.targets[dock.selected].clone();
    let pane = dock.pane.clone();
    let operation = match target {
        KeyboardTarget::Reorder { group, index } => {
            let old = workspace
                .layout
                .group(&group)
                .and_then(|tabs| tabs.panes.iter().position(|id| id == &pane))
                .unwrap_or(index);
            WorkspaceOperation::Move {
                pane,
                group,
                index: index + usize::from(old < index),
            }
        }
        KeyboardTarget::Drop(DropLocation {
            group,
            edge: DropEdge::Center,
        }) => WorkspaceOperation::Move {
            pane,
            group,
            index: usize::MAX,
        },
        KeyboardTarget::Drop(DropLocation { group, edge }) => {
            let Some((axis, after)) = (match edge {
                DropEdge::Left => Some((SplitAxis::Horizontal, false)),
                DropEdge::Right => Some((SplitAxis::Horizontal, true)),
                DropEdge::Top => Some((SplitAxis::Vertical, false)),
                DropEdge::Bottom => Some((SplitAxis::Vertical, true)),
                DropEdge::Center => None,
            }) else {
                cancel(world, workspace, gesture);
                return;
            };
            WorkspaceOperation::Split {
                pane,
                target: group,
                id: workspace.next_key("split"),
                group: workspace.next_key("group"),
                axis,
                after,
            }
        }
    };
    cancel(world, workspace, gesture);
    let _ = workspace.apply(operation);
}

pub(super) fn handle(
    world: &mut World,
    workspace: &mut Workspace,
    gesture: &mut WorkspaceGesture,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
) -> bool {
    if gesture.keyboard.is_some() {
        match key {
            UiKeyCode::ArrowRight | UiKeyCode::ArrowDown => {
                let Some(dock) = gesture.keyboard.as_mut() else {
                    return false;
                };
                dock.selected = (dock.selected + 1) % dock.targets.len();
                show_target(world, workspace, gesture);
            }
            UiKeyCode::ArrowLeft | UiKeyCode::ArrowUp => {
                let Some(dock) = gesture.keyboard.as_mut() else {
                    return false;
                };
                dock.selected = (dock.selected + dock.targets.len() - 1) % dock.targets.len();
                show_target(world, workspace, gesture);
            }
            UiKeyCode::Enter => commit(world, workspace, gesture),
            UiKeyCode::Escape => cancel(world, workspace, gesture),
            _ => return false,
        }
        return true;
    }
    if key != UiKeyCode::Space || !(modifiers.control || modifiers.super_key) {
        return false;
    }
    let Some(WorkspaceTarget::Tab { group, pane }) = workspace.targets.get(&target) else {
        return false;
    };
    let targets = candidates(workspace, group, pane);
    if targets.is_empty() {
        return true;
    }
    gesture.keyboard = Some(KeyboardDock {
        pane: pane.clone(),
        tab: target,
        targets,
        selected: 0,
    });
    show_target(world, workspace, gesture);
    true
}

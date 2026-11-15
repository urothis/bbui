//! Translate completed layout edits into app-visible widget notifications.

use super::*;
use bbui_widgets::DockNode;
use std::collections::BTreeMap;

fn pane_group<'a>(node: &'a DockNode, pane: &PaneId) -> Option<&'a str> {
    match node {
        DockNode::Tabs(group) => group.panes.contains(pane).then_some(group.id.as_str()),
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
            children.iter().find_map(|child| pane_group(child, pane))
        }
    }
}

pub(super) fn events_for_operation(
    operation: &WorkspaceOperation,
    before: &WorkspaceLayout,
    after: &WorkspaceLayout,
) -> Vec<WorkspaceEvent> {
    let primary = match operation {
        WorkspaceOperation::Open { pane, group, .. } => Some(WorkspaceEvent::PaneOpened {
            pane: pane.clone(),
            group: group.clone(),
            index: after
                .group(group)
                .and_then(|tabs| tabs.panes.iter().position(|id| id == pane))
                .unwrap_or(0),
        }),
        WorkspaceOperation::Select { .. } => None,
        WorkspaceOperation::Close { pane } => Some(WorkspaceEvent::PaneClosed {
            pane: pane.clone(),
            group: pane_group(&before.root, pane)
                .unwrap_or_default()
                .to_owned(),
        }),
        WorkspaceOperation::Move { pane, group, .. } => {
            let from = pane_group(&before.root, pane).unwrap_or_default();
            let index = after
                .group(group)
                .and_then(|tabs| tabs.panes.iter().position(|id| id == pane))
                .unwrap_or(0);
            if from == group {
                if before.group(group).is_some_and(|tabs| {
                    after
                        .group(group)
                        .is_some_and(|next| tabs.panes == next.panes)
                }) {
                    return selection_changes(before, after);
                } else {
                    Some(WorkspaceEvent::TabReordered {
                        pane: pane.clone(),
                        group: group.clone(),
                        index,
                    })
                }
            } else {
                Some(WorkspaceEvent::PaneMoved {
                    pane: pane.clone(),
                    from: from.to_owned(),
                    to: group.clone(),
                    index,
                })
            }
        }
        WorkspaceOperation::Split {
            pane,
            target,
            group,
            axis,
            after,
            ..
        } => Some(WorkspaceEvent::PaneDocked {
            pane: pane.clone(),
            target: target.clone(),
            group: group.clone(),
            axis: *axis,
            after: *after,
        }),
        WorkspaceOperation::Grid {
            pane,
            target,
            group,
            columns,
            ..
        } => Some(WorkspaceEvent::PaneGridded {
            pane: pane.clone(),
            target: target.clone(),
            group: group.clone(),
            columns: *columns,
        }),
        WorkspaceOperation::Resize { split, shares } => Some(WorkspaceEvent::SplitResized {
            split: split.clone(),
            shares: shares.clone(),
        }),
        WorkspaceOperation::SetGroupOptions { group, options } => {
            Some(WorkspaceEvent::GroupOptionsChanged {
                group: group.clone(),
                options: *options,
            })
        }
    };
    let mut events = primary.into_iter().collect::<Vec<_>>();
    events.extend(selection_changes(before, after));
    events
}

pub(super) fn selection_changes(
    before: &WorkspaceLayout,
    after: &WorkspaceLayout,
) -> Vec<WorkspaceEvent> {
    fn collect(node: &DockNode, groups: &mut BTreeMap<String, Option<PaneId>>) {
        match node {
            DockNode::Tabs(group) => {
                groups.insert(group.id.clone(), group.active.clone());
            }
            DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
                for child in children {
                    collect(child, groups);
                }
            }
        }
    }
    let mut old = BTreeMap::new();
    let mut new = BTreeMap::new();
    collect(&before.root, &mut old);
    collect(&after.root, &mut new);
    let mut groups: Vec<String> = old.keys().chain(new.keys()).cloned().collect();
    groups.sort();
    groups.dedup();
    groups
        .into_iter()
        .filter_map(|group| {
            let previous = old.get(&group).cloned().flatten();
            let current = new.get(&group).cloned().flatten();
            (previous != current).then_some(WorkspaceEvent::TabSelectionChanged {
                group,
                previous,
                current,
            })
        })
        .collect()
}

pub(super) fn changed_split(
    before: &WorkspaceLayout,
    after: &WorkspaceLayout,
) -> Option<(String, Vec<f32>)> {
    fn visit(node: &DockNode, after: &WorkspaceLayout) -> Option<(String, Vec<f32>)> {
        match node {
            DockNode::Tabs(_) => None,
            DockNode::Grid { children, .. } => {
                children.iter().find_map(|child| visit(child, after))
            }
            DockNode::Split {
                id,
                shares,
                children,
                ..
            } => {
                if let Some(DockNode::Split { shares: next, .. }) = find_node(&after.root, id)
                    && shares != next
                {
                    return Some((id.clone(), next.clone()));
                }
                children.iter().find_map(|child| visit(child, after))
            }
        }
    }
    visit(&before.root, after)
}

pub(super) fn find_node<'a>(node: &'a DockNode, id: &str) -> Option<&'a DockNode> {
    let own = match node {
        DockNode::Tabs(group) => &group.id,
        DockNode::Split { id, .. } | DockNode::Grid { id, .. } => id,
    };
    if own == id {
        return Some(node);
    }
    match node {
        DockNode::Tabs(_) => None,
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
            children.iter().find_map(|child| find_node(child, id))
        }
    }
}

pub(super) fn flush_workspace_events(world: &mut World) {
    if !world.contains_resource::<Workspace>()
        || !world.contains_resource::<Messages<WorkspaceEvent>>()
    {
        return;
    }
    world.resource_scope(|world, mut workspace: Mut<Workspace>| {
        let events = std::mem::take(&mut workspace.pending_events);
        let mut messages = world.resource_mut::<Messages<WorkspaceEvent>>();
        for event in events {
            messages.write(event);
        }
    });
}

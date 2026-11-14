use super::*;

fn split<'a>(node: &'a DockNode, id: &str) -> Option<(SplitAxis, &'a [f32])> {
    match node {
        DockNode::Tabs(_) => None,
        DockNode::Split {
            id: own,
            axis,
            shares,
            children,
        } => {
            if own == id {
                Some((*axis, shares))
            } else {
                children.iter().find_map(|child| split(child, id))
            }
        }
        DockNode::Grid { children, .. } => children.iter().find_map(|child| split(child, id)),
    }
}

pub(crate) fn find_split<'a>(
    layout: &'a WorkspaceLayout,
    id: &str,
) -> Option<(SplitAxis, &'a [f32])> {
    layout
        .windows
        .iter()
        .find_map(|window| split(&window.root, id))
}

pub(super) fn drop_location(
    world: &World,
    workspace: &Workspace,
    window: Entity,
    position: [f32; 2],
) -> Option<DropLocation> {
    let mut best: Option<(f32, DropLocation)> = None;
    for (entity, target) in &workspace.targets {
        let WorkspaceTarget::Group { group } = target else {
            continue;
        };
        let Some(rect) = world.get::<UiLayoutResult>(*entity) else {
            continue;
        };
        if rect.window != window
            || position[0] < rect.rect.x
            || position[0] > rect.rect.x + rect.rect.width
            || position[1] < rect.rect.y
            || position[1] > rect.rect.y + rect.rect.height
        {
            continue;
        }
        let x = (position[0] - rect.rect.x) / rect.rect.width.max(1.0);
        let y = (position[1] - rect.rect.y) / rect.rect.height.max(1.0);
        let over_tab = workspace.targets.iter().any(|(entity, target)| {
            matches!(target, WorkspaceTarget::Tab { group: candidate, .. } if candidate == group)
                && world.get::<UiLayoutResult>(*entity).is_some_and(|tab| {
                    tab.window == window
                        && position[0] >= tab.rect.x
                        && position[0] <= tab.rect.x + tab.rect.width
                        && position[1] >= tab.rect.y
                        && position[1] <= tab.rect.y + tab.rect.height
                })
        });
        let edge = if over_tab {
            DropEdge::Center
        } else if x < 0.22 {
            DropEdge::Left
        } else if x > 0.78 {
            DropEdge::Right
        } else if y < 0.22 {
            DropEdge::Top
        } else if y > 0.78 {
            DropEdge::Bottom
        } else {
            DropEdge::Center
        };
        let area = rect.rect.width * rect.rect.height;
        if best.as_ref().is_none_or(|(old, _)| area < *old) {
            best = Some((
                area,
                DropLocation {
                    group: group.clone(),
                    edge,
                },
            ));
        }
    }
    best.map(|(_, location)| location)
}

pub(super) fn pointer_down(
    world: &World,
    workspace: &Workspace,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) -> Option<Gesture> {
    match workspace.targets.get(&target)? {
        WorkspaceTarget::Tab { group, pane } => Some(Gesture::Tab {
            pane: pane.clone(),
            group: group.clone(),
            window,
            start: position,
            dragging: false,
        }),
        WorkspaceTarget::Splitter { id, index } => {
            let (axis, shares) = find_split(&workspace.layout, id)?;
            Some(Gesture::Splitter(SplitterGesture {
                id: id.clone(),
                index: *index,
                start: position,
                before: workspace.layout.clone(),
                shares: shares.to_vec(),
                axis,
            }))
        }
        _ => {
            let _ = world;
            None
        }
    }
}

pub(super) fn resize_split(
    world: &World,
    workspace: &mut Workspace,
    gesture: &SplitterGesture,
    position: [f32; 2],
) {
    let Some((entity, _)) = workspace.targets.iter().find(
        |(_, target)| matches!(target, WorkspaceTarget::Split { id: candidate } if candidate == &gesture.id),
    ) else {
        return;
    };
    let Some(rect) = world.get::<UiLayoutResult>(*entity) else {
        return;
    };
    let length = if gesture.axis == SplitAxis::Horizontal {
        rect.rect.width
    } else {
        rect.rect.height
    };
    let delta = if gesture.axis == SplitAxis::Horizontal {
        position[0] - gesture.start[0]
    } else {
        position[1] - gesture.start[1]
    };
    if length <= 0.0 || gesture.index + 1 >= gesture.shares.len() {
        return;
    }
    let mut next = gesture.shares.clone();
    let unit = gesture.shares.iter().sum::<f32>() / length;
    let change = (delta * unit).clamp(0.1 - next[gesture.index], next[gesture.index + 1] - 0.1);
    next[gesture.index] += change;
    next[gesture.index + 1] -= change;
    let _ = workspace.preview(WorkspaceOperation::Resize {
        split: gesture.id.clone(),
        shares: next,
    });
}

pub(super) fn insertion_index(
    world: &World,
    workspace: &Workspace,
    window: Entity,
    position: [f32; 2],
    group: &str,
) -> usize {
    let Some(group_view) = workspace.layout.group(group) else {
        return 0;
    };
    for (index, pane) in group_view.panes.iter().enumerate() {
        let Some((entity, _)) = workspace.targets.iter().find(|(_, target)| matches!(target, WorkspaceTarget::Tab { group: candidate, pane: candidate_pane } if candidate == group && candidate_pane == pane)) else { continue; };
        let Some(rect) = world.get::<UiLayoutResult>(*entity) else {
            continue;
        };
        if rect.window == window
            && position[0] >= rect.rect.x
            && position[0] <= rect.rect.x + rect.rect.width
            && position[1] >= rect.rect.y
            && position[1] <= rect.rect.y + rect.rect.height
        {
            return index + usize::from(position[0] > rect.rect.x + rect.rect.width * 0.5);
        }
    }
    group_view.panes.len()
}

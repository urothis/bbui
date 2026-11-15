use super::*;

pub(super) struct DropRequest {
    pub(super) pane: PaneId,
    pub(super) source: String,
    pub(super) destination: Option<DropLocation>,
    pub(super) window: Entity,
    pub(super) position: [f32; 2],
    pub(super) grid: bool,
}

pub(super) fn commit_drop(world: &World, workspace: &mut Workspace, request: DropRequest) {
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
        None => return,
    };
    let _ = workspace.apply(operation);
}

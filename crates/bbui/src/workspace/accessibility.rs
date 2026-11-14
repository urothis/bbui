//! Route AccessKit tab and splitter actions through workspace operations.

use super::*;
use accesskit::{Action, ActionData};
use bevy::a11y::{AccessibilityNode, ActionRequest};
use input::WorkspaceTarget;

pub(crate) fn route_workspace_accessibility_actions(
    mut requests: MessageReader<ActionRequest>,
    accessible: Query<&AccessibilityNode>,
    workspace: Option<ResMut<Workspace>>,
) {
    let Some(mut workspace) = workspace else {
        return;
    };
    for request in requests.read() {
        let Some(entity) = Entity::try_from_bits(request.target_node.0) else {
            continue;
        };
        let Ok(node) = accessible.get(entity) else {
            continue;
        };
        if node.0.is_disabled() || node.0.is_hidden() || !node.0.supports_action(request.action) {
            continue;
        }
        let Some(target) = workspace.targets.get(&entity).cloned() else {
            continue;
        };
        match target {
            WorkspaceTarget::Tab { group, pane } if request.action == Action::Click => {
                let _ = workspace.apply(WorkspaceOperation::Select { group, pane });
            }
            WorkspaceTarget::Close { pane } if request.action == Action::Click => {
                let _ = workspace.apply(WorkspaceOperation::Close { pane });
            }
            WorkspaceTarget::Splitter { id, index } => {
                let Some((_, shares)) = input::find_split(&workspace.layout, &id) else {
                    continue;
                };
                if index + 1 >= shares.len() {
                    continue;
                }
                let total = shares.iter().sum::<f32>();
                let current = shares[..=index].iter().sum::<f32>() / total;
                let desired = match request.action {
                    Action::Increment => current + 0.05,
                    Action::Decrement => current - 0.05,
                    Action::SetValue => match &request.data {
                        Some(ActionData::NumericValue(value)) => *value as f32 / 100.0,
                        _ => continue,
                    },
                    _ => continue,
                };
                let previous = shares[..index].iter().sum::<f32>() / total;
                let next = (shares[..=index + 1].iter().sum::<f32>() - 0.1) / total;
                let desired = desired.clamp(previous + 0.1 / total, next);
                let change = (desired - current) * total;
                let mut updated = shares.to_vec();
                updated[index] += change;
                updated[index + 1] -= change;
                let _ = workspace.apply(WorkspaceOperation::Resize {
                    split: id,
                    shares: updated,
                });
            }
            _ => {}
        }
    }
}

use super::*;

pub(super) fn despawn_tree(world: &mut World, entity: Entity) {
    let children = world
        .get::<UiChildren>(entity)
        .map(|children| children.0.clone())
        .unwrap_or_default();
    for child in children {
        despawn_tree(world, child);
    }
    if world.get_entity(entity).is_ok() {
        cleanup_removed_node(world, entity);
        world.despawn(entity);
    }
}

pub(super) fn keyed_entities(world: &mut World) -> HashMap<String, Entity> {
    let mut query = world.query::<(Entity, &UiKey)>();
    query
        .iter(world)
        .filter(|(_, key)| key.0.starts_with("ws:"))
        .map(|(entity, key)| (key.0.clone(), entity))
        .collect()
}

pub(super) fn open_panes(layout: &WorkspaceLayout) -> HashSet<PaneId> {
    fn collect(node: &DockNode, result: &mut HashSet<PaneId>) {
        match node {
            DockNode::Tabs(group) => result.extend(group.panes.iter().cloned()),
            DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
                for child in children {
                    collect(child, result);
                }
            }
        }
    }
    let mut result = HashSet::new();
    collect(&layout.root, &mut result);
    result
}

pub(super) fn pane_destinations(layout: &WorkspaceLayout) -> Vec<(PaneId, String, bool)> {
    fn collect(node: &DockNode, result: &mut Vec<(PaneId, String, bool)>) {
        match node {
            DockNode::Tabs(group) => {
                for pane in &group.panes {
                    result.push((
                        pane.clone(),
                        group.id.clone(),
                        group.active.as_ref() == Some(pane),
                    ));
                }
            }
            DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
                for child in children {
                    collect(child, result);
                }
            }
        }
    }
    let mut result = Vec::new();
    collect(&layout.root, &mut result);
    result
}

pub(super) fn collect_targets(
    node: &DockNode,
    keys: &HashMap<String, Entity>,
    targets: &mut HashMap<Entity, WorkspaceTarget>,
) {
    match node {
        DockNode::Tabs(group) => {
            if let Some(entity) = keys.get(&format!("ws:group:{}", group.id)) {
                targets.insert(
                    *entity,
                    WorkspaceTarget::Group {
                        group: group.id.clone(),
                    },
                );
            }
            for pane in &group.panes {
                if let Some(entity) = keys.get(&format!("ws:tab:{}:{}", group.id, pane.0)) {
                    targets.insert(
                        *entity,
                        WorkspaceTarget::Tab {
                            group: group.id.clone(),
                            pane: pane.clone(),
                        },
                    );
                }
                if let Some(entity) = keys.get(&format!("ws:close:{}", pane.0)) {
                    targets.insert(*entity, WorkspaceTarget::Close { pane: pane.clone() });
                }
            }
        }
        DockNode::Split { id, children, .. } => {
            if let Some(entity) = keys.get(&format!("ws:split:{id}")) {
                targets.insert(*entity, WorkspaceTarget::Split { id: id.clone() });
            }
            for index in 0..children.len().saturating_sub(1) {
                if let Some(entity) = keys.get(&format!("ws:splitter:{id}:{index}")) {
                    targets.insert(
                        *entity,
                        WorkspaceTarget::Splitter {
                            id: id.clone(),
                            index,
                        },
                    );
                }
            }
            for child in children {
                collect_targets(child, keys, targets);
            }
        }
        DockNode::Grid { children, .. } => {
            for child in children {
                collect_targets(child, keys, targets);
            }
        }
    }
}

pub(super) fn sync_preview(world: &mut World, workspace: &mut Workspace) {
    if let Some(old) = workspace.preview.take() {
        despawn_tree(world, old);
    }
    let hover = world
        .resource::<super::input::WorkspaceGesture>()
        .hover
        .clone();
    let Some(hover) = hover else {
        return;
    };
    let Some((entity, _)) = workspace.targets.iter().find(
        |(_, target)| matches!(target, WorkspaceTarget::Group { group } if group == &hover.group),
    ) else {
        return;
    };
    let Some(layout) = world.get::<bbui_layout::UiLayoutResult>(*entity) else {
        return;
    };
    let mut rect = layout.rect;
    match hover.edge {
        DropEdge::Center => {
            rect.x += rect.width * 0.2;
            rect.y += rect.height * 0.2;
            rect.width *= 0.6;
            rect.height *= 0.6;
        }
        DropEdge::Left => rect.width *= 0.28,
        DropEdge::Right => {
            rect.x += rect.width * 0.72;
            rect.width *= 0.28;
        }
        DropEdge::Top => rect.height *= 0.28,
        DropEdge::Bottom => {
            rect.y += rect.height * 0.72;
            rect.height *= 0.28;
        }
    }
    let color = world
        .resource::<Theme>()
        .color(workspace.appearance.drop_preview)
        .with_alpha(workspace.appearance.drop_preview_alpha.clamp(0.0, 1.0));
    let preview = world
        .spawn((
            UiNode,
            UiEnabled(false),
            UiLayoutRoot::new(layout.window, rect, layout.dpi_scale),
            UiLayoutNode {
                size: bbui_layout::UiSize {
                    width: UiLength::Fill(1.0),
                    height: UiLength::Fill(1.0),
                },
                z_index: 1000,
                ..default()
            },
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: 4.0,
                color,
            }),
        ))
        .id();
    workspace.preview = Some(preview);
}

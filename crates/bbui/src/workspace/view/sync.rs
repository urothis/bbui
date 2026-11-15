use super::*;

pub(crate) fn sync_workspace(world: &mut World) {
    if !world.contains_resource::<Workspace>() {
        return;
    }
    world.resource_scope(|world, mut workspace: Mut<Workspace>| {
        let background = world
            .resource::<Theme>()
            .color(workspace.appearance.group_background);
        if workspace.painted_background != Some(background) {
            workspace.painted_background = Some(background);
            workspace.dirty = true;
        }
        if !workspace.dirty {
            return;
        }

        let focused_workspace_target = workspace.window.and_then(|window| {
            world
                .get_resource::<UiFocus>()
                .and_then(|focus| focus.state(window))
                .and_then(|state| state.focused)
                .and_then(|entity| workspace.targets.get(&entity).cloned())
        });

        // Preserve pane entities while chrome is reconciled around them.
        for entity in workspace.live_panes.values().copied() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
            }
        }

        if workspace.window.is_none() {
            let mut query = world.query_filtered::<Entity, With<PrimaryWindow>>();
            let Some(window) = query.iter(world).next() else {
                return;
            };
            workspace.window = Some(window);
        }
        let Some(window) = workspace.window else {
            return;
        };
        let declaration = WidgetTree::new(chrome(&workspace.layout.root, &workspace));
        if let Some(root) = workspace.root {
            if declaration.reconcile(world, root).is_err() {
                return;
            }
        } else if let Ok(root) = declaration.spawn(world, None) {
            let dimensions = world
                .get::<Window>(window)
                .map(|window| [window.resolution.width(), window.resolution.height()])
                .unwrap_or([720.0, 480.0]);
            world.entity_mut(root).insert(UiLayoutRoot::new(
                window,
                Rect::new(0.0, 0.0, dimensions[0], dimensions[1]),
                1.0,
            ));
            workspace.root = Some(root);
        }

        let present = open_panes(&workspace.layout);
        let rebuild: Vec<PaneId> = workspace.rebuild_panes.drain().collect();
        for id in rebuild {
            if let Some(entity) = workspace.live_panes.remove(&id) {
                despawn_tree(world, entity);
            }
        }
        let removed: Vec<PaneId> = workspace
            .live_panes
            .keys()
            .filter(|id| !present.contains(*id))
            .cloned()
            .collect();
        for id in removed {
            if let Some(entity) = workspace.live_panes.remove(&id) {
                despawn_tree(world, entity);
            }
        }
        let keys = keyed_entities(world);
        let mut targets = HashMap::new();
        collect_targets(&workspace.layout.root, &keys, &mut targets);
        workspace.targets = targets;
        if let Some(target) = focused_workspace_target {
            let focused = match &target {
                WorkspaceTarget::Tab { group, pane } => Some((group.as_str(), pane)),
                WorkspaceTarget::Close { pane } => Some(("", pane)),
                _ => None,
            };
            if let Some((group, pane)) = focused {
                let fallback = workspace
                    .layout
                    .group(group)
                    .and_then(|tabs| tabs.active.as_ref())
                    .cloned()
                    .or_else(|| {
                        pane_destinations(&workspace.layout)
                            .into_iter()
                            .find(|(_, _, active)| *active)
                            .map(|(pane, _, _)| pane)
                    });
                let next = workspace.targets.iter().find_map(|(entity, target)| {
                    matches!(target, WorkspaceTarget::Tab { pane: candidate, .. } if candidate == pane)
                        .then_some(*entity)
                }).or_else(|| fallback.as_ref().and_then(|pane| workspace.targets.iter().find_map(|(entity, target)| {
                    matches!(target, WorkspaceTarget::Tab { pane: candidate, .. } if candidate == pane)
                        .then_some(*entity)
                })));
                let mut focus = world.resource_mut::<UiFocus>();
                focus.state_mut(window).focused = next;
                focus.focused = next;
            }
        }
        for (entity, target) in &workspace.targets {
            if matches!(target, WorkspaceTarget::Group { .. }) {
                world
                    .entity_mut(*entity)
                    .insert(UiPaint(Primitive::RoundedRect {
                        rect: ClipRect {
                            x: 0.0,
                            y: 0.0,
                            width: 0.0,
                            height: 0.0,
                        },
                        radius: 0.0,
                        color: background,
                    }));
            }
        }
        for (entity, target) in &workspace.targets {
            if matches!(target, WorkspaceTarget::Tab { .. })
                && world.get::<WorkspaceDockSemantic>(*entity).is_none()
            {
                world.entity_mut(*entity).insert(WorkspaceDockSemantic(
                    "Ctrl+Space to move this tab".to_owned(),
                ));
                world
                    .resource_mut::<UiDirty>()
                    .mark(UiDirtyDomain::Accessibility, *entity);
            }
            if matches!(
                target,
                WorkspaceTarget::Tab { .. }
                    | WorkspaceTarget::Close { .. }
                    | WorkspaceTarget::Splitter { .. }
            ) {
                world.entity_mut(*entity).insert(UiHistoryDelegate);
            }
            if let WorkspaceTarget::Splitter { id, index } = target
                && let Some((axis, shares)) = super::super::input::find_split(&workspace.layout, id)
                && *index + 1 < shares.len()
            {
                let total: f32 = shares.iter().sum();
                let semantic = WorkspaceSplitterSemantic {
                    id: id.clone(),
                    index: *index,
                    axis,
                    fraction: shares[..=*index].iter().sum::<f32>() / total,
                };
                if world.get::<WorkspaceSplitterSemantic>(*entity) != Some(&semantic) {
                    world.entity_mut(*entity).insert(semantic);
                    world
                        .resource_mut::<UiDirty>()
                        .mark(UiDirtyDomain::Accessibility, *entity);
                }
            }
        }
        sync_preview(world, &mut workspace);
        for (pane, group, active) in pane_destinations(&workspace.layout) {
            let Some(host) = keys.get(&format!("ws:content:{group}")).copied() else {
                continue;
            };
            let wrapper = if let Some(entity) = workspace.live_panes.get(&pane).copied() {
                entity
            } else {
                let entity = world
                    .spawn((
                        UiNode,
                        UiLayoutNode {
                            size: bbui_layout::UiSize {
                                width: UiLength::Fill(1.0),
                                height: UiLength::Fill(1.0),
                            },
                            ..default()
                        },
                        UiVisible(active),
                    ))
                    .id();
                if let Some(registration) = workspace.registrations.get(&pane) {
                    (registration.build)(world, entity);
                } else {
                    let placeholder = WidgetTree::new(WidgetNode::label(format!(
                        "Pane '{}' is unavailable",
                        pane.0
                    )));
                    let _ = placeholder.spawn(world, Some(entity));
                }
                workspace.live_panes.insert(pane.clone(), entity);
                entity
            };
            let _ = set_parent(world, wrapper, host);
            if world
                .get::<UiVisible>(wrapper)
                .is_none_or(|visible| visible.0 != active)
            {
                world.entity_mut(wrapper).insert(UiVisible(active));
            }
        }
        workspace.dirty = false;
    });
}

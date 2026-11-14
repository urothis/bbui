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
        let closed: Vec<String> = workspace
            .windows
            .iter()
            .filter(|(id, entity)| id.as_str() != "main" && world.get::<Window>(**entity).is_none())
            .map(|(id, _)| id.clone())
            .collect();
        for id in closed {
            let _ = workspace.apply(WorkspaceOperation::CloseWindow { window: id.clone() });
            workspace.windows.remove(&id);
            if let Some(camera) = workspace.cameras.remove(&id) {
                world.despawn(camera);
            }
            workspace.dirty = true;
        }
        if !workspace.dirty {
            return;
        }

        // Keep live pane wrappers out of any chrome subtree about to be reconciled or removed.
        let mut focus_moves = HashMap::new();
        for (pane, entity) in &workspace.live_panes {
            if let Some(old_window) = world
                .get::<bbui_layout::UiLayoutResult>(*entity)
                .map(|result| result.window)
                && let Some(focused) = world
                    .get_resource::<UiFocus>()
                    .and_then(|focus| focus.state(old_window))
                    .and_then(|state| state.focused)
                && is_below(world, focused, *entity)
            {
                focus_moves.insert(pane.clone(), (old_window, focused));
            }
        }
        for entity in workspace.live_panes.values().copied() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
            }
        }

        if !workspace.windows.contains_key("main") {
            let mut query = world.query_filtered::<Entity, With<PrimaryWindow>>();
            let Some(main) = query.iter(world).next() else {
                return;
            };
            workspace.windows.insert("main".into(), main);
        }
        let wanted: HashSet<String> = workspace
            .layout
            .windows
            .iter()
            .map(|window| window.id.clone())
            .collect();
        let obsolete: Vec<String> = workspace
            .roots
            .keys()
            .filter(|id| !wanted.contains(*id))
            .cloned()
            .collect();
        for id in obsolete {
            if let Some(root) = workspace.roots.remove(&id) {
                despawn_tree(world, root);
            }
            if let Some(camera) = workspace.cameras.remove(&id) {
                world.despawn(camera);
            }
            if let Some(window) = workspace.windows.remove(&id)
                && id != "main"
            {
                world.despawn(window);
            }
        }

        let entries: Vec<_> = workspace.layout.windows.clone();
        for entry in entries {
            let window = if let Some(window) = workspace.windows.get(&entry.id).copied() {
                window
            } else {
                let window = world
                    .spawn(Window {
                        title: format!("{} — BBUI", window_title(&workspace, &entry.root)),
                        resolution: WindowResolution::new(720, 480),
                        ..default()
                    })
                    .id();
                let camera = world
                    .spawn((
                        Camera2d,
                        Camera::default(),
                        RenderTarget::Window(WindowRef::Entity(window)),
                    ))
                    .id();
                workspace.windows.insert(entry.id.clone(), window);
                workspace.cameras.insert(entry.id.clone(), camera);
                window
            };
            let declaration = WidgetTree::new(chrome(&entry.root, &workspace));
            if let Some(root) = workspace.roots.get(&entry.id).copied() {
                if declaration.reconcile(world, root).is_err() {
                    continue;
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
                workspace.roots.insert(entry.id.clone(), root);
            }
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
        for window in &workspace.layout.windows {
            collect_targets(&window.root, &keys, &mut targets);
        }
        workspace.targets = targets;
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
        for (pane, group, active, window_id) in pane_destinations(&workspace.layout) {
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
            if active
                && let Some((old_window, focused)) = focus_moves.get(&pane).copied()
                && let Some(new_window) = workspace.windows.get(&window_id).copied()
                && old_window != new_window
            {
                let mut focus = world.resource_mut::<UiFocus>();
                let selection = focus.state_mut(old_window).selection.take();
                focus.state_mut(old_window).focused = None;
                focus.state_mut(new_window).focused = Some(focused);
                focus.state_mut(new_window).selection = selection;
                focus.focused = Some(focused);
                focus.active_window = Some(new_window);
            }
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

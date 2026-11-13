use super::*;

/// Keep collapsed panel children out of layout and hit testing.
pub fn sync_panel_children(world: &mut World) {
    world.init_resource::<WidgetPanelSyncCache>();
    world.init_resource::<WidgetPanelSyncInvalidations>();
    world.init_resource::<WidgetPanelSyncGate>();
    let Ok(changed_panels) = world.run_system_cached(changed_panels_for_sync) else {
        return;
    };
    let (needs_sync, all_panels) =
        world.resource_scope(|world, mut gate: Mut<WidgetPanelSyncGate>| {
            let now = world.read_change_tick();
            let pending = world.resource::<WidgetPanelSyncInvalidations>();
            let all_panels = !gate.initialized
                || world
                    .get_resource_change_ticks::<Theme>()
                    .is_some_and(|ticks| ticks.is_changed(gate.last, now));
            let changed = !gate.initialized
                || !changed_panels.is_empty()
                || !pending.panels.is_empty()
                || pending.topology_changed
                || all_panels;
            gate.initialized = true;
            gate.last = now;
            (changed, all_panels)
        });
    if !needs_sync {
        world.increment_change_tick();
        return;
    }
    let theme_changed = world
        .get_resource_change_ticks::<Theme>()
        .map(|ticks| ticks.changed.get());
    let mut cache = world
        .remove_resource::<WidgetPanelSyncCache>()
        .unwrap_or_default();
    let invalidations = std::mem::take(&mut *world.resource_mut::<WidgetPanelSyncInvalidations>());
    let mut panels: Vec<_> = if all_panels {
        world
            .query_filtered::<Entity, With<Panel>>()
            .iter(world)
            .collect()
    } else {
        changed_panels
            .into_iter()
            .chain(invalidations.panels.iter().copied())
            .collect()
    };
    panels.sort_unstable();
    panels.dedup();
    if invalidations.topology_changed {
        cache
            .stamps
            .retain(|panel, _| world.get::<Panel>(*panel).is_some());
        cache
            .clips
            .retain(|entity, _| world.get_entity(*entity).is_ok());
    }
    panels.sort_by_key(|panel| widget_hierarchy_depth(world, *panel));

    for panel in panels {
        let Some(stamp) = panel_sync_stamp(world, panel, theme_changed) else {
            continue;
        };
        let invalidated_descendant = invalidations.panels.contains(&panel);
        if cache.stamps.get(&panel) == Some(&stamp) && !invalidated_descendant {
            continue;
        }
        let Some((progress, expanded)) = world
            .get::<Panel>(panel)
            .map(|panel| (panel.state.progress, panel.state.target_expanded))
        else {
            continue;
        };
        sync_panel_reveal(world, panel, progress);
        let children = world
            .get::<bbui_core::UiChildren>(panel)
            .map_or_else(Vec::new, |children| children.0.clone());
        for child in children {
            if world.get::<crate::WidgetElement>(child).is_none() {
                continue;
            }
            set_visible(world, child, expanded || progress > 0.0);
        }
        apply_panel_clip(world, panel, &mut cache.clips);
        cache.stamps.insert(panel, stamp);
    }
    if invalidations.topology_changed {
        let detached: Vec<_> = cache
            .clips
            .keys()
            .copied()
            .filter(|entity| !has_panel_ancestor(world, *entity))
            .collect();
        for entity in detached {
            let Some(state) = cache.clips.remove(&entity) else {
                continue;
            };
            if let Some(mut layout) = world.get_mut::<UiLayoutResult>(entity)
                && state.applied_layout == Some(layout.clip)
                && layout.clip != state.base_layout
            {
                layout.clip = state.base_layout;
            }
            if let Some(mut hit) = world.get_mut::<UiHitBox>(entity)
                && state.applied_hit == Some(hit.clip)
                && hit.clip != state.base_hit
            {
                hit.clip = state.base_hit;
            }
        }
    }
    world.insert_resource(cache);
    bbui_core::clear_invalid_ui_focus(world);
    let now = world.read_change_tick();
    world.resource_mut::<WidgetPanelSyncGate>().last = now;
    world.increment_change_tick();
}

fn changed_panels_for_sync(query: Query<Entity, Changed<Panel>>) -> Vec<Entity> {
    query.iter().collect()
}

pub(super) fn panel_sync_stamp(
    world: &World,
    panel: Entity,
    theme_changed: Option<u32>,
) -> Option<PanelSyncStamp> {
    Some(PanelSyncStamp {
        panel_changed: entity_component_changed_tick::<Panel>(world, panel)?,
        children_changed: entity_component_changed_tick::<bbui_core::UiChildren>(world, panel),
        layout_changed: entity_component_changed_tick::<UiLayoutResult>(world, panel),
        theme_changed,
        progress_bits: world.get::<Panel>(panel)?.state.progress.to_bits(),
    })
}

pub(super) fn widget_hierarchy_depth(world: &World, mut entity: Entity) -> usize {
    let mut depth = 0;
    while let Some(parent) = world.get::<UiParent>(entity) {
        depth += 1;
        entity = parent.0;
        if depth >= 1024 {
            break;
        }
    }
    depth
}

pub(super) fn collect_panel_descendant_invalidations(world: &mut World) {
    world.init_resource::<WidgetPanelInvalidationGate>();
    let changed = world.resource_scope(|world, mut gate: Mut<WidgetPanelInvalidationGate>| {
        if !gate.initialized {
            gate.components = vec![
                world.register_component::<UiLayoutResult>(),
                world.register_component::<UiHitBox>(),
                world.register_component::<UiChildren>(),
                world.register_component::<UiParent>(),
            ];
            gate.initialized = true;
        }
        let changed = ui_components_changed_since(world, &gate.components, gate.last);
        gate.last = world.read_change_tick();
        changed
    });
    if changed
        && world
            .run_system_cached(collect_panel_descendant_invalidations_query)
            .is_err()
    {
        world
            .resource_mut::<WidgetPanelSyncInvalidations>()
            .topology_changed = true;
    }
    world.increment_change_tick();
}

#[expect(
    clippy::type_complexity,
    reason = "Panel clip invalidations track the layout, hitbox, and hierarchy inputs that can change a descendant reveal"
)]
pub(super) fn collect_panel_descendant_invalidations_query(
    changed: Query<
        Entity,
        Or<(
            Changed<UiLayoutResult>,
            Changed<UiHitBox>,
            Changed<bbui_core::UiChildren>,
            Changed<UiParent>,
        )>,
    >,
    parents: Query<Option<&'static UiParent>>,
    panels: Query<(), With<Panel>>,
    topology: Query<Entity, Or<(Changed<bbui_core::UiChildren>, Changed<UiParent>)>>,
    mut invalidated: ResMut<WidgetPanelSyncInvalidations>,
) {
    if !topology.is_empty() {
        invalidated.topology_changed = true;
    }
    for entity in &changed {
        let mut current = entity;
        for _ in 0..1024 {
            if panels.get(current).is_ok() {
                invalidated.panels.insert(current);
            }
            let Ok(Some(parent)) = parents.get(current) else {
                break;
            };
            current = parent.0;
        }
    }
}

pub(super) fn panel_descendant_entities(world: &World, panel: Entity) -> Vec<Entity> {
    let mut stack = world
        .get::<bbui_core::UiChildren>(panel)
        .map_or_else(Vec::new, |children| children.0.clone());
    let mut seen = HashSet::new();
    let mut descendants = Vec::new();
    while let Some(entity) = stack.pop() {
        if !seen.insert(entity) {
            continue;
        }
        descendants.push(entity);
        if let Some(children) = world.get::<bbui_core::UiChildren>(entity) {
            stack.extend(children.0.iter().copied());
        }
    }
    descendants
}

pub(super) fn panel_reveal_rect(
    world: &World,
    panel: Entity,
    header_spacing: f32,
    clips: &HashMap<Entity, PanelDescendantClip>,
) -> Option<Rect> {
    let progress = world.get::<Panel>(panel)?.state.progress;
    if progress >= 1.0 {
        return None;
    }
    let panel_layout = world.get::<UiLayoutResult>(panel).copied()?;
    let header_height = panel_layout.rect.height.min(header_spacing);
    let natural_content_height = (panel_layout.content_size[1] - header_height).max(0.0);
    let allocated_content_height = (panel_layout.rect.height - header_height).max(0.0);
    let content_height = (natural_content_height * progress).min(allocated_content_height);
    let mut reveal = Rect {
        x: panel_layout.rect.x,
        y: panel_layout.rect.y + header_height,
        width: panel_layout.rect.width,
        height: content_height,
    };
    let base_clip = clips
        .get(&panel)
        .and_then(|state| state.applied_layout.map(|_| state.base_layout))
        .unwrap_or(panel_layout.clip);
    if let Some(parent_clip) = base_clip {
        reveal = reveal.intersection(parent_clip).unwrap_or(Rect {
            x: reveal.x.max(parent_clip.x),
            y: reveal.y.max(parent_clip.y),
            width: 0.0,
            height: 0.0,
        });
    }
    Some(reveal)
}

pub(super) fn panel_effective_layout_clip(
    world: &World,
    entity: Entity,
    base: Option<Rect>,
    header_spacing: f32,
    clips: &HashMap<Entity, PanelDescendantClip>,
) -> Option<Rect> {
    let mut effective = base;
    let mut current = world.get::<UiParent>(entity).map(|parent| parent.0);
    for _ in 0..1024 {
        let Some(panel) = current else {
            break;
        };
        if world.get::<Panel>(panel).is_some()
            && let Some(reveal) = panel_reveal_rect(world, panel, header_spacing, clips)
        {
            effective = Some(effective.map_or(reveal, |old| {
                old.intersection(reveal).unwrap_or(Rect {
                    x: old.x.max(reveal.x),
                    y: old.y.max(reveal.y),
                    width: 0.0,
                    height: 0.0,
                })
            }));
        }
        current = world.get::<UiParent>(panel).map(|parent| parent.0);
    }
    effective
}

pub(super) fn panel_effective_hit_clip(
    world: &World,
    entity: Entity,
    base: Option<UiRect>,
    header_spacing: f32,
    clips: &HashMap<Entity, PanelDescendantClip>,
) -> Option<UiRect> {
    let mut effective = base;
    let mut current = world.get::<UiParent>(entity).map(|parent| parent.0);
    for _ in 0..1024 {
        let Some(panel) = current else {
            break;
        };
        if world.get::<Panel>(panel).is_some()
            && let Some(reveal) = panel_reveal_rect(world, panel, header_spacing, clips)
        {
            let reveal = to_ui_rect(reveal);
            effective = Some(effective.map_or(reveal, |old| {
                old.intersection(reveal).unwrap_or(UiRect::new(
                    old.x.max(reveal.x),
                    old.y.max(reveal.y),
                    0.0,
                    0.0,
                ))
            }));
        }
        current = world.get::<UiParent>(panel).map(|parent| parent.0);
    }
    effective
}

pub(super) fn apply_panel_clip(
    world: &mut World,
    panel: Entity,
    clips: &mut HashMap<Entity, PanelDescendantClip>,
) {
    let Some(panel_layout) = world.get::<UiLayoutResult>(panel).copied() else {
        return;
    };
    let header_spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.panel.header_height,
        |theme| theme.spacing.panel.header_height,
    );
    let descendants = panel_descendant_entities(world, panel);
    let mut live_descendants = HashSet::with_capacity(descendants.len());
    for entity in descendants {
        let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
            continue;
        };
        if layout.window != panel_layout.window {
            continue;
        }
        live_descendants.insert(entity);
        let mut state = clips.get(&entity).copied().unwrap_or_default();
        if state.applied_layout != Some(layout.clip) {
            state.base_layout = layout.clip;
        }
        let current_hit = world.get::<UiHitBox>(entity).map(|hit| hit.clip);
        if let Some(current_hit) = current_hit {
            if state.applied_hit != Some(current_hit) {
                state.base_hit = current_hit;
            }
        } else {
            state.base_hit = None;
            state.applied_hit = None;
        }
        clips.insert(entity, state);
        let layout_clip =
            panel_effective_layout_clip(world, entity, state.base_layout, header_spacing, clips);
        if layout.clip != layout_clip
            && let Some(mut layout) = world.get_mut::<UiLayoutResult>(entity)
        {
            layout.clip = layout_clip;
        }
        let hit_clip =
            panel_effective_hit_clip(world, entity, state.base_hit, header_spacing, clips);
        if let Some(mut hit) = world.get_mut::<UiHitBox>(entity) {
            if hit.clip != hit_clip {
                hit.clip = hit_clip;
            }
            state.applied_hit = Some(hit_clip);
        }
        state.applied_layout = Some(layout_clip);
        clips.insert(entity, state);
    }
    clips
        .retain(|entity, _| live_descendants.contains(entity) || world.get_entity(*entity).is_ok());
}

pub(super) fn has_ancestor(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    while let Some(parent) = world.get::<UiParent>(entity) {
        if parent.0 == ancestor {
            return true;
        }
        entity = parent.0;
    }
    false
}

pub(super) fn has_panel_ancestor(world: &World, mut entity: Entity) -> bool {
    for _ in 0..1024 {
        let Some(parent) = world.get::<UiParent>(entity) else {
            return false;
        };
        entity = parent.0;
        if world.get::<Panel>(entity).is_some() {
            return true;
        }
    }
    false
}

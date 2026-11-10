use super::*;

/// Resolve theme defaults, matching rules, inherited text values, and inline
/// values. Layout declarations are applied before the following layout phase.
pub fn resolve_ui_styles(world: &mut World) {
    world.init_resource::<UiDirty>();
    world.init_resource::<UiStyleResolutionCache>();
    world.init_resource::<UiStyleCollectorCursor>();
    world.init_resource::<UiStyleResourceCursor>();
    world.init_resource::<Theme>();
    world.init_resource::<UiStyleSheet>();
    world.init_resource::<UiReducedMotion>();
    collect_style_component_changes_gated(world);
    if world
        .run_system_cached(collect_style_resource_changes)
        .is_err()
    {
        world
            .resource_mut::<UiDirty>()
            .mark_all(UiDirtyDomain::Style);
    }
    resolve_ui_styles_dirty(world);
}

pub(super) fn resolve_ui_styles_pass(world: &mut World, dirty: Option<UiDirtyBatch>) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let sheet = world
        .get_resource::<UiStyleSheet>()
        .cloned()
        .unwrap_or_default();
    let mut nodes: Vec<Entity> = if let Some(batch) = dirty.filter(|batch| !batch.all) {
        let mut seen = std::collections::HashSet::new();
        let mut pending: Vec<_> = batch.entities.into_iter().collect();
        while let Some(entity) = pending.pop() {
            if !seen.insert(entity) {
                continue;
            }
            if let Some(children) = world.get::<UiChildren>(entity) {
                pending.extend(children.0.iter().copied());
            }
        }
        seen.into_iter()
            .filter(|entity| world.get::<UiNode>(*entity).is_some())
            .collect()
    } else {
        let mut query = world.query_filtered::<Entity, With<UiNode>>();
        query.iter(world).collect()
    };
    nodes.sort_by_key(|entity| {
        (
            hierarchy_depth(world, *entity),
            hierarchy_order(world, *entity),
        )
    });

    for entity in nodes {
        let base_layout = capture_base_layout_node(world, entity);
        let role = world.get::<UiRole>(entity);
        let classes = world.get::<UiClasses>(entity);
        let mut state = world
            .get::<UiStyleState>(entity)
            .copied()
            .unwrap_or_default();
        if world
            .get::<UiEnabled>(entity)
            .is_some_and(|enabled| !enabled.0)
        {
            state = state.union(UiStyleState::DISABLED);
        }
        if let Some(focus) = world.get_resource::<UiFocus>() {
            for window_state in focus.windows.values() {
                if window_state.hovered == Some(entity) {
                    state = state.union(UiStyleState::HOVERED);
                }
                if window_state.captured == Some(entity) {
                    state = state.union(UiStyleState::PRESSED);
                }
                if window_state.focused == Some(entity) {
                    state = state.union(UiStyleState::FOCUSED);
                }
            }
        }
        let default_inline = UiInlineStyle::default();
        let inline = world
            .get::<UiInlineStyle>(entity)
            .unwrap_or(&default_inline);
        let parent_style = world
            .get::<UiParent>(entity)
            .and_then(|parent| world.get::<UiComputedStyle>(parent.0));
        let mut target = resolve_style(&theme, &sheet, role, classes, state, parent_style, inline);
        add_base_layout_values(&mut target, base_layout);

        let default_current = UiComputedStyle::default();
        let current = world
            .get::<UiComputedStyle>(entity)
            .unwrap_or(&default_current);
        let default_previous = UiStyleTransitionState::default();
        let previous_transitions = world
            .get::<UiStyleTransitionState>(entity)
            .unwrap_or(&default_previous);
        let default_transitions = UiTransitions::default();
        let transitions = world
            .get::<UiTransitions>(entity)
            .unwrap_or(&default_transitions);
        let reduced_motion = world
            .get_resource::<UiReducedMotion>()
            .is_some_and(|setting| setting.0)
            || !world.contains_resource::<Time>()
            || nonnegative(theme.motion_scale) == 0.0;
        let motion_scale = nonnegative(theme.motion_scale);
        let (resolved, next_transitions) = transition_style(
            current,
            &target,
            transitions,
            previous_transitions,
            reduced_motion,
            motion_scale,
        );
        let changed = current != &resolved;
        let transition_changed = previous_transitions != &next_transitions;
        if changed {
            apply_box_shadow_style(world, entity, &resolved, &theme);
            world.entity_mut(entity).insert(resolved.clone());
        }
        if transition_changed {
            if next_transitions.tracks.is_empty() {
                world.entity_mut(entity).remove::<UiStyleTransitionState>();
            } else {
                world.entity_mut(entity).insert(next_transitions);
            }
        }
        apply_layout_style(world, entity, &resolved);
    }

    let mut cache = world.resource_mut::<UiStyleResolutionCache>();
    cache.resolved_passes = cache.resolved_passes.saturating_add(1);
    let settled_tick = world.read_change_tick();
    if let Some(mut cursor) = world.get_resource_mut::<UiStyleCollectorCursor>() {
        cursor.0 = settled_tick;
    }
}

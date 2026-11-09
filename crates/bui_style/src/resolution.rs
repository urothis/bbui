use super::*;

/// Resolve theme defaults, matching rules, inherited text values, and inline
/// values. Layout declarations are applied before the following layout phase.
pub fn resolve_ui_styles(world: &mut World) {
    world.init_resource::<UiStyleResolutionCache>();
    let input_stamp = ui_style_input_stamp(world);
    if world.resource::<UiStyleResolutionCache>().input_stamp == Some(input_stamp) {
        world
            .resource_mut::<UiStyleResolutionCache>()
            .skipped_passes += 1;
        return;
    }

    resolve_ui_styles_pass(world, None);
    let settled_stamp = ui_style_input_stamp(world);
    world.resource_mut::<UiStyleResolutionCache>().input_stamp = Some(settled_stamp);
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
        let role = world.get::<UiRole>(entity).cloned();
        let classes = world.get::<UiClasses>(entity).cloned();
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
        let inline = world
            .get::<UiInlineStyle>(entity)
            .cloned()
            .unwrap_or_default();
        let parent_style = world
            .get::<UiParent>(entity)
            .and_then(|parent| world.get::<UiComputedStyle>(parent.0))
            .cloned();
        let mut target = resolve_style(
            &theme,
            &sheet,
            role.as_ref(),
            classes.as_ref(),
            state,
            parent_style.as_ref(),
            &inline,
        );
        let base_layout = capture_base_layout_node(world, entity);
        add_base_layout_values(&mut target, base_layout);

        let current = world
            .get::<UiComputedStyle>(entity)
            .cloned()
            .unwrap_or_default();
        let previous_transitions = world
            .get::<UiStyleTransitionState>(entity)
            .cloned()
            .unwrap_or_default();
        let transitions = world
            .get::<UiTransitions>(entity)
            .cloned()
            .unwrap_or_default();
        let reduced_motion = world
            .get_resource::<UiReducedMotion>()
            .is_some_and(|setting| setting.0)
            || !world.contains_resource::<Time>()
            || nonnegative(theme.motion_scale) == 0.0;
        let motion_scale = nonnegative(theme.motion_scale);
        let (resolved, next_transitions) = transition_style(
            &current,
            &target,
            &transitions,
            &previous_transitions,
            reduced_motion,
            motion_scale,
        );
        if current != resolved {
            apply_box_shadow_style(world, entity, &resolved, &theme);
            world.entity_mut(entity).insert(resolved.clone());
        }
        if previous_transitions != next_transitions {
            if next_transitions.tracks.is_empty() {
                world.entity_mut(entity).remove::<UiStyleTransitionState>();
            } else {
                world.entity_mut(entity).insert(next_transitions);
            }
        }
        apply_layout_style(world, entity, &resolved);
    }

    let mut cache = world.resource_mut::<UiStyleResolutionCache>();
    cache.input_stamp = None;
    cache.resolved_passes = cache.resolved_passes.saturating_add(1);
    let settled_tick = world.read_change_tick();
    if let Some(mut cursor) = world.get_resource_mut::<UiStyleCollectorCursor>() {
        cursor.0 = settled_tick;
    }
}

pub(super) fn ui_style_input_stamp(world: &World) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_style_resource_ticks::<Theme>(world, &mut hasher);
    hash_style_resource_ticks::<UiStyleSheet>(world, &mut hasher);
    hash_style_resource_ticks::<UiReducedMotion>(world, &mut hasher);
    world.contains_resource::<Time>().hash(&mut hasher);

    if let Some(focus) = world.get_resource::<UiFocus>() {
        for (window, state) in &focus.windows {
            window.hash(&mut hasher);
            state.focused.hash(&mut hasher);
            state.hovered.hash(&mut hasher);
            state.captured.hash(&mut hasher);
        }
    }

    macro_rules! hash_style_component {
        ($entity:ident, $component:ty) => {
            if let Some(ticks) = $entity.get_change_ticks::<$component>() {
                true.hash(&mut hasher);
                ticks.added.get().hash(&mut hasher);
                ticks.changed.get().hash(&mut hasher);
            } else {
                false.hash(&mut hasher);
            }
        };
    }

    for entity in world.iter_entities() {
        if !entity.contains::<UiNode>()
            && !entity.contains::<UiParent>()
            && !entity.contains::<UiComputedStyle>()
            && !entity.contains::<UiBaseLayoutNode>()
            && !entity.contains::<UiLayoutNode>()
        {
            continue;
        }
        entity.id().hash(&mut hasher);
        hash_style_component!(entity, UiNode);
        hash_style_component!(entity, UiParent);
        hash_style_component!(entity, UiRole);
        hash_style_component!(entity, UiClasses);
        hash_style_component!(entity, UiStyleState);
        hash_style_component!(entity, UiEnabled);
        hash_style_component!(entity, UiInlineStyle);
        hash_style_component!(entity, UiComputedStyle);
        hash_style_component!(entity, UiTransitions);
        hash_style_component!(entity, UiStyleTransitionState);
        hash_style_component!(entity, UiBaseLayoutNode);
        hash_style_component!(entity, UiLayoutNode);
    }
    hasher.finish()
}

pub(super) fn hash_style_resource_ticks<R: Resource>(world: &World, hasher: &mut impl Hasher) {
    if let Some(ticks) = world.get_resource_change_ticks::<R>() {
        true.hash(hasher);
        ticks.added.get().hash(hasher);
        ticks.changed.get().hash(hasher);
    } else {
        false.hash(hasher);
    }
}

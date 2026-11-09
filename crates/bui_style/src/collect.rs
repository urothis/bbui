use super::*;

#[expect(
    clippy::type_complexity,
    reason = "Bevy change query covers the style inputs"
)]
pub(super) fn collect_style_component_changes(
    changed: Query<
        (
            Entity,
            Option<Ref<UiNode>>,
            Option<Ref<UiParent>>,
            Option<Ref<UiRole>>,
            Option<Ref<UiClasses>>,
            Option<Ref<UiStyleState>>,
            Option<Ref<UiEnabled>>,
            Option<Ref<UiInlineStyle>>,
            Option<Ref<UiTransitions>>,
            Option<Ref<UiBaseLayoutNode>>,
            Option<Ref<UiLayoutNode>>,
            Option<Ref<UiComputedStyle>>,
        ),
        Or<(
            Changed<UiNode>,
            Changed<UiParent>,
            Changed<UiRole>,
            Changed<UiClasses>,
            Changed<UiStyleState>,
            Changed<UiEnabled>,
            Changed<UiInlineStyle>,
            Changed<UiTransitions>,
            Changed<UiBaseLayoutNode>,
            Changed<UiLayoutNode>,
            Changed<UiComputedStyle>,
        )>,
    >,
    mut cursor: ResMut<UiStyleCollectorCursor>,
    tick: SystemChangeTick,
    mut dirty: ResMut<UiDirty>,
) {
    let last = cursor.0;
    let now = tick.this_run();
    for (
        entity,
        node,
        parent,
        role,
        classes,
        state,
        enabled,
        inline,
        transitions,
        base,
        layout,
        computed,
    ) in &changed
    {
        if [
            node.as_ref().map(DetectChanges::last_changed),
            parent.as_ref().map(DetectChanges::last_changed),
            role.as_ref().map(DetectChanges::last_changed),
            classes.as_ref().map(DetectChanges::last_changed),
            state.as_ref().map(DetectChanges::last_changed),
            enabled.as_ref().map(DetectChanges::last_changed),
            inline.as_ref().map(DetectChanges::last_changed),
            transitions.as_ref().map(DetectChanges::last_changed),
            base.as_ref().map(DetectChanges::last_changed),
            layout.as_ref().map(DetectChanges::last_changed),
            computed.as_ref().map(DetectChanges::last_changed),
        ]
        .into_iter()
        .flatten()
        .any(|changed| changed.is_newer_than(last, now))
        {
            dirty.mark(UiDirtyDomain::Style, entity);
        }
    }
    cursor.0 = now;
}

#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects each tracked resource separately"
)]
pub(super) fn collect_style_resource_changes(
    theme: Res<Theme>,
    sheet: Res<UiStyleSheet>,
    reduced_motion: Res<UiReducedMotion>,
    focus: Option<Res<UiFocus>>,
    mut cursor: ResMut<UiStyleResourceCursor>,
    tick: SystemChangeTick,
    mut cache: ResMut<UiStyleResolutionCache>,
    mut dirty: ResMut<UiDirty>,
) {
    let last = cursor.0;
    let now = tick.this_run();
    if cache.resolved_passes == 0
        || theme.last_changed().is_newer_than(last, now)
        || sheet.last_changed().is_newer_than(last, now)
        || reduced_motion.last_changed().is_newer_than(last, now)
    {
        dirty.mark_all(UiDirtyDomain::Style);
    }
    let mut targets = HashMap::<Entity, u8>::new();
    for state in focus.iter().flat_map(|focus| focus.windows.values()) {
        for (entity, bit) in [(state.focused, 1), (state.hovered, 2), (state.captured, 4)] {
            if let Some(entity) = entity {
                *targets.entry(entity).or_default() |= bit;
            }
        }
    }
    for entity in cache.focus_targets.keys().chain(targets.keys()) {
        if cache.focus_targets.get(entity) != targets.get(entity) {
            dirty.mark(UiDirtyDomain::Style, *entity);
        }
    }
    cache.focus_targets = targets;
    cursor.0 = now;
}

pub(super) fn resolve_ui_styles_dirty(world: &mut World) {
    let batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Style);
    if batch.is_empty() {
        world
            .resource_mut::<UiStyleResolutionCache>()
            .skipped_passes += 1;
        return;
    }
    resolve_ui_styles_pass(world, Some(batch));
}

pub(super) fn has_pointer_hover_transition(events: Res<UiRoutedEvents>) -> bool {
    events.events.iter().any(|event| {
        matches!(
            event,
            UiRoutedEvent::PointerEnter { .. } | UiRoutedEvent::PointerLeave { .. }
        )
    })
}

pub(super) fn converge_hover_layout(world: &mut World) {
    const MAX_HOVER_LAYOUT_PASSES: usize = 3;
    for _ in 0..MAX_HOVER_LAYOUT_PASSES {
        compute_layout(world);
        if !refresh_ui_hover(world) {
            return;
        }
        resolve_ui_styles(world);
    }

    // A state rule can move the hit target repeatedly. Bound that work, then
    // leave hover aligned with the final hitboxes; the next frame can continue
    // if the rule set is oscillating.
    compute_layout(world);
    refresh_ui_hover(world);
}

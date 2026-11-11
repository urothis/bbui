use super::*;
use bbui_motion::{
    Keyframe, KeyframeTrack, MotionBinding, MotionClip, MotionPlayer, MotionPriority,
};
use std::time::Duration;

const PANEL_PROGRESS_BINDING: u64 = 1_000;
const MENU_PROGRESS_BINDING: u64 = 1_001;

pub fn panel_progress_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        PANEL_PROGRESS_BINDING,
        |world, entity| Some(world.get::<Panel>(entity)?.state.progress),
        |world, entity, value| {
            let progress = {
                let Some(mut panel) = world.get_mut::<Panel>(entity) else {
                    return;
                };
                panel.state.progress = value.clamp(0.0, 1.0);
                panel.state.progress
            };
            sync_panel_reveal(world, entity, progress);
            let mut dirty = world.resource_mut::<UiDirty>();
            dirty.mark(UiDirtyDomain::Layout, entity);
            dirty.mark(UiDirtyDomain::Paint, entity);
            dirty.mark(UiDirtyDomain::HitTest, entity);
            dirty.mark(UiDirtyDomain::Accessibility, entity);
        },
    )
}

fn sync_panel_reveal(world: &mut World, entity: Entity, progress: f32) -> bool {
    if world
        .get::<UiLayoutNode>(entity)
        .is_none_or(|layout| layout.mode == UiLayoutMode::Leaf)
    {
        return false;
    }
    let target = UiContentReveal::new(progress);
    if let Some(mut reveal) = world.get_mut::<UiContentReveal>(entity) {
        if *reveal != target {
            *reveal = target;
            return true;
        }
    } else {
        world.entity_mut(entity).insert(target);
        return true;
    }
    false
}

pub fn menu_progress_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        MENU_PROGRESS_BINDING,
        |world, entity| {
            world
                .get_resource::<WidgetMenuAnimations>()?
                .0
                .get(&entity)
                .map(|state| state.progress)
        },
        |world, entity, value| {
            let Some(mut animations) = world.get_resource_mut::<WidgetMenuAnimations>() else {
                return;
            };
            let Some(state) = animations.0.get_mut(&entity) else {
                return;
            };
            state.progress = value.clamp(0.0, 1.0);
            if !state.target_open && state.progress <= f32::EPSILON {
                animations.0.remove(&entity);
            }
            mark_menu_paint_subtree(world, entity);
        },
    )
    .on_release(|world, _| {
        if let Some(mut gate) = world.get_resource_mut::<WidgetMenuAnimationGate>() {
            gate.initialized = false;
        }
    })
}

fn progress_clip(
    from: f32,
    to: f32,
    duration: Duration,
    curve: bbui_motion::MotionCurve,
    binding: MotionBinding<f32>,
) -> Option<MotionClip> {
    if duration.is_zero() {
        return None;
    }
    let frames = KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, from).curve(curve),
        Keyframe::at(duration, to),
    ])
    .ok()?;
    Some(MotionClip::new().track(binding, frames))
}

fn widget_motion_delta(world: &World) -> Duration {
    world
        .get_resource::<bevy_time::Time>()
        .map(bevy_time::Time::delta)
        .unwrap_or(Duration::from_secs_f32(1.0 / 60.0))
}

fn configure_widget_motion(world: &mut World) {
    let scale = world
        .get_resource::<Theme>()
        .map_or(1.0, |theme| theme.motion_scale);
    let reduced = world
        .get_resource::<bbui_style::UiReducedMotion>()
        .is_some_and(|value| value.0);
    let mut player = world.resource_mut::<MotionPlayer>();
    player.set_time_scale(if scale > 0.0 { 1.0 / scale } else { 0.0 });
    player.set_reduced_motion(reduced);
}

/// Generate panel clips when expansion targets change. The motion player owns playback.
pub fn sync_panel_animations(world: &mut World) {
    world.init_resource::<MotionPlayer>();
    configure_widget_motion(world);
    let Ok(candidates) = world.run_system_cached(changed_panels_for_animation) else {
        return;
    };
    if candidates.is_empty() {
        return;
    }
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let duration = theme.motion.duration(ThemeMotionToken::PanelExpand);
    let curve = theme.motion.animation(ThemeMotionToken::PanelExpand).curve;
    let snap = world
        .get_resource::<bbui_style::UiReducedMotion>()
        .is_some_and(|value| value.0)
        || theme.motion_scale <= 0.0;
    let binding = panel_progress_binding();
    let mut fresh = Vec::new();
    let candidates: Vec<_> = candidates
        .into_iter()
        .filter_map(|entity| {
            let panel = world.get::<Panel>(entity)?;
            Some((
                entity,
                panel.state.progress,
                if panel.state.target_expanded {
                    1.0
                } else {
                    0.0
                },
            ))
        })
        .collect();
    let mut geometry_changed = false;
    for (entity, progress, target) in candidates {
        geometry_changed |= sync_panel_reveal(world, entity, progress);
        if (progress - target).abs() <= f32::EPSILON {
            continue;
        }
        if world
            .resource::<MotionPlayer>()
            .targets(entity, &binding, &target)
        {
            continue;
        }
        if snap || duration.is_zero() {
            world
                .resource_mut::<MotionPlayer>()
                .cancel_binding(entity, binding.id());
            if let Some(mut panel) = world.get_mut::<Panel>(entity) {
                panel.state.progress = target;
            }
            sync_panel_reveal(world, entity, target);
            geometry_changed = true;
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Paint, entity);
        } else if let Some(clip) = progress_clip(progress, target, duration, curve, binding.clone())
            && let Some(handle) =
                world
                    .resource_mut::<MotionPlayer>()
                    .play(entity, clip, MotionPriority::Automatic)
        {
            fresh.push(handle);
            geometry_changed = true;
        }
    }
    if !fresh.is_empty() {
        let delta = widget_motion_delta(world);
        bbui_motion::advance_motion_handles_by(world, &fresh, delta);
    }
    // Widget actions run after the normal layout phase. Reflow once here only
    // when this phase changed panel geometry, before syncing descendant clips.
    if geometry_changed && world.get_resource::<bbui_layout::UiLayoutStats>().is_some() {
        bbui_layout::compute_layout(world);
    }
}

fn changed_panels_for_animation(query: Query<Entity, Changed<Panel>>) -> Vec<Entity> {
    query.iter().collect()
}

/// Keep menu surfaces at their target geometry while their visual progress is keyed.
pub fn sync_menu_animations(world: &mut World) {
    let delta = widget_motion_delta(world);
    sync_menu_animations_with_delta(world, delta);
}

fn sync_menu_animations_with_delta(world: &mut World, initial_delta: Duration) {
    world.init_resource::<WidgetMenuAnimations>();
    world.init_resource::<WidgetMenuAnimationGate>();
    world.init_resource::<MotionPlayer>();
    configure_widget_motion(world);
    let scan = world.resource_scope(|world, mut gate: Mut<WidgetMenuAnimationGate>| {
        if !gate.initialized {
            gate.components = vec![
                world.register_component::<MenuControl>(),
                world.register_component::<UiLayoutResult>(),
            ];
            gate.initialized = true;
            return true;
        }
        ui_components_changed_since(world, &gate.components, gate.last)
            || world
                .get_resource_change_ticks::<Theme>()
                .is_some_and(|ticks| ticks.is_changed(gate.last, world.read_change_tick()))
            || world
                .get_resource_change_ticks::<bbui_style::UiReducedMotion>()
                .is_some_and(|ticks| ticks.is_changed(gate.last, world.read_change_tick()))
    });
    if !scan {
        return;
    }
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spec = theme.motion.animation(ThemeMotionToken::MenuOpen);
    let snap = world
        .get_resource::<bbui_style::UiReducedMotion>()
        .is_some_and(|value| value.0)
        || theme.motion_scale <= 0.0
        || spec.duration.is_zero();
    let binding = menu_progress_binding();
    let mut fresh = Vec::new();
    let mut query = world.query::<(Entity, &MenuControl, &UiLayoutResult)>();
    let menus: Vec<_> = query
        .iter(world)
        .map(|(entity, menu, layout)| (entity, menu.session.open, layout.rect, menu.popup.bounds))
        .collect();
    let live: HashSet<_> = menus.iter().map(|(entity, ..)| *entity).collect();
    let old: Vec<_> = world
        .resource::<WidgetMenuAnimations>()
        .0
        .keys()
        .copied()
        .collect();
    for entity in old {
        if !live.contains(&entity) {
            world
                .resource_mut::<WidgetMenuAnimations>()
                .0
                .remove(&entity);
            world
                .resource_mut::<MotionPlayer>()
                .cancel_binding(entity, binding.id());
            mark_menu_paint_subtree(world, entity);
        }
    }
    for (entity, open, anchor, popup) in menus {
        let target = if open { 1.0 } else { 0.0 };
        let current = world
            .resource::<WidgetMenuAnimations>()
            .0
            .get(&entity)
            .copied();
        if current.is_none() && !open {
            continue;
        }
        let progress = current.map_or(0.0, |state| state.progress);
        let geometry_changed =
            current.is_some_and(|state| state.anchor != anchor || (open && state.popup != popup));
        if let Some(state) = world
            .resource_mut::<WidgetMenuAnimations>()
            .0
            .get_mut(&entity)
        {
            state.target_open = open;
            state.anchor = anchor;
            if open {
                state.popup = popup;
            }
        } else {
            world.resource_mut::<WidgetMenuAnimations>().0.insert(
                entity,
                MenuAnimationState {
                    progress,
                    target_open: open,
                    anchor,
                    popup,
                },
            );
        }
        if geometry_changed {
            mark_menu_paint_subtree(world, entity);
        }
        if (progress - target).abs() <= f32::EPSILON {
            continue;
        }
        if world
            .resource::<MotionPlayer>()
            .targets(entity, &binding, &target)
        {
            continue;
        }
        if snap {
            world
                .resource_mut::<MotionPlayer>()
                .cancel_binding(entity, binding.id());
            if let Some(state) = world
                .resource_mut::<WidgetMenuAnimations>()
                .0
                .get_mut(&entity)
            {
                state.progress = target;
            }
            mark_menu_paint_subtree(world, entity);
        } else if let Some(clip) =
            progress_clip(progress, target, spec.duration, spec.curve, binding.clone())
        {
            if let Some(handle) =
                world
                    .resource_mut::<MotionPlayer>()
                    .play(entity, clip, MotionPriority::Automatic)
            {
                fresh.push(handle);
            }
            mark_menu_paint_subtree(world, entity);
        }
    }
    if !fresh.is_empty() {
        bbui_motion::advance_motion_handles_by(world, &fresh, initial_delta);
    }
    let now = world.read_change_tick();
    world.resource_mut::<WidgetMenuAnimationGate>().last = now;
    world.increment_change_tick();
}

/// Test helper: generate menu clips and advance the shared player by an exact delta.
#[cfg(test)]
pub(super) fn advance_menu_motion_by(world: &mut World, delta_seconds: f32) {
    sync_menu_animations_with_delta(world, Duration::ZERO);
    bbui_motion::advance_motion_by(world, Duration::from_secs_f32(delta_seconds.max(0.0)));
}

pub(super) fn menu_animation_progress(world: &World, owner: Entity, _theme: &Theme) -> Option<f32> {
    world
        .get_resource::<WidgetMenuAnimations>()?
        .0
        .get(&owner)
        .map(|state| state.progress)
}

pub(super) fn interpolate_rect(from: Rect, to: Rect, progress: f32) -> Rect {
    let mix = |start: f32, end: f32| start + (end - start) * progress;
    Rect {
        x: mix(from.x, to.x),
        y: mix(from.y, to.y),
        width: mix(from.width, to.width).max(0.0),
        height: mix(from.height, to.height).max(0.0),
    }
}

pub(super) fn with_opacity(color: Rgba, opacity: f32) -> Rgba {
    Rgba(color.0, color.1, color.2, color.3 * opacity.clamp(0.0, 1.0))
}

pub(super) fn intersect_clip_rect(clip: ClipRect, rect: Rect) -> Option<ClipRect> {
    Rect::new(clip.x, clip.y, clip.width, clip.height)
        .intersection(rect)
        .map(ClipRect::from)
}

pub(super) fn toggle_exclusive_panel(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(clicked) = world.get::<Panel>(target).cloned() else {
        return;
    };
    let clicked_key = clicked
        .persistence_key
        .clone()
        .unwrap_or_else(|| clicked.title.clone());
    let mut ancestors = vec![target];
    let mut current = target;
    while let Some(parent) = world.get::<UiParent>(current).map(|parent| parent.0) {
        ancestors.push(parent);
        current = parent;
    }
    let group_scope = ancestors.into_iter().find_map(|entity| {
        world
            .get::<PanelGroup>(entity)
            .filter(|group| group.panel_keys.iter().any(|key| key == &clicked_key))
            .map(|group| {
                (
                    entity,
                    group.panel_keys.iter().cloned().collect::<HashSet<_>>(),
                )
            })
    });
    let fallback_parent = world.get::<UiParent>(target).map(|parent| parent.0);
    let mut keys = group_scope
        .as_ref()
        .map_or_else(HashSet::new, |(_, keys)| keys.clone());
    if keys.is_empty()
        && let Some(parent) = fallback_parent
    {
        let mut panels = world.query::<(&Panel, &UiParent)>();
        keys.extend(
            panels
                .iter(world)
                .filter(|(panel, ui_parent)| ui_parent.0 == parent && panel.exclusive)
                .map(|(panel, _)| {
                    panel
                        .persistence_key
                        .clone()
                        .unwrap_or_else(|| panel.title.clone())
                }),
        );
    }
    keys.insert(clicked_key.clone());
    if let Some((group_entity, _)) = group_scope.as_ref()
        && let Some(mut group) = world.get_mut::<PanelGroup>(*group_entity)
    {
        group.exclusive_open = Some(clicked_key.clone());
    }
    let mut panels = world.query::<(Entity, &Panel)>();
    let changes: Vec<_> =
        panels
            .iter(world)
            .filter_map(|(entity, panel)| {
                let key = panel.persistence_key.as_deref().unwrap_or(&panel.title);
                let in_scope = if let Some((group, _)) = group_scope.as_ref() {
                    entity == target || has_ancestor(world, entity, *group)
                } else if let Some(parent) = fallback_parent {
                    entity == target
                        || world
                            .get::<UiParent>(entity)
                            .is_some_and(|ui_parent| ui_parent.0 == parent)
                } else {
                    entity == target
                };
                (in_scope && (entity == target || (panel.exclusive && keys.contains(key))))
                    .then_some((entity, panel.state.target_expanded, key == clicked_key))
            })
            .collect();
    let mut committed = Vec::new();
    for (entity, before, expanded) in changes {
        if let Some(mut panel) = world.get_mut::<Panel>(entity)
            && panel.set_expanded(expanded)
        {
            committed.push((entity, before, expanded));
        }
    }
    if !committed.is_empty() {
        emitted.push(WidgetEvent::PanelGroupCommitted { changes: committed });
    }
}

pub(super) fn panel_header_contains(world: &World, target: Entity, position: [f32; 2]) -> bool {
    let Some(layout) = world.get::<UiLayoutResult>(target) else {
        // Standalone panels without measured layout retain their routed behavior.
        return true;
    };
    let header_height = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.panel.header_height,
        |theme| theme.spacing.panel.header_height,
    );
    let header = Rect {
        height: layout.rect.height.min(header_height),
        ..layout.rect
    };
    header.contains(position[0], position[1])
}

pub(super) fn panel_reorder_context(
    world: &World,
    panel_entity: Entity,
) -> Option<(Entity, String, String, Vec<String>)> {
    let panel = world.get::<Panel>(panel_entity)?;
    let panel_key = panel
        .persistence_key
        .clone()
        .or_else(|| world.get::<UiKey>(panel_entity).map(|key| key.0.clone()))?;
    let group_entity = world.get::<UiParent>(panel_entity)?.0;
    let group = world.get::<PanelGroup>(group_entity)?;
    if !group.reorderable || !group.panel_keys.contains(&panel_key) {
        return None;
    }
    let persistence_key = group
        .persistence_key
        .clone()
        .or_else(|| world.get::<UiKey>(group_entity).map(|key| key.0.clone()))?;
    crate::panels::validate_panel_order(world, group_entity, &group.panel_keys).ok()?;
    Some((
        group_entity,
        persistence_key,
        panel_key,
        group.panel_keys.clone(),
    ))
}

pub(super) fn reorder_panel_step(
    world: &mut World,
    panel_entity: Entity,
    delta: i32,
    emitted: &mut Vec<WidgetEvent>,
) {
    if delta == 0 {
        return;
    }
    let Some((group, persistence_key, panel_key, before)) =
        panel_reorder_context(world, panel_entity)
    else {
        return;
    };
    let Some(current_index) = before.iter().position(|key| key == &panel_key) else {
        return;
    };
    let destination = (current_index as i64 + i64::from(delta))
        .clamp(0, before.len().saturating_sub(1) as i64) as usize;
    if destination == current_index {
        return;
    }
    let mut after = before.clone();
    let key = after.remove(current_index);
    after.insert(destination, key);
    if crate::panels::restore_panel_order(world, group, &after) {
        emitted.push(WidgetEvent::PanelOrderCommitted {
            group,
            persistence_key,
            before,
            after,
        });
    }
}

pub(super) fn begin_panel_reorder(
    world: &mut World,
    window: Entity,
    panel_entity: Entity,
    position: [f32; 2],
) {
    if !panel_header_contains(world, panel_entity, position) {
        return;
    }
    let Some((group, persistence_key, moving_key, order)) =
        panel_reorder_context(world, panel_entity)
    else {
        return;
    };
    let Some(children) = world.get::<bbui_core::UiChildren>(group) else {
        return;
    };
    let mut keyed_positions = HashMap::new();
    for child in &children.0 {
        let Some(panel) = world.get::<Panel>(*child) else {
            continue;
        };
        let Some(key) = panel
            .persistence_key
            .clone()
            .or_else(|| world.get::<UiKey>(*child).map(|key| key.0.clone()))
        else {
            continue;
        };
        let center_y = world
            .get::<UiLayoutResult>(*child)
            .map_or(f32::NAN, |layout| layout.rect.y + layout.rect.height * 0.5);
        keyed_positions.insert(key, center_y);
    }
    let slots: Vec<_> = order
        .iter()
        .enumerate()
        .map(|(index, key)| {
            (
                key.clone(),
                keyed_positions
                    .get(key)
                    .copied()
                    .filter(|center| center.is_finite())
                    .unwrap_or(index as f32),
            )
        })
        .collect();
    world
        .resource_mut::<WidgetInteractionState>()
        .panel_reorders
        .insert(
            window,
            PanelReorderGesture {
                group,
                source: panel_entity,
                persistence_key,
                moving_key,
                before: order.clone(),
                current: order,
                slots,
            },
        );
}

pub(super) fn update_panel_reorder(world: &mut World, window: Entity, position: [f32; 2]) {
    let snapshot = world
        .resource::<WidgetInteractionState>()
        .panel_reorders
        .get(&window)
        .map(|gesture| {
            (
                gesture.group,
                gesture.moving_key.clone(),
                gesture.before.clone(),
                gesture.current.clone(),
                gesture.slots.clone(),
            )
        });
    let Some((group, moving_key, before, current, slots)) = snapshot else {
        return;
    };
    let insert_at = slots
        .iter()
        .filter(|(key, center)| key != &moving_key && position[1] >= *center)
        .count()
        .min(before.len().saturating_sub(1));
    let mut after = before;
    let Some(current_index) = after.iter().position(|key| key == &moving_key) else {
        return;
    };
    let key = after.remove(current_index);
    after.insert(insert_at, key);
    if after == current {
        return;
    }
    if crate::panels::preview_panel_order(world, group, &after)
        && let Some(gesture) = world
            .resource_mut::<WidgetInteractionState>()
            .panel_reorders
            .get_mut(&window)
    {
        gesture.current = after;
    }
}

pub(super) fn finish_panel_reorder(
    world: &mut World,
    gesture: PanelReorderGesture,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        let _ = crate::panels::restore_panel_order(world, gesture.group, &gesture.before);
        return;
    }
    if gesture.current == gesture.before {
        return;
    }
    if crate::panels::restore_panel_order(world, gesture.group, &gesture.current) {
        emitted.push(WidgetEvent::PanelOrderCommitted {
            group: gesture.group,
            persistence_key: gesture.persistence_key,
            before: gesture.before,
            after: gesture.current,
        });
    } else {
        emitted.push(WidgetEvent::PanelOrderRestoreFailed {
            group: gesture.group,
            reason: "panel order no longer matches the live group".into(),
        });
    }
}

pub(super) fn cancel_panel_reorder(
    world: &mut World,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    let gesture = world
        .resource_mut::<WidgetInteractionState>()
        .panel_reorders
        .remove(&window);
    if let Some(gesture) = gesture {
        finish_panel_reorder(world, gesture, true, emitted);
        true
    } else {
        false
    }
}

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

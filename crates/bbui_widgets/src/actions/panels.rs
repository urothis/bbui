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

pub(super) fn sync_panel_reveal(world: &mut World, entity: Entity, progress: f32) -> bool {
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

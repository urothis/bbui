use super::*;

pub struct MotionPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionSystems {
    Prepare,
    Advance,
}

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MotionPlayer>()
            .init_resource::<MotionEvents>()
            .configure_sets(
                Update,
                (MotionSystems::Prepare, MotionSystems::Advance)
                    .chain()
                    .in_set(UiSystems::Motion),
            )
            .add_systems(Update, clear_motion_events.in_set(MotionSystems::Prepare))
            .add_systems(Update, advance_motion.in_set(MotionSystems::Advance));
    }
}

fn clear_motion_events(mut events: bevy_ecs::prelude::ResMut<MotionEvents>) {
    events.0.clear();
}

pub fn advance_motion(world: &mut World) {
    let delta = world
        .get_resource::<Time>()
        .map(Time::delta)
        .unwrap_or(Duration::from_secs_f32(1.0 / 60.0));
    advance_motion_by(world, delta);
}

pub fn advance_motion_by(world: &mut World, delta: Duration) {
    advance_motion_selected(world, delta, None);
}

/// Advance only newly started handles in the current widget phase.
pub fn advance_motion_handles_by(world: &mut World, handles: &[MotionHandle], delta: Duration) {
    advance_motion_selected(world, delta, Some(handles));
}

/// Seek and immediately apply sampled values, including for a paused clip.
pub fn seek_motion(world: &mut World, handle: MotionHandle, time: Duration) -> bool {
    let Some(mut player) = world.get_resource_mut::<MotionPlayer>() else {
        return false;
    };
    let mut player = std::mem::take(&mut *player);
    if !player.seek(handle, time) {
        *world.resource_mut::<MotionPlayer>() = player;
        return false;
    }
    if let Some(active) = player.clips.get(&handle) {
        for track in &active.clip.tracks {
            track.apply(world, active.entity, active.time.min(track.duration()));
        }
    }
    *world.resource_mut::<MotionPlayer>() = player;
    true
}

fn advance_motion_selected(world: &mut World, delta: Duration, selected: Option<&[MotionHandle]>) {
    world.init_resource::<MotionPlayer>();
    world.init_resource::<MotionEvents>();
    if selected.is_none()
        && world.resource::<MotionPlayer>().clips.is_empty()
        && world.resource::<MotionPlayer>().pending.is_empty()
        && world.resource::<MotionPlayer>().pending_releases.is_empty()
    {
        world.resource_mut::<MotionPlayer>().advanced_this_frame = true;
        return;
    }
    let mut player = std::mem::take(&mut *world.resource_mut::<MotionPlayer>());
    if selected.is_none() {
        player.advanced_this_frame = true;
    }
    for released in player.pending_releases.drain(..) {
        for track in &released.clip.tracks {
            track.release(world, released.entity);
        }
    }
    let mut completed = Vec::new();
    for (handle, active) in &mut player.clips {
        if selected.is_some_and(|handles| !handles.contains(handle)) {
            continue;
        }
        if !world.entities().contains(active.entity) {
            completed.push((*handle, MotionEventKind::Canceled));
            continue;
        }
        if active.paused && !player.reduced_motion && player.time_scale > 0.0 {
            continue;
        }
        let duration = active.clip.duration;
        let snap = player.reduced_motion || player.time_scale == 0.0 || duration.is_zero();
        let elapsed = if active.defer_first_advance && !snap {
            active.defer_first_advance = false;
            Duration::ZERO
        } else if snap {
            duration
        } else {
            let seconds = delta.as_secs_f64() * f64::from(active.speed * player.time_scale);
            let scaled = if seconds >= Duration::MAX.as_secs_f64() {
                Duration::MAX
            } else {
                Duration::from_secs_f64(seconds.max(0.0))
            };
            if active.delay > scaled {
                active.delay -= scaled;
                continue;
            }
            let available = scaled.saturating_sub(active.delay);
            active.delay = Duration::ZERO;
            available
        };
        let mut finished = snap;
        if snap {
            active.time = if active.reverse {
                Duration::ZERO
            } else {
                duration
            };
        } else if !duration.is_zero() {
            let span = duration.as_nanos();
            let elapsed = elapsed.as_nanos();
            let current = active.time.as_nanos();
            let (cycles, position) = if active.reverse {
                if elapsed < current {
                    (0, current - elapsed)
                } else {
                    let excess = elapsed - current;
                    (
                        1 + excess / span,
                        if excess % span == 0 {
                            span
                        } else {
                            span - excess % span
                        },
                    )
                }
            } else {
                let total = current + elapsed;
                (total / span, total % span)
            };
            if cycles > 0 {
                active.completed_cycles = active
                    .completed_cycles
                    .saturating_add(cycles.min(u128::from(u32::MAX)) as u32);
                finished = active
                    .repeats
                    .is_some_and(|count| active.completed_cycles >= count);
            }
            let position = if finished {
                if active.reverse { 0 } else { span }
            } else {
                position
            };
            active.time = duration_from_nanos(position);
        }
        for track in &active.clip.tracks {
            track.apply(world, active.entity, active.time.min(track.duration()));
        }
        if finished {
            completed.push((*handle, MotionEventKind::Completed));
        }
    }
    for (handle, kind) in completed {
        if let Some(active) = player.clips.shift_remove(&handle) {
            if active.priority == MotionPriority::Explicit {
                for track in &active.clip.tracks {
                    track.release(world, active.entity);
                }
            }
            player.pending.push(MotionEvent {
                handle,
                entity: active.entity,
                kind,
            });
        }
    }
    world
        .resource_mut::<MotionEvents>()
        .0
        .append(&mut player.pending);
    *world.resource_mut::<MotionPlayer>() = player;
}

fn duration_from_nanos(nanos: u128) -> Duration {
    let seconds = (nanos / 1_000_000_000).min(u128::from(u64::MAX)) as u64;
    let subsec = (nanos % 1_000_000_000) as u32;
    Duration::new(seconds, subsec)
}

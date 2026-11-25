use super::Progress;
use bbui_core::UiDirtyDomain;
use bbui_motion::MotionBinding;
use bbui_style::{Theme, UiReducedMotion};
use bevy_ecs::prelude::{Query, Res, World};
use bevy_time::Time;

/// Bind an application-owned motion clip to the displayed progress value.
pub fn progress_value_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0005,
        |world, entity| Some(world.get::<Progress>(entity)?.value),
        |world: &mut World, entity, value| {
            if let Some(mut progress) = world.get_mut::<Progress>(entity) {
                progress.set(value);
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind an application-owned motion clip to the gradient fill palette.
pub fn progress_palette_phase_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0006,
        |world, entity| Some(world.get::<Progress>(entity)?.palette_phase),
        |world: &mut World, entity, phase| {
            if phase.is_finite()
                && let Some(mut progress) = world.get_mut::<Progress>(entity)
            {
                progress.palette_phase = phase;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a motion clip to the border palette around the filled portion.
pub fn progress_border_phase_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0007,
        |world, entity| {
            Some(
                world
                    .get::<Progress>(entity)?
                    .style
                    .fill_border
                    .as_ref()?
                    .phase,
            )
        },
        |world: &mut World, entity, phase| {
            if phase.is_finite()
                && let Some(mut progress) = world.get_mut::<Progress>(entity)
                && let Some(border) = &mut progress.style.fill_border
            {
                border.phase = phase;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a motion clip to the filled portion's border intensity.
pub fn progress_border_strength_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0009,
        |world, entity| {
            Some(
                world
                    .get::<Progress>(entity)?
                    .style
                    .fill_border
                    .as_ref()?
                    .strength,
            )
        },
        |world: &mut World, entity, strength| {
            if strength.is_finite()
                && let Some(mut progress) = world.get_mut::<Progress>(entity)
                && let Some(border) = &mut progress.style.fill_border
            {
                border.strength = strength;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a motion clip to the filled portion's glow intensity.
pub fn progress_glow_strength_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0008,
        |world, entity| {
            Some(
                world
                    .get::<Progress>(entity)?
                    .style
                    .fill_glow
                    .as_ref()?
                    .strength,
            )
        },
        |world: &mut World, entity, strength| {
            if strength.is_finite()
                && let Some(mut progress) = world.get_mut::<Progress>(entity)
                && let Some(glow) = &mut progress.style.fill_glow
            {
                glow.strength = strength;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a motion clip to the filled portion's glow palette rotation.
pub fn progress_glow_phase_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_000a,
        |world, entity| {
            Some(
                world
                    .get::<Progress>(entity)?
                    .style
                    .fill_glow
                    .as_ref()?
                    .phase,
            )
        },
        |world: &mut World, entity, phase| {
            if phase.is_finite()
                && let Some(mut progress) = world.get_mut::<Progress>(entity)
                && let Some(glow) = &mut progress.style.fill_glow
            {
                glow.phase = phase;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Advance only indeterminate bars. Reduced motion holds the indicator in the center.
pub(crate) fn advance_indeterminate_progress(
    time: Option<Res<Time>>,
    theme: Res<Theme>,
    reduced_motion: Option<Res<UiReducedMotion>>,
    mut progress: Query<&mut Progress>,
) {
    let still = reduced_motion.is_some_and(|setting| setting.0) || theme.motion_scale <= 0.0;
    let delta = time.map_or(0.0, |time| time.delta_secs());
    for mut bar in &mut progress {
        if !bar.indeterminate {
            continue;
        }
        let next = if still {
            0.25
        } else {
            (bar.phase + delta / (1.4 * theme.motion_scale)).rem_euclid(1.0)
        };
        if bar.phase != next {
            bar.phase = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::{prelude::World, schedule::Schedule};
    use std::time::Duration;

    #[test]
    fn indeterminate_motion_advances_and_reduced_motion_centers_it() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_millis(140));
        world.insert_resource(time);
        world.insert_resource(Theme::default());
        world.insert_resource(UiReducedMotion(false));
        let moving = world.spawn(Progress::indeterminate()).id();
        let fixed = world.spawn(Progress::new(0.4)).id();
        let mut schedule = Schedule::default();
        schedule.add_systems(advance_indeterminate_progress);

        schedule.run(&mut world);
        assert!(
            world
                .get::<Progress>(moving)
                .is_some_and(|bar| bar.phase > 0.0)
        );
        assert_eq!(world.get::<Progress>(fixed).map(|bar| bar.phase), Some(0.0));

        world.resource_mut::<UiReducedMotion>().0 = true;
        schedule.run(&mut world);
        assert_eq!(
            world.get::<Progress>(moving).map(|bar| bar.phase),
            Some(0.25)
        );
    }
}

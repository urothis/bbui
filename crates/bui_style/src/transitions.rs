use super::*;

/// Advance active transitions once per frame. Resolution runs in
/// `StyleAfterInput` too, but only this function consumes Bevy's frame delta.
pub fn advance_ui_style_transitions(world: &mut World) {
    let delta = world
        .get_resource::<Time>()
        .map(Time::delta)
        .unwrap_or_default();
    if delta.is_zero() {
        return;
    }
    let mut query = world.query_filtered::<Entity, With<UiStyleTransitionState>>();
    let entities: Vec<_> = query.iter(world).collect();
    for entity in entities {
        let Some(mut state) = world.get::<UiStyleTransitionState>(entity).cloned() else {
            continue;
        };
        let mut changed = false;
        for track in state.tracks.values_mut() {
            if track.elapsed < track.duration {
                let elapsed = track.elapsed.saturating_add(delta).min(track.duration);
                changed |= elapsed != track.elapsed;
                track.elapsed = elapsed;
            }
        }
        if changed {
            world.entity_mut(entity).insert(state);
            if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
                dirty.mark(UiDirtyDomain::Style, entity);
            }
        }
    }
}

pub(super) fn transition_style(
    current: &UiComputedStyle,
    target: &UiComputedStyle,
    transitions: &UiTransitions,
    previous: &UiStyleTransitionState,
    reduced_motion: bool,
    motion_scale: f32,
) -> (UiComputedStyle, UiStyleTransitionState) {
    let mut result = UiComputedStyle::default();
    let mut state = previous.clone();
    state
        .tracks
        .retain(|property, _| target.properties.contains_key(property));

    for (&property, target_value) in &target.properties {
        let target_value = target_value.clone();
        let current_value = current.get(property);
        let spec = transitions.0.get(&property).copied();
        let duration = spec.map_or(Duration::ZERO, |spec| {
            scaled_duration(spec.duration, motion_scale)
        });
        let eased = if reduced_motion { None } else { spec };
        let mut displayed = target_value.clone();

        if let Some(spec) = eased.filter(|_| !duration.is_zero()) {
            if let Some(track) = state.tracks.get(&property).cloned()
                && track.to == target_value
                && track.duration == duration
                && track.easing == spec.easing
            {
                let progress = (track.elapsed.as_secs_f64() / track.duration.as_secs_f64()) as f32;
                if progress >= 1.0 {
                    state.tracks.remove(&property);
                } else if let Some(interpolated) =
                    interpolate_style_value(track.from, track.to, track.easing.sample(progress))
                {
                    displayed = interpolated;
                } else {
                    state.tracks.remove(&property);
                }
            } else {
                state.tracks.remove(&property);
                if let Some(from) = current_value
                    && from != target_value
                    && interpolate_style_value(from.clone(), target_value.clone(), 0.0).is_some()
                {
                    state.tracks.insert(
                        property,
                        StyleTrack {
                            from: from.clone(),
                            to: target_value.clone(),
                            elapsed: Duration::ZERO,
                            duration,
                            easing: spec.easing,
                        },
                    );
                    displayed = from;
                }
            }
        } else {
            state.tracks.remove(&property);
        }

        result.properties.insert(property, displayed);
    }

    (result, state)
}

pub(super) fn interpolate_style_value(
    from: UiStyleValue,
    to: UiStyleValue,
    progress: f32,
) -> Option<UiStyleValue> {
    let progress = clamp(progress);
    Some(match (from, to) {
        (UiStyleValue::Color(from), UiStyleValue::Color(to)) => {
            UiStyleValue::Color(from.mix(to, progress))
        }
        (UiStyleValue::Scalar(from), UiStyleValue::Scalar(to)) => {
            UiStyleValue::Scalar(mix_channel(from, to, progress))
        }
        (UiStyleValue::BoxShadow(from), UiStyleValue::BoxShadow(to)) => {
            UiStyleValue::BoxShadow(from.interpolate(&to, progress)?)
        }
        (UiStyleValue::Length(UiLength::Px(from)), UiStyleValue::Length(UiLength::Px(to))) => {
            UiStyleValue::Length(UiLength::Px(mix_channel(from, to, progress)))
        }
        (
            UiStyleValue::Length(UiLength::Percent(from)),
            UiStyleValue::Length(UiLength::Percent(to)),
        ) => UiStyleValue::Length(UiLength::Percent(mix_channel(from, to, progress))),
        (UiStyleValue::Length(UiLength::Fill(from)), UiStyleValue::Length(UiLength::Fill(to))) => {
            UiStyleValue::Length(UiLength::Fill(mix_channel(from, to, progress)))
        }
        (UiStyleValue::Length(UiLength::Auto), UiStyleValue::Length(UiLength::Auto)) => {
            UiStyleValue::Length(UiLength::Auto)
        }
        (UiStyleValue::Edges(from), UiStyleValue::Edges(to)) => UiStyleValue::Edges(UiEdges {
            left: mix_channel(from.left, to.left, progress),
            top: mix_channel(from.top, to.top, progress),
            right: mix_channel(from.right, to.right, progress),
            bottom: mix_channel(from.bottom, to.bottom, progress),
        }),
        _ => return None,
    })
}

pub(super) fn scaled_duration(duration: Duration, scale: f32) -> Duration {
    if !scale.is_finite() || scale <= 0.0 || duration.is_zero() {
        return Duration::ZERO;
    }
    let seconds = (duration.as_secs_f64() * f64::from(scale)).min(Duration::MAX.as_secs_f64());
    Duration::from_secs_f64(seconds)
}

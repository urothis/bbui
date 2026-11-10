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
    world.init_resource::<UiStyleTransitionOutputs>();
    world.init_resource::<UiStylePaintOnlyOutputs>();
    world.resource_mut::<UiStylePaintOnlyOutputs>().clear();
    let mut entity_query = world.query_filtered::<Entity, With<UiStyleTransitionState>>();
    let entities: Vec<_> = entity_query.iter(world).collect();
    let mut query = world.query::<(&mut UiStyleTransitionState, Option<&mut UiComputedStyle>)>();
    let mut resolve_entities = Vec::new();
    let mut painted_entities = Vec::new();
    let mut finished_entities = Vec::new();
    for entity in entities {
        let input_changed = world
            .resource::<UiDirty>()
            .is_marked(UiDirtyDomain::Style, entity);
        let Ok((mut state, computed)) = query.get_mut(world, entity) else {
            continue;
        };
        if !state
            .tracks
            .values()
            .any(|track| track.elapsed < track.duration)
        {
            continue;
        }
        let paint_only = !input_changed
            && computed.is_some()
            && state
                .tracks
                .keys()
                .all(|property| paint_only_transition(*property));
        if paint_only {
            let background_only =
                state.tracks.len() == 1 && state.tracks.contains_key(&UiStyleProperty::Background);
            let Some(mut computed) = computed else {
                continue;
            };
            let mut painted = false;
            for (&property, track) in &mut state.tracks {
                track.elapsed = track.elapsed.saturating_add(delta).min(track.duration);
                let progress = (track.elapsed.as_secs_f64() / track.duration.as_secs_f64()) as f32;
                let value = if progress >= 1.0 {
                    Some(track.to.clone())
                } else {
                    interpolate_style_value(
                        track.from.clone(),
                        track.to.clone(),
                        track.easing.sample(progress),
                    )
                };
                if let Some(value) = value
                    && computed.properties.get(&property) != Some(&value)
                {
                    computed.properties.insert(property, value);
                    painted = true;
                }
            }
            state
                .tracks
                .retain(|_, track| track.elapsed < track.duration);
            if state.tracks.is_empty() {
                finished_entities.push(entity);
            }
            if painted {
                painted_entities.push((entity, background_only));
            }
        } else {
            for track in state.tracks.values_mut() {
                track.elapsed = track.elapsed.saturating_add(delta).min(track.duration);
            }
            resolve_entities.push(entity);
        }
    }
    let output_ticks: Vec<_> = painted_entities
        .iter()
        .filter_map(|(entity, background_only)| {
            world
                .get_entity(*entity)
                .ok()?
                .get_change_ticks::<UiComputedStyle>()
                .map(|ticks| (*entity, ticks.changed, *background_only))
        })
        .collect();
    {
        let mut outputs = world.resource_mut::<UiStyleTransitionOutputs>();
        outputs.0.extend(
            output_ticks
                .iter()
                .map(|(entity, tick, _)| (*entity, *tick)),
        );
    }
    {
        let mut outputs = world.resource_mut::<UiStylePaintOnlyOutputs>();
        outputs.0.extend(
            output_ticks
                .iter()
                .filter_map(|(entity, tick, background_only)| {
                    background_only.then_some((*entity, *tick))
                }),
        );
    }
    for entity in finished_entities {
        world.entity_mut(entity).remove::<UiStyleTransitionState>();
    }
    if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
        for entity in resolve_entities {
            dirty.mark(UiDirtyDomain::Style, entity);
        }
        for (entity, _) in painted_entities {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
}

fn paint_only_transition(property: UiStyleProperty) -> bool {
    matches!(
        property,
        UiStyleProperty::Background
            | UiStyleProperty::BorderColor
            | UiStyleProperty::AccentColor
            | UiStyleProperty::SelectionColor
            | UiStyleProperty::FocusColor
            | UiStyleProperty::DisabledColor
            | UiStyleProperty::ErrorColor
            | UiStyleProperty::IconColor
            | UiStyleProperty::MutedIconColor
            | UiStyleProperty::DisabledIconColor
            | UiStyleProperty::BorderWidth
            | UiStyleProperty::CornerRadius
            | UiStyleProperty::Opacity
    )
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

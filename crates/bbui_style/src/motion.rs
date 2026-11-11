use super::*;
use bbui_motion::{
    Keyframe, KeyframeTrack, MotionBinding, MotionClip, MotionPlayer, MotionPriority, MotionValue,
};

impl MotionValue for Rgba {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Option<Self> {
        Some(from.mix(*to, progress))
    }

    fn is_valid(&self) -> bool {
        (*self).is_finite()
    }
}

impl MotionValue for UiStyleValue {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Option<Self> {
        interpolate_style_value(from.clone(), to.clone(), progress)
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Color(color) => color.is_finite(),
            Self::Scalar(value) => value.is_finite(),
            Self::Length(
                UiLength::Px(value) | UiLength::Percent(value) | UiLength::Fill(value),
            ) => value.is_finite(),
            Self::Edges(edges) => [edges.left, edges.top, edges.right, edges.bottom]
                .into_iter()
                .all(f32::is_finite),
            Self::BoxShadow(shadows) => shadows.shadows.iter().all(|shadow| {
                [
                    shadow.x_offset,
                    shadow.y_offset,
                    shadow.spread_radius,
                    shadow.blur_radius,
                ]
                .into_iter()
                .all(f32::is_finite)
                    && match shadow.color {
                        UiShadowColor::Color(color) => color.is_finite(),
                        UiShadowColor::Token(_) => true,
                    }
            }),
            _ => true,
        }
    }
}

/// Stable typed binding for one computed style property. Applications can use
/// this in explicit keyframe clips as well as automatic style animations.
pub fn style_motion_binding(property: UiStyleProperty) -> MotionBinding<UiStyleValue> {
    MotionBinding::with_id(
        property as u64 + 1,
        move |world, entity| world.get::<UiComputedStyle>(entity)?.get(property),
        move |world, entity, value| {
            let Some(mut computed) = world.get_mut::<UiComputedStyle>(entity) else {
                return;
            };
            computed.properties.insert(property, value);
            let changed = computed.last_changed();
            if let Some(mut outputs) = world.get_resource_mut::<UiStyleMotionOutputs>() {
                outputs.0.insert(entity, changed);
            }
            if let Some(mut paint_outputs) = world.get_resource_mut::<UiStylePaintOnlyOutputs>() {
                let entry = paint_outputs.0.entry(entity).or_insert((changed, true));
                entry.0 = changed;
                entry.1 &= property == UiStyleProperty::Background;
            }
            let layout_property = matches!(
                property,
                UiStyleProperty::Width
                    | UiStyleProperty::Height
                    | UiStyleProperty::MinWidth
                    | UiStyleProperty::MinHeight
                    | UiStyleProperty::MaxWidth
                    | UiStyleProperty::MaxHeight
                    | UiStyleProperty::Padding
                    | UiStyleProperty::Margin
                    | UiStyleProperty::Gap
                    | UiStyleProperty::LayoutMode
                    | UiStyleProperty::AlignItems
                    | UiStyleProperty::JustifyContent
                    | UiStyleProperty::ClipContents
                    | UiStyleProperty::ZIndex
            );
            if (layout_property || property == UiStyleProperty::BoxShadow)
                && let Some(computed) = world.get::<UiComputedStyle>(entity).cloned()
            {
                if layout_property {
                    apply_layout_style(world, entity, &computed);
                }
                if property == UiStyleProperty::BoxShadow {
                    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
                    apply_box_shadow_style(world, entity, &computed, &theme);
                }
            }
            let mut dirty = world.resource_mut::<UiDirty>();
            dirty.mark(UiDirtyDomain::Paint, entity);
            if layout_property {
                dirty.mark(UiDirtyDomain::Layout, entity);
            }
            if matches!(
                property,
                UiStyleProperty::FontSize | UiStyleProperty::LineHeight
            ) {
                dirty.mark(UiDirtyDomain::TextMeasure, entity);
                dirty.mark(UiDirtyDomain::TextLayout, entity);
            }
        },
    )
    .on_release(|world, entity| {
        if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
            dirty.mark(UiDirtyDomain::Style, entity);
        }
    })
}

pub(super) fn sync_motion_settings(
    theme: Res<Theme>,
    reduced: Res<UiReducedMotion>,
    mut player: ResMut<MotionPlayer>,
) {
    player.set_reduced_motion(reduced.0);
    player.set_time_scale(if theme.motion_scale > 0.0 {
        1.0 / theme.motion_scale
    } else {
        0.0
    });
}

pub(super) fn begin_motion_frame(mut player: ResMut<MotionPlayer>) {
    player.begin_frame();
}

pub(super) fn clear_style_motion_paint_outputs(mut outputs: ResMut<UiStylePaintOnlyOutputs>) {
    outputs.clear();
}

pub(super) fn resolve_animated_style(
    world: &mut World,
    entity: Entity,
    current: &UiComputedStyle,
    target: &UiComputedStyle,
    animations: &UiAutoAnimations,
    reduced_motion: bool,
) -> UiComputedStyle {
    world.init_resource::<MotionPlayer>();
    world.init_resource::<UiStyleMotionOutputs>();
    world.init_resource::<UiStylePaintOnlyOutputs>();
    let mut result = target.clone();
    for (&property, target_value) in &target.properties {
        let binding = style_motion_binding(property);
        let current_value = current.get(property);
        let Some(from) = current_value else {
            continue;
        };
        if world
            .resource::<MotionPlayer>()
            .has_explicit(entity, binding.id())
        {
            result.properties.insert(property, from);
            continue;
        }
        if from == *target_value {
            if world
                .resource::<MotionPlayer>()
                .is_animating(entity, binding.id())
                && !world
                    .resource::<MotionPlayer>()
                    .targets(entity, &binding, target_value)
            {
                world
                    .resource_mut::<MotionPlayer>()
                    .cancel_automatic_binding(entity, binding.id());
            }
            continue;
        }
        if reduced_motion || !world.contains_resource::<Time>() {
            world
                .resource_mut::<MotionPlayer>()
                .cancel_automatic_binding(entity, binding.id());
            continue;
        }
        let player = world.resource::<MotionPlayer>();
        if player.targets(entity, &binding, target_value) {
            result.properties.insert(property, from);
            continue;
        }
        let Some(spec) = animations
            .0
            .get(&property)
            .copied()
            .filter(|spec| !spec.duration.is_zero())
        else {
            world
                .resource_mut::<MotionPlayer>()
                .cancel_automatic_binding(entity, binding.id());
            continue;
        };
        if interpolate_style_value(from.clone(), target_value.clone(), 0.0).is_none() {
            continue;
        }
        let frames = KeyframeTrack::new([
            Keyframe::at(Duration::ZERO, from.clone()).curve(spec.curve),
            Keyframe::at(spec.duration, target_value.clone()),
        ]);
        if let Ok(frames) = frames {
            let clip = MotionClip::new().track(binding, frames);
            let deferred = !world.resource::<MotionPlayer>().advanced_this_frame();
            if deferred {
                world.resource_mut::<MotionPlayer>().play_deferred(
                    entity,
                    clip,
                    MotionPriority::Automatic,
                );
            } else {
                world
                    .resource_mut::<MotionPlayer>()
                    .play(entity, clip, MotionPriority::Automatic);
            }
            result.properties.insert(property, from);
        }
    }
    for &property in current.properties.keys() {
        if !target.properties.contains_key(&property) {
            world
                .resource_mut::<MotionPlayer>()
                .cancel_automatic_binding(entity, style_motion_binding(property).id());
        }
    }
    result
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

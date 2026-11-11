use super::*;

pub(super) struct RetainedCheckboxContext<'a> {
    pub rect: Rect,
    pub clip: ClipRect,
    pub z: i32,
    pub theme: &'a Theme,
    pub effective_enabled: bool,
    pub order: u64,
    pub camera: Option<Entity>,
}

pub(super) fn checkbox_static_stamp(
    world: &World,
    entity: Entity,
    effective_enabled: bool,
    order: u64,
    camera: Option<Entity>,
) -> Option<CheckboxPaintStaticStamp> {
    if world.get::<Checkbox>(entity).is_none()
        || world.get::<UiRole>(entity).is_some()
        || world
            .get::<Tooltip>(entity)
            .is_some_and(|tooltip| tooltip.visible)
    {
        return None;
    }
    let reference = world.get_entity(entity).ok()?;
    Some(CheckboxPaintStaticStamp {
        checkbox: reference.get_change_ticks::<Checkbox>()?.changed,
        layout: reference.get_change_ticks::<UiLayoutResult>()?.changed,
        theme: world.get_resource_change_ticks::<Theme>()?.changed,
        effective_enabled,
        order,
        camera,
    })
}

/// Patch the only changing primitive of an ordinary checkbox. Exact style
/// output ticks and the static stamp guard the inputs that require repainting.
pub(super) fn recolor_retained_checkbox(
    world: &mut World,
    entity: Entity,
    context: RetainedCheckboxContext<'_>,
) -> Option<bool> {
    let RetainedCheckboxContext {
        rect,
        clip,
        z,
        theme,
        effective_enabled,
        order,
        camera,
    } = context;
    let reference = world.get_entity(entity).ok()?;
    let style_tick = reference.get_change_ticks::<UiComputedStyle>()?.changed;
    if !world
        .get_resource::<UiStylePaintOnlyOutputs>()?
        .background_only_at(entity, style_tick)
    {
        return None;
    }
    let checkbox = world.get::<Checkbox>(entity)?;
    let disabled = !(checkbox.enabled && effective_enabled);
    let box_size = rect.width.min(rect.height).max(0.0);
    let box_rect = ClipRect {
        x: rect.x,
        y: rect.y,
        width: box_size,
        height: rect.height,
    };
    let mark_count = match checkbox.value {
        CheckboxValue::Off => 0,
        CheckboxValue::Mixed => 1,
        CheckboxValue::On => 2,
    };
    let expected_len = 1 + mark_count + usize::from(!checkbox.label.is_empty());
    let display = world.resource::<DisplayList>();
    let fragment = display.widget_fragments().get(&entity)?;
    if fragment.order != order
        || fragment.target_camera != camera
        || fragment.items.len() != expected_len
    {
        return None;
    }
    let first = fragment.items.first()?;
    if first.clip != clip || first.z != z {
        return None;
    }
    let Primitive::RoundedRect {
        rect: retained_rect,
        radius,
        ..
    } = first.primitive
    else {
        return None;
    };
    if retained_rect != box_rect || radius != (box_size * 0.15).max(0.0) {
        return None;
    }
    let mark_color = if disabled {
        theme.color(ThemeColorToken::ControlDisabledMark)
    } else {
        theme.accent
    };
    let inset = (box_size * 0.24).max(1.0);
    let mark_z = z.saturating_add(1);
    match checkbox.value {
        CheckboxValue::Off => {}
        CheckboxValue::Mixed => {
            if !retained_line_matches(
                &fragment.items[1],
                [rect.x + inset, rect.y + rect.height * 0.5],
                [rect.x + box_size - inset, rect.y + rect.height * 0.5],
                (box_size * 0.12).max(1.0),
                mark_color,
                clip,
                mark_z,
            ) {
                return None;
            }
        }
        CheckboxValue::On => {
            let left = [rect.x + inset, rect.y + rect.height * 0.52];
            let bend = [rect.x + box_size * 0.43, rect.y + rect.height - inset];
            let tip = [rect.x + box_size - inset * 0.7, rect.y + inset];
            let width = (box_size * 0.12).max(1.0);
            if !retained_line_matches(
                &fragment.items[1],
                left,
                bend,
                width,
                mark_color,
                clip,
                mark_z,
            ) || !retained_line_matches(
                &fragment.items[2],
                bend,
                tip,
                width,
                mark_color,
                clip,
                mark_z,
            ) {
                return None;
            }
        }
    }
    if !checkbox.label.is_empty() {
        let label = fragment.items.last()?;
        let expected_color = if disabled {
            theme.color(ThemeColorToken::ControlDisabledText)
        } else {
            theme.text
        };
        let expected_origin = [
            rect.x + box_size + theme.spacing.control.checkbox_label_gap,
            rect.y,
        ];
        let Primitive::Text {
            origin,
            text,
            color,
        } = &label.primitive
        else {
            return None;
        };
        if text != &checkbox.label
            || *origin != expected_origin
            || *color != expected_color
            || label.clip != clip
            || label.z != z.saturating_add(2)
        {
            return None;
        }
    }
    let color = if disabled {
        theme.color(ThemeColorToken::ControlDisabledSurface)
    } else {
        theme.surface
    };
    world
        .resource_mut::<DisplayList>()
        .recolor_widget_fragment_rounded_rect(entity, 0, color)
}

fn retained_line_matches(
    item: &DrawItem,
    from: [f32; 2],
    to: [f32; 2],
    width: f32,
    color: Rgba,
    clip: ClipRect,
    z: i32,
) -> bool {
    item.clip == clip
        && item.z == z
        && matches!(item.primitive, Primitive::Line {
            from: actual_from,
            to: actual_to,
            width: actual_width,
            color: actual_color,
        } if actual_from == from && actual_to == to && actual_width == width && actual_color == color)
}

use super::*;

pub(in crate::actions) fn item(clip: ClipRect, z: i32, primitive: Primitive) -> DrawItem {
    DrawItem { clip, z, primitive }
}

pub(in crate::actions) fn push_rounded_outline(
    rect: Rect,
    radius: f32,
    color: Rgba,
    clip: ClipRect,
    z: i32,
    output: &mut Vec<DrawItem>,
) {
    output.push(item(
        clip,
        z,
        Primitive::RoundedBorder {
            rect: ClipRect::from(rect),
            radius,
            width: 1.0,
            color,
        },
    ));
}

/// Paint the shared surface used by command menus and choice dropdowns.
pub(in crate::actions) fn push_popup_surface(
    rect: Rect,
    clip: ClipRect,
    shadow_clip: ClipRect,
    z: i32,
    theme: &Theme,
    base_theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    push_popup_shadow(
        rect,
        theme.corner_radius,
        theme.surface,
        shadow_clip,
        z.saturating_sub(1),
        theme,
        output,
    );
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    push_rounded_outline(
        rect,
        theme.corner_radius,
        base_theme.color(ThemeColorToken::MenuBorder),
        clip,
        z.saturating_add(12),
        output,
    );
}

pub(in crate::actions) fn popup_viewport_clip(
    world: &World,
    window: Entity,
    parent_clip: Option<Rect>,
    fallback: Rect,
) -> ClipRect {
    let viewport = world.iter_entities().find_map(|entity| {
        entity
            .get::<UiLayoutRoot>()
            .filter(|root| root.window == window)
            .map(|root| root.viewport)
    });
    let clipped = match (parent_clip, viewport) {
        (Some(parent), Some(viewport)) => parent.intersection(viewport).unwrap_or(Rect {
            x: parent.x,
            y: parent.y,
            width: 0.0,
            height: 0.0,
        }),
        (Some(parent), None) => parent,
        (None, Some(viewport)) => viewport,
        (None, None) => fallback,
    };
    ClipRect::from(clipped)
}

pub(in crate::actions) fn push_popup_shadow(
    rect: Rect,
    radius: f32,
    fill: Rgba,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    for shadow in &theme.effects.popup_shadow.shadows {
        let color = match shadow.color {
            bbui_style::UiShadowColor::Color(color) => color,
            bbui_style::UiShadowColor::Token(token) => theme.color(token),
        };
        if !radius.is_finite()
            || !shadow.x_offset.is_finite()
            || !shadow.y_offset.is_finite()
            || !shadow.spread_radius.is_finite()
            || !shadow.blur_radius.is_finite()
            || shadow.blur_radius < 0.0
        {
            continue;
        }
        output.push(item(
            clip,
            z,
            Primitive::Shadow {
                rect: ClipRect::from(rect),
                radius: radius.max(0.0),
                color: fill,
                shadow: color,
                offset: [shadow.x_offset, shadow.y_offset],
                blur: shadow.blur_radius,
                spread: shadow.spread_radius,
            },
        ));
    }
}

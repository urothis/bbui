use super::*;

pub(in crate::actions) fn item(clip: ClipRect, z: i32, primitive: Primitive) -> DrawItem {
    DrawItem { clip, z, primitive }
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
            bui_style::UiShadowColor::Color(color) => color,
            bui_style::UiShadowColor::Token(token) => theme.color(token),
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

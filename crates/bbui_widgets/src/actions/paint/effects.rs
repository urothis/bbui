use super::*;

fn fixed_color_opacity(paint: &PaintEntityContext<'_>) -> f32 {
    paint
        .world
        .get::<UiComputedStyle>(paint.entity)
        .and_then(|style| style.scalar(UiStyleProperty::Opacity))
        .filter(|opacity| opacity.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}

fn fade_fixed_colors(colors: &mut [UiEffectColor], opacity: f32) {
    for color in colors {
        if let UiEffectColor::Fixed(value) = color {
            value.3 *= opacity;
        }
    }
}

pub(super) fn paint_edge_glow(paint: &mut PaintEntityContext<'_>) {
    let Some(effects) = paint.world.get::<UiVisualEffects>(paint.entity) else {
        return;
    };
    let Some(glow) = effects.edge_glow.as_ref() else {
        return;
    };
    let radius = effects
        .border
        .as_ref()
        .map_or(paint.theme.corner_radius, |border| border.radius.max(0.0));
    let mut glow = glow.clone();
    fade_fixed_colors(&mut glow.colors, fixed_color_opacity(paint));
    push_effect_glow(
        paint.output,
        paint.layout.clip.map(ClipRect::from),
        paint.z,
        ClipRect::from(paint.rect),
        radius,
        &glow,
        paint.theme,
    );
}

pub(super) fn push_effect_glow(
    output: &mut Vec<DrawItem>,
    clip: Option<ClipRect>,
    z: i32,
    rect: ClipRect,
    radius: f32,
    glow: &crate::UiEdgeGlow,
    theme: &Theme,
) {
    if glow.colors.is_empty()
        || !glow.blur.is_finite()
        || glow.blur < 0.0
        || !glow.spread.is_finite()
        || !glow.strength.is_finite()
        || !glow.phase.is_finite()
    {
        return;
    }
    let colors: Vec<_> = glow
        .colors
        .iter()
        .map(|color| {
            let mut resolved = color.resolve(theme);
            resolved.3 *= glow.strength.clamp(0.0, 1.0);
            resolved
        })
        .collect();
    if colors.iter().any(|color| !color.is_finite()) || colors.iter().all(|color| color.3 <= 0.0) {
        return;
    }
    let reach = (glow.blur * 1.8 + glow.spread.max(0.0) + 4.0).max(0.0);
    let fallback_clip = ClipRect {
        x: rect.x - reach,
        y: rect.y - reach,
        width: rect.width + 2.0 * reach,
        height: rect.height + 2.0 * reach,
    };
    let clip = clip.unwrap_or(fallback_clip);
    if colors.len() == 1 {
        output.push(item(
            clip,
            z,
            Primitive::Shadow {
                rect,
                radius,
                color: Rgba::TRANSPARENT,
                shadow: colors[0],
                offset: [0.0, 0.0],
                blur: glow.blur,
                spread: glow.spread,
            },
        ));
        return;
    }

    // Native border gradients provide the same moving palette as the border.
    // Thin translucent rings make its falloff without a separate shader path.
    const RINGS: usize = 10;
    let step = (glow.blur * 1.8 / RINGS as f32).max(0.5);
    for ring in (0..RINGS).rev() {
        let distance = glow.spread + (ring as f32 + 1.0) * step;
        let rect = ClipRect {
            x: rect.x - distance,
            y: rect.y - distance,
            width: rect.width + distance * 2.0,
            height: rect.height + distance * 2.0,
        };
        if rect.width <= 0.0 || rect.height <= 0.0 {
            continue;
        }
        let falloff = 1.0 - (ring as f32 + 0.5) / RINGS as f32;
        let colors = colors
            .iter()
            .map(|color| {
                Rgba(
                    color.0,
                    color.1,
                    color.2,
                    color.3 * falloff * falloff * 0.35,
                )
            })
            .collect();
        output.push(item(
            clip,
            z,
            Primitive::GradientBorder {
                rect,
                radius: (radius + distance).max(0.0),
                width: step * 2.5,
                colors,
                phase: glow.phase,
            },
        ));
    }
}

pub(super) fn paint_gradient_border(paint: &mut PaintEntityContext<'_>) {
    let Some(border) = paint
        .world
        .get::<UiVisualEffects>(paint.entity)
        .and_then(|effects| effects.border.as_ref())
    else {
        return;
    };
    let mut border = border.clone();
    fade_fixed_colors(&mut border.colors, fixed_color_opacity(paint));
    push_effect_border(
        paint.output,
        paint.clip,
        paint.z.saturating_add(20),
        ClipRect::from(paint.rect),
        &border,
        paint.theme,
    );
}

pub(super) fn push_effect_border(
    output: &mut Vec<DrawItem>,
    clip: ClipRect,
    z: i32,
    rect: ClipRect,
    border: &crate::UiBorderEffect,
    theme: &Theme,
) {
    if border.colors.is_empty()
        || !border.width.is_finite()
        || border.width <= 0.0
        || !border.radius.is_finite()
        || !border.phase.is_finite()
        || !border.strength.is_finite()
    {
        return;
    }
    let colors: Vec<_> = border
        .colors
        .iter()
        .map(|color| {
            let mut resolved = color.resolve(theme);
            resolved.3 *= border.strength.clamp(0.0, 1.0);
            resolved
        })
        .collect();
    if colors.iter().any(|color| !color.is_finite()) {
        return;
    }
    let primitive = if colors.len() == 1 {
        Primitive::RoundedBorder {
            rect,
            radius: border.radius.max(0.0),
            width: border.width,
            color: colors[0],
        }
    } else {
        Primitive::GradientBorder {
            rect,
            radius: border.radius.max(0.0),
            width: border.width,
            colors,
            phase: border.phase,
        }
    };
    output.push(item(clip, z, primitive));
}

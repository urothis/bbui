use super::*;

pub(super) fn paint_edge_glow(paint: &mut PaintEntityContext<'_>) {
    let Some(effects) = paint.world.get::<UiVisualEffects>(paint.entity) else {
        return;
    };
    let Some(glow) = effects.edge_glow.as_ref() else {
        return;
    };
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
            let mut resolved = color.resolve(paint.theme);
            resolved.3 *= glow.strength.clamp(0.0, 1.0);
            resolved
        })
        .collect();
    if colors.iter().any(|color| !color.is_finite()) || colors.iter().all(|color| color.3 <= 0.0) {
        return;
    }
    let reach = (glow.blur * 1.8 + glow.spread.max(0.0) + 4.0).max(0.0);
    let fallback_clip = ClipRect {
        x: paint.rect.x - reach,
        y: paint.rect.y - reach,
        width: paint.rect.width + 2.0 * reach,
        height: paint.rect.height + 2.0 * reach,
    };
    let clip = paint
        .layout
        .clip
        .map(ClipRect::from)
        .unwrap_or(fallback_clip);
    let radius = effects
        .border
        .as_ref()
        .map_or(paint.theme.corner_radius, |border| border.radius.max(0.0));
    if colors.len() == 1 {
        paint.output.push(item(
            clip,
            paint.z,
            Primitive::Shadow {
                rect: ClipRect::from(paint.rect),
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
            x: paint.rect.x - distance,
            y: paint.rect.y - distance,
            width: paint.rect.width + distance * 2.0,
            height: paint.rect.height + distance * 2.0,
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
        paint.output.push(item(
            clip,
            paint.z,
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
            let mut resolved = color.resolve(paint.theme);
            resolved.3 *= border.strength.clamp(0.0, 1.0);
            resolved
        })
        .collect();
    if colors.iter().any(|color| !color.is_finite()) {
        return;
    }
    let primitive = if colors.len() == 1 {
        Primitive::RoundedBorder {
            rect: ClipRect::from(paint.rect),
            radius: border.radius.max(0.0),
            width: border.width,
            color: colors[0],
        }
    } else {
        Primitive::GradientBorder {
            rect: ClipRect::from(paint.rect),
            radius: border.radius.max(0.0),
            width: border.width,
            colors,
            phase: border.phase,
        }
    };
    paint
        .output
        .push(item(paint.clip, paint.z.saturating_add(20), primitive));
}

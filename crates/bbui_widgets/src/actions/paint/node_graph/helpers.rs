use super::*;

pub(super) fn split_node_graph_bezier(points: [[f32; 2]; 4]) -> ([[f32; 2]; 4], [[f32; 2]; 4]) {
    let mid = |a: [f32; 2], b: [f32; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let a = mid(points[0], points[1]);
    let b = mid(points[1], points[2]);
    let c = mid(points[2], points[3]);
    let d = mid(a, b);
    let e = mid(b, c);
    let center = mid(d, e);
    ([points[0], a, d, center], [center, e, c, points[3]])
}

pub(in crate::actions) fn node_graph_text_primitive(
    placement: TextPlacement,
    color: Rgba,
    max_width: f32,
    justify: bevy_text::Justify,
    font: &TextFont,
    theme: &Theme,
    zoom: f32,
) -> Primitive {
    Primitive::TextWrapped {
        origin: placement.origin,
        text: placement.text,
        color,
        max_width: max_width.max(1.0),
        wrap: false,
        style: Some(TextRenderStyle {
            font: Some(font.clone().with_font_size(theme.font_size * zoom)),
            line_height: Some(LineHeight::RelativeToFont(theme.line_height)),
            letter_spacing: bevy_text::LetterSpacing::default(),
            justify,
            line_break: LineBreak::NoWrap,
        }),
    }
}

pub(in crate::actions) fn node_graph_category_color(category: NodeCategory, theme: &Theme) -> Rgba {
    let surface = theme.color(ThemeColorToken::NodeGraphHeaderSurface);
    let hue = match category {
        NodeCategory::General => return surface,
        NodeCategory::Input | NodeCategory::Converter => {
            theme.color(ThemeColorToken::NodeGraphTextSocket)
        }
        NodeCategory::Texture => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        NodeCategory::Color | NodeCategory::Output => {
            theme.color(ThemeColorToken::NodeGraphColorSocket)
        }
        NodeCategory::Geometry => theme.color(ThemeColorToken::NodeGraphVectorSocket),
    };
    surface.mix(hue, 0.5)
}

pub(in crate::actions) fn node_graph_canvas_surface(
    world: &World,
    entity: Entity,
    theme: &Theme,
) -> Rgba {
    let token_surface = theme.color(ThemeColorToken::NodeGraphSurface);
    let inline_background = world
        .get::<UiInlineStyle>(entity)
        .and_then(|style| style.0.get(&UiStyleProperty::Background));
    if !node_graph_has_background_override(world, entity) {
        return token_surface;
    }
    world
        .get::<UiComputedStyle>(entity)
        .and_then(|style| style.color(UiStyleProperty::Background))
        .or_else(|| match inline_background {
            Some(UiStyleValue::Color(color)) => Some(*color),
            Some(UiStyleValue::ColorToken(token)) => Some(theme.color(*token)),
            Some(_) | None => None,
        })
        .unwrap_or(token_surface)
}

pub(in crate::actions) fn node_graph_has_background_override(
    world: &World,
    entity: Entity,
) -> bool {
    node_graph_has_style_override(world, entity, UiStyleProperty::Background)
}

pub(in crate::actions) fn node_graph_has_border_override(world: &World, entity: Entity) -> bool {
    node_graph_has_style_override(world, entity, UiStyleProperty::BorderColor)
}

fn node_graph_has_style_override(world: &World, entity: Entity, property: UiStyleProperty) -> bool {
    if world
        .get::<UiInlineStyle>(entity)
        .is_some_and(|style| style.0.contains_key(&property))
    {
        return true;
    }
    let role = world.get::<UiRole>(entity);
    let classes = world.get::<UiClasses>(entity);
    let mut state = world
        .get::<UiStyleState>(entity)
        .copied()
        .unwrap_or_default();
    if world
        .get::<UiEnabled>(entity)
        .is_some_and(|enabled| !enabled.0)
    {
        state = state.union(UiStyleState::DISABLED);
    }
    if let Some(focus) = world.get_resource::<UiFocus>() {
        for window_state in focus.windows.values() {
            if window_state.hovered == Some(entity) {
                state = state.union(UiStyleState::HOVERED);
            }
            if window_state.captured == Some(entity) {
                state = state.union(UiStyleState::PRESSED);
            }
            if window_state.focused == Some(entity) {
                state = state.union(UiStyleState::FOCUSED);
            }
        }
    }
    world.get_resource::<UiStyleSheet>().is_some_and(|sheet| {
        sheet.rules.iter().any(|rule| {
            let selector = &rule.selector;
            rule.declaration.0.contains_key(&property)
                && selector
                    .role
                    .as_ref()
                    .is_none_or(|wanted| role.is_some_and(|actual| &actual.0 == wanted))
                && selector
                    .class
                    .as_ref()
                    .is_none_or(|wanted| classes.is_some_and(|actual| actual.contains(wanted)))
                && state.contains(selector.required_state)
                && !state.intersects(selector.forbidden_state)
        })
    })
}

pub(in crate::actions) fn cached_node_graph_output_label_widths(
    world: &mut World,
    owner: Entity,
    node_id: NodeId,
    labels: &[(usize, String)],
    font: &TextFont,
    theme: &Theme,
    zoom: f32,
) -> HashMap<(NodeId, usize), f32> {
    let layout_available = world.contains_resource::<TextPipeline>()
        && world.contains_resource::<Assets<Font>>()
        && world.contains_resource::<FontCx>()
        && world.contains_resource::<LayoutCx>();
    let signature = format!(
        "font={font:?};size={:08x};line={:08x};zoom={:08x};available={layout_available};labels={labels:?}",
        theme.font_size.to_bits(),
        theme.line_height.to_bits(),
        zoom.to_bits(),
    );
    if let Some(measurements) = world
        .resource::<WidgetPaintCache>()
        .node_graph_output_measurements
        .get(&owner)
        .and_then(|nodes| nodes.get(&node_id))
        .filter(|measurements| measurements.signature == signature)
    {
        return measurements
            .widths
            .iter()
            .map(|(index, width)| ((node_id, *index), *width))
            .collect();
    }

    let widths = if labels.is_empty() || !layout_available {
        HashMap::new()
    } else {
        let font = font
            .clone()
            .with_font_size(theme.font_size * zoom.max(0.01));
        world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
            world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
                world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                    world.resource_scope(|_, mut layout_cx: Mut<LayoutCx>| {
                        labels
                            .iter()
                            .filter_map(|(index, text)| {
                                let layout = TextLayout::shape_with_bevy(
                                    text,
                                    f32::INFINITY,
                                    &font,
                                    &mut pipeline,
                                    &fonts,
                                    &mut font_cx,
                                    &mut layout_cx,
                                )
                                .ok()?;
                                Some((*index, layout.bounds().width.max(0.0)))
                            })
                            .collect()
                    })
                })
            })
        })
    };
    world
        .resource_mut::<WidgetPaintCache>()
        .node_graph_output_measurements
        .entry(owner)
        .or_default()
        .insert(
            node_id,
            NodeGraphOutputLabelMeasurements {
                signature,
                widths: widths.clone(),
            },
        );
    widths
        .into_iter()
        .map(|(index, width)| ((node_id, index), width))
        .collect()
}

pub(in crate::actions) fn socket_type_color(
    socket_type: &crate::SocketType,
    theme: &Theme,
    registry: Option<&NodeGraphTypeRegistry>,
) -> Rgba {
    if let Some(color) = registry.and_then(|registry| registry.color(socket_type)) {
        return color;
    }
    match socket_type {
        crate::SocketType::Any => theme.text_muted,
        crate::SocketType::Boolean => theme.color(ThemeColorToken::NodeGraphBooleanSocket),
        crate::SocketType::Integer => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        crate::SocketType::Float => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        crate::SocketType::Vector2 | crate::SocketType::Vector3 => {
            theme.color(ThemeColorToken::NodeGraphVectorSocket)
        }
        crate::SocketType::Color => theme.color(ThemeColorToken::NodeGraphColorSocket),
        crate::SocketType::Text => theme.color(ThemeColorToken::NodeGraphTextSocket),
        crate::SocketType::Custom(_) => theme.text_muted,
    }
}

pub(in crate::actions) fn push_rect_outline(
    rect: Rect,
    color: Rgba,
    clip: ClipRect,
    z: i32,
    output: &mut Vec<DrawItem>,
) {
    push_rect_outline_with_width(rect, color, clip, z, 1.0, output);
}

pub(in crate::actions) fn push_rect_outline_with_width(
    rect: Rect,
    color: Rgba,
    clip: ClipRect,
    z: i32,
    width: f32,
    output: &mut Vec<DrawItem>,
) {
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    for (from, to) in [
        ([left, top], [right, top]),
        ([right, top], [right, bottom]),
        ([right, bottom], [left, bottom]),
        ([left, bottom], [left, top]),
    ] {
        output.push(item(
            clip,
            z,
            Primitive::Line {
                from,
                to,
                width,
                color,
            },
        ));
    }
}

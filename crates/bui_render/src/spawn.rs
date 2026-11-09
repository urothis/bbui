use super::*;

pub(super) fn same_hsv_wheel_geometry(first: &DrawItem, second: &DrawItem) -> bool {
    if first.clip != second.clip || first.z != second.z {
        return false;
    }
    matches!(
        (&first.primitive, &second.primitive),
        (
            Primitive::HsvWheel { rect: first, .. },
            Primitive::HsvWheel { rect: second, .. }
        ) if first == second
    )
}

pub(super) fn uses_theme_font_size(primitive: &Primitive) -> bool {
    match primitive {
        Primitive::Text { .. } => true,
        Primitive::TextWrapped { style, .. } => style
            .as_ref()
            .and_then(|style| style.font.as_ref())
            .is_none(),
        _ => false,
    }
}

pub(super) fn uses_theme_line_height(primitive: &Primitive) -> bool {
    match primitive {
        Primitive::Text { .. } => true,
        Primitive::TextWrapped { style, .. } => {
            style.as_ref().and_then(|style| style.line_height).is_none()
        }
        _ => false,
    }
}

pub(super) fn spawn_item_node(
    parent: &mut ChildSpawnerCommands<'_>,
    bundle: impl bevy::ecs::bundle::Bundle,
    z: i32,
    local_z: i32,
    mark_item: bool,
) -> Entity {
    if mark_item {
        parent
            .spawn((bundle, GlobalZIndex(z), ZIndex(local_z), RenderedItem))
            .id()
    } else {
        parent
            .spawn((bundle, GlobalZIndex(z), ZIndex(local_z)))
            .id()
    }
}

pub(super) fn spawn_primitive(
    parent: &mut ChildSpawnerCommands<'_>,
    item: &DrawItem,
    images: &ImageRegistry,
    context: PrimitiveRenderContext<'_>,
    output: &mut RenderedNodes,
) {
    match &item.primitive {
        Primitive::RoundedRect {
            rect,
            radius,
            color,
        } if valid_clip(*rect) => {
            let mut node = positioned_node(*rect, item.clip);
            node.border_radius = BorderRadius::all(Val::Px(radius.max(0.0)));
            output.push(spawn_item_node(
                parent,
                (node, BackgroundColor(color_to_bevy(*color))),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Ellipse { rect, color } if valid_clip(*rect) => {
            let mut node = positioned_node(*rect, item.clip);
            node.border_radius = BorderRadius::all(Val::Percent(50.0));
            output.push(spawn_item_node(
                parent,
                (node, BackgroundColor(color_to_bevy(*color))),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Line {
            from,
            to,
            width,
            color,
        } if from.iter().chain(to).all(|value| value.is_finite())
            && width.is_finite()
            && *width > 0.0 =>
        {
            if let Some(entity) = spawn_line(
                parent,
                item.clip,
                *from,
                *to,
                *width,
                *color,
                RenderNodeOrder {
                    global_z: item.z,
                    local_z: context.local_z,
                    mark_item: true,
                },
            ) {
                output.push(entity);
            }
        }
        Primitive::Text {
            origin,
            text,
            color,
        } if origin.iter().all(|value| value.is_finite()) && !text.is_empty() => {
            output.push(spawn_item_node(
                parent,
                (
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(origin[0] - item.clip.x),
                        top: Val::Px(origin[1] - item.clip.y),
                        width: Val::Px((item.clip.width - (origin[0] - item.clip.x)).max(0.0)),
                        ..Default::default()
                    },
                    Text::new(text.clone()),
                    context.default_text_font.clone(),
                    LineHeight::RelativeToFont(context.text_line_height),
                    TextColor(color_to_bevy(*color)),
                ),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::TextWrapped {
            origin,
            text,
            color,
            max_width,
            wrap,
            style,
        } if origin.iter().all(|value| value.is_finite())
            && max_width.is_finite()
            && *max_width > 0.0
            && !text.is_empty() =>
        {
            output.push(spawn_item_node(
                parent,
                (
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(origin[0] - item.clip.x),
                        top: Val::Px(origin[1] - item.clip.y),
                        width: Val::Px(*max_width),
                        ..Default::default()
                    },
                    Text::new(text.clone()),
                    style
                        .as_ref()
                        .and_then(|style| style.font.clone())
                        .unwrap_or_else(|| context.default_text_font.clone()),
                    style
                        .as_ref()
                        .and_then(|style| style.line_height)
                        .unwrap_or(LineHeight::RelativeToFont(context.text_line_height)),
                    style
                        .as_ref()
                        .map_or(LetterSpacing::default(), |style| style.letter_spacing),
                    TextColor(color_to_bevy(*color)),
                    TextBounds::new_horizontal(*max_width),
                    TextLayout::new(
                        style
                            .as_ref()
                            .map_or(Justify::default(), |style| style.justify),
                        style.as_ref().map_or(
                            if *wrap {
                                LineBreak::WordBoundary
                            } else {
                                LineBreak::NoWrap
                            },
                            |style| style.line_break,
                        ),
                    ),
                ),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Image { rect, asset_id } if valid_clip(*rect) => {
            if let Some(image) = images.get(*asset_id) {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(*rect, item.clip),
                        ImageNode::new(image.clone()),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        Primitive::Shadow {
            rect,
            radius,
            color,
            shadow,
            offset,
            blur,
            spread,
        } if valid_clip(*rect)
            && radius.is_finite()
            && offset.iter().all(|value| value.is_finite())
            && blur.is_finite()
            && *blur >= 0.0
            && spread.is_finite() =>
        {
            let mut node = positioned_node(*rect, item.clip);
            node.border_radius = BorderRadius::all(Val::Px(radius.max(0.0)));
            output.push(spawn_item_node(
                parent,
                (
                    node,
                    BackgroundColor(color_to_bevy(*color)),
                    BoxShadow::new(
                        color_to_bevy(*shadow),
                        Val::Px(offset[0]),
                        Val::Px(offset[1]),
                        Val::Px(*spread),
                        Val::Px(*blur),
                    ),
                ),
                item.z,
                context.local_z,
                true,
            ));
        }
        Primitive::Icon {
            rect,
            asset_id,
            tint,
        } if valid_clip(*rect) => {
            if let Some(image) = images.get(*asset_id) {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(*rect, item.clip),
                        ImageNode::new(image.clone()).with_color(color_to_bevy(*tint)),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        Primitive::Bezier { .. } => {
            if let (Some(values), Some(material)) =
                (bezier_material_values(item), context.bezier_material)
            {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(values.rect, item.clip),
                        MaterialNode(material.clone()),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        Primitive::HsvWheel { rect, .. } if valid_clip(*rect) => {
            if let Some(material) = context.hsv_material {
                output.push(spawn_item_node(
                    parent,
                    (
                        positioned_node(*rect, item.clip),
                        MaterialNode(material.clone()),
                    ),
                    item.z,
                    context.local_z,
                    true,
                ));
            }
        }
        _ => {}
    }
}

pub(super) fn spawn_line(
    parent: &mut ChildSpawnerCommands<'_>,
    clip: ClipRect,
    from: [f32; 2],
    to: [f32; 2],
    width: f32,
    color: Rgba,
    order: RenderNodeOrder,
) -> Option<Entity> {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return None;
    }
    let color = BackgroundColor(color_to_bevy(color));
    if dx.abs() <= f32::EPSILON {
        let rect = ClipRect {
            x: (from[0] - width * 0.5).round(),
            y: from[1].min(to[1]),
            width,
            height: length,
        };
        Some(spawn_item_node(
            parent,
            (positioned_node(rect, clip), color),
            order.global_z,
            order.local_z,
            order.mark_item,
        ))
    } else if dy.abs() <= f32::EPSILON {
        let rect = ClipRect {
            x: from[0].min(to[0]),
            y: (from[1] - width * 0.5).round(),
            width: length,
            height: width,
        };
        Some(spawn_item_node(
            parent,
            (positioned_node(rect, clip), color),
            order.global_z,
            order.local_z,
            order.mark_item,
        ))
    } else {
        let rect = ClipRect {
            x: (from[0] + to[0]) * 0.5 - length * 0.5,
            y: (from[1] + to[1]) * 0.5 - width * 0.5,
            width: length,
            height: width,
        };
        Some(spawn_item_node(
            parent,
            (
                positioned_node(rect, clip),
                UiTransform::from_rotation(Rot2::radians(dy.atan2(dx))),
                color,
            ),
            order.global_z,
            order.local_z,
            order.mark_item,
        ))
    }
}

pub(super) fn clip_node(clip: ClipRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(clip.x),
        top: Val::Px(clip.y),
        width: Val::Px(clip.width),
        height: Val::Px(clip.height),
        overflow: Overflow::clip(),
        ..Default::default()
    }
}

pub(super) fn positioned_node(rect: ClipRect, clip: ClipRect) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(rect.x - clip.x),
        top: Val::Px(rect.y - clip.y),
        width: Val::Px(rect.width),
        height: Val::Px(rect.height),
        ..Default::default()
    }
}

pub(super) fn valid_clip(rect: ClipRect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|value| value.is_finite())
        && rect.width > 0.0
        && rect.height > 0.0
}

pub(super) fn color_to_bevy(color: Rgba) -> Color {
    Color::srgba(color.0, color.1, color.2, color.3)
}

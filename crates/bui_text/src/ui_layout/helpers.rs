use super::*;

#[derive(Clone)]
pub(super) struct TextEntity {
    pub(super) entity: Entity,
    pub(super) buffer: TextBuffer,
    pub(super) node: Option<UiLayoutNode>,
    pub(super) result: Option<UiLayoutResult>,
}

pub(super) fn text_entities(
    world: &mut World,
    require_layout: bool,
    subset: Option<&HashSet<Entity>>,
) -> Vec<TextEntity> {
    if let Some(subset) = subset {
        return subset
            .iter()
            .filter_map(|entity| {
                let buffer = world.get::<TextBuffer>(*entity)?;
                world.get::<UiTextLayout>(*entity)?;
                let result = world.get::<UiLayoutResult>(*entity).copied();
                if require_layout && result.is_none() {
                    return None;
                }
                Some(TextEntity {
                    entity: *entity,
                    buffer: buffer.clone(),
                    node: world.get::<UiLayoutNode>(*entity).copied(),
                    result,
                })
            })
            .collect();
    }
    let mut query = world.query::<(Entity, &TextBuffer, &UiTextLayout)>();
    query
        .iter(world)
        .filter_map(|(entity, buffer, _)| {
            let result = world.get::<UiLayoutResult>(entity).copied();
            if require_layout && result.is_none() {
                return None;
            }
            Some(TextEntity {
                entity,
                buffer: buffer.clone(),
                node: world.get::<UiLayoutNode>(entity).copied(),
                result,
            })
        })
        .collect()
}

pub(super) fn intrinsic_width(layout: &UiTextLayout, node: Option<UiLayoutNode>) -> f32 {
    let Some(node) = node else {
        return f32::INFINITY;
    };
    match node.size.width {
        UiLength::Px(width) if layout.multiline && layout.wrap => {
            (width - node.padding.left - node.padding.right - layout.inset[0].max(0.0) * 2.0)
                .max(0.0)
        }
        _ => f32::INFINITY,
    }
}

pub(super) fn intrinsic_size(
    bounds: crate::TextBounds,
    layout: &UiTextLayout,
    node: Option<UiLayoutNode>,
    scale_factor: f32,
) -> [f32; 2] {
    let scale_factor = positive(scale_factor, 1.0);
    let padding = node.map_or([0.0; 2], |node| {
        [
            node.padding.left + node.padding.right,
            node.padding.top + node.padding.bottom,
        ]
    });
    [
        bounds.width / scale_factor + padding[0] + layout.inset[0].max(0.0) * 2.0,
        bounds.height / scale_factor + padding[1] + layout.inset[1].max(0.0) * 2.0,
    ]
}

pub(super) fn ensure_caret_visible(
    layout: &mut UiTextLayout,
    buffer: &TextBuffer,
    viewport: [f32; 2],
) -> bool {
    let Some(shaped) = layout.text_layout.as_ref() else {
        return false;
    };
    let scale = layout.layout_scale_factor();
    let mut caret = shaped.caret_rect_with_affinity(
        layout.display_caret(buffer),
        layout.display_caret_affinity(buffer),
    );
    scale_rect(&mut caret, 1.0 / scale);
    let bounds = shaped.bounds();
    let max_x = (bounds.width / scale.max(0.001)).max(caret.x + caret.width) - viewport[0];
    let max_y = (bounds.height / scale.max(0.001)).max(caret.y + caret.height) - viewport[1];
    let max_x = max_x.max(0.0);
    let max_y = max_y.max(0.0);
    let old = layout.scroll_offset;
    let mut x = finite_nonnegative(old[0]).min(max_x);
    let mut y = finite_nonnegative(old[1]).min(max_y);
    if viewport[0] > 0.0 {
        if caret.x < x {
            x = caret.x.max(0.0);
        } else if caret.x + caret.width > x + viewport[0] {
            x = (caret.x + caret.width - viewport[0]).max(0.0);
        }
    }
    if viewport[1] > 0.0 {
        if caret.y < y {
            y = caret.y.max(0.0);
        } else if caret.y + caret.height > y + viewport[1] {
            y = (caret.y + caret.height - viewport[1]).max(0.0);
        }
    }
    layout.scroll_offset = [x.min(max_x), y.min(max_y)];
    (layout.scroll_offset[0] - old[0]).abs() > 0.01
        || (layout.scroll_offset[1] - old[1]).abs() > 0.01
}

pub(super) fn clamp_scroll_offset(
    layout: &mut UiTextLayout,
    buffer: &TextBuffer,
    viewport: [f32; 2],
) -> bool {
    let Some(shaped) = layout.text_layout.as_ref() else {
        return false;
    };
    let scale = layout.layout_scale_factor();
    let mut caret = shaped.caret_rect_with_affinity(
        layout.display_caret(buffer),
        layout.display_caret_affinity(buffer),
    );
    scale_rect(&mut caret, 1.0 / scale);
    let bounds = shaped.bounds();
    let max_x =
        ((bounds.width / scale.max(0.001)).max(caret.x + caret.width) - viewport[0]).max(0.0);
    let max_y =
        ((bounds.height / scale.max(0.001)).max(caret.y + caret.height) - viewport[1]).max(0.0);
    let old = layout.scroll_offset;
    layout.scroll_offset = [
        finite_nonnegative(old[0]).min(max_x),
        finite_nonnegative(old[1]).min(max_y),
    ];
    (layout.scroll_offset[0] - finite_nonnegative(old[0])).abs() > 0.01
        || (layout.scroll_offset[1] - finite_nonnegative(old[1])).abs() > 0.01
}

pub(super) fn logical_viewport_metrics(
    world: &World,
    entity: Entity,
    result: Option<UiLayoutResult>,
) -> (Vec2, f32) {
    let viewports = world.get_resource::<UiViewports>();
    if let Some(result) = result {
        if let Some(viewport) = viewports.and_then(|viewports| viewports.0.get(&result.window)) {
            return (
                Vec2::new(viewport.rect.width, viewport.rect.height),
                positive(result.dpi_scale, 1.0),
            );
        }
        if let Some(root) = world.get::<UiLayoutRoot>(result.window) {
            return (
                Vec2::new(root.viewport.width, root.viewport.height),
                positive(result.dpi_scale, root.dpi_scale),
            );
        }
        return (Vec2::ZERO, positive(result.dpi_scale, 1.0));
    }
    let mut current = Some(entity);
    while let Some(node) = current {
        if let Some(root) = world.get::<UiLayoutRoot>(node) {
            if let Some(viewport) = viewports.and_then(|viewports| viewports.0.get(&root.window)) {
                return (
                    Vec2::new(viewport.rect.width, viewport.rect.height),
                    positive(viewport.dpi_scale, root.dpi_scale),
                );
            }
            return (
                Vec2::new(root.viewport.width, root.viewport.height),
                positive(root.dpi_scale, 1.0),
            );
        }
        current = world.get::<UiParent>(node).map(|parent| parent.0);
    }
    (Vec2::ZERO, 1.0)
}

pub(super) fn translate(rect: &mut TextRect, origin: [f32; 2]) {
    rect.x += origin[0];
    rect.y += origin[1];
}

pub(super) fn text_origin(
    layout: &UiTextLayout,
    result: UiLayoutResult,
    node: Option<UiLayoutNode>,
) -> [f32; 2] {
    [
        result.rect.x + node.map_or(0.0, |node| node.padding.left) + layout.inset[0].max(0.0)
            - layout.scroll_offset[0].max(0.0),
        result.rect.y + node.map_or(0.0, |node| node.padding.top) + layout.inset[1].max(0.0)
            - layout.scroll_offset[1].max(0.0),
    ]
}

pub(super) fn scale_rect(rect: &mut TextRect, scale: f32) {
    rect.x *= scale;
    rect.y *= scale;
    rect.width *= scale;
    rect.height *= scale;
}

pub(super) fn positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

pub(super) fn set_buffer_caret(
    buffer: &mut TextBuffer,
    target: TextCaretPosition,
    extend: bool,
) -> bool {
    let before = (buffer.anchor(), buffer.caret());
    let after = if extend {
        (buffer.anchor(), target.byte_index)
    } else {
        (target.byte_index, target.byte_index)
    };
    let affinities_after = if extend {
        (buffer.anchor_affinity(), target.affinity)
    } else {
        (target.affinity, target.affinity)
    };
    if before == after && affinities_after == (buffer.anchor_affinity(), buffer.caret_affinity()) {
        return false;
    }
    buffer.set_selection_with_affinity(after.0, after.1, affinities_after.0, affinities_after.1);
    true
}

pub(super) fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

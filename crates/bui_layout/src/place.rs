use super::*;

pub(super) fn distribute_linear(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let horizontal = style.mode == UiLayoutMode::Row;
    let main_extent = if horizontal {
        inner.width
    } else {
        inner.height
    };
    let cross_extent = if horizontal {
        inner.height
    } else {
        inner.width
    };
    let gaps = style.gap * children.len().saturating_sub(1) as f32;
    let mut main_sizes = Vec::with_capacity(children.len());
    let mut fill_weight = 0.0;
    for child in children {
        let child_style = world
            .get::<UiLayoutNode>(*child)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let length = if horizontal {
            child_style.size.width
        } else {
            child_style.size.height
        };
        let measured =
            measured_node_with_available(world, *child, [inner.width, inner.height], measurements);
        let basis = if horizontal { measured[0] } else { measured[1] };
        let (value, weight) = resolve_length(length, main_extent, basis);
        main_sizes.push(value);
        fill_weight += weight;
    }
    let fixed_total: f32 = main_sizes.iter().sum::<f32>() + gaps;
    let remaining = (main_extent - fixed_total).max(0.0);
    if fill_weight > 0.0 {
        for (index, child) in children.iter().enumerate() {
            let child_style = world
                .get::<UiLayoutNode>(*child)
                .copied()
                .unwrap_or_default();
            let length = if horizontal {
                child_style.size.width
            } else {
                child_style.size.height
            };
            if let UiLength::Fill(weight) = length {
                main_sizes[index] += remaining * nonnegative(weight) / fill_weight;
            }
        }
    }
    let used = main_sizes.iter().sum::<f32>() + gaps;
    let free = (main_extent - used).max(0.0);
    let (start_offset, extra_gap) = match style.justify_content {
        UiJustify::Start => (0.0, 0.0),
        UiJustify::Center => (free * 0.5, 0.0),
        UiJustify::End => (free, 0.0),
        UiJustify::SpaceBetween if children.len() > 1 => (0.0, free / (children.len() - 1) as f32),
        UiJustify::SpaceBetween => (0.0, 0.0),
    };
    let mut cursor = if horizontal { inner.x } else { inner.y };
    cursor += start_offset;
    let mut output = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        let child_style = world
            .get::<UiLayoutNode>(*child)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let measured =
            measured_node_with_available(world, *child, [inner.width, inner.height], measurements);
        let main = main_sizes[index];
        let cross_length = if horizontal {
            child_style.size.height
        } else {
            child_style.size.width
        };
        let cross_basis = if horizontal { measured[1] } else { measured[0] };
        let mut cross = resolve_length(cross_length, cross_extent, cross_basis).0;
        if style.align_items == UiAlign::Stretch
            && matches!(cross_length, UiLength::Auto | UiLength::Fill(_))
        {
            cross = cross_extent;
        }
        cross = cross.min(cross_extent).max(0.0);
        let cross_offset = match style.align_items {
            UiAlign::Start | UiAlign::Stretch => 0.0,
            UiAlign::Center => (cross_extent - cross) * 0.5,
            UiAlign::End => cross_extent - cross,
        };
        let rect = if horizontal {
            Rect::new(cursor, inner.y + cross_offset, main, cross)
        } else {
            Rect::new(inner.x + cross_offset, cursor, cross, main)
        };
        output.push(rect);
        cursor += main + style.gap + extra_gap;
    }
    output
}

pub(super) fn distribute_grid(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    columns: usize,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let rows = children.len().div_ceil(columns);
    let col_gap = style.gap;
    let row_gap = style.gap;
    let cell_w =
        ((inner.width - col_gap * columns.saturating_sub(1) as f32).max(0.0)) / columns as f32;
    let row_height =
        (inner.height - row_gap * rows.saturating_sub(1) as f32).max(0.0) / rows as f32;
    children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let col = index % columns;
            let row = index / columns;
            let cell = Rect::new(
                inner.x + col as f32 * (cell_w + col_gap),
                inner.y + row as f32 * (row_height + row_gap),
                cell_w,
                row_height,
            );
            align_child(world, *child, style, cell, measurements)
        })
        .collect()
}

pub(super) fn distribute_inline(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let mut output = Vec::with_capacity(children.len());
    let mut x = inner.x;
    let mut y = inner.y;
    let mut line_height: f32 = 0.0;
    for child in children {
        let size =
            measured_node_with_available(world, *child, [inner.width, inner.height], measurements);
        let margin = world
            .get::<UiLayoutNode>(*child)
            .map_or(UiEdges::ZERO, |node| node.margin.sanitized());
        let width = (size[0] + margin.left + margin.right).min(inner.width);
        let height = size[1] + margin.top + margin.bottom;
        if x > inner.x && x + width > inner.right() {
            x = inner.x;
            y += line_height + style.gap;
            line_height = 0.0;
        }
        output.push(Rect::new(
            x + margin.left,
            y + margin.top,
            (width - margin.left - margin.right).max(0.0),
            size[1],
        ));
        x += width + style.gap;
        line_height = line_height.max(height);
    }
    output
}

pub(super) fn align_child(
    world: &World,
    entity: Entity,
    parent: UiLayoutNode,
    slot: Rect,
    measurements: &mut MeasurementMap,
) -> Rect {
    let style = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let measured =
        measured_node_with_available(world, entity, [slot.width, slot.height], measurements);
    let width = resolve_length(style.size.width, slot.width, measured[0]).0;
    let height = resolve_length(style.size.height, slot.height, measured[1]).0;
    let align_x = match parent.align_items {
        UiAlign::Start | UiAlign::Stretch => 0.0,
        UiAlign::Center => (slot.width - width) * 0.5,
        UiAlign::End => slot.width - width,
    };
    let align_y = match parent.align_items {
        UiAlign::Start | UiAlign::Stretch => 0.0,
        UiAlign::Center => (slot.height - height) * 0.5,
        UiAlign::End => slot.height - height,
    };
    let width = if parent.align_items == UiAlign::Stretch
        && matches!(style.size.width, UiLength::Auto | UiLength::Fill(_))
    {
        slot.width
    } else {
        width
    };
    let height = if parent.align_items == UiAlign::Stretch
        && matches!(style.size.height, UiLength::Auto | UiLength::Fill(_))
    {
        slot.height
    } else {
        height
    };
    Rect::new(
        slot.x + align_x,
        slot.y + align_y,
        width.max(0.0),
        height.max(0.0),
    )
}

pub(super) fn inline_measure(
    sizes: &[[f32; 2]],
    gap: f32,
    max_width: f32,
    padding: UiEdges,
) -> [f32; 2] {
    [
        sizes.iter().map(|size| size[0]).sum::<f32>()
            + gap * sizes.len().saturating_sub(1) as f32
            + padding.left
            + padding.right,
        sizes.iter().map(|size| size[1]).fold(0.0, f32::max)
            + padding.top
            + padding.bottom
            + max_width.min(0.0),
    ]
}

pub(super) fn apply_margins_and_constraints(
    rect: Rect,
    margin: UiEdges,
    style: UiLayoutNode,
) -> Rect {
    let rect = Rect::new(
        rect.x + margin.left,
        rect.y + margin.top,
        (rect.width - margin.left - margin.right).max(0.0),
        (rect.height - margin.top - margin.bottom).max(0.0),
    );
    let size = constrain_size([rect.width, rect.height], style);
    Rect::new(rect.x, rect.y, size[0], size[1])
}

pub(super) fn constrain_size(size: [f32; 2], style: UiLayoutNode) -> [f32; 2] {
    [
        constrain_dimension(size[0], style.min_size[0], style.max_size[0]),
        constrain_dimension(size[1], style.min_size[1], style.max_size[1]),
    ]
}

pub(super) fn constrain_dimension(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut value = nonnegative(value);
    if let Some(min) = min {
        value = value.max(min);
    }
    if let Some(max) = max {
        value = value.min(max);
    }
    value
}

pub(super) fn resolve_length(length: UiLength, available: f32, measured: f32) -> (f32, f32) {
    match length {
        UiLength::Auto => (measured, 0.0),
        UiLength::Px(value) => (nonnegative(value), 0.0),
        UiLength::Percent(value) => (available * nonnegative(value), 0.0),
        UiLength::Fill(weight) => (0.0, nonnegative(weight)),
    }
}

pub(super) fn sanitize_length(length: UiLength) -> UiLength {
    match length {
        UiLength::Auto => UiLength::Auto,
        UiLength::Px(value) => UiLength::Px(nonnegative(value)),
        UiLength::Percent(value) => UiLength::Percent(nonnegative(value)),
        UiLength::Fill(value) => UiLength::Fill(nonnegative(value)),
    }
}

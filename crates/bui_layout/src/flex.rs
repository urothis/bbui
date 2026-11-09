use super::*;

pub(super) fn distribute_children(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    if children.is_empty() {
        return Vec::new();
    }
    match style.mode {
        UiLayoutMode::Row | UiLayoutMode::Column => {
            distribute_linear(world, children, style, inner, measurements)
        }
        UiLayoutMode::Flex { direction, wrap } => {
            distribute_flex(world, children, style, inner, direction, wrap, measurements)
        }
        UiLayoutMode::Overlay => children
            .iter()
            .map(|child| align_child(world, *child, style, inner, measurements))
            .collect(),
        UiLayoutMode::Grid { columns } => {
            distribute_grid(world, children, style, inner, columns.max(1), measurements)
        }
        UiLayoutMode::ResponsiveGrid {
            min_cell_width,
            max_columns,
        } => distribute_responsive_grid(
            world,
            children,
            style,
            inner,
            min_cell_width,
            max_columns,
            measurements,
        ),
        UiLayoutMode::Inline => distribute_inline(world, children, style, inner, measurements),
        UiLayoutMode::Leaf => Vec::new(),
    }
}

#[derive(Clone, Copy)]
pub(super) struct FlexItemLayout {
    style: UiLayoutNode,
    margins: UiEdges,
    measured: [f32; 2],
    main_axis: usize,
    min_outer_main: f32,
    max_outer_main: f32,
    allocated_outer_main: f32,
    grow: f32,
    shrink: f32,
    align_self: Option<UiAlign>,
}

pub(super) fn distribute_flex(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    direction: UiFlexDirection,
    wrap: bool,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let (main_axis, cross_axis) = flex_axes(direction);
    let main_extent = axis_extent(inner, main_axis);
    let available = [inner.width, inner.height];
    let mut items = Vec::with_capacity(children.len());
    for entity in children {
        let child_style = world
            .get::<UiLayoutNode>(*entity)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let item = world.get::<UiFlexItem>(*entity).copied();
        let margins = child_style.margin;
        let measured = measured_node_with_available(world, *entity, available, measurements);
        let main_length = item
            .and_then(|value| value.basis)
            .unwrap_or(axis_length(child_style.size, main_axis));
        let (basis, fill_weight) = resolve_length(main_length, main_extent, measured[main_axis]);
        let basis = constrain_dimension(
            basis.max(child_style.min_size[main_axis].unwrap_or(0.0)),
            child_style.min_size[main_axis],
            child_style.max_size[main_axis],
        );
        let margin_main = axis_margin_sum(margins, main_axis);
        let min_outer_main = child_style.min_size[main_axis].unwrap_or(0.0) + margin_main;
        let max_outer_main = child_style.max_size[main_axis].unwrap_or(f32::INFINITY) + margin_main;
        items.push(FlexItemLayout {
            style: child_style,
            margins,
            measured,
            main_axis,
            min_outer_main,
            max_outer_main,
            allocated_outer_main: basis + margin_main,
            grow: item.map_or(fill_weight, |value| nonnegative(value.grow)),
            shrink: item.map_or(1.0, |value| nonnegative(value.shrink)),
            align_self: item.and_then(|value| value.align_self),
        });
    }

    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut current = Vec::new();
    let mut used = 0.0;
    for (index, item) in items.iter().enumerate() {
        let gap = if current.is_empty() { 0.0 } else { style.gap };
        let next = used + gap + item.allocated_outer_main;
        if wrap && main_extent.is_finite() && !current.is_empty() && next > main_extent {
            lines.push(std::mem::take(&mut current));
            current.push(index);
            used = item.allocated_outer_main;
        } else {
            current.push(index);
            used = next;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }

    let mut line_cross_sizes = Vec::with_capacity(lines.len());
    for line in &lines {
        let gap_total = style.gap * line.len().saturating_sub(1) as f32;
        distribute_flex_line(&mut items, line, (main_extent - gap_total).max(0.0));
        line_cross_sizes.push(
            line.iter()
                .map(|index| {
                    let item = items[*index];
                    item.measured[cross_axis] + axis_margin_sum(item.margins, cross_axis)
                })
                .fold(0.0, f32::max),
        );
    }

    let mut output = vec![Rect::ZERO; children.len()];
    let mut cross_cursor = axis_origin(inner, cross_axis);
    for (line_index, line) in lines.iter().enumerate() {
        let line_cross = line_cross_sizes[line_index];
        let line_gap_total = style.gap * line.len().saturating_sub(1) as f32;
        let line_used = line
            .iter()
            .map(|index| items[*index].allocated_outer_main)
            .sum::<f32>()
            + line_gap_total;
        let free_main = (main_extent - line_used).max(0.0);
        let (main_offset, extra_gap) =
            justify_offsets(style.justify_content, free_main, line.len());
        let mut main_cursor = axis_origin(inner, main_axis) + main_offset;
        for index in line {
            let item = items[*index];
            let margin_cross = axis_margin_sum(item.margins, cross_axis);
            let available_cross = (line_cross - margin_cross).max(0.0);
            let cross_length = axis_length(item.style.size, cross_axis);
            let mut cross_size =
                resolve_length(cross_length, available_cross, item.measured[cross_axis]).0;
            let align = item.align_self.unwrap_or(style.align_items);
            if align == UiAlign::Stretch
                && matches!(cross_length, UiLength::Auto | UiLength::Fill(_))
            {
                cross_size = available_cross;
            }
            cross_size = constrain_dimension(
                cross_size,
                item.style.min_size[cross_axis],
                item.style.max_size[cross_axis],
            );
            let outer_cross = cross_size + margin_cross;
            let cross_free = line_cross - outer_cross;
            let cross_offset = match align {
                UiAlign::Start | UiAlign::Stretch => 0.0,
                UiAlign::Center => cross_free * 0.5,
                UiAlign::End => cross_free,
            };
            let outer_main = item.allocated_outer_main;
            let rect = if main_axis == 0 {
                Rect::new(
                    main_cursor,
                    cross_cursor + cross_offset,
                    outer_main,
                    outer_cross,
                )
            } else {
                Rect::new(
                    cross_cursor + cross_offset,
                    main_cursor,
                    outer_cross,
                    outer_main,
                )
            };
            output[*index] = rect;
            main_cursor += outer_main + style.gap + extra_gap;
        }
        cross_cursor += line_cross + style.gap;
    }
    output
}

pub(super) fn distribute_flex_line(items: &mut [FlexItemLayout], line: &[usize], available: f32) {
    let used = line
        .iter()
        .map(|index| items[*index].allocated_outer_main)
        .sum::<f32>();
    if used < available {
        let mut remaining = available - used;
        for _ in 0..line.len() {
            if remaining <= 0.001 {
                break;
            }
            let weight: f32 = line
                .iter()
                .map(|index| {
                    let item = items[*index];
                    if item.allocated_outer_main < item.max_outer_main {
                        item.grow
                    } else {
                        0.0
                    }
                })
                .sum();
            if weight <= 0.0 {
                break;
            }
            let mut consumed = 0.0;
            for index in line {
                let item = &mut items[*index];
                if item.grow <= 0.0 || item.allocated_outer_main >= item.max_outer_main {
                    continue;
                }
                let share = remaining * item.grow / weight;
                let delta = share.min(item.max_outer_main - item.allocated_outer_main);
                item.allocated_outer_main += delta;
                consumed += delta;
            }
            if consumed <= 0.001 {
                break;
            }
            remaining -= consumed;
        }
    } else if used > available {
        let mut remaining = used - available;
        for _ in 0..line.len() {
            if remaining <= 0.001 {
                break;
            }
            let weight: f32 = line
                .iter()
                .map(|index| {
                    let item = items[*index];
                    if item.allocated_outer_main > item.min_outer_main {
                        item.shrink
                            * (item.allocated_outer_main
                                - axis_margin_sum(item.margins, item.main_axis))
                            .max(1.0)
                    } else {
                        0.0
                    }
                })
                .sum();
            if weight <= 0.0 {
                break;
            }
            let mut consumed = 0.0;
            for index in line {
                let item = &mut items[*index];
                if item.shrink <= 0.0 || item.allocated_outer_main <= item.min_outer_main {
                    continue;
                }
                let basis = (item.allocated_outer_main
                    - axis_margin_sum(item.margins, item.main_axis))
                .max(1.0);
                let share = remaining * item.shrink * basis / weight;
                let delta = share.min(item.allocated_outer_main - item.min_outer_main);
                item.allocated_outer_main -= delta;
                consumed += delta;
            }
            if consumed <= 0.001 {
                break;
            }
            remaining -= consumed;
        }
    }
}

pub(super) fn distribute_responsive_grid(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    inner: Rect,
    min_cell_width: u32,
    max_columns: usize,
    measurements: &mut MeasurementMap,
) -> Vec<Rect> {
    let columns =
        responsive_grid_columns_with_gap(inner.width, min_cell_width, max_columns, style.gap)
            .min(children.len().max(1));
    let rows = children.len().div_ceil(columns);
    let cell_width =
        (inner.width - style.gap * columns.saturating_sub(1) as f32).max(0.0) / columns as f32;
    let mut row_heights = vec![0.0_f32; rows];
    for (index, child) in children.iter().enumerate() {
        let measured =
            measured_node_with_available(world, *child, [cell_width, inner.height], measurements);
        let margin = world
            .get::<UiLayoutNode>(*child)
            .map_or(UiEdges::ZERO, |node| node.margin.sanitized());
        row_heights[index / columns] =
            row_heights[index / columns].max(measured[1] + margin.top + margin.bottom);
    }
    let row_gap_total = style.gap * rows.saturating_sub(1) as f32;
    let used_height = row_heights.iter().sum::<f32>() + row_gap_total;
    let free_height = (inner.height - used_height).max(0.0);
    let (start_y, extra_gap) = justify_offsets(style.justify_content, free_height, rows);
    let mut row_y = inner.y + start_y;
    let mut result = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        let row = index / columns;
        let col = index % columns;
        let cell = Rect::new(
            inner.x + col as f32 * (cell_width + style.gap),
            row_y,
            cell_width,
            row_heights[row],
        );
        result.push(align_child(world, *child, style, cell, measurements));
        if col + 1 == columns || index + 1 == children.len() {
            row_y += row_heights[row] + style.gap + extra_gap;
        }
    }
    result
}

pub(super) fn flex_axes(direction: UiFlexDirection) -> (usize, usize) {
    match direction {
        UiFlexDirection::Row => (0, 1),
        UiFlexDirection::Column => (1, 0),
    }
}

pub(super) fn axis_length(size: UiSize, axis: usize) -> UiLength {
    if axis == 0 { size.width } else { size.height }
}

pub(super) fn axis_extent(rect: Rect, axis: usize) -> f32 {
    if axis == 0 { rect.width } else { rect.height }
}

pub(super) fn axis_origin(rect: Rect, axis: usize) -> f32 {
    if axis == 0 { rect.x } else { rect.y }
}

pub(super) fn axis_margin_sum(margins: UiEdges, axis: usize) -> f32 {
    if axis == 0 {
        margins.left + margins.right
    } else {
        margins.top + margins.bottom
    }
}

pub(super) fn justify_offsets(justify: UiJustify, free: f32, count: usize) -> (f32, f32) {
    match justify {
        UiJustify::Start => (0.0, 0.0),
        UiJustify::Center => (free * 0.5, 0.0),
        UiJustify::End => (free, 0.0),
        UiJustify::SpaceBetween if count > 1 => (0.0, free / (count - 1) as f32),
        UiJustify::SpaceBetween => (0.0, 0.0),
    }
}

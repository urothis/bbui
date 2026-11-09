use super::*;

impl UiLayoutNode {
    pub(super) fn sanitized(mut self) -> Self {
        self.padding = self.padding.sanitized();
        self.margin = self.margin.sanitized();
        self.gap = nonnegative(self.gap);
        self.min_size = self
            .min_size
            .map(|value| value.filter(|v| v.is_finite()).map(nonnegative));
        self.max_size = self
            .max_size
            .map(|value| value.filter(|v| v.is_finite()).map(nonnegative));
        self.size.width = sanitize_length(self.size.width);
        self.size.height = sanitize_length(self.size.height);
        self
    }
}

pub(super) fn measure_content_with_available(
    world: &World,
    entity: Entity,
    style: UiLayoutNode,
    children: &[Entity],
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let key = measure_key(available, MeasureKind::Content);
    if let Some(size) = measurements.get(&entity).and_then(|bucket| bucket.get(key)) {
        return size;
    }
    let size = measure_content_with_available_uncached(
        world,
        entity,
        style,
        children,
        available,
        measurements,
    );
    measurements.entry(entity).or_default().insert(key, size);
    size
}

pub(super) fn measure_content_with_available_uncached(
    world: &World,
    entity: Entity,
    style: UiLayoutNode,
    children: &[Entity],
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    if let Some(size) = world.get::<UiIntrinsicSize>(entity)
        && (style.mode == UiLayoutMode::Leaf || children.is_empty())
    {
        return [nonnegative(size.0[0]), nonnegative(size.0[1])];
    }
    if children.is_empty() || style.mode == UiLayoutMode::Leaf {
        let intrinsic = world
            .get::<UiIntrinsicSize>(entity)
            .map_or([0.0, 0.0], |size| size.0);
        return [
            nonnegative(intrinsic[0]) + style.padding.left + style.padding.right,
            nonnegative(intrinsic[1]) + style.padding.top + style.padding.bottom,
        ];
    }
    let child_available = [
        (available[0] - style.padding.left - style.padding.right).max(0.0),
        (available[1] - style.padding.top - style.padding.bottom).max(0.0),
    ];
    // These modes compute their own child measurements while deciding line or
    // cell assignments. Building the generic `sizes` vector first measured
    // every child a second time and allocated a vector that was never read.
    match style.mode {
        UiLayoutMode::Flex { direction, wrap } => {
            return measure_flex_content(
                world,
                children,
                style,
                available,
                direction,
                wrap,
                measurements,
            );
        }
        UiLayoutMode::ResponsiveGrid {
            min_cell_width,
            max_columns,
        } => {
            return measure_responsive_grid_content(
                world,
                children,
                style,
                available,
                min_cell_width,
                max_columns,
                measurements,
            );
        }
        _ => {}
    }
    let sizes: Vec<[f32; 2]> = children
        .iter()
        .map(|child| measured_node_with_available(world, *child, child_available, measurements))
        .collect();
    let gap = style.gap * children.len().saturating_sub(1) as f32;
    match style.mode {
        UiLayoutMode::Row => [
            sizes.iter().map(|size| size[0]).sum::<f32>()
                + gap
                + style.padding.left
                + style.padding.right,
            sizes.iter().map(|size| size[1]).fold(0.0, f32::max)
                + style.padding.top
                + style.padding.bottom,
        ],
        UiLayoutMode::Flex { direction, wrap } => measure_flex_content(
            world,
            children,
            style,
            available,
            direction,
            wrap,
            measurements,
        ),
        UiLayoutMode::Grid { columns } => {
            let columns = columns.max(1);
            let rows = children.len().div_ceil(columns);
            let width = sizes.iter().map(|size| size[0]).fold(0.0, f32::max) * columns as f32
                + style.gap * columns.saturating_sub(1) as f32;
            let height = sizes.iter().map(|size| size[1]).fold(0.0, f32::max) * rows as f32
                + style.gap * rows.saturating_sub(1) as f32;
            [
                width + style.padding.left + style.padding.right,
                height + style.padding.top + style.padding.bottom,
            ]
        }
        UiLayoutMode::ResponsiveGrid {
            min_cell_width,
            max_columns,
        } => measure_responsive_grid_content(
            world,
            children,
            style,
            available,
            min_cell_width,
            max_columns,
            measurements,
        ),
        UiLayoutMode::Overlay => [
            sizes.iter().map(|size| size[0]).fold(0.0, f32::max)
                + style.padding.left
                + style.padding.right,
            sizes.iter().map(|size| size[1]).fold(0.0, f32::max)
                + style.padding.top
                + style.padding.bottom,
        ],
        UiLayoutMode::Inline => inline_measure(&sizes, style.gap, f32::INFINITY, style.padding),
        UiLayoutMode::Column | UiLayoutMode::Leaf => [
            sizes.iter().map(|size| size[0]).fold(0.0, f32::max)
                + style.padding.left
                + style.padding.right,
            sizes.iter().map(|size| size[1]).sum::<f32>()
                + gap
                + style.padding.top
                + style.padding.bottom,
        ],
    }
}

pub(super) fn measured_node_with_available(
    world: &World,
    entity: Entity,
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let key = measure_key(available, MeasureKind::Node);
    if let Some(size) = measurements.get(&entity).and_then(|bucket| bucket.get(key)) {
        return size;
    }
    let size = measured_node_with_available_uncached(world, entity, available, measurements);
    measurements.entry(entity).or_default().insert(key, size);
    size
}

pub(super) fn measured_node_with_available_uncached(
    world: &World,
    entity: Entity,
    available: [f32; 2],
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let style = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    if let (Some(width), Some(height)) = (
        resolve_exact_length(style.size.width, available[0]),
        resolve_exact_length(style.size.height, available[1]),
    ) {
        return constrain_size([width, height], style);
    }
    let mut size = if let Some(children) = world.get::<bui_core::UiChildren>(entity) {
        if children.0.iter().all(|child| {
            world
                .get::<UiVisible>(*child)
                .is_none_or(|visible| visible.0)
        }) {
            measure_content_with_available(
                world,
                entity,
                style,
                &children.0,
                available,
                measurements,
            )
        } else {
            let visible = children
                .0
                .iter()
                .copied()
                .filter(|child| {
                    world
                        .get::<UiVisible>(*child)
                        .is_none_or(|visible| visible.0)
                })
                .collect::<Vec<_>>();
            measure_content_with_available(world, entity, style, &visible, available, measurements)
        }
    } else {
        measure_content_with_available(world, entity, style, &[], available, measurements)
    };
    size[0] = resolve_measured_length(style.size.width, available[0], size[0]);
    size[1] = resolve_measured_length(style.size.height, available[1], size[1]);
    constrain_size(size, style)
}

pub(super) fn resolve_exact_length(length: UiLength, available: f32) -> Option<f32> {
    match length {
        UiLength::Px(value) if value.is_finite() => Some(nonnegative(value)),
        UiLength::Percent(value) if available.is_finite() => {
            Some(nonnegative(available * nonnegative(value)))
        }
        _ => None,
    }
}

pub(super) fn resolve_measured_length(length: UiLength, available: f32, measured: f32) -> f32 {
    match length {
        UiLength::Px(value) => nonnegative(value),
        UiLength::Percent(value) if available.is_finite() => available * nonnegative(value),
        _ => measured,
    }
}

pub(super) fn resolve_flex_basis(length: UiLength, available: f32, measured: f32) -> f32 {
    match length {
        UiLength::Fill(_) => 0.0,
        UiLength::Percent(value) if available.is_finite() => available * nonnegative(value),
        UiLength::Percent(_) | UiLength::Auto => measured,
        UiLength::Px(value) => nonnegative(value),
    }
}

pub(super) fn responsive_grid_columns_with_gap(
    width: f32,
    min_cell_width: u32,
    max_columns: usize,
    gap: f32,
) -> usize {
    let max_columns = max_columns.max(1);
    if !width.is_finite() {
        return max_columns;
    }
    let minimum = min_cell_width.max(1) as f32;
    (((nonnegative(width) + nonnegative(gap)) / (minimum + nonnegative(gap))).floor() as usize)
        .max(1)
        .min(max_columns)
}

pub(super) fn measure_flex_content(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    available: [f32; 2],
    direction: UiFlexDirection,
    wrap: bool,
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let (main_axis, cross_axis) = flex_axes(direction);
    let inner_available = [
        (available[0] - style.padding.left - style.padding.right).max(0.0),
        (available[1] - style.padding.top - style.padding.bottom).max(0.0),
    ];
    let main_available = inner_available[main_axis];
    let mut lines: Vec<(f32, f32)> = Vec::new();
    let (mut line_main, mut line_cross, mut line_items) = (0.0_f32, 0.0_f32, 0usize);
    for child in children {
        let child_style = world
            .get::<UiLayoutNode>(*child)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let measured = measured_node_with_available(world, *child, inner_available, measurements);
        let margins = child_style.margin;
        let item = world.get::<UiFlexItem>(*child).copied();
        let main_length = item
            .and_then(|item| item.basis)
            .unwrap_or(axis_length(child_style.size, main_axis));
        let main_size = resolve_flex_basis(main_length, main_available, measured[main_axis]);
        let outer_main = main_size + axis_margin_sum(margins, main_axis);
        let outer_cross = measured[cross_axis] + axis_margin_sum(margins, cross_axis);
        let next_main = line_main + if line_items == 0 { 0.0 } else { style.gap } + outer_main;
        if wrap && main_available.is_finite() && line_items > 0 && next_main > main_available {
            lines.push((line_main, line_cross));
            line_main = outer_main;
            line_cross = outer_cross;
            line_items = 1;
        } else {
            line_main = next_main;
            line_cross = line_cross.max(outer_cross);
            line_items += 1;
        }
    }
    if line_items > 0 {
        lines.push((line_main, line_cross));
    }
    let natural_main = if wrap {
        lines.iter().map(|line| line.0).fold(0.0, f32::max)
    } else {
        lines.first().map_or(0.0, |line| line.0)
    };
    let natural_cross = if wrap {
        lines.iter().map(|line| line.1).sum::<f32>()
            + style.gap * lines.len().saturating_sub(1) as f32
    } else {
        lines.first().map_or(0.0, |line| line.1)
    };
    let mut result = [0.0; 2];
    result[main_axis] = natural_main;
    result[cross_axis] = natural_cross;
    result[0] += style.padding.left + style.padding.right;
    result[1] += style.padding.top + style.padding.bottom;
    result
}

pub(super) fn measure_responsive_grid_content(
    world: &World,
    children: &[Entity],
    style: UiLayoutNode,
    available: [f32; 2],
    min_cell_width: u32,
    max_columns: usize,
    measurements: &mut MeasurementMap,
) -> [f32; 2] {
    let max_columns = max_columns.max(1).min(children.len().max(1));
    let inner_width = if available[0].is_finite() {
        (available[0] - style.padding.left - style.padding.right).max(0.0)
    } else {
        f32::INFINITY
    };
    let columns =
        responsive_grid_columns_with_gap(inner_width, min_cell_width, max_columns, style.gap)
            .min(children.len().max(1));
    let rows = children.len().div_ceil(columns);
    let cell_width = if inner_width.is_finite() {
        (inner_width - style.gap * columns.saturating_sub(1) as f32).max(0.0) / columns as f32
    } else {
        let intrinsic_width = children
            .iter()
            .map(|child| {
                measured_node_with_available(
                    world,
                    *child,
                    [f32::INFINITY, available[1]],
                    measurements,
                )[0]
            })
            .fold(0.0, f32::max);
        (min_cell_width.max(1) as f32).max(intrinsic_width)
    };
    let mut row_heights = vec![0.0_f32; rows];
    for (index, child) in children.iter().enumerate() {
        let inner_height = if available[1].is_finite() {
            (available[1] - style.padding.top - style.padding.bottom).max(0.0)
        } else {
            available[1]
        };
        let measured =
            measured_node_with_available(world, *child, [cell_width, inner_height], measurements);
        let margin = world
            .get::<UiLayoutNode>(*child)
            .map_or(UiEdges::ZERO, |node| node.margin.sanitized());
        row_heights[index / columns] =
            row_heights[index / columns].max(measured[1] + margin.top + margin.bottom);
    }
    let row_gap = style.gap * rows.saturating_sub(1) as f32;
    let natural_width = cell_width * columns as f32
        + style.gap * columns.saturating_sub(1) as f32
        + style.padding.left
        + style.padding.right;
    [
        if available[0].is_finite() {
            available[0]
        } else {
            natural_width
        },
        row_heights.iter().sum::<f32>() + row_gap + style.padding.top + style.padding.bottom,
    ]
}

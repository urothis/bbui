use super::*;

pub(in crate::actions) fn paint_data_table_root(
    table: &DataTable,
    label: &str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let title_height = theme.control_height.max(1.0).min(rect.height.max(0.0));
    let column_height = theme
        .spacing
        .table
        .header_height
        .max(1.0)
        .min((rect.height - title_height).max(0.0));
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.background,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect {
                height: title_height,
                ..ClipRect::from(rect)
            },
            radius: theme.corner_radius,
            color: theme.color(ThemeColorToken::DataTableTitleSurface),
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect {
                x: rect.x,
                y: rect.y + title_height,
                width: rect.width,
                height: column_height,
            },
            radius: 0.0,
            color: theme.color(ThemeColorToken::DataTableHeaderSurface),
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.control.padding_x,
                rect.y + theme.spacing.control.padding_y,
            ],
            format!(
                "{label}  ·  {} rows × {} columns",
                table.rows.len(),
                table.columns.len()
            ),
            theme.text,
            theme,
            false,
        ),
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::Line {
            from: [rect.x, rect.y + title_height + column_height],
            to: [rect.x + rect.width, rect.y + title_height + column_height],
            width: 1.0,
            color: theme.border,
        },
    ));
}

pub(in crate::actions) fn paint_data_table_header(
    header: &DataTableHeaderControl,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let background = if header.active {
        theme.color(ThemeColorToken::DataTableSelectedHeaderSurface)
    } else {
        theme.color(ThemeColorToken::DataTableHeaderSurface)
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: 0.0,
            color: background,
        },
    ));
    let sort_glyph = match header.sort_ascending {
        Some(true) => "  ↑",
        Some(false) => "  ↓",
        None => "",
    };
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.table.cell_padding_x,
                rect.y + theme.spacing.table.cell_padding_y,
            ],
            format!("{}{sort_glyph}", header.label),
            theme.text,
            theme,
            false,
        ),
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::Line {
            from: [
                rect.x + rect.width - 1.0,
                rect.y + theme.spacing.table.cell_padding_y.min(rect.height * 0.5),
            ],
            to: [
                rect.x + rect.width - 1.0,
                rect.y + rect.height - theme.spacing.table.cell_padding_y.min(rect.height * 0.5),
            ],
            width: 1.0,
            color: if header.active {
                theme.focus
            } else {
                theme.border
            },
        },
    ));
}

pub(in crate::actions) fn paint_data_table_cell(
    cell: &DataTableCellControl,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let background = if cell.active {
        theme.color(ThemeColorToken::DataTableActiveCellSurface)
    } else if cell.selected {
        theme.color(ThemeColorToken::DataTableSelectedCellSurface)
    } else if cell.row_index % 2 == 1 {
        theme.surface.mix(theme.background, 0.24)
    } else {
        theme.surface
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: 0.0,
            color: background,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.table.cell_padding_x,
                rect.y + theme.spacing.table.cell_padding_y,
            ],
            cell.value.clone(),
            theme.text,
            theme,
            false,
        ),
    ));
    output.push(item(
        clip,
        z.saturating_add(if cell.active { 3 } else { 2 }),
        Primitive::Line {
            from: [rect.x, rect.y + rect.height - 1.0],
            to: [rect.x + rect.width, rect.y + rect.height - 1.0],
            width: 1.0,
            color: if cell.active {
                theme.focus
            } else {
                theme.border.mix(theme.surface, 0.5)
            },
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::Line {
            from: [rect.x + rect.width - 1.0, rect.y + 4.0],
            to: [rect.x + rect.width - 1.0, rect.y + rect.height - 4.0],
            width: 1.0,
            color: theme.border.mix(theme.surface, 0.5),
        },
    ));
    if cell.active {
        output.push(item(
            clip,
            z.saturating_add(3),
            Primitive::Line {
                from: [rect.x + 1.0, rect.y + 1.0],
                to: [rect.x + 1.0, rect.y + rect.height - 1.0],
                width: 2.0,
                color: theme.focus,
            },
        ));
    }
}

use super::*;

pub(in crate::actions) fn paint_node_graph_target(
    paint: NodeGraphPaint<'_>,
    target: NodeGraphPaintTarget<'_>,
    output: &mut Vec<DrawItem>,
) {
    let NodeGraphPaint {
        graph,
        state,
        output_label_widths,
        registry,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font,
    } = paint;
    let spacing = &theme.spacing.node_graph;
    let header = Rect {
        height: spacing.header_height.min(rect.height),
        ..rect
    };
    let paints_chrome = matches!(target, NodeGraphPaintTarget::Chrome);
    if paints_chrome {
        output.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: canvas_surface,
            },
        ));
    }
    if paints_chrome && let Some(header_clip) = intersect_clip_rect(clip, header) {
        output.push(item(
            header_clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect::from(header),
                radius: theme.corner_radius,
                color: theme.color(ThemeColorToken::NodeGraphHeaderSurface),
            },
        ));
        let inset = theme.spacing.control.padding_x.min(rect.width * 0.08);
        let summary_width = spacing.header_hint_width.min(rect.width * 0.44).max(1.0);
        let summary_right = rect.x + rect.width - inset;
        let summary_left = (summary_right - summary_width).max(rect.x + inset);
        let title_x = rect.x + inset;
        let title_width = (summary_left - title_x - inset).max(1.0);
        let summary = format!(
            "{} {} · {} {}",
            graph.nodes.len(),
            if graph.nodes.len() == 1 {
                "node"
            } else {
                "nodes"
            },
            graph.links.len(),
            if graph.links.len() == 1 {
                "link"
            } else {
                "links"
            },
        );
        let text_y = rect.y
            + spacing
                .title_padding_y
                .min((header.height - theme.font_size).max(0.0));
        output.push(item(
            header_clip,
            z.saturating_add(2),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [title_x, text_y],
                    text: label.to_owned(),
                },
                theme.text,
                title_width,
                bevy_text::Justify::Left,
                &font,
                theme,
                1.0,
            ),
        ));
        output.push(item(
            header_clip,
            z.saturating_add(2),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [summary_left, text_y],
                    text: summary,
                },
                theme.text_muted,
                summary_right - summary_left,
                bevy_text::Justify::Right,
                &font,
                theme,
                1.0,
            ),
        ));
    }

    let viewport = Rect {
        y: rect.y + header.height,
        height: (rect.height - header.height).max(0.0),
        ..rect
    };
    let Some(viewport_clip) = intersect_clip_rect(clip, viewport) else {
        return;
    };
    if paints_chrome {
        let grid_spacing = (spacing.grid_spacing * graph.zoom)
            .clamp(spacing.grid_spacing_min, spacing.grid_spacing_max);
        let grid = theme
            .color(ThemeColorToken::NodeGraphGridLine)
            .mix(canvas_surface, 0.6);
        let dot_size = (2.0 * graph.zoom.sqrt()).clamp(1.5, 2.6);
        let start_x = viewport.x + graph.pan[0].rem_euclid(grid_spacing);
        let start_y = viewport.y + graph.pan[1].rem_euclid(grid_spacing);
        let mut dot_x = start_x;
        let mut columns = 0;
        let mut dot_count = 0;
        while dot_x <= viewport.x + viewport.width && columns < 48 && dot_count < 1024 {
            let mut dot_y = start_y;
            let mut rows = 0;
            while dot_y <= viewport.y + viewport.height && rows < 48 && dot_count < 1024 {
                output.push(item(
                    viewport_clip,
                    z.saturating_add(1),
                    Primitive::Ellipse {
                        rect: ClipRect {
                            x: dot_x - dot_size * 0.5,
                            y: dot_y - dot_size * 0.5,
                            width: dot_size,
                            height: dot_size,
                        },
                        color: grid,
                    },
                ));
                dot_y += grid_spacing;
                rows += 1;
                dot_count += 1;
            }
            dot_x += grid_spacing;
            columns += 1;
        }
    }
    let paint = NodeGraphPaint {
        graph,
        state,
        output_label_widths,
        registry,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font,
    };
    match target {
        NodeGraphPaintTarget::Chrome => paint_graph_gesture(paint, target, viewport_clip, output),
        NodeGraphPaintTarget::Node(_) => paint_graph_node(paint, target, viewport_clip, output),
        NodeGraphPaintTarget::Link { .. } => {
            paint_graph_link(paint, target, viewport, viewport_clip, output)
        }
    }
}

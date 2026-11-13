use super::*;

pub(super) fn paint_graph_link(
    paint: NodeGraphPaint<'_>,
    target: NodeGraphPaintTarget<'_>,
    viewport: Rect,
    viewport_clip: ClipRect,
    output: &mut Vec<DrawItem>,
) {
    let graph = paint.graph;
    let rect = paint.rect;
    let theme = paint.theme;
    let registry = paint.registry;
    let z = paint.z;
    let spacing = &theme.spacing.node_graph;
    if let NodeGraphPaintTarget::Link { link, from, to } = target {
        let Some(points) = node_graph_link_points_between(graph, rect, link, from, to, spacing)
        else {
            return;
        };
        if node_graph_bezier_bounds(points, 8.0)
            .intersection(viewport)
            .is_none()
        {
            return;
        }
        let selected = graph.selected_link == Some(link.id);
        let (Some(source), Some(destination)) = (
            from.sockets.get(link.from_socket),
            to.sockets.get(link.to_socket),
        ) else {
            return;
        };
        let start_color = socket_type_color(&source.socket_type, theme, registry);
        let end_color = socket_type_color(&destination.socket_type, theme, registry);
        if selected {
            output.push(item(
                viewport_clip,
                z.saturating_add(3),
                Primitive::Bezier {
                    points,
                    width: 5.0,
                    color: theme.accent,
                },
            ));
        }
        output.push(item(
            viewport_clip,
            z.saturating_add(3),
            if source.socket_type == destination.socket_type {
                Primitive::Bezier {
                    points,
                    width: if selected { 3.0 } else { 2.5 },
                    color: start_color,
                }
            } else {
                Primitive::BezierGradient {
                    points,
                    width: if selected { 3.0 } else { 2.5 },
                    start_color,
                    end_color,
                }
            },
        ));
    }
}

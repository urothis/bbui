use super::*;

pub(super) fn bezier_point(points: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    [
        u.powi(3) * points[0][0]
            + 3.0 * u * u * t * points[1][0]
            + 3.0 * u * t * t * points[2][0]
            + t.powi(3) * points[3][0],
        u.powi(3) * points[0][1]
            + 3.0 * u * u * t * points[1][1]
            + 3.0 * u * t * t * points[2][1]
            + t.powi(3) * points[3][1],
    ]
}

pub(super) fn bezier_distance(points: [[f32; 2]; 4], point: [f32; 2]) -> f32 {
    let mut previous = points[0];
    let mut closest = f32::INFINITY;
    for index in 1..=24 {
        let current = bezier_point(points, index as f32 / 24.0);
        closest = closest.min(point_segment_distance(point, previous, current));
        previous = current;
    }
    closest
}

pub(super) fn point_segment_distance(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let denominator = delta[0] * delta[0] + delta[1] * delta[1];
    if denominator <= f32::EPSILON {
        return distance(point, start);
    }
    let amount = (((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1])
        / denominator)
        .clamp(0.0, 1.0);
    distance(
        point,
        [start[0] + amount * delta[0], start[1] + amount * delta[1]],
    )
}

pub(super) fn node_graph_link_crossed(
    graph: &NodeGraph,
    rect: Rect,
    link: crate::NodeLink,
    stroke: &[[f32; 2]],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> bool {
    if stroke.len() < 2 {
        return false;
    }
    let Some(points) = node_graph_link_points(graph, rect, link, spacing) else {
        return false;
    };
    let mut curve_previous = points[0];
    for sample in 1..=24 {
        let curve_current = bezier_point(points, sample as f32 / 24.0);
        if stroke.windows(2).any(|segment| {
            segments_close(curve_previous, curve_current, segment[0], segment[1], 4.0)
        }) {
            return true;
        }
        curve_previous = curve_current;
    }
    false
}

pub(super) fn segments_close(
    first_start: [f32; 2],
    first_end: [f32; 2],
    second_start: [f32; 2],
    second_end: [f32; 2],
    tolerance: f32,
) -> bool {
    point_segment_distance(first_start, second_start, second_end) <= tolerance
        || point_segment_distance(first_end, second_start, second_end) <= tolerance
        || point_segment_distance(second_start, first_start, first_end) <= tolerance
        || point_segment_distance(second_end, first_start, first_end) <= tolerance
}

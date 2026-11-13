use super::*;

const GRAPH_SPATIAL_CELL: f32 = 256.0;

fn graph_spatial_cell(value: f32, level: u8) -> i32 {
    (value / (GRAPH_SPATIAL_CELL * 2.0_f32.powi(i32::from(level)))).floor() as i32
}

fn insert_graph_spatial_bounds(
    cells: &mut GraphSpatialCells,
    overflow: &mut Vec<NodeGraphItemKey>,
    membership: &mut GraphSpatialMembership,
    levels: &mut HashSet<u8>,
    key: NodeGraphItemKey,
    rect: Rect,
) {
    if ![rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(f32::is_finite)
    {
        overflow.push(key);
        membership.insert(key, Vec::new());
        return;
    }
    if ![rect.x + rect.width, rect.y + rect.height]
        .into_iter()
        .all(f32::is_finite)
    {
        overflow.push(key);
        membership.insert(key, Vec::new());
        return;
    }
    let Some((level, x0, x1, y0, y1)) = (0..=31_u8).find_map(|level| {
        let x0 = graph_spatial_cell(rect.x, level);
        let x1 = graph_spatial_cell(rect.x + rect.width, level);
        let y0 = graph_spatial_cell(rect.y, level);
        let y1 = graph_spatial_cell(rect.y + rect.height, level);
        let width = (i64::from(x1) - i64::from(x0) + 1).max(0);
        let height = (i64::from(y1) - i64::from(y0) + 1).max(0);
        (width.saturating_mul(height) <= 256).then_some((level, x0, x1, y0, y1))
    }) else {
        overflow.push(key);
        membership.insert(key, Vec::new());
        return;
    };
    levels.insert(level);
    let mut occupied = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            cells.entry((level, x, y)).or_default().push(key);
            occupied.push((level, x, y));
        }
    }
    membership.insert(key, occupied);
}

fn graph_spatial_bounds_for_key(
    graph: &NodeGraph,
    layout: Rect,
    spacing: &bbui_style::ThemeNodeGraphSpacing,
    indices: &HashMap<NodeId, usize>,
    link_indices: &HashMap<u64, usize>,
    key: NodeGraphItemKey,
) -> Option<Rect> {
    let offset = [
        layout.x + graph.pan[0],
        layout.y + spacing.header_height + graph.pan[1],
    ];
    match key {
        NodeGraphItemKey::Node(id) => {
            let node = indices.get(&id).and_then(|index| graph.nodes.get(*index))?;
            let mut rect = node_graph_node_rect(graph, layout, node, spacing);
            rect.x -= offset[0];
            rect.y -= offset[1];
            Some(rect)
        }
        NodeGraphItemKey::Link(id) => {
            let link = link_indices
                .get(&id)
                .and_then(|index| graph.links.get(*index))?;
            let from = indices
                .get(&link.from_node)
                .and_then(|index| graph.nodes.get(*index))?;
            let to = indices
                .get(&link.to_node)
                .and_then(|index| graph.nodes.get(*index))?;
            let mut points =
                node_graph_link_points_between(graph, layout, *link, from, to, spacing)?;
            for point in &mut points {
                point[0] -= offset[0];
                point[1] -= offset[1];
            }
            let mut bounds = node_graph_bezier_bounds(points, 8.0);
            // Link controls use a minimum 16 px span before their 8 px padding.
            // Keep the index conservative for nearly vertical or horizontal curves.
            bounds.width = bounds.width.max(32.0);
            bounds.height = bounds.height.max(32.0);
            Some(bounds)
        }
        NodeGraphItemKey::Socket(_, _) => None,
    }
}

pub(super) fn build_node_graph_spatial_index(
    graph: &NodeGraph,
    layout: Rect,
    spacing: &bbui_style::ThemeNodeGraphSpacing,
    indices: &HashMap<NodeId, usize>,
    link_indices: &HashMap<u64, usize>,
) -> (
    GraphSpatialCells,
    Vec<NodeGraphItemKey>,
    GraphSpatialMembership,
    HashSet<u8>,
) {
    let mut cells = HashMap::new();
    let mut overflow = Vec::new();
    let mut membership = HashMap::new();
    let mut levels = HashSet::new();
    for node in &graph.nodes {
        let key = NodeGraphItemKey::Node(node.id);
        if let Some(rect) =
            graph_spatial_bounds_for_key(graph, layout, spacing, indices, link_indices, key)
        {
            insert_graph_spatial_bounds(
                &mut cells,
                &mut overflow,
                &mut membership,
                &mut levels,
                key,
                rect,
            );
        }
    }
    for link in &graph.links {
        let key = NodeGraphItemKey::Link(link.id);
        if let Some(rect) =
            graph_spatial_bounds_for_key(graph, layout, spacing, indices, link_indices, key)
        {
            insert_graph_spatial_bounds(
                &mut cells,
                &mut overflow,
                &mut membership,
                &mut levels,
                key,
                rect,
            );
        }
    }
    (cells, overflow, membership, levels)
}

pub(super) fn update_node_graph_spatial_index(
    cache: &mut WidgetNodeGraphItems,
    owner: Entity,
    graph: &NodeGraph,
    layout: Rect,
    spacing: &bbui_style::ThemeNodeGraphSpacing,
    keys: &[NodeGraphItemKey],
) {
    let (
        Some(cells),
        Some(overflow),
        Some(membership),
        Some(levels),
        Some(indices),
        Some(link_indices),
    ) = (
        cache.spatial_cells.get_mut(&owner),
        cache.spatial_overflow.get_mut(&owner),
        cache.spatial_membership.get_mut(&owner),
        cache.spatial_levels.get_mut(&owner),
        cache.node_indices.get(&owner),
        cache.link_indices.get(&owner),
    )
    else {
        return;
    };
    for &key in keys {
        if let Some(occupied) = membership.remove(&key) {
            for cell in occupied {
                if let Some(entries) = cells.get_mut(&cell) {
                    entries.retain(|entry| *entry != key);
                }
            }
        }
        overflow.retain(|entry| *entry != key);
        if let Some(rect) =
            graph_spatial_bounds_for_key(graph, layout, spacing, indices, link_indices, key)
        {
            insert_graph_spatial_bounds(cells, overflow, membership, levels, key, rect);
        }
    }
}

pub(super) fn node_graph_spatial_candidates(
    cache: &WidgetNodeGraphItems,
    owner: Entity,
    graph: &NodeGraph,
    layout: UiLayoutResult,
    spacing: &bbui_style::ThemeNodeGraphSpacing,
) -> HashSet<NodeGraphItemKey> {
    let viewport = layout
        .clip
        .unwrap_or(layout.rect)
        .intersection(layout.rect)
        .and_then(|clip| {
            clip.intersection(Rect {
                y: layout.rect.y + spacing.header_height,
                height: (layout.rect.height - spacing.header_height).max(0.0),
                ..layout.rect
            })
        })
        .unwrap_or_default();
    let offset = [
        layout.rect.x + graph.pan[0],
        layout.rect.y + spacing.header_height + graph.pan[1],
    ];
    let mut candidates = HashSet::new();
    if let (Some(cells), Some(levels)) = (
        cache.spatial_cells.get(&owner),
        cache.spatial_levels.get(&owner),
    ) {
        for &level in levels {
            let x0 = graph_spatial_cell(viewport.x - offset[0], level);
            let x1 = graph_spatial_cell(viewport.x + viewport.width - offset[0], level);
            let y0 = graph_spatial_cell(viewport.y - offset[1], level);
            let y1 = graph_spatial_cell(viewport.y + viewport.height - offset[1], level);
            let width = (i64::from(x1) - i64::from(x0) + 1).max(0);
            let height = (i64::from(y1) - i64::from(y0) + 1).max(0);
            if width.saturating_mul(height) > 4096 {
                candidates.extend(
                    cells
                        .iter()
                        .filter(|((entry_level, x, y), _)| {
                            *entry_level == level
                                && (*x >= x0 && *x <= x1)
                                && (*y >= y0 && *y <= y1)
                        })
                        .flat_map(|(_, entries)| entries.iter().copied()),
                );
                continue;
            }
            for x in x0..=x1 {
                for y in y0..=y1 {
                    candidates.extend(cells.get(&(level, x, y)).into_iter().flatten().copied());
                }
            }
        }
    }
    candidates.extend(
        cache
            .spatial_overflow
            .get(&owner)
            .into_iter()
            .flatten()
            .copied(),
    );
    candidates
}

#[cfg(test)]
mod spatial_tests {
    use super::*;

    #[test]
    fn near_vertical_link_index_covers_minimum_control_bounds() {
        let mut graph = NodeGraph::default();
        let from = graph.add_node(
            "Source",
            [0.0, 0.0],
            vec![NodeSocket::output("out", SocketType::Float)],
        );
        let source_width = graph.nodes[0].size[0];
        let to = graph.add_node(
            "Sink",
            [source_width, 0.0],
            vec![NodeSocket::input("in", SocketType::Float)],
        );
        let link = graph.create_link(from, 0, to, 0).unwrap();
        let nodes = graph
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, index))
            .collect();
        let links = graph
            .links
            .iter()
            .enumerate()
            .map(|(index, link)| (link.id, index))
            .collect();
        let bounds = graph_spatial_bounds_for_key(
            &graph,
            Rect::new(0.0, 0.0, 500.0, 300.0),
            &Theme::default().spacing.node_graph,
            &nodes,
            &links,
            NodeGraphItemKey::Link(link),
        )
        .unwrap();
        assert!(bounds.width >= 32.0 && bounds.height >= 32.0);
    }
}

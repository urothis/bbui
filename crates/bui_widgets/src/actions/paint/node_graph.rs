use super::*;

pub(in crate::actions) struct NodeGraphPaint<'a> {
    pub(in crate::actions) graph: &'a NodeGraph,
    pub(in crate::actions) state: NodeGraphEditorState,
    pub(in crate::actions) output_label_widths: HashMap<(NodeId, usize), f32>,
    pub(in crate::actions) canvas_surface: Rgba,
    pub(in crate::actions) label: &'a str,
    pub(in crate::actions) rect: Rect,
    pub(in crate::actions) clip: ClipRect,
    pub(in crate::actions) z: i32,
    pub(in crate::actions) theme: &'a Theme,
    pub(in crate::actions) font: TextFont,
}

#[derive(Clone, Copy)]
pub(in crate::actions) enum NodeGraphPaintTarget<'a> {
    Chrome,
    Node(&'a crate::NodeModel),
    Link {
        link: crate::NodeLink,
        from: &'a crate::NodeModel,
        to: &'a crate::NodeModel,
    },
}

#[cfg(test)]
pub(in crate::actions) fn paint_node_graph(paint: NodeGraphPaint<'_>, output: &mut Vec<DrawItem>) {
    let NodeGraphPaint {
        graph,
        state,
        output_label_widths,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font,
    } = paint;
    let make_paint = |output_label_widths| NodeGraphPaint {
        graph,
        state: state.clone(),
        output_label_widths,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font: font.clone(),
    };
    paint_node_graph_target(
        make_paint(output_label_widths.clone()),
        NodeGraphPaintTarget::Chrome,
        output,
    );
    let nodes_by_id: HashMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    for link in &graph.links {
        let (Some(from), Some(to)) = (
            nodes_by_id.get(&link.from_node).copied(),
            nodes_by_id.get(&link.to_node).copied(),
        ) else {
            continue;
        };
        paint_node_graph_target(
            make_paint(HashMap::new()),
            NodeGraphPaintTarget::Link {
                link: *link,
                from,
                to,
            },
            output,
        );
    }
    for node in &graph.nodes {
        paint_node_graph_target(
            make_paint(output_label_widths.clone()),
            NodeGraphPaintTarget::Node(node),
            output,
        );
    }
}

pub(in crate::actions) fn paint_node_graph_target(
    paint: NodeGraphPaint<'_>,
    target: NodeGraphPaintTarget<'_>,
    output: &mut Vec<DrawItem>,
) {
    let NodeGraphPaint {
        graph,
        state,
        output_label_widths,
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
        output.push(item(
            viewport_clip,
            z.saturating_add(3),
            Primitive::Bezier {
                points,
                width: if selected { 3.5 } else { 2.5 },
                color: if selected {
                    theme.accent
                } else {
                    from.sockets
                        .get(link.from_socket)
                        .map_or(theme.text_muted, |socket| {
                            socket_type_color(socket.socket_type, theme)
                        })
                },
            },
        ));
    }

    if let NodeGraphPaintTarget::Node(node) = target {
        let node_rect = node_graph_node_rect(graph, rect, node, spacing);
        let Some(node_clip) = intersect_clip_rect(viewport_clip, node_rect) else {
            return;
        };
        let selected = graph.selected.contains(&node.id);
        output.push(item(
            node_clip,
            z.saturating_add(4),
            Primitive::RoundedRect {
                rect: ClipRect::from(node_rect),
                radius: spacing.node_radius,
                color: if selected {
                    theme.surface.mix(theme.accent, 0.08)
                } else {
                    theme.surface
                },
            },
        ));
        let title_height = (spacing.header_height * graph.zoom).min(node_rect.height);
        let node_header = Rect {
            height: title_height,
            ..node_rect
        };
        let title_clip = intersect_clip_rect(node_clip, node_header).unwrap_or(node_clip);
        output.push(item(
            title_clip,
            z.saturating_add(5),
            Primitive::RoundedRect {
                rect: ClipRect::from(node_header),
                radius: spacing.node_radius,
                color: node_graph_category_color(node.category, theme),
            },
        ));
        let zoom = graph.zoom.max(0.01);
        let title_x = node_rect.x + (spacing.title_padding_x + 8.0) * zoom;
        let title_width =
            (node_rect.x + node_rect.width - spacing.body_padding * zoom - title_x).max(1.0);
        output.push(item(
            title_clip,
            z.saturating_add(6),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [title_x, node_rect.y + spacing.title_padding_y * zoom],
                    text: node.title.clone(),
                },
                theme.text,
                title_width,
                bevy_text::Justify::Left,
                &font,
                theme,
                zoom,
            ),
        ));
        output.push(item(
            title_clip,
            z.saturating_add(6),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [
                        node_rect.x + spacing.collapse_toggle_inset_x * zoom,
                        node_rect.y + spacing.title_padding_y * zoom,
                    ],
                    text: if node.collapsed { "▸" } else { "▾" }.to_owned(),
                },
                theme.text,
                (spacing.title_padding_x - spacing.collapse_toggle_inset_x).max(1.0) * zoom,
                bevy_text::Justify::Left,
                &font,
                theme,
                zoom,
            ),
        ));

        for (socket_index, socket) in node.sockets.iter().enumerate() {
            let Some(center) = node_graph_socket_position(graph, rect, node, socket_index, spacing)
            else {
                continue;
            };
            let color = socket_type_color(socket.socket_type, theme);
            let radius = (spacing.socket_radius * graph.zoom).max(0.0);
            output.push(item(
                viewport_clip,
                z.saturating_add(8),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        x: center[0] - radius,
                        y: center[1] - radius,
                        width: radius * 2.0,
                        height: radius * 2.0,
                    },
                    radius,
                    color,
                },
            ));
            let label_row = node.sockets[..socket_index]
                .iter()
                .filter(|previous| previous.direction == socket.direction)
                .count();
            let opposite_direction = match socket.direction {
                SocketDirection::Input => SocketDirection::Output,
                SocketDirection::Output => SocketDirection::Input,
            };
            let has_counterpart = node
                .sockets
                .iter()
                .filter(|candidate| candidate.direction == opposite_direction)
                .nth(label_row)
                .is_some();
            let content_left = node_rect.x + spacing.socket_label_inset_x * zoom;
            let content_right = node_rect.x + node_rect.width - spacing.socket_label_inset_x * zoom;
            let content_width = (content_right - content_left).max(1.0);
            let split = content_left + content_width * 0.5;
            let (text_x, max_width, text_clip) = match socket.direction {
                SocketDirection::Input if has_counterpart => (
                    content_left,
                    (split - content_left).max(1.0),
                    intersect_clip_rect(
                        node_clip,
                        Rect::new(
                            content_left,
                            node_rect.y,
                            split - content_left,
                            node_rect.height,
                        ),
                    )
                    .unwrap_or(node_clip),
                ),
                SocketDirection::Input => (content_left, content_width, node_clip),
                SocketDirection::Output => {
                    let lane_left = if has_counterpart { split } else { content_left };
                    let lane_width = (content_right - lane_left).max(1.0);
                    let label_width = (output_label_widths
                        .get(&(node.id, socket_index))
                        .copied()
                        .unwrap_or(lane_width)
                        + 1.0)
                        .clamp(1.0, lane_width);
                    let clip = if has_counterpart {
                        intersect_clip_rect(
                            node_clip,
                            Rect::new(lane_left, node_rect.y, lane_width, node_rect.height),
                        )
                        .unwrap_or(node_clip)
                    } else {
                        node_clip
                    };
                    (content_right - label_width, label_width, clip)
                }
            };
            output.push(item(
                text_clip,
                z.saturating_add(8),
                node_graph_text_primitive(
                    TextPlacement {
                        origin: [text_x, center[1] - spacing.socket_label_inset_y * zoom],
                        text: socket.name.clone(),
                    },
                    theme.text_muted,
                    max_width,
                    bevy_text::Justify::Left,
                    &font,
                    theme,
                    zoom,
                ),
            ));
        }
        push_rect_outline_with_width(
            node_rect,
            if selected {
                theme.accent
            } else {
                theme.border.mix(theme.surface, 0.2)
            },
            node_clip,
            z.saturating_add(7),
            if selected { 2.0 } else { 1.0 },
            output,
        );
    }

    if matches!(target, NodeGraphPaintTarget::Chrome)
        && let Some(gesture) = state.gesture
    {
        match gesture {
            NodeGraphGesture::LinkDrag {
                start_node,
                start_socket,
                start_direction,
                current_pointer,
                ..
            } => {
                if let Some(node) = graph.nodes.iter().find(|node| node.id == start_node)
                    && let Some(start) =
                        node_graph_socket_position(graph, rect, node, start_socket, spacing)
                {
                    let control = ((current_pointer[0] - start[0]).abs() * 0.5)
                        .max(spacing.connection_tangent_min);
                    let sign = if start_direction == SocketDirection::Output {
                        1.0
                    } else {
                        -1.0
                    };
                    output.push(item(
                        viewport_clip,
                        z.saturating_add(9),
                        Primitive::Bezier {
                            points: [
                                start,
                                [start[0] + control * sign, start[1]],
                                [current_pointer[0] - control * sign, current_pointer[1]],
                                current_pointer,
                            ],
                            width: 2.0,
                            color: theme.accent,
                        },
                    ));
                }
            }
            NodeGraphGesture::BoxSelect {
                start_pointer,
                current_pointer,
                ..
            } => {
                let selection = Rect::new(
                    start_pointer[0].min(current_pointer[0]),
                    start_pointer[1].min(current_pointer[1]),
                    (start_pointer[0] - current_pointer[0]).abs(),
                    (start_pointer[1] - current_pointer[1]).abs(),
                );
                let outline = theme.accent;
                output.push(item(
                    viewport_clip,
                    z.saturating_add(9),
                    Primitive::RoundedRect {
                        rect: ClipRect::from(selection),
                        radius: 0.0,
                        color: theme.color(ThemeColorToken::NodeGraphSelectionOverlay),
                    },
                ));
                push_rect_outline(
                    selection,
                    outline,
                    viewport_clip,
                    z.saturating_add(10),
                    output,
                );
            }
            NodeGraphGesture::CutLinks { points, .. } => {
                for pair in points.windows(2) {
                    output.push(item(
                        viewport_clip,
                        z.saturating_add(10),
                        Primitive::Line {
                            from: pair[0],
                            to: pair[1],
                            width: 2.0,
                            color: theme.error,
                        },
                    ));
                }
            }
            NodeGraphGesture::Pan { .. }
            | NodeGraphGesture::MoveNodes { .. }
            | NodeGraphGesture::Selection { .. }
            | NodeGraphGesture::SelectLink { .. } => {}
        }
    }
}

pub(in crate::actions) fn node_graph_text_primitive(
    placement: TextPlacement,
    color: Rgba,
    max_width: f32,
    justify: bevy_text::Justify,
    font: &TextFont,
    theme: &Theme,
    zoom: f32,
) -> Primitive {
    Primitive::TextWrapped {
        origin: placement.origin,
        text: placement.text,
        color,
        max_width: max_width.max(1.0),
        wrap: false,
        style: Some(TextRenderStyle {
            font: Some(font.clone().with_font_size(theme.font_size * zoom)),
            line_height: Some(LineHeight::RelativeToFont(theme.line_height)),
            letter_spacing: bevy_text::LetterSpacing::default(),
            justify,
            line_break: LineBreak::NoWrap,
        }),
    }
}

pub(in crate::actions) fn node_graph_category_color(category: NodeCategory, theme: &Theme) -> Rgba {
    let surface = theme.color(ThemeColorToken::NodeGraphHeaderSurface);
    let hue = match category {
        NodeCategory::General => return surface,
        NodeCategory::Input | NodeCategory::Converter => {
            theme.color(ThemeColorToken::NodeGraphTextSocket)
        }
        NodeCategory::Texture => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        NodeCategory::Color | NodeCategory::Output => {
            theme.color(ThemeColorToken::NodeGraphColorSocket)
        }
        NodeCategory::Geometry => theme.color(ThemeColorToken::NodeGraphVectorSocket),
    };
    surface.mix(hue, 0.5)
}

pub(in crate::actions) fn node_graph_canvas_surface(
    world: &World,
    entity: Entity,
    theme: &Theme,
) -> Rgba {
    let token_surface = theme.color(ThemeColorToken::NodeGraphSurface);
    let inline_background = world
        .get::<UiInlineStyle>(entity)
        .and_then(|style| style.0.get(&UiStyleProperty::Background));
    if !node_graph_has_background_override(world, entity) {
        return token_surface;
    }
    world
        .get::<UiComputedStyle>(entity)
        .and_then(|style| style.color(UiStyleProperty::Background))
        .or_else(|| match inline_background {
            Some(UiStyleValue::Color(color)) => Some(*color),
            Some(UiStyleValue::ColorToken(token)) => Some(theme.color(*token)),
            Some(_) | None => None,
        })
        .unwrap_or(token_surface)
}

pub(in crate::actions) fn node_graph_has_background_override(
    world: &World,
    entity: Entity,
) -> bool {
    if world
        .get::<UiInlineStyle>(entity)
        .is_some_and(|style| style.0.contains_key(&UiStyleProperty::Background))
    {
        return true;
    }
    let role = world.get::<UiRole>(entity);
    let classes = world.get::<UiClasses>(entity);
    let mut state = world
        .get::<UiStyleState>(entity)
        .copied()
        .unwrap_or_default();
    if world
        .get::<UiEnabled>(entity)
        .is_some_and(|enabled| !enabled.0)
    {
        state = state.union(UiStyleState::DISABLED);
    }
    if let Some(focus) = world.get_resource::<UiFocus>() {
        for window_state in focus.windows.values() {
            if window_state.hovered == Some(entity) {
                state = state.union(UiStyleState::HOVERED);
            }
            if window_state.captured == Some(entity) {
                state = state.union(UiStyleState::PRESSED);
            }
            if window_state.focused == Some(entity) {
                state = state.union(UiStyleState::FOCUSED);
            }
        }
    }
    world.get_resource::<UiStyleSheet>().is_some_and(|sheet| {
        sheet.rules.iter().any(|rule| {
            let selector = &rule.selector;
            rule.declaration
                .0
                .contains_key(&UiStyleProperty::Background)
                && selector
                    .role
                    .as_ref()
                    .is_none_or(|wanted| role.is_some_and(|actual| &actual.0 == wanted))
                && selector
                    .class
                    .as_ref()
                    .is_none_or(|wanted| classes.is_some_and(|actual| actual.contains(wanted)))
                && state.contains(selector.required_state)
                && !state.intersects(selector.forbidden_state)
        })
    })
}

pub(in crate::actions) fn cached_node_graph_output_label_widths(
    world: &mut World,
    owner: Entity,
    node_id: NodeId,
    labels: &[(usize, String)],
    font: &TextFont,
    theme: &Theme,
    zoom: f32,
) -> HashMap<(NodeId, usize), f32> {
    let layout_available = world.contains_resource::<TextPipeline>()
        && world.contains_resource::<Assets<Font>>()
        && world.contains_resource::<FontCx>()
        && world.contains_resource::<LayoutCx>();
    let signature = format!(
        "font={font:?};size={:08x};line={:08x};zoom={:08x};available={layout_available};labels={labels:?}",
        theme.font_size.to_bits(),
        theme.line_height.to_bits(),
        zoom.to_bits(),
    );
    if let Some(measurements) = world
        .resource::<WidgetPaintCache>()
        .node_graph_output_measurements
        .get(&owner)
        .and_then(|nodes| nodes.get(&node_id))
        .filter(|measurements| measurements.signature == signature)
    {
        return measurements
            .widths
            .iter()
            .map(|(index, width)| ((node_id, *index), *width))
            .collect();
    }

    let widths = if labels.is_empty() || !layout_available {
        HashMap::new()
    } else {
        let font = font
            .clone()
            .with_font_size(theme.font_size * zoom.max(0.01));
        world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
            world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
                world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                    world.resource_scope(|_, mut layout_cx: Mut<LayoutCx>| {
                        labels
                            .iter()
                            .filter_map(|(index, text)| {
                                let layout = TextLayout::shape_with_bevy(
                                    text,
                                    f32::INFINITY,
                                    &font,
                                    &mut pipeline,
                                    &fonts,
                                    &mut font_cx,
                                    &mut layout_cx,
                                )
                                .ok()?;
                                Some((*index, layout.bounds().width.max(0.0)))
                            })
                            .collect()
                    })
                })
            })
        })
    };
    world
        .resource_mut::<WidgetPaintCache>()
        .node_graph_output_measurements
        .entry(owner)
        .or_default()
        .insert(
            node_id,
            NodeGraphOutputLabelMeasurements {
                signature,
                widths: widths.clone(),
            },
        );
    widths
        .into_iter()
        .map(|(index, width)| ((node_id, index), width))
        .collect()
}

pub(in crate::actions) fn socket_type_color(socket_type: crate::SocketType, theme: &Theme) -> Rgba {
    match socket_type {
        crate::SocketType::Any => theme.text_muted,
        crate::SocketType::Boolean => theme.color(ThemeColorToken::NodeGraphBooleanSocket),
        crate::SocketType::Float => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        crate::SocketType::Vector2 | crate::SocketType::Vector3 => {
            theme.color(ThemeColorToken::NodeGraphVectorSocket)
        }
        crate::SocketType::Color => theme.color(ThemeColorToken::NodeGraphColorSocket),
        crate::SocketType::Text => theme.color(ThemeColorToken::NodeGraphTextSocket),
    }
}

pub(in crate::actions) fn push_rect_outline(
    rect: Rect,
    color: Rgba,
    clip: ClipRect,
    z: i32,
    output: &mut Vec<DrawItem>,
) {
    push_rect_outline_with_width(rect, color, clip, z, 1.0, output);
}

pub(in crate::actions) fn push_rect_outline_with_width(
    rect: Rect,
    color: Rgba,
    clip: ClipRect,
    z: i32,
    width: f32,
    output: &mut Vec<DrawItem>,
) {
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    for (from, to) in [
        ([left, top], [right, top]),
        ([right, top], [right, bottom]),
        ([right, bottom], [left, bottom]),
        ([left, bottom], [left, top]),
    ] {
        output.push(item(
            clip,
            z,
            Primitive::Line {
                from,
                to,
                width,
                color,
            },
        ));
    }
}

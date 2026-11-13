use super::*;

pub(super) fn paint_editor_widgets(paint: &mut PaintEntityContext<'_>) -> bool {
    let world = &mut *paint.world;
    let entity = paint.entity;
    let layout = paint.layout;
    let rect = paint.rect;
    let clip = paint.clip;
    let z = paint.z;
    let theme = paint.theme;
    let base_theme = paint.base_theme;
    let enabled = paint.enabled;
    let is_box = paint.is_box;
    let camera_items = &mut *paint.output;

    if let Some(tooltip) = world.get::<Tooltip>(entity)
        && tooltip.visible
    {
        paint_tooltip_overlay(world, entity, tooltip, layout.window, theme, camera_items);
    }

    if is_box {
        camera_items.push(item(
            clip,
            z,
            Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: theme.corner_radius,
                color: theme.surface,
            },
        ));
        let border_width = world
            .get::<UiComputedStyle>(entity)
            .and_then(|style| style.scalar(UiStyleProperty::BorderWidth))
            .filter(|width| width.is_finite())
            .unwrap_or(1.0)
            .clamp(0.0, 8.0);
        if border_width > 0.0 {
            push_rect_outline_with_width(
                rect,
                theme.border,
                clip,
                z.saturating_add(1),
                border_width,
                camera_items,
            );
        }
        return true;
    }

    if let Some(mut checkbox) = world.get::<Checkbox>(entity).cloned() {
        checkbox.enabled &= enabled;
        if world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "property-decorator")
        {
            let size = if checkbox.is_checked() { 6.0 } else { 3.0 };
            camera_items.push(item(
                clip,
                z,
                Primitive::Ellipse {
                    rect: ClipRect {
                        x: rect.x + (rect.width - size) * 0.5,
                        y: rect.y + (rect.height - size) * 0.5,
                        width: size,
                        height: size,
                    },
                    color: if checkbox.is_checked() {
                        theme.accent
                    } else {
                        theme.text_muted
                    },
                },
            ));
            return true;
        }
        if world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "filled-checkbox")
        {
            let size = rect.height.min(13.0);
            let square = ClipRect {
                x: rect.x,
                y: rect.y + (rect.height - size) * 0.5,
                width: size,
                height: size,
            };
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: square,
                    radius: 2.0,
                    color: if checkbox.is_checked() {
                        theme.accent
                    } else {
                        base_theme.text_muted.mix(base_theme.surface, 0.4)
                    },
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        x: square.x + 1.0,
                        y: square.y + 1.0,
                        width: (square.width - 2.0).max(0.0),
                        height: (square.height - 2.0).max(0.0),
                    },
                    radius: 1.0,
                    color: if checkbox.is_checked() {
                        theme.accent
                    } else {
                        base_theme.surface_hover
                    },
                },
            ));
            if checkbox.is_checked() {
                camera_items.push(item(
                    clip,
                    z.saturating_add(1),
                    styled_text_primitive_for_entity(
                        world,
                        entity,
                        clip,
                        TextPlacement {
                            origin: [square.x + 1.0, square.y - 2.0],
                            text: "✓".into(),
                        },
                        theme.text,
                        theme,
                        false,
                    ),
                ));
            }
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [square.x + size + 4.0, rect.y + 2.0],
                        text: checkbox.label,
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            return true;
        }
        if world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "segmented-toggle")
        {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: 2.0,
                    color: if checkbox.is_checked() {
                        theme.accent
                    } else {
                        theme.surface_pressed
                    },
                },
            ));
            let text_width = checkbox.label.chars().count() as f32 * theme.font_size * 0.55;
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + (rect.width - text_width).max(0.0) * 0.5,
                            rect.y + theme.spacing.control.padding_y,
                        ],
                        text: checkbox.label,
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            return true;
        }
        camera_items.extend(checkbox.draw_items_clipped(rect, layout.clip, theme, z));
        return true;
    }
    if let Some(group) = world.get::<RadioGroup>(entity) {
        paint_radio_group(
            RadioGroupPaint {
                group,
                label: world
                    .get::<Label>(entity)
                    .map_or("Options", |label| label.text.as_str()),
                rect,
                clip,
                z,
                enabled,
                theme,
            },
            camera_items,
        );
        return true;
    }
    if let Some(ramp) = world.get::<ColorRamp>(entity) {
        paint_color_ramp(
            ColorRampPaint {
                ramp,
                state: world
                    .get::<ColorRampEditorState>(entity)
                    .copied()
                    .unwrap_or_default(),
                label: world
                    .get::<Label>(entity)
                    .map_or("Color ramp", |label| label.text.as_str()),
                rect,
                clip,
                z,
                theme,
            },
            camera_items,
        );
        return true;
    }
    if let Some(editor) = world.get::<HsvColorEditor>(entity).copied() {
        paint_hsv_color_editor(
            HsvColorPaint {
                editor,
                state: world
                    .get::<HsvColorEditorState>(entity)
                    .cloned()
                    .unwrap_or_default(),
                label: world
                    .get::<Label>(entity)
                    .map_or("Color", |label| label.text.as_str()),
                rect,
                clip,
                z,
                theme,
            },
            camera_items,
        );
        return true;
    }
    if let Some(editor) = world.get::<CurveEditor>(entity) {
        paint_curve_editor(
            CurveEditorPaint {
                editor,
                label: world
                    .get::<Label>(entity)
                    .map_or("Curve", |label| label.text.as_str()),
                rect,
                clip,
                z,
                theme,
            },
            camera_items,
        );
        return true;
    }
    if let Some(graph) = world.get::<NodeGraph>(entity) {
        let font = resolve_text_font(world, entity, ui_default_text_font(theme.font_size));
        let canvas_surface = node_graph_canvas_surface(world, entity, base_theme);
        paint_node_graph_target(
            NodeGraphPaint {
                graph,
                state: world
                    .get::<NodeGraphEditorState>(entity)
                    .cloned()
                    .unwrap_or_default(),
                output_label_widths: HashMap::new(),
                registry: world.get_resource::<NodeGraphTypeRegistry>(),
                canvas_surface,
                label: world
                    .get::<Label>(entity)
                    .map_or("Node graph", |label| label.text.as_str()),
                rect,
                clip,
                z,
                theme,
                font,
            },
            NodeGraphPaintTarget::Chrome,
            camera_items,
        );
        return true;
    }
    if let Some(node_control) = world.get::<NodeGraphNodeControl>(entity) {
        let owner = node_control.owner;
        let node_id = node_control.node_id;
        let Some(owner_layout) = world.get::<UiLayoutResult>(owner).copied() else {
            return true;
        };
        let owner_clip = match owner_layout.clip {
            Some(parent_clip) => {
                let Some(clip) = parent_clip.intersection(owner_layout.rect) else {
                    return true;
                };
                ClipRect::from(clip)
            }
            None => ClipRect::from(owner_layout.rect),
        };
        let owner_theme = styled_theme(world, owner, base_theme);
        let font = resolve_text_font(world, owner, ui_default_text_font(owner_theme.font_size));
        let Some(node_index) = world
            .resource::<WidgetNodeGraphItems>()
            .node_indices
            .get(&owner)
            .and_then(|indices| indices.get(&node_id))
            .copied()
        else {
            return true;
        };
        let (zoom, output_labels) = {
            let Some(graph) = world.get::<NodeGraph>(owner) else {
                return true;
            };
            let Some(node) = graph
                .nodes
                .get(node_index)
                .filter(|node| node.id == node_id)
            else {
                return true;
            };
            (
                graph.zoom,
                node.sockets
                    .iter()
                    .enumerate()
                    .filter(|(_, socket)| {
                        socket.direction == SocketDirection::Output
                            && socket.placement == SocketPlacement::Auto
                    })
                    .map(|(index, socket)| (index, socket.name.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        let output_label_widths = cached_node_graph_output_label_widths(
            world,
            owner,
            node_id,
            &output_labels,
            &font,
            &owner_theme,
            zoom,
        );
        let Some(graph) = world.get::<NodeGraph>(owner) else {
            return true;
        };
        let Some(node) = graph
            .nodes
            .get(node_index)
            .filter(|node| node.id == node_id)
        else {
            return true;
        };
        paint_node_graph_target(
            NodeGraphPaint {
                graph,
                state: NodeGraphEditorState::default(),
                output_label_widths,
                registry: world.get_resource::<NodeGraphTypeRegistry>(),
                canvas_surface: owner_theme.color(ThemeColorToken::NodeGraphSurface),
                label: "Node graph",
                rect: owner_layout.rect,
                clip: owner_clip,
                z: owner_layout.z,
                theme: &owner_theme,
                font,
            },
            NodeGraphPaintTarget::Node(node),
            camera_items,
        );
        return true;
    }
    if let Some(link_control) = world.get::<NodeGraphLinkControl>(entity) {
        let owner = link_control.owner;
        let Some(owner_layout) = world.get::<UiLayoutResult>(owner).copied() else {
            return true;
        };
        let owner_clip = match owner_layout.clip {
            Some(parent_clip) => {
                let Some(clip) = parent_clip.intersection(owner_layout.rect) else {
                    return true;
                };
                ClipRect::from(clip)
            }
            None => ClipRect::from(owner_layout.rect),
        };
        let owner_theme = styled_theme(world, owner, base_theme);
        let font = resolve_text_font(world, owner, ui_default_text_font(owner_theme.font_size));
        let indices = world.resource::<WidgetNodeGraphItems>();
        let Some(link_index) = indices
            .link_indices
            .get(&owner)
            .and_then(|links| links.get(&link_control.link_id))
            .copied()
        else {
            return true;
        };
        let (Some(graph), Some(node_indices)) = (
            world.get::<NodeGraph>(owner),
            indices.node_indices.get(&owner),
        ) else {
            return true;
        };
        let Some(link) = graph.links.get(link_index).copied() else {
            return true;
        };
        let (Some(from), Some(to)) = (
            node_indices
                .get(&link.from_node)
                .and_then(|index| graph.nodes.get(*index)),
            node_indices
                .get(&link.to_node)
                .and_then(|index| graph.nodes.get(*index)),
        ) else {
            return true;
        };
        paint_node_graph_target(
            NodeGraphPaint {
                graph,
                state: NodeGraphEditorState::default(),
                output_label_widths: HashMap::new(),
                registry: world.get_resource::<NodeGraphTypeRegistry>(),
                canvas_surface: owner_theme.color(ThemeColorToken::NodeGraphSurface),
                label: "Node graph",
                rect: owner_layout.rect,
                clip: owner_clip,
                z: owner_layout.z,
                theme: &owner_theme,
                font,
            },
            NodeGraphPaintTarget::Link { link, from, to },
            camera_items,
        );
        return true;
    }
    if let Some(control) = world.get::<crate::canvas::CanvasObjectControl>(entity) {
        let owner = control.owner;
        let (Some(canvas), Some(paint), Some(owner_layout)) = (
            world.get::<Canvas>(owner),
            world.get::<crate::canvas::CanvasObjectPaint>(entity),
            world.get::<UiLayoutResult>(owner).copied(),
        ) else {
            return true;
        };
        let Some(owner_clip) = owner_layout
            .clip
            .unwrap_or(owner_layout.rect)
            .intersection(owner_layout.rect)
        else {
            return true;
        };
        let owner_theme = styled_theme(world, owner, base_theme);
        crate::canvas::paint_canvas_object_fragment(
            canvas,
            &paint.0,
            owner_layout.rect,
            ClipRect::from(owner_clip),
            owner_layout.z,
            &owner_theme,
            world.get_resource::<crate::canvas::CanvasImageAssets>(),
            camera_items,
        );
        return true;
    }
    if world.get::<Canvas>(entity).is_some() {
        crate::canvas::paint_canvas_background(rect, clip, z, theme, camera_items);
        return true;
    }
    if let Some(header) = world.get::<DataTableHeaderControl>(entity).cloned() {
        paint_data_table_header(&header, rect, clip, z, theme, camera_items);
        return true;
    }
    if let Some(cell) = world.get::<DataTableCellControl>(entity).cloned() {
        paint_data_table_cell(&cell, rect, clip, z, theme, camera_items);
        return true;
    }
    if let Some(table) = world.get::<DataTable>(entity) {
        let label = world
            .get::<Label>(entity)
            .map_or("Table", |label| label.text.as_str());
        paint_data_table_root(table, label, rect, clip, z, theme, camera_items);
        return true;
    }
    if let Some(collection_item) = world.get::<CollectionItemControl>(entity).cloned() {
        let mut display_item = collection_item.clone();
        if let Some(state) = world.get::<CollectionEditorState>(collection_item.owner)
            && state.rename_id == Some(collection_item.id)
        {
            let cursor = state
                .rename_buffer
                .char_indices()
                .nth(state.rename_cursor)
                .map_or(state.rename_buffer.len(), |(index, _)| index);
            display_item.label = format!(
                "{}|{}",
                &state.rename_buffer[..cursor],
                &state.rename_buffer[cursor..]
            );
        }
        let owner_theme = styled_theme(world, collection_item.owner, theme);
        paint_collection_item(
            &display_item,
            rect,
            clip,
            z,
            enabled,
            &owner_theme,
            camera_items,
        );
        return true;
    }
    false
}

use super::*;

#[test]
pub(super) fn menu_and_node_graph_painters_consume_semantic_theme_tokens() {
    for kind in [bbui_style::ThemeKind::Dark, bbui_style::ThemeKind::Light] {
        let theme = Theme::new(kind);
        let rect = UiRect::new(20.0, 30.0, 360.0, 240.0);
        let clip = ClipRect::from(rect);
        let mut graph_paint = Vec::new();
        paint_node_graph(
            NodeGraphPaint {
                registry: None,
                graph: &NodeGraph::default(),
                state: NodeGraphEditorState {
                    gesture: Some(NodeGraphGesture::BoxSelect {
                        start_pointer: [60.0, 100.0],
                        current_pointer: [140.0, 160.0],
                        additive: false,
                        before: String::new(),
                    }),
                },
                output_label_widths: HashMap::new(),
                canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
                label: "Graph",
                rect,
                clip,
                z: 1,
                theme: &theme,
                font: ui_default_text_font(theme.font_size),
            },
            &mut graph_paint,
        );
        assert!(graph_paint.iter().any(|item| matches!(
                item.primitive,
                Primitive::Ellipse { color, .. }
                    if color == theme.color(ThemeColorToken::NodeGraphGridLine).mix(theme.color(ThemeColorToken::NodeGraphSurface), 0.6)
            )));
        assert!(graph_paint.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { color, .. }
                if color == theme.color(ThemeColorToken::NodeGraphSelectionOverlay)
        )));

        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        let window = app.world_mut().spawn_empty().id();
        let mut menu = MenuControl::new(
            "File",
            crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
            MenuDisplay::Dropdown,
        );
        menu.open(None);
        menu.popup.open = true;
        menu.popup.bounds = UiRect::new(8.0, 32.0, 100.0, 44.0);
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: UiRect::new(8.0, 4.0, 100.0, 24.0),
                clip: None,
                z: 1,
                content_size: [100.0, 24.0],
                dpi_scale: 1.0,
            },
            menu,
        ));
        collect_widget_paint(app.world_mut());
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(
                    item.primitive,
                    Primitive::RoundedBorder { color, radius, .. }
                        if color == theme.color(ThemeColorToken::MenuBorder)
                            && radius == theme.corner_radius
                ))
        );
    }
}

#[test]
pub(super) fn node_graph_painter_keeps_category_headers_and_clips_zoomed_text() {
    let theme = Theme::new(bbui_style::ThemeKind::Dark);
    let mut graph = NodeGraph::default();
    let node_id = graph.add_node_with_category(
        "A deliberately long texture node title",
        [18.0, 20.0],
        vec![
            NodeSocket::input("A deliberately long input label", crate::SocketType::Float),
            NodeSocket::output("A deliberately long output label", crate::SocketType::Color),
        ],
        NodeCategory::Texture,
    );
    graph.zoom = 1.5;
    graph.selected.insert(node_id);
    let rect = Rect::new(20.0, 16.0, 360.0, 240.0);
    let clip = ClipRect::from(rect);
    let node = graph.nodes.iter().find(|node| node.id == node_id).unwrap();
    let node_rect = node_graph_node_rect(&graph, rect, node, &theme.spacing.node_graph);
    let mut items = Vec::new();
    let paint = || NodeGraphPaint {
        registry: None,
        graph: &graph,
        state: NodeGraphEditorState::default(),
        output_label_widths: HashMap::from([((node_id, 1), 44.0)]),
        canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
        label: "Texture graph",
        rect,
        clip,
        z: 1,
        theme: &theme,
        font: ui_default_text_font(theme.font_size),
    };
    paint_node_graph_target(paint(), NodeGraphPaintTarget::Chrome, &mut items);
    paint_node_graph_target(paint(), NodeGraphPaintTarget::Node(node), &mut items);

    let category_header = node_graph_category_color(NodeCategory::Texture, &theme);
    assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect: header, color, .. }
                if (header.x - node_rect.x).abs() < 0.01
                    && (header.y - node_rect.y).abs() < 0.01
                    && (header.height - theme.spacing.node_graph.header_height * graph.zoom).abs() < 0.01
                    && color == category_header
        )));
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::Line { color, width, .. }
            if color == theme.accent && (width - 2.0).abs() < 0.01
    )));

    for text in [
        "A deliberately long texture node title",
        "A deliberately long input label",
        "A deliberately long output label",
    ] {
        let draw = items
                .iter()
                .find(|item| matches!(&item.primitive, Primitive::TextWrapped { text: candidate, .. } if candidate == text))
                .unwrap_or_else(|| panic!("missing node graph label {text:?}"));
        assert!(draw.clip.x >= node_rect.x - 0.01);
        assert!(draw.clip.x + draw.clip.width <= node_rect.x + node_rect.width + 0.01);
        assert!(draw.clip.y >= node_rect.y - 0.01);
        assert!(draw.clip.y + draw.clip.height <= node_rect.y + node_rect.height + 0.01);
        let Primitive::TextWrapped {
            max_width, style, ..
        } = &draw.primitive
        else {
            panic!();
        };
        assert!(*max_width <= node_rect.width);
        let font_size = style.as_ref().unwrap().font.as_ref().unwrap().font_size;
        assert!(
            matches!(font_size, FontSize::Px(size) if (size - theme.font_size * graph.zoom).abs() < 0.01)
        );
    }
    let output = items
            .iter()
            .find(|item| {
                matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "A deliberately long output label")
            })
            .unwrap();
    let Primitive::TextWrapped {
        origin, max_width, ..
    } = &output.primitive
    else {
        panic!();
    };
    let content_right =
        node_rect.x + node_rect.width - theme.spacing.node_graph.socket_label_inset_x * graph.zoom;
    assert!((origin[0] - (content_right - 45.0)).abs() < 0.01);
    assert!((*max_width - 45.0).abs() < 0.01);
    assert!(output.clip.x >= node_rect.x + node_rect.width * 0.49);
}

#[test]
pub(super) fn node_graph_default_height_tracks_socket_rows_and_preserves_explicit_size() {
    let theme = Theme::new(bbui_style::ThemeKind::Dark);
    let mut graph = NodeGraph::default();
    let one_row = graph.add_node(
        "One row",
        [10.0, 10.0],
        vec![NodeSocket::input("Input", crate::SocketType::Float)],
    );
    let two_rows = graph.add_node(
        "Two rows",
        [220.0, 10.0],
        vec![
            NodeSocket::input("First", crate::SocketType::Float),
            NodeSocket::input("Second", crate::SocketType::Float),
        ],
    );
    let rect = Rect::new(0.0, 0.0, 500.0, 300.0);
    let node = graph.nodes.iter().find(|node| node.id == one_row).unwrap();
    assert_eq!(node.size[1], 58.0);
    assert_eq!(
        node_graph_node_rect(&graph, rect, node, &theme.spacing.node_graph).height,
        58.0
    );
    let node = graph.nodes.iter().find(|node| node.id == two_rows).unwrap();
    assert_eq!(
        node_graph_node_rect(&graph, rect, node, &theme.spacing.node_graph).height,
        80.0
    );
    graph.nodes[0].collapsed = true;
    assert_eq!(
        node_graph_node_rect(&graph, rect, &graph.nodes[0], &theme.spacing.node_graph).height,
        theme.spacing.node_graph.header_height
    );
    graph.nodes[1].size[1] = 126.0;
    assert_eq!(
        node_graph_node_rect(&graph, rect, &graph.nodes[1], &theme.spacing.node_graph).height,
        126.0
    );
}

#[test]
pub(super) fn node_graph_canvas_uses_its_token_and_respects_explicit_background_styles() {
    let theme = Theme::new(bbui_style::ThemeKind::Dark);
    let mut world = World::new();
    let entity = world.spawn((UiNode, UiRole::new("node-graph"))).id();
    let token_surface = theme.color(ThemeColorToken::NodeGraphSurface);
    assert_eq!(
        node_graph_canvas_surface(&world, entity, &theme),
        token_surface
    );
    assert_ne!(token_surface, theme.surface);

    let custom_color = Rgba(0.36, 0.12, 0.18, 1.0);
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(custom_color),
    );
    world.entity_mut(entity).insert((
        inline,
        UiComputedStyle {
            properties: HashMap::from([(
                UiStyleProperty::Background,
                UiStyleValue::Color(custom_color),
            )]),
        },
    ));
    assert_eq!(
        node_graph_canvas_surface(&world, entity, &theme),
        custom_color
    );

    world.entity_mut(entity).remove::<UiInlineStyle>();
    let mut declaration = UiStyleDeclaration::default();
    declaration.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(custom_color),
    );
    world.insert_resource(UiStyleSheet {
        rules: vec![UiStyleRule {
            selector: UiStyleSelector {
                role: Some("node-graph".to_owned()),
                ..Default::default()
            },
            declaration,
            priority: 0,
        }],
    });
    assert_eq!(
        node_graph_canvas_surface(&world, entity, &theme),
        custom_color
    );
}

#[test]
pub(super) fn node_graph_hover_does_not_recolor_its_grid_or_nodes() {
    let theme = Theme::new(bbui_style::ThemeKind::Dark);
    let mut world = World::new();
    let entity = world
        .spawn((
            NodeGraph::default(),
            UiComputedStyle {
                properties: HashMap::from([(
                    UiStyleProperty::Background,
                    UiStyleValue::Color(theme.surface_hover),
                )]),
            },
        ))
        .id();
    let idle = styled_theme(&world, entity, &theme);
    assert_eq!(idle.background, theme.background);
    assert_eq!(idle.surface, theme.surface);
    assert_eq!(
        node_graph_canvas_surface(&world, entity, &theme),
        theme.color(ThemeColorToken::NodeGraphSurface)
    );

    let custom_color = Rgba(0.36, 0.12, 0.18, 1.0);
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(custom_color),
    );
    world.entity_mut(entity).insert((
        inline,
        UiComputedStyle {
            properties: HashMap::from([(
                UiStyleProperty::Background,
                UiStyleValue::Color(custom_color),
            )]),
        },
    ));
    assert_eq!(styled_theme(&world, entity, &theme).surface, custom_color);
    assert_eq!(
        node_graph_canvas_surface(&world, entity, &theme),
        custom_color
    );
}

#[test]
pub(super) fn node_graph_focus_does_not_recolor_its_grid_or_unselected_nodes() {
    let theme = Theme::new(bbui_style::ThemeKind::Dark);
    let mut world = World::new();
    let entity = world
        .spawn((
            NodeGraph::default(),
            UiRole::new("node-graph"),
            UiComputedStyle {
                properties: HashMap::from([(
                    UiStyleProperty::BorderColor,
                    UiStyleValue::Color(theme.focus),
                )]),
            },
        ))
        .id();
    assert_eq!(styled_theme(&world, entity, &theme).border, theme.border);

    let custom_border = Rgba(0.36, 0.12, 0.18, 1.0);
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::BorderColor,
        UiStyleValue::Color(custom_border),
    );
    world.entity_mut(entity).insert((
        inline,
        UiComputedStyle {
            properties: HashMap::from([(
                UiStyleProperty::BorderColor,
                UiStyleValue::Color(custom_border),
            )]),
        },
    ));
    assert_eq!(styled_theme(&world, entity, &theme).border, custom_border);

    world.entity_mut(entity).remove::<UiInlineStyle>();
    let mut declaration = UiStyleDeclaration::default();
    declaration.set(
        UiStyleProperty::BorderColor,
        UiStyleValue::Color(custom_border),
    );
    world.insert_resource(UiStyleSheet {
        rules: vec![UiStyleRule {
            selector: UiStyleSelector {
                role: Some("node-graph".to_owned()),
                ..Default::default()
            },
            declaration,
            priority: 0,
        }],
    });
    assert_eq!(styled_theme(&world, entity, &theme).border, custom_border);
}

#[test]
pub(super) fn tree_text_field_uses_shared_buffer_and_external_clipboard_requests() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::text_field("Name", "old"))
        .spawn(app.world_mut(), None)
        .unwrap();
    assert!(
        app.world()
            .get::<bbui_text::UiExternalClipboard>(owner)
            .is_some()
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "!".into(),
    }];
    bbui_text::process_text_events(app.world_mut());
    sync_text_controls(app.world_mut());
    assert_eq!(
        app.world().get::<TextBuffer>(owner).unwrap().as_str(),
        "old!"
    );
    assert_eq!(app.world().get::<TextField>(owner).unwrap().text, "old!");

    app.world_mut()
        .resource_mut::<UiRoutedEvents>()
        .events
        .clear();
    app.world_mut()
        .get_mut::<TextBuffer>(owner)
        .unwrap()
        .select_all();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: owner,
        key: UiKeyCode::X,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..Default::default()
        },
    }];
    process_routed_widget_events(app.world_mut());
    assert_eq!(app.world().get::<TextBuffer>(owner).unwrap().as_str(), "");
    assert!(
        app.world_mut()
            .resource_mut::<ClipboardRequestQueue>()
            .drain()
            .any(|request| request == ClipboardRequest::Write("old!".into()))
    );
}

#[test]
pub(super) fn curve_editor_paints_gpu_curve_grid_points_and_bezier_handles() {
    let mut app = App::new();
    let (window, entity) = spawn_curve_editor_widget(&mut app, UiRect::new(0.0, 0.0, 360.0, 240.0));
    {
        let mut editor = app.world_mut().get_mut::<CurveEditor>(entity).unwrap();
        editor.interpolation = Interpolation::Smooth;
        let id = editor.add_point(0.5, 0.72).unwrap();
        editor.selected = Some(id);
        editor.set_handles(id, [-0.11, -0.08], [0.13, 0.12]);
    }
    app.update();

    let items = app.world().resource::<DisplayList>().widget_items();
    assert!(items.len() >= 24);
    assert!(items.iter().all(|item| item.z >= 11));
    assert!(
        items
            .iter()
            .any(|item| matches!(item.primitive, Primitive::Bezier { .. }))
    );
    assert!(
        items
            .iter()
            .filter(|item| matches!(item.primitive, Primitive::Line { .. }))
            .count()
            >= 12
    );
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { rect, .. }
            if rect.width
                == curve_plot_rect(UiRect::new(0.0, 0.0, 360.0, 240.0), &Theme::default())
                    .width
    )));
    assert_ne!(window, Entity::PLACEHOLDER);
}

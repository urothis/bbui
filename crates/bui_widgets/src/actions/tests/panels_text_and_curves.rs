use super::*;

#[test]
pub(super) fn reorderable_panel_group_routes_keyboard_drag_cancel_and_persists_order() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, bui_layout::LayoutPlugin, WidgetPlugin));
    let window = app.world_mut().spawn_empty().id();
    let make_group = |world: &mut World, order: [&str; 3]| {
        let group = world
            .spawn((
                UiNode,
                UiKey("settings".into()),
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 240.0, 300.0), 1.0),
                UiLayoutNode::column(4.0),
                PanelGroup::with_panels(order).with_reordering("settings"),
            ))
            .id();
        let mut panels = HashMap::new();
        for key in order {
            let mut panel = Panel::new(key, false);
            panel.persistence_key = Some(key.to_owned());
            let entity = world
                .spawn((
                    UiNode,
                    UiVisible(true),
                    UiEnabled(true),
                    UiFocusable,
                    UiKey(key.to_owned()),
                    UiLayoutNode::leaf(
                        bui_layout::UiLength::Fill(1.0),
                        bui_layout::UiLength::Px(80.0),
                    ),
                    panel,
                ))
                .id();
            set_parent(world, entity, group).unwrap();
            panels.insert(key.to_owned(), entity);
        }
        (group, panels)
    };
    let (group, panels) = make_group(app.world_mut(), ["alpha", "beta", "gamma"]);
    app.update();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(panels["alpha"]);

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window,
            key: UiKeyCode::ArrowDown,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        });
    app.update();
    app.update();
    let after_keyboard = vec!["beta".to_owned(), "alpha".to_owned(), "gamma".to_owned()];
    assert_eq!(
        app.world().get::<PanelGroup>(group).unwrap().panel_keys,
        after_keyboard
    );
    assert_eq!(
        app.world().get::<UiChildren>(group).unwrap().0,
        [panels["beta"], panels["alpha"], panels["gamma"]]
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelOrderCommitted { group: target, persistence_key, before, after } if *target == group && persistence_key == "settings" && before == &["alpha", "beta", "gamma"] && after == &after_keyboard)));
    assert_eq!(
        app.world().resource::<PanelOrderStore>().get("settings"),
        Some(after_keyboard.as_slice())
    );

    let alpha_y = app
        .world()
        .get::<UiLayoutResult>(panels["alpha"])
        .unwrap()
        .rect
        .y;
    let gamma_y = app
        .world()
        .get::<UiLayoutResult>(panels["gamma"])
        .unwrap()
        .rect
        .y;
    let beta_y = app
        .world()
        .get::<UiLayoutResult>(panels["beta"])
        .unwrap()
        .rect
        .y;
    assert!(
        beta_y < alpha_y && alpha_y < gamma_y,
        "sibling layout should follow keyed order: beta={beta_y}, alpha={alpha_y}, gamma={gamma_y}"
    );

    let start = [8.0, alpha_y + 8.0];
    let end = [8.0, 290.0];
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 1.0,
        },
        UiInputEvent::PointerMove {
            window,
            position: end,
            time_seconds: 1.2,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: end,
            time_seconds: 1.3,
        },
    ]);
    app.update();
    let after_drag = vec!["beta".to_owned(), "gamma".to_owned(), "alpha".to_owned()];
    assert_eq!(
        app.world().get::<PanelGroup>(group).unwrap().panel_keys,
        after_drag
    );
    assert!(
        !app.world()
            .get::<Panel>(panels["alpha"])
            .unwrap()
            .state
            .target_expanded
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelOrderCommitted { group: target, before, after, .. } if *target == group && before == &after_keyboard && after == &after_drag)));
    assert_eq!(
        app.world().resource::<PanelOrderStore>().get("settings"),
        Some(after_drag.as_slice())
    );

    let beta = panels["beta"];
    let beta_y = app.world().get::<UiLayoutResult>(beta).unwrap().rect.y;
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [8.0, beta_y + 8.0],
            time_seconds: 2.0,
        },
        UiInputEvent::PointerMove {
            window,
            position: [8.0, 250.0],
            time_seconds: 2.2,
        },
    ]);
    app.update();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerCancel { window });
    app.update();
    assert_eq!(
        app.world().get::<PanelGroup>(group).unwrap().panel_keys,
        after_drag
    );
    assert_eq!(
        app.world().resource::<PanelOrderStore>().get("settings"),
        Some(after_drag.as_slice()),
        "cancel restores the persisted committed order"
    );

    for entity in [panels["alpha"], panels["beta"], panels["gamma"], group] {
        app.world_mut().despawn(entity);
    }
    let (rebuilt, rebuilt_panels) = make_group(app.world_mut(), ["alpha", "beta", "gamma"]);
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<PanelGroup>(rebuilt).unwrap().panel_keys,
        after_drag
    );
    assert_eq!(
        app.world().get::<UiChildren>(rebuilt).unwrap().0,
        [
            rebuilt_panels["beta"],
            rebuilt_panels["gamma"],
            rebuilt_panels["alpha"]
        ]
    );
    assert_eq!(
        app.world().get::<UiKey>(rebuilt).unwrap().0,
        "settings",
        "the reorder group's stable UiKey survives tree reconstruction"
    );
}

#[test]
pub(super) fn panel_widget_system_snaps_for_zero_scale_and_reduced_motion() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut()
        .init_resource::<bui_style::UiReducedMotion>();
    let panel = app.world_mut().spawn(Panel::new("Motion", false)).id();

    app.world_mut().resource_mut::<Theme>().motion_scale = 0.0;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert!(state.target_expanded);
    assert_eq!(state.progress, 1.0);

    app.world_mut().resource_mut::<Theme>().motion_scale = 1.0;
    app.world_mut()
        .resource_mut::<bui_style::UiReducedMotion>()
        .0 = true;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert!(!state.target_expanded);
    assert_eq!(state.progress, 0.0);
}

#[test]
pub(super) fn panel_widget_uses_typed_theme_duration_and_leaves_settled_state_unchanged() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, WidgetPlugin))
        .init_resource::<bevy_time::Time>()
        .init_resource::<bui_style::UiReducedMotion>();
    let panel = app.world_mut().spawn(Panel::new("Motion", false)).id();
    app.world_mut().resource_mut::<Theme>().motion.panel_expand =
        std::time::Duration::from_millis(400);
    app.update();

    let panel_id = app.world().components().component_id::<Panel>().unwrap();
    let changed_before = app
        .world()
        .get_entity(panel)
        .unwrap()
        .get_change_ticks_by_id(panel_id)
        .unwrap()
        .changed
        .get();
    app.update();
    let changed_after = app
        .world()
        .get_entity(panel)
        .unwrap()
        .get_change_ticks_by_id(panel_id)
        .unwrap()
        .changed
        .get();
    assert_eq!(changed_after, changed_before, "settled panels stay clean");

    app.world_mut()
        .resource_mut::<bevy_time::Time>()
        .advance_by(std::time::Duration::from_millis(200));
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert_eq!(state.animation_seconds, 0.4);
    assert!((state.progress - 0.5).abs() < 0.001);

    app.world_mut()
        .resource_mut::<bui_style::UiReducedMotion>()
        .0 = true;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
    app.update();
    let state = &app.world().get::<Panel>(panel).unwrap().state;
    assert_eq!(state.progress, 0.0, "reduced motion keeps toggling usable");
}

#[test]
pub(super) fn panel_clip_sync_is_idle_safe_and_restores_clips_after_reopen_or_reparent() {
    let mut app = App::new();
    app.init_resource::<Theme>()
        .init_resource::<WidgetPanelSyncCache>()
        .init_resource::<WidgetPanelSyncInvalidations>()
        .add_systems(
            Update,
            (collect_panel_descendant_invalidations, sync_panel_children).chain(),
        );
    let window = app.world_mut().spawn_empty().id();
    let panel_rect = Rect::new(0.0, 0.0, 160.0, 120.0);
    let panel = app
        .world_mut()
        .spawn((
            UiNode,
            Panel::new("Details", false),
            UiLayoutResult {
                window,
                rect: panel_rect,
                clip: None,
                z: 1,
                content_size: [panel_rect.width, panel_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let child_rect = Rect::new(8.0, 32.0, 140.0, 24.0);
    let child = app
        .world_mut()
        .spawn((
            UiNode,
            UiVisible(true),
            crate::WidgetElement {
                key: None,
                spec: crate::declarative::WidgetSpec::Label {
                    text: "Child".into(),
                },
            },
            UiLayoutResult {
                window,
                rect: child_rect,
                clip: None,
                z: 2,
                content_size: [child_rect.width, child_rect.height],
                dpi_scale: 1.0,
            },
            UiHitBox {
                window: Some(window),
                rect: to_ui_rect(child_rect),
                clip: None,
                z_index: 2,
            },
        ))
        .id();
    set_parent(app.world_mut(), child, panel).unwrap();

    app.update();
    let collapsed_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert_eq!(collapsed_clip.height, 0.0);
    app.update();
    app.update();
    let stable_layout_tick =
        entity_component_changed_tick::<UiLayoutResult>(app.world(), child).unwrap();
    let stable_hitbox_tick = entity_component_changed_tick::<UiHitBox>(app.world(), child).unwrap();
    app.update();
    assert_eq!(
        entity_component_changed_tick::<UiLayoutResult>(app.world(), child),
        Some(stable_layout_tick),
        "idle collapsed panels must not rewrite descendant layouts"
    );
    assert_eq!(
        entity_component_changed_tick::<UiHitBox>(app.world(), child),
        Some(stable_hitbox_tick),
        "idle collapsed panels must not rewrite descendant hitboxes"
    );

    {
        let mut layout = app.world_mut().get_mut::<UiLayoutResult>(child).unwrap();
        layout.rect.y += 4.0;
        layout.clip = None;
    }
    app.update();
    let direct_edit_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert_eq!(direct_edit_clip.height, 0.0);
    assert_eq!(
        app.world().get::<UiLayoutResult>(child).unwrap().rect.y,
        36.0
    );

    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.25;
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .target_expanded = true;
    app.update();
    let quarter_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert!(quarter_clip.height > 0.0);
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.5;
    app.update();
    let half_clip = app
        .world()
        .get::<UiLayoutResult>(child)
        .unwrap()
        .clip
        .unwrap();
    assert!(half_clip.height > quarter_clip.height);
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 1.0;
    app.update();
    assert_eq!(app.world().get::<UiLayoutResult>(child).unwrap().clip, None);
    assert_eq!(app.world().get::<UiHitBox>(child).unwrap().clip, None);

    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.0;
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .target_expanded = false;
    app.update();
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .progress = 0.25;
    app.world_mut()
        .get_mut::<Panel>(panel)
        .unwrap()
        .state
        .target_expanded = true;
    app.update();
    assert!(
        app.world()
            .get::<UiLayoutResult>(child)
            .unwrap()
            .clip
            .unwrap()
            .height
            > 0.0
    );

    detach_from_parent(app.world_mut(), child);
    app.update();
    assert_eq!(app.world().get::<UiLayoutResult>(child).unwrap().clip, None);
    assert_eq!(app.world().get::<UiHitBox>(child).unwrap().clip, None);
    assert!(
        !app.world()
            .resource::<WidgetPanelSyncCache>()
            .clips
            .contains_key(&child)
    );
}

#[test]
pub(super) fn menu_and_node_graph_painters_consume_semantic_theme_tokens() {
    for kind in [bui_style::ThemeKind::Dark, bui_style::ThemeKind::Light] {
        let theme = Theme::new(kind);
        let rect = UiRect::new(20.0, 30.0, 360.0, 240.0);
        let clip = ClipRect::from(rect);
        let mut graph_paint = Vec::new();
        paint_node_graph(
            NodeGraphPaint {
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
    let theme = Theme::new(bui_style::ThemeKind::Dark);
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
    let theme = Theme::new(bui_style::ThemeKind::Dark);
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
    let theme = Theme::new(bui_style::ThemeKind::Dark);
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
pub(super) fn panel_pointer_click_toggles_only_from_header() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let panel = app.world_mut().spawn(Panel::new("Details", false)).id();
    app.world_mut().entity_mut(panel).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 180.0, 100.0),
        clip: None,
        z: 1,
        content_size: [180.0, 100.0],
        dpi_scale: 1.0,
    });

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: panel,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [12.0, 45.0],
    }];
    app.update();
    assert!(
        !app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: panel,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [12.0, 12.0],
    }];
    app.update();
    assert!(
        app.world()
            .get::<Panel>(panel)
            .unwrap()
            .state
            .target_expanded
    );
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
            .get::<bui_text::UiExternalClipboard>(owner)
            .is_some()
    );
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "!".into(),
    }];
    bui_text::process_text_events(app.world_mut());
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

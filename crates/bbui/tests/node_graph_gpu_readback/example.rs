use super::*;

#[test]
fn node_graph_example_composition_renders_dark_and_light_review_captures() {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 820;

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    force_fallback_adapter: true,
                    ..Default::default()
                })),
                ..Default::default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(BbuiPlugin)
    .init_resource::<CapturedFrames>();
    app.finish();
    app.cleanup();

    let mut target_image =
        Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    target_image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(target_image);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();
    let background = app.world().resource::<Theme>().background;
    let surface = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 8.0,
                ..Default::default()
            },
            UiLayoutRoot::new(
                camera,
                Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
                1.0,
            ),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: WIDTH as f32,
                    height: HEIGHT as f32,
                },
                radius: 0.0,
                color: background,
            }),
        ))
        .id();
    WidgetTree::new(WidgetNode::column([
        WidgetNode::label("NodeGraph · typed sockets · stable links"),
        WidgetNode::node_graph("Material graph", example_graph()).with_key("material-graph"),
        WidgetNode::label(
            "Drag headers to move · drag sockets to connect or reconnect · Alt-drag cuts links",
        ),
        WidgetNode::label(
            "Middle-drag pans · Ctrl-wheel zooms · Ctrl+A selects nodes · C collapses",
        ),
        WidgetNode::label(
            "N adds a node · arrows move selection · Delete removes · Ctrl+Z/Y undo/redo",
        ),
    ]))
    .spawn(app.world_mut(), Some(surface))
    .expect("example composition should mount");
    let graph_entity = app
        .world()
        .iter_entities()
        .find(|entity| {
            entity
                .get::<UiKey>()
                .is_some_and(|key| key.0 == "material-graph")
        })
        .expect("example graph should be keyed")
        .id();
    {
        let mut layout = app
            .world_mut()
            .get_mut::<UiLayoutNode>(graph_entity)
            .unwrap();
        layout.size.width = UiLength::Px(1180.0);
        layout.size.height = UiLength::Px(600.0);
        layout.clip_contents = true;
    }

    for _ in 0..5 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let graph_rect = app
        .world()
        .get::<UiLayoutResult>(graph_entity)
        .expect("example graph should receive layout")
        .rect;
    let expected_outer_labels = [
        "NodeGraph · typed sockets · stable links",
        "Drag headers to move · drag sockets to connect or reconnect · Alt-drag cuts links",
        "Middle-drag pans · Ctrl-wheel zooms · Ctrl+A selects nodes · C collapses",
        "N adds a node · arrows move selection · Delete removes · Ctrl+Z/Y undo/redo",
    ];
    let label_layouts = {
        let world = app.world_mut();
        let mut label_query = world.query::<(&Label, &UiLayoutResult)>();
        label_query
            .iter(world)
            .map(|(label, layout)| (label.text.clone(), *layout))
            .collect::<HashMap<_, _>>()
    };
    for label in expected_outer_labels {
        assert!(
            label_layouts.contains_key(label),
            "example label has no layout result: {label}"
        );
    }
    let (nodes, sockets, links) = {
        let world = app.world_mut();
        let mut node_query = world.query::<&NodeGraphNodeControl>();
        let nodes: Vec<_> = node_query
            .iter(world)
            .filter(|control| control.owner == graph_entity)
            .cloned()
            .collect();
        let mut socket_query = world.query::<&NodeGraphSocketControl>();
        let sockets: Vec<_> = socket_query
            .iter(world)
            .filter(|control| control.owner == graph_entity)
            .cloned()
            .collect();
        let mut link_query = world.query::<&NodeGraphLinkControl>();
        let links: Vec<_> = link_query
            .iter(world)
            .filter(|control| control.owner == graph_entity)
            .cloned()
            .collect();
        (nodes, sockets, links)
    };
    assert_eq!(nodes.len(), 4);
    assert_eq!(sockets.len(), 8);
    assert_eq!(links.len(), 5);
    let (node_bounds, socket_bounds, graph) = {
        let world = app.world_mut();
        let mut node_query = world.query::<(&NodeGraphNodeControl, &UiHitBox)>();
        let node_bounds: Vec<_> = node_query
            .iter(world)
            .filter(|(control, _)| control.owner == graph_entity)
            .map(|(control, hitbox)| {
                (
                    control.clone(),
                    Rect::new(
                        hitbox.rect.x,
                        hitbox.rect.y,
                        hitbox.rect.width,
                        hitbox.rect.height,
                    ),
                )
            })
            .collect();
        let mut socket_query = world.query::<(&NodeGraphSocketControl, &UiHitBox)>();
        let socket_bounds: Vec<_> = socket_query
            .iter(world)
            .filter(|(control, _)| control.owner == graph_entity)
            .map(|(control, hitbox)| {
                (
                    control.clone(),
                    Rect::new(
                        hitbox.rect.x,
                        hitbox.rect.y,
                        hitbox.rect.width,
                        hitbox.rect.height,
                    ),
                )
            })
            .collect();
        (
            node_bounds,
            socket_bounds,
            world.get::<NodeGraph>(graph_entity).unwrap().clone(),
        )
    };
    assert_eq!(node_bounds.len(), nodes.len());
    assert_eq!(socket_bounds.len(), sockets.len());

    for (scenario, kind) in [ThemeKind::Dark, ThemeKind::Light].into_iter().enumerate() {
        let theme = Theme::new(kind);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        app.world_mut()
            .entity_mut(surface)
            .insert(UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: WIDTH as f32,
                    height: HEIGHT as f32,
                },
                radius: 0.0,
                color: theme.background,
            }));
        for _ in 0..3 {
            app.update();
        }
        let chrome = app
            .world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&graph_entity)
            .expect("example graph should publish a chrome fragment after theme repaint");
        assert!(
            chrome.items.len() > 2,
            "example graph chrome has too few draw items: {}",
            chrome.items.len()
        );
        let header_rect = ClipRect {
            x: graph_rect.x,
            y: graph_rect.y,
            width: graph_rect.width,
            height: theme
                .spacing
                .node_graph
                .header_height
                .min(graph_rect.height),
        };
        assert!(
            chrome.items.iter().any(|item| matches!(
                &item.primitive,
                Primitive::RoundedRect { rect, .. } if *rect == header_rect
            )),
            "example graph root fragment is missing its header surface"
        );
        let readback = app
            .world_mut()
            .spawn(Readback::texture(target.clone()))
            .observe(
                move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                      mut frames: ResMut<CapturedFrames>| {
                    frames.0.insert(10 + scenario, captured.data.clone());
                },
            )
            .id();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline
            && !app
                .world()
                .resource::<CapturedFrames>()
                .0
                .contains_key(&(10 + scenario))
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let pixels = app
            .world()
            .resource::<CapturedFrames>()
            .0
            .get(&(10 + scenario))
            .cloned()
            .expect("example NodeGraph GPU readback did not complete");
        app.world_mut().entity_mut(readback).despawn();
        if let Some(capture_dir) = std::env::var_os("BBUI_NODE_GRAPH_CAPTURE_DIR") {
            let capture_dir = std::path::PathBuf::from(capture_dir);
            fs::create_dir_all(&capture_dir).expect("create NodeGraph capture directory");
            let theme_name = match kind {
                ThemeKind::Dark => "dark",
                ThemeKind::Light => "light",
            };
            write_capture(
                &capture_dir.join(format!("node_graph_example_{theme_name}.png")),
                &pixels,
                WIDTH,
                HEIGHT,
            );
        }

        assert_color(
            pixel_at_width(
                &pixels,
                WIDTH,
                graph_rect.x as usize + 4,
                graph_rect.y as usize + 4,
            ),
            theme.color(ThemeColorToken::NodeGraphHeaderSurface),
            8,
            "example graph header surface",
        );
        assert_color(
            pixel_at_width(
                &pixels,
                WIDTH,
                graph_rect.x as usize + 4,
                (graph_rect.y + theme.spacing.node_graph.header_height + 5.0) as usize,
            ),
            theme.color(ThemeColorToken::NodeGraphSurface),
            8,
            "example canvas background",
        );
        assert_solid_region(
            &pixels,
            WIDTH,
            Rect::new(graph_rect.x + 260.0, graph_rect.y + 246.0, 8.0, 2.0),
            theme.color(ThemeColorToken::NodeGraphSurface),
            5,
            "empty canvas stripe has no disconnected link strokes",
        );
        let label_ink_regions = [
            (expected_outer_labels[0], 175.0, 90.0),
            (expected_outer_labels[1], 400.0, 150.0),
            (expected_outer_labels[2], 340.0, 150.0),
            (expected_outer_labels[3], 340.0, 150.0),
        ];
        for (label, right_offset, right_width) in label_ink_regions {
            let layout = label_layouts.get(label).unwrap();
            assert!(
                layout.rect.width >= right_offset + right_width,
                "example label was laid out too narrowly for its text: {label:?}, rect={:?}",
                layout.rect
            );
            assert_region_has_ink(
                &pixels,
                WIDTH,
                layout.rect,
                theme.background,
                8,
                &format!("example label {label:?}"),
            );
            assert_region_has_ink(
                &pixels,
                WIDTH,
                Rect::new(
                    layout.rect.x + right_offset,
                    layout.rect.y,
                    right_width,
                    layout.rect.height,
                ),
                theme.background,
                8,
                &format!("right side of example label {label:?}"),
            );
        }
        assert_region_has_ink(
            &pixels,
            WIDTH,
            Rect::new(
                graph_rect.x + 8.0,
                graph_rect.y + 2.0,
                280.0,
                theme.spacing.node_graph.header_height - 4.0,
            ),
            theme.color(ThemeColorToken::NodeGraphHeaderSurface),
            8,
            "graph title in the left header region",
        );
        assert_region_has_ink(
            &pixels,
            WIDTH,
            Rect::new(
                graph_rect.x + graph_rect.width - 270.0,
                graph_rect.y + 2.0,
                262.0,
                theme.spacing.node_graph.header_height - 4.0,
            ),
            theme.color(ThemeColorToken::NodeGraphHeaderSurface),
            8,
            "node/link summary in the right header region",
        );
        assert_solid_region(
            &pixels,
            WIDTH,
            Rect::new(
                graph_rect.x + 300.0,
                graph_rect.y + 2.0,
                graph_rect.width - 600.0,
                theme.spacing.node_graph.header_height - 4.0,
            ),
            theme.color(ThemeColorToken::NodeGraphHeaderSurface),
            8,
            "clear gap between graph title and node/link summary",
        );

        for (_, rect) in &node_bounds {
            assert_text_stays_inside_node_header(
                &pixels,
                WIDTH,
                HEIGHT,
                *rect,
                theme.spacing.node_graph.header_height,
                theme.text,
            );
            let body_patch = Rect::new(
                rect.x + rect.width * 0.5 - 8.0,
                rect.y
                    + theme.spacing.node_graph.header_height
                    + theme.spacing.node_graph.socket_spacing * 0.25,
                16.0,
                8.0,
            );
            assert_solid_region(
                &pixels,
                WIDTH,
                body_patch,
                theme.surface,
                8,
                "node body is opaque over the background grid",
            );
        }

        let socket_positions: HashMap<_, _> = socket_bounds
            .iter()
            .map(|(socket, rect)| {
                (
                    (socket.node_id, socket.socket_index),
                    (
                        [rect.x + rect.width * 0.5, rect.y + rect.height * 0.5],
                        socket.socket_type,
                    ),
                )
            })
            .collect();
        for (socket, rect) in &socket_bounds {
            assert_color_near_position_sized(
                &pixels,
                [WIDTH, HEIGHT],
                [rect.x + rect.width * 0.5, rect.y + rect.height * 0.5],
                socket_color(&theme, socket.socket_type),
                6,
                20,
                "typed socket circle",
            );
            let node_rect = node_bounds
                .iter()
                .find(|(node, _)| node.node_id == socket.node_id)
                .map(|(_, rect)| *rect)
                .expect("socket owner should have a node rectangle");
            let center_x = rect.x + rect.width * 0.5;
            let center_y = rect.y + rect.height * 0.5;
            let middle_x = node_rect.x + node_rect.width * 0.5;
            let label_region = match socket.direction {
                bbui::widgets::SocketDirection::Input => Rect::new(
                    center_x + 6.0,
                    center_y - 9.0,
                    (middle_x - center_x - 8.0).max(1.0),
                    18.0,
                ),
                bbui::widgets::SocketDirection::Output => Rect::new(
                    middle_x + 8.0,
                    center_y - 9.0,
                    (center_x - middle_x - 14.0).max(1.0),
                    18.0,
                ),
            };
            assert_region_has_color(
                &pixels,
                WIDTH,
                label_region,
                theme.text_muted,
                28,
                &format!(
                    "{} label belongs beside its {:?} socket",
                    socket.label, socket.direction
                ),
            );
        }
        for link in &graph.links {
            let (from, socket_type) = socket_positions
                .get(&(link.from_node, link.from_socket))
                .expect("link source should have a visible socket");
            let (to, _) = socket_positions
                .get(&(link.to_node, link.to_socket))
                .expect("link destination should have a visible socket");
            assert_color_near_position_sized(
                &pixels,
                [WIDTH, HEIGHT],
                [(from[0] + to[0]) * 0.5, (from[1] + to[1]) * 0.5],
                socket_color(&theme, *socket_type),
                14,
                24,
                "link color follows its socket type",
            );
        }
        println!("NodeGraph example readback passed: {kind:?}, bounds={graph_rect:?}");
    }
}

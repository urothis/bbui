use super::*;

#[test]
fn node_graph_renders_nodes_typed_sockets_links_selection_and_collapse() {
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

    let mut target = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();
    let (graph, source_id, output_id) = sample_graph();
    let root = WidgetTree::new(WidgetNode::node_graph("Material graph", graph).with_key("graph"))
        .spawn(app.world_mut(), None)
        .expect("test graph should mount");
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(root).unwrap();
        layout.size.width = UiLength::Px(GRAPH_RECT.width);
        layout.size.height = UiLength::Px(GRAPH_RECT.height);
        layout.clip_contents = true;
    }
    app.world_mut()
        .entity_mut(root)
        .insert(UiLayoutRoot::new(camera, GRAPH_RECT, 1.0));

    for _ in 0..4 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let (nodes, sockets, links) = {
        let world = app.world_mut();
        let mut node_query = world.query::<&NodeGraphNodeControl>();
        let nodes: Vec<_> = node_query
            .iter(world)
            .filter(|control| control.owner == root)
            .cloned()
            .collect();
        let mut socket_query = world.query::<&NodeGraphSocketControl>();
        let sockets: Vec<_> = socket_query
            .iter(world)
            .filter(|control| control.owner == root)
            .cloned()
            .collect();
        let mut link_query = world.query::<&NodeGraphLinkControl>();
        let links: Vec<_> = link_query
            .iter(world)
            .filter(|control| control.owner == root)
            .cloned()
            .collect();
        (nodes, sockets, links)
    };
    assert_eq!(nodes.len(), 2);
    assert!(
        nodes
            .iter()
            .any(|node| node.node_id == source_id && node.collapsed)
    );
    assert!(
        nodes
            .iter()
            .any(|node| node.node_id == output_id && node.selected)
    );
    assert_eq!(sockets.len(), 3);
    assert!(
        sockets.iter().any(|socket| {
            socket.node_id == source_id && socket.socket_type == SocketType::Float
        })
    );
    assert!(
        sockets.iter().any(|socket| {
            socket.node_id == output_id && socket.socket_type == SocketType::Color
        })
    );
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].from_node, source_id);
    assert_eq!(links[0].to_node, output_id);
    let selected_node_rect = {
        let world = app.world_mut();
        let mut query = world.query::<(&NodeGraphNodeControl, &UiHitBox)>();
        query
            .iter(world)
            .find(|(control, _)| control.owner == root && control.node_id == output_id)
            .map(|(_, hitbox)| hitbox.rect)
            .expect("selected output should have hit-test bounds")
    };

    let mut scenario = 0;
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        for dpi in [1.0, 2.0] {
            let theme = Theme::new(kind);
            *app.world_mut().resource_mut::<Theme>() = theme.clone();
            app.world_mut()
                .get_mut::<UiLayoutRoot>(root)
                .unwrap()
                .dpi_scale = dpi;
            for _ in 0..3 {
                app.update();
            }
            let chrome = app
                .world()
                .resource::<DisplayList>()
                .widget_fragments()
                .get(&root)
                .expect("graph root should publish a chrome fragment after theme/layout repaint");
            assert!(
                chrome.items.len() > 2,
                "graph chrome has too few draw items: {}",
                chrome.items.len()
            );
            let header_rect = ClipRect {
                x: GRAPH_RECT.x,
                y: GRAPH_RECT.y,
                width: GRAPH_RECT.width,
                height: theme
                    .spacing
                    .node_graph
                    .header_height
                    .min(GRAPH_RECT.height),
            };
            assert!(
                chrome.items.iter().any(|item| matches!(
                    &item.primitive,
                    Primitive::RoundedRect { rect, .. } if *rect == header_rect
                )),
                "graph root fragment is missing its header surface"
            );
            let readback = app
                .world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(
                    move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                          mut frames: ResMut<CapturedFrames>| {
                        frames.0.insert(scenario, captured.data.clone());
                    },
                )
                .id();
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline
                && !app
                    .world()
                    .resource::<CapturedFrames>()
                    .0
                    .contains_key(&scenario)
            {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let pixels = app
                .world()
                .resource::<CapturedFrames>()
                .0
                .get(&scenario)
                .cloned()
                .expect("NodeGraph GPU readback did not complete");
            app.world_mut().entity_mut(readback).despawn();

            if let Some(capture_dir) = std::env::var_os("BBUI_NODE_GRAPH_CAPTURE_DIR") {
                let capture_dir = std::path::PathBuf::from(capture_dir);
                fs::create_dir_all(&capture_dir).expect("create NodeGraph capture directory");
                let theme_name = match kind {
                    ThemeKind::Dark => "dark",
                    ThemeKind::Light => "light",
                };
                write_capture(
                    &capture_dir.join(format!("node_graph_{theme_name}_dpi{}.png", dpi as u8)),
                    &pixels,
                    TARGET_WIDTH,
                    TARGET_HEIGHT,
                );
            }

            assert_color(
                pixel_at(&pixels, 16 + 180, 16 + 54),
                theme
                    .color(ThemeColorToken::NodeGraphHeaderSurface)
                    .mix(theme.color(ThemeColorToken::NodeGraphTextSocket), 0.5),
                6,
                "collapsed source node header",
            );
            assert_color(
                pixel_at(&pixels, 16 + 450, 16 + 122),
                theme
                    .color(ThemeColorToken::NodeGraphHeaderSurface)
                    .mix(theme.color(ThemeColorToken::NodeGraphColorSocket), 0.5),
                7,
                "selected output node header",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_node_rect.x + selected_node_rect.width * 0.5).round() as usize,
                    (selected_node_rect.y
                        + theme.spacing.node_graph.header_height
                        + theme.spacing.node_graph.socket_spacing * 0.5)
                        .round() as usize,
                ),
                theme.surface.mix(theme.accent, 0.08),
                7,
                "selected output node body",
            );
            assert_color_near_position(
                &pixels,
                [
                    selected_node_rect.x + 1.0,
                    selected_node_rect.y + selected_node_rect.height * 0.75,
                ],
                theme.accent,
                2,
                16,
                "selected node outline",
            );
            assert_solid_region(
                &pixels,
                TARGET_WIDTH,
                Rect::new(
                    GRAPH_RECT.x + 140.0,
                    GRAPH_RECT.y + 2.0,
                    100.0,
                    theme.spacing.node_graph.header_height - 4.0,
                ),
                theme.color(ThemeColorToken::NodeGraphHeaderSurface),
                8,
                "clear space between compact graph header title and summary",
            );
            assert_color_near_position(
                &pixels,
                [16.0 + 202.0, 16.0 + 62.0],
                theme.color(ThemeColorToken::NodeGraphFloatSocket),
                2,
                8,
                "collapsed Float output socket",
            );
            assert_color_near_position(
                &pixels,
                [16.0 + 282.0, 16.0 + 157.0],
                theme.color(ThemeColorToken::NodeGraphFloatSocket),
                2,
                8,
                "Float input socket",
            );
            assert_color_near_position(
                &pixels,
                [16.0 + 462.0, 16.0 + 157.0],
                theme.color(ThemeColorToken::NodeGraphColorSocket),
                2,
                8,
                "Color output socket",
            );
            assert_color_near_position(
                &pixels,
                [16.0 + 242.0, 16.0 + 109.5],
                theme.color(ThemeColorToken::NodeGraphFloatSocket),
                4,
                24,
                "typed link curve",
            );
            assert_color(
                pixel_at(&pixels, 16 + 460, 16 + 260),
                theme.color(ThemeColorToken::NodeGraphSurface),
                6,
                "graph viewport background",
            );
            println!("NodeGraph scene readback passed: {kind:?}, dpi={dpi}");
            scenario += 1;
        }
    }
}

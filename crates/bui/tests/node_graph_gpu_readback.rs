//! Pixel coverage for typed NodeGraph nodes, sockets, links, selection, and collapse.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

use bevy::{
    app::PluginGroup,
    asset::Assets,
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, ResMut, Resource, WindowPlugin},
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use bui::{
    BuiPlugin,
    widgets::{
        NodeGraph, NodeGraphLinkControl, NodeGraphNodeControl, NodeGraphSocketControl, NodeSocket,
        SocketType, WidgetNode, WidgetTree,
    },
};
use bui_layout::{Rect, UiLayoutNode, UiLayoutRoot, UiLength};
use bui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};

const TARGET_WIDTH: u32 = 512;
const TARGET_HEIGHT: u32 = 320;
const GRAPH_RECT: Rect = Rect::new(16.0, 16.0, 480.0, 280.0);
#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, Vec<u8>>);

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn rgba_bytes(color: Rgba) -> [u8; 4] {
    [color.0, color.1, color.2, color.3]
        .map(|channel| (channel * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn assert_color(pixel: &[u8], color: Rgba, tolerance: u8, what: &str) {
    for (actual, expected) in pixel.iter().zip(rgba_bytes(color)) {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "{what}: pixel={pixel:?}, expected={expected:?} ±{tolerance}"
        );
    }
}

fn assert_color_near_position(
    pixels: &[u8],
    position: [f32; 2],
    color: Rgba,
    radius: usize,
    tolerance: u8,
    what: &str,
) {
    let expected = rgba_bytes(color);
    let center_x = position[0].round() as usize;
    let center_y = position[1].round() as usize;
    let x_start = center_x.saturating_sub(radius);
    let x_end = (center_x + radius).min(TARGET_WIDTH as usize - 1);
    let y_start = center_y.saturating_sub(radius);
    let y_end = (center_y + radius).min(TARGET_HEIGHT as usize - 1);
    let found = (y_start..=y_end).any(|y| {
        (x_start..=x_end).any(|x| {
            pixel_at(pixels, x, y)
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
        })
    });
    let neighborhood: Vec<_> = (y_start..=y_end)
        .step_by(2)
        .flat_map(|y| {
            (x_start..=x_end).step_by(2).map(move |x| {
                let pixel = pixel_at(pixels, x, y);
                [pixel[0], pixel[1], pixel[2], pixel[3]]
            })
        })
        .collect();
    assert!(
        found,
        "{what} not visible near {position:?} with color {expected:?} ±{tolerance}; center={:?}, neighborhood={neighborhood:?}",
        pixel_at(pixels, center_x, center_y)
    );
}

fn sample_graph() -> (NodeGraph, bui::widgets::NodeId, bui::widgets::NodeId) {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Scalar Input",
        [22.0, 20.0],
        vec![NodeSocket::output("Value", SocketType::Float)],
    );
    graph.nodes[0].collapsed = true;
    let output = graph.add_node(
        "Color Output",
        [282.0, 90.0],
        vec![
            NodeSocket::input("Value", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
    );
    graph.selected.insert(output);
    graph
        .create_link(source, 0, output, 0)
        .expect("Float sockets should connect");
    (graph, source, output)
}

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
    .add_plugins(BuiPlugin)
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

            assert_color(
                pixel_at(&pixels, 16 + 180, 16 + 54),
                theme.color(ThemeColorToken::NodeGraphHeaderSurface),
                6,
                "collapsed source node header",
            );
            assert_color(
                pixel_at(&pixels, 16 + 450, 16 + 122),
                theme.accent,
                7,
                "selected output node header",
            );
            assert_color(
                pixel_at(&pixels, 16 + 340, 16 + 210),
                theme.color(ThemeColorToken::NodeGraphSelectedNodeSurface),
                7,
                "selected output node body",
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
                theme.surface,
                6,
                "graph viewport background",
            );
            println!("NodeGraph scene readback passed: {kind:?}, dpi={dpi}");
            scenario += 1;
        }
    }
}

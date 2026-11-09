//! Pixel coverage for typed NodeGraph nodes, sockets, links, selection, and collapse.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    fs,
    path::Path,
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
        Label, NodeCategory, NodeGraph, NodeGraphLinkControl, NodeGraphNodeControl,
        NodeGraphSocketControl, NodeSocket, SocketType, WidgetNode, WidgetTree,
    },
};
use bui_core::{UiHitBox, UiKey, UiNode};
use bui_layout::{Rect, UiEdges, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bui_render::{ClipRect, DisplayList, Primitive, UiPaint};
use bui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};
use image::ImageEncoder;

const TARGET_WIDTH: u32 = 512;
const TARGET_HEIGHT: u32 = 320;
const GRAPH_RECT: Rect = Rect::new(16.0, 16.0, 480.0, 280.0);
#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, Vec<u8>>);

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    pixel_at_width(pixels, TARGET_WIDTH, x, y)
}

fn pixel_at_width(pixels: &[u8], width: u32, x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (width as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn write_capture(path: &Path, pixels: &[u8], width: u32, height: u32) {
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height as usize {
        for x in 0..width as usize {
            rgb.extend_from_slice(&pixel_at_width(pixels, width, x, y)[..3]);
        }
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)
        .expect("encode NodeGraph GPU capture");
    fs::write(path, png).expect("write NodeGraph GPU capture");
}

fn rgba_bytes(color: Rgba) -> [u8; 4] {
    [color.0, color.1, color.2, color.3]
        .map(|channel| (channel * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn assert_color(pixel: &[u8], color: Rgba, tolerance: u8, what: &str) {
    assert!(
        pixel_matches(pixel, color, tolerance),
        "{what}: pixel={pixel:?}, expected={:?} ±{tolerance}",
        rgba_bytes(color)
    );
}

fn pixel_matches(pixel: &[u8], color: Rgba, tolerance: u8) -> bool {
    pixel
        .iter()
        .zip(rgba_bytes(color))
        .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
}

fn assert_solid_region(
    pixels: &[u8],
    width: u32,
    rect: Rect,
    color: Rgba,
    tolerance: u8,
    what: &str,
) {
    let x_start = rect.x.floor().max(0.0) as usize;
    let x_end = (rect.x + rect.width).ceil().max(0.0) as usize;
    let y_start = rect.y.floor().max(0.0) as usize;
    let y_end = (rect.y + rect.height).ceil().max(0.0) as usize;
    for y in y_start..y_end {
        for x in x_start..x_end {
            assert!(
                pixel_matches(pixel_at_width(pixels, width, x, y), color, tolerance),
                "{what}: unexpected ink at ({x}, {y})"
            );
        }
    }
}

fn assert_region_has_ink(
    pixels: &[u8],
    width: u32,
    rect: Rect,
    background: Rgba,
    tolerance: u8,
    what: &str,
) {
    let x_start = rect.x.floor().max(0.0) as usize;
    let x_end = (rect.x + rect.width).ceil().max(0.0) as usize;
    let y_start = rect.y.floor().max(0.0) as usize;
    let y_end = (rect.y + rect.height).ceil().max(0.0) as usize;
    let found = (y_start..y_end).any(|y| {
        (x_start..x_end)
            .any(|x| !pixel_matches(pixel_at_width(pixels, width, x, y), background, tolerance))
    });
    assert!(found, "{what}: no visible pixels in {rect:?}");
}

fn assert_region_has_color(
    pixels: &[u8],
    width: u32,
    rect: Rect,
    color: Rgba,
    tolerance: u8,
    what: &str,
) {
    let x_start = rect.x.floor().max(0.0) as usize;
    let x_end = (rect.x + rect.width).ceil().max(0.0) as usize;
    let y_start = rect.y.floor().max(0.0) as usize;
    let y_end = (rect.y + rect.height).ceil().max(0.0) as usize;
    let found = (y_start..y_end).any(|y| {
        (x_start..x_end)
            .any(|x| pixel_matches(pixel_at_width(pixels, width, x, y), color, tolerance))
    });
    assert!(found, "{what}: no text-colored pixels in {rect:?}");
}

fn assert_text_stays_inside_node_header(
    pixels: &[u8],
    width: u32,
    height: u32,
    node_rect: Rect,
    header_height: f32,
    text: Rgba,
) {
    let top = (node_rect.y + 2.0).floor().max(0.0) as usize;
    let bottom = (node_rect.y + header_height - 2.0).ceil().max(0.0) as usize;
    let scan_left = (node_rect.x - 32.0).floor().max(0.0) as usize;
    let scan_right = (node_rect.x + node_rect.width + 32.0)
        .ceil()
        .max(0.0)
        .min(width as f32) as usize;
    let inside_left = node_rect.x.floor().max(0.0) as usize;
    let inside_right = (node_rect.x + node_rect.width)
        .ceil()
        .max(0.0)
        .min(width as f32) as usize;
    for y in top..bottom.min(height as usize) {
        for x in scan_left..scan_right {
            if x < inside_left || x >= inside_right {
                assert!(
                    !pixel_matches(pixel_at_width(pixels, width, x, y), text, 16),
                    "node header text escaped its node bounds near ({x}, {y})"
                );
            }
        }
    }
}

fn socket_color(theme: &Theme, socket_type: SocketType) -> Rgba {
    match socket_type {
        SocketType::Float => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        SocketType::Color => theme.color(ThemeColorToken::NodeGraphColorSocket),
        SocketType::Boolean => theme.color(ThemeColorToken::NodeGraphBooleanSocket),
        SocketType::Vector2 | SocketType::Vector3 => {
            theme.color(ThemeColorToken::NodeGraphVectorSocket)
        }
        SocketType::Text => theme.color(ThemeColorToken::NodeGraphTextSocket),
        SocketType::Any => theme.text_muted,
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
    assert_color_near_position_sized(
        pixels,
        [TARGET_WIDTH, TARGET_HEIGHT],
        position,
        color,
        radius,
        tolerance,
        what,
    );
}

fn assert_color_near_position_sized(
    pixels: &[u8],
    size: [u32; 2],
    position: [f32; 2],
    color: Rgba,
    radius: usize,
    tolerance: u8,
    what: &str,
) {
    let [width, height] = size;
    let expected = rgba_bytes(color);
    let center_x = position[0].round() as usize;
    let center_y = position[1].round() as usize;
    let x_start = center_x.saturating_sub(radius);
    let x_end = (center_x + radius).min(width as usize - 1);
    let y_start = center_y.saturating_sub(radius);
    let y_end = (center_y + radius).min(height as usize - 1);
    let found = (y_start..=y_end).any(|y| {
        (x_start..=x_end).any(|x| {
            pixel_at_width(pixels, width, x, y)
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
        })
    });
    let neighborhood: Vec<_> = (y_start..=y_end)
        .step_by(2)
        .flat_map(|y| {
            (x_start..=x_end).step_by(2).map(move |x| {
                let pixel = pixel_at_width(pixels, width, x, y);
                [pixel[0], pixel[1], pixel[2], pixel[3]]
            })
        })
        .collect();
    assert!(
        found,
        "{what} not visible near {position:?} with color {expected:?} ±{tolerance}; center={:?}, neighborhood={neighborhood:?}",
        pixel_at_width(pixels, width, center_x, center_y)
    );
}

fn sample_graph() -> (NodeGraph, bui::widgets::NodeId, bui::widgets::NodeId) {
    let mut graph = NodeGraph::default();
    let source = graph.add_node_with_category(
        "Scalar Input",
        [22.0, 20.0],
        vec![NodeSocket::output("Value", SocketType::Float)],
        NodeCategory::Input,
    );
    graph.nodes[0].collapsed = true;
    let output = graph.add_node_with_category(
        "Color Output",
        [282.0, 90.0],
        vec![
            NodeSocket::input("Value", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Output,
    );
    graph.selected.insert(output);
    graph
        .create_link(source, 0, output, 0)
        .expect("Float sockets should connect");
    (graph, source, output)
}

fn example_graph() -> NodeGraph {
    let mut graph = NodeGraph::default();
    let noise = graph.add_node_with_category(
        "Noise Texture",
        [30.0, 45.0],
        vec![NodeSocket::output("Factor", SocketType::Float)],
        NodeCategory::Texture,
    );
    let color_ramp = graph.add_node_with_category(
        "Color Ramp",
        [30.0, 260.0],
        vec![
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Color,
    );
    let mix = graph.add_node_with_category(
        "Mix",
        [400.0, 150.0],
        vec![
            NodeSocket::input("A", SocketType::Color),
            NodeSocket::input("B", SocketType::Color),
            NodeSocket::input("Factor", SocketType::Float),
            NodeSocket::output("Color", SocketType::Color),
        ],
        NodeCategory::Converter,
    );
    let output = graph.add_node_with_category(
        "Material Output",
        [810.0, 150.0],
        vec![NodeSocket::input("Surface", SocketType::Color)],
        NodeCategory::Output,
    );
    graph.create_link(noise, 0, color_ramp, 0).unwrap();
    graph.create_link(color_ramp, 1, mix, 0).unwrap();
    graph.create_link(color_ramp, 1, mix, 1).unwrap();
    graph.create_link(noise, 0, mix, 2).unwrap();
    graph.create_link(mix, 3, output, 0).unwrap();
    graph.pan = [80.0, 105.0];
    graph
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

            if let Some(capture_dir) = std::env::var_os("BUI_NODE_GRAPH_CAPTURE_DIR") {
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
    .add_plugins(BuiPlugin)
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
        if let Some(capture_dir) = std::env::var_os("BUI_NODE_GRAPH_CAPTURE_DIR") {
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
                bui::widgets::SocketDirection::Input => Rect::new(
                    center_x + 6.0,
                    center_y - 9.0,
                    (middle_x - center_x - 8.0).max(1.0),
                    18.0,
                ),
                bui::widgets::SocketDirection::Output => Rect::new(
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

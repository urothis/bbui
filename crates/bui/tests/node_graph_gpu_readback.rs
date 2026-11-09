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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[path = "node_graph_gpu_readback/example.rs"]
mod example;
#[path = "node_graph_gpu_readback/rendering.rs"]
mod rendering;

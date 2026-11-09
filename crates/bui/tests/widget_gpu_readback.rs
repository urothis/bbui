//! Pixel coverage for real menu-overlay, HSV-editor, and curve-editor widget painters.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    fs, thread,
    time::{Duration, Instant},
};

use bevy::{
    app::PluginGroup,
    asset::Assets,
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, Entity, ResMut, Resource, WindowPlugin},
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    ui::IsDefaultUiCamera,
    window::ExitCondition,
    winit::WinitPlugin,
};
use bui::{
    BuiPlugin,
    widgets::{MenuEntry, WidgetNode, WidgetTree},
};
use bui_core::UiChildren;
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bui_render::{ClipRect, DisplayList, Primitive, UiPaint};
use bui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};
use bui_widgets::{
    ColorRamp, CurveEditor, DataTable, Dropdown, DropdownOptionControl, DropdownPopupState,
    Interpolation, ListItem, ListModel, MenuControl, PreviewGrid, PreviewGridItem, TableColumn,
    TableRow, TreeModel, TreeNode,
    declarative::{
        CollectionItemControl, CollectionItemKind, DataTableCellControl, DataTableHeaderControl,
    },
};

const TARGET_WIDTH: u32 = 1200;
const TARGET_HEIGHT: u32 = 1200;

#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, CapturedFrame>);

#[derive(Clone)]
struct CapturedFrame {
    bytes: Vec<u8>,
    completed_at: Instant,
}

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

#[cfg(test)]
fn write_capture_ppm(path: &std::path::Path, pixels: &[u8]) {
    let mut ppm = format!("P6\n{TARGET_WIDTH} {TARGET_HEIGHT}\n255\n").into_bytes();
    ppm.reserve(TARGET_WIDTH as usize * TARGET_HEIGHT as usize * 3);
    for y in 0..TARGET_HEIGHT as usize {
        for x in 0..TARGET_WIDTH as usize {
            ppm.extend_from_slice(&pixel_at(pixels, x, y)[..3]);
        }
    }
    fs::write(path, ppm).expect("write optional widget visual capture");
}

#[cfg(test)]
fn capture_frame(
    app: &mut App,
    target: &bevy::prelude::Handle<Image>,
    frame: usize,
) -> (Vec<u8>, Duration) {
    let started = Instant::now();
    let readback = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(
            move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                  mut frames: ResMut<CapturedFrames>| {
                frames.0.insert(
                    frame,
                    CapturedFrame {
                        bytes: captured.data.clone(),
                        completed_at: Instant::now(),
                    },
                );
            },
        )
        .id();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline
        && !app
            .world()
            .resource::<CapturedFrames>()
            .0
            .contains_key(&frame)
    {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let captured = app
        .world()
        .resource::<CapturedFrames>()
        .0
        .get(&frame)
        .cloned()
        .expect("widget GPU readback did not complete before the deadline");
    app.world_mut().entity_mut(readback).despawn();
    (
        captured.bytes,
        captured.completed_at.duration_since(started),
    )
}

fn assert_color(pixel: &[u8], color: Rgba, what: &str) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 4,
            "{what} color differed: actual={pixel:?}, expected={expected:?}"
        );
    }
}

fn assert_near(pixel: &[u8], expected: [u8; 4], tolerance: u8, what: &str) {
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "{what} color differed: actual={pixel:?}, expected={expected:?} ±{tolerance}"
        );
    }
}

fn assert_color_near_position(
    pixels: &[u8],
    position: [f32; 2],
    color: Rgba,
    tolerance: u8,
    radius: usize,
    what: &str,
) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
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
    assert!(
        found,
        "{what} was not visible near {position:?} with color {expected:?} ±{tolerance}"
    );
}

fn curve_plot(rect: Rect, theme: &Theme) -> Rect {
    let spacing = theme.spacing.editor;
    let left = spacing.gutter_width + spacing.padding_x;
    let vertical = theme.control_height + spacing.padding_y;
    Rect {
        x: rect.x + left,
        y: rect.y + vertical,
        width: (rect.width - left - spacing.padding_x).max(1.0),
        height: (rect.height - vertical - spacing.padding_y).max(1.0),
    }
}

fn ramp_track(rect: Rect) -> Rect {
    Rect {
        x: rect.x + 12.0,
        y: rect.y + 38.0,
        width: (rect.width - 24.0).max(1.0),
        height: 25.0,
    }
}

fn blend(foreground: Rgba, background: Rgba, amount: f32) -> Rgba {
    let amount = amount.clamp(0.0, 1.0);
    let keep = 1.0 - amount;
    Rgba(
        foreground.0 * keep + background.0 * amount,
        foreground.1 * keep + background.1 * amount,
        foreground.2 * keep + background.2 * amount,
        foreground.3 * keep + background.3 * amount,
    )
}

fn curve_screen_position(plot: Rect, position: [f32; 2]) -> [f32; 2] {
    [
        plot.x + position[0] * plot.width,
        plot.y + (1.0 - position[1]) * plot.height,
    ]
}

#[cfg(test)]
fn collection_item(
    world: &bevy::prelude::World,
    owner: Entity,
    id: u64,
) -> (Entity, Rect, CollectionItemControl) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<CollectionItemControl>(*entity)
                .is_some_and(|item| item.id == id)
        })
        .expect("collection item should be mounted in the widget tree");
    let control = world.get::<CollectionItemControl>(entity).unwrap().clone();
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    (entity, rect, control)
}

#[cfg(test)]
fn table_header(world: &bevy::prelude::World, owner: Entity, column: usize) -> (Entity, Rect) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<DataTableHeaderControl>(*entity)
                .is_some_and(|header| header.column == column)
        })
        .expect("virtualized table header should be visible");
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    (entity, rect)
}

#[cfg(test)]
fn table_cell(
    world: &bevy::prelude::World,
    owner: Entity,
    row_id: u64,
    column: usize,
) -> (Entity, Rect, DataTableCellControl) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<DataTableCellControl>(*entity)
                .is_some_and(|cell| cell.row_id == row_id && cell.column == column)
        })
        .expect("virtualized table cell should be visible");
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    let cell = world.get::<DataTableCellControl>(entity).unwrap().clone();
    (entity, rect, cell)
}

#[cfg(test)]
fn dropdown_option(
    world: &bevy::prelude::World,
    owner: Entity,
    index: usize,
) -> (Entity, Rect, DropdownOptionControl) {
    let children = world.get::<UiChildren>(owner).unwrap();
    let entity = children
        .0
        .iter()
        .copied()
        .find(|entity| {
            world
                .get::<DropdownOptionControl>(*entity)
                .is_some_and(|option| option.index == index)
        })
        .expect("visible dropdown option should be mounted");
    let rect = world
        .get::<bui_layout::UiLayoutResult>(entity)
        .unwrap()
        .rect;
    let option = world.get::<DropdownOptionControl>(entity).unwrap().clone();
    (entity, rect, option)
}

fn assert_table_header_text_visible(pixels: &[u8], rect: Rect, theme: &Theme) -> usize {
    let background = blend(theme.surface, theme.background, 0.12);
    let background = [
        (background.0 * 255.0).round() as u8,
        (background.1 * 255.0).round() as u8,
        (background.2 * 255.0).round() as u8,
    ];
    let text = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let start_x = rect.x.ceil() as usize + 3;
    let end_x = (rect.x + rect.width - 24.0).floor().max(rect.x) as usize;
    let start_y = rect.y.ceil() as usize + 2;
    let end_y = (rect.y + rect.height - 2.0).floor() as usize;
    let glyph_pixels = (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y);
            let from_background = (0..3)
                .map(|channel| actual[channel].abs_diff(background[channel]))
                .map(u16::from)
                .sum::<u16>();
            let from_text = (0..3)
                .map(|channel| actual[channel].abs_diff(text[channel]))
                .map(u16::from)
                .sum::<u16>();
            from_background >= 36 && from_text < from_background
        })
        .count();
    assert!(
        glyph_pixels >= 5,
        "expected sorted table header glyph pixels, found {glyph_pixels}"
    );
    glyph_pixels
}

fn assert_menu_label_visible(pixels: &[u8], rect: Rect, theme: &Theme) -> usize {
    let surface = [
        (theme.surface.0 * 255.0).round() as u8,
        (theme.surface.1 * 255.0).round() as u8,
        (theme.surface.2 * 255.0).round() as u8,
    ];
    let text = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let start_x = rect.x.ceil() as usize + 6;
    let end_x = (rect.x + 100.0).floor() as usize;
    let start_y = rect.y.ceil() as usize + 2;
    let end_y = (rect.y + rect.height - 2.0).floor() as usize;
    let glyph_pixels = (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y);
            let from_surface = (0..3)
                .map(|channel| actual[channel].abs_diff(surface[channel]))
                .map(u16::from)
                .sum::<u16>();
            let from_text = (0..3)
                .map(|channel| actual[channel].abs_diff(text[channel]))
                .map(u16::from)
                .sum::<u16>();
            from_surface >= 45 && from_text < from_surface
        })
        .count();
    assert!(
        glyph_pixels >= 5,
        "expected visible menu label glyph pixels, found {glyph_pixels}"
    );
    glyph_pixels
}

fn assert_option_label_visible(
    pixels: &[u8],
    rect: Rect,
    background: Rgba,
    theme: &Theme,
    label: &str,
) -> usize {
    let surface = [
        (background.0 * 255.0).round() as u8,
        (background.1 * 255.0).round() as u8,
        (background.2 * 255.0).round() as u8,
    ];
    let text = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let start_x = rect.x.ceil() as usize + 4;
    let end_x = (rect.x + 100.0).floor() as usize;
    let start_y = rect.y.ceil() as usize + 2;
    let end_y = (rect.y + rect.height - 2.0).floor() as usize;
    let glyph_pixels = (start_y..end_y)
        .flat_map(|y| (start_x..end_x).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y);
            let from_surface = (0..3)
                .map(|channel| actual[channel].abs_diff(surface[channel]))
                .map(u16::from)
                .sum::<u16>();
            let from_text = (0..3)
                .map(|channel| actual[channel].abs_diff(text[channel]))
                .map(u16::from)
                .sum::<u16>();
            from_surface >= 36 && from_text < from_surface
        })
        .count();
    assert!(
        glyph_pixels >= 5,
        "expected visible {label:?} dropdown label pixels, found {glyph_pixels}"
    );
    glyph_pixels
}

#[path = "widget_gpu_readback/scene.rs"]
mod scene;

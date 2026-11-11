//! Pixel checks for Canvas shape, image, selection, and clip rendering.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use bbui::{
    BbuiPlugin,
    widgets::{Canvas, CanvasShape, WidgetNode, WidgetTree},
};
use bbui_layout::{Rect, UiLayoutNode, UiLayoutRoot, UiLength};
use bbui_style::{Theme, ThemeColorToken, ThemeKind};
use bbui_widgets::canvas::{CanvasImageAssets, CanvasImageState};
use bevy::{
    app::PluginGroup,
    asset::{AssetPlugin, Assets},
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

const TARGET_WIDTH: u32 = 512;
const TARGET_HEIGHT: u32 = 320;
const CANVAS_RECT: Rect = Rect::new(16.0, 16.0, 460.0, 270.0);
const RECT_COLOR: [f32; 4] = [0.20, 0.45, 0.82, 1.0];
const ELLIPSE_COLOR: [f32; 4] = [0.86, 0.26, 0.12, 1.0];
const LINE_COLOR: [f32; 4] = [0.18, 0.78, 0.42, 1.0];
const CLIPPED_COLOR: [f32; 4] = [0.85, 0.15, 0.72, 1.0];

#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, Vec<u8>>);

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn assert_rgba_near(pixel: &[u8], expected: [f32; 4], tolerance: u8, what: &str) {
    for (actual, expected) in pixel
        .iter()
        .zip(expected.map(|channel| (channel * 255.0).round().clamp(0.0, 255.0) as u8))
    {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "{what}: pixel={pixel:?}, expected={expected:?} ±{tolerance}"
        );
    }
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn canvas_selected_fill(theme: &Theme) -> [f32; 4] {
    let overlay = theme.color(ThemeColorToken::CanvasSelectionOverlay);
    let alpha = overlay.3;
    let base = ELLIPSE_COLOR;
    [
        linear_to_srgb(srgb_to_linear(overlay.0) * alpha + srgb_to_linear(base[0]) * (1.0 - alpha)),
        linear_to_srgb(srgb_to_linear(overlay.1) * alpha + srgb_to_linear(base[1]) * (1.0 - alpha)),
        linear_to_srgb(srgb_to_linear(overlay.2) * alpha + srgb_to_linear(base[2]) * (1.0 - alpha)),
        1.0,
    ]
}

fn assert_color_near_position(
    pixels: &[u8],
    position: [f32; 2],
    expected: [f32; 4],
    radius: usize,
    tolerance: u8,
    what: &str,
) {
    let center_x = position[0].round() as usize;
    let center_y = position[1].round() as usize;
    let x_start = center_x.saturating_sub(radius);
    let x_end = (center_x + radius).min(TARGET_WIDTH as usize - 1);
    let y_start = center_y.saturating_sub(radius);
    let y_end = (center_y + radius).min(TARGET_HEIGHT as usize - 1);
    let expected_bytes = expected.map(|channel| (channel * 255.0).round().clamp(0.0, 255.0) as u8);
    let found = (y_start..=y_end).any(|y| {
        (x_start..=x_end).any(|x| {
            pixel_at(pixels, x, y)
                .iter()
                .zip(expected_bytes)
                .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
        })
    });
    assert!(
        found,
        "{what} not visible near {position:?} with color {expected_bytes:?} ±{tolerance}"
    );
}

#[cfg(test)]
fn sample_canvas() -> (Canvas, u64) {
    let mut canvas = Canvas::default();
    canvas
        .add(
            [20.0, 20.0],
            CanvasShape::Rectangle { size: [80.0, 60.0] },
            RECT_COLOR,
        )
        .expect("rectangle should be valid");
    let selected_ellipse = canvas
        .add(
            [125.0, 20.0],
            CanvasShape::Ellipse { size: [80.0, 70.0] },
            ELLIPSE_COLOR,
        )
        .expect("ellipse should be valid");
    canvas.selected.insert(selected_ellipse);
    canvas
        .add(
            [25.0, 112.0],
            CanvasShape::Line {
                end: [180.0, 152.0],
            },
            LINE_COLOR,
        )
        .expect("line should be valid");
    canvas
        .add(
            [248.0, 24.0],
            CanvasShape::Image {
                size: [88.0, 88.0],
                asset: "canvas/checker.svg".to_owned(),
            },
            [1.0; 4],
        )
        .expect("image should be valid");
    canvas
        .add(
            [420.0, 210.0],
            CanvasShape::Rectangle {
                size: [100.0, 80.0],
            },
            CLIPPED_COLOR,
        )
        .expect("clipped rectangle should be valid");
    (canvas, selected_ellipse)
}

#[test]
fn canvas_shapes_svg_selection_and_clip_render_at_dark_light_dpi_variants() {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .set(AssetPlugin {
                file_path: asset_root.to_string_lossy().into_owned(),
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
    let (canvas, selected_ellipse) = sample_canvas();
    let root = WidgetTree::new(WidgetNode::canvas("Test canvas", canvas).with_key("canvas"))
        .spawn(app.world_mut(), None)
        .expect("test canvas should mount");
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(root).unwrap();
        layout.size.width = UiLength::Px(CANVAS_RECT.width);
        layout.size.height = UiLength::Px(CANVAS_RECT.height);
        layout.clip_contents = true;
    }
    app.world_mut()
        .entity_mut(root)
        .insert(UiLayoutRoot::new(camera, CANVAS_RECT, 1.0));

    let asset_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < asset_deadline
        && app
            .world()
            .resource::<CanvasImageAssets>()
            .state("canvas/checker.svg")
            != Some(&CanvasImageState::Ready)
    {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.world()
            .resource::<CanvasImageAssets>()
            .state("canvas/checker.svg"),
        Some(&CanvasImageState::Ready),
        "Canvas SVG asset did not load: {:?}",
        app.world()
            .resource::<CanvasImageAssets>()
            .state("canvas/checker.svg")
    );
    let controls: Vec<_> = {
        let world = app.world_mut();
        let mut query = world.query::<&bbui_widgets::CanvasObjectControl>();
        query
            .iter(world)
            .filter(|control| control.owner == root)
            .cloned()
            .collect()
    };
    let mounted_controls = controls.len();
    assert_eq!(mounted_controls, 5, "visible canvas objects should mount");
    assert!(controls.iter().any(|control| {
        control.owner == root && control.object_id == selected_ellipse && control.selected
    }));

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
                .expect("Canvas GPU readback did not complete");
            app.world_mut().entity_mut(readback).despawn();

            assert_rgba_near(
                pixel_at(&pixels, 16 + 40, 16 + 45),
                RECT_COLOR,
                5,
                "canvas rectangle fill",
            );
            assert_rgba_near(
                pixel_at(&pixels, 16 + 165, 16 + 55),
                canvas_selected_fill(&theme),
                7,
                "canvas selected ellipse fill with translucent selection overlay",
            );
            assert_color_near_position(
                &pixels,
                [16.0 + 105.0, 16.0 + 132.0],
                LINE_COLOR,
                4,
                10,
                "canvas line",
            );
            assert_rgba_near(
                pixel_at(&pixels, 16 + 248 + 44, 16 + 24 + 44),
                [0.89, 0.54, 0.27, 1.0],
                16,
                "canvas SVG image center",
            );
            assert_rgba_near(
                pixel_at(&pixels, 16 + 450, 16 + 240),
                CLIPPED_COLOR,
                5,
                "canvas clipped rectangle inside viewport",
            );
            let outside_clip = pixel_at(&pixels, 16 + 470, 16 + 240);
            assert!(
                outside_clip
                    .iter()
                    .zip(CLIPPED_COLOR)
                    .any(|(actual, expected)| {
                        actual.abs_diff((expected * 255.0).round() as u8) > 32
                    }),
                "canvas content leaked outside its clip: {outside_clip:?}"
            );
            assert_color_near_position(
                &pixels,
                [16.0 + 165.0, 16.0 + 20.0],
                [
                    theme.selection.0,
                    theme.selection.1,
                    theme.selection.2,
                    theme.selection.3,
                ],
                3,
                12,
                "selected ellipse outline",
            );
            assert_rgba_near(
                pixel_at(&pixels, 16 + 400, 16 + 180),
                [
                    theme.surface.0,
                    theme.surface.1,
                    theme.surface.2,
                    theme.surface.3,
                ],
                5,
                "canvas background",
            );
            println!("Canvas scene readback passed: {kind:?}, dpi={dpi}");
            scenario += 1;
        }
    }
}

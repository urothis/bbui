//! Pixel-level smoke coverage for the actual Bevy UI render pipeline.
//!
//! Linux uses Mesa's software Vulkan adapter in CI. The test does not need a desktop window:
//! Bevy renders to an `Image` target and copies the result back with `GpuReadbackPlugin`.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

use bbui::BbuiPlugin;
use bbui_core::{UiNode, set_parent};
use bbui_layout::{
    Rect, UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength, UiSize,
};
use bbui_render::{
    ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive, RenderTargetCamera, UiPaint,
};
use bbui_style::{Rgba, Theme, ThemeKind};
use bevy::{
    app::PluginGroup,
    asset::{Assets, RenderAssetUsages},
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, ResMut, Resource, WindowPlugin},
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};

const TARGET_WIDTH: u32 = 96;
const TARGET_HEIGHT: u32 = 64;

#[derive(Resource, Default)]
struct CapturedPixels(HashMap<usize, CapturedFrame>);

#[derive(Clone)]
struct CapturedFrame {
    pixels: Vec<u8>,
    completed_at: Instant,
}

fn full_rect() -> ClipRect {
    ClipRect {
        x: 0.0,
        y: 0.0,
        width: TARGET_WIDTH as f32,
        height: TARGET_HEIGHT as f32,
    }
}

fn assert_theme_pixel(pixel: &[u8], color: Rgba, what: &str) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 3,
            "{what} color differed: actual={pixel:?}, expected={expected:?}"
        );
    }
}

fn assert_pixel_near(pixel: &[u8], expected: [u8; 4], tolerance: u8, what: &str) {
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "{what} color differed: actual={pixel:?}, expected={expected:?} ±{tolerance}"
        );
    }
}

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn assert_text_is_visible_and_clipped(pixels: &[u8], theme: &Theme) -> usize {
    let background = pixel_at(pixels, 84, 16)[0];
    let text = (theme.text.0 * 255.0).round() as u8;
    let visible_pixels = (4..28)
        .flat_map(|y| (64..80).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let actual = pixel_at(pixels, *x, *y)[0];
            let distance_to_background = actual.abs_diff(background);
            distance_to_background >= 60 && actual.abs_diff(text) < distance_to_background
        })
        .count();
    assert!(
        visible_pixels >= 8,
        "expected visible text glyph pixels inside its clip, found {visible_pixels}"
    );

    for y in 4..28 {
        for x in 80..TARGET_WIDTH as usize {
            if x == 89 && (4..24).contains(&y) {
                continue;
            }
            assert_theme_pixel(
                pixel_at(pixels, x, y),
                theme.background,
                "outside text clip",
            );
        }
    }
    visible_pixels
}

#[test]
fn offscreen_image_target_contains_bbui_gpu_output() {
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
                    force_fallback_adapter: cfg!(target_os = "linux"),
                    ..Default::default()
                })),
                ..Default::default()
            })
            .disable::<WinitPlugin>()
            // The pixel scene is UI-only; PBR adds startup systems that expect a windowed app.
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(BbuiPlugin)
    .init_resource::<CapturedPixels>();
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
    let icon = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        ));
    app.world_mut()
        .resource_mut::<ImageRegistry>()
        .insert(1, icon);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();
    app.world_mut().resource_mut::<RenderTargetCamera>().0 = Some(camera);

    let layout_root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(TARGET_WIDTH as f32),
                    height: UiLength::Px(TARGET_HEIGHT as f32),
                },
                ..Default::default()
            },
            UiLayoutRoot::new(
                camera,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: TARGET_WIDTH as f32,
                    height: TARGET_HEIGHT as f32,
                },
                1.0,
            ),
        ))
        .id();
    let clip_wrapper = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(16.0),
                    height: UiLength::Px(TARGET_HEIGHT as f32),
                },
                clip_contents: true,
                ..Default::default()
            },
        ))
        .id();
    set_parent(app.world_mut(), clip_wrapper, layout_root).unwrap();
    let mut fill_layout = UiLayoutNode::leaf(
        UiLength::Px(TARGET_WIDTH as f32),
        UiLength::Px(TARGET_HEIGHT as f32),
    );
    fill_layout.z_index = 1;
    let accent_fill = app
        .world_mut()
        .spawn((
            UiNode,
            fill_layout,
            UiPaint(Primitive::RoundedRect {
                rect: full_rect(),
                radius: 0.0,
                color: Theme::default().accent,
            }),
        ))
        .id();
    set_parent(app.world_mut(), accent_fill, clip_wrapper).unwrap();

    // Let the image asset and UI render pipeline complete their first frame before readback.
    for _ in 0..10 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }

    let mut scenario_index = 0;
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        for dpi_scale in [1.0, 2.0] {
            let theme = Theme::new(kind);
            *app.world_mut().resource_mut::<Theme>() = theme.clone();
            app.world_mut()
                .get_mut::<UiLayoutRoot>(layout_root)
                .unwrap()
                .dpi_scale = dpi_scale;
            app.world_mut().get_mut::<UiPaint>(accent_fill).unwrap().0 = Primitive::RoundedRect {
                rect: full_rect(),
                radius: 0.0,
                color: theme.accent,
            };
            {
                let mut display_list = app.world_mut().resource_mut::<DisplayList>();
                display_list.clear();
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 0,
                    primitive: Primitive::RoundedRect {
                        rect: full_rect(),
                        radius: 0.0,
                        color: theme.background,
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 2,
                    primitive: Primitive::RoundedRect {
                        rect: ClipRect {
                            x: 24.0,
                            y: 2.0,
                            width: 14.0,
                            height: 14.0,
                        },
                        radius: 0.0,
                        color: Rgba(0.20, 0.80, 0.20, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 3,
                    primitive: Primitive::RoundedRect {
                        rect: ClipRect {
                            x: 30.0,
                            y: 6.0,
                            width: 14.0,
                            height: 14.0,
                        },
                        radius: 0.0,
                        color: Rgba(0.80, 0.20, 0.20, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 4,
                    primitive: Primitive::Icon {
                        rect: ClipRect {
                            x: 50.0,
                            y: 4.0,
                            width: 12.0,
                            height: 12.0,
                        },
                        asset_id: 1,
                        tint: Rgba(0.95, 0.20, 0.75, 1.0),
                    },
                });
                let shadow_width = if dpi_scale == 1.0 { 14.0 } else { 22.0 };
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 4,
                    primitive: Primitive::Shadow {
                        rect: ClipRect {
                            x: 22.0,
                            y: 20.0,
                            width: shadow_width,
                            height: 10.0,
                        },
                        radius: 2.0,
                        color: theme.surface,
                        shadow: Rgba(0.92, 0.08, 0.06, 1.0),
                        offset: [3.0, 3.0],
                        blur: 0.0,
                        spread: 0.0,
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 5,
                    primitive: Primitive::HsvWheel {
                        rect: ClipRect {
                            x: 0.0,
                            y: 36.0,
                            width: 28.0,
                            height: 28.0,
                        },
                        hue_degrees: 0.0,
                        saturation: 0.5,
                        value: 0.8,
                    },
                });
                let left_clip = ClipRect {
                    x: 24.0,
                    y: 32.0,
                    width: 28.0,
                    height: 24.0,
                };
                let right_clip = ClipRect {
                    x: 44.0,
                    y: 32.0,
                    width: 28.0,
                    height: 24.0,
                };
                let crossing_fill = ClipRect {
                    x: 20.0,
                    y: 30.0,
                    width: 60.0,
                    height: 28.0,
                };
                // Keep z values interleaved between two overlapping clip groups. The final
                // left-group fill must cover the right-group fill in their shared area.
                display_list.push(DrawItem {
                    clip: left_clip,
                    z: 6,
                    primitive: Primitive::RoundedRect {
                        rect: crossing_fill,
                        radius: 0.0,
                        color: Rgba(0.15, 0.30, 0.90, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: right_clip,
                    z: 7,
                    primitive: Primitive::RoundedRect {
                        rect: crossing_fill,
                        radius: 0.0,
                        color: Rgba(0.10, 0.80, 0.20, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: left_clip,
                    z: 8,
                    primitive: Primitive::RoundedRect {
                        rect: crossing_fill,
                        radius: 0.0,
                        color: Rgba(0.90, 0.20, 0.10, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: ClipRect {
                        x: 64.0,
                        y: 4.0,
                        width: 16.0,
                        height: 24.0,
                    },
                    z: 5,
                    primitive: Primitive::TextWrapped {
                        origin: [64.0, 4.0],
                        text: "MMMMMMMM".to_owned(),
                        color: theme.text,
                        max_width: 32.0,
                        wrap: false,
                        style: None,
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 9,
                    primitive: Primitive::Ellipse {
                        rect: ClipRect {
                            x: 78.0,
                            y: 36.0,
                            width: 16.0,
                            height: 24.0,
                        },
                        color: Rgba(0.95, 0.30, 0.10, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 10,
                    primitive: Primitive::Line {
                        from: [89.0, 4.0],
                        to: [89.0, 24.0],
                        width: 1.0,
                        color: Rgba(1.0, 1.0, 0.0, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 11,
                    primitive: Primitive::Line {
                        from: [34.0, 60.0],
                        to: [58.0, 60.0],
                        width: 1.0,
                        color: Rgba(0.0, 0.60, 1.0, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 12,
                    primitive: Primitive::Line {
                        from: [34.0, 56.0],
                        to: [58.0, 32.0],
                        width: 1.0,
                        color: Rgba(0.20, 1.0, 0.80, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 13,
                    primitive: Primitive::Bezier {
                        points: [[46.0, 24.0], [50.0, 24.0], [54.0, 24.0], [58.0, 24.0]],
                        width: 6.0,
                        color: Rgba(0.95, 0.10, 0.12, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 14,
                    primitive: Primitive::Bezier {
                        points: [[46.0, 24.0], [50.0, 24.0], [54.0, 24.0], [58.0, 24.0]],
                        width: 6.0,
                        color: Rgba(0.05, 0.20, 0.95, 1.0),
                    },
                });
                display_list.push(DrawItem {
                    clip: full_rect(),
                    z: 15,
                    primitive: Primitive::RoundedRect {
                        rect: ClipRect {
                            x: 50.0,
                            y: 22.0,
                            width: 6.0,
                            height: 5.0,
                        },
                        radius: 0.0,
                        color: Rgba(0.15, 0.90, 0.35, 1.0),
                    },
                });
            }

            for _ in 0..3 {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let current_scenario = scenario_index;
            scenario_index += 1;
            let readback_started = Instant::now();
            let readback = app
                .world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(
                    move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                          mut pixels: ResMut<CapturedPixels>| {
                        pixels.0.insert(
                            current_scenario,
                            CapturedFrame {
                                pixels: captured.data.clone(),
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
                    .resource::<CapturedPixels>()
                    .0
                    .contains_key(&current_scenario)
            {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let pixels = app
                .world()
                .resource::<CapturedPixels>()
                .0
                .get(&current_scenario)
                .cloned()
                .expect("GPU readback did not complete before the deadline");
            let readback_latency = pixels.completed_at.duration_since(readback_started);
            let pixels = pixels.pixels;
            app.world_mut().entity_mut(readback).despawn();

            let layout = app.world().get::<UiLayoutResult>(accent_fill).unwrap();
            assert_eq!(layout.dpi_scale, dpi_scale);
            assert_eq!(
                layout.clip,
                Some(Rect::new(0.0, 0.0, 16.0, TARGET_HEIGHT as f32))
            );
            assert_eq!(
                layout.rect,
                Rect::new(0.0, 0.0, TARGET_WIDTH as f32, TARGET_HEIGHT as f32)
            );

            assert_theme_pixel(
                pixel_at(&pixels, 10, 16),
                theme.accent,
                "clipped theme accent",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 20, 16),
                theme.background,
                "outside-clip theme background",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 15, 16),
                theme.accent,
                "last pixel inside clip",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 16, 16),
                theme.background,
                "first pixel outside clip",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 26, 8),
                Rgba(0.20, 0.80, 0.20, 1.0),
                "lower z-order region",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 32, 8),
                Rgba(0.80, 0.20, 0.20, 1.0),
                "higher z-order overlap",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 30, 40),
                Rgba(0.90, 0.20, 0.10, 1.0),
                "left clip group's later z item",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 48, 40),
                Rgba(0.90, 0.20, 0.10, 1.0),
                "interleaved z across overlapping clip groups",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 62, 40),
                Rgba(0.10, 0.80, 0.20, 1.0),
                "right clip group's region",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 72, 40),
                theme.background,
                "outside both overlapping clip groups",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 56, 10),
                Rgba(0.95, 0.20, 0.75, 1.0),
                "tinted icon",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 28, 25),
                theme.surface,
                "box shadow content fill",
            );
            if dpi_scale == 1.0 {
                assert_theme_pixel(
                    pixel_at(&pixels, 38, 25),
                    Rgba(0.92, 0.08, 0.06, 1.0),
                    "box shadow outside resized content",
                );
            } else {
                assert_theme_pixel(
                    pixel_at(&pixels, 38, 25),
                    theme.surface,
                    "box shadow resized content extent",
                );
            }
            assert_theme_pixel(
                pixel_at(&pixels, 86, 48),
                Rgba(0.95, 0.30, 0.10, 1.0),
                "filled ellipse center",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 80, 48),
                Rgba(0.95, 0.30, 0.10, 1.0),
                "filled ellipse horizontal axis",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 86, 38),
                Rgba(0.95, 0.30, 0.10, 1.0),
                "filled ellipse vertical axis",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 78, 36),
                theme.background,
                "ellipse corner remains transparent",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 89, 14),
                Rgba(1.0, 1.0, 0.0, 1.0),
                "one-pixel vertical line",
            );
            assert_theme_pixel(
                pixel_at(&pixels, 40, 60),
                Rgba(0.0, 0.60, 1.0, 1.0),
                "one-pixel horizontal line",
            );
            let diagonal_hits = (42..=46)
                .flat_map(|y| (44..=48).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let pixel = pixel_at(&pixels, *x, *y);
                    pixel[0] < 220 && pixel[1] > 100 && pixel[2] > 80
                })
                .count();
            assert!(
                diagonal_hits > 0,
                "one-pixel diagonal line was not visible near its midpoint; neighborhood hits={diagonal_hits}"
            );
            assert_pixel_near(
                pixel_at(&pixels, 14, 37),
                [138, 0, 255, 255],
                8,
                "GPU HSV hue ring",
            );
            assert_pixel_near(
                pixel_at(&pixels, 13, 50),
                [204, 194, 194, 255],
                8,
                "GPU HSV saturation/value disk",
            );
            assert_pixel_near(
                pixel_at(&pixels, 56, 24),
                [13, 51, 242, 255],
                6,
                "later Bezier within one batched draw",
            );
            assert_pixel_near(
                pixel_at(&pixels, 52, 24),
                [38, 230, 89, 255],
                6,
                "non-Bezier z order after batched curves",
            );
            let visible_glyph_pixels = assert_text_is_visible_and_clipped(&pixels, &theme);
            println!(
                "GPU scene {kind:?} dpi={dpi_scale}: background={:?}, accent={:?}, lower-z={:?}, upper-z={:?}, icon-tint={:?}, hsv-ring={:?}, hsv-disk={:?}, visible glyph pixels={visible_glyph_pixels}, CPU readback round-trip={readback_latency:?}",
                pixel_at(&pixels, 84, 16),
                pixel_at(&pixels, 10, 16),
                pixel_at(&pixels, 26, 8),
                pixel_at(&pixels, 32, 8),
                pixel_at(&pixels, 56, 10),
                pixel_at(&pixels, 14, 37),
                pixel_at(&pixels, 13, 50),
            );
        }
    }
}

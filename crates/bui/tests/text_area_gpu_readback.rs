//! Pixel coverage for wrapped TextArea glyphs, selection, and caret paint.
#![cfg(target_os = "linux")]

use std::{collections::HashMap, thread, time::Duration};

use bevy::{
    app::PluginGroup,
    asset::{Assets, Handle},
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
    widgets::{WidgetNode, WidgetTree},
};
use bui_core::{UiChildren, UiFocus};
use bui_layout::{Rect, UiLayoutNode, UiLayoutRoot, UiLength};
use bui_render::{ClipRect, DisplayList, Primitive, UiPaint};
use bui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};
use bui_text::{TextBuffer, UiTextLayout};

const TARGET_WIDTH: u32 = 640;
const TARGET_HEIGHT: u32 = 360;

#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, Vec<u8>>);

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn color_bytes(color: Rgba) -> [u8; 4] {
    [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ]
}

fn color_distance(pixel: &[u8], color: [u8; 4]) -> u16 {
    pixel
        .iter()
        .zip(color)
        .map(|(actual, expected)| u16::from(actual.abs_diff(expected)))
        .sum()
}

fn blended(foreground: Rgba, background: Rgba) -> [u8; 4] {
    let alpha = foreground.3.clamp(0.0, 1.0);
    color_bytes(Rgba(
        foreground.0 * alpha + background.0 * (1.0 - alpha),
        foreground.1 * alpha + background.1 * (1.0 - alpha),
        foreground.2 * alpha + background.2 * (1.0 - alpha),
        1.0,
    ))
}

fn assert_line_pixels(
    pixels: &[u8],
    origin: [f32; 2],
    max_width: f32,
    glyph_y: f32,
    scale: f32,
    text: Rgba,
    line: usize,
) {
    let text = color_bytes(text);
    let left = origin[0].floor().max(0.0) as usize;
    let right = (origin[0] + max_width).ceil().min(TARGET_WIDTH as f32) as usize;
    let center_y = origin[1] + glyph_y / scale;
    let top = (center_y - 6.0).floor().max(0.0) as usize;
    let bottom = (center_y + 15.0).ceil().min(TARGET_HEIGHT as f32) as usize;
    let count = (top..bottom)
        .flat_map(|y| (left..right).map(move |x| (x, y)))
        .filter(|(x, y)| color_distance(pixel_at(pixels, *x, *y), text) < 70)
        .count();
    assert!(
        count >= 4,
        "wrapped text line {line} has only {count} near-foreground pixels"
    );
}

fn capture(app: &mut App, target: &Handle<Image>, frame: usize) -> Vec<u8> {
    let readback = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(
            move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                  mut frames: ResMut<CapturedFrames>| {
                frames.0.insert(frame, captured.data.clone());
            },
        )
        .id();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline
        && !app
            .world()
            .resource::<CapturedFrames>()
            .0
            .contains_key(&frame)
    {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let pixels = app
        .world()
        .resource::<CapturedFrames>()
        .0
        .get(&frame)
        .cloned()
        .expect("TextArea GPU readback did not complete before the deadline");
    app.world_mut().entity_mut(readback).despawn();
    pixels
}

#[test]
fn offscreen_text_area_shows_wrapped_selection_and_caret_for_dark_light_and_dpi() {
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

    let mut image = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();
    app.world_mut()
        .resource_mut::<bui_render::RenderTargetCamera>()
        .0 = Some(camera);

    let value = "    Wrapped text uses the same Bevy shaping for glyph painting and caret geometry. This paragraph fills several visual lines and keeps an unselected line visible below the selected spaces. More words keep the content long enough to verify wrapping in the GPU output.";
    let root = WidgetTree::new(WidgetNode::column([WidgetNode::text_area("Memo", value)]))
        .spawn(app.world_mut(), None)
        .unwrap();
    {
        let mut node = app.world_mut().get_mut::<UiLayoutNode>(root).unwrap();
        node.size.width = UiLength::Px(TARGET_WIDTH as f32);
        node.size.height = UiLength::Px(TARGET_HEIGHT as f32);
    }
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(
            camera,
            Rect::new(0.0, 0.0, TARGET_WIDTH as f32, TARGET_HEIGHT as f32),
            1.0,
        ),
        UiPaint(Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: TARGET_WIDTH as f32,
                height: TARGET_HEIGHT as f32,
            },
            radius: 0.0,
            color: Theme::default().background,
        }),
    ));
    let area = app.world().get::<UiChildren>(root).unwrap().0[0];
    {
        let mut node = app.world_mut().get_mut::<UiLayoutNode>(area).unwrap();
        node.size.width = UiLength::Px(420.0);
        node.size.height = UiLength::Px(180.0);
    }
    app.world_mut()
        .get_mut::<TextBuffer>(area)
        .unwrap()
        .set_selection(0, 4);
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(camera)
        .focused = Some(area);

    for _ in 0..10 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }

    for (scenario, (kind, dpi_scale)) in [
        (ThemeKind::Dark, 1.0),
        (ThemeKind::Dark, 2.0),
        (ThemeKind::Light, 1.0),
        (ThemeKind::Light, 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        let theme = Theme::new(kind);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        app.world_mut()
            .get_mut::<UiLayoutRoot>(root)
            .unwrap()
            .dpi_scale = dpi_scale;
        if let Some(mut paint) = app.world_mut().get_mut::<UiPaint>(root)
            && let Primitive::RoundedRect { color, .. } = &mut paint.0
        {
            *color = theme.background;
        }
        for _ in 0..3 {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        let area_result = *app.world().get::<bui_layout::UiLayoutResult>(area).unwrap();
        assert_eq!(area_result.dpi_scale, dpi_scale);
        let selection_overlay = theme.color(ThemeColorToken::TextSelectionOverlay);
        let (origin, text_draw, line_y, selection_rect, caret_rect, line_count) = {
            let buffer = app.world().get::<TextBuffer>(area).unwrap();
            let text_layout = app.world().get::<UiTextLayout>(area).unwrap();
            let shaped = text_layout.layout().expect("TextArea should be shaped");
            let first_line_glyph = shaped
                .glyphs()
                .iter()
                .find(|glyph| glyph.line == 0 && !glyph.character.is_whitespace())
                .expect("first shaped line should contain a visible glyph");
            let second_line_glyph = shaped
                .glyphs()
                .iter()
                .find(|glyph| glyph.line == 1 && !glyph.character.is_whitespace())
                .expect("wrapped second line should contain a visible glyph");
            (
                text_layout.text_origin().unwrap(),
                app.world()
                    .resource::<DisplayList>()
                    .items()
                    .iter()
                    .find_map(|item| match &item.primitive {
                        Primitive::TextWrapped {
                            text,
                            max_width,
                            wrap: true,
                            ..
                        } if text.contains("Wrapped text") => Some(*max_width),
                        _ => None,
                    })
                    .expect("renderer display list should contain wrapped editor text"),
                [first_line_glyph.y, second_line_glyph.y],
                text_layout
                    .selection_rects(buffer)
                    .into_iter()
                    .next()
                    .expect("selected leading spaces should produce selection geometry"),
                text_layout.caret_rect(buffer).unwrap(),
                shaped.line_count(),
            )
        };
        assert!(line_count >= 2, "content should wrap at DPI {dpi_scale}");
        let display_list = app.world().resource::<DisplayList>();
        assert!(display_list.items().iter().any(|item| {
            matches!(
                &item.primitive,
                Primitive::TextWrapped {
                    text,
                    wrap: true,
                    ..
                } if text.contains("Wrapped text")
            )
        }));
        assert!(display_list.items().iter().any(|item| {
            matches!(
                item.primitive,
                Primitive::RoundedRect { color, .. } if color == selection_overlay
            )
        }));
        assert!(
            display_list.items().iter().any(|item| {
                let Primitive::Line { from, to, .. } = &item.primitive else {
                    return false;
                };
                (from[0] - caret_rect.x).abs() < 0.01
                    && (from[1] - caret_rect.y).abs() < 0.01
                    && (to[0] - caret_rect.x).abs() < 0.01
                    && (to[1] - (caret_rect.y + caret_rect.height)).abs() < 0.01
            }),
            "focused TextArea should paint its caret line"
        );
        let pixels = capture(&mut app, &target, scenario);
        assert_line_pixels(
            &pixels, origin, text_draw, line_y[0], dpi_scale, theme.text, 0,
        );
        assert_line_pixels(
            &pixels, origin, text_draw, line_y[1], dpi_scale, theme.text, 1,
        );

        let caret_sample = [caret_rect.x, caret_rect.y + caret_rect.height * 0.5];
        let caret_color = color_bytes(theme.text);
        let mut nearest_caret = (u16::MAX, [0usize, 0usize], [0u8; 4]);
        let line_top = (caret_rect.y + caret_rect.height * 0.45).floor() as isize;
        let line_bottom = (caret_rect.y + caret_rect.height).ceil() as isize;
        let found_caret = (line_top..line_bottom).any(|y| {
            (-4..=4).any(|dx| {
                let x = (caret_rect.x.round() as isize + dx).clamp(0, TARGET_WIDTH as isize - 1)
                    as usize;
                let y = y.clamp(0, TARGET_HEIGHT as isize - 1) as usize;
                let pixel = pixel_at(&pixels, x, y);
                let distance = color_distance(pixel, caret_color);
                if distance < nearest_caret.0 {
                    nearest_caret = (distance, [x, y], pixel.try_into().unwrap());
                }
                distance < 48
            })
        });
        assert!(
            found_caret,
            "focused caret should be visible at {caret_sample:?} with geometry {caret_rect:?}; nearest={nearest_caret:?}, expected={caret_color:?}"
        );

        // The selected range is four leading spaces, so a lower sample in the
        // shaped selection rectangle contains the overlay without glyph ink.
        let selection_sample = [
            (selection_rect.x + selection_rect.width * 0.5).floor() as usize,
            (selection_rect.y + selection_rect.height * 0.78).floor() as usize,
        ];
        let expected_selection = blended(selection_overlay, theme.surface);
        let selection_pixel = pixel_at(&pixels, selection_sample[0], selection_sample[1]);
        assert!(
            color_distance(selection_pixel, expected_selection) < 100,
            "selection overlay should be GPU-visible at {selection_sample:?}: pixel={selection_pixel:?}, expected≈{expected_selection:?}"
        );
    }
}

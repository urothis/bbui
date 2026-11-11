//! Pixel coverage for the numeric expression error treatment on the real Bevy UI renderer.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

use bbui::{
    BbuiPlugin,
    widgets::{WidgetNode, WidgetTree},
};
use bbui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bbui_render::{DisplayList, Primitive};
use bbui_style::{Rgba, Theme, ThemeKind};
use bbui_widgets::NumericField;
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

const TARGET_WIDTH: u32 = 256;
const TARGET_HEIGHT: u32 = 96;

#[derive(Resource, Default)]
struct CapturedPixels(HashMap<usize, Vec<u8>>);

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
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
            "{what} pixel differed: actual={pixel:?}, expected={expected:?}"
        );
    }
}

#[test]
fn numeric_expression_errors_show_error_text_and_outline_for_theme_and_dpi_variants() {
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
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();

    let field = WidgetTree::new(WidgetNode::number("Scale", 0.5, 0.0, 1.0, 0.1))
        .spawn(app.world_mut(), None)
        .unwrap();
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(field).unwrap();
        layout.size.width = UiLength::Px(200.0);
        layout.size.height = UiLength::Px(40.0);
    }
    app.world_mut().entity_mut(field).insert(UiLayoutRoot::new(
        camera,
        Rect::new(12.0, 16.0, 200.0, 40.0),
        1.0,
    ));
    let error = app
        .world_mut()
        .get_mut::<NumericField>(field)
        .unwrap()
        .evaluate_input("1 / 0", &HashMap::new())
        .unwrap_err();
    assert_eq!(error.to_string(), "division by zero");
    assert_eq!(
        app.world().get::<NumericField>(field).unwrap().value,
        0.5,
        "failed expressions must preserve the current numeric value"
    );

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
            .get_mut::<UiLayoutRoot>(field)
            .unwrap()
            .dpi_scale = dpi_scale;
        for _ in 0..4 {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        let rect = app
            .world()
            .get::<UiLayoutResult>(field)
            .expect("numeric root should have a computed layout")
            .rect;
        let primitives = app
            .world()
            .resource::<DisplayList>()
            .camera_items(camera)
            .unwrap_or_default();
        assert!(primitives.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, color, .. }
                if text.contains("division by zero") && *color == theme.error
        )));
        assert_eq!(
            primitives
                .iter()
                .filter(|item| matches!(
                    &item.primitive,
                    Primitive::Line { color, .. } if *color == theme.error
                ))
                .count(),
            4,
            "the error treatment should paint four outline edges"
        );

        let readback = app
            .world_mut()
            .spawn(Readback::texture(target.clone()))
            .observe(
                move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                      mut pixels: ResMut<CapturedPixels>| {
                    pixels.0.insert(scenario, captured.data.clone());
                },
            )
            .id();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline
            && !app
                .world()
                .resource::<CapturedPixels>()
                .0
                .contains_key(&scenario)
        {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let pixels = app
            .world()
            .resource::<CapturedPixels>()
            .0
            .get(&scenario)
            .cloned()
            .expect("numeric expression error GPU readback did not complete");
        app.world_mut().entity_mut(readback).despawn();

        // The top outline stroke is 2px thick and clipped to the numeric control bounds.
        // Its inner row stays crisp at both configured DPI scales.
        assert_color(
            pixel_at(
                &pixels,
                (rect.x + rect.width * 0.5).round() as usize,
                rect.y.round() as usize,
            ),
            theme.error,
            "numeric expression error outline",
        );

        let red_text_pixels = (rect.y as usize + 3..(rect.y + rect.height - 3.0) as usize)
            .flat_map(|y| {
                (rect.x as usize + 4..(rect.x + rect.width - 4.0) as usize).map(move |x| (x, y))
            })
            .filter(|(x, y)| {
                let pixel = pixel_at(&pixels, *x, *y);
                let expected = [
                    (theme.error.0 * 255.0).round() as u8,
                    (theme.error.1 * 255.0).round() as u8,
                    (theme.error.2 * 255.0).round() as u8,
                ];
                pixel[..3]
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 24)
            })
            .count();
        assert!(
            red_text_pixels > 0,
            "expected GPU-rendered error-colored glyph pixels, found none for {kind:?} at DPI {dpi_scale}"
        );
    }
}

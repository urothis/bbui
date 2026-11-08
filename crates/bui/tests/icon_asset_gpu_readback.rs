//! GPU readback for AssetServer-backed icon buttons and their visible-owner lifecycle.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bevy::{
    app::PluginGroup,
    asset::{AssetPlugin, Assets, Handle},
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, ResMut, Resource, WindowPlugin},
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
use bui::{BuiPlugin, widgets::WidgetNode};
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bui_render::{DisplayList, ImageRegistry, Primitive, UiPaint};
use bui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};
use bui_widgets::{
    IconButtonAsset, IconButtonAssetState, IconButtonAssetStatus, WidgetTree,
    icon_assets::IconButtonAssetCache,
};

const TARGET_WIDTH: u32 = 128;
const TARGET_HEIGHT: u32 = 96;
const CHECKER_SVG: &[u8] = include_bytes!("../../../assets/canvas/checker.svg");

#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, Vec<u8>>);

fn temp_directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("bui-icon-gpu-{}-{nonce}", std::process::id()))
}

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
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

fn expected_tinted_pixel(source: [u8; 3], tint: Rgba) -> [u8; 4] {
    let source = source.map(|channel| f32::from(channel) / 255.0);
    let tint = [tint.0, tint.1, tint.2];
    let mut result = [0; 4];
    for channel in 0..3 {
        let linear = srgb_to_linear(source[channel]) * srgb_to_linear(tint[channel]);
        result[channel] = (linear_to_srgb(linear) * 255.0).round() as u8;
    }
    result[3] = 255;
    result
}

fn assert_pixel_near(actual: &[u8], expected: [u8; 4], what: &str) {
    for (channel, expected) in actual.iter().zip(expected) {
        assert!(
            channel.abs_diff(expected) <= 8,
            "{what}: actual={actual:?}, expected={expected:?} ±8"
        );
    }
}

fn await_state(app: &mut App, entity: bevy::prelude::Entity, expected: IconButtonAssetStatus) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if app
            .world()
            .get::<IconButtonAssetState>(entity)
            .is_some_and(|state| state.status == expected)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "icon asset did not reach {expected:?}; state={:?}",
            app.world()
                .get::<IconButtonAssetState>(entity)
                .map(|state| &state.status)
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn await_failed_state(app: &mut App, entity: bevy::prelude::Entity) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if app
            .world()
            .get::<IconButtonAssetState>(entity)
            .is_some_and(|state| matches!(state.status, IconButtonAssetStatus::Failed(_)))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "icon asset did not reach a failure state; state={:?}",
            app.world()
                .get::<IconButtonAssetState>(entity)
                .map(|state| &state.status)
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn expected_readback(app: &mut App, target: &Handle<Image>, frame: usize) -> Vec<u8> {
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
    let pixels = app
        .world()
        .resource::<CapturedFrames>()
        .0
        .get(&frame)
        .cloned()
        .expect("icon GPU readback did not complete before the deadline");
    app.world_mut().entity_mut(readback).despawn();
    pixels
}

#[test]
fn icon_assets_retry_tint_and_release_in_dark_light_and_dpi_variants() {
    let directory = temp_directory();
    fs::create_dir_all(&directory).unwrap();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: directory.to_string_lossy().into_owned(),
                ..Default::default()
            })
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

    let mut target_image = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target_image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(target_image);
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Image(target.clone().into()),
        IsDefaultUiCamera,
    ));
    // The layout root points at a non-camera window identity so the widget painter and icon
    // overlay use the default screen layer, while the one target camera still captures that layer.
    let layout_window = app.world_mut().spawn_empty().id();
    let icon = WidgetTree::new(WidgetNode::icon_button_asset(
        "recovery.svg",
        "Recovered icon",
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(icon).unwrap();
        layout.size.width = UiLength::Px(TARGET_WIDTH as f32);
        layout.size.height = UiLength::Px(TARGET_HEIGHT as f32);
    }
    app.world_mut().entity_mut(icon).insert((
        UiLayoutRoot::new(
            layout_window,
            Rect::new(0.0, 0.0, TARGET_WIDTH as f32, TARGET_HEIGHT as f32),
            1.0,
        ),
        UiPaint(Primitive::RoundedRect {
            rect: bui_render::ClipRect {
                x: 0.0,
                y: 0.0,
                width: TARGET_WIDTH as f32,
                height: TARGET_HEIGHT as f32,
            },
            radius: 0.0,
            color: Theme::default().background,
        }),
    ));

    // A missing file must be a visible error state. Creating it and explicitly retrying should
    // then replace the error overlay with a ready image without rebuilding the widget tree.
    await_failed_state(&mut app, icon);
    assert!(matches!(
        app.world()
            .get::<IconButtonAssetState>(icon)
            .unwrap()
            .status,
        IconButtonAssetStatus::Failed(_)
    ));
    let failed_overlays = app.world().resource::<DisplayList>().overlay_widget_items();
    assert_eq!(
        failed_overlays
            .iter()
            .filter(|item| matches!(item.primitive, Primitive::Line { .. }))
            .count(),
        2,
        "a failed icon has a visible error mark"
    );

    fs::write(directory.join("recovery.svg"), CHECKER_SVG).unwrap();
    app.world_mut()
        .get_mut::<IconButtonAsset>(icon)
        .unwrap()
        .retry();
    await_state(&mut app, icon, IconButtonAssetStatus::Ready);
    let icon_asset_id = app
        .world()
        .resource::<IconButtonAssetCache>()
        .asset_id(icon)
        .expect("ready icon should have a renderer registry ID");
    let retained_image = app
        .world()
        .resource::<ImageRegistry>()
        .get(icon_asset_id)
        .expect("ready icon should resolve through ImageRegistry")
        .clone();
    let retained_image_id = retained_image.id();

    for (frame, (kind, dpi_scale)) in [
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
            .get_mut::<UiLayoutRoot>(icon)
            .unwrap()
            .dpi_scale = dpi_scale;
        for _ in 0..3 {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }
        let layout = *app.world().get::<UiLayoutResult>(icon).unwrap();
        assert_eq!(layout.dpi_scale, dpi_scale);
        let overlay = app
            .world()
            .resource::<DisplayList>()
            .overlay_widget_items()
            .to_vec();
        let icon_draw = overlay
            .iter()
            .find_map(|item| match &item.primitive {
                Primitive::Icon {
                    rect,
                    asset_id,
                    tint,
                } => Some((*rect, *asset_id, *tint)),
                _ => None,
            })
            .expect("screen-root icon should use the widget overlay layer");
        assert_eq!(icon_draw.1, icon_asset_id);
        assert_eq!(icon_draw.2, theme.color(ThemeColorToken::Icon));
        assert!(
            app.world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .any(|item| matches!(item.primitive, Primitive::RoundedRect { .. })),
            "adding the icon overlay must preserve the base widget/manual layer"
        );

        let pixels = expected_readback(&mut app, &target, frame);
        // The SVG's center is #e38a45; the icon uses ThemeColorToken::Icon as its sRGB tint.
        let expected = expected_tinted_pixel([0xe3, 0x8a, 0x45], theme.text);
        let center = [
            (layout.rect.x + layout.rect.width * 0.5).round() as usize,
            (layout.rect.y + layout.rect.height * 0.5).round() as usize,
        ];
        assert_pixel_near(
            pixel_at(&pixels, center[0], center[1]),
            expected,
            "tinted SVG icon center",
        );
        assert!(icon_draw.0.width > 0.0 && icon_draw.0.height > 0.0);
    }

    app.world_mut()
        .entity_mut(icon)
        .insert(bui_core::UiVisible(false));
    let hidden_deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<IconButtonAssetCache>()
        .visible_count()
        != 0
    {
        app.update();
        assert!(
            Instant::now() < hidden_deadline,
            "hidden icon was not released"
        );
    }
    assert!(
        app.world()
            .resource::<ImageRegistry>()
            .get(icon_asset_id)
            .is_none()
    );
    assert!(matches!(
        app.world()
            .get::<IconButtonAssetState>(icon)
            .unwrap()
            .status,
        IconButtonAssetStatus::Hidden
    ));
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(retained_image_id)
            .is_some()
    );
    drop(retained_image);
    let release_deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<Assets<Image>>()
        .get(retained_image_id)
        .is_some()
    {
        app.update();
        assert!(
            Instant::now() < release_deadline,
            "icon image asset stayed retained"
        );
        thread::sleep(Duration::from_millis(5));
    }
    fs::remove_dir_all(directory).unwrap();
}

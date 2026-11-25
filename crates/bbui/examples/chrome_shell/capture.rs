//! Opt-in dark and light desktop and compact captures from Bevy's offscreen renderer.

use super::*;
use bbui_style::ThemeKind;
use bevy::{
    app::PluginGroup,
    camera::{ClearColor, RenderTarget},
    image::Image,
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    ui::IsDefaultUiCamera,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Resource, Default)]
struct CapturedFrame(Option<Vec<u8>>);

#[allow(clippy::expect_used)]
fn read_frame(app: &mut App, target: &Handle<Image>) -> Vec<u8> {
    let readback = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(
            |completed: bevy::ecs::observer::On<ReadbackComplete>,
             mut frame: ResMut<CapturedFrame>| {
                frame.0 = Some(completed.data.clone());
            },
        )
        .id();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && app.world().resource::<CapturedFrame>().0.is_none() {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let bytes = app
        .world()
        .resource::<CapturedFrame>()
        .0
        .clone()
        .expect("chrome GPU capture timed out");
    app.world_mut().despawn(readback);
    bytes
}

#[allow(clippy::expect_used)]
fn save_png(path: &std::path::Path, padded: &[u8], width: u32, height: u32) {
    let row_stride = (width as usize * 4).div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        pixels.extend_from_slice(&padded[y * row_stride..y * row_stride + width as usize * 4]);
    }
    image::save_buffer_with_format(
        path,
        &pixels,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("save chrome capture");
}

#[allow(clippy::expect_used)]
fn capture(kind: ThemeKind, mode: &str, width: u32, height: u32) {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    force_fallback_adapter: true,
                    ..default()
                })),
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(BbuiPlugin)
    .init_resource::<CapturedFrame>();
    app.finish();
    app.cleanup();
    *app.world_mut().resource_mut::<Theme>() = Theme::new(kind);
    app.world_mut()
        .insert_resource(ClearColor(if kind == ThemeKind::Dark {
            Color::srgb(0.12, 0.12, 0.12)
        } else {
            Color::srgb(0.86, 0.86, 0.86)
        }));
    let mut target = Image::new_target_texture(width, height, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Image(target.clone().into()),
        IsDefaultUiCamera,
        Window {
            resolution: WindowResolution::new(width, height),
            ..default()
        },
        PrimaryWindow,
    ));
    install_chrome(app.world_mut());
    for _ in 0..5 {
        app.update();
    }
    let bytes = read_frame(&mut app, &target);
    let output =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/visuals/examples");
    std::fs::create_dir_all(&output).expect("create gallery directory");
    let theme = if kind == ThemeKind::Dark {
        "dark"
    } else {
        "light"
    };
    save_png(
        &output.join(format!("chrome-{mode}-{theme}.png")),
        &bytes,
        width,
        height,
    );
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/chrome-{desktop,mobile}-{dark,light}.png"]
fn capture_chrome_dark_and_light() {
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        capture(kind, "desktop", 1536, 900);
        capture(kind, "mobile", 390, 844);
    }
}

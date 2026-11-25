//! Opt-in offscreen captures for the README preview.

use super::*;
use bevy::{
    app::PluginGroup,
    camera::RenderTarget,
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
use std::{collections::HashMap, thread, time::Instant};

#[derive(Resource, Default)]
struct Frames(HashMap<usize, Vec<u8>>);

#[allow(clippy::expect_used)]
fn read_frame(app: &mut App, target: &Handle<Image>, index: usize) -> Vec<u8> {
    let readback = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(
            move |completed: bevy::ecs::observer::On<ReadbackComplete>,
                  mut frames: ResMut<Frames>| {
                frames.0.insert(index, completed.data.clone());
            },
        )
        .id();
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline && !app.world().resource::<Frames>().0.contains_key(&index) {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let bytes = app
        .world()
        .resource::<Frames>()
        .0
        .get(&index)
        .cloned()
        .expect("progress gallery GPU capture timed out");
    app.world_mut().despawn(readback);
    bytes
}

#[allow(clippy::expect_used)]
fn save_png(path: &std::path::Path, padded: &[u8]) {
    let width = WIDTH as usize;
    let height = HEIGHT as usize;
    let row_stride = (width * 4).div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        pixels.extend_from_slice(&padded[y * row_stride..y * row_stride + width * 4]);
    }
    image::save_buffer_with_format(
        path,
        &pixels,
        WIDTH as u32,
        HEIGHT as u32,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("save progress gallery capture");
}

fn new_app() -> (App, Handle<Image>) {
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
    .init_resource::<Frames>();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::ZERO,
    ));
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(
        WIDTH as u32,
        HEIGHT as u32,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let window = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            IsDefaultUiCamera,
            Window {
                resolution: WindowResolution::new(WIDTH as u32, HEIGHT as u32),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    install_ui(app.world_mut(), window);
    (app, target)
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/progress-gallery.png"]
#[allow(clippy::expect_used)]
fn capture_progress_gallery() {
    let (mut app, target) = new_app();
    for _ in 0..8 {
        app.update();
    }
    for (key, binding) in [
        ("segmented-bar", progress_value_binding().id()),
        ("luminous-bar", progress_glow_strength_binding().id()),
    ] {
        if let Some(bar) = entity(app.world(), key) {
            app.world_mut()
                .resource_mut::<MotionPlayer>()
                .cancel_binding(bar, binding);
        }
    }
    for (key, value) in [("segmented-bar", 0.56), ("indeterminate-bar", 0.25)] {
        if let Some(bar) = entity(app.world(), key) {
            let mut progress = app.world_mut().get_mut::<Progress>(bar).expect("bar");
            if progress.indeterminate {
                progress.phase = value;
            } else {
                progress.value = value;
            }
        }
    }
    if let Some(bar) = entity(app.world(), "luminous-bar") {
        app.world_mut()
            .get_mut::<Progress>(bar)
            .expect("luminous bar")
            .style
            .fill_glow
            .as_mut()
            .expect("glow")
            .strength = 0.8;
    }
    for _ in 0..4 {
        app.update();
    }
    let output =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/visuals/examples");
    std::fs::create_dir_all(&output).expect("create progress gallery directory");
    save_png(
        &output.join("progress-gallery.png"),
        &read_frame(&mut app, &target, 0),
    );

    let Some(frames_dir) = std::env::var_os("BBUI_PROGRESS_CAPTURE_DIR") else {
        return;
    };
    let frames_dir = std::path::PathBuf::from(frames_dir);
    std::fs::create_dir_all(&frames_dir).expect("create progress frame directory");
    let keys = [
        "gradient-bar",
        "segmented-bar",
        "luminous-bar",
        "indeterminate-bar",
    ];
    let entities: Vec<Entity> = keys
        .iter()
        .map(|key| entity(app.world(), key).expect("progress bar entity"))
        .collect();
    for (entity, binding) in [
        (entities[0], progress_palette_phase_binding().id()),
        (entities[1], progress_value_binding().id()),
        (entities[2], progress_border_phase_binding().id()),
        (entities[2], progress_glow_phase_binding().id()),
        (entities[2], progress_glow_strength_binding().id()),
    ] {
        app.world_mut()
            .resource_mut::<MotionPlayer>()
            .cancel_binding(entity, binding);
    }
    for frame in 0..120 {
        let phase = frame as f32 / 120.0;
        let pulse = 0.35 + 0.65 * (1.0 - (std::f32::consts::TAU * phase).cos()) * 0.5;
        app.world_mut()
            .get_mut::<Progress>(entities[0])
            .expect("gradient")
            .palette_phase = phase;
        let triangle = 1.0 - (2.0 * phase - 1.0).abs();
        app.world_mut()
            .get_mut::<Progress>(entities[1])
            .expect("segmented")
            .value = 0.15 + 0.8 * triangle;
        {
            let mut luminous = app
                .world_mut()
                .get_mut::<Progress>(entities[2])
                .expect("luminous");
            luminous.style.fill_border.as_mut().expect("border").phase = phase;
            let glow = luminous.style.fill_glow.as_mut().expect("glow");
            glow.phase = phase;
            glow.strength = pulse;
        }
        app.world_mut()
            .get_mut::<Progress>(entities[3])
            .expect("indeterminate")
            .phase = phase;
        app.update();
        app.update();
        save_png(
            &frames_dir.join(format!("{frame:03}.png")),
            &read_frame(&mut app, &target, frame + 1),
        );
        app.world_mut()
            .resource_mut::<Frames>()
            .0
            .remove(&(frame + 1));
    }
}

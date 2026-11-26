//! Opt-in offscreen capture for the toast gradient gallery.

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
    time::TimeUpdateStrategy,
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
        .expect("gradient toast GPU capture timed out");
    app.world_mut().despawn(readback);
    app.world_mut().resource_mut::<Frames>().0.remove(&index);
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
        WIDTH,
        HEIGHT,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("save gradient toast capture");
}

#[allow(clippy::expect_used)]
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
    .add_systems(PostUpdate, process)
    .init_resource::<Frames>();
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let window = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            IsDefaultUiCamera,
            Window {
                resolution: WindowResolution::new(WIDTH, HEIGHT),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    install(app.world_mut(), window);
    (app, target)
}

#[test]
#[ignore = "run explicitly to write the toast gradient gallery media"]
#[allow(clippy::expect_used)]
fn capture_toast_gradient_borders() {
    let (mut app, target) = new_app();
    app.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    for _ in 0..8 {
        app.update();
    }
    let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/visuals/examples/toast-gradient-borders-dark.png");
    std::fs::create_dir_all(output.parent().expect("gallery directory"))
        .expect("create gallery directory");
    save_png(&output, &read_frame(&mut app, &target, 0));

    let Some(frames_dir) = std::env::var_os("BBUI_NOTICE_GRADIENT_FRAMES_DIR") else {
        return;
    };
    let frames_dir = std::path::PathBuf::from(frames_dir);
    std::fs::create_dir_all(&frames_dir).expect("create animation frame directory");
    let cards: Vec<_> = app
        .world()
        .iter_entities()
        .filter_map(|entry| {
            let key = entry.get::<UiKey>()?.0.as_str();
            (key.starts_with("toast:") && key[7..].chars().all(|c| c.is_ascii_digit()))
                .then_some(entry.id())
        })
        .collect();
    assert_eq!(cards.len(), 4);
    let initial_phases: Vec<_> = cards
        .iter()
        .map(|&entity| {
            app.world()
                .get::<UiVisualEffects>(entity)
                .and_then(|effects| effects.border.as_ref())
                .expect("card gradient border")
                .phase
        })
        .collect();
    let mut middle_phases = Vec::new();
    for frame in 0..120 {
        app.world_mut()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
                16_666_667,
            )));
        app.update();
        if frame == 59 {
            middle_phases = cards
                .iter()
                .map(|&entity| {
                    app.world()
                        .get::<UiVisualEffects>(entity)
                        .and_then(|effects| effects.border.as_ref())
                        .expect("animated card border")
                        .phase
                })
                .collect();
        }
        app.world_mut()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        save_png(
            &frames_dir.join(format!("{frame:03}.png")),
            &read_frame(&mut app, &target, frame + 1),
        );
    }
    for (initial, middle) in initial_phases.into_iter().zip(middle_phases) {
        assert!(middle > initial + 0.2, "border phase must advance");
    }
}

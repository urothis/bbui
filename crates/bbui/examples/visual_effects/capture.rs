//! Opt-in captures from Bevy's offscreen renderer.

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
        .expect("visual effects GPU capture timed out");
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
    .expect("save visual effects capture");
}

fn new_app(install: fn(&mut World, Entity)) -> (App, Handle<Image>) {
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
    install(app.world_mut(), window);
    (app, target)
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/visual-effects-*.png"]
#[allow(clippy::expect_used)]
fn capture_visual_effects() {
    let (mut app, target) = new_app(install_ui);

    let output =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/visuals/examples");
    std::fs::create_dir_all(&output).expect("create visual gallery directory");
    for _ in 0..8 {
        app.update();
    }
    save_png(
        &output.join("visual-effects-dark.png"),
        &read_frame(&mut app, &target, 0),
    );

    let mut border_entities = Vec::new();
    for key in ["border-card", "combined-card"] {
        let entity = app.world().iter_entities().find_map(|entry| {
            entry
                .get::<UiKey>()
                .is_some_and(|value| value.0 == key)
                .then_some(entry.id())
        });
        if let Some(entity) = entity {
            border_entities.push(entity);
            app.world_mut()
                .resource_mut::<MotionPlayer>()
                .cancel_binding(entity, border_phase_binding().id());
        }
    }
    for &entity in &border_entities {
        let mut effects = app
            .world_mut()
            .get_mut::<UiVisualEffects>(entity)
            .expect("effects");
        effects.border.as_mut().expect("border").phase = 0.33;
    }
    for _ in 0..4 {
        app.update();
    }
    save_png(
        &output.join("visual-effects-rotated.png"),
        &read_frame(&mut app, &target, 1),
    );

    if let Some(frames) = std::env::var_os("BBUI_CAPTURE_MOTION_DIR") {
        let glow_entities: Vec<_> = ["glow-card", "combined-card"]
            .into_iter()
            .filter_map(|key| {
                app.world().iter_entities().find_map(|entry| {
                    entry
                        .get::<UiKey>()
                        .is_some_and(|value| value.0 == key)
                        .then_some(entry.id())
                })
            })
            .collect();
        for &entity in &glow_entities {
            app.world_mut()
                .resource_mut::<MotionPlayer>()
                .cancel_binding(entity, edge_glow_strength_binding().id());
        }
        let frames = std::path::PathBuf::from(frames);
        std::fs::create_dir_all(&frames).expect("create motion frame directory");
        for frame in 0..120 {
            let phase = frame as f32 / 120.0;
            let pulse = 0.55 + 0.45 * (1.0 - (std::f32::consts::TAU * phase).cos()) * 0.5;
            for &entity in &border_entities {
                let mut effects = app
                    .world_mut()
                    .get_mut::<UiVisualEffects>(entity)
                    .expect("effects");
                effects.border.as_mut().expect("border").phase = phase;
            }
            for &entity in &glow_entities {
                let mut effects = app
                    .world_mut()
                    .get_mut::<UiVisualEffects>(entity)
                    .expect("effects");
                effects.edge_glow.as_mut().expect("edge glow").strength = pulse;
            }
            app.update();
            save_png(
                &frames.join(format!("{frame:03}.png")),
                &read_frame(&mut app, &target, frame + 2),
            );
            app.world_mut()
                .resource_mut::<Frames>()
                .0
                .remove(&(frame + 2));
        }
    }
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/visual-effect-variants.png"]
#[allow(clippy::expect_used)]
fn capture_visual_effect_variants() {
    let (mut app, target) = new_app(variants::install);
    for _ in 0..8 {
        app.update();
    }
    let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/visuals/examples/visual-effect-variants.png");
    save_png(&output, &read_frame(&mut app, &target, 0));

    let Some(frames) = std::env::var_os("BBUI_CAPTURE_VARIANTS_DIR") else {
        return;
    };
    let frames = std::path::PathBuf::from(frames);
    std::fs::create_dir_all(&frames).expect("create variant motion frame directory");
    let entities: Vec<_> = [
        ("border-solid-animated", border_strength_binding().id()),
        ("border-gradient-animated", border_phase_binding().id()),
        ("glow-solid-animated", edge_glow_strength_binding().id()),
        ("glow-gradient-animated", edge_glow_phase_binding().id()),
    ]
    .into_iter()
    .map(|(key, binding)| {
        let entity = app
            .world()
            .iter_entities()
            .find_map(|entry| {
                entry
                    .get::<UiKey>()
                    .is_some_and(|value| value.0 == key)
                    .then_some(entry.id())
            })
            .expect("variant card");
        app.world_mut()
            .resource_mut::<MotionPlayer>()
            .cancel_binding(entity, binding);
        entity
    })
    .collect();
    for frame in 0..120 {
        let phase = frame as f32 / 120.0;
        let pulse = 0.35 + 0.65 * (1.0 - (std::f32::consts::TAU * phase).cos()) * 0.5;
        for (index, &entity) in entities.iter().enumerate() {
            let mut effects = app
                .world_mut()
                .get_mut::<UiVisualEffects>(entity)
                .expect("variant effects");
            match index {
                0 => effects.border.as_mut().expect("border").strength = pulse,
                1 => effects.border.as_mut().expect("border").phase = phase,
                2 => effects.edge_glow.as_mut().expect("glow").strength = pulse,
                _ => effects.edge_glow.as_mut().expect("glow").phase = phase,
            }
        }
        app.update();
        save_png(
            &frames.join(format!("{frame:03}.png")),
            &read_frame(&mut app, &target, frame + 1),
        );
        app.world_mut()
            .resource_mut::<Frames>()
            .0
            .remove(&(frame + 1));
    }
}

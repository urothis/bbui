//! Offscreen captures from the runnable docked scene example.

use super::*;
use bbui_style::{Theme, ThemeKind};
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
    window::ExitCondition,
    winit::WinitPlugin,
};
use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

const WIDTH: u32 = 1200;
const HEIGHT: u32 = 760;

#[derive(Resource, Default)]
struct Frames(HashMap<usize, Vec<u8>>);

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
    let deadline = Instant::now() + Duration::from_secs(25);
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
        .expect("GPU capture timed out");
    app.world_mut().despawn(readback);
    bytes
}

fn save_png(path: &std::path::Path, padded: &[u8]) {
    let row_stride = (WIDTH as usize * 4).div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
    for y in 0..HEIGHT as usize {
        pixels.extend_from_slice(&padded[y * row_stride..y * row_stride + WIDTH as usize * 4]);
    }
    image::save_buffer_with_format(
        path,
        &pixels,
        WIDTH,
        HEIGHT,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("save docked scene capture");
}

#[test]
#[ignore = "writes docked-scene dark, light, and overlay gallery PNGs"]
fn capture_docked_scene_dark_and_light() {
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
            .disable::<WinitPlugin>(),
    )
    .add_plugins(BbuiPlugin)
    .init_resource::<SceneOrbit>()
    .init_resource::<Frames>()
    .add_systems(Startup, (setup, install_workspace).chain())
    .add_systems(Update, sync_scene_activity.after(UiSystems::Widgets));
    app.finish();
    app.cleanup();
    let mut target = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..default()
        },
        PrimaryWindow,
    ));
    app.update();
    let mut cameras = app.world_mut().query_filtered::<Entity, With<Camera2d>>();
    let camera_ids: Vec<_> = cameras.iter(app.world()).collect();
    for camera in camera_ids {
        app.world_mut()
            .entity_mut(camera)
            .insert(RenderTarget::Image(target.clone().into()));
        app.world_mut().entity_mut(camera).insert(IsDefaultUiCamera);
    }
    let output =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/visuals/examples");
    std::fs::create_dir_all(&output).expect("create gallery directory");
    for (index, kind) in [ThemeKind::Dark, ThemeKind::Light].into_iter().enumerate() {
        *app.world_mut().resource_mut::<Theme>() = Theme::new(kind);
        for _ in 0..8 {
            app.update();
        }
        let mut cameras = app
            .world_mut()
            .query_filtered::<&Camera, With<DockedSceneCamera>>();
        let camera = cameras.single(app.world()).expect("scene camera");
        assert!(
            camera.is_active && camera.viewport.is_none(),
            "scene pane did not activate its camera"
        );
        let mut scene_cameras = app
            .world_mut()
            .query_filtered::<Entity, With<DockedSceneCamera>>();
        let scene_camera = scene_cameras
            .single(app.world())
            .expect("scene camera entity");
        let scene_image = app
            .world()
            .get::<RenderTarget>(scene_camera)
            .and_then(RenderTarget::as_image)
            .expect("scene image target");
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(scene_image)
            .expect("viewport image asset");
        let image_size = image.texture_descriptor.size;
        let mut surfaces = app.world_mut().query::<(&UiKey, &UiLayoutResult)>();
        let pane = surfaces
            .iter(app.world())
            .find_map(|(key, layout)| (key.0 == SCENE_KEY).then_some(layout.rect))
            .expect("scene pane layout");
        assert!(image_size.width.abs_diff(pane.width as u32) <= 2);
        assert!(image_size.height.abs_diff(pane.height as u32) <= 2);
        let composed = read_frame(&mut app, &target, index);
        let stride = (WIDTH as usize * 4).div_ceil(256) * 256;
        let visible: std::collections::HashSet<_> = (140..680)
            .step_by(8)
            .flat_map(|y| (80..800).step_by(8).map(move |x| (y, x)))
            .map(|(y, x)| {
                let offset = y * stride + x * 4;
                [composed[offset], composed[offset + 1], composed[offset + 2]]
            })
            .collect();
        assert!(
            visible.len() > 32,
            "3D scene is hidden behind workspace chrome"
        );
        save_png(
            &output.join(format!(
                "docked-scene-{}.png",
                if kind == ThemeKind::Dark {
                    "dark"
                } else {
                    "light"
                }
            )),
            &composed,
        );
    }
    *app.world_mut().resource_mut::<Theme>() = Theme::new(ThemeKind::Dark);
    let mut windows = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>();
    let window = windows.single(app.world()).expect("capture window");
    ModalDialog::new(
        "Scene settings",
        "This dialog remains above the live 3D viewport.",
        vec![ModalAction::new("close", "Done")],
    )
    .show(app.world_mut(), window)
    .expect("open scene overlay");
    for _ in 0..8 {
        app.update();
    }
    save_png(
        &output.join("docked-scene-overlay.png"),
        &read_frame(&mut app, &target, 2),
    );
}

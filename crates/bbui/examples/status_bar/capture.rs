//! Opt-in dark and light captures of the status bar from Bevy's offscreen renderer.

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
    window::{ExitCondition, WindowPlugin, WindowResolution},
    winit::WinitPlugin,
};
use std::{
    collections::HashMap,
    thread,
    time::{Duration, Instant},
};

const WIDTH: u32 = 1536;
const HEIGHT: u32 = 900;

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
    let deadline = Instant::now() + Duration::from_secs(20);
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
        .expect("status bar GPU capture timed out");
    app.world_mut().despawn(readback);
    bytes
}

#[allow(clippy::expect_used)]
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
    .expect("save status bar capture");
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/status-bar-*.png"]
#[allow(clippy::expect_used)]
fn capture_status_bar() {
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
    let mut target = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.world_mut().spawn((
        Camera2d,
        RenderTarget::Image(target.clone().into()),
        IsDefaultUiCamera,
        Window {
            resolution: WindowResolution::new(WIDTH, HEIGHT),
            ..default()
        },
        PrimaryWindow,
    ));
    install(app.world_mut());
    let output =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/visuals/examples");
    std::fs::create_dir_all(&output).expect("create gallery directory");

    for (theme_index, kind) in [ThemeKind::Dark, ThemeKind::Light].into_iter().enumerate() {
        *app.world_mut().resource_mut::<Theme>() = Theme::new(kind);
        editor_content::configure_theme(&mut app.world_mut().resource_mut::<Theme>());
        app.world_mut()
            .insert_resource(ClearColor(if kind == ThemeKind::Dark {
                Color::srgb(0.12, 0.12, 0.12)
            } else {
                Color::srgb(0.86, 0.86, 0.86)
            }));
        for (scenario_index, scenario) in ["default", "compact", "hover"].into_iter().enumerate() {
            let status_root = app
                .world()
                .iter_entities()
                .find_map(|entry| {
                    entry
                        .get::<UiKey>()
                        .is_some_and(|key| key.0 == "status:bar")
                        .then_some(entry.id())
                })
                .expect("status bar mounted");
            let appearance = StatusBarAppearance::default();
            WidgetTree::new(
                StatusBar.view_with_slots(&appearance, status_slots(scenario == "compact")),
            )
            .reconcile(app.world_mut(), status_root)
            .expect("reconcile status scenario");
            let cancel = app.world().iter_entities().find_map(|entry| {
                entry
                    .get::<UiKey>()
                    .is_some_and(|key| key.0 == "status:cancel")
                    .then_some(entry.id())
            });
            if let Some(cancel) = cancel {
                let mut button = app
                    .world_mut()
                    .get_mut::<bbui::widgets::Button>(cancel)
                    .expect("cancel button");
                button.status = if scenario == "hover" {
                    bbui::widgets::WidgetStatus::Hovered
                } else {
                    bbui::widgets::WidgetStatus::Idle
                };
            }
            for _ in 0..5 {
                app.update();
            }
            if scenario == "default" && kind == ThemeKind::Dark {
                let label = app
                    .world()
                    .iter_entities()
                    .find_map(|entry| {
                        (entry.get::<UiKey>()?.0 == "host:progress-label")
                            .then(|| {
                                entry
                                    .get::<bbui_layout::UiLayoutResult>()
                                    .map(|layout| layout.rect)
                            })
                            .flatten()
                    })
                    .expect("center label layout");
                assert!((label.x + label.width / 2.0 - WIDTH as f32 / 2.0).abs() < 1.0);
            }
            let bytes = read_frame(&mut app, &target, theme_index * 3 + scenario_index);
            let theme = if kind == ThemeKind::Dark {
                "dark"
            } else {
                "light"
            };
            let name = if scenario == "default" {
                format!("status-bar-{theme}.png")
            } else {
                format!("status-bar-{scenario}-{theme}.png")
            };
            save_png(&output.join(name), &bytes);
        }
    }
}

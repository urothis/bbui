//! Opt-in dark and light captures of the command palette from Bevy's offscreen renderer.

use super::*;
use bbui_style::ThemeKind;
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
        .expect("command palette GPU capture timed out");
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
    .expect("save command palette capture");
}

#[test]
#[ignore = "run explicitly to write command palette gallery captures"]
#[allow(clippy::expect_used)]
fn capture_command_palette_dark_and_light() {
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
    install_editor(app.world_mut());
    for _ in 0..3 {
        app.update();
    }
    install_palette(app.world_mut());
    let output =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/visuals/examples");
    std::fs::create_dir_all(&output).expect("create gallery directory");
    for (theme_index, kind) in [ThemeKind::Dark, ThemeKind::Light].into_iter().enumerate() {
        *app.world_mut().resource_mut::<Theme>() = Theme::new(kind);
        editor_content::configure_theme(&mut app.world_mut().resource_mut::<Theme>());
        let theme_name = if kind == ThemeKind::Dark {
            "dark"
        } else {
            "light"
        };
        for (scenario_index, scenario) in ["default", "empty", "long", "compact", "keyboard"]
            .into_iter()
            .enumerate()
        {
            let mut state = app
                .world_mut()
                .remove_resource::<ExampleState>()
                .expect("palette state");
            if scenario == "compact" {
                state.registry.unregister("long");
            }
            state.palette.query = match scenario {
                "empty" => "no matching command".into(),
                "long" => "very long".into(),
                "keyboard" => String::new(),
                _ => "material".into(),
            };
            state.palette.selected = if scenario == "keyboard" { 7 } else { 0 };
            if scenario == "long"
                && !state
                    .registry
                    .commands()
                    .iter()
                    .any(|command| command.id == "long")
            {
                let mut command = Command::new(
                    "long",
                    "Very Long Material Processing Command That Must Be Truncated Before It Reaches The Shortcut",
                );
                command.category = "Material".into();
                command.description =
                    "Create a multi-stage material from the selected texture inputs".into();
                command.icon = Some("◆".into());
                command.shortcut_hint = Some("Ctrl+Shift+M".into());
                state
                    .registry
                    .register(command)
                    .expect("register long command");
            }
            let width = if scenario == "compact" {
                410.0
            } else {
                WIDTH as f32
            };
            palette_page(&state.palette, &state.registry, width, HEIGHT as f32)
                .reconcile(app.world_mut(), state.root)
                .expect("reconcile palette scenario");
            if scenario == "keyboard" {
                state.palette.scroll_selected_into_view(
                    app.world_mut(),
                    state.root,
                    &CommandPaletteAppearance::default(),
                );
            }
            app.world_mut().insert_resource(state);
            for _ in 0..5 {
                app.update();
            }
            let index = theme_index * 5 + scenario_index;
            let bytes = read_frame(&mut app, &target, index);
            let name = if scenario == "default" {
                format!("command-palette-{theme_name}.png")
            } else {
                format!("command-palette-{scenario}-{theme_name}.png")
            };
            save_png(&output.join(name), &bytes);
        }
    }
}

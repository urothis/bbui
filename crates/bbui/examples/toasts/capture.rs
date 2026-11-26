//! Opt-in dark and light captures of the toasts from Bevy's offscreen renderer.

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
        .expect("toasts GPU capture timed out");
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
    .expect("save toasts capture");
}

#[allow(clippy::expect_used)]
fn save_motion_crop(path: &std::path::Path, padded: &[u8]) {
    const LEFT: usize = 1228;
    const TOP: usize = 700;
    const CROP_WIDTH: usize = 302;
    const CROP_HEIGHT: usize = 160;
    let row_stride = (WIDTH as usize * 4).div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(CROP_WIDTH * CROP_HEIGHT * 4);
    for y in TOP..TOP + CROP_HEIGHT {
        let start = y * row_stride + LEFT * 4;
        pixels.extend_from_slice(&padded[start..start + CROP_WIDTH * 4]);
    }
    image::save_buffer_with_format(
        path,
        &pixels,
        CROP_WIDTH as u32,
        CROP_HEIGHT as u32,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("save toast motion crop");
}

#[allow(clippy::expect_used)]
fn capture_motion_frames(app: &mut App, target: &Handle<Image>, output: &std::path::Path) {
    app.world_mut()
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::ZERO,
        ));
    std::fs::create_dir_all(output).expect("create toast motion frames directory");
    let window = app.world().resource::<ExampleState>().window;
    let toast_root = {
        let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
        keys.iter(app.world())
            .find_map(|(entity, key)| (key.0 == "toast:center").then_some(entity))
            .expect("toast stack mounted")
    };
    let mut center = ToastCenter::default();
    center.set_motion(ToastMotion {
        enter_travel_curve: MotionCurve::cubic_bezier(0.16, 0.95, 0.3, 1.0)
            .unwrap_or(MotionCurve::EaseOut),
        exit_opacity_curve: MotionCurve::EaseInOut,
        ..ToastMotion::default()
    });
    for _ in 0..3 {
        center.post(
            window,
            ToastSeverity::Info,
            None,
            "",
            ToastLifetime::For(Duration::ZERO),
        );
    }
    center.advance(Duration::from_secs(1));
    let id = center.post(
        window,
        ToastSeverity::Success,
        Some("Render complete".into()),
        "Preview is ready to inspect",
        ToastLifetime::Persistent,
    );
    WidgetTree::new(center.floating_stack(
        window,
        &ToastAppearance::default(),
        &ToastOverlayAppearance::default(),
    ))
    .reconcile(app.world_mut(), toast_root)
    .expect("mount incoming toast");
    app.world_mut().insert_resource(center);
    app.update();
    let mut center = app
        .world_mut()
        .remove_resource::<ToastCenter>()
        .expect("toast center");
    WidgetTree::new(center.floating_stack(
        window,
        &ToastAppearance::default(),
        &ToastOverlayAppearance::default(),
    ))
    .reconcile(app.world_mut(), toast_root)
    .expect("start entrance transition");
    app.world_mut().insert_resource(center);
    app.update();
    for frame in 0..120 {
        if frame == 106 {
            let mut center = app
                .world_mut()
                .remove_resource::<ToastCenter>()
                .expect("toast center");
            center.acknowledge(id).expect("dismiss animated toast");
            WidgetTree::new(center.floating_stack(
                window,
                &ToastAppearance::default(),
                &ToastOverlayAppearance::default(),
            ))
            .reconcile(app.world_mut(), toast_root)
            .expect("start exit transition");
            app.world_mut().insert_resource(center);
        }
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_nanos(16_666_667),
            ));
        app.update();
        if matches!(frame, 0 | 10 | 20 | 105 | 119) {
            let mut layouts = app
                .world_mut()
                .query::<(&UiKey, &bbui_layout::UiLayoutResult)>();
            let mut card = None;
            let mut message = None;
            for (key, layout) in layouts.iter(app.world()) {
                if key.0 == format!("toast:{id}") {
                    card = Some(layout.rect);
                } else if key.0 == format!("toast:message:{id}") {
                    message = Some(layout.rect);
                }
            }
            let card = card.expect("animated card layout");
            let message = message.expect("animated message layout");
            assert!(
                (card.height - 110.0).abs() < 0.01,
                "card height must stay fixed: {card:?}"
            );
            assert!(
                (message.y - card.y - 75.0).abs() < 0.01,
                "message must follow card: {card:?} {message:?}"
            );
        }
        app.world_mut()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::ZERO,
            ));
        save_motion_crop(
            &output.join(format!("{frame:03}.png")),
            &read_frame(app, target, 100 + frame),
        );
        app.world_mut()
            .resource_mut::<Frames>()
            .0
            .remove(&(100 + frame));
    }
    app.world_mut()
        .insert_resource(bevy::time::TimeUpdateStrategy::Automatic);
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/toasts-{dark,light}.png"]
#[allow(clippy::expect_used)]
fn capture_toasts_dark_and_light() {
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
    install_toasts(app.world_mut());
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
        let name = if kind == ThemeKind::Dark {
            "dark"
        } else {
            "light"
        };
        for (scenario_index, scenario) in [
            "default",
            "long",
            "empty",
            "hover",
            "dismiss-copy",
            "enter",
            "exit",
        ]
        .into_iter()
        .enumerate()
        {
            let mut state = app
                .world_mut()
                .remove_resource::<ExampleState>()
                .expect("example state");
            let mut center = ToastCenter::default();
            if scenario == "enter" {
                for _ in 0..3 {
                    center.post(
                        state.window,
                        ToastSeverity::Info,
                        None,
                        "",
                        ToastLifetime::For(Duration::ZERO),
                    );
                }
                center.advance(Duration::from_secs(1));
                center.set_motion(ToastMotion {
                    enter: Duration::from_secs(2),
                    exit: Duration::from_secs(2),
                    travel: 18.0,
                    ..ToastMotion::default()
                });
            } else {
                center.set_motion(ToastMotion {
                    enter: Duration::ZERO,
                    ..ToastMotion::default()
                });
            }
            if scenario != "empty" {
                center.post(
                    state.window,
                    ToastSeverity::Info,
                    Some("Workspace".into()),
                    "Project loaded",
                    ToastLifetime::Persistent,
                );
                center.post(
                    state.window,
                    ToastSeverity::Success,
                    Some("Export".into()),
                    "Preview ready",
                    ToastLifetime::Persistent,
                );
                if scenario == "long" {
                    center.post(state.window, ToastSeverity::Error, Some("Export failed".into()), "The output directory is unavailable. Choose another location and try exporting again.", ToastLifetime::Persistent);
                } else {
                    center.post(
                        state.window,
                        ToastSeverity::Warning,
                        Some("Storage".into()),
                        "Low disk space",
                        ToastLifetime::Persistent,
                    );
                }
            }
            if scenario != "enter" {
                center.advance(Duration::from_millis(200));
            }
            let toast_root = {
                let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                keys.iter(app.world())
                    .find_map(|(entity, key)| (key.0 == "toast:center").then_some(entity))
                    .expect("toast stack mounted")
            };
            let toast_appearance = if scenario == "dismiss-copy" {
                ToastAppearance {
                    dismiss_label: "Close".into(),
                    dismiss_width: 58.0,
                    dismiss_height: 26.0,
                    ..ToastAppearance::default()
                }
            } else {
                ToastAppearance::default()
            };
            WidgetTree::new(center.floating_stack(
                state.window,
                &toast_appearance,
                &ToastOverlayAppearance::default(),
            ))
            .reconcile(app.world_mut(), toast_root)
            .expect("reconcile toast scenario");
            if matches!(scenario, "enter" | "exit") {
                for _ in 0..2 {
                    app.update();
                }
                if scenario == "enter" {
                    center.advance(Duration::from_millis(1));
                } else {
                    center.set_motion(ToastMotion {
                        enter: Duration::from_secs(2),
                        exit: Duration::from_millis(800),
                        travel: 18.0,
                        ..ToastMotion::default()
                    });
                    let id = center.visible(state.window)[0].id;
                    center.acknowledge(id);
                }
                WidgetTree::new(center.floating_stack(
                    state.window,
                    &toast_appearance,
                    &ToastOverlayAppearance::default(),
                ))
                .reconcile(app.world_mut(), toast_root)
                .expect("reconcile toast motion target");
                app.update();
                thread::sleep(Duration::from_millis(650));
            }
            state.generation = center.generation();
            app.world_mut().insert_resource(center);
            app.world_mut().insert_resource(state);
            let mut buttons = app.world_mut().query::<&mut bbui::widgets::Button>();
            for mut button in buttons.iter_mut(app.world_mut()) {
                button.status = bbui::widgets::WidgetStatus::Idle;
            }
            if scenario == "hover" {
                let entity = {
                    let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                    keys.iter(app.world())
                        .find_map(|(entity, key)| {
                            key.0.starts_with("toast:dismiss:").then_some(entity)
                        })
                        .expect("dismiss button")
                };
                app.world_mut()
                    .get_mut::<bbui::widgets::Button>(entity)
                    .expect("dismiss control")
                    .status = bbui::widgets::WidgetStatus::Hovered;
            }
            for _ in 0..5 {
                app.update();
            }
            if matches!(scenario, "enter" | "exit") && kind == ThemeKind::Dark {
                let target = if scenario == "enter" {
                    "toast:6"
                } else {
                    "toast:3"
                };
                let mut query = app.world_mut().query::<(
                    &UiKey,
                    &bbui_style::UiComputedStyle,
                    &bbui_style::UiInlineStyle,
                )>();
                let mut found = false;
                for (key, computed, inline) in query.iter(app.world()) {
                    if key.0 == target {
                        found = true;
                        let value = computed
                            .scalar(UiStyleProperty::TranslateY)
                            .expect("animated toast translation");
                        assert!(
                            (0.0..18.0).contains(&value),
                            "{} should be mid-transition: {}",
                            key.0,
                            value,
                        );
                        assert_eq!(
                            inline.0.get(&UiStyleProperty::TranslateY),
                            Some(&UiStyleValue::Scalar(if scenario == "enter" {
                                0.0
                            } else {
                                18.0
                            }))
                        );
                    }
                }
                assert!(found, "animated toast card should remain mounted");
            }
            if matches!(scenario, "default" | "empty") {
                let mut layouts = app
                    .world_mut()
                    .query::<(&UiKey, &bbui_layout::UiLayoutResult)>();
                let mut rect = |key: &str| {
                    layouts
                        .iter(app.world())
                        .find_map(|(node_key, layout)| (node_key.0 == key).then_some(layout.rect))
                        .expect("visible layout node")
                };
                assert_eq!(rect("toast:workspace").width, WIDTH as f32);
                let stack = rect("toast:center");
                if scenario == "default" {
                    assert_eq!(stack.width, 280.0);
                    assert!(stack.x >= WIDTH as f32 - stack.width - 20.0);
                } else {
                    assert_eq!(stack.width, 0.0);
                    assert_eq!(stack.height, 0.0);
                }
            }
            if scenario == "dismiss-copy" {
                let mut layouts = app
                    .world_mut()
                    .query::<(&UiKey, &bbui_layout::UiLayoutResult)>();
                let button = layouts
                    .iter(app.world())
                    .find_map(|(key, layout)| (key.0 == "toast:dismiss:3").then_some(layout.rect))
                    .expect("text dismiss control");
                assert_eq!(button.width, 58.0);
                assert_eq!(button.height, 26.0);
            }
            let bytes = read_frame(&mut app, &target, theme_index * 7 + scenario_index);
            if kind == ThemeKind::Dark && scenario == "default" {
                let mut layouts = app
                    .world_mut()
                    .query::<(&UiKey, &bbui_layout::UiLayoutResult)>();
                let card = layouts
                    .iter(app.world())
                    .find_map(|(key, layout)| (key.0 == "toast:3").then_some(layout.rect))
                    .expect("settled toast card");
                let left = card.x as usize;
                let right = (card.x + card.width) as usize - 1;
                let top = card.y as usize;
                let bottom = (card.y + card.height) as usize - 1;
                let middle_x = (left + right) / 2;
                let middle_y = (top + bottom) / 2;
                let row_stride = (WIDTH as usize * 4).div_ceil(256) * 256;
                let pixel = |x: usize, y: usize| {
                    let start = y * row_stride + x * 4;
                    &bytes[start..start + 3]
                };
                let border = pixel(left, middle_y);
                assert_eq!(border, pixel(right, middle_y), "right border clipped");
                assert_eq!(border, pixel(middle_x, top), "top border clipped");
                assert_eq!(border, pixel(middle_x, bottom), "bottom border clipped");
                assert_ne!(
                    border,
                    pixel(left + 2, middle_y),
                    "border blends into card fill"
                );
            }
            let filename = if scenario == "default" {
                format!("toasts-{name}.png")
            } else {
                format!("toasts-{scenario}-{name}.png")
            };
            save_png(&output.join(filename), &bytes);
        }
        if kind == ThemeKind::Dark
            && let Some(frames) = std::env::var_os("BBUI_TOASTS_CAPTURE_DIR")
        {
            capture_motion_frames(&mut app, &target, &std::path::PathBuf::from(frames));
        }
    }
}

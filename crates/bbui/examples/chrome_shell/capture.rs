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
fn save_controls_preview(path: &std::path::Path, padded: &[u8], width: u32) {
    let left = width.saturating_sub(320) as usize;
    let crop_width = width as usize - left;
    let crop_height = 48usize;
    let row_stride = (width as usize * 4).div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(crop_width * crop_height * 4);
    for y in 0..crop_height {
        let start = y * row_stride + left * 4;
        pixels.extend_from_slice(&padded[start..start + crop_width * 4]);
    }
    let crop = image::RgbaImage::from_raw(crop_width as u32, crop_height as u32, pixels)
        .expect("control crop dimensions");
    image::imageops::resize(
        &crop,
        (crop_width * 3) as u32,
        (crop_height * 3) as u32,
        image::imageops::FilterType::Lanczos3,
    )
    .save(path)
    .expect("save controls preview");
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
    if mode == "desktop-progress" {
        let mut muted = UiInlineStyle::default();
        muted.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::MutedText);
        install_chrome_with_center(
            app.world_mut(),
            Some(
                WidgetNode::row([
                    WidgetNode::label("Exporting").with_style(muted.clone()),
                    WidgetNode::progress(0.65)
                        .with_key("host:center-progress")
                        .with_width(UiLength::Px(78.0))
                        .with_height(UiLength::Px(8.0)),
                    WidgetNode::label("65%").with_style(muted),
                ])
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))
                .with_gap(8.0),
            ),
        );
    } else if mode.starts_with("mobile") {
        install_chrome_with_options(
            app.world_mut(),
            None,
            matches!(mode, "mobile" | "mobile-insets-open"),
            if mode.starts_with("mobile-insets") {
                UiEdges {
                    left: 12.0,
                    top: 28.0,
                    right: 12.0,
                    bottom: 24.0,
                }
            } else {
                UiEdges::ZERO
            },
            mode == "mobile-long",
        );
    } else {
        install_chrome(app.world_mut());
    }
    if mode.ends_with("hover") {
        let key = if mode.starts_with("desktop") {
            "chrome:close"
        } else {
            "chrome:mobile:nav:assets"
        };
        let entity = {
            let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
            keys.iter(app.world())
                .find_map(|(entity, found)| (found.0 == key).then_some(entity))
                .expect("hover target mounted")
        };
        app.world_mut()
            .get_mut::<bbui::widgets::Button>(entity)
            .expect("hover button")
            .status = bbui::widgets::WidgetStatus::Hovered;
    }
    for _ in 0..5 {
        app.update();
    }
    if mode.starts_with("desktop") {
        let center_target = {
            let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
            keys.iter(app.world())
                .find_map(|(entity, key)| (key.0 == "chrome:center-drag").then_some(entity))
                .expect("center drag target")
        };
        let chrome = {
            let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
            keys.iter(app.world())
                .find_map(|(entity, key)| (key.0 == "chrome:desktop").then_some(entity))
                .expect("desktop chrome")
        };
        let center_rect = app
            .world()
            .get::<bbui_layout::UiLayoutResult>(center_target)
            .expect("center layout")
            .rect;
        let chrome_rect = app
            .world()
            .get::<bbui_layout::UiLayoutResult>(chrome)
            .expect("chrome layout")
            .rect;
        let center = [
            center_rect.x + center_rect.width * 0.5,
            center_rect.y + center_rect.height * 0.5,
        ];
        assert!((center[0] - (chrome_rect.x + chrome_rect.width * 0.5)).abs() < 1.0);
        assert!(
            app.world()
                .get::<bbui_core::UiHitBox>(center_target)
                .expect("center hitbox")
                .contains(center)
        );
        for (control, expected_lines) in [("minimize", 1), ("maximize", 0), ("close", 2)] {
            let entity = {
                let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                keys.iter(app.world())
                    .find_map(|(entity, key)| {
                        (key.0 == format!("chrome:{control}")).then_some(entity)
                    })
                    .expect("window control mounted")
            };
            let rect = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(entity)
                .expect("control layout")
                .rect;
            let fragment = app
                .world()
                .resource::<bbui::render::DisplayList>()
                .widget_fragments()
                .get(&entity)
                .expect("control paint");
            let lines: Vec<_> = fragment
                .items
                .iter()
                .filter_map(|item| match item.primitive {
                    bbui::render::Primitive::Line { from, to, .. } => Some((from, to)),
                    _ => None,
                })
                .collect();
            assert_eq!(lines.len(), expected_lines);
            for (from, to) in lines {
                assert!(((from[0] + to[0]) * 0.5 - (rect.x + rect.width * 0.5)).abs() < 1.0);
                assert!(from[0] >= rect.x && from[0] <= rect.x + rect.width);
                assert!(to[0] >= rect.x && to[0] <= rect.x + rect.width);
                assert!(from[1] >= rect.y && from[1] <= rect.y + rect.height);
                assert!(to[1] >= rect.y && to[1] <= rect.y + rect.height);
            }
            if control == "maximize" {
                assert!(fragment.items.iter().any(|item| matches!(item.primitive,
                    bbui::render::Primitive::RoundedBorder { rect: border, .. }
                    if (border.x + border.width * 0.5 - (rect.x + rect.width * 0.5)).abs() < 1.0
                        && (border.y + border.height * 0.5 - (rect.y + rect.height * 0.5)).abs() < 1.0
                )));
            }
            assert!(
                !fragment.items.iter().any(|item| matches!(
                    item.primitive,
                    bbui::render::Primitive::TextWrapped { .. }
                ))
            );
        }
    }
    if mode.starts_with("mobile") {
        let mut keys = vec![
            "chrome:mobile:commands",
            "chrome:mobile:nav:scene",
            "chrome:mobile:nav:material",
            "chrome:mobile:nav:assets",
        ];
        if matches!(mode, "mobile" | "mobile-insets-open") {
            keys.push("chrome:mobile:sheet-close");
        }
        for key in keys {
            let entity = {
                let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                keys.iter(app.world())
                    .find_map(|(entity, found)| (found.0 == key).then_some(entity))
                    .expect("mobile chrome action")
            };
            let rect = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(entity)
                .expect("mobile action layout")
                .rect;
            assert!(rect.height >= 40.0, "{key} has height {}", rect.height);
            assert!(
                app.world()
                    .get::<bbui_core::UiHitBox>(entity)
                    .expect("mobile action hitbox")
                    .contains([rect.x + rect.width * 0.5, rect.y + rect.height * 0.5])
            );
        }
        if mode.starts_with("mobile-insets") {
            for (key, expected_height) in [
                ("chrome:mobile:toolbar", 88.0),
                ("chrome:mobile:bottom-nav", 92.0),
            ] {
                let entity = {
                    let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                    keys.iter(app.world())
                        .find_map(|(entity, found)| (found.0 == key).then_some(entity))
                        .expect("safe-area chrome region")
                };
                let rect = app
                    .world()
                    .get::<bbui_layout::UiLayoutResult>(entity)
                    .expect("safe-area region layout")
                    .rect;
                assert!((rect.height - expected_height).abs() < 1.0);
            }
            let content = {
                let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                keys.iter(app.world())
                    .find_map(|(entity, found)| {
                        (found.0 == "host:mobile-content").then_some(entity)
                    })
                    .expect("mobile content")
            };
            let rect = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(content)
                .expect("content layout")
                .rect;
            assert!(rect.x >= 12.0 && rect.x + rect.width <= width as f32 - 12.0);
        }
        if mode == "mobile-long" {
            for key in ["host:mobile-title", "chrome:mobile:nav:scene"] {
                let entity = {
                    let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
                    keys.iter(app.world())
                        .find_map(|(entity, found)| (found.0 == key).then_some(entity))
                        .expect("long mobile label")
                };
                let fragment = app
                    .world()
                    .resource::<bbui::render::DisplayList>()
                    .widget_fragments()
                    .get(&entity)
                    .expect("long label paint");
                assert!(fragment.items.iter().any(|item| matches!(
                    &item.primitive,
                    bbui::render::Primitive::TextWrapped { text, .. } if text.ends_with('…')
                )));
            }
        }
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
    if mode == "desktop" {
        save_controls_preview(
            &output.join(format!("chrome-controls-{theme}.png")),
            &bytes,
            width,
        );
    }
    if mode == "mobile" {
        let backdrop = {
            let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
            keys.iter(app.world())
                .find_map(|(entity, key)| {
                    (key.0 == "chrome:mobile:sheet-backdrop").then_some(entity)
                })
                .expect("sheet backdrop")
        };
        let position = [width as f32 * 0.5, height as f32 * 0.6];
        let window = {
            let mut windows = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>();
            windows.single(app.world()).expect("primary window")
        };
        app.world_mut()
            .resource_mut::<bbui_core::UiInputQueue>()
            .push(bbui_core::UiInputEvent::PointerMove {
                window,
                position,
                time_seconds: 1.0,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<bbui_core::UiFocus>()
                .state(window)
                .and_then(|state| state.hovered),
            Some(backdrop)
        );
        for (pressed, time_seconds) in [(true, 1.1), (false, 1.2)] {
            app.world_mut()
                .resource_mut::<bbui_core::UiInputQueue>()
                .push(bbui_core::UiInputEvent::PointerButton {
                    window,
                    button: bbui_core::UiPointerButton::Primary,
                    pressed,
                    position,
                    time_seconds,
                });
            app.update();
        }
        assert!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .any(|event| {
                    chrome_action(app.world(), event) == Some(ChromeAction::DismissSheet)
                })
        );
    }
    if mode.starts_with("desktop") {
        let hit_target_key = if mode == "desktop-progress" {
            "host:center-progress"
        } else {
            "host:center-title"
        };
        let hit_target = {
            let mut keys = app.world_mut().query::<(Entity, &UiKey)>();
            keys.iter(app.world())
                .find_map(|(entity, key)| (key.0 == hit_target_key).then_some(entity))
                .expect("center hit target")
        };
        let rect = app
            .world()
            .get::<bbui_layout::UiLayoutResult>(hit_target)
            .expect("center hit layout")
            .rect;
        let position = [rect.x + rect.width * 0.5, rect.y + rect.height * 0.5];
        let window = {
            let mut windows = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>();
            windows.single(app.world()).expect("primary window")
        };
        app.world_mut()
            .resource_mut::<bbui_core::UiInputQueue>()
            .push(bbui_core::UiInputEvent::PointerMove {
                window,
                position,
                time_seconds: 1.0,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<bbui_core::UiFocus>()
                .state(window)
                .and_then(|state| state.hovered),
            Some(hit_target)
        );
        app.world_mut()
            .resource_mut::<bbui_core::UiInputQueue>()
            .push(bbui_core::UiInputEvent::PointerButton {
                window,
                button: bbui_core::UiPointerButton::Primary,
                pressed: true,
                position,
                time_seconds: 1.1,
            });
        app.update();
        let press = app
            .world()
            .resource::<bbui_core::UiRoutedEvents>()
            .events
            .iter()
            .find(|event| {
                matches!(event,
                    bbui_core::UiRoutedEvent::PointerDown { target, .. } if *target == hit_target
                )
            })
            .cloned()
            .expect("center pointer press");
        assert!(handle_chrome_press(app.world_mut(), &press));
    }
}

#[test]
#[ignore = "run explicitly to write docs/visuals/examples/chrome-{desktop,mobile}-{dark,light}.png"]
fn capture_chrome_dark_and_light() {
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        capture(kind, "desktop", 1536, 900);
        capture(kind, "desktop-progress", 1536, 900);
        capture(kind, "mobile", 390, 844);
        capture(kind, "mobile-closed", 390, 844);
        capture(kind, "mobile-insets", 390, 844);
        capture(kind, "mobile-insets-open", 390, 844);
        capture(kind, "mobile-long", 390, 844);
        capture(kind, "desktop-hover", 1536, 900);
        capture(kind, "mobile-hover", 390, 844);
    }
}

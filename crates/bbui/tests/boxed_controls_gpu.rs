//! Pixel checks for boxed layout, Slider, radio, icon, and menu visuals.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    fs, thread,
    time::{Duration, Instant},
};

use bbui::{BbuiPlugin, widgets::WidgetNode, widgets::WidgetTree};
use bbui_core::{UiFocus, UiKey};
use bbui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength};
use bbui_render::{DisplayList, Primitive};
use bbui_style::{Rgba, Theme, ThemeColorToken, ThemeKind};
use bbui_widgets::{MenuControl, MenuEntry, MenuItemControl, RadioGroup, Slider};
use bevy::{
    app::PluginGroup,
    asset::Assets,
    camera::{ClearColor, RenderTarget},
    color::Color,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, Entity, ResMut, Resource, WindowPlugin},
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};

const TARGET_WIDTH: u32 = 512;
const TARGET_HEIGHT: u32 = 480;
const ROOT_RECT: Rect = Rect::new(16.0, 16.0, 480.0, 440.0);

#[derive(Resource, Default)]
struct CapturedFrames(HashMap<usize, Vec<u8>>);

#[cfg(test)]
fn find_key(world: &mut bevy::prelude::World, expected: &str) -> Entity {
    let mut query = world.query::<(Entity, &UiKey)>();
    query
        .iter(world)
        .find_map(|(entity, key)| (key.0 == expected).then_some(entity))
        .unwrap_or_else(|| panic!("widget key {expected:?} was not mounted"))
}

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

#[cfg(test)]
fn write_capture_ppm(path: &std::path::Path, pixels: &[u8]) {
    let mut ppm = format!("P6\n{TARGET_WIDTH} {TARGET_HEIGHT}\n255\n").into_bytes();
    ppm.reserve(TARGET_WIDTH as usize * TARGET_HEIGHT as usize * 3);
    for y in 0..TARGET_HEIGHT as usize {
        for x in 0..TARGET_WIDTH as usize {
            ppm.extend_from_slice(&pixel_at(pixels, x, y)[..3]);
        }
    }
    fs::write(path, ppm).expect("write optional controls visual capture");
}

fn assert_rgba_near(pixel: &[u8], color: Rgba, tolerance: u8, what: &str) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "{what}: actual={pixel:?}, expected={expected:?} ±{tolerance}"
        );
    }
}

fn assert_color_near_position(
    pixels: &[u8],
    position: [f32; 2],
    color: Rgba,
    tolerance: u8,
    radius: usize,
    what: &str,
) {
    let expected = [
        (color.0 * 255.0).round() as u8,
        (color.1 * 255.0).round() as u8,
        (color.2 * 255.0).round() as u8,
        (color.3 * 255.0).round() as u8,
    ];
    let center_x = position[0].round() as usize;
    let center_y = position[1].round() as usize;
    let x_start = center_x.saturating_sub(radius);
    let x_end = (center_x + radius).min(TARGET_WIDTH as usize - 1);
    let y_start = center_y.saturating_sub(radius);
    let y_end = (center_y + radius).min(TARGET_HEIGHT as usize - 1);
    let found = (y_start..=y_end).any(|y| {
        (x_start..=x_end).any(|x| {
            pixel_at(pixels, x, y)
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
        })
    });
    assert!(
        found,
        "{what}: no pixel near {position:?} matched {expected:?} ±{tolerance}"
    );
}

fn assert_visible_glyph(pixels: &[u8], rect: Rect, theme: &Theme, what: &str) {
    let color = [
        (theme.text.0 * 255.0).round() as u8,
        (theme.text.1 * 255.0).round() as u8,
        (theme.text.2 * 255.0).round() as u8,
    ];
    let bounds = [
        rect.x.floor().max(0.0) as usize,
        rect.y.floor().max(0.0) as usize,
        (rect.x + rect.width).ceil().min(TARGET_WIDTH as f32) as usize,
        (rect.y + rect.height).ceil().min(TARGET_HEIGHT as f32) as usize,
    ];
    let matching = (bounds[1]..bounds[3])
        .flat_map(|y| (bounds[0]..bounds[2]).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let pixel = pixel_at(pixels, *x, *y);
            pixel[..3]
                .iter()
                .zip(color)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 48)
        })
        .count();
    assert!(
        matching >= 5,
        "{what} glyph was not visible; matched={matching}"
    );
}

#[test]
fn boxed_slider_radio_icon_menu_render_dark_light_at_two_dpi_scales() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
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
    .add_plugins(BbuiPlugin)
    .init_resource::<CapturedFrames>();
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();

    let tree = WidgetNode::column([
        WidgetNode::boxed([WidgetNode::label("Boxed group").with_key("box-label")])
            .with_key("box")
            .with_width(UiLength::Px(440.0))
            .with_height(UiLength::Px(72.0)),
        WidgetNode::slider_range("Opacity", 0.7, 0.0, 1.0, 0.05)
            .with_key("slider")
            .with_width(UiLength::Px(440.0))
            .with_height(UiLength::Px(60.0)),
        WidgetNode::slider_range("Locked", 0.3, 0.0, 1.0, 0.05)
            .with_key("disabled-slider")
            .with_width(UiLength::Px(440.0))
            .with_height(UiLength::Px(60.0)),
        WidgetNode::radio_group("Mode", RadioGroup::new(["Object", "Edit"], Some(1)))
            .with_key("radio")
            .with_width(UiLength::Px(300.0))
            .with_height(UiLength::Px(72.0)),
        WidgetNode::icon_button("+", "Add object")
            .with_key("icon")
            .with_width(UiLength::Px(44.0))
            .with_height(UiLength::Px(36.0)),
        WidgetNode::menu(
            "File",
            [
                MenuEntry::command("new", "New"),
                MenuEntry::command("open", "Open"),
            ],
        )
        .with_key("menu")
        .with_width(UiLength::Px(160.0))
        .with_height(UiLength::Px(32.0)),
    ])
    .with_key("root")
    .with_width(UiLength::Px(ROOT_RECT.width))
    .with_height(UiLength::Px(ROOT_RECT.height));
    let root = WidgetTree::new(tree)
        .spawn(app.world_mut(), None)
        .expect("controls should mount");
    app.world_mut()
        .entity_mut(root)
        .insert(UiLayoutRoot::new(camera, ROOT_RECT, 1.0));

    let slider = find_key(app.world_mut(), "slider");
    let disabled_slider = find_key(app.world_mut(), "disabled-slider");
    let menu = find_key(app.world_mut(), "menu");
    app.world_mut()
        .get_mut::<Slider>(disabled_slider)
        .unwrap()
        .field
        .enabled = false;
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(camera)
        .focused = Some(slider);
    {
        let mut menu_control = app.world_mut().get_mut::<MenuControl>(menu).unwrap();
        menu_control.session.open(None);
        let tree = menu_control.tree.clone();
        menu_control.session.move_selection(&tree, 1);
    }

    for _ in 0..8 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let menu_items = app.world().get::<MenuControl>(menu).unwrap().items.clone();
    assert_eq!(menu_items.len(), 2, "open menu should mount two rows");
    let selected_menu_item = menu_items
        .iter()
        .copied()
        .find(|entity| app.world().get::<MenuItemControl>(*entity).unwrap().index == 0)
        .unwrap();

    let root_node = app.world().get::<UiLayoutNode>(root).unwrap();
    assert_eq!(root_node.size.width, UiLength::Px(ROOT_RECT.width));
    let box_entity = find_key(app.world_mut(), "box");
    let radio_entity = find_key(app.world_mut(), "radio");
    let icon_entity = find_key(app.world_mut(), "icon");
    assert_eq!(
        app.world()
            .get::<RadioGroup>(radio_entity)
            .unwrap()
            .selected,
        Some(1)
    );

    let mut scenario = 0;
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        for dpi in [1.0, 2.0] {
            let theme = Theme::new(kind);
            *app.world_mut().resource_mut::<Theme>() = theme.clone();
            *app.world_mut().resource_mut::<ClearColor>() = ClearColor(Color::srgba(
                theme.background.0,
                theme.background.1,
                theme.background.2,
                theme.background.3,
            ));
            app.world_mut()
                .get_mut::<UiLayoutRoot>(root)
                .unwrap()
                .dpi_scale = dpi;
            for _ in 0..3 {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }

            let box_rect = app.world().get::<UiLayoutResult>(box_entity).unwrap().rect;
            let slider_rect = app.world().get::<UiLayoutResult>(slider).unwrap().rect;
            let disabled_rect = app
                .world()
                .get::<UiLayoutResult>(disabled_slider)
                .unwrap()
                .rect;
            let radio_rect = app
                .world()
                .get::<UiLayoutResult>(radio_entity)
                .unwrap()
                .rect;
            let icon_rect = app.world().get::<UiLayoutResult>(icon_entity).unwrap().rect;
            let menu_rect = app.world().get::<UiLayoutResult>(menu).unwrap().rect;
            let popup_rect = app
                .world()
                .get::<UiLayoutResult>(selected_menu_item)
                .unwrap()
                .rect;

            let readback = app
                .world_mut()
                .spawn(Readback::texture(target.clone()))
                .observe(
                    move |captured: bevy::ecs::observer::On<ReadbackComplete>,
                          mut frames: ResMut<CapturedFrames>| {
                        frames.0.insert(scenario, captured.data.clone());
                    },
                )
                .id();
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline
                && !app
                    .world()
                    .resource::<CapturedFrames>()
                    .0
                    .contains_key(&scenario)
            {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let pixels = app
                .world()
                .resource::<CapturedFrames>()
                .0
                .get(&scenario)
                .cloned()
                .expect("boxed controls GPU readback did not complete");
            app.world_mut().entity_mut(readback).despawn();
            assert_rgba_near(
                pixel_at(&pixels, 8, 8),
                theme.background,
                5,
                "theme-matched offscreen scene background",
            );
            if let Some(directory) = std::env::var_os("BBUI_CONTROLS_CAPTURE_DIR") {
                let directory = std::path::PathBuf::from(directory);
                fs::create_dir_all(&directory).expect("create controls capture directory");
                write_capture_ppm(
                    &directory.join(format!("{kind:?}-dpi{}.ppm", dpi as u32)),
                    &pixels,
                );
            }

            let box_sample = [
                (box_rect.x + box_rect.width - 18.0).round() as usize,
                (box_rect.y + box_rect.height - 16.0).round() as usize,
            ];
            assert_rgba_near(
                pixel_at(&pixels, box_sample[0], box_sample[1]),
                theme.surface,
                5,
                "boxed container surface",
            );
            assert_color_near_position(
                &pixels,
                [box_rect.x + box_rect.width * 0.5, box_rect.y + 1.0],
                theme.border,
                6,
                2,
                "boxed container border",
            );

            let radius = theme.spacing.editor.handle_radius.clamp(3.0, 9.0);
            let left = slider_rect.x + theme.spacing.control.padding_x + radius;
            let right =
                slider_rect.x + slider_rect.width - theme.spacing.control.padding_x - radius;
            let center_y =
                slider_rect.y + slider_rect.height - theme.spacing.control.padding_y - radius;
            let handle_x = left + (right - left) * 0.7;
            assert_color_near_position(
                &pixels,
                [handle_x, center_y],
                theme.accent,
                5,
                2,
                "enabled slider handle",
            );
            assert_color_near_position(
                &pixels,
                [handle_x - radius - 1.0, center_y],
                theme.focus,
                8,
                2,
                "focused slider ring",
            );

            let disabled_left = disabled_rect.x + theme.spacing.control.padding_x + radius;
            let disabled_right =
                disabled_rect.x + disabled_rect.width - theme.spacing.control.padding_x - radius;
            let disabled_x = disabled_left + (disabled_right - disabled_left) * 0.3;
            let disabled_y =
                disabled_rect.y + disabled_rect.height - theme.spacing.control.padding_y - radius;
            let disabled_handle_color = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .find_map(|item| match &item.primitive {
                    Primitive::Ellipse { rect, color }
                        if disabled_x >= rect.x
                            && disabled_x <= rect.x + rect.width
                            && disabled_y >= rect.y
                            && disabled_y <= rect.y + rect.height =>
                    {
                        Some(*color)
                    }
                    _ => None,
                })
                .expect("disabled slider should emit its handle ellipse");
            assert_ne!(
                disabled_handle_color, theme.accent,
                "disabled slider handle should not use the enabled accent color"
            );
            assert_color_near_position(
                &pixels,
                [disabled_x, disabled_y],
                disabled_handle_color,
                6,
                2,
                "disabled slider handle",
            );

            let control_height = theme.control_height.max(1.0);
            let diameter = (control_height - 8.0).clamp(8.0, 16.0);
            let radio_x = radio_rect.x + theme.spacing.control.padding_x;
            let selected_y = radio_rect.y
                + control_height * 2.0
                + (control_height - diameter) * 0.5
                + diameter * 0.5;
            assert_color_near_position(
                &pixels,
                [radio_x + diameter * 0.5, selected_y],
                theme.accent,
                6,
                1,
                "selected radio mark",
            );

            assert_visible_glyph(&pixels, icon_rect, &theme, "icon button");
            assert_visible_glyph(&pixels, menu_rect, &theme, "menu trigger");
            let popup_center = [
                (popup_rect.x + popup_rect.width * 0.5).round() as usize,
                (popup_rect.y + popup_rect.height * 0.5).round() as usize,
            ];
            assert_rgba_near(
                pixel_at(&pixels, popup_center[0], popup_center[1]),
                theme.color(ThemeColorToken::MenuSelectedSurface),
                6,
                "selected menu row",
            );

            println!("boxed controls GPU scene passed: {kind:?}, dpi={dpi}");
            scenario += 1;
        }
    }
}

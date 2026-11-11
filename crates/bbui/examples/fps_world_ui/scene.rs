//! Build the offscreen BBUI console and the 3D shooting range.

use bbui::UiCrosshairPointer;
use bbui_core::UiKey;
use bbui_layout::Rect;
use bbui_render::{RenderTargetCamera, UiWorldSurface};
use bbui_widgets::WidgetNode;
use bevy::{
    asset::Assets,
    camera::{Camera, RenderTarget},
    math::Dir3,
    mesh::Meshable,
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::*,
    render::{render_resource::TextureFormat, view::Tonemapping},
    ui::IsDefaultUiCamera,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use super::{
    ConsoleUi, Crosshair, GameState, PANEL_HEIGHT, PANEL_WIDTH, Player, RangeLamp, RangeTarget,
    TARGET_COUNT, property_editor,
};

fn keyed(world: &World, key: &str) -> Option<Entity> {
    world.iter_entities().find_map(|entry| {
        entry
            .get::<UiKey>()
            .is_some_and(|value| value.0 == key)
            .then_some(entry.id())
    })
}

fn box_mesh(world: &mut World, size: Vec3) -> Handle<Mesh> {
    world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(size.x, size.y, size.z))
}

fn box_material(world: &mut World, color: Color) -> Handle<StandardMaterial> {
    world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: color,
            metallic: 0.18,
            perceptual_roughness: 0.75,
            ..default()
        })
}

fn spawn_box(world: &mut World, size: Vec3, position: Vec3, material: &Handle<StandardMaterial>) {
    let mesh = box_mesh(world, size);
    world.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(position),
    ));
}

pub(super) fn setup(world: &mut World) {
    let physical_window = match world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    {
        Ok(window) => window,
        Err(error) => {
            eprintln!("FPS example requires a primary window: {error}");
            return;
        }
    };
    if let Some(mut cursor) = world.get_mut::<CursorOptions>(physical_window) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }

    let ui_texture = world
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            PANEL_WIDTH,
            PANEL_HEIGHT,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let ui_camera = world
        .spawn((
            Camera2d,
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(ui_texture.clone().into()),
        ))
        .id();
    world.resource_mut::<RenderTargetCamera>().0 = Some(ui_camera);

    let range_panel = WidgetNode::panel(
        "Shooting Range",
        true,
        [
            WidgetNode::checkbox("Range lights", true).with_key("range-power"),
            WidgetNode::property_row_with_decorator(
                "Brightness",
                WidgetNode::slider_range("", 0.7, 0.1, 1.0, 0.01)
                    .with_role("filled-slider")
                    .with_key("range-brightness"),
            ),
            WidgetNode::button("Reset targets").with_key("range-reset"),
            WidgetNode::label("Hits: 0 / 5").with_key("range-score"),
            WidgetNode::progress(0.0).with_key("range-progress"),
        ],
    );
    property_editor::mount(
        world,
        ui_camera,
        Rect::new(0.0, 0.0, PANEL_WIDTH as f32, PANEL_HEIGHT as f32),
        Some(property_editor::EditorExtraPanel {
            key: "range-panel",
            node: range_panel,
        }),
    );
    let (Some(power), Some(brightness), Some(reset), Some(score), Some(progress)) = (
        keyed(world, "range-power"),
        keyed(world, "range-brightness"),
        keyed(world, "range-reset"),
        keyed(world, "range-score"),
        keyed(world, "range-progress"),
    ) else {
        eprintln!("FPS example is missing a console control");
        return;
    };
    world.insert_resource(ConsoleUi {
        power,
        brightness,
        reset,
        score,
        progress,
    });

    let wall = box_material(world, Color::srgb(0.26, 0.31, 0.39));
    let floor = box_material(world, Color::srgb(0.32, 0.36, 0.39));
    let trim = box_material(world, Color::srgb(0.10, 0.15, 0.20));
    let ready = box_material(world, Color::srgb(0.85, 0.27, 0.16));
    let hit = box_material(world, Color::srgb(0.17, 0.75, 0.39));
    world.insert_resource(GameState {
        score: 0,
        lights_on: true,
        brightness: 0.7,
        ready_material: ready.clone(),
        hit_material: hit,
    });
    spawn_box(
        world,
        Vec3::new(10.0, 0.2, 14.0),
        Vec3::new(0.0, -0.1, -0.6),
        &floor,
    );
    spawn_box(
        world,
        Vec3::new(10.0, 4.0, 0.2),
        Vec3::new(0.0, 2.0, -7.5),
        &wall,
    );
    spawn_box(
        world,
        Vec3::new(0.2, 4.0, 14.0),
        Vec3::new(-5.0, 2.0, -0.6),
        &wall,
    );
    spawn_box(
        world,
        Vec3::new(0.2, 4.0, 14.0),
        Vec3::new(5.0, 2.0, -0.6),
        &wall,
    );
    spawn_box(
        world,
        Vec3::new(10.0, 0.2, 14.0),
        Vec3::new(0.0, 4.0, -0.6),
        &trim,
    );

    for index in 0..TARGET_COUNT {
        let x = -3.4 + index as f32 * 1.7;
        let mesh = box_mesh(world, Vec3::splat(0.55));
        world.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(ready.clone()),
            Transform::from_xyz(x, 1.55, -6.8),
            RangeTarget { alive: true },
        ));
        spawn_box(
            world,
            Vec3::new(0.1, 1.5, 0.1),
            Vec3::new(x, 0.75, -6.9),
            &trim,
        );
    }

    let panel_size = Vec2::new(2.55, 3.385);
    let panel_mesh = world.resource_mut::<Assets<Mesh>>().add(
        Plane3d::default()
            .mesh()
            .size(panel_size.x, panel_size.y)
            .normal(Dir3::Z),
    );
    let panel_material = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color_texture: Some(ui_texture),
            unlit: true,
            ..default()
        });
    let panel_position = Vec3::new(-1.45, 2.0, -1.45);
    spawn_box(
        world,
        Vec3::new(2.71, 3.545, 0.12),
        panel_position - Vec3::Z * 0.08,
        &trim,
    );
    world.spawn((
        Mesh3d(panel_mesh),
        MeshMaterial3d(panel_material),
        Transform::from_translation(panel_position),
        UiWorldSurface {
            target_camera: ui_camera,
            texture_asset_id: 1,
            world_size: panel_size,
            resolution: UVec2::new(PANEL_WIDTH, PANEL_HEIGHT),
        },
    ));

    world.spawn((
        PointLight {
            intensity: 500_000.0 * 0.7,
            range: 14.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 3.3, -2.0),
        RangeLamp,
    ));
    world.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.47, 0.55, 0.69),
        brightness: 600.0,
        affects_lightmapped_meshes: true,
    });

    let yaw = (1.45_f32 / 4.75).atan();
    let pitch = 0.02;
    world.spawn((
        Camera3d::default(),
        Tonemapping::None,
        IsDefaultUiCamera,
        UiCrosshairPointer { max_distance: 7.5 },
        Transform::from_xyz(0.0, 1.9, 3.3)
            .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)),
        Player { yaw, pitch },
    ));
    world.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(4.0),
            height: Val::Px(4.0),
            ..default()
        },
        BackgroundColor(Color::WHITE),
        Crosshair,
        ZIndex(100),
    ));
    world.spawn((
        Text::new("WASD move  |  mouse look  |  click / drag editor  |  F1 cursor"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            bottom: Val::Px(20.0),
            ..default()
        },
        TextColor(Color::WHITE),
        ZIndex(100),
    ));
}

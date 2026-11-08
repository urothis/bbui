//! Run with `cargo run -p bui_render --example world_surface`.
//!
//! The BUI UI camera renders to an image, and a lit-independent 3D plane displays that image.
//! `UiWorldSurface` gives the host the matching ray-to-UI coordinate mapping for pointer input.

use bevy::{
    asset::RenderAssetUsages,
    camera::{Camera, RenderTarget},
    math::{Dir3, UVec2, Vec2, Vec3},
    mesh::Meshable,
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::{
        App, Assets, Camera2d, Camera3d, Color, Commands, DefaultPlugins, Image, Mesh, Mesh3d,
        Plane3d, ResMut, Startup, Transform,
    },
    render::render_resource::{TextureDimension, TextureFormat},
};
use bui_core::CorePlugin;
use bui_render::{
    BuiRenderPlugin, ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive, RenderTargetCamera,
    UiWorldSurface,
};
use bui_style::Rgba;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((CorePlugin, BuiRenderPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut image_registry: ResMut<ImageRegistry>,
    mut render_target: ResMut<RenderTargetCamera>,
    mut display_list: ResMut<DisplayList>,
) {
    const WIDTH: u32 = 512;
    const HEIGHT: u32 = 256;
    const TEXTURE_ASSET_ID: u64 = 1;

    let target = images.add(Image::new_target_texture(
        WIDTH,
        HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let mut icon_pixels = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16_i32 {
        for x in 0..16_i32 {
            let inside = (x - 7).abs() + (y - 7).abs() <= 6;
            if inside {
                icon_pixels.extend_from_slice(&[100, 210, 240, 255]);
            } else {
                icon_pixels.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    let icon = images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        icon_pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ));
    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -1,
                ..Default::default()
            },
            RenderTarget::Image(target.clone().into()),
        ))
        .id();
    render_target.0 = Some(ui_camera);
    image_registry.insert(TEXTURE_ASSET_ID, target.clone());
    image_registry.insert(2, icon);

    let world_size = Vec2::new(3.2, 1.6);
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(world_size.x, world_size.y)
                    .normal(Dir3::Z),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(target),
            unlit: true,
            ..Default::default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        UiWorldSurface {
            target_camera: ui_camera,
            texture_asset_id: TEXTURE_ASSET_ID,
            world_size,
            resolution: UVec2::new(WIDTH, HEIGHT),
        },
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: WIDTH as f32,
        height: HEIGHT as f32,
    };
    display_list.push(DrawItem {
        clip,
        z: 0,
        primitive: Primitive::RoundedRect {
            rect: ClipRect {
                x: 12.0,
                y: 12.0,
                width: 488.0,
                height: 232.0,
            },
            radius: 18.0,
            color: Rgba(0.08, 0.10, 0.14, 1.0),
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 1,
        primitive: Primitive::RoundedRect {
            rect: ClipRect {
                x: 28.0,
                y: 28.0,
                width: 456.0,
                height: 200.0,
            },
            radius: 12.0,
            color: Rgba(0.14, 0.20, 0.29, 1.0),
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 2,
        primitive: Primitive::Text {
            origin: [58.0, 70.0],
            text: "BUI world surface".to_owned(),
            color: Rgba(0.95, 0.97, 1.0, 1.0),
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 3,
        primitive: Primitive::Text {
            origin: [58.0, 112.0],
            text: "Rendered through Bevy's GPU UI pipeline".to_owned(),
            color: Rgba(0.63, 0.76, 0.91, 1.0),
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 4,
        primitive: Primitive::Line {
            from: [58.0, 158.0],
            to: [440.0, 158.0],
            width: 3.0,
            color: Rgba(0.32, 0.70, 0.94, 1.0),
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 5,
        primitive: Primitive::Shadow {
            rect: ClipRect {
                x: 34.0,
                y: 178.0,
                width: 92.0,
                height: 30.0,
            },
            radius: 8.0,
            color: Rgba(0.12, 0.20, 0.29, 1.0),
            shadow: Rgba(0.0, 0.0, 0.0, 0.55),
            offset: [0.0, 3.0],
            blur: 7.0,
            spread: 1.0,
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 6,
        primitive: Primitive::Icon {
            rect: ClipRect {
                x: 54.0,
                y: 47.0,
                width: 22.0,
                height: 22.0,
            },
            asset_id: 2,
            tint: Rgba(1.0, 1.0, 1.0, 1.0),
        },
    });
    display_list.push(DrawItem {
        clip,
        z: 7,
        primitive: Primitive::Bezier {
            points: [
                [145.0, 193.0],
                [220.0, 150.0],
                [300.0, 225.0],
                [390.0, 190.0],
            ],
            width: 3.0,
            color: Rgba(0.35, 0.82, 0.70, 1.0),
        },
    });
}

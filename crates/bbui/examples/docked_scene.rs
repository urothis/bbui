//! A live Bevy scene mounted inside a BBUI dock pane.
//!
//! Drag the splitter or switch tabs to see the camera follow its pane. Right-drag
//! to orbit, middle-drag to pan, and scroll over the scene to zoom.

use bbui::prelude::*;
use bbui::render::{Primitive, UiPaint};
use bbui::widgets::WidgetNode;
use bbui_core::{UiFocus, UiKey, UiSystems, UiZIndex};
use bbui_layout::{Rect, UiEdges, UiLayoutResult};
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::Tonemapping;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "docked_scene/capture.rs"]
mod capture;

const SCENE_KEY: &str = "docked-scene:surface";

#[derive(Component)]
struct DockedSceneCamera;

#[derive(Resource)]
struct SceneOrbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
    center: Vec3,
    drag: Option<MouseButton>,
    previous_pointer: Option<Vec2>,
}

impl Default for SceneOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.58,
            pitch: -0.42,
            distance: 9.0,
            center: Vec3::ZERO,
            drag: None,
            previous_pointer: None,
        }
    }
}

fn surface_rect(layout: &UiLayoutResult) -> Option<Rect> {
    let rect = layout.rect;
    let clip = layout.clip.unwrap_or(rect);
    let left = rect.x.max(clip.x);
    let top = rect.y.max(clip.y);
    let right = (rect.x + rect.width).min(clip.x + clip.width);
    let bottom = (rect.y + rect.height).min(clip.y + clip.height);
    (right > left && bottom > top).then_some(Rect::new(left, top, right - left, bottom - top))
}

// Bevy injects these independent event, layout, and camera resources as system parameters.
#[allow(clippy::too_many_arguments)]
fn scene_input(
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    workspace: Res<Workspace>,
    surfaces: Query<(&UiKey, &UiLayoutResult)>,
    focus: Res<UiFocus>,
    modals: Res<ModalDialogs>,
    mut buttons: MessageReader<MouseButtonInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut orbit: ResMut<SceneOrbit>,
    mut camera: Single<&mut Transform, With<DockedSceneCamera>>,
) {
    let (window_entity, window) = *window;
    let active = workspace
        .layout()
        .group("scene-tabs")
        .and_then(|group| group.active.as_ref())
        .is_some_and(|pane| pane.0 == "scene");
    let rect = active
        .then(|| {
            surfaces.iter().find_map(|(key, layout)| {
                (key.0 == SCENE_KEY).then(|| surface_rect(layout)).flatten()
            })
        })
        .flatten();
    let pointer = window.cursor_position();
    let inside = rect.zip(pointer).is_some_and(|(rect, pointer)| {
        pointer.x >= rect.x
            && pointer.y >= rect.y
            && pointer.x < rect.x + rect.width
            && pointer.y < rect.y + rect.height
    });
    let ui_busy = modals.is_open(window_entity)
        || focus
            .state(window_entity)
            .is_some_and(|state| state.captured.is_some());
    if !active || ui_busy {
        orbit.drag = None;
        orbit.previous_pointer = None;
    }
    for event in buttons.read() {
        if event.window != window_entity {
            continue;
        }
        if event.state.is_pressed() && inside && !ui_busy {
            if matches!(event.button, MouseButton::Right | MouseButton::Middle) {
                orbit.drag = Some(event.button);
                orbit.previous_pointer = pointer;
            }
        } else if !event.state.is_pressed() && orbit.drag == Some(event.button) {
            orbit.drag = None;
        }
    }
    if let Some(pointer) = pointer {
        if let (Some(previous), Some(drag)) = (orbit.previous_pointer, orbit.drag) {
            let delta = pointer - previous;
            if drag == MouseButton::Right {
                orbit.yaw -= delta.x * 0.007;
                orbit.pitch = (orbit.pitch - delta.y * 0.007).clamp(-1.45, 1.45);
            } else if drag == MouseButton::Middle {
                let right = camera.right();
                let up = camera.up();
                let distance = orbit.distance;
                orbit.center += (-right * delta.x + up * delta.y) * distance * 0.002;
            }
        }
        orbit.previous_pointer = Some(pointer);
    }
    for event in wheel.read() {
        if event.window == window_entity && inside && !ui_busy {
            orbit.distance = (orbit.distance * (-event.y * 0.11).exp()).clamp(2.5, 35.0);
        }
    }
    let direction = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        -orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    );
    camera.translation = orbit.center + direction * orbit.distance;
    camera.look_at(orbit.center, Vec3::Y);
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Bevy's ViewportNode resizes this image to the pane's computed UI size.
    let scene_image = images.add(Image::new_target_texture(
        0,
        0,
        TextureFormat::Bgra8UnormSrgb,
        None,
    ));
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Camera {
            // Render before the UI camera samples the scene texture.
            order: -1,
            ..default()
        },
        RenderTarget::Image(scene_image.into()),
        Transform::from_xyz(5.0, 4.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        DockedSceneCamera,
    ));
    commands.spawn((
        Camera2d,
        Tonemapping::None,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    ));
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 5_000.0,
            ..default()
        },
        Transform::from_xyz(3.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let ground = meshes.add(Plane3d::default().mesh().size(12.0, 12.0));
    commands.spawn((
        Mesh3d(ground),
        MeshMaterial3d(materials.add(Color::srgb(0.20, 0.31, 0.31))),
    ));
    let grid_material = materials.add(Color::srgb(0.30, 0.43, 0.43));
    let grid_line = meshes.add(Cuboid::new(0.012, 0.008, 12.0));
    for coordinate in -5..=5 {
        commands.spawn((
            Mesh3d(grid_line.clone()),
            MeshMaterial3d(grid_material.clone()),
            Transform::from_xyz(coordinate as f32, 0.008, 0.0),
        ));
        commands.spawn((
            Mesh3d(grid_line.clone()),
            MeshMaterial3d(grid_material.clone()),
            Transform::from_xyz(0.0, 0.008, coordinate as f32)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        ));
    }
    for (x, z, height, color) in [
        (-2.5, -1.0, 1.2, Color::srgb(0.39, 0.73, 0.80)),
        (-0.7, 0.6, 2.3, Color::srgb(0.75, 0.53, 0.36)),
        (1.5, -0.8, 1.7, Color::srgb(0.62, 0.68, 0.45)),
        (2.6, 1.6, 0.9, Color::srgb(0.76, 0.63, 0.76)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.2, height, 1.2))),
            MeshMaterial3d(materials.add(color)),
            Transform::from_xyz(x, height / 2.0, z),
        ));
    }
}

fn install_workspace(world: &mut World) {
    let Ok(scene_camera) = world
        .query_filtered::<Entity, With<DockedSceneCamera>>()
        .single(world)
    else {
        eprintln!("docked scene requires a scene camera");
        return;
    };
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("docked scene requires a primary window");
        return;
    };
    let Ok(shell) = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::row([
                WidgetNode::label("SCENE  /  DOCKED VIEWPORT").with_width(UiLength::Fill(1.0)),
                WidgetNode::label("Perspective  ·  Solid"),
            ])
            .with_height(UiLength::Px(42.0))
            .with_padding(UiEdges::horizontal_vertical(12.0, 8.0)),
            WidgetNode::column([])
                .with_key("scene:workspace-host")
                .with_height(UiLength::Fill(1.0)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
    .spawn(world, None) else {
        eprintln!("failed to mount scene shell");
        return;
    };
    let Some(window_data) = world.get::<Window>(window) else {
        return;
    };
    let dimensions = window_data.resolution.size();
    world.entity_mut(shell).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, dimensions.x, dimensions.y),
        1.0,
    ));
    let Some(host) = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "scene:workspace-host").then_some(entity))
    else {
        eprintln!("scene workspace host is missing");
        return;
    };
    let Ok(mut workspace) = Workspace::new(WorkspaceLayout::new(DockNode::Split {
        id: "main-split".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![3.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new(
                "scene-tabs",
                vec!["scene".into(), "notes".into()],
            )),
            DockNode::Tabs(TabGroup::new("inspector-tabs", vec!["inspector".into()])),
        ],
    })) else {
        eprintln!("invalid scene workspace layout");
        return;
    };
    workspace.set_mount_parent(host);
    workspace.register_pane("scene", "Scene", move |world, parent| {
        if let Err(error) = WidgetTree::new(
            WidgetNode::column([
                WidgetNode::column([])
                    .with_key(SCENE_KEY)
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Fill(1.0)),
                WidgetNode::label("Right drag: orbit   ·   Middle drag: pan   ·   Wheel: zoom")
                    .with_height(UiLength::Px(30.0))
                    .with_padding(UiEdges::horizontal_vertical(12.0, 6.0)),
            ])
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
        )
        .spawn(world, Some(parent))
        {
            eprintln!("failed to mount scene pane: {error:?}");
        }
        if let Some(surface) = world
            .query::<(Entity, &UiKey)>()
            .iter(world)
            .find_map(|(entity, key)| (key.0 == SCENE_KEY).then_some(entity))
        {
            world.entity_mut(surface).insert((
                UiPaint(Primitive::Viewport {
                    rect: Rect::new(0.0, 0.0, 0.0, 0.0).into(),
                    camera: scene_camera,
                }),
                UiZIndex(1),
            ));
        }
    });
    workspace.register_pane("notes", "Notes", |world, parent| {
        if let Err(error) = WidgetTree::new(WidgetNode::label(
            "The scene camera sleeps while this tab is open.",
        ))
        .spawn(world, Some(parent))
        {
            eprintln!("failed to mount notes pane: {error:?}");
        }
    });
    workspace.register_pane("inspector", "Inspector", |world, parent| {
        if let Err(error) = WidgetTree::new(
            WidgetNode::column([
                WidgetNode::panel(
                    "View",
                    true,
                    [
                        WidgetNode::label("Perspective camera"),
                        WidgetNode::label("Orbit  ·  Pan  ·  Zoom"),
                        WidgetNode::label("Viewport follows the active dock tab"),
                    ],
                )
                .with_width(UiLength::Fill(1.0)),
                WidgetNode::panel(
                    "Scene collection",
                    true,
                    [
                        WidgetNode::label("▾  Geometry"),
                        WidgetNode::label("    ◼  Copper tower"),
                        WidgetNode::label("    ◼  Sage tower"),
                        WidgetNode::label("    ◼  Lavender block"),
                        WidgetNode::label("    ◼  Cyan block"),
                        WidgetNode::label("▾  Lighting"),
                        WidgetNode::label("    ◉  Sun"),
                    ],
                )
                .with_width(UiLength::Fill(1.0)),
                WidgetNode::panel(
                    "Dock behavior",
                    true,
                    [
                        WidgetNode::label("Resize the split to reframe the scene."),
                        WidgetNode::label("Open Notes to pause the camera."),
                    ],
                )
                .with_width(UiLength::Fill(1.0)),
            ])
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
            .with_gap(8.0)
            .with_padding(UiEdges::all(10.0)),
        )
        .spawn(world, Some(parent))
        {
            eprintln!("failed to mount inspector pane: {error:?}");
        }
    });
    world.insert_resource(workspace);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI docked scene".into(),
                resolution: WindowResolution::new(1200, 760),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .init_resource::<SceneOrbit>()
        .add_systems(Startup, (setup, install_workspace).chain())
        .add_systems(Update, scene_input)
        .add_systems(Update, sync_scene_activity.after(UiSystems::Widgets))
        .run();
}

fn sync_scene_activity(
    workspace: Res<Workspace>,
    mut camera: Single<&mut Camera, With<DockedSceneCamera>>,
) {
    let active = workspace
        .layout()
        .group("scene-tabs")
        .and_then(|group| group.active.as_ref())
        .is_some_and(|pane| pane.0 == "scene");
    if camera.is_active != active {
        camera.is_active = active;
    }
}

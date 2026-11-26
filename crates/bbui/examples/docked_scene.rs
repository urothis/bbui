//! A live Bevy scene mounted inside a BBUI dock pane.
//!
//! Drag the splitter or switch tabs to see the camera follow its pane. Right-drag
//! to orbit, middle-drag to pan, and scroll over the scene to zoom.

use bbui::prelude::*;
use bbui::widgets::WidgetNode;
use bbui_core::{UiFocus, UiKey};
use bbui_layout::{Rect, UiEdges, UiLayoutResult};
use bevy::camera::{ClearColorConfig, Viewport};
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
use bevy::prelude::*;
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

/// Convert the clipped logical pane rectangle to Bevy's physical viewport.
fn physical_viewport(rect: Rect, scale: f32, size: UVec2) -> Option<Viewport> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let left = (rect.x * scale).round().clamp(0.0, size.x as f32) as u32;
    let top = (rect.y * scale).round().clamp(0.0, size.y as f32) as u32;
    let right = ((rect.x + rect.width) * scale)
        .round()
        .clamp(0.0, size.x as f32) as u32;
    let bottom = ((rect.y + rect.height) * scale)
        .round()
        .clamp(0.0, size.y as f32) as u32;
    (right > left && bottom > top).then_some(Viewport {
        physical_position: UVec2::new(left, top),
        physical_size: UVec2::new(right - left, bottom - top),
        depth: 0.0..1.0,
    })
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn pane_viewport_uses_physical_pixels_and_clips_to_window() {
        let viewport = physical_viewport(
            Rect::new(20.0, 12.0, 160.0, 90.0),
            1.5,
            UVec2::new(300, 200),
        )
        .unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(30, 18));
        assert_eq!(viewport.physical_size, UVec2::new(240, 135));
        let clipped = physical_viewport(
            Rect::new(180.0, 100.0, 80.0, 80.0),
            1.5,
            UVec2::new(300, 200),
        )
        .unwrap();
        assert_eq!(clipped.physical_size, UVec2::new(30, 50));
        assert!(
            physical_viewport(Rect::new(300.0, 0.0, 20.0, 20.0), 1.0, UVec2::new(300, 200))
                .is_none()
        );
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

fn sync_scene_viewport(
    window: Single<&Window, With<PrimaryWindow>>,
    workspace: Res<Workspace>,
    surfaces: Query<(&UiKey, &UiLayoutResult)>,
    mut camera: Single<&mut Camera, With<DockedSceneCamera>>,
) {
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
    camera.viewport = rect.and_then(|rect| {
        physical_viewport(
            rect,
            window.scale_factor(),
            UVec2::new(
                window.resolution.physical_width(),
                window.resolution.physical_height(),
            ),
        )
    });
    camera.is_active = camera.viewport.is_some();
}

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
    if !active || modals.is_open(window_entity) {
        orbit.drag = None;
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Camera {
            // Draw after the opaque dock group background. The viewport confines
            // the scene to its content rect; tab chrome and the inspector stay outside it.
            order: 2,
            is_active: false,
            ..default()
        },
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
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("primary window");
    let shell = WidgetTree::new(
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
    .spawn(world, None)
    .expect("shell");
    let dimensions = world.get::<Window>(window).unwrap().resolution.size();
    world.entity_mut(shell).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, dimensions.x, dimensions.y),
        1.0,
    ));
    let host = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "scene:workspace-host").then_some(entity))
        .expect("workspace host");
    let mut workspace = Workspace::new(WorkspaceLayout::new(DockNode::Split {
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
    }))
    .expect("workspace layout");
    workspace.set_mount_parent(host);
    workspace.register_pane("scene", "Scene", |world, parent| {
        WidgetTree::new(
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
        .expect("scene pane");
    });
    workspace.register_pane("notes", "Notes", |world, parent| {
        WidgetTree::new(WidgetNode::label(
            "The scene camera sleeps while this tab is open.",
        ))
        .spawn(world, Some(parent))
        .expect("notes pane");
    });
    workspace.register_pane("inspector", "Inspector", |world, parent| {
        WidgetTree::new(
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
        .expect("inspector pane");
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
        .add_systems(Startup, (setup, install_workspace))
        .add_systems(Update, scene_input)
        .add_systems(PostUpdate, sync_scene_viewport)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_clips_and_scales_without_leaving_window() {
        let viewport =
            physical_viewport(Rect::new(-5.0, 10.0, 75.0, 40.0), 2.0, UVec2::new(100, 80)).unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(0, 20));
        assert_eq!(viewport.physical_size, UVec2::new(100, 60));
        assert!(
            physical_viewport(Rect::new(200.0, 0.0, 20.0, 20.0), 1.0, UVec2::new(100, 80))
                .is_none()
        );
    }
}

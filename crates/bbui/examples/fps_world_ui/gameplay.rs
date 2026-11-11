//! Route FPS movement, shooting, and console edits into range state.

use bbui::WorldPointerState;
use bbui_core::UiFocus;
use bbui_widgets::{
    Label, Progress, TextArea, TextField, WidgetEvent, WidgetEventQueue, actions::WidgetValue,
};
use bevy::{
    camera::Camera,
    input::mouse::{MouseButton, MouseMotion},
    pbr::{MeshMaterial3d, StandardMaterial},
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility},
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use super::{
    ConsoleUi, Crosshair, GameState, Player, RangeLamp, RangeTarget, TARGET_COUNT, property_editor,
};

type TextControls = Or<(With<TextField>, With<TextArea>)>;

pub(super) fn player_input(
    mut motion: MessageReader<MouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    focus: Res<UiFocus>,
    text_fields: Query<(), TextControls>,
    mut player: Single<(&mut Transform, &mut GlobalTransform, &mut Player)>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    // Escape belongs to the focused UI control (cancel, close popup, or undo an edit).
    if keys.just_pressed(KeyCode::F1) {
        let locked = cursor.grab_mode != CursorGrabMode::None;
        cursor.grab_mode = if locked {
            CursorGrabMode::None
        } else {
            CursorGrabMode::Locked
        };
        cursor.visible = locked;
    }
    let typing = focus
        .focused
        .is_some_and(|entity| text_fields.get(entity).is_ok());
    let (transform, global_transform, look) = &mut *player;
    if cursor.grab_mode != CursorGrabMode::None && !typing {
        let mut camera_changed = false;
        for event in motion.read() {
            camera_changed |= event.delta != Vec2::ZERO;
            look.yaw -= event.delta.x * 0.0025;
            look.pitch = (look.pitch - event.delta.y * 0.0025).clamp(-1.45, 1.45);
        }
        if camera_changed {
            transform.rotation =
                Quat::from_rotation_y(look.yaw) * Quat::from_rotation_x(look.pitch);
        }
        let forward = Vec3::new(-look.yaw.sin(), 0.0, -look.yaw.cos());
        let right = Vec3::new(look.yaw.cos(), 0.0, -look.yaw.sin());
        let mut direction = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            direction += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            direction -= forward;
        }
        if keys.pressed(KeyCode::KeyD) {
            direction += right;
        }
        if keys.pressed(KeyCode::KeyA) {
            direction -= right;
        }
        if let Some(direction) = direction.try_normalize() {
            let speed = if keys.pressed(KeyCode::ShiftLeft) {
                5.0
            } else {
                2.7
            };
            transform.translation += direction * speed * time.delta_secs();
            transform.translation.x = transform.translation.x.clamp(-4.5, 4.5);
            transform.translation.z = transform.translation.z.clamp(-6.5, 5.6);
            camera_changed = true;
        }
        // Input routing runs in Update, before Bevy's normal transform propagation.
        // Keep the ray used for this frame's crosshair aligned with the moved camera.
        if camera_changed {
            **global_transform = GlobalTransform::from(**transform);
        }
    } else {
        for _ in motion.read() {}
    }
}

pub(super) fn shoot_targets(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    pointers: Res<WorldPointerState>,
    player: Single<(&Camera, &GlobalTransform), With<Player>>,
    mut ray_cast: MeshRayCast,
    mut targets: Query<(&mut RangeTarget, &mut MeshMaterial3d<StandardMaterial>)>,
    mut game: ResMut<GameState>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    if pointers.hover_target(window).is_some() {
        return;
    }
    let (camera, transform) = *player;
    let Some(viewport) = camera.logical_viewport_rect() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, viewport.center()) else {
        return;
    };
    let settings = MeshRayCastSettings::default().with_visibility(RayCastVisibility::Visible);
    let Some((entity, hit)) = ray_cast.cast_ray(ray, &settings).first() else {
        return;
    };
    if hit.distance > 30.0 {
        return;
    }
    let Ok((mut target, mut material)) = targets.get_mut(*entity) else {
        return;
    };
    if target.alive {
        target.alive = false;
        material.0 = game.hit_material.clone();
        game.score += 1;
    }
}

pub(super) fn sync_crosshair(
    pointers: Res<WorldPointerState>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut crosshair: Single<&mut BackgroundColor, With<Crosshair>>,
) {
    if !pointers.is_changed() {
        return;
    }
    let color = if pointers.hover_target(*window).is_some() {
        Color::srgb(0.27, 0.72, 1.0)
    } else {
        Color::WHITE
    };
    if crosshair.0 != color {
        crosshair.0 = color;
    }
}

pub(super) fn apply_console(world: &mut World) {
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let Some(ui) = world.get_resource::<ConsoleUi>().copied() else {
        return;
    };
    for event in &events {
        match event {
            WidgetEvent::ValueChanged {
                target,
                after: WidgetValue::Bool(value),
                ..
            } if *target == ui.power => world.resource_mut::<GameState>().lights_on = *value,
            WidgetEvent::ValueChanged {
                target,
                after: WidgetValue::Number(value),
                ..
            } if *target == ui.brightness => {
                world.resource_mut::<GameState>().brightness = *value as f32;
            }
            WidgetEvent::Activated { target } if *target == ui.reset => {
                let ready_material = world.resource::<GameState>().ready_material.clone();
                world.resource_mut::<GameState>().score = 0;
                let mut targets =
                    world.query::<(&mut RangeTarget, &mut MeshMaterial3d<StandardMaterial>)>();
                for (mut target, mut material) in targets.iter_mut(world) {
                    target.alive = true;
                    material.0 = ready_material.clone();
                }
            }
            _ => {}
        }
    }
    property_editor::process_events(world, events);
}

pub(super) fn sync_game_ui(
    game: Res<GameState>,
    ui: Res<ConsoleUi>,
    mut labels: Query<&mut Label>,
    mut progress: Query<&mut Progress>,
    mut lights: Query<&mut PointLight, With<RangeLamp>>,
) {
    if !game.is_changed() {
        return;
    }
    if let Ok(mut label) = labels.get_mut(ui.score) {
        label.text = format!("Hits: {} / {TARGET_COUNT}", game.score);
    }
    if let Ok(mut bar) = progress.get_mut(ui.progress) {
        bar.set(game.score as f32 / TARGET_COUNT as f32);
    }
    for mut light in &mut lights {
        light.intensity = if game.lights_on {
            500_000.0 * game.brightness
        } else {
            0.0
        };
    }
}

//! Run with `cargo run -p bbui --example fps_world_ui`.
//!
//! WASD moves, mouse looks, left click shoots targets or interacts with the
//! in-world properties editor. Drag its controls by moving the crosshair while held.
//! Escape releases or recaptures the mouse.

#[path = "fps_world_ui/gameplay.rs"]
mod gameplay;
#[path = "properties_editor/shared.rs"]
mod property_editor;
#[path = "fps_world_ui/scene.rs"]
mod scene;

use bbui::{BbuiPlugin, bridge_world_pointer};
use bbui_core::UiSystems;
use bevy::{pbr::StandardMaterial, prelude::*, window::WindowResolution};

const PANEL_WIDTH: u32 = property_editor::WIDTH as u32;
const PANEL_HEIGHT: u32 = property_editor::HEIGHT as u32;
const TARGET_COUNT: usize = 5;

#[derive(Component)]
struct Player {
    yaw: f32,
    pitch: f32,
}

#[derive(Component)]
struct RangeTarget {
    alive: bool,
}

#[derive(Component)]
struct RangeLamp;

#[derive(Component)]
struct Crosshair;

#[derive(Resource)]
struct GameState {
    score: usize,
    lights_on: bool,
    brightness: f32,
    ready_material: Handle<StandardMaterial>,
    hit_material: Handle<StandardMaterial>,
}

#[derive(Resource, Clone, Copy)]
struct ConsoleUi {
    power: Entity,
    brightness: Entity,
    reset: Entity,
    score: Entity,
    progress: Entity,
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    file_path: property_editor::EXAMPLE_ASSET_PATH.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "BBUI · FPS world-space properties editor".into(),
                        resolution: WindowResolution::new(1600, 900),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, scene::setup)
        .add_systems(
            Update,
            gameplay::player_input
                .in_set(UiSystems::Input)
                .before(bridge_world_pointer),
        )
        .add_systems(Update, gameplay::shoot_targets.after(UiSystems::Input))
        .add_systems(Update, gameplay::sync_crosshair.after(UiSystems::Input))
        .add_systems(
            Update,
            (gameplay::apply_console, gameplay::sync_game_ui)
                .chain()
                .after(bbui_widgets::actions::collect_widget_paint),
        )
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bbui::widgets::{WidgetEvent, WidgetEventQueue, actions::WidgetValue};
    use bbui_core::{UiKey, UiVisible};
    use bbui_render::UiWorldSurface;
    use bevy::{
        asset::AssetPlugin,
        clipboard::ClipboardPlugin,
        input::InputPlugin,
        window::{CursorOptions, PrimaryWindow},
    };

    fn keyed(world: &World, key: &str) -> Option<Entity> {
        world.iter_entities().find_map(|entry| {
            entry
                .get::<UiKey>()
                .is_some_and(|value| value.0 == key)
                .then_some(entry.id())
        })
    }

    #[test]
    fn world_editor_mounts_full_pages_and_applies_range_controls() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(WindowPlugin {
                primary_window: None,
                ..default()
            })
            .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
            .add_plugins(BbuiPlugin);
        app.world_mut().init_resource::<Assets<StandardMaterial>>();
        app.world_mut()
            .spawn((Window::default(), PrimaryWindow, CursorOptions::default()));
        scene::setup(app.world_mut());
        app.update();

        let world = app.world();
        assert_eq!(
            world
                .iter_entities()
                .filter(|entry| entry.get::<UiWorldSurface>().is_some())
                .count(),
            1
        );
        let object = keyed(world, "properties-object-panels").expect("object page");
        let modifiers = keyed(world, "page-modifiers").expect("modifiers page");
        let tab = keyed(world, "tab-modifiers").expect("modifiers tab");
        let power = keyed(world, "range-power").expect("range power");
        let brightness = keyed(world, "range-brightness").expect("range brightness");
        let reset = keyed(world, "range-reset").expect("range reset");
        assert!(
            world
                .get::<UiVisible>(object)
                .is_none_or(|visible| visible.0)
        );
        assert_eq!(
            world.get::<UiVisible>(modifiers).map(|visible| visible.0),
            Some(false)
        );

        app.world_mut().resource_mut::<GameState>().score = 2;
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .extend([
                WidgetEvent::Activated { target: tab },
                WidgetEvent::ValueChanged {
                    target: power,
                    before: WidgetValue::Bool(true),
                    after: WidgetValue::Bool(false),
                },
                WidgetEvent::ValueChanged {
                    target: brightness,
                    before: WidgetValue::Number(0.7),
                    after: WidgetValue::Number(0.4),
                },
                WidgetEvent::Activated { target: reset },
            ]);
        gameplay::apply_console(app.world_mut());
        let world = app.world();
        assert_eq!(
            world.get::<UiVisible>(modifiers).map(|visible| visible.0),
            Some(true)
        );
        assert_eq!(
            world.get::<UiVisible>(object).map(|visible| visible.0),
            Some(false)
        );
        let game = world.resource::<GameState>();
        assert!(!game.lights_on);
        assert_eq!(game.brightness, 0.4);
        assert_eq!(game.score, 0);
    }
}

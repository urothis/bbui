use super::*;
use bbui_core::{UiChildren, UiFocus, UiInputEvent, UiInputQueue, UiKey, UiPointerButton};
use bbui_layout::UiLayoutResult;
use bbui_render::{DisplayList, Primitive};
use bbui_widgets::{DockNode, SplitAxis, TabGroup, WidgetNode, WidgetTree};
use bevy::{
    asset::AssetPlugin,
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    window::{PrimaryWindow, WindowPlugin},
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(crate::BbuiPlugin);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app
}

fn keyed(world: &mut World, key: &str) -> Entity {
    let mut query = world.query::<(Entity, &UiKey)>();
    query
        .iter(world)
        .find(|(_, candidate)| candidate.0 == key)
        .map(|(entity, _)| entity)
        .expect("workspace key")
}

fn center(app: &App, entity: Entity) -> [f32; 2] {
    let rect = app.world().get::<UiLayoutResult>(entity).unwrap().rect;
    [rect.x + rect.width * 0.5, rect.y + rect.height * 0.5]
}

fn push_pointer(app: &mut App, event: UiInputEvent) {
    app.world_mut().resource_mut::<UiInputQueue>().push(event);
    app.update();
}

mod lifecycle;
mod pointer;

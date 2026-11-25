//! Run with `cargo run -p bbui --example modal_dialog`.
//! Tab stays within the dialog; Enter chooses an action, Escape dismisses it.

use bbui::prelude::*;
use bbui::widgets::{WidgetEvent, WidgetEventQueue};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "modal_dialog/capture.rs"]
mod capture;

#[derive(Resource)]
struct EditButton(Entity);

fn confirmation() -> ModalDialog {
    ModalDialog::new(
        "Unsaved changes",
        "Save your project before closing?",
        vec![
            ModalAction::new("save", "Save"),
            ModalAction::new("discard", "Discard"),
            ModalAction::new("cancel", "Cancel"),
        ],
    )
}

fn install_dialog(world: &mut World) {
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        error!("Modal example requires a primary window");
        return;
    };
    let page = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("Project editor"),
            WidgetNode::button("Edit project").with_key("edit-project"),
            WidgetNode::label("The dialog blocks this page while it is open."),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
    .spawn(world, None);
    let Ok(page) = page else {
        error!("Could not mount example page: {page:?}");
        return;
    };
    let Some(window_state) = world.get::<Window>(window) else {
        return;
    };
    let resolution = &window_state.resolution;
    let dimensions = [resolution.width(), resolution.height()];
    world.entity_mut(page).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, dimensions[0], dimensions[1]),
        1.0,
    ));
    let mut buttons = world.query::<(Entity, &UiKey)>();
    if let Some(button) = buttons
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "edit-project").then_some(entity))
    {
        world.insert_resource(EditButton(button));
    }
    if let Err(error) = confirmation().show(world, window) {
        error!("Could not open dialog: {error:?}");
    }
}

fn reopen_dialog(world: &mut World) {
    let Some(button) = world.get_resource::<EditButton>().map(|button| button.0) else {
        return;
    };
    let activated = world
        .resource_mut::<WidgetEventQueue>()
        .take()
        .into_iter()
        .any(|event| matches!(event, WidgetEvent::Activated { target } if target == button));
    if !activated {
        return;
    }
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        return;
    };
    if let Err(error) = confirmation().show(world, window) {
        error!("Could not reopen dialog: {error:?}");
    }
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_dialog(world);
}

fn report(mut events: MessageReader<ModalEvent>) {
    for event in events.read() {
        info!("Dialog decision: {:?}", event.dismissal);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, report)
        .add_systems(PostUpdate, reopen_dialog)
        .run();
}

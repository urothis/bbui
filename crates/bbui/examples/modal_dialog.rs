//! Run with `cargo run -p bbui --example modal_dialog`.
//! Tab stays within the dialog; Enter chooses an action, Escape dismisses it.

use bbui::prelude::*;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let window = windows.single(world).expect("primary window");
    let page = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("Project editor"),
            WidgetNode::button("Edit project"),
            WidgetNode::label("The dialog blocks this page while it is open."),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
    .spawn(world, None)
    .expect("page");
    let resolution = &world.get::<Window>(window).unwrap().resolution;
    let dimensions = [resolution.width(), resolution.height()];
    world.entity_mut(page).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, dimensions[0], dimensions[1]),
        1.0,
    ));
    ModalDialog::new(
        "Unsaved changes",
        "Save your project before closing?",
        vec![
            ModalAction::new("save", "Save"),
            ModalAction::new("discard", "Discard"),
            ModalAction::new("cancel", "Cancel"),
        ],
    )
    .show(world, window)
    .expect("dialog");
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
        .run();
}

//! Search and activate application commands with BBUI widgets.
//! Run with `cargo run -p bbui --example command_palette`.

use bbui::prelude::*;
use bbui::widgets::{Command, CommandPalette, CommandRegistry, WidgetEventQueue};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "command_palette/capture.rs"]
mod capture;

#[derive(Resource)]
struct ExampleState {
    registry: CommandRegistry,
    palette: CommandPalette,
    root: Entity,
}

fn page(palette: &CommandPalette, registry: &CommandRegistry) -> WidgetTree {
    let mut layout = UiLayoutNode::column(0.0);
    layout.align_items = UiAlign::Center;
    layout.justify_content = UiJustify::Center;
    WidgetTree::new(
        WidgetNode::column([WidgetNode::column([
            WidgetNode::label("Command palette").with_role("modal-title"),
            WidgetNode::label("Type to search, then choose an action."),
            palette.view(registry),
        ])
        .with_width(UiLength::Px(460.0))
        .with_gap(12.0)
        .with_padding(UiEdges::all(20.0))
        .with_role("box")])
        .with_layout(layout)
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
}

fn install_palette(world: &mut World) {
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        return;
    };
    let Some(size) = world
        .get::<Window>(window)
        .map(|state| [state.resolution.width(), state.resolution.height()])
    else {
        return;
    };
    let mut registry = CommandRegistry::default();
    for (id, label, category) in [
        ("new", "New project", "File"),
        ("open", "Open project", "File"),
        ("save", "Save project", "File"),
        ("export", "Export image", "File"),
        ("undo", "Undo change", "Edit"),
        ("redo", "Redo change", "Edit"),
    ] {
        let mut command = Command::new(id, label);
        command.category = category.into();
        if let Err(error) = registry.register(command) {
            error!("Command registration failed: {error:?}");
        }
    }
    let mut palette = CommandPalette::default();
    palette.show();
    let Ok(root) = page(&palette, &registry).spawn(world, None) else {
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, size[0], size[1]),
        1.0,
    ));
    world.insert_resource(ExampleState {
        registry,
        palette,
        root,
    });
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_palette(world);
}

fn process(world: &mut World) {
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let Some(mut state) = world.remove_resource::<ExampleState>() else {
        return;
    };
    let mut changed = false;
    for event in &events {
        if state
            .palette
            .handle_widget_event(world, event, &mut state.registry)
            .is_some()
        {
            changed = true;
        }
        if matches!(event, bbui::widgets::WidgetEvent::TextChanged { .. }) {
            changed = true;
        }
    }
    if changed
        && let Err(error) = page(&state.palette, &state.registry).reconcile(world, state.root)
    {
        error!("Palette update failed: {error:?}");
    }
    world.insert_resource(state);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, process)
        .run();
}

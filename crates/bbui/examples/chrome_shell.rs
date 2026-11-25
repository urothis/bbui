//! Desktop and compact mobile application chrome in one window.
//! Run with `cargo run -p bbui --example chrome_shell`.

use bbui::prelude::*;
use bbui::widgets::WidgetEventQueue;
use bbui::widgets::{Command, CommandRegistry};
use bbui_core::UiRoutedEvents;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "chrome_shell/capture.rs"]
mod capture;

fn install_chrome(world: &mut World) {
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
    let mut commands = CommandRegistry::default();
    for (id, label) in [("new", "New"), ("open", "Open"), ("save", "Save")] {
        if let Err(error) = commands.register(Command::new(id, label)) {
            error!("Command failed: {error:?}");
        }
    }
    let desktop = DesktopChrome::new("Project Editor");
    let mut mobile = MobileChrome::new(
        "Project Editor",
        vec![
            MobileNavItem::new("files", "Files"),
            MobileNavItem::new("scene", "Scene"),
            MobileNavItem::new("tools", "Tools"),
        ],
    );
    mobile.navigate("files", "Files");
    let page = WidgetTree::new(
        WidgetNode::row([
            WidgetNode::column([
                desktop.view(&commands),
                WidgetNode::label("Desktop work area").with_width(UiLength::Fill(1.0)),
                WidgetNode::label("The title region starts a native window drag on press."),
            ])
            .with_width(UiLength::Px(620.0))
            .with_height(UiLength::Fill(1.0)),
            mobile
                .view(
                    &commands,
                    WidgetNode::column([
                        WidgetNode::label("Compact work area"),
                        WidgetNode::label("Touch-friendly navigation stays at the bottom."),
                    ])
                    .with_padding(UiEdges::all(12.0)),
                    Some(
                        WidgetNode::column([
                            WidgetNode::button("Workspace"),
                            WidgetNode::button("Assets"),
                            WidgetNode::button("Exports"),
                        ])
                        .with_gap(8.0),
                    ),
                )
                .with_width(UiLength::Px(360.0)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    );
    let Ok(root) = page.spawn(world, None) else {
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, size[0], size[1]),
        1.0,
    ));
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_chrome(world);
}

#[derive(Resource, Default)]
struct MaximizeState(bool);

fn process_chrome(world: &mut World) {
    let routed = world.resource::<UiRoutedEvents>().events.clone();
    for event in &routed {
        handle_chrome_press(world, event);
    }
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        return;
    };
    for event in &events {
        let Some(action) = chrome_action(world, event) else {
            continue;
        };
        if matches!(action, ChromeAction::ToggleMaximize) {
            let current = world.resource::<MaximizeState>().0;
            world.resource_mut::<MaximizeState>().0 = !current;
        }
        let maximized = world.resource::<MaximizeState>().0;
        if let Some(mut window_state) = world.get_mut::<Window>(window) {
            apply_native_chrome_action(&mut window_state, !maximized, &action);
        }
        info!("Chrome action: {action:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BbuiPlugin)
        .init_resource::<MaximizeState>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, process_chrome)
        .run();
}

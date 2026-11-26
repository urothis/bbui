//! Floating toasts within an application shell.
//! Run with `cargo run -p bbui --example toasts`.

use bbui::prelude::*;
use bbui::widgets::{Command, CommandRegistry, WidgetEventQueue};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use std::time::Duration;

#[cfg(test)]
#[path = "toasts/capture.rs"]
mod capture;
#[path = "editor_workspace/content.rs"]
mod editor_content;

#[derive(Resource)]
struct ExampleState {
    window: Entity,
    generation: u64,
}

fn page(center: &mut ToastCenter, window: Entity) -> WidgetTree {
    let mut brand_style = UiInlineStyle::default();
    brand_style.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::Accent);
    let mut muted = UiInlineStyle::default();
    muted.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::MutedText);
    muted.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(12.5));
    let center_title = WidgetNode::row([WidgetNode::label("Material.001")
        .with_role("centered-label")
        .with_width(UiLength::Fill(1.0))
        .with_style(muted)])
    .with_layout(UiLayoutNode {
        align_items: UiAlign::Center,
        ..UiLayoutNode::row(0.0)
    })
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0));
    let mut registry = CommandRegistry::default();
    for (id, label) in [("save", "Save"), ("export", "Export")] {
        if let Err(error) = registry.register(Command::new(id, label)) {
            error!("Command error: {error:?}");
        }
    }
    WidgetTree::new(
        WidgetNode::column([
            DesktopChrome::new("Material Studio").view_with_slots(
                &registry,
                &DesktopChromeAppearance::default(),
                DesktopChromeSlots {
                    leading: Some(
                        WidgetNode::label("◆")
                            .with_width(UiLength::Px(18.0))
                            .with_style(brand_style),
                    ),
                    center: Some(center_title),
                    trailing: None,
                },
            ),
            center
                .floating_view(
                    window,
                    WidgetNode::column([])
                        .with_key("toast:workspace")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Fill(1.0)),
                    &ToastAppearance::default(),
                    &ToastOverlayAppearance::default(),
                )
                .with_height(UiLength::Fill(1.0)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
}

fn install_editor(world: &mut World, host: Entity) {
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "editor-columns".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![18.0, 63.0, 19.0],
        children: vec![
            DockNode::Tabs(TabGroup::new("scene-group", vec!["scene".into()])),
            DockNode::Split {
                id: "editor-center".into(),
                axis: SplitAxis::Vertical,
                shares: vec![3.0, 1.0],
                children: vec![
                    DockNode::Tabs(TabGroup::new("graph-group", vec!["graph".into()])),
                    DockNode::Tabs(TabGroup::new("console-group", vec!["console".into()])),
                ],
            },
            DockNode::Tabs(TabGroup::new("properties-group", vec!["properties".into()])),
        ],
    });
    let Ok(mut workspace) = Workspace::new(layout) else {
        return;
    };
    workspace.set_mount_parent(host);
    for (id, title) in [
        ("scene", "Scene"),
        ("graph", "Material Graph"),
        ("properties", "Properties"),
        ("console", "Console"),
    ] {
        workspace.register_pane(id, title, move |world, parent| {
            editor_content::mount(world, parent, id)
        });
    }
    world.insert_resource(workspace);
}

fn install_toasts(world: &mut World) {
    editor_content::configure_theme(&mut world.resource_mut::<Theme>());
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
    let mut center = world.resource_mut::<ToastCenter>();
    center.post(
        window,
        ToastSeverity::Info,
        Some("Workspace".into()),
        "Project loaded",
        ToastLifetime::For(Duration::from_secs(30)),
    );
    center.post(
        window,
        ToastSeverity::Success,
        Some("Export".into()),
        "Preview ready",
        ToastLifetime::Persistent,
    );
    center.post(
        window,
        ToastSeverity::Warning,
        Some("Storage".into()),
        "Low disk space",
        ToastLifetime::Persistent,
    );
    let tree = page(&mut world.resource_mut::<ToastCenter>(), window);
    let Ok(root) = tree.spawn(world, None) else {
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, size[0], size[1]),
        1.0,
    ));
    let mut keys = world.query::<(Entity, &UiKey)>();
    if let Some(host) = keys
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "toast:workspace").then_some(entity))
    {
        install_editor(world, host);
    }
    let generation = world.resource::<ToastCenter>().generation();
    world.insert_resource(ExampleState { window, generation });
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_toasts(world);
}

fn process(world: &mut World) {
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let Some(mut state) = world.remove_resource::<ExampleState>() else {
        return;
    };
    let Some(mut center) = world.remove_resource::<ToastCenter>() else {
        world.insert_resource(state);
        return;
    };
    for event in &events {
        if let Some(toast) = center.handle_widget_event(world, event) {
            info!("Dismissed toast {}", toast.id);
        }
    }
    if state.generation != center.generation() {
        let mut keys = world.query::<(Entity, &UiKey)>();
        if let Some(stack) = keys
            .iter(world)
            .find_map(|(entity, key)| (key.0 == "toast:center").then_some(entity))
            && let Err(error) = WidgetTree::new(center.floating_stack(
                state.window,
                &ToastAppearance::default(),
                &ToastOverlayAppearance::default(),
            ))
            .reconcile(world, stack)
        {
            error!("Toast update failed: {error:?}");
        }
    }
    state.generation = center.generation();
    world.insert_resource(center);
    world.insert_resource(state);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Material Studio".into(),
                resolution: WindowResolution::new(1536, 900),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, process)
        .run();
}

//! A small editor assembled from BBUI's dockable workspace and editor widgets.
//!
//! Run with `cargo run -p bbui --example editor_workspace`.
//! Drag tabs to dock panes, drag dividers to resize, and use F1–F4 to reopen panes.
//! F5 saves the layout to `target/editor-workspace-layout.json`; F9 restores it.

use bbui::prelude::*;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use std::path::PathBuf;

#[cfg(test)]
#[path = "editor_workspace/capture.rs"]
mod capture;
#[path = "editor_workspace/content.rs"]
mod content;

const LAYOUT_PATH: &str = "target/editor-workspace-layout.json";
const PANES: [(KeyCode, &str); 4] = [
    (KeyCode::F1, "scene"),
    (KeyCode::F2, "graph"),
    (KeyCode::F3, "properties"),
    (KeyCode::F4, "console"),
];

fn default_layout() -> WorkspaceLayout {
    WorkspaceLayout::new(DockNode::Split {
        id: "editor-columns".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![19.0, 56.0, 25.0],
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
    })
}

fn first_group(node: &DockNode) -> Option<&str> {
    match node {
        DockNode::Tabs(group) => Some(&group.id),
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
            children.iter().find_map(first_group)
        }
    }
}

fn contains_pane(node: &DockNode, pane: &str) -> bool {
    match node {
        DockNode::Tabs(group) => group.panes.iter().any(|id| id.0 == pane),
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
            children.iter().any(|child| contains_pane(child, pane))
        }
    }
}

fn layout_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .join(LAYOUT_PATH)
}

fn shortcuts(keys: Res<ButtonInput<KeyCode>>, mut workspace: ResMut<Workspace>) {
    if keys.just_pressed(KeyCode::F5) {
        let path = layout_path();
        match serde_json::to_string_pretty(workspace.layout())
            .map_err(|error| error.to_string())
            .and_then(|json| std::fs::write(&path, json).map_err(|error| error.to_string()))
        {
            Ok(()) => info!("Saved workspace layout to {}", path.display()),
            Err(error) => warn!("Could not save workspace layout: {error}"),
        }
    }
    if keys.just_pressed(KeyCode::F9) {
        let path = layout_path();
        match std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|json| {
                serde_json::from_str::<WorkspaceLayout>(&json).map_err(|error| error.to_string())
            })
            .and_then(|layout| {
                workspace
                    .restore(layout)
                    .map_err(|error| format!("{error:?}"))
            }) {
            Ok(()) => info!("Restored workspace layout from {}", path.display()),
            Err(error) => warn!("Could not restore workspace layout: {error}"),
        }
    }
    let Some(pane) = PANES
        .into_iter()
        .find_map(|(key, pane)| keys.just_pressed(key).then_some(pane))
    else {
        return;
    };
    if contains_pane(&workspace.layout().root, pane) {
        return;
    }
    let Some(group) = first_group(&workspace.layout().root).map(str::to_owned) else {
        return;
    };
    if let Err(error) = workspace.apply(WorkspaceOperation::Open {
        pane: pane.into(),
        group,
        index: usize::MAX,
    }) {
        warn!("Could not open {pane}: {error:?}");
    }
}

fn report_workspace_events(mut events: MessageReader<WorkspaceEvent>) {
    for event in events.read() {
        info!("Workspace changed: {event:?}");
    }
}

fn install_workspace(world: &mut World) {
    content::configure_theme(&mut world.resource_mut::<Theme>());
    let Ok(mut workspace) = Workspace::new(default_layout()) else {
        error!("Could not create editor workspace");
        return;
    };
    for (id, title) in [
        ("scene", "Scene"),
        ("graph", "Material Graph"),
        ("properties", "Properties"),
        ("console", "Console"),
    ] {
        workspace.register_pane(id, title, move |world, parent| {
            content::mount(world, parent, id);
        });
    }
    world.insert_resource(workspace);
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_workspace(world);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Editor Workspace".into(),
                resolution: WindowResolution::new(1536, 900),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (shortcuts, report_workspace_events))
        .run();
}

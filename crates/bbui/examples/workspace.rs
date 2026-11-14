//! Run with `cargo run -p bbui --example workspace`.
//! Drag a tab to another group, to a group edge to split, or outside to detach.
//! Drag a divider to resize. Close buttons unmount panes; F1–F4 reopen them.

use bbui::prelude::*;
use bevy::prelude::*;
use bevy::window::WindowResolution;

#[path = "workspace/content.rs"]
mod content;

fn first_group(node: &DockNode) -> Option<&str> {
    match node {
        DockNode::Tabs(group) => Some(&group.id),
        DockNode::Split { children, .. } | DockNode::Grid { children, .. } => {
            children.iter().find_map(first_group)
        }
    }
}

fn reopen(keys: Res<ButtonInput<KeyCode>>, mut workspace: ResMut<Workspace>) {
    let pane = [
        (KeyCode::F1, "document"),
        (KeyCode::F2, "preview"),
        (KeyCode::F3, "inspector"),
        (KeyCode::F4, "console"),
    ]
    .into_iter()
    .find_map(|(key, pane)| keys.just_pressed(key).then_some(pane));
    let Some(pane) = pane else {
        return;
    };
    let Some(group) = workspace
        .layout()
        .windows
        .iter()
        .find(|window| window.id == "main")
        .and_then(|window| first_group(&window.root))
        .map(str::to_owned)
    else {
        return;
    };
    let _ = workspace.apply(WorkspaceOperation::Open {
        pane: pane.into(),
        group,
        index: usize::MAX,
    });
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    content::configure_theme(&mut world.resource_mut::<Theme>());
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "main-split".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![2.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new(
                "documents",
                vec!["document".into(), "preview".into()],
            )),
            DockNode::Grid {
                id: "tools-grid".into(),
                columns: 1,
                children: vec![
                    DockNode::Tabs(TabGroup::new("inspector", vec!["inspector".into()])),
                    DockNode::Tabs(TabGroup::new("console", vec!["console".into()])),
                ],
            },
        ],
    });
    let Ok(mut workspace) = Workspace::new(layout) else {
        return;
    };
    workspace.register_pane("document", "Document", |world, parent| {
        content::mount(world, parent, "document");
    });
    workspace.register_pane("preview", "Preview", |world, parent| {
        content::mount(world, parent, "preview");
    });
    workspace.register_pane("inspector", "Inspector", |world, parent| {
        content::mount(world, parent, "inspector");
    });
    workspace.register_pane("console", "Console", |world, parent| {
        content::mount(world, parent, "console");
    });
    world.insert_resource(workspace);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Workspace".into(),
                resolution: WindowResolution::new(1100, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, reopen)
        .run();
}

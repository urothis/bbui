//! Run with `cargo run -p bbui --example workspace`.
//! Drag a tab to another group, to a group edge to split, or outside to detach.
//! Drag a divider to resize. Close buttons unmount panes; F1–F4 reopen them.

use bbui::prelude::*;
use bevy::prelude::*;
use bevy::window::WindowResolution;

fn mount_page(world: &mut World, parent: Entity, heading: &str, detail: &str) {
    let node = WidgetNode::column([
        WidgetNode::label(heading).with_height(UiLength::Px(34.0)),
        WidgetNode::label(detail),
        WidgetNode::label("F1 Document · F2 Preview · F3 Inspector · F4 Console"),
        WidgetNode::text_area(
            "Notes",
            "This editable text keeps its live state when its pane moves.",
        )
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(120.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
    .with_padding(UiEdges::all(14.0));
    let _ = WidgetTree::new(node).spawn(world, Some(parent));
}

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
        mount_page(
            world,
            parent,
            "Document",
            "Drag this tab to a pane edge or out of the window.",
        );
    });
    workspace.register_pane("preview", "Preview", |world, parent| {
        mount_page(
            world,
            parent,
            "Preview",
            "Switch tabs to reveal this retained pane.",
        );
    });
    workspace.register_pane("inspector", "Inspector", |world, parent| {
        mount_page(
            world,
            parent,
            "Inspector",
            "Drag a tab here to combine panes.",
        );
    });
    workspace.register_pane("console", "Console", |world, parent| {
        mount_page(
            world,
            parent,
            "Console",
            "Drag the split divider to resize its neighbors.",
        );
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

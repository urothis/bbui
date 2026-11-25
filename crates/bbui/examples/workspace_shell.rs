//! A BBUI title/command row and status strip around an embedded dock workspace.
//! Run with `cargo run -p bbui --example workspace_shell`.

use bbui::prelude::*;
use bbui::widgets::WidgetNode;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        return;
    };
    let shell = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::row([
                WidgetNode::label("Project Editor").with_width(UiLength::Fill(1.0)),
                WidgetNode::button("Save"),
                WidgetNode::button("Export"),
            ])
            .with_key("shell:commands")
            .with_height(UiLength::Px(40.0)),
            WidgetNode::column([])
                .with_key("shell:workspace")
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0)),
            WidgetNode::row([
                WidgetNode::label("Ready").with_width(UiLength::Fill(1.0)),
                WidgetNode::progress(0.65).with_width(UiLength::Px(160.0)),
            ])
            .with_key("shell:status")
            .with_height(UiLength::Px(28.0)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
    .spawn(world, None)
    .expect("shell widgets");
    let resolution = &world.get::<Window>(window).unwrap().resolution;
    let dimensions = [resolution.width(), resolution.height()];
    world.entity_mut(shell).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, dimensions[0], dimensions[1]),
        1.0,
    ));
    let mut keys = world.query::<(Entity, &UiKey)>();
    let workspace_host = keys
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "shell:workspace").then_some(entity))
        .expect("workspace host");
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "columns".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![3.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new("viewport", vec!["terrain".into()])),
            DockNode::Tabs(TabGroup::new("inspector", vec!["properties".into()])),
        ],
    });
    let mut workspace = Workspace::new(layout).expect("workspace layout");
    workspace.set_mount_parent(workspace_host);
    workspace.register_pane("terrain", "Terrain", |world, parent| {
        WidgetTree::new(WidgetNode::label("Terrain viewport goes here"))
            .spawn(world, Some(parent))
            .expect("terrain pane");
    });
    workspace.register_pane("properties", "Properties", |world, parent| {
        WidgetTree::new(WidgetNode::label("Area properties"))
            .spawn(world, Some(parent))
            .expect("properties pane");
    });
    world.insert_resource(workspace);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Workspace Shell".into(),
                resolution: WindowResolution::new(1100, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

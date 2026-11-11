//! Run with `cargo run -p bbui --example showcase`.
//!
//! The controls are Bevy entities. BBUI lays them out, routes input, and paints them each frame.

use bbui::prelude::*;
use bbui::widgets::{
    CanvasShape, MenuEntry, NodeSocket, PreviewGridItem, SocketType, TreeNode, WidgetEventQueue,
    WidgetNode,
};
use bbui_layout::{Rect, UiEdges};
use bbui_media::{BbuiMediaPlayer, BbuiMediaSequence};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::{
    MotionCurve, UiAnimationSpec, UiAutoAnimations, UiReducedMotion, UiStyleProperty,
};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use std::time::Duration;

fn sample_canvas() -> Option<Canvas> {
    let mut canvas = Canvas::default();
    canvas.add(
        [32.0, 48.0],
        CanvasShape::Rectangle {
            size: [180.0, 120.0],
        },
        [0.21, 0.44, 0.75, 1.0],
    )?;
    canvas.add(
        [240.0, 92.0],
        CanvasShape::Ellipse {
            size: [170.0, 125.0],
        },
        [0.86, 0.36, 0.22, 0.9],
    )?;
    canvas.add(
        [470.0, 45.0],
        CanvasShape::Image {
            size: [150.0, 150.0],
            asset: "canvas/checker.svg".into(),
        },
        [1.0; 4],
    )?;
    Some(canvas)
}

fn sample_graph() -> Result<NodeGraph, String> {
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Value",
        [25.0, 60.0],
        vec![NodeSocket::output("Value", SocketType::Float)],
    );
    let output = graph.add_node(
        "Output",
        [340.0, 120.0],
        vec![NodeSocket::input("Value", SocketType::Float)],
    );
    graph
        .create_link(source, 0, output, 0)
        .map_err(|error| format!("sample graph link: {error:?}"))?;
    Ok(graph)
}

fn setup(world: &mut World) {
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("showcase requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 8.0,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(
                window,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 760.0,
                },
                1.0,
            ),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: background,
            }),
        ))
        .id();
    let mut tree = bbui! {
        column {
            label("BBUI: Bevy-native controls"),
            panel("Scene", true) {
                checkbox("Visible", true),
                checkbox("Selectable", false),
                number("Scale", 1.0, 0.0, 10.0, 0.1),
                number_unbounded("Offset", 0.0, 0.01),
                text_field("Name", "Cube"),
                text_area("Notes", "Multi-line text wraps and scrolls.\nUse IME, selection, and local undo here."),
            },
            row {
                button("Apply"),
                dropdown("Mode", ["Object", "Edit", "Sculpt"], Some(0)),
            },
            progress(0.4),
            label("Drag numbers; Shift is precise, Ctrl snaps. Tab moves focus."),
        }
    };
    let commands = [
        MenuEntry::submenu(
            "add",
            "Add",
            vec![
                MenuEntry::command("add_cube", "Add Cube"),
                MenuEntry::command("add_sphere", "Add Sphere"),
            ],
        ),
        MenuEntry::command("delete", "Delete"),
    ];
    tree.root.children.push(WidgetNode::row([
        WidgetNode::menu("Commands", commands.clone()),
        WidgetNode::pie_menu("Pie Commands", commands),
    ]));
    let assets = ListModel {
        items: ["Cube", "Light", "Camera", "Material"]
            .into_iter()
            .enumerate()
            .map(|(index, value)| ListItem {
                id: index as u64 + 1,
                value: value.into(),
                enabled: true,
            })
            .collect(),
        ..ListModel::default()
    };
    let scene_tree = TreeModel {
        nodes: vec![
            TreeNode {
                id: 1,
                parent: None,
                children: vec![2, 3],
                value: "Scene".into(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![],
                value: "Cube".into(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
                value: "Light".into(),
                expanded: false,
                enabled: true,
            },
        ],
        ..TreeModel::default()
    };
    let previews = PreviewGrid {
        items: ["Clay", "Metal", "Glass", "Fabric"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let mut item = PreviewGridItem::new(index as u64 + 1, label);
                item.tint = match index {
                    0 => [0.72, 0.48, 0.32, 1.0],
                    1 => [0.55, 0.62, 0.7, 1.0],
                    2 => [0.32, 0.7, 0.82, 1.0],
                    _ => [0.72, 0.28, 0.5, 1.0],
                };
                item
            })
            .collect(),
        ..PreviewGrid::default()
    };
    let table = DataTable {
        columns: vec![
            TableColumn::new("Object", 180.0),
            TableColumn::new("Type", 130.0),
        ],
        rows: vec![
            TableRow {
                id: 1,
                cells: vec!["Cube".into(), "Mesh".into()],
            },
            TableRow {
                id: 2,
                cells: vec!["Light".into(), "Lamp".into()],
            },
            TableRow {
                id: 3,
                cells: vec!["Camera".into(), "Camera".into()],
            },
        ],
        ..DataTable::default()
    };
    let Ok(browser_directory) = std::env::current_dir() else {
        eprintln!("showcase needs a working directory");
        return;
    };
    let Some(canvas) = sample_canvas() else {
        eprintln!("showcase canvas geometry is invalid");
        return;
    };
    let Ok(graph) = sample_graph() else {
        eprintln!("showcase graph links are invalid");
        return;
    };
    tree.root.children.extend([
        WidgetNode::boxed([
            WidgetNode::label("Controls, assets, and layout"),
            WidgetNode::label(
                "The right strip plays GIF, VP8 WebM, and H.264 MP4 through Bevy assets.",
            ),
            WidgetNode::property_row(
                "Opacity",
                WidgetNode::slider_range("Opacity", 0.7, 0.0, 1.0, 0.01),
            ),
            WidgetNode::radio_group(
                "Mode",
                RadioGroup::new(["Object", "Edit", "Sculpt"], Some(0)),
            ),
            WidgetNode::row([
                WidgetNode::icon_button_asset("canvas/checker.svg", "Checker icon"),
                WidgetNode::tooltip(
                    "This tooltip follows the icon",
                    WidgetNode::button("Hover me"),
                ),
                WidgetNode::label("\u{1F7D5}").with_key("showcase-font"),
            ]),
            WidgetNode::responsive_grid(
                140,
                3,
                [
                    WidgetNode::button("Grid A"),
                    WidgetNode::button("Grid B"),
                    WidgetNode::button("Grid C"),
                ],
            ),
        ]),
        WidgetNode::column([
            WidgetNode::label("Reorderable panels: drag headers or press Ctrl+Up/Down"),
            WidgetNode::panel(
                "Transform",
                true,
                [WidgetNode::number("X", 1.0, -10.0, 10.0, 0.1)],
            )
            .with_key("transform"),
            WidgetNode::panel("Material", false, [WidgetNode::text_field("Name", "Clay")])
                .with_key("material"),
        ])
        .with_key("showcase-panels"),
        WidgetNode::boxed([
            WidgetNode::label("Color and curve editors"),
            WidgetNode::hsv_color_default("Color"),
            WidgetNode::color_ramp_default("Ramp"),
            WidgetNode::curve_editor_default("Curve"),
        ]),
        WidgetNode::boxed([
            WidgetNode::label("Collections and table"),
            WidgetNode::row([
                WidgetNode::list("Assets", assets),
                WidgetNode::tree("Outliner", scene_tree),
                WidgetNode::preview_grid("Materials", previews),
            ]),
            WidgetNode::data_table("Objects", table),
        ]),
        WidgetNode::boxed([
            WidgetNode::label("Canvas and node graph"),
            WidgetNode::canvas("Canvas", canvas).with_height(UiLength::Px(300.0)),
            WidgetNode::node_graph("Nodes", graph).with_height(UiLength::Px(300.0)),
        ]),
        WidgetNode::boxed([
            WidgetNode::label("File browser"),
            WidgetNode::file_browser(browser_directory).with_height(UiLength::Px(320.0)),
        ]),
    ]);
    tree.root = WidgetNode::scroll([tree.root])
        .with_width(UiLength::Px(900.0))
        .with_height(UiLength::Fill(1.0));
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
        return;
    }
    let Some(group) = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "showcase-panels").then_some(entity))
    else {
        eprintln!("showcase panel group is missing");
        return;
    };
    world.entity_mut(group).insert(
        PanelGroup::with_panels(["transform", "material"]).with_reordering("showcase-panels"),
    );
    let Some(font_label) = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "showcase-font").then_some(entity))
    else {
        eprintln!("showcase font label is missing");
        return;
    };
    world
        .entity_mut(font_label)
        .insert(UiFontAsset::new("fonts/NotoSansSymbols2-Regular.ttf"));
    let asset_server = world.resource::<AssetServer>().clone();
    for (index, path) in [
        "media/loop.gif",
        "media/sample.webm",
        "media/sample-h264.mp4",
    ]
    .into_iter()
    .enumerate()
    {
        let sequence = asset_server.load::<BbuiMediaSequence>(path);
        world.spawn((
            bevy::ui::Node {
                position_type: bevy::ui::PositionType::Absolute,
                left: bevy::ui::Val::Px(950.0),
                top: bevy::ui::Val::Px(90.0 + index as f32 * 190.0),
                width: bevy::ui::Val::Px(280.0),
                height: bevy::ui::Val::Px(150.0),
                ..default()
            },
            ImageNode::default(),
            BbuiMediaPlayer::new(sequence),
        ));
    }
    let transition = UiAnimationSpec::new(Duration::from_millis(180)).curve(MotionCurve::EaseOut);
    let mut transitions = UiAutoAnimations::default();
    for property in [UiStyleProperty::BorderColor, UiStyleProperty::AccentColor] {
        transitions.set(property, transition);
    }
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<UiNode>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.entity_mut(entity).insert(transitions.clone());
    }
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("widget event: {event:?}");
    }
}

fn toggle_theme(keys: Res<ButtonInput<KeyCode>>, mut theme: ResMut<Theme>) {
    if keys.just_pressed(KeyCode::F6) {
        let next = match theme.kind {
            ThemeKind::Dark => ThemeKind::Light,
            ThemeKind::Light => ThemeKind::Dark,
        };
        *theme = Theme::new(next);
    }
}

fn toggle_reduced_motion(keys: Res<ButtonInput<KeyCode>>, mut reduced: ResMut<UiReducedMotion>) {
    if keys.just_pressed(KeyCode::F7) {
        reduced.0 = !reduced.0;
    }
}

fn sync_theme_background(theme: Res<Theme>, mut roots: Query<&mut UiPaint, With<UiLayoutRoot>>) {
    if !theme.is_changed() {
        return;
    }
    for mut paint in &mut roots {
        if let Primitive::RoundedRect { color, .. } = &mut paint.0 {
            *color = theme.background;
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Showcase (F6 theme, F7 reduced motion)".into(),
                resolution: WindowResolution::new(1280, 760),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                log_widget_events.after(bbui_core::UiSystems::Widgets),
                (toggle_theme, sync_theme_background)
                    .chain()
                    .before(bbui_core::UiSystems::Style),
                toggle_reduced_motion,
            ),
        )
        .run();
}

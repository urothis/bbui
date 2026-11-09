//! Blender-inspired object properties editor built from BUI controls.
//!
//! Run with `cargo run -p bui --example properties_editor`.
//! Drag numbers, toggle visibility, switch modes, collapse panels, and use the menus.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{Label, MenuEntry, Panel, WidgetEvent, WidgetEventQueue, WidgetNode};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};
use bui_style::{Rgba, UiInlineStyle, UiStyleProperty, UiStyleValue};

const WIDTH: f32 = 520.0;
const HEIGHT: f32 = 690.0;

#[derive(Resource, Clone, Copy)]
struct PropertyControls {
    transform: Entity,
    visibility: Entity,
    display: Entity,
    status: Entity,
}

fn axis(label: &str, value: f64, suffix: &str) -> WidgetNode {
    WidgetNode::row([
        WidgetNode::label(format!("{label} {suffix}"))
            .with_width(UiLength::Px(38.0))
            .with_height(UiLength::Px(22.0)),
        surface(
            WidgetNode::number("", value, -10_000.0, 10_000.0, 0.1),
            Rgba(0.29, 0.29, 0.29, 1.0),
        )
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(22.0)),
    ])
    .with_gap(4.0)
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Px(22.0))
}

fn surface(node: WidgetNode, color: Rgba) -> WidgetNode {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::Background, UiStyleValue::Color(color));
    node.with_style(style)
}

fn check(label: &str, checked: bool) -> WidgetNode {
    WidgetNode::checkbox(label, checked)
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(22.0))
}

fn toolbar_button(icon: &str, description: &str, key: &str) -> WidgetNode {
    WidgetNode::icon_button(icon, description)
        .with_key(key)
        .with_width(UiLength::Px(32.0))
        .with_height(UiLength::Px(32.0))
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .expect("properties editor requires a window");
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(0.0),
                gap: 0.0,
                clip_contents: true,
                ..UiLayoutNode::default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, WIDTH, HEIGHT), 1.0),
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

    let menu = WidgetNode::menu_bar([
        WidgetNode::menu(
            "File",
            [
                MenuEntry::command("new", "New Scene"),
                MenuEntry::command("open", "Open…"),
                MenuEntry::command("save", "Save Scene"),
            ],
        )
        .with_width(UiLength::Px(52.0)),
        WidgetNode::menu(
            "Edit",
            [
                MenuEntry::command("undo", "Undo"),
                MenuEntry::command("redo", "Redo"),
            ],
        )
        .with_width(UiLength::Px(52.0)),
        WidgetNode::menu(
            "Render",
            [
                MenuEntry::command("render", "Render Image"),
                MenuEntry::command("animation", "Render Animation"),
            ],
        )
        .with_width(UiLength::Px(72.0)),
        WidgetNode::menu("Help", [MenuEntry::command("about", "About BUI")])
            .with_width(UiLength::Px(56.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Px(29.0));

    let transform = WidgetNode::panel(
        "Transform",
        true,
        [
            WidgetNode::label("Location").with_height(UiLength::Px(18.0)),
            axis("X", 0.0, "m"),
            axis("Y", 1.25, "m"),
            axis("Z", 0.0, "m"),
            WidgetNode::label("Rotation").with_height(UiLength::Px(18.0)),
            axis("X", 0.0, "°"),
            axis("Y", 0.0, "°"),
            axis("Z", 90.0, "°"),
            WidgetNode::property_row(
                "Scale",
                WidgetNode::slider_range("", 1.0, 0.0, 2.0, 0.01).with_height(UiLength::Px(32.0)),
            )
            .with_height(UiLength::Px(32.0)),
            WidgetNode::property_row(
                "Mode",
                WidgetNode::dropdown("", ["Object Mode", "Edit Mode", "Sculpt Mode"], Some(0))
                    .with_height(UiLength::Px(22.0)),
            )
            .with_height(UiLength::Px(22.0)),
        ],
    )
    .with_key("transform")
    .with_gap(2.0)
    .with_width(UiLength::Fill(1.0));

    let visibility = WidgetNode::panel(
        "Visibility",
        true,
        [
            WidgetNode::row([
                surface(check("Viewport", true), Rgba(0.25, 0.43, 0.68, 1.0)),
                surface(check("Visible", true), Rgba(0.25, 0.43, 0.68, 1.0)),
                surface(check("Render", true), Rgba(0.25, 0.43, 0.68, 1.0)),
            ])
            .with_gap(2.0)
            .with_height(UiLength::Px(22.0)),
            WidgetNode::row([check("Camera", true), check("Diffuse", true)])
                .with_gap(4.0)
                .with_height(UiLength::Px(22.0)),
            WidgetNode::row([check("Glossy", true), check("Transmission", true)])
                .with_gap(4.0)
                .with_height(UiLength::Px(22.0)),
            WidgetNode::row([check("Volume Scatter", false), check("Shadow", true)])
                .with_gap(4.0)
                .with_height(UiLength::Px(22.0)),
            WidgetNode::label("Mask").with_height(UiLength::Px(18.0)),
            check("Shadow Catcher", false),
            check("Holdout", false),
            check("Motion Blur", true),
            WidgetNode::property_row(
                "Steps",
                WidgetNode::number("", 1.0, 1.0, 64.0, 1.0).with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
        ],
    )
    .with_key("visibility")
    .with_gap(3.0)
    .with_width(UiLength::Fill(1.0));

    let inspector = WidgetNode::column([
        WidgetNode::row([
            WidgetNode::label("▣  Cube").with_width(UiLength::Fill(1.0)),
            WidgetNode::label("Object Properties").with_width(UiLength::Px(138.0)),
        ])
        .with_height(UiLength::Px(30.0)),
        transform,
        visibility,
        WidgetNode::panel(
            "Viewport Display",
            false,
            [check("Show Name", false), check("Show Axis", true)],
        )
        .with_key("viewport")
        .with_width(UiLength::Fill(1.0)),
    ])
    .with_gap(3.0)
    .with_width(UiLength::Fill(1.0));

    let content = WidgetNode::row([
        WidgetNode::column([
            toolbar_button("T", "Show Transform", "rail-transform"),
            toolbar_button("V", "Show Visibility", "rail-visibility"),
            toolbar_button("D", "Show Viewport Display", "rail-display"),
            toolbar_button("A", "Show All Panels", "rail-all"),
        ])
        .with_width(UiLength::Px(35.0)),
        WidgetNode::scroll([inspector])
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0));

    let tree = WidgetTree::new(
        WidgetNode::column([
            menu,
            content,
            WidgetNode::overlay([
                WidgetNode::progress(0.25)
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Px(24.0)),
                WidgetNode::row([
                    WidgetNode::label("").with_width(UiLength::Px(213.0)),
                    WidgetNode::label("Rendering 25%").with_width(UiLength::Px(160.0)),
                ])
                .with_height(UiLength::Px(24.0)),
            ])
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(24.0)),
            WidgetNode::label("Drag numeric values · click tabs to switch sections")
                .with_key("property-status")
                .with_height(UiLength::Px(22.0)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    );
    tree.spawn(world, Some(root))
        .expect("valid properties editor widget tree");
    let mut query = world.query::<(Entity, &UiKey)>();
    let keyed = query
        .iter(world)
        .map(|(entity, key)| (key.0.clone(), entity))
        .collect::<std::collections::HashMap<_, _>>();
    let lookup = |key: &str| *keyed.get(key).expect("property control has a key");
    world.insert_resource(PropertyControls {
        transform: lookup("transform"),
        visibility: lookup("visibility"),
        display: lookup("viewport"),
        status: lookup("property-status"),
    });
}

fn handle_events(world: &mut World) {
    let controls = *world.resource::<PropertyControls>();
    let events = world.resource_mut::<WidgetEventQueue>().take();
    for event in events {
        if let WidgetEvent::Activated { target } = &event {
            let section = world.get::<UiKey>(*target).map(|key| key.0.as_str());
            let active = match section {
                Some("rail-transform") => Some(("Transform", [true, false, false])),
                Some("rail-visibility") => Some(("Visibility", [false, true, false])),
                Some("rail-display") => Some(("Viewport Display", [false, false, true])),
                Some("rail-all") => Some(("All sections", [true, true, true])),
                _ => None,
            };
            if let Some((name, expanded)) = active {
                for (panel, open) in [controls.transform, controls.visibility, controls.display]
                    .into_iter()
                    .zip(expanded)
                {
                    world.get_mut::<Panel>(panel).unwrap().set_expanded(open);
                }
                world.get_mut::<Label>(controls.status).unwrap().text =
                    format!("{name} · drag values · click tabs to switch sections");
            }
        }
        info!("properties editor: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI Properties Editor".into(),
                resolution: WindowResolution::new(WIDTH as u32, HEIGHT as u32),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_events.after(bui_core::UiSystems::Widgets))
        .run();
}

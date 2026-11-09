//! Blender-inspired object properties editor built from BUI controls.
//!
//! Run with `cargo run -p bui --example properties_editor`.
//! Drag numbers, toggle visibility, switch modes, collapse panels, and use the menus.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{
    Dropdown, IconButton, Label, MenuEntry, Panel, WidgetEvent, WidgetEventQueue, WidgetNode,
};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};
use bui_style::Rgba;
use bui_widgets::actions::WidgetValue;

const WIDTH: f32 = 520.0;
const HEIGHT: f32 = 690.0;

#[derive(Resource, Clone, Copy)]
struct PropertyControls {
    transform: Entity,
    visibility: Entity,
    display: Entity,
    status: Entity,
    object_name: Entity,
}

fn axis(label: &str, value: f64, suffix: &str) -> WidgetNode {
    let role = if suffix == "°" {
        "vector-axis-degrees"
    } else {
        "vector-axis-meters"
    };
    WidgetNode::number(label, value, -10_000.0, 10_000.0, 0.1)
        .with_role(role)
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(20.0))
}

fn check(label: &str, checked: bool) -> WidgetNode {
    WidgetNode::checkbox(label, checked)
        .with_role("filled-checkbox")
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(20.0))
}

fn toolbar_button(icon: &str, description: &str, key: &str) -> WidgetNode {
    WidgetNode::icon_button(icon, description)
        .with_key(key)
        .with_role("flat-icon-button")
        .with_width(UiLength::Px(26.0))
        .with_height(UiLength::Px(27.0))
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    {
        let mut theme = world.resource_mut::<Theme>();
        theme.font_size = 11.5;
        theme.corner_radius = 3.0;
        theme.background = Rgba(0.12, 0.12, 0.12, 1.0);
        theme.surface = Rgba(0.22, 0.22, 0.22, 1.0);
        theme.spacing.control.padding_x = 6.0;
        theme.spacing.control.padding_y = 2.0;
        theme.spacing.control.checkbox_label_gap = 4.0;
        theme.spacing.control.slider_height = 20.0;
        theme.spacing.control.slider_track_height = 20.0;
        theme.spacing.panel.content_padding = 7.0;
        theme.spacing.property_row.label_width = 185.0;
        theme.spacing.property_row.gap = 7.0;
    }
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("properties editor requires a window");
        return;
    };
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
        .with_role("flat-menu")
        .with_width(UiLength::Px(43.0)),
        WidgetNode::menu(
            "Edit",
            [
                MenuEntry::command("undo", "Undo"),
                MenuEntry::command("redo", "Redo"),
            ],
        )
        .with_role("flat-menu")
        .with_width(UiLength::Px(43.0)),
        WidgetNode::menu(
            "Render",
            [
                MenuEntry::command("render", "Render Image"),
                MenuEntry::command("animation", "Render Animation"),
            ],
        )
        .with_role("flat-menu")
        .with_width(UiLength::Px(60.0)),
        WidgetNode::menu("Help", [MenuEntry::command("about", "About BUI")])
            .with_role("flat-menu")
            .with_width(UiLength::Px(45.0)),
        WidgetNode::label("").with_width(UiLength::Fill(1.0)),
        WidgetNode::label("▶").with_width(UiLength::Px(25.0)),
        WidgetNode::menu(
            "⌕",
            [
                MenuEntry::command("find-transform", "Transform"),
                MenuEntry::command("find-visibility", "Visibility"),
                MenuEntry::command("find-display", "Viewport Display"),
            ],
        )
        .with_role("flat-menu")
        .with_width(UiLength::Px(25.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Px(28.0));

    let transform = WidgetNode::panel(
        "Transform",
        true,
        [
            WidgetNode::label("Location").with_height(UiLength::Px(17.0)),
            axis("X", 0.0, "m"),
            axis("Y", 1.25, "m"),
            axis("Z", 0.0, "m"),
            WidgetNode::label("Rotation").with_height(UiLength::Px(17.0)),
            axis("X", 0.0, "°"),
            axis("Y", 0.0, "°"),
            axis("Z", 90.0, "°"),
            WidgetNode::property_row(
                "Scale",
                WidgetNode::slider_range("", 1.0, 0.0, 4.0, 0.01)
                    .with_role("filled-slider")
                    .with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
            WidgetNode::property_row(
                "Mode",
                WidgetNode::dropdown("", ["Object Mode", "Edit Mode", "Sculpt Mode"], Some(0))
                    .with_role("compact-dropdown")
                    .with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
        ],
    )
    .with_key("transform")
    .with_gap(1.0)
    .with_width(UiLength::Fill(1.0));

    let visibility = WidgetNode::panel(
        "Visibility",
        true,
        [
            WidgetNode::row([
                WidgetNode::checkbox("▸", true)
                    .with_role("segmented-toggle")
                    .with_width(UiLength::Fill(1.0)),
                WidgetNode::checkbox("◉", true)
                    .with_role("segmented-toggle")
                    .with_width(UiLength::Fill(1.0)),
                WidgetNode::checkbox("▣", true)
                    .with_role("segmented-toggle")
                    .with_width(UiLength::Fill(1.0)),
            ])
            .with_role("visibility-toggle-strip")
            .with_gap(1.0)
            .with_height(UiLength::Px(20.0)),
            WidgetNode::row([check("Camera", true), check("Diffuse", true)])
                .with_gap(4.0)
                .with_height(UiLength::Px(20.0)),
            WidgetNode::row([check("Glossy", true), check("Transmission", true)])
                .with_gap(4.0)
                .with_height(UiLength::Px(20.0)),
            WidgetNode::row([check("Volume Scatter", false), check("Shadow", true)])
                .with_gap(4.0)
                .with_height(UiLength::Px(20.0)),
            WidgetNode::panel(
                "Mask",
                true,
                [check("Shadow Catcher", false), check("Holdout", false)],
            )
            .with_role("sub-panel")
            .with_gap(0.0)
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Motion Blur",
                true,
                [WidgetNode::property_row(
                    "Steps",
                    WidgetNode::number("", 1.0, 1.0, 64.0, 1.0).with_height(UiLength::Px(20.0)),
                )
                .with_height(UiLength::Px(20.0))],
            )
            .with_role("sub-panel")
            .with_gap(0.0)
            .with_width(UiLength::Fill(1.0)),
        ],
    )
    .with_key("visibility")
    .with_gap(2.0)
    .with_width(UiLength::Fill(1.0));

    let inspector = WidgetNode::column([
        WidgetNode::row([
            WidgetNode::dropdown("", ["Cube", "Camera", "Light"], Some(0))
                .with_key("object-browse")
                .with_role("icon-dropdown")
                .with_width(UiLength::Px(34.0)),
            WidgetNode::text_field("", "Cube")
                .with_key("object-name")
                .with_width(UiLength::Fill(1.0)),
            WidgetNode::icon_button("◇", "Keep object data")
                .with_key("object-fake")
                .with_width(UiLength::Px(20.0)),
            WidgetNode::icon_button("▣", "Duplicate object data")
                .with_key("object-duplicate")
                .with_width(UiLength::Px(20.0)),
            WidgetNode::icon_button("×", "Unlink object data")
                .with_key("object-unlink")
                .with_width(UiLength::Px(20.0)),
        ])
        .with_gap(1.0)
        .with_height(UiLength::Px(21.0)),
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
            toolbar_button("▣", "Show Transform", "rail-transform"),
            toolbar_button("⚒", "Show Visibility", "rail-visibility"),
            toolbar_button("◕", "Show Viewport Display", "rail-display"),
            toolbar_button("☷", "Show All Panels", "rail-all"),
        ])
        .with_width(UiLength::Px(29.0)),
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
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("widget tree setup: {error:?}");
        return;
    }
    let mut query = world.query::<(Entity, &UiKey)>();
    let keyed = query
        .iter(world)
        .map(|(entity, key)| (key.0.clone(), entity))
        .collect::<std::collections::HashMap<_, _>>();
    let (Some(&transform), Some(&visibility), Some(&display), Some(&status), Some(&object_name)) = (
        keyed.get("transform"),
        keyed.get("visibility"),
        keyed.get("viewport"),
        keyed.get("property-status"),
        keyed.get("object-name"),
    ) else {
        eprintln!("properties editor controls are incomplete");
        return;
    };
    world.insert_resource(PropertyControls {
        transform,
        visibility,
        display,
        status,
        object_name,
    });
}

fn handle_events(world: &mut World) {
    let Some(controls) = world.get_resource::<PropertyControls>().copied() else {
        return;
    };
    let events = world.resource_mut::<WidgetEventQueue>().take();
    for event in events {
        if let WidgetEvent::Activated { target } = &event {
            let section = world.get::<UiKey>(*target).map(|key| key.0.clone());
            let active = match section.as_deref() {
                Some("rail-transform") => Some(("Transform", [true, true, false])),
                Some("rail-visibility") => Some(("Visibility", [true, true, false])),
                Some("rail-display") => Some(("Viewport Display", [true, true, true])),
                Some("rail-all") => Some(("All sections", [true, true, true])),
                _ => None,
            };
            if section.as_deref() == Some("object-fake")
                && let Some(mut button) = world.get_mut::<IconButton>(*target)
            {
                button.icon = if button.icon == "◇" { "◆" } else { "◇" }.into();
            }
            if section.as_deref() == Some("object-duplicate") {
                let name = world
                    .get::<TextBuffer>(controls.object_name)
                    .map_or("Object".to_owned(), |buffer| buffer.as_str().to_owned());
                if let Some(mut buffer) = world.get_mut::<TextBuffer>(controls.object_name) {
                    buffer.select_all();
                    buffer.insert_text(&format!("{name}.001"));
                }
            }
            if section.as_deref() == Some("object-unlink")
                && let Some(mut buffer) = world.get_mut::<TextBuffer>(controls.object_name)
            {
                buffer.select_all();
                buffer.insert_text("");
            }
            if let Some((name, expanded)) = active {
                for (panel, open) in [controls.transform, controls.visibility, controls.display]
                    .into_iter()
                    .zip(expanded)
                {
                    if let Some(mut panel) = world.get_mut::<Panel>(panel) {
                        panel.set_expanded(open);
                    }
                }
                if let Some(mut status) = world.get_mut::<Label>(controls.status) {
                    status.text = format!("{name} · drag values · click tabs to switch sections");
                }
            }
        }
        if let WidgetEvent::ValueChanged {
            target,
            after: WidgetValue::Option(Some(index)),
            ..
        } = &event
            && world
                .get::<UiKey>(*target)
                .is_some_and(|key| key.0 == "object-browse")
            && let Some(name) = world
                .get::<Dropdown>(*target)
                .and_then(|dropdown| dropdown.options.get(*index))
                .cloned()
            && let Some(mut buffer) = world.get_mut::<TextBuffer>(controls.object_name)
        {
            buffer.select_all();
            buffer.insert_text(&name);
        }
        if let WidgetEvent::MenuCommand { command_id, .. } = &event {
            let expanded = match command_id.as_str() {
                "find-transform" | "find-visibility" => Some([true, true, false]),
                "find-display" => Some([true, true, true]),
                _ => None,
            };
            if let Some(expanded) = expanded {
                for (panel, open) in [controls.transform, controls.visibility, controls.display]
                    .into_iter()
                    .zip(expanded)
                {
                    if let Some(mut panel) = world.get_mut::<Panel>(panel) {
                        panel.set_expanded(open);
                    }
                }
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

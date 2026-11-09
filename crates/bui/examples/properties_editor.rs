//! Blender-inspired object properties editor built from BUI controls.
//!
//! Run with `cargo run -p bui --example properties_editor`.
//! Drag numbers, toggle visibility, switch modes, collapse panels, and use the menus.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui::widgets::{
    Dropdown, IconButton, Label, MenuEntry, Panel, PanelGroup, Progress, WidgetEvent,
    WidgetEventQueue, WidgetNode,
};
use bui_layout::{Rect, UiEdges};
use bui_render::{ClipRect, Primitive, UiPaint};
use bui_style::Rgba;
use bui_widgets::actions::WidgetValue;

#[path = "properties_editor/pages.rs"]
mod pages;

use pages::{material_page, modifier_page, nodes_page, outliner_page, preferences_page};

const WIDTH: f32 = 520.0;
const HEIGHT: f32 = 690.0;

#[derive(Resource, Clone, Copy)]
struct PropertyControls {
    transform: Entity,
    visibility: Entity,
    display: Entity,
    status: Entity,
    object_name: Entity,
    progress: Entity,
    progress_label: Entity,
    mirror_modifier: Entity,
    pages: [Entity; 6],
    tabs: [Entity; 6],
}

const PAGE_KEYS: [&str; 6] = [
    "page-object",
    "page-modifiers",
    "page-material",
    "page-outliner",
    "page-preferences",
    "page-nodes",
];
const TAB_KEYS: [&str; 6] = [
    "tab-object",
    "tab-modifiers",
    "tab-material",
    "tab-outliner",
    "tab-preferences",
    "tab-nodes",
];
const TAB_NAMES: [&str; 6] = [
    "Object",
    "Modifiers",
    "Material",
    "Outliner",
    "Preferences",
    "Geometry Nodes",
];

fn tab_role(index: usize, active: bool) -> &'static str {
    match (index, active) {
        (0, true) => "side-tab-orange-active",
        (0, false) => "side-tab-orange",
        (1, true) | (5, true) => "side-tab-blue-active",
        (1, false) | (5, false) => "side-tab-blue",
        (2, true) => "side-tab-red-active",
        (2, false) => "side-tab-red",
        (_, true) => "side-tab-active",
        _ => "side-tab",
    }
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

fn toolbar_button(index: usize, icon: &str) -> WidgetNode {
    WidgetNode::icon_button(icon, TAB_NAMES[index])
        .with_key(TAB_KEYS[index])
        .with_role(tab_role(index, index == 0))
        .with_width(UiLength::Px(28.0))
        .with_height(UiLength::Px(28.0))
}

fn activate_page(world: &mut World, controls: PropertyControls, selected: usize) {
    for (index, (page, tab)) in controls.pages.into_iter().zip(controls.tabs).enumerate() {
        let active = index == selected;
        if let Some(mut visible) = world.get_mut::<UiVisible>(page)
            && visible.0 != active
        {
            visible.0 = active;
        }
        if let Some(mut role) = world.get_mut::<UiRole>(tab) {
            let expected = tab_role(index, active);
            if role.0 != expected {
                role.0 = expected.into();
            }
        }
    }
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    {
        let mut theme = world.resource_mut::<Theme>();
        theme.font_size = 11.5;
        theme.corner_radius = 3.0;
        theme.background = Rgba(48.0 / 255.0, 48.0 / 255.0, 48.0 / 255.0, 1.0);
        theme.surface = Rgba(61.0 / 255.0, 61.0 / 255.0, 61.0 / 255.0, 1.0);
        theme.surface_hover = Rgba(84.0 / 255.0, 84.0 / 255.0, 84.0 / 255.0, 1.0);
        theme.spacing.control.padding_x = 6.0;
        theme.spacing.control.padding_y = 2.0;
        theme.spacing.control.checkbox_label_gap = 4.0;
        theme.spacing.control.slider_height = 20.0;
        theme.spacing.control.slider_track_height = 20.0;
        theme.spacing.panel.content_padding = 7.0;
        theme.spacing.property_row.label_width = 181.0;
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
        .with_role("properties-flat-menu")
        .with_width(UiLength::Px(43.0))
        .with_height(UiLength::Px(20.0)),
        WidgetNode::menu(
            "Edit",
            [
                MenuEntry::command("undo", "Undo"),
                MenuEntry::command("redo", "Redo"),
            ],
        )
        .with_role("properties-flat-menu")
        .with_width(UiLength::Px(43.0))
        .with_height(UiLength::Px(20.0)),
        WidgetNode::menu(
            "Render",
            [
                MenuEntry::command("render", "Render Image"),
                MenuEntry::command("animation", "Render Animation"),
            ],
        )
        .with_role("properties-flat-menu")
        .with_width(UiLength::Px(60.0))
        .with_height(UiLength::Px(20.0)),
        WidgetNode::menu("Help", [MenuEntry::command("about", "About BUI")])
            .with_role("properties-flat-menu")
            .with_width(UiLength::Px(45.0))
            .with_height(UiLength::Px(20.0)),
        WidgetNode::label("")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(20.0)),
        WidgetNode::icon_button("", "Advance render preview")
            .with_key("header-play")
            .with_role("header-play-icon")
            .with_width(UiLength::Px(25.0))
            .with_height(UiLength::Px(20.0)),
        WidgetNode::menu(
            "⌕",
            [
                MenuEntry::command("find-transform", "Transform"),
                MenuEntry::command("find-visibility", "Visibility"),
                MenuEntry::command("find-display", "Viewport Display"),
            ],
        )
        .with_role("properties-flat-menu")
        .with_width(UiLength::Px(25.0))
        .with_height(UiLength::Px(20.0)),
    ])
    .with_role("properties-menu-bar")
    .with_gap(0.0)
    .with_padding(UiEdges::all(4.0))
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Px(28.0));

    let transform = WidgetNode::panel(
        "Transform",
        true,
        [
            WidgetNode::label("").with_height(UiLength::Px(2.0)),
            WidgetNode::label("Location").with_height(UiLength::Px(17.0)),
            WidgetNode::label("").with_height(UiLength::Px(3.0)),
            axis("X", 0.0, "m"),
            axis("Y", 1.25, "m"),
            axis("Z", 0.0, "m"),
            WidgetNode::label("Rotation").with_height(UiLength::Px(17.0)),
            WidgetNode::label("").with_height(UiLength::Px(3.0)),
            axis("X", 0.0, "°"),
            axis("Y", 0.0, "°"),
            axis("Z", 90.0, "°"),
            WidgetNode::property_row_with_decorator(
                "Scale",
                WidgetNode::slider_range("", 1.0, 0.0, 4.0, 0.01)
                    .with_role("filled-slider")
                    .with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
            WidgetNode::property_row_with_decorator(
                "Mode",
                WidgetNode::dropdown("", ["Object Mode", "Edit Mode", "Sculpt Mode"], Some(0))
                    .with_role("compact-dropdown")
                    .with_height(UiLength::Px(20.0)),
            )
            .with_height(UiLength::Px(20.0)),
        ],
    )
    .with_key("transform")
    .with_role("object-panel")
    .with_gap(1.0)
    .with_width(UiLength::Fill(1.0));

    let visibility = WidgetNode::panel(
        "Visibility",
        true,
        [
            WidgetNode::column([
                WidgetNode::label("").with_height(UiLength::Px(5.0)),
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
            ])
            .with_gap(2.0)
            .with_padding(UiEdges::horizontal_vertical(8.0, 0.0))
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Mask",
                true,
                [check("Shadow Catcher", false), check("Holdout", false)],
            )
            .with_role("sub-panel")
            .with_padding(UiEdges {
                left: 8.0,
                top: 28.0,
                right: 8.0,
                bottom: 8.0,
            })
            .with_gap(0.0)
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::panel(
                "Motion Blur",
                true,
                [WidgetNode::property_row_with_decorator(
                    "Steps",
                    WidgetNode::number("", 1.0, 1.0, 64.0, 1.0)
                        .with_role("centered-number")
                        .with_height(UiLength::Px(20.0)),
                )
                .with_height(UiLength::Px(20.0))],
            )
            .with_role("sub-panel")
            .with_padding(UiEdges {
                left: 8.0,
                top: 28.0,
                right: 8.0,
                bottom: 8.0,
            })
            .with_gap(0.0)
            .with_width(UiLength::Fill(1.0)),
            WidgetNode::label("").with_height(UiLength::Px(5.0)),
        ],
    )
    .with_key("visibility")
    .with_gap(2.0)
    .with_padding(UiEdges {
        left: 0.0,
        top: 28.0,
        right: 0.0,
        bottom: 8.0,
    })
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
        .with_role("properties-tab-rail")
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
    .with_key(PAGE_KEYS[0])
    .with_gap(1.0)
    .with_width(UiLength::Fill(1.0));

    let modifiers = modifier_page();
    let material = material_page();
    let outliner = outliner_page();
    let preferences = preferences_page();
    let nodes = nodes_page();

    let content = WidgetNode::row([
        WidgetNode::column([
            toolbar_button(0, "▣"),
            toolbar_button(1, "⚒"),
            toolbar_button(2, "◕"),
            toolbar_button(3, "☷"),
            toolbar_button(4, "⚙"),
            toolbar_button(5, "✣"),
        ])
        .with_gap(1.0)
        .with_padding(UiEdges {
            left: 0.0,
            top: 1.0,
            right: 0.0,
            bottom: 0.0,
        })
        .with_width(UiLength::Px(29.0)),
        WidgetNode::scroll([inspector, modifiers, material, outliner, preferences, nodes])
            .with_padding(UiEdges {
                left: 0.0,
                top: 0.0,
                right: 8.0,
                bottom: 0.0,
            })
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0));

    let tree = WidgetTree::new(
        WidgetNode::column([
            menu,
            content,
            WidgetNode::column([
                WidgetNode::overlay([
                    WidgetNode::progress(0.25)
                        .with_key("render-progress")
                        .with_role("properties-progress")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(18.0)),
                    WidgetNode::label("Rendering 25%")
                        .with_key("render-progress-label")
                        .with_role("centered-label")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(18.0)),
                ])
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(18.0)),
                WidgetNode::row([
                    WidgetNode::label("i").with_width(UiLength::Px(16.0)),
                    WidgetNode::label("Drag numeric values · click tabs to switch pages")
                        .with_key("property-status")
                        .with_width(UiLength::Fill(1.0)),
                ])
                .with_gap(4.0)
                .with_height(UiLength::Px(20.0)),
            ])
            .with_gap(4.0)
            .with_padding(UiEdges::all(8.0))
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(72.0)),
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
    let (
        Some(&transform),
        Some(&visibility),
        Some(&display),
        Some(&status),
        Some(&object_name),
        Some(&progress),
        Some(&progress_label),
        Some(&mirror_modifier),
    ) = (
        keyed.get("transform"),
        keyed.get("visibility"),
        keyed.get("viewport"),
        keyed.get("property-status"),
        keyed.get("object-name"),
        keyed.get("render-progress"),
        keyed.get("render-progress-label"),
        keyed.get("mirror-modifier"),
    )
    else {
        eprintln!("properties editor controls are incomplete");
        return;
    };
    let Some(page_entities) = PAGE_KEYS
        .iter()
        .map(|key| keyed.get(*key).copied())
        .collect::<Option<Vec<_>>>()
    else {
        eprintln!("properties editor pages are incomplete");
        return;
    };
    let Ok(pages): Result<[Entity; 6], _> = page_entities.try_into() else {
        return;
    };
    let Some(tab_entities) = TAB_KEYS
        .iter()
        .map(|key| keyed.get(*key).copied())
        .collect::<Option<Vec<_>>>()
    else {
        eprintln!("properties editor tabs are incomplete");
        return;
    };
    let Ok(tabs): Result<[Entity; 6], _> = tab_entities.try_into() else {
        return;
    };
    for page in pages.iter().skip(1) {
        world.entity_mut(*page).insert(UiVisible(false));
    }
    world.entity_mut(pages[0]).insert(
        PanelGroup::with_panels(["transform", "visibility", "viewport"])
            .with_reordering("properties-object-panels"),
    );
    let secondary_groups: [(&str, &[&str], &str); 3] = [
        (
            PAGE_KEYS[1],
            &["modifier-subdivision", "modifier-ramp", "modifier-curves"],
            "properties-modifier-panels",
        ),
        (
            PAGE_KEYS[2],
            &["material-preview", "material-surface"],
            "properties-material-panels",
        ),
        (
            PAGE_KEYS[4],
            &[
                "prefs-input",
                "prefs-paths",
                "prefs-addons",
                "prefs-interface",
            ],
            "properties-preferences-panels",
        ),
    ];
    for (page_key, panel_keys, persistence_key) in secondary_groups {
        let content_key = format!("{page_key}-content");
        let Some(&content) = keyed.get(&content_key) else {
            eprintln!("properties editor panel group {content_key} is incomplete");
            return;
        };
        world.entity_mut(content).insert(
            PanelGroup::with_panels(panel_keys.iter().copied()).with_reordering(persistence_key),
        );
    }
    world.entity_mut(mirror_modifier).insert(UiVisible(false));
    world.insert_resource(PropertyControls {
        transform,
        visibility,
        display,
        status,
        object_name,
        progress,
        progress_label,
        mirror_modifier,
        pages,
        tabs,
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
            if let Some(index) = TAB_KEYS
                .iter()
                .position(|key| Some(*key) == section.as_deref())
            {
                activate_page(world, controls, index);
                if let Some(mut status) = world.get_mut::<Label>(controls.status) {
                    status.text = format!(
                        "{} · drag values · click tabs to switch pages",
                        TAB_NAMES[index]
                    );
                }
            }
            if section.as_deref() == Some("object-fake")
                && let Some(mut button) = world.get_mut::<IconButton>(*target)
            {
                button.icon = if button.icon == "◇" { "◆" } else { "◇" }.into();
            }
            if section.as_deref() == Some("header-play") {
                let next = if let Some(mut progress) = world.get_mut::<Progress>(controls.progress)
                {
                    let next = if progress.value >= 1.0 {
                        0.25
                    } else {
                        (progress.value + 0.25).min(1.0)
                    };
                    progress.set(next);
                    Some(next)
                } else {
                    None
                };
                if let Some(next) = next
                    && let Some(mut label) = world.get_mut::<Label>(controls.progress_label)
                {
                    label.text = format!("Rendering {}%", (next * 100.0).round() as u32);
                }
            }
            if section.as_deref() == Some("add-modifier")
                && let Some(mut visible) = world.get_mut::<UiVisible>(controls.mirror_modifier)
            {
                visible.0 = !visible.0;
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
                activate_page(world, controls, 0);
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

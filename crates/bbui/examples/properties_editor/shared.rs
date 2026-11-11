//! Shared properties editor composition for the window and FPS world display.

use bbui::prelude::*;
use bbui::widgets::{
    Dropdown, EditorChrome, IconButton, Label, MenuEntry, Panel, PanelGroup, Progress, WidgetEvent,
    WidgetNode,
};
use bbui_layout::{Rect, UiEdges};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Rgba;
use bbui_widgets::actions::WidgetValue;
use bevy::prelude::*;

#[path = "events.rs"]
mod events;
#[path = "pages.rs"]
mod pages;
#[path = "tree.rs"]
mod tree;
pub use events::process_events;

use pages::{material_page, modifier_page, nodes_page, outliner_page, preferences_page};

pub const WIDTH: f32 = 520.0;
pub const HEIGHT: f32 = 690.0;

#[cfg(not(target_arch = "wasm32"))]
pub const EXAMPLE_ASSET_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
#[cfg(target_arch = "wasm32")]
pub const EXAMPLE_ASSET_PATH: &str = "assets";

pub struct EditorExtraPanel {
    pub key: &'static str,
    pub node: WidgetNode,
}

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
        .with_tab_selected(index == 0)
        .with_role(tab_role(index, index == 0))
        .with_width(UiLength::Px(28.0))
        .with_height(UiLength::Px(28.0))
}

fn toolbar_asset_button(index: usize, path: &str) -> WidgetNode {
    WidgetNode::icon_button_asset(path, TAB_NAMES[index])
        .with_key(TAB_KEYS[index])
        .with_tab_selected(index == 0)
        .with_role(tab_role(index, index == 0))
        .with_width(UiLength::Px(28.0))
        .with_height(UiLength::Px(28.0))
}

pub fn mount(
    world: &mut World,
    target: Entity,
    bounds: Rect,
    extra_panel: Option<EditorExtraPanel>,
) {
    let extra_panel_key = extra_panel.as_ref().map(|panel| panel.key);
    {
        let mut theme = world.resource_mut::<Theme>();
        if let Ok(scale) = std::env::var("BBUI_EXAMPLE_MOTION_SCALE")
            && let Ok(scale) = scale.parse::<f32>()
            && scale.is_finite()
            && scale > 0.0
        {
            theme.motion_scale = scale;
        }
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
            UiLayoutRoot::new(target, bounds, 1.0),
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

    let tree = tree::build_tree(extra_panel);
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
    let mut object_panel_keys = vec!["transform", "visibility", "viewport"];
    if let Some(key) = extra_panel_key {
        object_panel_keys.push(key);
    }
    let object_panels = PanelGroup::with_panels(object_panel_keys);
    world
        .entity_mut(pages[0])
        .insert(object_panels.with_reordering("properties-object-panels"));
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

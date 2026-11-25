//! Desktop and compact chrome around BBUI's composed editor controls.
//! Run with `cargo run -p bbui --example chrome_shell`.

use bbui::prelude::*;
use bbui::widgets::{Command, CommandRegistry, WidgetEventQueue};
use bbui_core::UiRoutedEvents;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "chrome_shell/capture.rs"]
mod capture;
#[path = "editor_workspace/content.rs"]
mod editor_content;

fn commands() -> CommandRegistry {
    let mut commands = CommandRegistry::default();
    for (id, label) in [("new", "New"), ("open", "Open"), ("save", "Save")] {
        if let Err(error) = commands.register(Command::new(id, label)) {
            error!("Command failed: {error:?}");
        }
    }
    commands
}

fn editor_layout() -> WorkspaceLayout {
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

fn desktop_page(center: Option<WidgetNode>) -> WidgetNode {
    let mut accent = UiInlineStyle::default();
    accent.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::Accent);
    let center = center.unwrap_or_else(|| {
        let mut muted = UiInlineStyle::default();
        muted.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::MutedText);
        muted.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(12.5));
        WidgetNode::row([WidgetNode::label("Material.001")
            .with_key("host:center-title")
            .with_role("centered-label")
            .with_width(UiLength::Fill(1.0))
            .with_style(muted)])
        .with_layout(UiLayoutNode {
            align_items: UiAlign::Center,
            ..UiLayoutNode::row(0.0)
        })
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0))
    });
    WidgetNode::column([
        DesktopChrome::new("Material Studio").view_with_slots(
            &commands(),
            &DesktopChromeAppearance::default(),
            DesktopChromeSlots {
                leading: Some(
                    WidgetNode::label("◆")
                        .with_width(UiLength::Px(18.0))
                        .with_style(accent),
                ),
                center: Some(center),
                trailing: None,
            },
        ),
        WidgetNode::column([])
            .with_key("chrome:workspace")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
        WidgetNode::row([
            WidgetNode::label("Material.001  ·  All changes saved").with_width(UiLength::Fill(1.0)),
            WidgetNode::label("4 nodes  ·  3 links"),
        ])
        .with_height(UiLength::Px(28.0))
        .with_padding(UiEdges::all(5.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
}

fn mobile_page() -> WidgetNode {
    let mut mobile = MobileChrome::new(
        "Material Studio",
        vec![
            MobileNavItem::new("scene", "Scene"),
            MobileNavItem::new("material", "Material"),
            MobileNavItem::new("assets", "Assets"),
        ],
    );
    mobile.selected = Some("scene".into());
    mobile.sheet_title = Some("Material.001".into());
    mobile.view(
        &commands(),
        WidgetNode::scroll([
            editor_content::scene_pane().with_height(UiLength::Px(310.0)),
            editor_content::console_pane().with_height(UiLength::Px(190.0)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
        Some(
            WidgetNode::column([
                WidgetNode::property_row_with_decorator(
                    "Metallic",
                    WidgetNode::slider_range("", 0.35, 0.0, 1.0, 0.01),
                ),
                WidgetNode::property_row_with_decorator(
                    "Roughness",
                    WidgetNode::slider_range("", 0.45, 0.0, 1.0, 0.01),
                ),
                WidgetNode::row([
                    WidgetNode::button("Assign material"),
                    WidgetNode::button("Edit graph"),
                ])
                .with_gap(8.0),
            ])
            .with_gap(8.0)
            .with_height(UiLength::Px(130.0)),
        ),
    )
}

fn install_chrome(world: &mut World) {
    install_chrome_with_center(world, None);
}

fn install_chrome_with_center(world: &mut World, center: Option<WidgetNode>) {
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
    let compact = compact_chrome(size[0]);
    let page = if compact {
        mobile_page()
    } else {
        desktop_page(center)
    };
    let Ok(root) = WidgetTree::new(page).spawn(world, None) else {
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, size[0], size[1]),
        1.0,
    ));
    if compact {
        return;
    }
    let mut keys = world.query::<(Entity, &UiKey)>();
    let Some(host) = keys
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "chrome:workspace").then_some(entity))
    else {
        return;
    };
    let Ok(mut workspace) = Workspace::new(editor_layout()) else {
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

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_chrome(world);
}

#[derive(Resource, Default)]
struct MaximizeState(bool);

fn process_chrome(world: &mut World) {
    let routed = world.resource::<UiRoutedEvents>().events.clone();
    for event in &routed {
        handle_chrome_press(world, event);
    }
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let mut windows = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        return;
    };
    for event in &events {
        let Some(action) = chrome_action(world, event) else {
            continue;
        };
        if matches!(action, ChromeAction::ToggleMaximize) {
            let current = world.resource::<MaximizeState>().0;
            world.resource_mut::<MaximizeState>().0 = !current;
        }
        let maximized = world.resource::<MaximizeState>().0;
        if let Some(mut window_state) = world.get_mut::<Window>(window) {
            apply_native_chrome_action(&mut window_state, !maximized, &action);
        }
        info!("Chrome action: {action:?}");
    }
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
        .init_resource::<MaximizeState>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, process_chrome)
        .run();
}

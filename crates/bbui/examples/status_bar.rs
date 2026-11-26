//! Application-composed status bar in an application shell.
//! Run with `cargo run -p bbui --example status_bar`.

use bbui::prelude::*;
use bbui::widgets::{Command, CommandRegistry, WidgetEvent, WidgetEventQueue};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "status_bar/capture.rs"]
mod capture;
#[path = "editor_workspace/content.rs"]
mod editor_content;

fn status_slots(compact: bool) -> StatusBarSlots {
    let text = |size, token| {
        let mut style = UiInlineStyle::default();
        style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(size));
        style.set_color_token(UiStyleProperty::Foreground, token);
        style
    };
    let leading = WidgetNode::row([
        WidgetNode::label("Material.001")
            .with_key("host:document")
            .with_style(text(11.5, ThemeColorToken::Text)),
        WidgetNode::label("Rendering preview")
            .with_key("host:activity")
            .with_live_region(UiLiveRegion::polite())
            .with_style(text(11.5, ThemeColorToken::Accent)),
    ])
    .with_gap(16.0);
    let leading = if compact {
        WidgetNode::label("Material.001")
            .with_key("host:document")
            .with_style(text(11.5, ThemeColorToken::Text))
    } else {
        leading
    };
    let mut cancel_style = text(11.5, ThemeColorToken::Text);
    cancel_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Background);
    cancel_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(4.0));
    StatusBarSlots {
        leading: Some(leading),
        center: if compact {
            None
        } else {
            Some(
                WidgetNode::label("Preview 65%")
                    .with_key("host:progress-label")
                    .with_live_region(UiLiveRegion::polite())
                    .with_style(text(11.5, ThemeColorToken::MutedText)),
            )
        },
        trailing: Some(
            WidgetNode::row([
                WidgetNode::progress(0.65)
                    .with_key("host:progress")
                    .with_width(UiLength::Px(if compact { 100.0 } else { 150.0 })),
                WidgetNode::button("Cancel")
                    .with_key("status:cancel")
                    .with_accessible_name("Cancel rendering preview")
                    .with_style(cancel_style),
            ])
            .with_gap(14.0),
        ),
    }
}

fn page() -> WidgetTree {
    let mut brand_style = UiInlineStyle::default();
    brand_style.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::Accent);
    let mut muted = UiInlineStyle::default();
    muted.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::MutedText);
    muted.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(12.5));
    let center_title = WidgetNode::row([WidgetNode::label("Material.001")
        .with_role("centered-label")
        .with_width(UiLength::Fill(1.0))
        .with_style(muted)])
    .with_layout(UiLayoutNode {
        align_items: UiAlign::Center,
        ..UiLayoutNode::row(0.0)
    })
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0));
    let mut registry = CommandRegistry::default();
    for (id, label) in [("save", "Save"), ("export", "Export")] {
        if let Err(error) = registry.register(Command::new(id, label)) {
            error!("Command error: {error:?}");
        }
    }
    WidgetTree::new(
        WidgetNode::column([
            DesktopChrome::new("Material Studio").view_with_slots(
                &registry,
                &DesktopChromeAppearance::default(),
                DesktopChromeSlots {
                    leading: Some(
                        WidgetNode::label("◆")
                            .with_width(UiLength::Px(18.0))
                            .with_style(brand_style),
                    ),
                    center: Some(center_title),
                    trailing: None,
                },
            ),
            WidgetNode::column([])
                .with_key("status:workspace")
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0)),
            StatusBar.view_with_slots(&StatusBarAppearance::default(), status_slots(false)),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0)),
    )
}

fn install_editor(world: &mut World, host: Entity) {
    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "editor-columns".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![18.0, 63.0, 19.0],
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
    });
    let Ok(mut workspace) = Workspace::new(layout) else {
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

fn install(world: &mut World) {
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
    let Ok(root) = page().spawn(world, None) else {
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, size[0], size[1]),
        1.0,
    ));
    let mut keys = world.query::<(Entity, &UiKey)>();
    if let Some(host) = keys
        .iter(world)
        .find_map(|(entity, key)| (key.0 == "status:workspace").then_some(entity))
    {
        install_editor(world, host);
    }
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install(world);
}

fn process(world: &mut World) {
    let events = world.resource_mut::<WidgetEventQueue>().take();
    for event in &events {
        if let WidgetEvent::Activated { target } = event
            && world
                .get::<UiKey>(*target)
                .is_some_and(|key| key.0 == "status:cancel")
        {
            info!("Host should cancel its active job");
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Status Bar".into(),
                resolution: WindowResolution::new(1536, 900),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, process)
        .run();
}

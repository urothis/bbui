//! Search application commands over BBUI's composed editor workspace.
//! Run with `cargo run -p bbui --example command_palette`.

use bbui::prelude::*;
use bbui::widgets::{
    Command, CommandPalette, CommandPaletteAppearance, CommandPaletteSession, CommandRegistry,
    WidgetEventQueue,
};
use bbui_core::{UiRoutedEvent, UiRoutedEvents};
use bbui_style::Rgba;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[cfg(test)]
#[path = "command_palette/capture.rs"]
mod capture;
#[path = "editor_workspace/content.rs"]
mod editor_content;

#[derive(Resource)]
struct ExampleState {
    registry: CommandRegistry,
    palette: CommandPalette,
    root: Entity,
    window: Entity,
    session: Option<CommandPaletteSession>,
}

fn foreground(mut node: WidgetNode) -> WidgetNode {
    node.style
        .get_or_insert_with(UiInlineStyle::default)
        .set(UiStyleProperty::ZIndex, UiStyleValue::Integer(10_004));
    node.children = node.children.into_iter().map(foreground).collect();
    node
}

fn palette_page(
    palette: &CommandPalette,
    registry: &CommandRegistry,
    viewport_width: f32,
    viewport_height: f32,
) -> WidgetTree {
    let mut backdrop = UiInlineStyle::default();
    backdrop.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(Rgba(0.0, 0.0, 0.0, 0.62)),
    );
    backdrop.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(10_000));
    let mut appearance = CommandPaletteAppearance {
        title: "Workspace commands".into(),
        subtitle: Some("Search actions across the material editor".into()),
        footer: Some("↑ ↓  Move     Enter  Run command     Esc  Dismiss".into()),
        width: UiLength::Px((viewport_width - 32.0).clamp(100.0, 620.0)),
        max_height: Some((viewport_height - 24.0).max(180.0)),
        ..Default::default()
    };
    if viewport_width <= 480.0 {
        appearance.subtitle = None;
        appearance.footer = Some("↑ ↓ Navigate    Enter Run    Esc Close".into());
        appearance.show_shortcuts = false;
        appearance.show_descriptions = false;
    }
    appearance
        .surface_style
        .set(UiStyleProperty::ZIndex, UiStyleValue::Integer(10_002));
    let mut panel = palette.view_with(registry, &appearance);
    panel.children = panel.children.into_iter().map(foreground).collect();
    let mut layout = UiLayoutNode::column(0.0);
    layout.align_items = UiAlign::Center;
    layout.justify_content = UiJustify::Center;
    layout.z_index = 10_000;
    WidgetTree::new(
        WidgetNode::column([panel])
            .with_key("command-palette:backdrop")
            .with_role("box")
            .with_layout(layout)
            .with_style(backdrop)
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0)),
    )
}

fn install_editor(world: &mut World) {
    editor_content::configure_theme(&mut world.resource_mut::<Theme>());
    let layout = WorkspaceLayout::new(DockNode::Split {
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
    });
    let Ok(mut workspace) = Workspace::new(layout) else {
        return;
    };
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

fn install_palette(world: &mut World) {
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
    let mut registry = CommandRegistry::default();
    for (id, label, category, hint, icon) in [
        ("add-noise", "Add Noise Texture", "Nodes", "Shift+A", "✦"),
        ("add-ramp", "Add Color Ramp", "Nodes", "Shift+R", "◈"),
        ("frame-nodes", "Frame Selected Nodes", "View", "Home", "⌗"),
        ("assign-material", "Assign Material", "Material", "M", "◆"),
        (
            "material-settings",
            "Edit Material Settings",
            "Material",
            "",
            "⚙",
        ),
        ("save", "Save Workspace", "File", "Ctrl+S", "▣"),
        ("export", "Export Material", "File", "Ctrl+E", "↗"),
        ("render", "Render Material Preview", "Render", "F12", "◉"),
    ] {
        let mut command = Command::new(id, label);
        command.category = category.into();
        command.description = match id {
            "add-noise" => "Add procedural variation to the graph",
            "add-ramp" => "Map values to a color gradient",
            "frame-nodes" => "Fit selected nodes in the canvas",
            "assign-material" => "Apply Material.001 to the selected object",
            "material-settings" => "Open the material properties panel",
            "save" => "Save the current editor layout",
            "export" => "Write the material to a file",
            "render" => "Generate a material preview image",
            _ => "",
        }
        .into();
        command.shortcut_hint = Some(hint.into());
        command.icon = Some(icon.into());
        if let Err(error) = registry.register(command) {
            error!("Command registration failed: {error:?}");
        }
    }
    let mut palette = CommandPalette::default();
    palette.show();
    palette.query = "material".into();
    let Ok(root) = palette_page(&palette, &registry, size[0], size[1]).spawn(world, None) else {
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        bbui_layout::Rect::new(0.0, 0.0, size[0], size[1]),
        1.0,
    ));
    let session = CommandPaletteSession::begin(world, window, root);
    world.insert_resource(ExampleState {
        registry,
        palette,
        root,
        window,
        session,
    });
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    install_editor(world);
}

fn mount_palette(world: &mut World) {
    if world.contains_resource::<ExampleState>() {
        return;
    }
    install_palette(world);
}

fn process(world: &mut World) {
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let keys: Vec<_> = world
        .resource::<UiRoutedEvents>()
        .events
        .iter()
        .filter_map(|event| {
            if let UiRoutedEvent::Key {
                window,
                key,
                pressed: true,
                ..
            } = event
            {
                Some((*window, *key))
            } else {
                None
            }
        })
        .collect();
    let Some(mut state) = world.remove_resource::<ExampleState>() else {
        return;
    };
    let mut changed = false;
    for event in &events {
        let was_open = state.palette.open;
        if state
            .palette
            .handle_widget_event(world, event, &mut state.registry)
            .is_some()
        {
            changed = true;
        }
        changed |= was_open != state.palette.open;
        if matches!(event, bbui::widgets::WidgetEvent::TextChanged { .. }) {
            changed = true;
        }
    }
    for (window, key) in keys {
        if window != state.window
            || !matches!(
                key,
                bbui_core::UiKeyCode::Escape
                    | bbui_core::UiKeyCode::ArrowDown
                    | bbui_core::UiKeyCode::ArrowUp
                    | bbui_core::UiKeyCode::Enter
            )
        {
            continue;
        }
        let was_open = state.palette.open;
        let previous = state.palette.selected;
        let activated = state.palette.handle_key(key, &mut state.registry);
        changed |= activated.is_some()
            || previous != state.palette.selected
            || was_open != state.palette.open;
    }
    if !state.palette.open {
        if let Some(session) = state.session.take() {
            session.end(world);
        }
        if let Some(mut visible) = world.get_mut::<UiVisible>(state.root) {
            visible.0 = false;
        }
    }
    if changed
        && let Err(error) = palette_page(
            &state.palette,
            &state.registry,
            world
                .get::<Window>(state.window)
                .map_or(620.0, |window| window.resolution.width()),
            world
                .get::<Window>(state.window)
                .map_or(900.0, |window| window.resolution.height()),
        )
        .reconcile(world, state.root)
    {
        error!("Palette update failed: {error:?}");
    }
    if changed {
        let appearance = CommandPaletteAppearance {
            max_height: Some(
                (world
                    .get::<Window>(state.window)
                    .map_or(900.0, |window| window.resolution.height())
                    - 24.0)
                    .max(180.0),
            ),
            ..Default::default()
        };
        state
            .palette
            .scroll_selected_into_view(world, state.root, &appearance);
    }
    world.insert_resource(state);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, (mount_palette, process).chain())
        .run();
}

# Getting started

BBUI is a Bevy 0.20.1 plugin. Use Rust 1.99 or newer. In a host project, add Bevy and this workspace's public crate:

```toml
[dependencies]
bevy = "=0.20.1"
bbui = { git = "https://github.com/urothis/bbui", package = "bbui" }
```

A window UI needs a Bevy camera, a `UiLayoutRoot`, and a mounted `WidgetTree`. This example uses the same public path as [`controls.rs`](../crates/bbui/examples/controls.rs):

```rust
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bbui::prelude::*;
use bbui::widgets::WidgetEventQueue;

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world.query_filtered::<Entity, With<PrimaryWindow>>().single(world) else {
        return;
    };
    let root = world.spawn((
        UiNode,
        UiLayoutNode::column(8.0),
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 600.0, 400.0), 1.0),
    )).id();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("Settings"),
        WidgetNode::checkbox("Visible", true).with_key("visible"),
        WidgetNode::button("Apply").with_key("apply"),
    ]));
    if let Err(error) = tree.spawn(world, Some(root)) {
        error!("UI mount failed: {error:?}");
    }
}

fn handle_ui(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("UI event: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BbuiPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, handle_ui.after(UiSystems::Widgets))
        .run();
}
```

For a copyable, compiled example, run `cargo run -p bbui --example controls` from this workspace. Read [widgets](widgets.md) for events, state, and keyed updates. The [gallery](visuals/README.md) links screenshots for every example.

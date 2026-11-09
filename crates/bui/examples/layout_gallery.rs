//! A declarative layout gallery. Run with `cargo run -p bui --example layout_gallery`.
//! Resize the window to watch the responsive grid and wrapped flex row update.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bui::prelude::*;
use bui_layout::Rect;

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("layout gallery requires a primary window");
        return;
    };
    let mut tree = bui! {
        scroll {
            boxed {
                label("Layout gallery: resize the window"),
                property_row("Opacity") { slider_range("Value", 0.5, 0.0, 1.0, 0.05) },
                property_row("Mode") { dropdown("Mode", ["Object", "Edit", "Sculpt"], Some(0)) },
                grid(3) {
                    button("Grid A"), button("Grid B"), button("Grid C"),
                    button("Grid D"), button("Grid E"), button("Grid F"),
                },
                responsive_grid(140, 4) {
                    button("Responsive A"), button("Responsive B"),
                    button("Responsive C"), button("Responsive D"),
                },
                flex_row(true) {
                    button("Wrap one"), button("Wrap two"), button("Wrap three"),
                    button("Wrap four"), button("Wrap five"),
                },
                overlay { button("Behind"), label("Overlay text") },
                inline { label("Inline"), label("text"), label("flow") },
                boxed { label("The outer container clips and scrolls this gallery.") },
            }
        }
    };
    tree.root = tree
        .root
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0));
    let Ok(root) = tree.spawn(world, None) else {
        eprintln!("failed to spawn layout gallery tree");
        return;
    };
    world.entity_mut(root).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 900.0, 620.0),
        1.0,
    ));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BUI Layout Gallery".into(),
                resolution: WindowResolution::new(900, 620),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

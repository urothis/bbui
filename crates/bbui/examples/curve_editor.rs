//! Run with `cargo run -p bbui --example curve_editor`.
//! Shift-click adds a point; drag points or Bezier handles. Middle-drag pans,
//! the wheel zooms, Ctrl+Left/Right changes point selection, and arrows edit.

use bbui::prelude::*;
use bbui::widgets::{CurveEditor, Interpolation, WidgetEventQueue};
use bbui_layout::{Rect, UiEdges};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("curve editor example requires a primary window");
        return;
    };
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 8.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 900.0, 640.0), 1.0),
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

    let mut curve = CurveEditor::default();
    let Some(middle) = curve.add_point(0.48, 0.82) else {
        eprintln!("failed to insert middle curve point");
        return;
    };
    curve.interpolation = Interpolation::Smooth;
    curve.set_handles(middle, [-0.16, -0.1], [0.2, 0.14]);
    curve.select_point(middle);
    let tree = bbui! {
        column {
            label("Animation curve editors"),
            curve_editor("Ease", curve),
            curve_editor_default("Linear curve"),
            label("Shift-click adds. Drag points and handles. Middle-drag pans; wheel zooms."),
            label("A adds at the selected segment. Delete removes. C cycles Constant / Linear / Smooth."),
            label("Ctrl+Left/Right selects points. Arrow keys move the selected point. Ctrl+Z undoes edits."),
        }
    };
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("curve editor widget tree: {error:?}");
    }
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("curve editor: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Curve Editor".into(),
                resolution: WindowResolution::new(900, 640),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            log_widget_events.after(bbui_core::UiSystems::Widgets),
        )
        .run();
}

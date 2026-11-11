//! Run with `cargo run -p bbui --example text_area`.
//!
//! Click or Tab into the editor. It wraps at its viewport, scrolls with the
//! wheel, supports the platform IME, and uses the host clipboard for Ctrl+C/X/V.
//! Ctrl+Z and Ctrl+Shift+Z operate on the focused buffer's local edit history.

use bbui::prelude::*;
use bbui::widgets::{WidgetEventQueue, WidgetNode};
use bbui_core::UiChildren;
use bbui_layout::{Rect, UiEdges, UiLength};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

fn setup(world: &mut World) {
    let background = world.resource::<Theme>().background;
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("text area example requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(24.0),
                gap: 12.0,
                clip_contents: true,
                ..UiLayoutNode::column(12.0)
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 960.0, 720.0), 1.0),
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

    let text = "A wrapped TextArea uses Bevy's shaped glyph layout for painting, hit testing, selection, and caret placement.\n\
        Resize the window to see the lines reflow. Scroll inside the editor to move through this longer note.\n\
        Type normally or use a platform input method editor. Uncommitted composition is shown with an underline.\n\
        Copy, cut, and paste use the host clipboard bridge; undo and redo stay local to this text buffer.\n\
        The same buffer powers the displayed text, visual-line keyboard navigation, selection geometry, and caret.\n\
        Keep typing here to test wrapped caret movement, selection extension, and automatic caret scrolling.\n\
        The editor is intentionally long enough to expose its scroll behavior without a second control.";
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("Multiline TextArea"),
        WidgetNode::label(
            "Click or Tab to focus. Use wheel to scroll; Ctrl+C/X/V use the OS clipboard.",
        ),
        WidgetNode::label(
            "Arrow keys follow visual wrapped lines. Shift extends selection. Ctrl+Z / Ctrl+Shift+Z undo / redo.",
        ),
        WidgetNode::text_area("Notes", text).with_key("notes"),
        WidgetNode::label(
            "IME preedit, wrapping, selection, caret paint, and scrolling share one shaped layout.",
        ),
    ]));
    let Ok(tree_root) = tree.spawn(world, Some(root)) else {
        eprintln!("failed to spawn text area example tree");
        return;
    };
    let Some(area) = world.get::<UiChildren>(tree_root).and_then(|children| {
        children
            .0
            .iter()
            .copied()
            .find(|entity| world.get::<bbui_widgets::TextArea>(*entity).is_some())
    }) else {
        eprintln!("text area control is missing");
        return;
    };
    if let Some(mut layout) = world.get_mut::<UiLayoutNode>(area) {
        layout.size.width = UiLength::Px(820.0);
        layout.size.height = UiLength::Px(390.0);
    }
}

fn log_widget_events(mut events: ResMut<WidgetEventQueue>) {
    for event in events.take() {
        info!("text area event: {event:?}");
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI TextArea — wrapping, IME, clipboard, and scroll".into(),
                resolution: WindowResolution::new(960, 720),
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

//! Continuous, gradient, segmented, glowing, and indeterminate progress bars.
//!
//! Run with `cargo run -p bbui --example progress_gallery`.

use std::time::Duration;

use bbui::prelude::*;
use bbui_layout::Rect;
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Rgba;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 650.0;
const CYAN: Rgba = Rgba(0.27, 0.91, 0.88, 1.0);
const BLUE: Rgba = Rgba(0.28, 0.68, 1.0, 1.0);
const VIOLET: Rgba = Rgba(0.66, 0.42, 1.0, 1.0);
const PINK: Rgba = Rgba(1.0, 0.37, 0.7, 1.0);

fn card(title: &str, detail: &str, bar: WidgetNode) -> WidgetNode {
    WidgetNode::boxed([
        WidgetNode::label(title).with_height(UiLength::Px(36.0)),
        WidgetNode::label(detail).with_height(UiLength::Px(54.0)),
        bar.with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(24.0)),
    ])
    .with_width(UiLength::Px(345.0))
    .with_height(UiLength::Px(215.0))
    .with_padding(UiEdges::all(20.0))
    .with_gap(18.0)
}

fn root(world: &mut World, window: Entity) -> Entity {
    let background = world.resource::<Theme>().background;
    world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(25.0),
                ..default()
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
        .id()
}

fn entity(world: &World, key: &str) -> Option<Entity> {
    world.iter_entities().find_map(|entry| {
        entry
            .get::<UiKey>()
            .is_some_and(|value| value.0 == key)
            .then_some(entry.id())
    })
}

fn loop_motion(world: &mut World, key: &str, clip: MotionClip) {
    let Some(entity) = entity(world, key) else {
        return;
    };
    let mut player = world.resource_mut::<MotionPlayer>();
    if let Some(handle) = player.play(entity, clip, MotionPriority::Explicit) {
        player.set_repeats(handle, None);
    }
}

fn install_ui(world: &mut World, window: Entity) {
    let root = root(world, window);
    let gradient =
        ProgressStyle::gradient([CYAN, BLUE, VIOLET, PINK]).with_corners(ProgressCorners::Pill);
    let segmented = ProgressStyle::solid(CYAN)
        .with_segments(14)
        .with_corners(ProgressCorners::Pill);
    let luminous = ProgressStyle::gradient([CYAN, BLUE, VIOLET, PINK])
        .with_corners(ProgressCorners::Radius(8.0))
        .with_fill_border(UiBorderEffect::gradient(
            [CYAN, BLUE, VIOLET, PINK],
            1.5,
            8.0,
        ))
        .with_fill_glow(UiEdgeGlow::gradient(
            [
                Rgba(0.27, 0.91, 0.88, 0.22),
                Rgba(0.28, 0.68, 1.0, 0.22),
                Rgba(0.66, 0.42, 1.0, 0.22),
                Rgba(1.0, 0.37, 0.7, 0.22),
            ],
            11.0,
            0.0,
        ));
    let tree = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("PROGRESS  /  LIVE PREVIEW").with_height(UiLength::Px(28.0)),
            WidgetNode::label("Corners, shape, palette, and light can be configured independently")
                .with_height(UiLength::Px(42.0)),
            WidgetNode::row([
                card(
                    "Plain",
                    "Theme accent / 38%",
                    WidgetNode::progress(0.38).with_key("plain-bar"),
                ),
                card(
                    "Gradient",
                    "Pill corners / moving palette",
                    WidgetNode::progress(0.68)
                        .with_progress_style(gradient)
                        .with_key("gradient-bar"),
                ),
                card(
                    "Segmented",
                    "Rounded segments / changing value",
                    WidgetNode::progress(0.52)
                        .with_progress_style(segmented)
                        .with_key("segmented-bar"),
                ),
            ])
            .with_gap(24.0)
            .with_height(UiLength::Px(245.0)),
            WidgetNode::row([
                card(
                    "Luminous",
                    "8 px corners / animated edge light",
                    WidgetNode::progress(0.72)
                        .with_progress_style(luminous)
                        .with_key("luminous-bar"),
                ),
                card(
                    "Indeterminate",
                    "Moving indicator / still in reduced motion",
                    WidgetNode::indeterminate_progress()
                        .with_progress_style(
                            ProgressStyle::default().with_corners(ProgressCorners::Pill),
                        )
                        .with_key("indeterminate-bar"),
                ),
                card(
                    "Square",
                    "Zero-radius track and fill",
                    WidgetNode::progress(0.64)
                        .with_progress_style(
                            ProgressStyle::solid(BLUE).with_corners(ProgressCorners::Square),
                        )
                        .with_key("square-bar"),
                ),
            ])
            .with_gap(24.0)
            .with_height(UiLength::Px(245.0)),
        ])
        .with_gap(12.0),
    );
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("progress gallery setup: {error:?}");
        return;
    }
    if let Ok(track) = KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.0),
        Keyframe::at(Duration::from_secs(3), 1.0),
    ]) {
        loop_motion(
            world,
            "gradient-bar",
            MotionClip::new().track(progress_palette_phase_binding(), track),
        );
    }
    if let Ok(track) = KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.15),
        Keyframe::at(Duration::from_secs(1), 0.95),
        Keyframe::at(Duration::from_secs(2), 0.15),
    ]) {
        loop_motion(
            world,
            "segmented-bar",
            MotionClip::new().track(progress_value_binding(), track),
        );
    }
    if let Ok(track) = KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.35).curve(MotionCurve::EaseInOut),
        Keyframe::at(Duration::from_secs(1), 1.0).curve(MotionCurve::EaseInOut),
        Keyframe::at(Duration::from_secs(2), 0.35),
    ]) {
        loop_motion(
            world,
            "luminous-bar",
            MotionClip::new().track(progress_glow_strength_binding(), track),
        );
    }
    if let Ok(track) = KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.0),
        Keyframe::at(Duration::from_secs(4), 1.0),
    ]) {
        loop_motion(
            world,
            "luminous-bar",
            MotionClip::new()
                .track(progress_border_phase_binding(), track.clone())
                .track(progress_glow_phase_binding(), track),
        );
    }
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    if let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    {
        install_ui(world, window);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Progress Gallery".into(),
                resolution: WindowResolution::new(WIDTH as u32, HEIGHT as u32),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .run();
}

#[cfg(test)]
#[path = "progress_gallery/capture.rs"]
mod capture;

//! Animated gradient borders and edge glow on ordinary BBUI widgets.
//!
//! Run with `cargo run -p bbui --example visual_effects`.
//! Set `BBUI_VISUAL_VARIANTS=1` to compare eight solid/gradient variants.

use std::time::Duration;

use bbui::prelude::*;
use bbui::widgets::{
    UiBorderEffect, UiEdgeGlow, UiVisualEffects, WidgetNode, border_phase_binding,
    edge_glow_strength_binding,
};
use bbui_layout::Rect;
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Rgba;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[path = "visual_effects/variants.rs"]
mod variants;

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 720.0;
const BLUE: Rgba = Rgba(0.28, 0.68, 1.0, 1.0);
const VIOLET: Rgba = Rgba(0.66, 0.42, 1.0, 1.0);
const PINK: Rgba = Rgba(1.0, 0.37, 0.7, 1.0);
const CYAN: Rgba = Rgba(0.27, 0.91, 0.88, 1.0);

fn card(key: &str, title: &str, description: &str, effects: UiVisualEffects) -> WidgetNode {
    WidgetNode::boxed([
        WidgetNode::label(title).with_height(UiLength::Px(38.0)),
        WidgetNode::label(description).with_height(UiLength::Px(70.0)),
        WidgetNode::label("APPEARANCE").with_height(UiLength::Px(24.0)),
        WidgetNode::checkbox("Animate effect", true).with_height(UiLength::Px(30.0)),
        WidgetNode::property_row(
            "Mode",
            WidgetNode::dropdown("Mode", ["Ambient", "Focused", "Off"], Some(0))
                .with_height(UiLength::Px(27.0)),
        )
        .with_height(UiLength::Px(32.0)),
        WidgetNode::button("Apply to selection")
            .with_width(UiLength::Px(170.0))
            .with_height(UiLength::Px(32.0)),
    ])
    .with_key(key)
    .with_visual_effects(effects)
    .with_width(UiLength::Px(342.0))
    .with_height(UiLength::Px(380.0))
    .with_padding(UiEdges::all(20.0))
    .with_gap(12.0)
}

fn spawn_root(world: &mut World, window: Entity, padding: f32) -> Entity {
    let background = world.resource::<Theme>().background;
    world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(padding),
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

fn install_ui(world: &mut World, window: Entity) {
    let root = spawn_root(world, window, 34.0);

    let border = UiBorderEffect::gradient([BLUE, VIOLET, PINK, CYAN], 3.0, 8.0);
    let glow = UiEdgeGlow::solid(Rgba(0.27, 0.91, 0.88, 0.22), 10.0, 0.0);
    let tree = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("VISUAL EFFECTS  /  LIVE PREVIEW").with_height(UiLength::Px(26.0)),
            WidgetNode::label("Animated borders and light for editor surfaces")
                .with_height(UiLength::Px(44.0)),
            WidgetNode::row([
                card(
                    "border-card",
                    "Gradient border",
                    "Color travels around the card edge.",
                    UiVisualEffects::default().with_border(border.clone()),
                ),
                card(
                    "glow-card",
                    "Edge glow",
                    "Soft colored light hugs the card edge.",
                    UiVisualEffects::default().with_edge_glow(glow),
                ),
                card(
                    "combined-card",
                    "Border + edge glow",
                    "A brighter treatment for an active panel.",
                    UiVisualEffects::default()
                        .with_border(border)
                        .with_edge_glow(UiEdgeGlow::solid(Rgba(0.66, 0.42, 1.0, 0.22), 10.0, 0.0)),
                ),
            ])
            .with_gap(40.0)
            .with_height(UiLength::Px(430.0)),
            WidgetNode::label(
                "Preview uses the BBUI renderer. Borders rotate; the combined glow breathes.",
            )
            .with_height(UiLength::Px(28.0)),
        ])
        .with_gap(18.0),
    );
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("visual effects example setup: {error:?}");
        return;
    }
    for key in ["border-card", "combined-card"] {
        let entity = world.iter_entities().find_map(|entry| {
            entry
                .get::<UiKey>()
                .is_some_and(|value| value.0 == key)
                .then_some(entry.id())
        });
        let Some(entity) = entity else { continue };
        let Ok(frames) = KeyframeTrack::new([
            Keyframe::at(Duration::ZERO, 0.0),
            Keyframe::at(Duration::from_secs(7), 1.0),
        ]) else {
            continue;
        };
        let clip = MotionClip::new().track(border_phase_binding(), frames);
        let mut player = world.resource_mut::<MotionPlayer>();
        if let Some(handle) = player.play(entity, clip, MotionPriority::Explicit) {
            player.set_repeats(handle, None);
        }
    }
    let entity = world.iter_entities().find_map(|entry| {
        entry
            .get::<UiKey>()
            .is_some_and(|value| value.0 == "combined-card")
            .then_some(entry.id())
    });
    if let Some(entity) = entity {
        let Ok(frames) = KeyframeTrack::new([
            Keyframe::at(Duration::ZERO, 0.55).curve(MotionCurve::EaseInOut),
            Keyframe::at(Duration::from_secs(2), 1.0).curve(MotionCurve::EaseInOut),
            Keyframe::at(Duration::from_secs(4), 0.55),
        ]) else {
            return;
        };
        let clip = MotionClip::new().track(edge_glow_strength_binding(), frames);
        let mut player = world.resource_mut::<MotionPlayer>();
        if let Some(handle) = player.play(entity, clip, MotionPriority::Explicit) {
            player.set_repeats(handle, None);
        }
    }
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    if let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    {
        if std::env::var_os("BBUI_VISUAL_VARIANTS").is_some() {
            variants::install(world, window);
        } else {
            install_ui(world, window);
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Visual Effects".into(),
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
#[path = "visual_effects/capture.rs"]
mod capture;

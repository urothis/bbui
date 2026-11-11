//! Compare keyframe curves visually. Press F5 to replay the three bars.
//!
//! Run with `cargo run -p bbui --example keyframe_gallery`.

use std::time::Duration;

use bbui::prelude::*;
use bbui_layout::{Rect, UiEdges};
use bbui_motion::{Keyframe, KeyframeTrack, MotionClip, MotionCurve, MotionPlayer, MotionPriority};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::{Rgba, UiInlineStyle, UiStyleProperty, UiStyleValue, style_motion_binding};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

const BARS: [(&str, MotionCurve, Rgba); 3] = [
    ("Linear", MotionCurve::Linear, Rgba(0.29, 0.58, 0.86, 1.0)),
    ("Ease In", MotionCurve::EaseIn, Rgba(0.62, 0.43, 0.84, 1.0)),
    (
        "Ease Out",
        MotionCurve::EaseOut,
        Rgba(0.30, 0.70, 0.59, 1.0),
    ),
];

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("keyframe gallery requires a primary window");
        return;
    };
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(28.0),
                gap: 14.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 760.0, 380.0), 1.0),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: world.resource::<Theme>().background,
            }),
        ))
        .id();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::label("KEYFRAME CURVES"),
        WidgetNode::label("Three curves, one out-and-back timeline. Press F5 to replay."),
        WidgetNode::label("Linear"),
        WidgetNode::button("Linear")
            .with_key("linear-bar")
            .with_width(UiLength::Px(80.0))
            .with_height(UiLength::Px(38.0)),
        WidgetNode::label("Ease In"),
        WidgetNode::button("Ease In")
            .with_key("ease-in-bar")
            .with_width(UiLength::Px(80.0))
            .with_height(UiLength::Px(38.0)),
        WidgetNode::label("Ease Out"),
        WidgetNode::button("Ease Out")
            .with_key("ease-out-bar")
            .with_width(UiLength::Px(80.0))
            .with_height(UiLength::Px(38.0)),
    ]));
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("keyframe gallery setup: {error:?}");
        return;
    }
    for (index, (_, _, color)) in BARS.iter().enumerate() {
        let key = ["linear-bar", "ease-in-bar", "ease-out-bar"][index];
        let entity = world.iter_entities().find_map(|entry| {
            entry
                .get::<UiKey>()
                .is_some_and(|value| value.0 == key)
                .then_some(entry.id())
        });
        if let Some(entity) = entity {
            let mut style = UiInlineStyle::default();
            style.set(UiStyleProperty::Background, UiStyleValue::Color(*color));
            style.set(
                UiStyleProperty::Width,
                UiStyleValue::Length(UiLength::Px(80.0)),
            );
            world.entity_mut(entity).insert(style);
        }
    }
}

fn replay(world: &mut World) {
    let pressed = world
        .resource::<ButtonInput<KeyCode>>()
        .just_pressed(KeyCode::F5);
    if !pressed {
        return;
    }
    for (index, (_, curve, _)) in BARS.iter().enumerate() {
        let key = ["linear-bar", "ease-in-bar", "ease-out-bar"][index];
        let entity = world.iter_entities().find_map(|entry| {
            entry
                .get::<UiKey>()
                .is_some_and(|value| value.0 == key)
                .then_some(entry.id())
        });
        let Some(entity) = entity else { continue };
        let Ok(frames) = KeyframeTrack::new([
            Keyframe::at(Duration::ZERO, UiStyleValue::Length(UiLength::Px(80.0))).curve(*curve),
            Keyframe::at(
                Duration::from_millis(900),
                UiStyleValue::Length(UiLength::Px(660.0)),
            )
            .curve(MotionCurve::EaseInOut),
            Keyframe::at(
                Duration::from_millis(1800),
                UiStyleValue::Length(UiLength::Px(80.0)),
            ),
        ]) else {
            continue;
        };
        let clip = MotionClip::new().track(style_motion_binding(UiStyleProperty::Width), frames);
        world
            .resource_mut::<MotionPlayer>()
            .play(entity, clip, MotionPriority::Explicit);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Keyframe Gallery".into(),
                resolution: WindowResolution::new(760, 380),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, replay.before(bbui_core::UiSystems::Motion))
        .run();
}

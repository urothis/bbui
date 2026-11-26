//! Four toast severities using BBUI's animated gradient border effect.
//! Run with `cargo run -p bbui --example toast_gradient_borders`.

use std::{collections::HashSet, time::Duration};

use bbui::prelude::*;
use bbui::widgets::WidgetEventQueue;
use bbui_core::UiChildren;
use bbui_layout::Rect;
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Rgba;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

#[cfg(test)]
#[path = "toast_gradient_borders/capture.rs"]
mod capture;

const WIDTH: u32 = 1100;
const HEIGHT: u32 = 760;

/// Change these two values to customize the reusable border-phase clip.
#[derive(Clone, Copy)]
struct BorderMotion {
    cycle: Duration,
    curve: MotionCurve,
}

const BORDER_MOTION: BorderMotion = BorderMotion {
    cycle: Duration::from_secs(2),
    curve: MotionCurve::Linear,
};

fn toast_motion() -> ToastMotion {
    ToastMotion {
        enter_travel_curve: MotionCurve::cubic_bezier(0.16, 0.95, 0.3, 1.0)
            .unwrap_or(MotionCurve::EaseOut),
        exit_opacity_curve: MotionCurve::EaseInOut,
        ..ToastMotion::default()
    }
}

#[derive(Resource)]
struct ExampleState {
    window: Entity,
    toast_root: Entity,
    appearance: ToastAppearance,
    generation: u64,
    border_motion: BorderMotion,
    animated_cards: HashSet<Entity>,
}

fn animate_borders(world: &mut World, state: &mut ExampleState) {
    let cards: HashSet<_> = world
        .get::<UiChildren>(state.toast_root)
        .into_iter()
        .flat_map(|children| children.0.iter().copied())
        .filter(|&entity| {
            world
                .get::<UiVisualEffects>(entity)
                .is_some_and(|effects| effects.border.is_some())
        })
        .collect();
    state.animated_cards.retain(|entity| cards.contains(entity));
    let reduced = world
        .get_resource::<UiReducedMotion>()
        .is_some_and(|value| value.0)
        || world.resource::<Theme>().motion_scale <= 0.0;
    if reduced || state.border_motion.cycle.is_zero() {
        let mut player = world.resource_mut::<MotionPlayer>();
        for entity in state.animated_cards.drain() {
            player.cancel_binding(entity, border_phase_binding().id());
        }
        for entity in cards {
            if let Some(mut effects) = world.get_mut::<UiVisualEffects>(entity)
                && let Some(border) = &mut effects.border
                && border.phase != 0.0
            {
                border.phase = 0.0;
            }
        }
        return;
    }
    let new_cards: Vec<_> = cards.difference(&state.animated_cards).copied().collect();
    for entity in new_cards {
        let Ok(track) = KeyframeTrack::new([
            Keyframe::at(Duration::ZERO, 0.0).curve(state.border_motion.curve),
            Keyframe::at(state.border_motion.cycle, 1.0),
        ]) else {
            continue;
        };
        let clip = MotionClip::new().track(border_phase_binding(), track);
        let mut player = world.resource_mut::<MotionPlayer>();
        if let Some(handle) = player.play(entity, clip, MotionPriority::Explicit) {
            player.set_repeats(handle, None);
            state.animated_cards.insert(entity);
        }
    }
}

fn text_style(size: f32, color: ThemeColorToken) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(size));
    style.set_color_token(UiStyleProperty::Foreground, color);
    style
}

fn gradient(colors: [Rgba; 4]) -> UiVisualEffects {
    UiVisualEffects::default()
        .with_border(UiBorderEffect::gradient(colors, 2.0, 8.0).with_strength(0.95))
}

fn gradient_appearance() -> ToastAppearance {
    let mut appearance = ToastAppearance {
        gap: 14.0,
        card_padding: UiEdges::horizontal_vertical(14.0, 12.0),
        message_wrap_columns: 38,
        ..ToastAppearance::default()
    };
    appearance
        .card_style
        .set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(0.0));
    appearance.info_effects = Some(gradient([
        Rgba(0.25, 0.57, 0.98, 1.0),
        Rgba(0.34, 0.79, 0.98, 1.0),
        Rgba(0.46, 0.48, 0.95, 1.0),
        Rgba(0.19, 0.67, 0.92, 1.0),
    ]));
    appearance.success_effects = Some(gradient([
        Rgba(0.16, 0.75, 0.42, 1.0),
        Rgba(0.37, 0.88, 0.62, 1.0),
        Rgba(0.08, 0.61, 0.51, 1.0),
        Rgba(0.47, 0.83, 0.44, 1.0),
    ]));
    appearance.warning_effects = Some(gradient([
        Rgba(0.98, 0.62, 0.14, 1.0),
        Rgba(1.0, 0.78, 0.29, 1.0),
        Rgba(0.96, 0.42, 0.17, 1.0),
        Rgba(0.91, 0.69, 0.21, 1.0),
    ]));
    appearance.error_effects = Some(gradient([
        Rgba(0.96, 0.30, 0.32, 1.0),
        Rgba(1.0, 0.49, 0.47, 1.0),
        Rgba(0.82, 0.24, 0.47, 1.0),
        Rgba(0.98, 0.39, 0.28, 1.0),
    ]));
    appearance
}

fn page(center: &mut ToastCenter, window: Entity, appearance: &ToastAppearance) -> WidgetTree {
    let copy = WidgetNode::column([
        WidgetNode::label("TOASTS / LIVE BORDERS")
            .with_height(UiLength::Px(24.0))
            .with_style(text_style(12.0, ThemeColorToken::Accent)),
        WidgetNode::label("A color for every kind")
            .with_height(UiLength::Px(54.0))
            .with_style(text_style(29.0, ThemeColorToken::Text)),
        WidgetNode::label("Four toast cards, four rotating palettes.")
            .with_height(UiLength::Px(30.0))
            .with_style(text_style(14.0, ThemeColorToken::MutedText)),
        WidgetNode::label("Dismiss a card to see its exit motion.")
            .with_height(UiLength::Px(30.0))
            .with_style(text_style(13.0, ThemeColorToken::MutedText)),
    ])
    .with_width(UiLength::Px(560.0))
    .with_gap(10.0);
    let content = WidgetNode::column([
        WidgetNode::column([]).with_height(UiLength::Fill(1.0)),
        copy,
        WidgetNode::column([]).with_height(UiLength::Fill(1.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
    .with_padding(UiEdges::all(58.0));
    WidgetTree::new(center.floating_view(
        window,
        content,
        appearance,
        &ToastOverlayAppearance {
            width: 320.0,
            margin: UiEdges::all(42.0),
            ..Default::default()
        },
    ))
}

fn install(world: &mut World, window: Entity) {
    let appearance = gradient_appearance();
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode::default(),
            UiLayoutRoot::new(
                window,
                Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
                1.0,
            ),
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
    {
        let mut center = world.resource_mut::<ToastCenter>();
        center.set_visible_limit(4);
        center.set_motion(ToastMotion {
            enter: Duration::ZERO,
            ..toast_motion()
        });
        for (severity, title, message) in [
            (
                ToastSeverity::Error,
                "Export failed",
                "Output could not be written",
            ),
            (
                ToastSeverity::Warning,
                "Storage almost full",
                "Check available disk space",
            ),
            (
                ToastSeverity::Success,
                "Render complete",
                "Preview is ready to inspect",
            ),
            (
                ToastSeverity::Info,
                "Workspace ready",
                "Project assets are indexed",
            ),
        ] {
            center.post(
                window,
                severity,
                Some(title.into()),
                message,
                ToastLifetime::Persistent,
            );
        }
        center.set_motion(toast_motion());
    };
    let tree = page(
        &mut world.resource_mut::<ToastCenter>(),
        window,
        &appearance,
    );
    if let Err(error) = tree.spawn(world, Some(root)) {
        error!("Toast gradient example setup: {error:?}");
        return;
    }
    let Some(toast_root) = world.iter_entities().find_map(|entry| {
        entry
            .get::<UiKey>()
            .is_some_and(|key| key.0 == "toast:center")
            .then_some(entry.id())
    }) else {
        error!("Toast gradient example setup: toast stack was not mounted");
        return;
    };
    let mut state = ExampleState {
        window,
        toast_root,
        appearance,
        generation: world.resource::<ToastCenter>().generation(),
        border_motion: BORDER_MOTION,
        animated_cards: HashSet::new(),
    };
    animate_borders(world, &mut state);
    world.insert_resource(state);
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    if let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    {
        install(world, window);
    }
}

fn process(world: &mut World) {
    let events = world.resource_mut::<WidgetEventQueue>().take();
    let Some(mut state) = world.remove_resource::<ExampleState>() else {
        return;
    };
    let Some(mut center) = world.remove_resource::<ToastCenter>() else {
        world.insert_resource(state);
        return;
    };
    for event in &events {
        center.handle_widget_event(world, event);
    }
    if state.generation != center.generation() {
        let stack = center.floating_stack(
            state.window,
            &state.appearance,
            &ToastOverlayAppearance {
                width: 320.0,
                margin: UiEdges::all(42.0),
                ..Default::default()
            },
        );
        match WidgetTree::new(stack).reconcile(world, state.toast_root) {
            Ok(root) => state.toast_root = root,
            Err(error) => error!("Toast gradient example update: {error:?}"),
        }
    }
    state.generation = center.generation();
    world.insert_resource(center);
    animate_borders(world, &mut state);
    world.insert_resource(state);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Toast Gradients".into(),
                resolution: WindowResolution::new(WIDTH, HEIGHT),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, process)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_loops_follow_new_cards_and_reduced_motion() {
        let mut world = World::new();
        world.init_resource::<Theme>();
        world.init_resource::<MotionPlayer>();
        let effect = gradient([Rgba(0.2, 0.6, 1.0, 1.0); 4]);
        let first = world.spawn((UiKey("toast:1".into()), effect.clone())).id();
        let toast_root = world.spawn(UiChildren(vec![first])).id();
        let mut state = ExampleState {
            window: Entity::PLACEHOLDER,
            toast_root,
            appearance: ToastAppearance::default(),
            generation: 0,
            border_motion: BORDER_MOTION,
            animated_cards: HashSet::new(),
        };
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 1);
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 1);

        let second = world.spawn((UiKey("toast:2".into()), effect)).id();
        world
            .get_mut::<UiChildren>(toast_root)
            .unwrap()
            .0
            .push(second);
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 2);

        world.insert_resource(UiReducedMotion(true));
        let card = state.animated_cards.iter().copied().next().unwrap();
        world
            .get_mut::<UiVisualEffects>(card)
            .unwrap()
            .border
            .as_mut()
            .unwrap()
            .phase = 0.4;
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 0);
        assert!(state.animated_cards.is_empty());
        assert_eq!(
            world
                .get::<UiVisualEffects>(card)
                .unwrap()
                .border
                .as_ref()
                .unwrap()
                .phase,
            0.0
        );

        world.insert_resource(UiReducedMotion(false));
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 2);

        world.resource_mut::<Theme>().motion_scale = 0.0;
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 0);
        world.resource_mut::<Theme>().motion_scale = 1.0;
        animate_borders(&mut world, &mut state);
        assert_eq!(world.resource::<MotionPlayer>().active_count(), 2);
    }
}

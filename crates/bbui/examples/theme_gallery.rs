//! Run with `cargo run -p bbui --example theme_gallery`.
//! Press F6 or the button to transition between the dark and light themes.

use bbui::prelude::*;
use bbui::widgets::{EditorChrome, WidgetEvent, WidgetEventQueue, WidgetNode};
use bbui_layout::{Rect, UiEdges};
use bbui_motion::MotionCurve;
use bbui_render::{ClipRect, Primitive, UiPaint};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use std::time::Duration;

const WIDTH: f32 = 720.0;
const HEIGHT: f32 = 430.0;
const TRANSITION: Duration = Duration::from_millis(1100);

#[derive(Resource)]
struct ThemePreview {
    button: Entity,
    active: Option<ThemeFade>,
}

struct ThemeFade {
    from: Theme,
    to: Theme,
    elapsed: Duration,
}

fn blend_theme(from: &Theme, to: &Theme, progress: f32) -> Theme {
    let mut theme = from.clone();
    macro_rules! mix_color {
        ($($field:ident),+ $(,)?) => { $(theme.$field = from.$field.mix(to.$field, progress);)+ };
    }
    mix_color!(
        background,
        surface,
        surface_hover,
        surface_pressed,
        accent,
        border,
        selection,
        error,
        focus,
        disabled_surface,
        disabled_border,
    );
    // Keep labels readable while the two surfaces pass through middle gray.
    let text_progress = ((progress - 0.42) / 0.16).clamp(0.0, 1.0);
    theme.text = from.text.mix(to.text, text_progress);
    theme.text_muted = from.text_muted.mix(to.text_muted, text_progress);
    theme.disabled = from.disabled.mix(to.disabled, text_progress);
    theme
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("theme gallery requires a primary window");
        return;
    };
    let theme = world.resource::<Theme>();
    let background = theme.background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(20.0),
                gap: 0.0,
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
        .id();

    let tree = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::row([
                WidgetNode::column([
                    WidgetNode::label("APPEARANCE / LIVE THEME").with_height(UiLength::Px(23.0)),
                    WidgetNode::label("A compact editor surface with controls in context")
                        .with_height(UiLength::Px(22.0)),
                ])
                .with_gap(1.0)
                .with_width(UiLength::Fill(1.0)),
                WidgetNode::button("Switch theme  F6")
                    .with_key("theme-switch")
                    .with_width(UiLength::Px(148.0))
                    .with_height(UiLength::Px(28.0)),
            ])
            .with_gap(12.0)
            .with_height(UiLength::Px(54.0)),
            EditorChrome::primary_panel(
                "Object",
                true,
                [
                    WidgetNode::label("Transform").with_height(UiLength::Px(21.0)),
                    WidgetNode::number("X", 0.0, -100.0, 100.0, 0.1)
                        .with_role("vector-axis-meters")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(24.0)),
                    WidgetNode::number("Y", 1.25, -100.0, 100.0, 0.1)
                        .with_role("vector-axis-meters")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(24.0)),
                    WidgetNode::number("Z", 90.0, -360.0, 360.0, 1.0)
                        .with_role("vector-axis-degrees")
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(24.0)),
                    WidgetNode::label("Display").with_height(UiLength::Px(21.0)),
                    WidgetNode::row([
                        WidgetNode::checkbox("Visible", true)
                            .with_role("filled-checkbox")
                            .with_width(UiLength::Fill(1.0)),
                        WidgetNode::checkbox("Selectable", true)
                            .with_role("filled-checkbox")
                            .with_width(UiLength::Fill(1.0)),
                        WidgetNode::checkbox("Render", false)
                            .with_role("filled-checkbox")
                            .with_width(UiLength::Fill(1.0)),
                    ])
                    .with_gap(7.0)
                    .with_height(UiLength::Px(26.0)),
                ],
            )
            .with_width(UiLength::Fill(1.0))
            .with_gap(3.0),
            EditorChrome::primary_panel(
                "Material",
                true,
                [
                    WidgetNode::row([
                        WidgetNode::text_field("Name", "Ceramic Blue")
                            .with_width(UiLength::Px(300.0))
                            .with_height(UiLength::Px(25.0)),
                        WidgetNode::dropdown(
                            "Blend",
                            ["Opaque", "Alpha Blend", "Additive"],
                            Some(0),
                        )
                        .with_role("compact-dropdown")
                        .with_width(UiLength::Px(300.0))
                        .with_height(UiLength::Px(25.0)),
                    ])
                    .with_gap(12.0)
                    .with_height(UiLength::Px(28.0)),
                    WidgetNode::slider_range("Roughness", 0.42, 0.0, 1.0, 0.01)
                        .with_role("filled-slider")
                        .with_height(UiLength::Px(26.0)),
                ],
            )
            .with_width(UiLength::Fill(1.0))
            .with_gap(4.0),
            WidgetNode::label(
                "F6 switches themes  ·  All controls remain interactive during the transition",
            )
            .with_height(UiLength::Px(24.0)),
        ])
        .with_gap(10.0)
        .with_width(UiLength::Fill(1.0)),
    );
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("theme gallery setup: {error:?}");
        return;
    }
    let Some(button) = world
        .iter_entities()
        .find(|entity| {
            entity
                .get::<UiKey>()
                .is_some_and(|key| key.0 == "theme-switch")
        })
        .map(|entity| entity.id())
    else {
        eprintln!("theme switch button is missing");
        return;
    };
    world.insert_resource(ThemePreview {
        button,
        active: None,
    });
}

fn request_theme_switch(
    keys: Res<ButtonInput<KeyCode>>,
    mut events: ResMut<WidgetEventQueue>,
    theme: Res<Theme>,
    mut preview: ResMut<ThemePreview>,
) {
    let clicked = events.take().into_iter().any(
        |event| matches!(event, WidgetEvent::Activated { target } if target == preview.button),
    );
    if !clicked && !keys.just_pressed(KeyCode::F6) {
        return;
    }
    let current_target = preview
        .active
        .as_ref()
        .map_or(theme.kind, |fade| fade.to.kind);
    let next = match current_target {
        ThemeKind::Dark => ThemeKind::Light,
        ThemeKind::Light => ThemeKind::Dark,
    };
    preview.active = Some(ThemeFade {
        from: theme.clone(),
        to: Theme::new(next),
        elapsed: Duration::ZERO,
    });
}

fn advance_theme(
    time: Res<Time>,
    mut preview: ResMut<ThemePreview>,
    mut theme: ResMut<Theme>,
    mut roots: Query<&mut UiPaint, With<UiLayoutRoot>>,
) {
    let Some(fade) = preview.active.as_mut() else {
        return;
    };
    fade.elapsed += time.delta();
    let progress =
        MotionCurve::EaseInOut.sample(fade.elapsed.as_secs_f32() / TRANSITION.as_secs_f32());
    *theme = blend_theme(&fade.from, &fade.to, progress);
    if fade.elapsed >= TRANSITION {
        *theme = fade.to.clone();
        preview.active = None;
    }
    for mut paint in &mut roots {
        if let Primitive::RoundedRect { color, .. } = &mut paint.0 {
            *color = theme.background;
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI Theme Gallery".into(),
                resolution: WindowResolution::new(WIDTH as u32, HEIGHT as u32),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (request_theme_switch, advance_theme)
                .chain()
                .after(bbui_core::UiSystems::Widgets),
        )
        .run();
}

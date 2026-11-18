//! Solid and multicolor border/glow combinations on the same card geometry.

use super::*;

const GLOW_PALETTE: [Rgba; 4] = [
    Rgba(0.28, 0.68, 1.0, 0.22),
    Rgba(0.66, 0.42, 1.0, 0.22),
    Rgba(1.0, 0.37, 0.7, 0.22),
    Rgba(0.27, 0.91, 0.88, 0.22),
];

fn card(key: &str, title: &str, detail: &str, effects: UiVisualEffects) -> WidgetNode {
    WidgetNode::boxed([
        WidgetNode::label(title).with_height(UiLength::Px(32.0)),
        WidgetNode::label(detail).with_height(UiLength::Px(48.0)),
        WidgetNode::button("Preview control")
            .with_width(UiLength::Px(140.0))
            .with_height(UiLength::Px(29.0)),
    ])
    .with_key(key)
    .with_width(UiLength::Px(250.0))
    .with_height(UiLength::Px(220.0))
    .with_padding(UiEdges::all(16.0))
    .with_gap(10.0)
    .with_visual_effects(effects)
}

fn entity(world: &World, key: &str) -> Option<Entity> {
    world.iter_entities().find_map(|entry| {
        entry
            .get::<UiKey>()
            .is_some_and(|value| value.0 == key)
            .then_some(entry.id())
    })
}

fn play_loop(world: &mut World, key: &str, clip: MotionClip) {
    let Some(entity) = entity(world, key) else {
        return;
    };
    let mut player = world.resource_mut::<MotionPlayer>();
    if let Some(handle) = player.play(entity, clip, MotionPriority::Explicit) {
        player.set_repeats(handle, None);
    }
}

fn pulse() -> Option<KeyframeTrack<f32>> {
    KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.35).curve(MotionCurve::EaseInOut),
        Keyframe::at(Duration::from_secs(1), 1.0).curve(MotionCurve::EaseInOut),
        Keyframe::at(Duration::from_secs(2), 0.35),
    ])
    .ok()
}

fn rotation() -> Option<KeyframeTrack<f32>> {
    KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.0),
        Keyframe::at(Duration::from_secs(2), 1.0),
    ])
    .ok()
}

pub(super) fn install(world: &mut World, window: Entity) {
    let root = spawn_root(world, window, 25.0);
    let border = UiBorderEffect::gradient([BLUE, VIOLET, PINK, CYAN], 3.0, 8.0);
    let glow = UiEdgeGlow::gradient(GLOW_PALETTE, 10.0, 0.0);
    let tree = WidgetTree::new(
        WidgetNode::column([
            WidgetNode::label("VISUAL EFFECTS  /  SOLID AND GRADIENT")
                .with_height(UiLength::Px(25.0)),
            WidgetNode::label("Static color or motion, independently for borders and edge glow")
                .with_height(UiLength::Px(26.0)),
            WidgetNode::label("BORDER").with_height(UiLength::Px(24.0)),
            WidgetNode::row([
                card(
                    "border-solid-static",
                    "Solid / static",
                    "One color, fixed intensity",
                    UiVisualEffects::default().with_border(UiBorderEffect::solid(CYAN, 3.0, 8.0)),
                ),
                card(
                    "border-solid-animated",
                    "Solid / animated",
                    "One color, pulsing intensity",
                    UiVisualEffects::default()
                        .with_border(UiBorderEffect::solid(CYAN, 3.0, 8.0).with_strength(0.35)),
                ),
                card(
                    "border-gradient-static",
                    "Gradient / static",
                    "Four fixed colors",
                    UiVisualEffects::default().with_border(border.clone()),
                ),
                card(
                    "border-gradient-animated",
                    "Gradient / animated",
                    "Colors travel around the edge",
                    UiVisualEffects::default().with_border(border),
                ),
            ])
            .with_gap(40.0)
            .with_height(UiLength::Px(230.0)),
            WidgetNode::label("EDGE GLOW").with_height(UiLength::Px(24.0)),
            WidgetNode::row([
                card(
                    "glow-solid-static",
                    "Solid / static",
                    "One color, fixed intensity",
                    UiVisualEffects::default().with_edge_glow(UiEdgeGlow::solid(
                        Rgba(0.27, 0.91, 0.88, 0.22),
                        10.0,
                        0.0,
                    )),
                ),
                card(
                    "glow-solid-animated",
                    "Solid / animated",
                    "One color, pulsing intensity",
                    UiVisualEffects::default().with_edge_glow(
                        UiEdgeGlow::solid(Rgba(0.27, 0.91, 0.88, 0.22), 10.0, 0.0)
                            .with_strength(0.35),
                    ),
                ),
                card(
                    "glow-gradient-static",
                    "Gradient / static",
                    "Four fixed colors",
                    UiVisualEffects::default().with_edge_glow(glow.clone()),
                ),
                card(
                    "glow-gradient-animated",
                    "Gradient / animated",
                    "Colors travel around the edge",
                    UiVisualEffects::default().with_edge_glow(glow),
                ),
            ])
            .with_gap(40.0)
            .with_height(UiLength::Px(230.0)),
            WidgetNode::label("All eight variants use the same BBUI widget API.")
                .with_height(UiLength::Px(24.0)),
        ])
        .with_gap(10.0),
    );
    if let Err(error) = tree.spawn(world, Some(root)) {
        eprintln!("visual effect variants setup: {error:?}");
        return;
    }
    if let Some(track) = pulse() {
        play_loop(
            world,
            "border-solid-animated",
            MotionClip::new().track(border_strength_binding(), track),
        );
    }
    if let Some(track) = rotation() {
        play_loop(
            world,
            "border-gradient-animated",
            MotionClip::new().track(border_phase_binding(), track),
        );
    }
    if let Some(track) = pulse() {
        play_loop(
            world,
            "glow-solid-animated",
            MotionClip::new().track(edge_glow_strength_binding(), track),
        );
    }
    if let Some(track) = rotation() {
        play_loop(
            world,
            "glow-gradient-animated",
            MotionClip::new().track(edge_glow_phase_binding(), track),
        );
    }
}

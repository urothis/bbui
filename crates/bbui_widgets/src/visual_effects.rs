//! Optional paint effects for any declarative or ECS-managed BBUI node.

use bbui_core::UiDirtyDomain;
use bbui_motion::MotionBinding;
use bbui_style::{Rgba, Theme, ThemeColorToken};
use bevy_ecs::prelude::{Component, World};

/// A fixed color or a color resolved from the current theme when painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiEffectColor {
    Fixed(Rgba),
    Theme(ThemeColorToken),
}

impl UiEffectColor {
    pub fn resolve(self, theme: &Theme) -> Rgba {
        match self {
            Self::Fixed(color) => color,
            Self::Theme(token) => theme.color(token),
        }
    }
}

impl From<Rgba> for UiEffectColor {
    fn from(color: Rgba) -> Self {
        Self::Fixed(color)
    }
}

impl From<ThemeColorToken> for UiEffectColor {
    fn from(token: ThemeColorToken) -> Self {
        Self::Theme(token)
    }
}

/// A rounded border with one color or an evenly spaced palette.
#[derive(Debug, Clone, PartialEq)]
pub struct UiBorderEffect {
    pub colors: Vec<UiEffectColor>,
    pub width: f32,
    pub radius: f32,
    /// Rotation of a multicolor palette, measured in turns.
    pub phase: f32,
    /// Multiplier for every color's alpha.
    pub strength: f32,
}

impl UiBorderEffect {
    pub fn solid(color: impl Into<UiEffectColor>, width: f32, radius: f32) -> Self {
        Self::gradient([color], width, radius)
    }

    pub fn gradient(
        colors: impl IntoIterator<Item = impl Into<UiEffectColor>>,
        width: f32,
        radius: f32,
    ) -> Self {
        Self {
            colors: colors.into_iter().map(Into::into).collect(),
            width,
            radius,
            phase: 0.0,
            strength: 1.0,
        }
    }

    pub fn with_phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_strength(mut self, strength: f32) -> Self {
        self.strength = strength;
        self
    }
}

/// Colored light following a node's silhouette.
#[derive(Debug, Clone, PartialEq)]
pub struct UiEdgeGlow {
    pub colors: Vec<UiEffectColor>,
    pub blur: f32,
    pub spread: f32,
    pub strength: f32,
    /// Rotation of a multicolor palette, measured in turns.
    pub phase: f32,
}

impl UiEdgeGlow {
    pub fn solid(color: impl Into<UiEffectColor>, blur: f32, spread: f32) -> Self {
        Self::gradient([color], blur, spread)
    }

    pub fn gradient(
        colors: impl IntoIterator<Item = impl Into<UiEffectColor>>,
        blur: f32,
        spread: f32,
    ) -> Self {
        Self {
            colors: colors.into_iter().map(Into::into).collect(),
            blur,
            spread,
            strength: 1.0,
            phase: 0.0,
        }
    }

    pub fn with_strength(mut self, strength: f32) -> Self {
        self.strength = strength;
        self
    }

    pub fn with_phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }
}

/// Decoration drawn behind and around a BBUI node without changing its layout or hit box.
#[derive(Component, Debug, Clone, Default, PartialEq)]
#[component(summary_tick)]
pub struct UiVisualEffects {
    pub border: Option<UiBorderEffect>,
    pub edge_glow: Option<UiEdgeGlow>,
}

impl UiVisualEffects {
    pub fn with_border(mut self, border: UiBorderEffect) -> Self {
        self.border = Some(border);
        self
    }

    pub fn with_edge_glow(mut self, edge_glow: UiEdgeGlow) -> Self {
        self.edge_glow = Some(edge_glow);
        self
    }
}

/// Bind a `MotionClip` to a multicolor border's rotation.
pub fn border_phase_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0001,
        |world, entity| {
            world
                .get::<UiVisualEffects>(entity)?
                .border
                .as_ref()
                .map(|border| border.phase)
        },
        |world: &mut World, entity, phase| {
            if let Some(mut effects) = world.get_mut::<UiVisualEffects>(entity)
                && let Some(border) = &mut effects.border
            {
                border.phase = phase;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a `MotionClip` to a border's alpha multiplier.
pub fn border_strength_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0003,
        |world, entity| {
            world
                .get::<UiVisualEffects>(entity)?
                .border
                .as_ref()
                .map(|border| border.strength)
        },
        |world: &mut World, entity, strength| {
            if let Some(mut effects) = world.get_mut::<UiVisualEffects>(entity)
                && let Some(border) = &mut effects.border
            {
                border.strength = strength;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a `MotionClip` to the edge glow intensity of an existing node.
pub fn edge_glow_strength_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0002,
        |world, entity| {
            world
                .get::<UiVisualEffects>(entity)?
                .edge_glow
                .as_ref()
                .map(|glow| glow.strength)
        },
        |world: &mut World, entity, strength| {
            if let Some(mut effects) = world.get_mut::<UiVisualEffects>(entity)
                && let Some(glow) = &mut effects.edge_glow
            {
                glow.strength = strength;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

/// Bind a `MotionClip` to a multicolor glow's rotation.
pub fn edge_glow_phase_binding() -> MotionBinding<f32> {
    MotionBinding::with_id(
        0x4242_5549_0004,
        |world, entity| {
            world
                .get::<UiVisualEffects>(entity)?
                .edge_glow
                .as_ref()
                .map(|glow| glow.phase)
        },
        |world: &mut World, entity, phase| {
            if let Some(mut effects) = world.get_mut::<UiVisualEffects>(entity)
                && let Some(glow) = &mut effects.edge_glow
            {
                glow.phase = phase;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

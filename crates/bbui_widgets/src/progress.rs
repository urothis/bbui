//! Progress values and appearance, shared by declarative and ECS-owned controls.

use crate::{UiBorderEffect, UiEdgeGlow, UiEffectColor};
use bevy_ecs::prelude::Component;

/// Colors across the filled part of a progress bar.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ProgressFill {
    #[default]
    ThemeAccent,
    Solid(UiEffectColor),
    Gradient(Vec<UiEffectColor>),
}

/// Corner shape shared by the track, fill, fill border, and fill glow.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ProgressCorners {
    /// Follow the active theme's corner radius.
    #[default]
    Theme,
    Square,
    /// A radius in logical pixels, capped to fit the painted rectangle.
    Radius(f32),
    /// Round each end as far as its rectangle allows.
    Pill,
}

impl ProgressCorners {
    pub(crate) fn radius(self, width: f32, height: f32, theme_radius: f32) -> f32 {
        if !width.is_finite() || !height.is_finite() {
            return 0.0;
        }
        let limit = width.max(0.0).min(height.max(0.0)) * 0.5;
        let requested = match self {
            Self::Theme => theme_radius,
            Self::Square => 0.0,
            Self::Radius(radius) if radius.is_finite() => radius,
            Self::Radius(_) => theme_radius,
            Self::Pill => limit,
        };
        if requested.is_finite() {
            requested.clamp(0.0, limit)
        } else {
            0.0
        }
    }
}

/// Visual options for a progress bar. All options are independent.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProgressStyle {
    pub fill: ProgressFill,
    pub track: Option<UiEffectColor>,
    pub corners: ProgressCorners,
    /// Number of equal segments. Reduced to fit narrow bars; `None` paints a continuous fill.
    pub segments: Option<usize>,
    /// The border radius follows `corners`; the border's own radius is ignored here.
    pub fill_border: Option<UiBorderEffect>,
    pub fill_glow: Option<UiEdgeGlow>,
}

impl ProgressStyle {
    pub fn solid(color: impl Into<UiEffectColor>) -> Self {
        Self {
            fill: ProgressFill::Solid(color.into()),
            ..Self::default()
        }
    }

    pub fn gradient(colors: impl IntoIterator<Item = impl Into<UiEffectColor>>) -> Self {
        Self {
            fill: ProgressFill::Gradient(colors.into_iter().map(Into::into).collect()),
            ..Self::default()
        }
    }

    pub fn with_track(mut self, color: impl Into<UiEffectColor>) -> Self {
        self.track = Some(color.into());
        self
    }

    pub fn with_corners(mut self, corners: ProgressCorners) -> Self {
        self.corners = corners;
        self
    }

    pub fn with_segments(mut self, count: usize) -> Self {
        self.segments = Some(count);
        self
    }

    pub fn with_fill_border(mut self, border: UiBorderEffect) -> Self {
        self.fill_border = Some(border);
        self
    }

    pub fn with_fill_glow(mut self, glow: UiEdgeGlow) -> Self {
        self.fill_glow = Some(glow);
        self
    }
}

/// A value from zero to one, or an indeterminate moving indicator.
#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
pub struct Progress {
    pub value: f32,
    pub indeterminate: bool,
    /// The indeterminate sweep position, measured in turns.
    pub phase: f32,
    /// Rotation of a gradient fill palette, measured in turns.
    pub palette_phase: f32,
    pub style: ProgressStyle,
}

impl Progress {
    pub fn new(value: f32) -> Self {
        Self {
            value: if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            },
            indeterminate: false,
            phase: 0.0,
            palette_phase: 0.0,
            style: ProgressStyle::default(),
        }
    }

    pub fn indeterminate() -> Self {
        Self {
            indeterminate: true,
            ..Self::new(0.0)
        }
    }

    pub fn with_style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    pub fn set(&mut self, value: f32) {
        if value.is_finite() {
            self.value = value.clamp(0.0, 1.0);
        }
    }
}

mod motion;
pub(crate) use motion::advance_indeterminate_progress;
pub use motion::{
    progress_border_phase_binding, progress_border_strength_binding, progress_glow_phase_binding,
    progress_glow_strength_binding, progress_palette_phase_binding, progress_value_binding,
};

#[cfg(test)]
mod tests {
    use super::ProgressCorners;

    #[test]
    fn corner_radius_fits_a_partial_fill_or_narrow_segment() {
        assert_eq!(ProgressCorners::Pill.radius(8.0, 24.0, 3.0), 4.0);
        assert_eq!(ProgressCorners::Radius(20.0).radius(8.0, 24.0, 3.0), 4.0);
        assert_eq!(ProgressCorners::Square.radius(8.0, 24.0, 3.0), 0.0);
        assert_eq!(ProgressCorners::Theme.radius(8.0, 24.0, 3.0), 3.0);
    }
}

//! Bounded numeric sliders built on the shared numeric-field value semantics.

use crate::{NumericField, NumericFieldKind};
use bevy_ecs::prelude::Component;
use std::ops::{Deref, DerefMut};

/// A range control backed by the same normalization, defaults, units, and
/// stepping rules as [`NumericField`]. Sliders require finite bounds.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Slider {
    /// Shared numeric state used by text fields, scrub gestures, and history.
    pub field: NumericField,
}

impl Slider {
    /// Creates a floating point slider with a finite inclusive range.
    pub fn new(value: f64, min: f64, max: f64, step: f64) -> Result<Self, &'static str> {
        Self::with_kind(value, min, max, step, NumericFieldKind::Float)
    }

    /// Creates an integer or floating point slider with finite bounds.
    pub fn with_kind(
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        kind: NumericFieldKind,
    ) -> Result<Self, &'static str> {
        if !min.is_finite() || !max.is_finite() {
            return Err("slider bounds must be finite");
        }
        if !(max - min).is_finite() {
            return Err("slider range span must be finite");
        }
        let field = NumericField::new(value, Some(min), Some(max), step, kind)?;
        Ok(Self { field })
    }

    /// Converts an existing bounded numeric field into a slider without
    /// changing its value, default, unit, precision, or edit settings.
    pub fn from_field(field: NumericField) -> Result<Self, &'static str> {
        match (field.min, field.max) {
            (Some(min), Some(max)) if min.is_finite() && max.is_finite() => {
                if !(max - min).is_finite() {
                    return Err("slider range span must be finite");
                }
                let mut normalized =
                    NumericField::new(field.value, Some(min), Some(max), field.step, field.kind)?;
                normalized.set_default(field.default)?;
                let mut field = field;
                field.value = normalized.value;
                field.default = normalized.default;
                Ok(Self { field })
            }
            _ => Err("slider requires finite minimum and maximum bounds"),
        }
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.field.unit = Some(unit.into());
        self
    }

    pub fn with_default(mut self, value: f64) -> Result<Self, &'static str> {
        self.field.set_default(value)?;
        Ok(self)
    }

    pub fn range(&self) -> [f64; 2] {
        let min = self.field.min.unwrap_or(self.field.value);
        let max = self.field.max.unwrap_or(min);
        [min, max]
    }

    /// Returns the current position in the closed interval `[0, 1]`.
    /// A fixed-range slider is represented at its midpoint.
    pub fn normalized_value(&self) -> f64 {
        let [min, max] = self.range();
        if min == max {
            0.5
        } else {
            ((self.field.value - min) / (max - min)).clamp(0.0, 1.0)
        }
    }

    /// Maps a normalized track position to the numeric range and applies the
    /// shared field rules (clamping, integer rounding, enabled/read-only).
    pub fn set_normalized_value(&mut self, position: f64) -> Result<bool, &'static str> {
        if !position.is_finite() {
            return Err("slider position must be finite");
        }
        let [min, max] = self.range();
        self.field
            .set_value(min + (max - min) * position.clamp(0.0, 1.0))
    }

    pub fn set_value(&mut self, value: f64) -> Result<bool, &'static str> {
        self.field.set_value(value)
    }

    pub fn set_range(&mut self, min: f64, max: f64) -> Result<bool, &'static str> {
        if !min.is_finite() || !max.is_finite() {
            return Err("slider bounds must be finite");
        }
        if !(max - min).is_finite() {
            return Err("slider range span must be finite");
        }
        self.field.set_bounds(Some(min), Some(max))
    }
}

impl Deref for Slider {
    type Target = NumericField;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl DerefMut for Slider {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_maps_track_positions_and_reuses_numeric_field_normalization() {
        let mut slider = Slider::new(0.25, 0.0, 1.0, 0.1).unwrap().with_unit("m");
        assert_eq!(slider.normalized_value(), 0.25);
        assert!(slider.set_normalized_value(2.0).unwrap());
        assert_eq!(slider.value, 1.0);
        assert_eq!(slider.display(), "1 m");
        assert!(!slider.set_normalized_value(1.0).unwrap());
    }

    #[test]
    fn integer_slider_rounds_track_values_and_keyboard_steps_by_configured_unit() {
        let mut slider = Slider::with_kind(3.0, 0.0, 10.0, 2.0, NumericFieldKind::Integer)
            .unwrap()
            .with_unit("px");
        assert!(slider.set_normalized_value(0.51).unwrap());
        assert_eq!(slider.value, 5.0);
        assert!(slider.step_by(1));
        assert_eq!(slider.value, 7.0);
        assert_eq!(slider.display(), "7 px");
    }

    #[test]
    fn slider_default_range_and_read_only_validation_share_field_behavior() {
        let slider = Slider::new(4.0, 0.0, 10.0, 0.5)
            .unwrap()
            .with_default(2.0)
            .unwrap();
        assert_eq!(slider.default, 2.0);
        assert_eq!(slider.range(), [0.0, 10.0]);
        assert!(Slider::new(0.0, f64::NEG_INFINITY, 1.0, 0.1).is_err());
        assert!(Slider::new(0.0, 1.0, 0.0, 0.1).is_err());
        assert!(Slider::new(0.0, 0.0, 1.0, 0.0).is_err());
        assert!(Slider::new(f64::NAN, 0.0, 1.0, 0.1).is_err());

        let mut readonly = slider;
        readonly.read_only = true;
        assert!(!readonly.set_normalized_value(0.9).unwrap());
        assert_eq!(readonly.value, 4.0);
    }

    #[test]
    fn slider_can_be_built_from_a_bounded_field_and_fixed_ranges_have_midpoint_handle() {
        let field =
            NumericField::new(2.0, Some(2.0), Some(2.0), 0.5, NumericFieldKind::Float).unwrap();
        let mut slider = Slider::from_field(field).unwrap();
        assert_eq!(slider.normalized_value(), 0.5);
        assert!(!slider.set_normalized_value(0.0).unwrap());
        assert!(Slider::from_field(NumericField::float(2.0).unwrap()).is_err());
    }

    #[test]
    fn slider_revalidates_public_field_state_and_rejects_overflowing_ranges() {
        let mut field =
            NumericField::new(2.0, Some(0.0), Some(10.0), 1.0, NumericFieldKind::Float).unwrap();
        field.value = 20.0;
        field.default = -2.0;
        let slider = Slider::from_field(field).unwrap();
        assert_eq!(slider.value, 10.0);
        assert_eq!(slider.default, 0.0);

        assert!(Slider::new(0.0, -1.0e308, 1.0e308, 1.0).is_err());
        assert!(Slider::new(0.0, -8.0e307, 8.0e307, 1.0).is_ok());
        let mut slider = Slider::new(0.0, -8.0e307, 8.0e307, 1.0).unwrap();
        assert!(slider.set_normalized_value(1.0).unwrap());
        assert_eq!(slider.value, 8.0e307);
        assert!(slider.set_range(-1.0e308, 1.0e308).is_err());
    }
}

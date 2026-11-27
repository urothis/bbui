//! Bounded numeric sliders built on the shared numeric-field value semantics.

use crate::{NumericField, NumericFieldKind, NumericValue};
use bevy_ecs::prelude::Component;
use std::ops::{Deref, DerefMut};

/// A range control backed by the same normalization, defaults, units, and
/// stepping rules as [`NumericField`]. Sliders require finite bounds.
#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
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
        if field.kind() != NumericFieldKind::Float {
            return match (field.typed_min(), field.typed_max()) {
                (Some(NumericValue::Integer(min)), Some(NumericValue::Integer(max)))
                    if min <= max =>
                {
                    Ok(Self { field })
                }
                (Some(NumericValue::Unsigned(min)), Some(NumericValue::Unsigned(max)))
                    if min <= max =>
                {
                    Ok(Self { field })
                }
                _ => Err("exact slider requires typed minimum and maximum bounds"),
            };
        }
        match (field.min_projection(), field.max_projection()) {
            (Some(min), Some(max)) if min.is_finite() && max.is_finite() => {
                if !(max - min).is_finite() {
                    return Err("slider range span must be finite");
                }
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
        let min = self
            .field
            .min_projection()
            .unwrap_or(self.field.value_projection());
        let max = self.field.max_projection().unwrap_or(min);
        [min, max]
    }

    /// Returns the current position in the closed interval `[0, 1]`.
    /// A fixed-range slider is represented at its midpoint.
    pub fn normalized_value(&self) -> f64 {
        match (
            self.field.typed_value(),
            self.field.typed_min(),
            self.field.typed_max(),
        ) {
            (
                NumericValue::Integer(value),
                Some(NumericValue::Integer(min)),
                Some(NumericValue::Integer(max)),
            ) => {
                if min == max {
                    return 0.5;
                }
                return ((i128::from(value) - i128::from(min)) as f64
                    / (i128::from(max) - i128::from(min)) as f64)
                    .clamp(0.0, 1.0);
            }
            (
                NumericValue::Unsigned(value),
                Some(NumericValue::Unsigned(min)),
                Some(NumericValue::Unsigned(max)),
            ) => {
                if min == max {
                    return 0.5;
                }
                return ((u128::from(value) - u128::from(min)) as f64
                    / (u128::from(max) - u128::from(min)) as f64)
                    .clamp(0.0, 1.0);
            }
            _ => {}
        }
        let [min, max] = self.range();
        if min == max {
            0.5
        } else {
            ((self.field.value_projection() - min) / (max - min)).clamp(0.0, 1.0)
        }
    }

    /// Maps a normalized track position to the numeric range and applies the
    /// shared field rules (clamping, integer rounding, enabled/read-only).
    pub fn set_normalized_value(&mut self, position: f64) -> Result<bool, &'static str> {
        if !position.is_finite() {
            return Err("slider position must be finite");
        }
        let position = position.clamp(0.0, 1.0);
        match (self.field.typed_min(), self.field.typed_max()) {
            (Some(NumericValue::Integer(min)), Some(NumericValue::Integer(max))) => {
                let span = i128::from(max) - i128::from(min);
                let offset = if position == 1.0 {
                    span
                } else {
                    (span as f64 * position).round() as i128
                };
                let next =
                    (i128::from(min) + offset).clamp(i128::from(min), i128::from(max)) as i64;
                return self.field.set_typed_value(NumericValue::Integer(next));
            }
            (Some(NumericValue::Unsigned(min)), Some(NumericValue::Unsigned(max))) => {
                let span = u128::from(max) - u128::from(min);
                let offset = if position == 1.0 {
                    span
                } else {
                    (span as f64 * position).round() as u128
                };
                let next =
                    (u128::from(min) + offset).clamp(u128::from(min), u128::from(max)) as u64;
                return self.field.set_typed_value(NumericValue::Unsigned(next));
            }
            _ => {}
        }
        let [min, max] = self.range();
        self.field
            .set_value(min + (max - min) * position.clamp(0.0, 1.0))
    }

    pub fn set_value(&mut self, value: f64) -> Result<bool, &'static str> {
        self.field.set_value(value)
    }

    pub fn set_typed_value(&mut self, value: NumericValue) -> Result<bool, &'static str> {
        self.field.set_typed_value(value)
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
    fn full_width_unsigned_slider_keeps_exact_endpoints() {
        let mut field = NumericField::unsigned(u64::MAX - 5);
        field
            .set_typed_bounds(
                Some(NumericValue::Unsigned(u64::MAX - 10)),
                Some(NumericValue::Unsigned(u64::MAX)),
            )
            .unwrap();
        let mut slider = Slider::from_field(field).unwrap();
        assert_eq!(slider.normalized_value(), 0.5);
        assert!(slider.set_normalized_value(1.0).unwrap());
        assert_eq!(slider.typed_value(), NumericValue::Unsigned(u64::MAX));
        assert!(slider.set_normalized_value(0.0).unwrap());
        assert_eq!(slider.typed_value(), NumericValue::Unsigned(u64::MAX - 10));
    }

    #[test]
    fn slider_maps_track_positions_and_reuses_numeric_field_normalization() {
        let mut slider = Slider::new(0.25, 0.0, 1.0, 0.1).unwrap().with_unit("m");
        assert_eq!(slider.normalized_value(), 0.25);
        assert!(slider.set_normalized_value(2.0).unwrap());
        assert_eq!(slider.value_projection(), 1.0);
        assert_eq!(slider.display(), "1 m");
        assert!(!slider.set_normalized_value(1.0).unwrap());
    }

    #[test]
    fn integer_slider_rounds_track_values_and_keyboard_steps_by_configured_unit() {
        let mut slider = Slider::with_kind(3.0, 0.0, 10.0, 2.0, NumericFieldKind::Integer)
            .unwrap()
            .with_unit("px");
        assert!(slider.set_normalized_value(0.51).unwrap());
        assert_eq!(slider.value_projection(), 5.0);
        assert!(slider.step_by(1));
        assert_eq!(slider.value_projection(), 7.0);
        assert_eq!(slider.display(), "7 px");
    }

    #[test]
    fn slider_default_range_and_read_only_validation_share_field_behavior() {
        let slider = Slider::new(4.0, 0.0, 10.0, 0.5)
            .unwrap()
            .with_default(2.0)
            .unwrap();
        assert_eq!(slider.default_projection(), 2.0);
        assert_eq!(slider.range(), [0.0, 10.0]);
        assert!(Slider::new(0.0, f64::NEG_INFINITY, 1.0, 0.1).is_err());
        assert!(Slider::new(0.0, 1.0, 0.0, 0.1).is_err());
        assert!(Slider::new(0.0, 0.0, 1.0, 0.0).is_err());
        assert!(Slider::new(f64::NAN, 0.0, 1.0, 0.1).is_err());

        let mut readonly = slider;
        readonly.read_only = true;
        assert!(!readonly.set_normalized_value(0.9).unwrap());
        assert_eq!(readonly.value_projection(), 4.0);
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
    fn slider_setters_keep_field_state_valid_and_reject_overflowing_ranges() {
        let mut field =
            NumericField::new(2.0, Some(0.0), Some(10.0), 1.0, NumericFieldKind::Float).unwrap();
        assert!(field.set_value(20.0).unwrap());
        field.set_default(-2.0).unwrap();
        let slider = Slider::from_field(field).unwrap();
        assert_eq!(slider.value_projection(), 10.0);
        assert_eq!(slider.default_projection(), 0.0);

        assert!(Slider::new(0.0, -1.0e308, 1.0e308, 1.0).is_err());
        assert!(Slider::new(0.0, -8.0e307, 8.0e307, 1.0).is_ok());
        let mut slider = Slider::new(0.0, -8.0e307, 8.0e307, 1.0).unwrap();
        assert!(slider.set_normalized_value(1.0).unwrap());
        assert_eq!(slider.value_projection(), 8.0e307);
        assert!(slider.set_range(-1.0e308, 1.0e308).is_err());
    }
}

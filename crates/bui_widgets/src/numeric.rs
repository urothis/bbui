//! Bounded and unbounded numeric controls, parsing, stepping, and formatting.

use bevy_ecs::prelude::Component;
use bui_text::expression::{self, Error};
pub use bui_text::format_float;
use std::collections::HashMap;

/// Values available to a numeric field's typed expression. Attach this
/// component to the field entity and update it when application data changes.
/// The field's current value is always exposed as `x` and takes precedence.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct NumericExpressionVariables(pub HashMap<String, f64>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumericFieldKind {
    Integer,
    #[default]
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumericValue {
    Integer(i64),
    Float(f64),
}

impl NumericValue {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
pub struct NumericField {
    pub value: f64,
    pub default: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: f64,
    pub kind: NumericFieldKind,
    pub unit: Option<String>,
    pub enabled: bool,
    pub read_only: bool,
    pub editing: Option<String>,
    /// The displayed value is selected when typing begins.
    pub replace_on_input: bool,
    /// Last failed expression, retained until corrected or cancelled.
    pub expression_error: Option<Error>,
    pub precision: usize,
}

impl NumericField {
    pub fn new(
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        step: f64,
        kind: NumericFieldKind,
    ) -> Result<Self, &'static str> {
        if !value.is_finite()
            || !step.is_finite()
            || step <= 0.0
            || min.is_some_and(|bound| !bound.is_finite())
            || max.is_some_and(|bound| !bound.is_finite())
            || min.zip(max).is_some_and(|(min, max)| min > max)
        {
            return Err("invalid numeric field value, bounds, or step");
        }
        if kind == NumericFieldKind::Integer && integer_bounds(min, max).is_none() {
            return Err("integer numeric bounds contain no representable value");
        }
        let mut field = Self {
            value,
            default: value,
            min,
            max,
            step,
            kind,
            unit: None,
            enabled: true,
            read_only: false,
            editing: None,
            replace_on_input: false,
            expression_error: None,
            precision: 6,
        };
        field.value = field.normalize(value);
        field.default = field.value;
        Ok(field)
    }

    pub fn float(value: f64) -> Result<Self, &'static str> {
        Self::new(value, None, None, 1.0, NumericFieldKind::Float)
    }

    pub fn integer(value: i64) -> Self {
        let value = value as f64;
        Self {
            value,
            default: value,
            min: None,
            max: None,
            step: 1.0,
            kind: NumericFieldKind::Integer,
            unit: None,
            enabled: true,
            read_only: false,
            editing: None,
            replace_on_input: false,
            expression_error: None,
            precision: 6,
        }
    }

    pub fn typed_value(&self) -> NumericValue {
        match self.kind {
            NumericFieldKind::Integer => NumericValue::Integer(self.value.round() as i64),
            NumericFieldKind::Float => NumericValue::Float(self.value),
        }
    }

    pub fn set_default(&mut self, value: f64) -> Result<(), &'static str> {
        if !value.is_finite() {
            return Err("default must be finite");
        }
        self.default = self.normalize(value);
        Ok(())
    }

    pub fn set_bounds(&mut self, min: Option<f64>, max: Option<f64>) -> Result<bool, &'static str> {
        if min.is_some_and(|bound| !bound.is_finite())
            || max.is_some_and(|bound| !bound.is_finite())
            || min.zip(max).is_some_and(|(min, max)| min > max)
        {
            return Err("invalid numeric bounds");
        }
        if self.kind == NumericFieldKind::Integer && integer_bounds(min, max).is_none() {
            return Err("integer numeric bounds contain no representable value");
        }
        self.min = min;
        self.max = max;
        let old = self.value;
        self.value = self.normalize(self.value);
        self.default = self.normalize(self.default);
        Ok(old != self.value)
    }

    pub fn set_value(&mut self, value: f64) -> Result<bool, &'static str> {
        if !self.enabled || self.read_only {
            return Ok(false);
        }
        self.restore_value(value)
    }

    /// Restore a value from history or an external data source, bypassing the
    /// user-facing enabled/read-only guards while retaining numeric validation.
    pub fn restore_value(&mut self, value: f64) -> Result<bool, &'static str> {
        if !value.is_finite() {
            return Err("value must be finite");
        }
        let value = self.normalize(value);
        let changed = value != self.value;
        self.value = value;
        self.expression_error = None;
        Ok(changed)
    }

    pub fn reset(&mut self) -> bool {
        if !self.enabled || self.read_only || self.value == self.default {
            return false;
        }
        self.value = self.default;
        self.expression_error = None;
        true
    }

    pub fn evaluate_input(
        &mut self,
        input: &str,
        vars: &HashMap<String, f64>,
    ) -> Result<bool, Error> {
        if !self.enabled || self.read_only {
            return Ok(false);
        }
        let mut context = vars.clone();
        context.insert("x".into(), self.value);
        let result = match expression::evaluate(input, &context) {
            Ok(value) => value,
            Err(error) => {
                self.expression_error = Some(error.clone());
                return Err(error);
            }
        };
        let value = self.normalize(result);
        let changed = value != self.value;
        self.value = value;
        self.editing = None;
        self.replace_on_input = false;
        self.expression_error = None;
        Ok(changed)
    }

    /// Start or replace the uncommitted edit buffer.
    pub fn begin_edit(&mut self) {
        if !self.enabled || self.read_only {
            return;
        }
        if self.editing.is_none() {
            self.editing = Some(format_float(self.value, self.precision));
            self.replace_on_input = true;
            self.expression_error = None;
        }
    }

    pub fn edit_insert(&mut self, text: &str) {
        if !self.enabled || self.read_only {
            return;
        }
        self.begin_edit();
        if let Some(editing) = &mut self.editing {
            if self.replace_on_input {
                editing.clear();
                self.replace_on_input = false;
            }
            editing.push_str(text);
            self.expression_error = None;
        }
    }

    /// Delete one input character, or clear the selected displayed value.
    pub fn edit_backspace(&mut self) {
        if !self.enabled || self.read_only {
            return;
        }
        if let Some(editing) = &mut self.editing {
            if self.replace_on_input {
                editing.clear();
                self.replace_on_input = false;
            } else {
                editing.pop();
            }
            self.expression_error = None;
        }
    }

    /// Select the current edit buffer so the next typed text replaces it.
    pub fn edit_select_all(&mut self) {
        if self.enabled && !self.read_only && self.editing.is_some() {
            self.replace_on_input = true;
        }
    }

    /// Commit the edit buffer. Parse failures leave both value and buffer intact.
    pub fn commit_edit(&mut self, vars: &HashMap<String, f64>) -> Result<bool, Error> {
        let Some(text) = self.editing.clone() else {
            return Ok(false);
        };
        self.evaluate_input(&text, vars)
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.replace_on_input = false;
        self.expression_error = None;
    }

    pub fn step_by(&mut self, steps: i32) -> bool {
        if !self.enabled || self.read_only || steps == 0 {
            return false;
        }
        let value = self.value + self.step * f64::from(steps);
        if !value.is_finite() {
            return false;
        }
        let value = self.normalize(value);
        let changed = value != self.value;
        self.value = value;
        self.expression_error = None;
        changed
    }

    /// Numeric text shown in the control before appending a configured unit.
    pub fn value_text(&self) -> String {
        match self.kind {
            NumericFieldKind::Integer => format!("{}", self.value.round() as i64),
            NumericFieldKind::Float => format_float(self.value, self.precision),
        }
    }

    /// Full-precision numeric text for lossless copy and paste.
    pub fn clipboard_value(&self) -> String {
        match self.kind {
            NumericFieldKind::Integer => format!("{}", self.value.round() as i64),
            NumericFieldKind::Float => self.value.to_string(),
        }
    }

    pub fn display(&self) -> String {
        let mut value = self.editing.clone().unwrap_or_else(|| self.value_text());
        if let Some(unit) = &self.unit
            && !unit.is_empty()
        {
            value.push(' ');
            value.push_str(unit);
        }
        value
    }

    fn normalize(&self, value: f64) -> f64 {
        match self.kind {
            NumericFieldKind::Integer => {
                let (min, max) = integer_bounds(self.min, self.max).unwrap_or((0.0, 0.0));
                value.round().clamp(min, max)
            }
            NumericFieldKind::Float => value.clamp(
                self.min.unwrap_or(f64::NEG_INFINITY),
                self.max.unwrap_or(f64::INFINITY),
            ),
        }
    }
}

fn integer_bounds(min: Option<f64>, max: Option<f64>) -> Option<(f64, f64)> {
    let min = min.map(f64::ceil).unwrap_or(f64::NEG_INFINITY);
    let max = max.map(f64::floor).unwrap_or(f64::INFINITY);
    (min <= max).then_some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbounded_integer_and_float_fields_step_and_format() {
        let mut integer = NumericField::integer(5);
        assert_eq!(integer.typed_value(), NumericValue::Integer(5));
        integer.step_by(-2);
        assert_eq!(integer.value, 3.0);
        let float = NumericField::float(0.00000042).unwrap();
        assert_eq!(float.display(), "4.2e-7");
    }

    #[test]
    fn expression_commit_is_atomic_and_uses_the_current_x_value() {
        let mut field =
            NumericField::new(2.0, Some(0.0), Some(10.0), 1.0, NumericFieldKind::Float).unwrap();
        field.editing = Some("x * 2".into());
        assert!(field.commit_edit(&HashMap::new()).unwrap());
        assert_eq!(field.value, 4.0);
        field.editing = Some("1 / 0".into());
        assert!(field.commit_edit(&HashMap::new()).is_err());
        assert_eq!(field.value, 4.0);
        assert_eq!(field.editing.as_deref(), Some("1 / 0"));
    }

    #[test]
    fn expression_variables_keep_current_x_authoritative() {
        let mut field = NumericField::float(3.0).unwrap();
        let mut variables = HashMap::from([
            ("x".to_owned(), 999.0),
            ("c".to_owned(), 1.0),
            ("y".to_owned(), 7.0),
        ]);
        assert!(!field.evaluate_input("x if c else y", &variables).unwrap());
        assert_eq!(field.value, 3.0);
        variables.insert("c".to_owned(), 0.0);
        assert!(field.evaluate_input("x if c else y", &variables).unwrap());
        assert_eq!(field.value, 7.0);
    }

    #[test]
    fn editing_replaces_displayed_value_and_keeps_invalid_input_for_correction() {
        let mut field = NumericField::float(3.0).unwrap();
        field.begin_edit();
        assert_eq!(field.editing.as_deref(), Some("3"));
        field.edit_insert("2*pi");
        assert_eq!(field.editing.as_deref(), Some("2*pi"));
        field.edit_backspace();
        assert_eq!(field.editing.as_deref(), Some("2*p"));
        field.edit_select_all();
        field.edit_insert("1/0");
        assert!(field.commit_edit(&HashMap::new()).is_err());
        assert_eq!(field.value, 3.0);
        assert_eq!(field.editing.as_deref(), Some("1/0"));
        field.cancel_edit();
        assert_eq!(field.editing, None);
        assert!(!field.replace_on_input);
    }

    #[test]
    fn integer_normalization_and_displayed_units_are_consistent() {
        let mut field =
            NumericField::new(2.4, Some(0.0), Some(5.0), 1.0, NumericFieldKind::Integer).unwrap();
        field.unit = Some("px".into());
        assert_eq!(field.value, 2.0);
        assert_eq!(field.display(), "2 px");
        field.set_value(7.0).unwrap();
        assert_eq!(field.value, 5.0);
    }

    #[test]
    fn integer_bounds_clamp_to_representable_values_and_reject_empty_ranges() {
        let mut field =
            NumericField::new(4.0, Some(0.2), Some(3.7), 1.0, NumericFieldKind::Integer).unwrap();
        assert_eq!(field.value, 3.0);
        assert!(field.set_value(0.0).unwrap());
        assert_eq!(field.value, 1.0);
        assert!(field.set_value(4.0).unwrap());
        assert_eq!(field.value, 3.0);

        assert!(
            NumericField::new(1.5, Some(1.2), Some(1.8), 1.0, NumericFieldKind::Integer,).is_err()
        );
        assert!(field.set_bounds(Some(1.2), Some(1.8)).is_err());
        assert_eq!(field.min, Some(0.2));
        assert_eq!(field.max, Some(3.7));
        assert_eq!(field.value, 3.0);
    }

    #[test]
    fn historical_restore_bypasses_interaction_guards_but_normalizes_values() {
        let mut field =
            NumericField::new(2.0, Some(0.2), Some(3.7), 1.0, NumericFieldKind::Integer).unwrap();
        field.enabled = false;
        field.read_only = true;
        assert!(!field.set_value(1.0).unwrap());
        assert_eq!(field.value, 2.0);
        assert!(field.restore_value(0.0).unwrap());
        assert_eq!(field.value, 1.0);
        assert!(field.restore_value(4.0).unwrap());
        assert_eq!(field.value, 3.0);
        assert!(field.restore_value(f64::NAN).is_err());
        assert_eq!(field.value, 3.0);
    }

    #[test]
    fn significant_float_format_handles_round_numbers_and_small_values() {
        assert_eq!(format_float(42.0, 6), "42");
        assert_eq!(format_float(0.00042, 6), "0.00042");
        assert_eq!(format_float(-0.00000042, 6), "-4.2e-7");
        assert_eq!(format_float(1_500_000_000.0, 6), "1.5e9");
    }

    #[test]
    fn clipboard_preserves_full_float_precision_without_units() {
        let mut field = NumericField::float(1.234_567_890_123_456_7).unwrap();
        field.unit = Some("m".to_owned());
        assert_eq!(field.display(), "1.23457 m");
        let copied = field.clipboard_value();
        assert_eq!(copied.parse::<f64>().unwrap(), field.value);
        assert!(!copied.contains('m'));
    }

    #[test]
    fn disabled_and_read_only_fields_never_start_a_text_edit() {
        let mut field = NumericField::float(3.0).unwrap();
        field.read_only = true;
        field.begin_edit();
        field.edit_insert("4");
        assert_eq!(field.editing, None);
        assert_eq!(field.value, 3.0);
        field.read_only = false;
        field.enabled = false;
        field.begin_edit();
        field.edit_insert("4");
        assert_eq!(field.editing, None);
        assert_eq!(field.value, 3.0);
    }

    #[test]
    fn unbounded_stepping_rejects_overflow_without_corrupting_the_value() {
        let mut field =
            NumericField::new(f64::MAX, None, None, f64::MAX, NumericFieldKind::Float).unwrap();
        assert!(!field.step_by(1));
        assert_eq!(field.value, f64::MAX);
    }

    #[test]
    fn displayed_float_parses_back_within_configured_significant_precision() {
        for original in [
            0.000_000_42,
            -0.003_141_592_65,
            12.345_678_9,
            1_500_000_000.0,
        ] {
            let mut field = NumericField::float(original).unwrap();
            field.precision = 6;
            let displayed = field.display();
            field.set_value(0.0).unwrap();
            field.evaluate_input(&displayed, &HashMap::new()).unwrap();
            let relative_error = (field.value - original).abs() / original.abs();
            assert!(
                relative_error < 0.000_01,
                "{original} -> {displayed} -> {}",
                field.value
            );
        }
    }

    #[test]
    fn generated_float_displays_round_trip_through_numeric_expression_input() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut checked = 0;
        for _ in 0..1024 {
            // Deterministic xorshift values exercise normal, subnormal, and
            // widely separated magnitudes without adding a property-test dep.
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let original = f64::from_bits(state);
            if !original.is_finite() || original == 0.0 {
                continue;
            }
            for precision in 1..=17 {
                let mut field = NumericField::float(original).unwrap();
                field.precision = precision;
                let displayed = field.value_text();
                let mut edited = NumericField::float(0.0).unwrap();
                edited
                    .evaluate_input(&displayed, &HashMap::new())
                    .unwrap_or_else(|error| {
                        panic!(
                            "{original} at precision {precision} displayed as {displayed}: {error}"
                        )
                    });
                let relative_error = (edited.value - original).abs() / original.abs();
                let allowed_error = 0.5 * 10_f64.powi(1 - precision as i32) + 2.0 * f64::EPSILON;
                assert!(
                    relative_error <= allowed_error,
                    "{original} at precision {precision} displayed as {displayed}, parsed as {} (relative error {relative_error}, allowed {allowed_error})",
                    edited.value
                );
                checked += 1;
            }
        }
        assert!(
            checked > 15_000,
            "generated too few finite values: {checked}"
        );
    }

    #[test]
    fn display_keeps_the_edit_buffer_stable_until_commit_or_cancel() {
        let mut field = NumericField::float(0.000_000_42).unwrap();
        assert_eq!(field.display(), "4.2e-7");
        field.begin_edit();
        field.edit_insert("2*pi");
        assert_eq!(field.display(), "2*pi");
        field.set_value(1.0).unwrap();
        assert_eq!(field.display(), "2*pi");
        field.cancel_edit();
        assert_eq!(field.display(), "1");
    }
}

//! Bounded and unbounded numeric controls, parsing, stepping, and formatting.

pub use bbui_core::NumericValue;
use bbui_text::expression::{self, Error};
pub use bbui_text::format_float;
use bevy_ecs::prelude::Component;
use std::collections::HashMap;

/// Values available to a numeric field's typed expression. Attach this
/// component to the field entity and update it when application data changes.
/// The field's current value is always exposed as `x` and takes precedence.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct NumericExpressionVariables(pub HashMap<String, NumericValue>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumericFieldKind {
    Integer,
    Unsigned,
    #[default]
    Float,
}

#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
pub struct NumericField {
    value: f64,
    /// Authoritative value for full-width integer fields. `value` is a legacy
    /// float projection used by bounded sliders and paint geometry.
    exact_value: Option<NumericValue>,
    default: f64,
    exact_default: Option<NumericValue>,
    min: Option<f64>,
    max: Option<f64>,
    exact_min: Option<NumericValue>,
    exact_max: Option<NumericValue>,
    step: f64,
    exact_step: Option<u64>,
    kind: NumericFieldKind,
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
    pub fn value_projection(&self) -> f64 {
        self.value
    }
    pub fn default_projection(&self) -> f64 {
        self.default
    }
    pub fn bounds_projection(&self) -> (Option<f64>, Option<f64>) {
        (self.min, self.max)
    }
    pub fn min_projection(&self) -> Option<f64> {
        self.min
    }
    pub fn max_projection(&self) -> Option<f64> {
        self.max
    }
    pub fn step_projection(&self) -> f64 {
        self.step
    }
    pub fn kind(&self) -> NumericFieldKind {
        self.kind
    }
    pub fn typed_bounds(&self) -> (Option<NumericValue>, Option<NumericValue>) {
        (self.exact_min, self.exact_max)
    }
    pub fn typed_min(&self) -> Option<NumericValue> {
        self.exact_min
    }
    pub fn typed_max(&self) -> Option<NumericValue> {
        self.exact_max
    }
    pub fn typed_default(&self) -> Option<NumericValue> {
        self.exact_default
    }
    pub fn typed_step(&self) -> Option<u64> {
        self.exact_step
    }

    pub fn set_step(&mut self, step: f64) -> Result<(), &'static str> {
        if !step.is_finite() || step <= 0.0 {
            return Err("numeric step must be finite and positive");
        }
        if self.kind != NumericFieldKind::Float {
            if step.fract() != 0.0 || step > 9_007_199_254_740_992.0 {
                return Err("integer numeric step must be exactly representable");
            }
            self.exact_step = Some(step as u64);
        }
        self.step = step;
        Ok(())
    }

    pub fn set_typed_step(&mut self, step: u64) -> Result<(), &'static str> {
        if self.kind == NumericFieldKind::Float || step == 0 {
            return Err("typed step requires a positive integer field step");
        }
        self.exact_step = Some(step);
        self.step = step as f64;
        Ok(())
    }

    pub fn new(
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        step: f64,
        kind: NumericFieldKind,
    ) -> Result<Self, &'static str> {
        if kind != NumericFieldKind::Float
            && (value.abs() > 9_007_199_254_740_992.0
                || min.is_some_and(|min| min.abs() > 9_007_199_254_740_992.0)
                || max.is_some_and(|max| max.abs() > 9_007_199_254_740_992.0))
        {
            return Err("full-width integers require a typed constructor");
        }
        if !value.is_finite()
            || !step.is_finite()
            || step <= 0.0
            || min.is_some_and(|bound| !bound.is_finite())
            || max.is_some_and(|bound| !bound.is_finite())
            || min.zip(max).is_some_and(|(min, max)| min > max)
        {
            return Err("invalid numeric field value, bounds, or step");
        }
        if kind == NumericFieldKind::Unsigned
            && (value < 0.0
                || min.is_some_and(|bound| bound < 0.0)
                || max.is_some_and(|bound| bound < 0.0))
        {
            return Err("unsigned numeric values and bounds cannot be negative");
        }
        if kind != NumericFieldKind::Float
            && (step.fract() != 0.0 || step > 9_007_199_254_740_992.0)
        {
            return Err("integer numeric step must be an exactly representable whole number");
        }
        if kind == NumericFieldKind::Integer && integer_bounds(min, max).is_none() {
            return Err("integer numeric bounds contain no representable value");
        }
        let mut field = Self {
            value,
            exact_value: None,
            default: value,
            exact_default: None,
            min,
            max,
            exact_min: None,
            exact_max: None,
            step,
            exact_step: (kind != NumericFieldKind::Float).then_some(step as u64),
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
        if kind == NumericFieldKind::Integer {
            let exact = NumericValue::Integer(field.value as i64);
            field.exact_value = Some(exact);
            field.exact_default = Some(exact);
            field.exact_min = min.map(|bound| NumericValue::Integer(bound.ceil() as i64));
            field.exact_max = max.map(|bound| NumericValue::Integer(bound.floor() as i64));
        }
        if kind == NumericFieldKind::Unsigned {
            let exact = NumericValue::Unsigned(field.value as u64);
            field.exact_value = Some(exact);
            field.exact_default = Some(exact);
            field.exact_min = min.map(|bound| NumericValue::Unsigned(bound.ceil().max(0.0) as u64));
            field.exact_max =
                max.map(|bound| NumericValue::Unsigned(bound.floor().max(0.0) as u64));
        }
        Ok(field)
    }

    pub fn float(value: f64) -> Result<Self, &'static str> {
        Self::new(value, None, None, 1.0, NumericFieldKind::Float)
    }

    pub fn integer(value: i64) -> Self {
        let projected = value as f64;
        Self {
            value: projected,
            exact_value: Some(NumericValue::Integer(value)),
            default: projected,
            exact_default: Some(NumericValue::Integer(value)),
            min: None,
            max: None,
            exact_min: None,
            exact_max: None,
            step: 1.0,
            exact_step: Some(1),
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

    pub fn unsigned(value: u64) -> Self {
        let projected = value as f64;
        Self {
            value: projected,
            exact_value: Some(NumericValue::Unsigned(value)),
            default: projected,
            exact_default: Some(NumericValue::Unsigned(value)),
            min: None,
            max: None,
            exact_min: None,
            exact_max: None,
            step: 1.0,
            exact_step: Some(1),
            kind: NumericFieldKind::Unsigned,
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
        if let Some(value) = self.exact_value {
            return value;
        }
        match self.kind {
            NumericFieldKind::Integer => NumericValue::Integer(self.value.round() as i64),
            NumericFieldKind::Unsigned => NumericValue::Unsigned(self.value.round() as u64),
            NumericFieldKind::Float => NumericValue::Float(self.value),
        }
    }

    pub fn set_typed_value(&mut self, value: NumericValue) -> Result<bool, &'static str> {
        if !self.enabled || self.read_only {
            return Ok(false);
        }
        self.restore_typed_value(value)
    }

    pub fn restore_typed_value(&mut self, value: NumericValue) -> Result<bool, &'static str> {
        let value = self.normalize_typed(value)?;
        let changed = self.typed_value() != value;
        self.exact_value = match value {
            NumericValue::Float(_) => None,
            exact => Some(exact),
        };
        self.value = value.as_f64();
        self.expression_error = None;
        Ok(changed)
    }

    pub fn set_typed_default(&mut self, value: NumericValue) -> Result<(), &'static str> {
        let value = self.normalize_typed(value)?;
        self.exact_default = match value {
            NumericValue::Float(_) => None,
            exact => Some(exact),
        };
        self.default = value.as_f64();
        Ok(())
    }

    pub fn set_typed_bounds(
        &mut self,
        min: Option<NumericValue>,
        max: Option<NumericValue>,
    ) -> Result<bool, &'static str> {
        let valid = match (self.kind, min, max) {
            (
                NumericFieldKind::Float,
                Some(NumericValue::Float(a)),
                Some(NumericValue::Float(b)),
            ) => a.is_finite() && b.is_finite() && a <= b,
            (NumericFieldKind::Float, Some(NumericValue::Float(a)), None) => a.is_finite(),
            (NumericFieldKind::Float, None, Some(NumericValue::Float(b))) => b.is_finite(),
            (NumericFieldKind::Float, None, None) => true,
            (
                NumericFieldKind::Integer,
                Some(NumericValue::Integer(a)),
                Some(NumericValue::Integer(b)),
            ) => a <= b,
            (
                NumericFieldKind::Unsigned,
                Some(NumericValue::Unsigned(a)),
                Some(NumericValue::Unsigned(b)),
            ) => a <= b,
            (NumericFieldKind::Integer, Some(NumericValue::Integer(_)), None)
            | (NumericFieldKind::Integer, None, Some(NumericValue::Integer(_)))
            | (NumericFieldKind::Unsigned, Some(NumericValue::Unsigned(_)), None)
            | (NumericFieldKind::Unsigned, None, Some(NumericValue::Unsigned(_)))
            | (NumericFieldKind::Integer | NumericFieldKind::Unsigned, None, None) => true,
            _ => false,
        };
        if !valid {
            return Err("invalid typed numeric bounds");
        }
        let before = self.typed_value();
        self.exact_min = min;
        self.exact_max = max;
        self.min = min.map(NumericValue::as_f64);
        self.max = max.map(NumericValue::as_f64);
        let value = self.typed_value();
        self.restore_typed_value(value)?;
        if let Some(default) = self.exact_default {
            self.set_typed_default(default)?;
        }
        Ok(before != self.typed_value())
    }

    fn normalize_typed(&self, value: NumericValue) -> Result<NumericValue, &'static str> {
        match (self.kind, value) {
            (NumericFieldKind::Float, NumericValue::Float(value)) if value.is_finite() => {
                Ok(NumericValue::Float(self.normalize(value)))
            }
            (NumericFieldKind::Integer, NumericValue::Integer(value)) => {
                let min = match self.exact_min {
                    Some(NumericValue::Integer(min)) => min,
                    _ => i64::MIN,
                };
                let max = match self.exact_max {
                    Some(NumericValue::Integer(max)) => max,
                    _ => i64::MAX,
                };
                Ok(NumericValue::Integer(value.clamp(min, max)))
            }
            (NumericFieldKind::Unsigned, NumericValue::Unsigned(value)) => {
                let min = match self.exact_min {
                    Some(NumericValue::Unsigned(min)) => min,
                    _ => u64::MIN,
                };
                let max = match self.exact_max {
                    Some(NumericValue::Unsigned(max)) => max,
                    _ => u64::MAX,
                };
                Ok(NumericValue::Unsigned(value.clamp(min, max)))
            }
            _ => Err("numeric value has wrong type or is non-finite"),
        }
    }

    pub fn set_default(&mut self, value: f64) -> Result<(), &'static str> {
        if self.kind != NumericFieldKind::Float {
            return self.set_typed_default(self.legacy_integer(value)?);
        }
        if !value.is_finite() {
            return Err("default must be finite");
        }
        self.default = self.normalize(value);
        Ok(())
    }

    pub fn set_bounds(&mut self, min: Option<f64>, max: Option<f64>) -> Result<bool, &'static str> {
        if self.kind != NumericFieldKind::Float {
            let min = min
                .map(|value| self.legacy_integer(value.ceil()))
                .transpose()?;
            let max = max
                .map(|value| self.legacy_integer(value.floor()))
                .transpose()?;
            return self.set_typed_bounds(min, max);
        }
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
        if self.kind != NumericFieldKind::Float {
            return self.set_typed_value(self.legacy_integer(value)?);
        }
        if !self.enabled || self.read_only {
            return Ok(false);
        }
        self.restore_value(value)
    }

    /// Restore a value from history or an external data source, bypassing the
    /// user-facing enabled/read-only guards while retaining numeric validation.
    pub fn restore_value(&mut self, value: f64) -> Result<bool, &'static str> {
        if self.kind != NumericFieldKind::Float {
            return self.restore_typed_value(self.legacy_integer(value)?);
        }
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
        if self.kind != NumericFieldKind::Float {
            if !self.enabled
                || self.read_only
                || self.typed_value() == self.exact_default.unwrap_or(self.typed_value())
            {
                return false;
            }
            if let Some(default) = self.exact_default {
                return self.restore_typed_value(default).unwrap_or(false);
            }
        }
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
        vars: &HashMap<String, NumericValue>,
    ) -> Result<bool, Error> {
        if !self.enabled || self.read_only {
            return Ok(false);
        }
        if self.kind != NumericFieldKind::Float {
            let result = match self.typed_value() {
                NumericValue::Integer(value) => {
                    expression::evaluate_exact_i64_with_vars(input, value, vars)
                        .map(NumericValue::Integer)
                }
                NumericValue::Unsigned(value) => {
                    expression::evaluate_exact_u64_with_vars(input, value, vars)
                        .map(NumericValue::Unsigned)
                }
                NumericValue::Float(_) => Err(Error::Domain),
            };
            let has_division = input.contains('/');
            let needs_legacy = matches!(&result, Err(Error::Domain)) || has_division;
            let legacy_safe = needs_legacy;
            let evaluate_fractional = || {
                expression::evaluate_typed_safe(input, self.typed_value().as_f64(), vars).and_then(
                    |value| {
                        let rounded = value.round();
                        if rounded.abs() > 9_007_199_254_740_992.0 {
                            return Err(Error::Overflow);
                        }
                        match self.kind {
                            NumericFieldKind::Integer => Ok(NumericValue::Integer(rounded as i64)),
                            NumericFieldKind::Unsigned if rounded >= 0.0 => {
                                Ok(NumericValue::Unsigned(rounded as u64))
                            }
                            _ => Err(Error::Domain),
                        }
                    },
                )
            };
            let result = match result {
                Err(Error::Domain) if legacy_safe => evaluate_fractional(),
                Ok(exact) if legacy_safe && has_division => match evaluate_fractional() {
                    Err(Error::Domain) => Ok(exact),
                    result => result,
                },
                result => result,
            };
            let value = match result {
                Ok(value) => value,
                Err(error) => {
                    self.expression_error = Some(error.clone());
                    return Err(error);
                }
            };
            let changed = self.set_typed_value(value).map_err(|_| Error::Overflow)?;
            self.editing = None;
            self.replace_on_input = false;
            self.expression_error = None;
            return Ok(changed);
        }
        let result = match expression::evaluate_typed(input, self.value, vars) {
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
            self.editing = Some(self.value_text());
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
    pub fn commit_edit(&mut self, vars: &HashMap<String, NumericValue>) -> Result<bool, Error> {
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
        if let Some(step) = self.exact_step {
            let current = self.typed_value();
            let delta = i128::from(step) * i128::from(steps);
            let next = match current {
                NumericValue::Integer(value) => NumericValue::Integer(
                    (i128::from(value) + delta).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                        as i64,
                ),
                NumericValue::Unsigned(value) => NumericValue::Unsigned(
                    (i128::from(value) + delta).clamp(0, i128::from(u64::MAX)) as u64,
                ),
                NumericValue::Float(_) => return false,
            };
            return self.set_typed_value(next).unwrap_or(false);
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
        if let Some(value) = self.exact_value {
            return match value {
                NumericValue::Integer(value) => value.to_string(),
                NumericValue::Unsigned(value) => value.to_string(),
                NumericValue::Float(value) => format_float(value, self.precision),
            };
        }
        match self.kind {
            NumericFieldKind::Integer => format!("{}", self.value.round() as i64),
            NumericFieldKind::Unsigned => format!("{}", self.value.round() as u64),
            NumericFieldKind::Float => format_float(self.value, self.precision),
        }
    }

    /// Full-precision numeric text for lossless copy and paste.
    pub fn clipboard_value(&self) -> String {
        if self.exact_value.is_some() {
            return self.value_text();
        }
        match self.kind {
            NumericFieldKind::Integer => format!("{}", self.value.round() as i64),
            NumericFieldKind::Unsigned => format!("{}", self.value.round() as u64),
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
            NumericFieldKind::Integer | NumericFieldKind::Unsigned => {
                let (min, max) = integer_bounds(self.min, self.max).unwrap_or((0.0, 0.0));
                value.round().clamp(min, max)
            }
            NumericFieldKind::Float => value.clamp(
                self.min.unwrap_or(f64::NEG_INFINITY),
                self.max.unwrap_or(f64::INFINITY),
            ),
        }
    }

    fn legacy_integer(&self, value: f64) -> Result<NumericValue, &'static str> {
        if !value.is_finite() || value.fract() != 0.0 || value.abs() > 9_007_199_254_740_992.0 {
            return Err("exact integer requires a typed value");
        }
        match self.kind {
            NumericFieldKind::Integer => Ok(NumericValue::Integer(value as i64)),
            NumericFieldKind::Unsigned if value >= 0.0 => Ok(NumericValue::Unsigned(value as u64)),
            _ => Err("wrong integer kind"),
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
    fn exact_full_width_signed_and_unsigned_values_survive_edits() {
        let mut signed = NumericField::integer(i64::MIN);
        assert_eq!(signed.value_text(), i64::MIN.to_string());
        signed.begin_edit();
        assert_eq!(signed.editing.as_deref(), Some("-9223372036854775808"));
        assert!(signed.evaluate_input("x + 1", &HashMap::new()).unwrap());
        assert_eq!(signed.typed_value(), NumericValue::Integer(i64::MIN + 1));
        assert_eq!(signed.clipboard_value(), (i64::MIN + 1).to_string());

        let mut unsigned = NumericField::unsigned(u64::MAX);
        unsigned.begin_edit();
        assert!(unsigned.evaluate_input("x + 1", &HashMap::new()).is_err());
        assert_eq!(unsigned.typed_value(), NumericValue::Unsigned(u64::MAX));
        assert!(unsigned.editing.is_some(), "invalid draft remains editable");
        unsigned.cancel_edit();
        assert!(unsigned.step_by(-1));
        assert_eq!(unsigned.typed_value(), NumericValue::Unsigned(u64::MAX - 1));
        assert!(
            unsigned
                .restore_typed_value(NumericValue::Unsigned(u64::MAX))
                .unwrap()
        );
        assert_eq!(unsigned.value_text(), u64::MAX.to_string());
    }

    #[test]
    fn integer_fields_keep_named_expression_features_without_rounding_full_width_values() {
        let mut field = NumericField::unsigned(u64::MAX - 1);
        let vars = HashMap::from([
            ("c".into(), NumericValue::Integer(1)),
            ("y".into(), NumericValue::Unsigned(3)),
        ]);
        assert!(field.evaluate_input("x + y if c else 0", &vars).is_err());
        assert_eq!(field.typed_value(), NumericValue::Unsigned(u64::MAX - 1));
        assert!(field.evaluate_input("max(x, 7)", &vars).is_ok());
        assert_eq!(field.typed_value(), NumericValue::Unsigned(u64::MAX - 1));
        assert!(field.evaluate_input("x - y if c else 0", &vars).unwrap());
        assert_eq!(field.typed_value(), NumericValue::Unsigned(u64::MAX - 4));

        let mut safe = NumericField::integer(3);
        assert!(safe.evaluate_input("round(1.7)", &HashMap::new()).unwrap());
        assert_eq!(safe.typed_value(), NumericValue::Integer(2));
        assert!(
            safe.evaluate_input("min(4, 3) + 2*pi", &HashMap::new())
                .unwrap()
        );
        assert_eq!(safe.typed_value(), NumericValue::Integer(9));
        assert!(safe.evaluate_input("3/2*2", &HashMap::new()).unwrap());
        assert_eq!(safe.typed_value(), NumericValue::Integer(3));
        assert!(safe.evaluate_input("sqrt(2)*2", &HashMap::new()).is_ok());
        assert_eq!(safe.typed_value(), NumericValue::Integer(3));
        let unused_wide = HashMap::from([("unused".into(), NumericValue::Unsigned(u64::MAX))]);
        assert!(safe.evaluate_input("sqrt(2)*2", &unused_wide).is_ok());
        assert!(
            safe.evaluate_input("sqrt(2)*2 if 1 else 18446744073709551615", &unused_wide)
                .is_ok()
        );
        let selected_wide = HashMap::from([("wide".into(), NumericValue::Unsigned(u64::MAX))]);
        assert!(safe.evaluate_input("sqrt(wide)", &selected_wide).is_err());
        assert!(
            safe.evaluate_input("sqrt(18446744073709551615)", &HashMap::new())
                .is_err()
        );
        assert!(
            field
                .evaluate_input("x + sqrt(2)", &HashMap::new())
                .is_err()
        );
        assert!(field.evaluate_input("sqrt(2)", &HashMap::new()).is_ok());
        assert_eq!(field.typed_value(), NumericValue::Unsigned(1));

        let mut wide_literal = NumericField::unsigned(0);
        assert!(
            wide_literal
                .evaluate_input("18446744073709551615/2", &HashMap::new())
                .unwrap()
        );
        assert_eq!(
            wide_literal.typed_value(),
            NumericValue::Unsigned(9_223_372_036_854_775_808)
        );
    }

    #[test]
    fn typed_variables_and_setters_keep_full_width_state_consistent() {
        let mut field = NumericField::unsigned(u64::MAX);
        let vars = HashMap::from([("limit".to_owned(), NumericValue::Unsigned(u64::MAX - 1))]);
        assert!(field.evaluate_input("x - limit", &vars).unwrap());
        assert_eq!(field.typed_value(), NumericValue::Unsigned(1));
        assert!(
            field
                .set_typed_value(NumericValue::Unsigned(u64::MAX))
                .unwrap()
        );
        assert_eq!(field.value_text(), u64::MAX.to_string());
        assert_eq!(field.value_projection(), u64::MAX as f64);
        assert!(field.set_value(u64::MAX as f64).is_err());
        assert!(field.set_typed_step(u64::MAX).is_ok());
        assert_eq!(field.typed_step(), Some(u64::MAX));
        assert!(field.set_step(0.5).is_err());
        assert_eq!(field.typed_step(), Some(u64::MAX));
    }

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
            ("x".to_owned(), NumericValue::Float(999.0)),
            ("c".to_owned(), NumericValue::Float(1.0)),
            ("y".to_owned(), NumericValue::Float(7.0)),
        ]);
        assert!(!field.evaluate_input("x if c else y", &variables).unwrap());
        assert_eq!(field.value, 3.0);
        variables.insert("c".to_owned(), NumericValue::Float(0.0));
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
    fn unsigned_constructor_rejects_negative_inputs_and_fractional_steps() {
        for (value, min, max) in [
            (-1.0, None, None),
            (0.0, Some(-1.0), Some(10.0)),
            (0.0, Some(0.0), Some(-1.0)),
        ] {
            assert!(NumericField::new(value, min, max, 1.0, NumericFieldKind::Unsigned).is_err());
        }
        assert!(NumericField::new(2.0, None, None, 0.5, NumericFieldKind::Integer).is_err());
        assert!(NumericField::new(2.0, None, None, 0.5, NumericFieldKind::Unsigned).is_err());
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

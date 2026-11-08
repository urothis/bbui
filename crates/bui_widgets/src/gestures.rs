//! Pointer-gesture state independent of platform cursor capture.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericTarget {
    pub start: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericModifiers {
    pub precision: bool,
    pub snap: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumericGesture {
    targets: Vec<NumericTarget>,
    scaled_delta: f64,
    values: Vec<f64>,
}

impl NumericGesture {
    pub fn begin(targets: Vec<NumericTarget>) -> Result<Self, &'static str> {
        if targets.is_empty() {
            return Err("gesture has no targets");
        }
        if targets.iter().any(|target| {
            !target.start.is_finite()
                || !target.min.is_finite()
                || !target.max.is_finite()
                || !target.step.is_finite()
                || target.min > target.max
                || target.step <= 0.0
                || target.start < target.min
                || target.start > target.max
        }) {
            return Err("invalid numeric target");
        }
        let values = targets.iter().map(|target| target.start).collect();
        Ok(Self {
            targets,
            scaled_delta: 0.0,
            values,
        })
    }

    /// Apply relative pointer motion. Call again when modifiers change, even with zero motion.
    pub fn update(&mut self, movement: f64, modifiers: NumericModifiers) -> &[f64] {
        let scale = if modifiers.precision { 0.1 } else { 1.0 };
        if movement.is_finite() {
            let next = self.scaled_delta + movement * scale;
            if next.is_finite() {
                self.scaled_delta = next;
            }
        }
        for (target, value) in self.targets.iter().zip(&mut self.values) {
            let raw = target.start + self.scaled_delta * target.step;
            let adjusted = if modifiers.snap {
                (raw / target.step).round() * target.step
            } else {
                raw
            };
            *value = adjusted.clamp(target.min, target.max);
        }
        &self.values
    }

    /// Values to restore if Escape or pointer-capture loss cancels the gesture.
    pub fn cancel(self) -> Vec<f64> {
        self.targets
            .into_iter()
            .map(|target| target.start)
            .collect()
    }

    /// Committed values; the caller records them as one undo transaction.
    pub fn commit(self) -> Vec<f64> {
        self.values
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckboxPaint {
    target_value: bool,
    visited: std::collections::HashSet<u64>,
}

impl CheckboxPaint {
    pub fn begin(first_id: u64, first_value: bool) -> Self {
        Self {
            target_value: !first_value,
            visited: [first_id].into(),
        }
    }

    pub fn target_value(&self) -> bool {
        self.target_value
    }

    /// Returns a value only the first time this checkbox is traversed.
    pub fn visit(&mut self, id: u64) -> Option<bool> {
        self.visited.insert(id).then_some(self.target_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_drag_recomputes_modifiers_and_restores_every_target() {
        let mut gesture = NumericGesture::begin(vec![
            NumericTarget {
                start: 2.0,
                min: 0.0,
                max: 10.0,
                step: 1.0,
            },
            NumericTarget {
                start: 20.0,
                min: 0.0,
                max: 30.0,
                step: 2.0,
            },
        ])
        .unwrap();
        assert_eq!(
            gesture.update(
                5.0,
                NumericModifiers {
                    precision: true,
                    snap: false
                }
            ),
            &[2.5, 21.0]
        );
        assert_eq!(
            gesture.update(
                0.0,
                NumericModifiers {
                    precision: false,
                    snap: true
                }
            ),
            &[3.0, 22.0]
        );
        assert_eq!(
            gesture.update(
                0.0,
                NumericModifiers {
                    precision: false,
                    snap: false
                }
            ),
            &[2.5, 21.0],
            "precision changes with no pointer motion must not jump"
        );
        assert_eq!(
            gesture.update(
                1.0,
                NumericModifiers {
                    precision: false,
                    snap: false
                }
            ),
            &[3.5, 23.0]
        );
        assert_eq!(gesture.cancel(), vec![2.0, 20.0]);
    }

    #[test]
    fn checkbox_is_only_painted_once() {
        let mut paint = CheckboxPaint::begin(1, false);
        assert!(paint.target_value());
        assert_eq!(paint.visit(2), Some(true));
        assert_eq!(paint.visit(2), None);
        assert_eq!(paint.visit(1), None);
    }
}

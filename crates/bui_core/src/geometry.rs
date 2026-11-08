//! Small, renderer-independent geometry types shared by layout and input.

/// A finite-position, non-negative-size rectangle in logical UI coordinates.
///
/// Rectangles are half-open: the left and top edges are included while the
/// right and bottom edges are excluded. This makes adjacent hit regions
/// deterministic.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UiRect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        self.is_finite()
            && !self.is_empty()
            && x >= self.x
            && y >= self.y
            && x < self.right()
            && y < self.bottom()
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.is_finite() || !other.is_finite() {
            return None;
        }
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right > x && bottom > y).then_some(Self::new(x, y, right - x, bottom - y))
    }

    pub fn translated(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    pub fn inset(self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self::new(
            self.x + left,
            self.y + top,
            (self.width - left - right).max(0.0),
            (self.height - top - bottom).max(0.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_half_open_and_clipped() {
        let a = UiRect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.contains(0.0, 0.0));
        assert!(!a.contains(10.0, 5.0));
        assert_eq!(
            a.intersection(UiRect::new(5.0, 5.0, 10.0, 10.0)),
            Some(UiRect::new(5.0, 5.0, 5.0, 5.0))
        );
        assert_eq!(a.intersection(UiRect::new(10.0, 0.0, 2.0, 2.0)), None);
    }
}

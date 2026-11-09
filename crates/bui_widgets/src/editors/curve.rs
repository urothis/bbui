use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub id: u64,
    pub position: [f32; 2],
    /// Handle offsets relative to the point position.
    pub left_handle: [f32; 2],
    pub right_handle: [f32; 2],
}

impl CurvePoint {
    pub fn new(id: u64, x: f32, y: f32) -> Self {
        Self {
            id,
            position: [x, y],
            left_handle: [-0.15, 0.0],
            right_handle: [0.15, 0.0],
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct CurveEditor {
    pub points: Vec<CurvePoint>,
    pub interpolation: Interpolation,
    pub selected: Option<u64>,
    pub pan: [f32; 2],
    pub zoom: f32,
    next_id: u64,
}

impl Default for CurveEditor {
    fn default() -> Self {
        Self {
            points: vec![CurvePoint::new(1, 0.0, 0.0), CurvePoint::new(2, 1.0, 1.0)],
            interpolation: Interpolation::Linear,
            selected: None,
            pan: [0.0, 0.0],
            zoom: 1.0,
            next_id: 3,
        }
    }
}

impl CurveEditor {
    pub fn add_point(&mut self, x: f32, y: f32) -> Option<u64> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let x = x.clamp(0.0, 1.0);
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1)?;
        self.points.push(CurvePoint::new(id, x, y));
        self.sort_points();
        self.selected = Some(id);
        Some(id)
    }

    pub fn remove_point(&mut self, id: u64) -> bool {
        if self.points.len() <= 2 {
            return false;
        }
        let Some(index) = self.points.iter().position(|point| point.id == id) else {
            return false;
        };
        self.points.remove(index);
        if self.selected == Some(id) {
            self.selected = None;
        }
        true
    }

    pub fn select_point(&mut self, id: u64) -> bool {
        if self.points.iter().any(|point| point.id == id) && self.selected != Some(id) {
            self.selected = Some(id);
            true
        } else {
            false
        }
    }

    pub fn move_point(&mut self, id: u64, position: [f32; 2]) -> bool {
        if !position[0].is_finite() || !position[1].is_finite() {
            return false;
        }
        let Some(point) = self.points.iter_mut().find(|point| point.id == id) else {
            return false;
        };
        let position = [position[0].clamp(0.0, 1.0), position[1]];
        let changed = point.position != position;
        point.position = position;
        self.sort_points();
        changed
    }

    pub fn set_handles(&mut self, id: u64, left: [f32; 2], right: [f32; 2]) -> bool {
        if !left.into_iter().chain(right).all(f32::is_finite) {
            return false;
        }
        let Some(point) = self.points.iter_mut().find(|point| point.id == id) else {
            return false;
        };
        let changed = point.left_handle != left || point.right_handle != right;
        point.left_handle = left;
        point.right_handle = right;
        changed
    }

    pub fn zoom_by(&mut self, factor: f32, anchor: [f32; 2]) {
        if !factor.is_finite() || factor <= 0.0 || !anchor.into_iter().all(f32::is_finite) {
            return;
        }
        let current_zoom = if self.zoom.is_finite() && self.zoom > 0.0 {
            self.zoom
        } else {
            1.0
        };
        let new_zoom = (current_zoom * factor).clamp(0.05, 100.0);
        let ratio = new_zoom / current_zoom;
        self.pan = [
            anchor[0] - (anchor[0] - self.pan[0]) * ratio,
            anchor[1] - (anchor[1] - self.pan[1]) * ratio,
        ];
        self.zoom = new_zoom;
    }

    pub fn reset_view(&mut self) {
        self.pan = [0.0, 0.0];
        self.zoom = 1.0;
    }

    /// Convert a normalized curve point into the editor's normalized viewport.
    pub fn curve_to_view(&self, position: [f32; 2]) -> [f32; 2] {
        [
            position[0] * self.zoom + self.pan[0],
            position[1] * self.zoom + self.pan[1],
        ]
    }

    /// Convert a normalized viewport coordinate into curve coordinates.
    pub fn view_to_curve(&self, position: [f32; 2]) -> [f32; 2] {
        let zoom = if self.zoom.is_finite() && self.zoom > 0.0 {
            self.zoom
        } else {
            1.0
        };
        [
            (position[0] - self.pan[0]) / zoom,
            (position[1] - self.pan[1]) / zoom,
        ]
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.points.len() < 2 {
            return Err("a curve needs at least two points".to_owned());
        }
        let mut ids = BTreeSet::new();
        for point in &self.points {
            if point.id == 0 || !ids.insert(point.id) {
                return Err("curve point ids must be unique and nonzero".to_owned());
            }
            if !point.position[0].is_finite()
                || !(0.0..=1.0).contains(&point.position[0])
                || !point.position[1].is_finite()
            {
                return Err(
                    "curve point positions must be finite and x must be in 0..=1".to_owned(),
                );
            }
            if !point
                .left_handle
                .into_iter()
                .chain(point.right_handle)
                .all(f32::is_finite)
            {
                return Err("curve handles must be finite".to_owned());
            }
        }
        if self
            .points
            .windows(2)
            .any(|pair| pair[0].position[0].total_cmp(&pair[1].position[0]).is_gt())
        {
            return Err("curve points must be ordered by x position".to_owned());
        }
        if self
            .selected
            .is_some_and(|selected| !ids.contains(&selected))
        {
            return Err("selected curve point does not exist".to_owned());
        }
        if !self.pan.into_iter().all(f32::is_finite)
            || !self.zoom.is_finite()
            || !(0.05..=100.0).contains(&self.zoom)
        {
            return Err("curve viewport must have finite pan and zoom in 0.05..=100".to_owned());
        }
        if self.next_id == 0 || self.next_id <= ids.last().copied().unwrap_or(0) {
            return Err("next curve point id must exceed every point id".to_owned());
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&CurveEditorSnapshot {
            version: 1,
            points: self.points.clone(),
            interpolation: self.interpolation,
            selected: self.selected,
            pan: self.pan,
            zoom: self.zoom,
            next_id: self.next_id,
        })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut snapshot: CurveEditorSnapshot =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if snapshot.version != 1 {
            return Err(format!(
                "unsupported curve editor version {}",
                snapshot.version
            ));
        }
        let mut editor = Self {
            points: std::mem::take(&mut snapshot.points),
            interpolation: snapshot.interpolation,
            selected: snapshot.selected,
            pan: snapshot.pan,
            zoom: snapshot.zoom,
            next_id: snapshot.next_id,
        };
        editor.sort_points();
        editor.validate()?;
        Ok(editor)
    }

    pub fn pan_by(&mut self, delta: [f32; 2]) {
        if delta.into_iter().all(f32::is_finite) {
            self.pan[0] += delta[0];
            self.pan[1] += delta[1];
        }
    }

    pub fn evaluate(&self, x: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        if x <= self.points[0].position[0] {
            return self.points[0].position[1];
        }
        let Some(last) = self.points.last() else {
            return 0.0;
        };
        if x >= last.position[0] {
            return last.position[1];
        }
        let Some(pair) = self.points.windows(2).find(|pair| x <= pair[1].position[0]) else {
            return last.position[1];
        };
        let left = pair[0];
        let right = pair[1];
        let dx = right.position[0] - left.position[0];
        if dx <= f32::EPSILON {
            return right.position[1];
        }
        match self.interpolation {
            Interpolation::Constant => left.position[1],
            Interpolation::Linear => {
                let t = (x - left.position[0]) / dx;
                left.position[1] * (1.0 - t) + right.position[1] * t
            }
            Interpolation::Smooth => self.evaluate_bezier(left, right, x),
        }
    }

    fn evaluate_bezier(&self, left: CurvePoint, right: CurvePoint, x: f32) -> f32 {
        let p0 = left.position;
        let p1 = [
            left.position[0] + left.right_handle[0],
            left.position[1] + left.right_handle[1],
        ];
        let p2 = [
            right.position[0] + right.left_handle[0],
            right.position[1] + right.left_handle[1],
        ];
        let p3 = right.position;
        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..18 {
            let t = (low + high) * 0.5;
            if cubic(p0[0], p1[0], p2[0], p3[0], t) < x {
                low = t;
            } else {
                high = t;
            }
        }
        cubic(p0[1], p1[1], p2[1], p3[1], (low + high) * 0.5)
    }

    fn sort_points(&mut self) {
        self.points.sort_by(|a, b| {
            a.position[0]
                .total_cmp(&b.position[0])
                .then_with(|| a.id.cmp(&b.id))
        });
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CurveEditorSnapshot {
    version: u32,
    points: Vec<CurvePoint>,
    interpolation: Interpolation,
    selected: Option<u64>,
    pan: [f32; 2],
    zoom: f32,
    next_id: u64,
}

fn cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

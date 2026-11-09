use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CanvasShape {
    Rectangle { size: [f32; 2] },
    Ellipse { size: [f32; 2] },
    Line { end: [f32; 2] },
    Image { size: [f32; 2], asset: String },
}

impl CanvasShape {
    pub(super) fn valid(&self) -> bool {
        match self {
            Self::Rectangle { size } | Self::Ellipse { size } => {
                size.iter().all(|v| v.is_finite() && *v > 0.0)
            }
            Self::Image { size, asset } => {
                !asset.is_empty()
                    && std::path::Path::new(asset)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                    && size.iter().all(|v| v.is_finite() && *v > 0.0)
            }
            Self::Line { end } => end.iter().all(|v| v.is_finite()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasObject {
    pub id: u64,
    pub origin: [f32; 2],
    pub shape: CanvasShape,
    /// Linear RGBA; image objects use this as a tint.
    pub color: [f32; 4],
    pub visible: bool,
    pub locked: bool,
}

impl CanvasObject {
    pub(super) fn valid(&self) -> bool {
        let bounds = self.bounds();
        self.id != 0
            && self.origin.iter().all(|v| v.is_finite())
            && self
                .color
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self.shape.valid()
            && bounds.is_finite()
            && bounds.right().is_finite()
            && bounds.bottom().is_finite()
    }

    pub fn bounds(&self) -> UiRect {
        match &self.shape {
            CanvasShape::Rectangle { size }
            | CanvasShape::Ellipse { size }
            | CanvasShape::Image { size, .. } => {
                UiRect::new(self.origin[0], self.origin[1], size[0], size[1])
            }
            CanvasShape::Line { end } => UiRect::new(
                self.origin[0].min(end[0]),
                self.origin[1].min(end[1]),
                (self.origin[0] - end[0]).abs(),
                (self.origin[1] - end[1]).abs(),
            ),
        }
    }

    fn hit(&self, point: [f32; 2], tolerance: f32) -> bool {
        if !self.visible || self.locked {
            return false;
        }
        match &self.shape {
            CanvasShape::Rectangle { .. } | CanvasShape::Image { .. } => {
                self.bounds().contains(point[0], point[1])
            }
            CanvasShape::Ellipse { size } => {
                let x = (point[0] - self.origin[0]) / size[0] * 2.0 - 1.0;
                let y = (point[1] - self.origin[1]) / size[1] * 2.0 - 1.0;
                x * x + y * y <= 1.0
            }
            CanvasShape::Line { end } => {
                let dx = end[0] - self.origin[0];
                let dy = end[1] - self.origin[1];
                let length_sq = dx * dx + dy * dy;
                let t = if length_sq > 0.0 {
                    (((point[0] - self.origin[0]) * dx + (point[1] - self.origin[1]) * dy)
                        / length_sq)
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let ex = point[0] - (self.origin[0] + t * dx);
                let ey = point[1] - (self.origin[1] + t * dy);
                ex * ex + ey * ey <= tolerance * tolerance
            }
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Canvas {
    pub objects: Vec<CanvasObject>,
    pub selected: BTreeSet<u64>,
    /// Screen-space translation in logical UI pixels.
    pub pan: [f32; 2],
    pub zoom: f32,
    next_id: u64,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            selected: BTreeSet::new(),
            pan: [0.0, 0.0],
            zoom: 1.0,
            next_id: 1,
        }
    }
}

impl Canvas {
    pub const MIN_ZOOM: f32 = 0.05;
    pub const MAX_ZOOM: f32 = 32.0;

    pub fn world_to_screen(&self, point: [f32; 2]) -> [f32; 2] {
        [
            point[0] * self.zoom + self.pan[0],
            point[1] * self.zoom + self.pan[1],
        ]
    }

    pub fn screen_to_world(&self, point: [f32; 2]) -> [f32; 2] {
        [
            (point[0] - self.pan[0]) / self.zoom,
            (point[1] - self.pan[1]) / self.zoom,
        ]
    }

    pub fn pan_by(&mut self, delta: [f32; 2]) -> bool {
        if !delta.iter().all(|v| v.is_finite()) {
            return false;
        }
        let next = [self.pan[0] + delta[0], self.pan[1] + delta[1]];
        if !next.iter().all(|v| v.is_finite()) {
            return false;
        }
        let changed = next != self.pan;
        self.pan = next;
        changed
    }

    pub fn zoom_at(&mut self, screen: [f32; 2], factor: f32) -> bool {
        if !screen.iter().all(|v| v.is_finite()) || !factor.is_finite() || factor <= 0.0 {
            return false;
        }
        let next_zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        if next_zoom == self.zoom {
            return false;
        }
        let anchor = self.screen_to_world(screen);
        let next_pan = [
            screen[0] - anchor[0] * next_zoom,
            screen[1] - anchor[1] * next_zoom,
        ];
        if !next_pan.iter().all(|v| v.is_finite()) {
            return false;
        }
        self.zoom = next_zoom;
        self.pan = next_pan;
        true
    }

    pub fn add(&mut self, origin: [f32; 2], shape: CanvasShape, color: [f32; 4]) -> Option<u64> {
        let id = self.next_id;
        let next_id = id.checked_add(1)?;
        let object = CanvasObject {
            id,
            origin,
            shape,
            color,
            visible: true,
            locked: false,
        };
        if !object.valid() {
            return None;
        }
        self.objects.push(object);
        self.next_id = next_id;
        Some(id)
    }

    pub fn remove_selected(&mut self) -> usize {
        let before = self.objects.len();
        self.objects
            .retain(|object| object.locked || !self.selected.contains(&object.id));
        self.selected.clear();
        before - self.objects.len()
    }

    pub fn move_selected(&mut self, delta: [f32; 2]) -> bool {
        if !delta.iter().all(|v| v.is_finite()) {
            return false;
        }
        let mut changed = false;
        for object in &self.objects {
            if !self.selected.contains(&object.id) || object.locked {
                continue;
            }
            let mut candidate = object.clone();
            candidate.origin[0] += delta[0];
            candidate.origin[1] += delta[1];
            if let CanvasShape::Line { end } = &mut candidate.shape {
                end[0] += delta[0];
                end[1] += delta[1];
            }
            if !candidate.valid() {
                return false;
            }
        }
        for object in &mut self.objects {
            if self.selected.contains(&object.id) && !object.locked {
                object.origin[0] += delta[0];
                object.origin[1] += delta[1];
                if let CanvasShape::Line { end } = &mut object.shape {
                    end[0] += delta[0];
                    end[1] += delta[1];
                }
                changed = true;
            }
        }
        changed
    }

    /// Returns the topmost unlocked object intersecting the clipped viewport.
    pub fn hit_test(&self, screen: [f32; 2], viewport: UiRect) -> Option<u64> {
        if !viewport.contains(screen[0], screen[1]) {
            return None;
        }
        let point = self.screen_to_world(screen);
        self.objects
            .iter()
            .rev()
            .find(|object| object.hit(point, 6.0 / self.zoom))
            .map(|object| object.id)
    }

    pub fn hit_test_indexed(
        &self,
        index: &CanvasSpatialIndex,
        screen: [f32; 2],
        viewport: UiRect,
    ) -> Option<u64> {
        if !index.usable {
            return self.hit_test(screen, viewport);
        }
        if !viewport.contains(screen[0], screen[1]) || !self.zoom.is_finite() || self.zoom <= 0.0 {
            return None;
        }
        let point = self.screen_to_world(screen);
        let tolerance = 6.0 / self.zoom;
        let query = UiRect::new(
            point[0] - tolerance,
            point[1] - tolerance,
            tolerance * 2.0,
            tolerance * 2.0,
        );
        index
            .query(query)?
            .into_iter()
            .rev()
            .filter_map(|position| self.objects.get(position))
            .find(|object| object.hit(point, tolerance))
            .map(|object| object.id)
    }

    pub fn select_at(&mut self, screen: [f32; 2], viewport: UiRect, extend: bool) -> bool {
        self.select_hit(self.hit_test(screen, viewport), extend)
    }

    pub fn select_hit(&mut self, hit: Option<u64>, extend: bool) -> bool {
        let before = self.selected.clone();
        if !extend {
            self.selected.clear();
        }
        if let Some(id) = hit {
            if extend && !self.selected.insert(id) {
                self.selected.remove(&id);
            } else if !extend {
                self.selected.insert(id);
            }
        }
        self.selected != before
    }

    pub(super) fn move_selected_at_indices(&mut self, indices: &[usize], delta: [f32; 2]) -> bool {
        if !delta.iter().all(|value| value.is_finite()) {
            return false;
        }
        for index in indices {
            let Some(object) = self.objects.get(*index) else {
                continue;
            };
            if !self.selected.contains(&object.id) || object.locked {
                continue;
            }
            let mut candidate = object.clone();
            candidate.origin[0] += delta[0];
            candidate.origin[1] += delta[1];
            if let CanvasShape::Line { end } = &mut candidate.shape {
                end[0] += delta[0];
                end[1] += delta[1];
            }
            if !candidate.valid() {
                return false;
            }
        }
        let mut changed = false;
        for index in indices {
            let Some(object) = self.objects.get_mut(*index) else {
                continue;
            };
            if !self.selected.contains(&object.id) || object.locked {
                continue;
            }
            object.origin[0] += delta[0];
            object.origin[1] += delta[1];
            if let CanvasShape::Line { end } = &mut object.shape {
                end[0] += delta[0];
                end[1] += delta[1];
            }
            changed = true;
        }
        changed
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&CanvasSnapshot {
            version: 1,
            objects: self.objects.clone(),
            selected: self.selected.clone(),
            pan: self.pan,
            zoom: self.zoom,
            next_id: self.next_id,
        })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let saved: CanvasSnapshot =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if saved.version != 1 {
            return Err("unsupported canvas version".to_owned());
        }
        if !saved.pan.iter().all(|v| v.is_finite())
            || !saved.zoom.is_finite()
            || !(Self::MIN_ZOOM..=Self::MAX_ZOOM).contains(&saved.zoom)
            || saved.next_id == 0
        {
            return Err("invalid canvas transform".to_owned());
        }
        let ids: BTreeSet<_> = saved.objects.iter().map(|object| object.id).collect();
        if ids.len() != saved.objects.len()
            || saved.objects.iter().any(|object| !object.valid())
            || ids
                .iter()
                .next_back()
                .is_some_and(|id| *id >= saved.next_id)
            || !saved.selected.is_subset(&ids)
        {
            return Err("invalid canvas objects or selection".to_owned());
        }
        Ok(Self {
            objects: saved.objects,
            selected: saved.selected,
            pan: saved.pan,
            zoom: saved.zoom,
            next_id: saved.next_id,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct CanvasSnapshot {
    version: u32,
    objects: Vec<CanvasObject>,
    selected: BTreeSet<u64>,
    pan: [f32; 2],
    zoom: f32,
    next_id: u64,
}

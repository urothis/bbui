use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    Constant,
    #[default]
    Linear,
    Smooth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    pub id: u64,
    pub position: f32,
    pub color: Rgba,
}

#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
pub struct ColorRamp {
    pub stops: Vec<ColorStop>,
    pub interpolation: Interpolation,
    pub selected: Option<u64>,
    next_id: u64,
}

impl Default for ColorRamp {
    fn default() -> Self {
        Self {
            stops: vec![
                ColorStop {
                    id: 1,
                    position: 0.0,
                    color: Rgba(0.0, 0.0, 0.0, 1.0),
                },
                ColorStop {
                    id: 2,
                    position: 1.0,
                    color: Rgba(1.0, 1.0, 1.0, 1.0),
                },
            ],
            interpolation: Interpolation::Linear,
            selected: Some(1),
            next_id: 3,
        }
    }
}

impl ColorRamp {
    pub fn add_stop(&mut self, position: f32, color: Rgba) -> Option<u64> {
        if !position.is_finite() || !valid_color(color) {
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1)?;
        self.stops.push(ColorStop {
            id,
            position: position.clamp(0.0, 1.0),
            color,
        });
        self.sort_stops();
        self.selected = Some(id);
        Some(id)
    }

    pub fn remove_stop(&mut self, id: u64) -> bool {
        if self.stops.len() <= 2 {
            return false;
        }
        let Some(index) = self.stops.iter().position(|stop| stop.id == id) else {
            return false;
        };
        self.stops.remove(index);
        if self.selected == Some(id) {
            self.selected = Some(self.stops[index.min(self.stops.len() - 1)].id);
        }
        true
    }

    pub fn move_stop(&mut self, id: u64, position: f32) -> bool {
        if !position.is_finite() {
            return false;
        }
        let Some(stop) = self.stops.iter_mut().find(|stop| stop.id == id) else {
            return false;
        };
        let position = position.clamp(0.0, 1.0);
        let changed = stop.position != position;
        stop.position = position;
        self.sort_stops();
        changed
    }

    pub fn set_color(&mut self, id: u64, color: Rgba) -> bool {
        if !valid_color(color) {
            return false;
        }
        let Some(stop) = self.stops.iter_mut().find(|stop| stop.id == id) else {
            return false;
        };
        let changed = stop.color != color;
        stop.color = color;
        changed
    }

    pub fn set_color_channel(&mut self, id: u64, channel: ColorChannel, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(stop) = self.stops.iter_mut().find(|stop| stop.id == id) else {
            return false;
        };
        let value = value.clamp(0.0, 1.0);
        let component = match channel {
            ColorChannel::Red => &mut stop.color.0,
            ColorChannel::Green => &mut stop.color.1,
            ColorChannel::Blue => &mut stop.color.2,
            ColorChannel::Alpha => &mut stop.color.3,
        };
        let changed = *component != value;
        *component = value;
        changed
    }

    pub fn set_interpolation(&mut self, interpolation: Interpolation) -> bool {
        let changed = self.interpolation != interpolation;
        self.interpolation = interpolation;
        changed
    }

    pub fn selected_stop(&self) -> Option<&ColorStop> {
        let selected = self.selected?;
        self.stops.iter().find(|stop| stop.id == selected)
    }

    pub fn select_stop(&mut self, id: u64) -> bool {
        if !self.stops.iter().any(|stop| stop.id == id) {
            return false;
        }
        let changed = self.selected != Some(id);
        self.selected = Some(id);
        changed
    }

    pub fn select_relative(&mut self, delta: i32) -> bool {
        if self.stops.is_empty() {
            return false;
        }
        let index = self
            .selected
            .and_then(|id| self.stops.iter().position(|stop| stop.id == id))
            .unwrap_or(0);
        let next = (index as i64 + i64::from(delta)).rem_euclid(self.stops.len() as i64) as usize;
        self.select_stop(self.stops[next].id)
    }

    pub fn move_selected_by(&mut self, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let Some(id) = self.selected else {
            return false;
        };
        let Some(stop) = self.stops.iter().find(|stop| stop.id == id) else {
            return false;
        };
        self.move_stop(id, stop.position + delta)
    }

    /// Serialize a ramp snapshot as versioned JSON suitable for storage or undo history.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let snapshot = ColorRampSnapshot {
            version: 1,
            interpolation: self.interpolation,
            selected: self.selected,
            next_id: self.next_id,
            stops: self
                .stops
                .iter()
                .map(|stop| ColorStopSnapshot {
                    id: stop.id,
                    position: stop.position,
                    color: [stop.color.0, stop.color.1, stop.color.2, stop.color.3],
                })
                .collect(),
        };
        serde_json::to_string(&snapshot)
    }

    /// Check the invariants required for editing and versioned serialization.
    pub fn validate(&self) -> Result<(), String> {
        if self.stops.len() < 2 {
            return Err("a color ramp requires at least two stops".into());
        }
        let mut ids = BTreeSet::new();
        for stop in &self.stops {
            if stop.id == 0 || !ids.insert(stop.id) {
                return Err("color-ramp stop IDs must be unique and nonzero".into());
            }
            if !stop.position.is_finite() || !(0.0..=1.0).contains(&stop.position) {
                return Err("color-ramp stop positions must be finite values in 0..=1".into());
            }
            if !valid_color(stop.color) {
                return Err("color-ramp channels must be finite values in 0..=1".into());
            }
        }
        if self
            .selected
            .is_some_and(|selected| !ids.contains(&selected))
        {
            return Err("selected stop ID is not present in the ramp".into());
        }
        Ok(())
    }

    /// Deserialize a versioned JSON snapshot, rejecting invalid ramp invariants.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let snapshot: ColorRampSnapshot =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if snapshot.version != 1 {
            return Err(format!(
                "unsupported color-ramp version {}",
                snapshot.version
            ));
        }
        if snapshot.stops.len() < 2 {
            return Err("a color ramp requires at least two stops".into());
        }
        let mut ids = BTreeSet::new();
        let mut stops = Vec::with_capacity(snapshot.stops.len());
        for stop in snapshot.stops {
            if stop.id == 0 || !ids.insert(stop.id) {
                return Err("color-ramp stop IDs must be unique and nonzero".into());
            }
            if !stop.position.is_finite() || !(0.0..=1.0).contains(&stop.position) {
                return Err("color-ramp stop positions must be finite values in 0..=1".into());
            }
            if !stop
                .color
                .into_iter()
                .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
            {
                return Err("color-ramp channels must be finite values in 0..=1".into());
            }
            stops.push(ColorStop {
                id: stop.id,
                position: stop.position,
                color: Rgba(stop.color[0], stop.color[1], stop.color[2], stop.color[3]),
            });
        }
        let next_id = snapshot.next_id.max(
            ids.iter()
                .next_back()
                .copied()
                .unwrap_or(0)
                .saturating_add(1),
        );
        if snapshot
            .selected
            .is_some_and(|selected| !ids.contains(&selected))
        {
            return Err("selected stop ID is not present in the ramp".into());
        }
        let mut ramp = Self {
            stops,
            interpolation: snapshot.interpolation,
            selected: snapshot.selected,
            next_id,
        };
        ramp.sort_stops();
        Ok(ramp)
    }

    pub fn sample(&self, position: f32) -> Rgba {
        if self.stops.is_empty() {
            return Rgba(0.0, 0.0, 0.0, 1.0);
        }
        let position = position.clamp(0.0, 1.0);
        let Some((left, right)) = self
            .stops
            .windows(2)
            .find_map(|pair| (position <= pair[1].position).then_some((pair[0], pair[1])))
        else {
            return self
                .stops
                .last()
                .map_or(Rgba(0.0, 0.0, 0.0, 1.0), |stop| stop.color);
        };
        if self.interpolation == Interpolation::Constant || right.position <= left.position {
            return left.color;
        }
        let mut t = ((position - left.position) / (right.position - left.position)).clamp(0.0, 1.0);
        if self.interpolation == Interpolation::Smooth {
            t = t * t * (3.0 - 2.0 * t);
        }
        mix_color(left.color, right.color, t)
    }

    fn sort_stops(&mut self) {
        self.stops.sort_by(|a, b| {
            a.position
                .total_cmp(&b.position)
                .then_with(|| a.id.cmp(&b.id))
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChannel {
    #[default]
    Red,
    Green,
    Blue,
    Alpha,
}

impl ColorChannel {
    pub const ALL: [Self; 4] = [Self::Red, Self::Green, Self::Blue, Self::Alpha];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Red => "R",
            Self::Green => "G",
            Self::Blue => "B",
            Self::Alpha => "A",
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Red => 0,
            Self::Green => 1,
            Self::Blue => 2,
            Self::Alpha => 3,
        }
    }

    pub const fn from_index(index: usize) -> Self {
        match index % 4 {
            0 => Self::Red,
            1 => Self::Green,
            2 => Self::Blue,
            _ => Self::Alpha,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct ColorRampSnapshot {
    pub(super) version: u32,
    pub(super) interpolation: Interpolation,
    pub(super) selected: Option<u64>,
    pub(super) next_id: u64,
    pub(super) stops: Vec<ColorStopSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(super) struct ColorStopSnapshot {
    pub(super) id: u64,
    pub(super) position: f32,
    pub(super) color: [f32; 4],
}

fn mix_color(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let one_minus = 1.0 - t;
    Rgba(
        a.0 * one_minus + b.0 * t,
        a.1 * one_minus + b.1 * t,
        a.2 * one_minus + b.2 * t,
        a.3 * one_minus + b.3 * t,
    )
}

pub(super) fn valid_color(color: Rgba) -> bool {
    [color.0, color.1, color.2, color.3]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
}

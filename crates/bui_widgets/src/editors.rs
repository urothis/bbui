//! Data models for color ramps, animation curves, and node graphs.

use bevy_ecs::prelude::Component;
use bui_style::Rgba;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
            return self.stops.last().unwrap().color;
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
struct ColorRampSnapshot {
    version: u32,
    interpolation: Interpolation,
    selected: Option<u64>,
    next_id: u64,
    stops: Vec<ColorStopSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct ColorStopSnapshot {
    id: u64,
    position: f32,
    color: [f32; 4],
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

fn valid_color(color: Rgba) -> bool {
    [color.0, color.1, color.2, color.3]
        .into_iter()
        .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
}

/// Editable HSV state with a round-trippable hue even when saturation is zero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct HsvColorEditor {
    pub hue_degrees: f32,
    pub saturation: f32,
    pub value: f32,
    pub alpha: f32,
}

impl Default for HsvColorEditor {
    fn default() -> Self {
        Self {
            hue_degrees: 0.0,
            saturation: 1.0,
            value: 1.0,
            alpha: 1.0,
        }
    }
}

impl HsvColorEditor {
    pub fn from_rgba(color: Rgba) -> Result<Self, String> {
        if !valid_color(color) {
            return Err("color channels must be finite values in 0..=1".into());
        }
        let [hue_degrees, saturation, value] = rgb_to_hsv(color);
        Ok(Self {
            hue_degrees,
            saturation,
            value,
            alpha: color.3,
        })
    }

    pub fn color(self) -> Rgba {
        hsv_to_rgb([self.hue_degrees, self.saturation, self.value], self.alpha)
    }

    pub fn set_hsv(&mut self, field: HsvColorField, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let value = match field {
            HsvColorField::Hue => value.rem_euclid(360.0),
            HsvColorField::Saturation | HsvColorField::Value | HsvColorField::Alpha => {
                value.clamp(0.0, 1.0)
            }
            _ => return false,
        };
        let channel = match field {
            HsvColorField::Hue => &mut self.hue_degrees,
            HsvColorField::Saturation => &mut self.saturation,
            HsvColorField::Value => &mut self.value,
            HsvColorField::Alpha => &mut self.alpha,
            _ => unreachable!(),
        };
        let changed = *channel != value;
        *channel = value;
        changed
    }

    pub fn set_rgb(&mut self, field: HsvColorField, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let value = value.clamp(0.0, 1.0);
        let mut color = self.color();
        let channel = match field {
            HsvColorField::Red => &mut color.0,
            HsvColorField::Green => &mut color.1,
            HsvColorField::Blue => &mut color.2,
            _ => return false,
        };
        let before = *channel;
        *channel = value;
        if before == value {
            return false;
        }
        self.set_rgba(color)
    }

    pub fn set_rgba(&mut self, color: Rgba) -> bool {
        if !valid_color(color) {
            return false;
        }
        let before = *self;
        let old_hue = self.hue_degrees;
        let [hue_degrees, saturation, value] = rgb_to_hsv(color);
        self.hue_degrees = if saturation <= f32::EPSILON {
            old_hue
        } else {
            hue_degrees
        };
        self.saturation = saturation;
        self.value = value;
        self.alpha = color.3;
        *self != before
    }

    pub fn set_hex(&mut self, input: &str) -> Result<bool, String> {
        let color = parse_hex_color(input, self.alpha)?;
        Ok(self.set_rgba(color))
    }

    pub fn hex(self) -> String {
        let color = self.color();
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            channel_to_byte(color.0),
            channel_to_byte(color.1),
            channel_to_byte(color.2),
            channel_to_byte(color.3)
        )
    }

    pub fn validate(self) -> Result<(), String> {
        if !self.hue_degrees.is_finite() || !(0.0..360.0).contains(&self.hue_degrees) {
            return Err("hue must be finite and within 0..360 degrees".into());
        }
        if ![self.saturation, self.value, self.alpha]
            .into_iter()
            .all(|channel| channel.is_finite() && (0.0..=1.0).contains(&channel))
        {
            return Err("saturation, value and alpha must be finite values in 0..=1".into());
        }
        Ok(())
    }

    /// Serialize hue as well as RGBA, preserving hue while the color is gray or black.
    pub fn to_json(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&HsvColorSnapshot {
            version: 1,
            hue_degrees: self.hue_degrees,
            saturation: self.saturation,
            value: self.value,
            alpha: self.alpha,
        })
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let snapshot: HsvColorSnapshot =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if snapshot.version != 1 {
            return Err(format!(
                "unsupported HSV color version {}",
                snapshot.version
            ));
        }
        let editor = Self {
            hue_degrees: snapshot.hue_degrees,
            saturation: snapshot.saturation,
            value: snapshot.value,
            alpha: snapshot.alpha,
        };
        editor.validate()?;
        Ok(editor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HsvColorField {
    Hue,
    Saturation,
    Value,
    Red,
    Green,
    Blue,
    Alpha,
    Hex,
}

impl HsvColorField {
    pub const ALL: [Self; 8] = [
        Self::Hue,
        Self::Saturation,
        Self::Value,
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::Alpha,
        Self::Hex,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hue => "H",
            Self::Saturation => "S",
            Self::Value => "V",
            Self::Red => "R",
            Self::Green => "G",
            Self::Blue => "B",
            Self::Alpha => "A",
            Self::Hex => "Hex",
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct HsvColorEditorState {
    pub active_field: HsvColorField,
    pub editing: bool,
    pub edit_buffer: String,
}

impl Default for HsvColorEditorState {
    fn default() -> Self {
        Self {
            active_field: HsvColorField::Hue,
            editing: false,
            edit_buffer: String::new(),
        }
    }
}

/// Convert sRGB RGB channels to HSV.
pub fn rgb_to_hsv(color: Rgba) -> [f32; 3] {
    let max = color.0.max(color.1).max(color.2);
    let min = color.0.min(color.1).min(color.2);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == color.0 {
        60.0 * (((color.1 - color.2) / delta).rem_euclid(6.0))
    } else if max == color.1 {
        60.0 * ((color.2 - color.0) / delta + 2.0)
    } else {
        60.0 * ((color.0 - color.1) / delta + 4.0)
    };
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    [hue.rem_euclid(360.0), saturation, max]
}

/// Convert HSV to sRGB RGB channels.
pub fn hsv_to_rgb(hsv: [f32; 3], alpha: f32) -> Rgba {
    let hue = if hsv[0].is_finite() {
        hsv[0].rem_euclid(360.0)
    } else {
        0.0
    };
    let saturation = if hsv[1].is_finite() {
        hsv[1].clamp(0.0, 1.0)
    } else {
        0.0
    };
    let value = if hsv[2].is_finite() {
        hsv[2].clamp(0.0, 1.0)
    } else {
        0.0
    };
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match (hue / 60.0).floor() as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = value - chroma;
    Rgba(
        red + m,
        green + m,
        blue + m,
        if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        },
    )
}

pub fn parse_hex_color(input: &str, fallback_alpha: f32) -> Result<Rgba, String> {
    if !fallback_alpha.is_finite() {
        return Err("fallback alpha must be finite".into());
    }
    let input = input.trim();
    let digits = input.strip_prefix('#').unwrap_or(input);
    let expanded = match digits.len() {
        3 | 4 => {
            let mut expanded = String::with_capacity(digits.len() * 2);
            for digit in digits.chars() {
                expanded.push(digit);
                expanded.push(digit);
            }
            expanded
        }
        6 | 8 => digits.to_owned(),
        _ => return Err("hex color must contain 3, 4, 6 or 8 digits".into()),
    };
    if !expanded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("hex color contains a non-hexadecimal digit".into());
    }
    let parse = |index| {
        u8::from_str_radix(&expanded[index..index + 2], 16)
            .map(|value| f32::from(value) / 255.0)
            .map_err(|error| error.to_string())
    };
    let red = parse(0)?;
    let green = parse(2)?;
    let blue = parse(4)?;
    let alpha = if expanded.len() == 8 {
        parse(6)?
    } else {
        fallback_alpha.clamp(0.0, 1.0)
    };
    Ok(Rgba(red, green, blue, alpha))
}

fn channel_to_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct HsvColorSnapshot {
    version: u32,
    hue_degrees: f32,
    saturation: f32,
    value: f32,
    alpha: f32,
}

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
        if x >= self.points.last().unwrap().position[0] {
            return self.points.last().unwrap().position[1];
        }
        let Some(pair) = self.points.windows(2).find(|pair| x <= pair[1].position[0]) else {
            return self.points.last().unwrap().position[1];
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketType {
    Any,
    Boolean,
    Float,
    Vector2,
    Vector3,
    Color,
    Text,
}

impl SocketType {
    fn compatible_with(self, other: Self) -> bool {
        self == Self::Any || other == Self::Any || self == other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SocketDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSocket {
    pub name: String,
    pub direction: SocketDirection,
    pub socket_type: SocketType,
}

impl NodeSocket {
    pub fn input(name: impl Into<String>, socket_type: SocketType) -> Self {
        Self {
            name: name.into(),
            direction: SocketDirection::Input,
            socket_type,
        }
    }
    pub fn output(name: impl Into<String>, socket_type: SocketType) -> Self {
        Self {
            name: name.into(),
            direction: SocketDirection::Output,
            socket_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeModel {
    pub id: NodeId,
    pub title: String,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub sockets: Vec<NodeSocket>,
    pub collapsed: bool,
}

impl NodeModel {
    pub fn new(
        id: NodeId,
        title: impl Into<String>,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            position,
            size: [180.0, 120.0],
            sockets,
            collapsed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeLink {
    pub id: u64,
    pub from_node: NodeId,
    pub from_socket: usize,
    pub to_node: NodeId,
    pub to_socket: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkEndpoint {
    From,
    To,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeGraphError {
    MissingNode,
    MissingSocket,
    WrongDirection,
    IncompatibleSockets,
    SelfLink,
}

#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeGraph {
    pub nodes: Vec<NodeModel>,
    pub links: Vec<NodeLink>,
    pub selected: BTreeSet<NodeId>,
    pub selected_link: Option<u64>,
    pub pan: [f32; 2],
    pub zoom: f32,
    next_node: u64,
    next_link: u64,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            selected: BTreeSet::new(),
            selected_link: None,
            pan: [0.0, 0.0],
            zoom: 1.0,
            next_node: 1,
            next_link: 1,
        }
    }
}

impl NodeGraph {
    pub fn add_node(
        &mut self,
        title: impl Into<String>,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
    ) -> NodeId {
        let id = NodeId(self.next_node);
        self.next_node = self.next_node.saturating_add(1);
        self.nodes
            .push(NodeModel::new(id, title, position, sockets));
        id
    }

    pub fn move_node(&mut self, id: NodeId, position: [f32; 2]) -> bool {
        if !position.into_iter().all(f32::is_finite) {
            return false;
        }
        let Some(node) = self.nodes.iter_mut().find(|node| node.id == id) else {
            return false;
        };
        let changed = node.position != position;
        node.position = position;
        changed
    }

    pub fn select(&mut self, id: NodeId, additive: bool) -> bool {
        if !self.nodes.iter().any(|node| node.id == id) {
            return false;
        }
        let before = self.selected.clone();
        let before_link = self.selected_link;
        self.selected_link = None;
        if !additive {
            self.selected.clear();
        }
        if additive && self.selected.contains(&id) {
            self.selected.remove(&id);
        } else {
            self.selected.insert(id);
        }
        before != self.selected || before_link.is_some()
    }

    pub fn select_link(&mut self, id: u64) -> bool {
        if !self.links.iter().any(|link| link.id == id) {
            return false;
        }
        let changed = self.selected_link != Some(id) || !self.selected.is_empty();
        self.selected.clear();
        self.selected_link = Some(id);
        changed
    }

    pub fn select_all(&mut self) -> bool {
        let before = self.selected.clone();
        let before_link = self.selected_link;
        self.selected = self.nodes.iter().map(|node| node.id).collect();
        self.selected_link = None;
        before != self.selected || before_link.is_some()
    }

    pub fn move_selected_by(&mut self, delta: [f32; 2]) -> bool {
        if !delta.into_iter().all(f32::is_finite) {
            return false;
        }
        let mut changed = false;
        for node in &mut self.nodes {
            if self.selected.contains(&node.id) {
                let next = [node.position[0] + delta[0], node.position[1] + delta[1]];
                if next.into_iter().all(f32::is_finite) && node.position != next {
                    node.position = next;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn toggle_selected_collapsed(&mut self) -> bool {
        let selected: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| self.selected.contains(&node.id))
            .map(|node| node.id)
            .collect();
        if selected.is_empty() {
            return false;
        }
        let collapse = self
            .nodes
            .iter()
            .find(|node| node.id == selected[0])
            .is_some_and(|node| !node.collapsed);
        for node in &mut self.nodes {
            if self.selected.contains(&node.id) {
                node.collapsed = collapse;
            }
        }
        true
    }

    pub fn create_link(
        &mut self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<u64, NodeGraphError> {
        self.validate_link(from_node, from_socket, to_node, to_socket)?;
        self.links
            .retain(|link| !(link.to_node == to_node && link.to_socket == to_socket));
        let id = self.next_link;
        self.next_link = self.next_link.saturating_add(1);
        self.links.push(NodeLink {
            id,
            from_node,
            from_socket,
            to_node,
            to_socket,
        });
        Ok(id)
    }

    pub fn reconnect_link(
        &mut self,
        link_id: u64,
        endpoint: LinkEndpoint,
        node_id: NodeId,
        socket_index: usize,
    ) -> Result<(), NodeGraphError> {
        let old = *self
            .links
            .iter()
            .find(|link| link.id == link_id)
            .ok_or(NodeGraphError::MissingSocket)?;
        let (from_node, from_socket, to_node, to_socket) = match endpoint {
            LinkEndpoint::From => (node_id, socket_index, old.to_node, old.to_socket),
            LinkEndpoint::To => (old.from_node, old.from_socket, node_id, socket_index),
        };
        self.validate_link(from_node, from_socket, to_node, to_socket)?;
        if endpoint == LinkEndpoint::To {
            self.links.retain(|link| {
                link.id == link_id || link.to_node != to_node || link.to_socket != to_socket
            });
        }
        let link = self
            .links
            .iter_mut()
            .find(|link| link.id == link_id)
            .unwrap();
        link.from_node = from_node;
        link.from_socket = from_socket;
        link.to_node = to_node;
        link.to_socket = to_socket;
        Ok(())
    }

    pub fn remove_link(&mut self, id: u64) -> bool {
        let before = self.links.len();
        self.links.retain(|link| link.id != id);
        if self.selected_link == Some(id) {
            self.selected_link = None;
        }
        self.links.len() != before
    }

    pub fn remove_node(&mut self, id: NodeId) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|node| node.id != id);
        if self.nodes.len() == before {
            return false;
        }
        self.links
            .retain(|link| link.from_node != id && link.to_node != id);
        self.selected.remove(&id);
        self.selected_link = self
            .selected_link
            .filter(|link_id| self.links.iter().any(|link| link.id == *link_id));
        true
    }

    pub fn pan_by(&mut self, delta: [f32; 2]) {
        if delta.into_iter().all(f32::is_finite) {
            self.pan[0] += delta[0];
            self.pan[1] += delta[1];
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(0.1, 10.0);
        }
    }

    pub fn to_json(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string(self).map_err(|error| error.to_string())
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut graph: Self = serde_json::from_str(json).map_err(|error| error.to_string())?;
        graph.next_node = graph
            .nodes
            .iter()
            .map(|node| node.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(graph.next_node);
        graph.next_link = graph
            .links
            .iter()
            .map(|link| link.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(graph.next_link);
        graph.validate()?;
        Ok(graph)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.zoom.is_finite() || self.zoom <= 0.0 || !self.pan.into_iter().all(f32::is_finite) {
            return Err("node graph view transform must be finite with positive zoom".into());
        }
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            if node.id.0 == 0 || !node_ids.insert(node.id) {
                return Err("node IDs must be unique and nonzero".into());
            }
            if !node.position.into_iter().all(f32::is_finite)
                || !node.size.into_iter().all(f32::is_finite)
                || node.size[0] < 80.0
                || node.size[1] < 32.0
            {
                return Err(format!("node {} has invalid geometry", node.id.0));
            }
        }
        if !self.selected.iter().all(|id| node_ids.contains(id)) {
            return Err("selected node IDs must exist in the graph".into());
        }
        let mut link_ids = BTreeSet::new();
        let mut destinations = BTreeSet::new();
        for link in &self.links {
            if link.id == 0 || !link_ids.insert(link.id) {
                return Err("link IDs must be unique and nonzero".into());
            }
            self.validate_link(
                link.from_node,
                link.from_socket,
                link.to_node,
                link.to_socket,
            )
            .map_err(|error| format!("link {} is invalid: {error:?}", link.id))?;
            if !destinations.insert((link.to_node, link.to_socket)) {
                return Err("input sockets can have only one link".into());
            }
        }
        if self.selected_link.is_some_and(|id| !link_ids.contains(&id)) {
            return Err("selected link ID must exist in the graph".into());
        }
        Ok(())
    }

    fn validate_link(
        &self,
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    ) -> Result<(), NodeGraphError> {
        if from_node == to_node {
            return Err(NodeGraphError::SelfLink);
        }
        let from_node = self
            .nodes
            .iter()
            .find(|node| node.id == from_node)
            .ok_or(NodeGraphError::MissingNode)?;
        let to_node = self
            .nodes
            .iter()
            .find(|node| node.id == to_node)
            .ok_or(NodeGraphError::MissingNode)?;
        let from = from_node
            .sockets
            .get(from_socket)
            .ok_or(NodeGraphError::MissingSocket)?;
        let to = to_node
            .sockets
            .get(to_socket)
            .ok_or(NodeGraphError::MissingSocket)?;
        if from.direction != SocketDirection::Output || to.direction != SocketDirection::Input {
            return Err(NodeGraphError::WrongDirection);
        }
        if !from.socket_type.compatible_with(to.socket_type) {
            return Err(NodeGraphError::IncompatibleSockets);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_ramp_edits_stops_and_samples_interpolation() {
        let mut ramp = ColorRamp::default();
        let id = ramp.add_stop(0.5, Rgba(1.0, 0.0, 0.0, 1.0)).unwrap();
        assert_eq!(ramp.stops.len(), 3);
        assert_eq!(ramp.sample(0.5), Rgba(1.0, 0.0, 0.0, 1.0));
        assert!(ramp.move_stop(id, 0.75));
        assert!(ramp.remove_stop(id));
        assert!(!ramp.remove_stop(1));
    }

    #[test]
    fn color_ramp_json_round_trip_preserves_stops_selection_and_mode() {
        let mut ramp = ColorRamp::default();
        let inserted = ramp.add_stop(0.35, Rgba(0.2, 0.4, 0.6, 0.8)).unwrap();
        ramp.set_interpolation(Interpolation::Smooth);
        ramp.set_color_channel(inserted, ColorChannel::Green, 0.55);
        ramp.select_stop(2);

        let restored = ColorRamp::from_json(&ramp.to_json().unwrap()).unwrap();
        assert_eq!(restored, ramp);
        assert_eq!(restored.selected_stop().unwrap().id, 2);
        assert_eq!(restored.interpolation, Interpolation::Smooth);
    }

    #[test]
    fn color_ramp_json_rejects_invalid_positions_ids_and_versions() {
        let ramp = ColorRamp::default();
        let valid = ramp.to_json().unwrap();
        let mut invalid_position: ColorRampSnapshot = serde_json::from_str(&valid).unwrap();
        invalid_position.stops[0].position = 1.5;
        assert!(ColorRamp::from_json(&serde_json::to_string(&invalid_position).unwrap()).is_err());

        let mut duplicate_id: ColorRampSnapshot = serde_json::from_str(&valid).unwrap();
        duplicate_id.stops[1].id = duplicate_id.stops[0].id;
        assert!(ColorRamp::from_json(&serde_json::to_string(&duplicate_id).unwrap()).is_err());

        let mut future: ColorRampSnapshot = serde_json::from_str(&valid).unwrap();
        future.version = 2;
        assert!(ColorRamp::from_json(&serde_json::to_string(&future).unwrap()).is_err());
    }

    #[test]
    fn hsv_conversion_round_trips_primary_and_mixed_srgb_colors() {
        for color in [
            Rgba(1.0, 0.0, 0.0, 1.0),
            Rgba(0.0, 1.0, 0.0, 0.5),
            Rgba(0.0, 0.0, 1.0, 0.25),
            Rgba(0.2, 0.4, 0.6, 0.8),
            Rgba(0.0, 0.0, 0.0, 1.0),
        ] {
            let hsv = rgb_to_hsv(color);
            let round_trip = hsv_to_rgb(hsv, color.3);
            assert!((round_trip.0 - color.0).abs() < 1.0e-5);
            assert!((round_trip.1 - color.1).abs() < 1.0e-5);
            assert!((round_trip.2 - color.2).abs() < 1.0e-5);
            assert!((round_trip.3 - color.3).abs() < 1.0e-5);
        }
        assert_eq!(rgb_to_hsv(Rgba(0.0, 1.0, 0.0, 1.0))[0], 120.0);
        assert_eq!(rgb_to_hsv(Rgba(0.0, 0.0, 1.0, 1.0))[0], 240.0);
    }

    #[test]
    fn hsv_hex_entry_and_serialization_preserve_color_and_achromatic_hue() {
        let mut editor = HsvColorEditor::default();
        assert!(editor.set_hex("#33669980").unwrap());
        assert_eq!(editor.hex(), "#33669980");
        let color = editor.color();
        assert!((color.0 - 0x33 as f32 / 255.0).abs() < 1.0e-5);
        assert!((color.1 - 0x66 as f32 / 255.0).abs() < 1.0e-5);
        assert!((color.2 - 0x99 as f32 / 255.0).abs() < 1.0e-5);
        assert!((color.3 - 0x80 as f32 / 255.0).abs() < 1.0e-5);
        assert!(editor.set_hsv(HsvColorField::Hue, 217.0));
        assert!(editor.set_hsv(HsvColorField::Saturation, 0.0));
        assert_eq!(editor.hue_degrees, 217.0);
        assert_eq!(editor.hex(), "#99999980");

        let restored = HsvColorEditor::from_json(&editor.to_json().unwrap()).unwrap();
        assert_eq!(restored, editor);
        assert!(parse_hex_color("#abc", 0.25).is_ok());
        assert!(parse_hex_color("not-hex", 1.0).is_err());
        assert!(parse_hex_color("#fff", f32::NAN).is_err());
    }

    #[test]
    fn curve_points_handles_and_view_state_drive_evaluation() {
        let mut curve = CurveEditor::default();
        let id = curve.add_point(0.5, 0.2).unwrap();
        assert!((curve.evaluate(0.25) - 0.1).abs() < 0.02);
        assert!(curve.move_point(id, [0.6, 0.4]));
        assert!(curve.set_handles(id, [-0.1, 0.0], [0.1, 0.0]));
        curve.zoom_by(2.0, [0.5, 0.5]);
        assert_eq!(curve.zoom, 2.0);
        assert!(curve.remove_point(id));
    }

    #[test]
    fn curve_editor_json_round_trip_preserves_points_handles_and_view() {
        let mut curve = CurveEditor::default();
        let id = curve.add_point(0.4, 0.72).unwrap();
        curve.set_handles(id, [-0.12, -0.08], [0.18, 0.14]);
        curve.interpolation = Interpolation::Smooth;
        curve.pan = [0.13, -0.04];
        curve.zoom = 2.25;
        curve.select_point(id);
        curve.validate().unwrap();

        let restored = CurveEditor::from_json(&curve.to_json().unwrap()).unwrap();
        assert_eq!(restored, curve);

        let mut invalid = curve.clone();
        invalid.selected = Some(u64::MAX);
        assert!(invalid.validate().is_err());
        let mut invalid = curve.clone();
        invalid.zoom = f32::NAN;
        assert!(CurveEditor::from_json(&invalid.to_json().unwrap()).is_err());
        let mut invalid = curve.clone();
        invalid.points.swap(0, 1);
        assert!(invalid.validate().is_err());

        let mut unsupported: serde_json::Value =
            serde_json::from_str(&curve.to_json().unwrap()).unwrap();
        unsupported["version"] = serde_json::Value::from(99);
        assert!(CurveEditor::from_json(&unsupported.to_string()).is_err());
    }

    #[test]
    fn node_graph_validates_and_reconnects_typed_links() {
        let mut graph = NodeGraph::default();
        let source = graph.add_node(
            "source",
            [0.0, 0.0],
            vec![NodeSocket::output("value", SocketType::Float)],
        );
        let sink = graph.add_node(
            "sink",
            [1.0, 0.0],
            vec![NodeSocket::input("value", SocketType::Float)],
        );
        let wrong = graph.add_node(
            "wrong",
            [2.0, 0.0],
            vec![NodeSocket::input("value", SocketType::Text)],
        );
        let link = graph.create_link(source, 0, sink, 0).unwrap();
        assert_eq!(
            graph.create_link(source, 0, wrong, 0),
            Err(NodeGraphError::IncompatibleSockets)
        );
        assert_eq!(
            graph.reconnect_link(link, LinkEndpoint::To, wrong, 0),
            Err(NodeGraphError::IncompatibleSockets)
        );
        assert_eq!(graph.links[0].to_node, sink);
        graph.select(source, false);
        graph.pan_by([2.0, -1.0]);
        graph.zoom_by(2.0);
        assert!(graph.remove_node(source));
        assert!(graph.links.is_empty());
    }

    #[test]
    fn node_graph_serialization_preserves_identity_selection_view_and_connections() {
        let mut graph = NodeGraph::default();
        let source = graph.add_node(
            "Source",
            [24.0, 40.0],
            vec![NodeSocket::output("value", SocketType::Float)],
        );
        let sink = graph.add_node(
            "Sink",
            [320.0, 80.0],
            vec![NodeSocket::input("value", SocketType::Float)],
        );
        let link = graph.create_link(source, 0, sink, 0).unwrap();
        graph.select(source, false);
        graph.nodes[1].collapsed = true;
        graph.pan = [18.0, -12.0];
        graph.zoom = 1.4;

        let restored = NodeGraph::from_json(&graph.to_json().unwrap()).unwrap();
        assert_eq!(restored, graph);
        assert_eq!(restored.links[0].id, link);

        let mut invalid = graph.clone();
        invalid.nodes[0].position[0] = f32::NAN;
        assert!(invalid.to_json().is_err());

        let mut invalid_link = graph.clone();
        invalid_link.links[0].to_socket = usize::MAX;
        let json = serde_json::to_string(&invalid_link).unwrap();
        assert!(NodeGraph::from_json(&json).is_err());
    }
}

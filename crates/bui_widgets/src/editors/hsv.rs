use super::*;

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
            _ => return false,
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

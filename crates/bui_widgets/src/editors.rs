//! Data models for color ramps, animation curves, and node graphs.

use bevy_ecs::prelude::Component;
use bui_style::Rgba;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

mod color_ramp;
pub use color_ramp::*;

mod hsv;
pub use hsv::*;

mod curve;
pub use curve::*;

mod node_graph;
pub use node_graph::*;

#[cfg(test)]
#[path = "editors/tests.rs"]
mod tests;

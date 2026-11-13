//! Data models for color ramps, animation curves, and node graphs.

use bbui_style::Rgba;
use bevy_ecs::prelude::Component;
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

mod node_graph_registry;
pub use node_graph_registry::*;

#[cfg(test)]
#[path = "editors/tests.rs"]
mod tests;

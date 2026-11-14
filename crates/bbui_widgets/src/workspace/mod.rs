//! Persistent, window-independent docking layout.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod layout;
mod operations;
mod tree;
mod types;
pub use layout::{WorkspaceLayout, WorkspaceWindow};
pub use tree::{DockNode, PaneId, SplitAxis, TabGroup, TabGroupOptions};
use tree::{containing_group, find_node_mut};
pub use types::{WorkspaceError, WorkspaceOperation};

#[cfg(test)]
mod tests;

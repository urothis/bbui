//! Persistent, window-independent docking layout.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod layout;
mod operations;
mod tree;
mod types;
pub use layout::WorkspaceLayout;
pub use tree::{DockNode, PaneId, SplitAxis, TabGroup, TabGroupOptions};
use tree::{containing_group, find_node_mut};
pub use types::{WorkspaceError, WorkspaceEvent, WorkspaceOperation, WorkspaceRestoreSource};

#[cfg(test)]
mod tests;

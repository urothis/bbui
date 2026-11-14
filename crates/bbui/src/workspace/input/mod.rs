use super::*;
use bbui_core::{UiKeyCode, UiPointerButton, UiRoutedEvent, UiRoutedEvents};
use bbui_layout::UiLayoutResult;
use bbui_widgets::{DockNode, SplitAxis};

#[derive(Clone)]
pub(super) enum WorkspaceTarget {
    Tab { group: String, pane: PaneId },
    Close { pane: PaneId },
    Group { group: String },
    Split { id: String },
    Splitter { id: String, index: usize },
}

#[derive(Clone)]
enum Gesture {
    Tab {
        pane: PaneId,
        group: String,
        window: Entity,
        start: [f32; 2],
        dragging: bool,
    },
    Splitter(SplitterGesture),
}

#[derive(Clone)]
struct SplitterGesture {
    id: String,
    index: usize,
    start: [f32; 2],
    before: WorkspaceLayout,
    shares: Vec<f32>,
    axis: SplitAxis,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DropEdge {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone)]
pub(super) struct DropLocation {
    pub group: String,
    pub edge: DropEdge,
}

#[derive(Resource, Default)]
pub(super) struct WorkspaceGesture {
    active: Option<Gesture>,
    pub hover: Option<DropLocation>,
    cross_window_events: Vec<UiRoutedEvent>,
}

mod geometry;
mod handlers;
pub(super) use bridge::bridge_cross_window_input;
pub(super) use geometry::find_split;
use geometry::*;
pub(super) use handlers::process_workspace_input;
mod bridge;

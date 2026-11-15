use super::*;
use bbui_core::{
    UiInputEvent, UiInputQueue, UiKeyCode, UiModifiers, UiPointerButton, UiRoutedEvent,
    UiRoutedEvents,
};
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
    keyboard: Option<keyboard::KeyboardDock>,
}

mod docking;
mod geometry;
mod handlers;
mod keyboard;
use docking::*;
pub(super) use geometry::find_split;
use geometry::*;
pub(super) use handlers::process_workspace_input;

pub(super) fn cancel_tab_drag_on_window_leave(
    queue: Res<UiInputQueue>,
    mut gesture: ResMut<WorkspaceGesture>,
    mut workspace: Option<ResMut<Workspace>>,
) {
    let Some(Gesture::Tab { window, .. }) = gesture.active.as_ref() else {
        return;
    };
    if queue
        .iter()
        .any(|event| matches!(event, UiInputEvent::PointerLeave { window: left } if left == window))
    {
        gesture.active = None;
        gesture.hover = None;
        if let Some(workspace) = workspace.as_mut() {
            workspace.dirty = true;
        }
    }
}

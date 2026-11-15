use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceOperation {
    Open {
        pane: PaneId,
        group: String,
        index: usize,
    },
    Select {
        group: String,
        pane: PaneId,
    },
    Close {
        pane: PaneId,
    },
    Move {
        pane: PaneId,
        group: String,
        index: usize,
    },
    Split {
        pane: PaneId,
        target: String,
        id: String,
        group: String,
        axis: SplitAxis,
        after: bool,
    },
    Grid {
        pane: PaneId,
        target: String,
        id: String,
        group: String,
        columns: usize,
    },
    Resize {
        split: String,
        shares: Vec<f32>,
    },
    SetGroupOptions {
        group: String,
        options: TabGroupOptions,
    },
}

/// A completed workspace change. BBUI has already updated its layout.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceEvent {
    PaneOpened {
        pane: PaneId,
        group: String,
        index: usize,
    },
    TabSelected {
        pane: PaneId,
        group: String,
    },
    PaneClosed {
        pane: PaneId,
        group: String,
    },
    TabReordered {
        pane: PaneId,
        group: String,
        index: usize,
    },
    PaneMoved {
        pane: PaneId,
        from: String,
        to: String,
        index: usize,
    },
    PaneDocked {
        pane: PaneId,
        target: String,
        group: String,
        axis: SplitAxis,
        after: bool,
    },
    PaneGridded {
        pane: PaneId,
        target: String,
        group: String,
        columns: usize,
    },
    SplitResized {
        split: String,
        shares: Vec<f32>,
    },
    GroupOptionsChanged {
        group: String,
        options: TabGroupOptions,
    },
    LayoutRestored {
        source: WorkspaceRestoreSource,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRestoreSource {
    Restore,
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    UnknownPane,
    UnknownGroup,
    DropRejected,
    DuplicateOrEmptyId,
    InvalidSelection,
    InvalidContainer,
    UnsupportedVersion,
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownPane => "unknown pane",
            Self::UnknownGroup => "unknown tab group or split",
            Self::DropRejected => "workspace operation rejected",
            Self::DuplicateOrEmptyId => "duplicate or empty workspace identifier",
            Self::InvalidSelection => "active tab is missing from its group",
            Self::InvalidContainer => "invalid split or grid layout",
            Self::UnsupportedVersion => "unsupported workspace layout version",
        })
    }
}

impl std::error::Error for WorkspaceError {}

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
    Detach {
        pane: PaneId,
        window: String,
        group: String,
    },
    Resize {
        split: String,
        shares: Vec<f32>,
    },
    SetGroupOptions {
        group: String,
        options: TabGroupOptions,
    },
    CloseWindow {
        window: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    UnknownPane,
    UnknownGroup,
    UnknownWindow,
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
            Self::UnknownWindow => "unknown workspace window",
            Self::DropRejected => "workspace operation rejected",
            Self::DuplicateOrEmptyId => "duplicate or empty workspace identifier",
            Self::InvalidSelection => "active tab is missing from its group",
            Self::InvalidContainer => "invalid split or grid layout",
            Self::UnsupportedVersion => "unsupported workspace layout version",
        })
    }
}

impl std::error::Error for WorkspaceError {}

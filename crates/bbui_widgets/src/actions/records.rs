use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum WidgetActionKind {
    Activate,
    Toggle,
    SetBool(bool),
    SetNumber(f64),
    StepNumber(i32),
    ResetNumber,
    InsertText(String),
    ChooseOption(usize),
    FileNavigateUp,
    FileNavigateTo(PathBuf),
    FileRefresh,
    FileSetFilter(String),
    FileSetView(FileView),
    FileSetSort(FileSort, bool),
    FileOpenIndex(usize),
    FileBookmarkCurrent,
    FileRemoveBookmarkCurrent,
    FileCreateDirectory(String),
    FileRenameSelected(String),
    FileDeleteSelected,
    FileCopySelectedTo(PathBuf),
    FileMoveSelectedTo(PathBuf),
    TogglePanel,
    PanelReorderStep(i32),
    ChooseMenuCommand(String),
    ClearMenuHistory,
    ColorRampAddStop {
        position: f32,
    },
    ColorRampRemoveSelectedStop,
    ColorRampSelectStop(u64),
    ColorRampMoveStop {
        id: u64,
        position: f32,
    },
    ColorRampSetChannel {
        channel: ColorChannel,
        value: f32,
    },
    ColorRampSetInterpolation(Interpolation),
    HsvColorSet(Rgba),
    HsvColorSetHsv {
        field: HsvColorField,
        value: f32,
    },
    HsvColorSetRgb {
        field: HsvColorField,
        value: f32,
    },
    HsvColorSetHex(String),
    CurveEditorAddPoint {
        position: [f32; 2],
    },
    CurveEditorRemoveSelectedPoint,
    CurveEditorSelectPoint(u64),
    CurveEditorMovePoint {
        id: u64,
        position: [f32; 2],
    },
    CurveEditorSetInterpolation(Interpolation),
    CurveEditorResetView,
    CollectionSelect {
        id: u64,
        ctrl: bool,
        shift: bool,
    },
    CollectionSelectAll,
    CollectionBeginRename(u64),
    CollectionCommitRename,
    CollectionCancelRename,
    CollectionMove {
        id: u64,
        index: usize,
    },
    CollectionScroll(i32),
    TreeToggleExpanded(u64),
    TreeMoveNode {
        id: u64,
        parent: Option<u64>,
        index: usize,
    },
    DataTableSelectCell {
        row_id: u64,
        column: usize,
        ctrl: bool,
        shift: bool,
    },
    DataTableSelectAll,
    DataTableSortColumn(usize),
    DataTableResizeColumn {
        column: usize,
        width: f32,
    },
    DataTableMoveActiveRow {
        delta: i32,
        extend: bool,
    },
    DataTableMoveActiveColumn(i32),
    DataTableSetActiveColumn(usize),
    DataTableScrollRows(i32),
    DataTableActivateCell,
    NodeGraphSelectNode {
        id: NodeId,
        additive: bool,
    },
    NodeGraphSelectLink(u64),
    NodeGraphSelectAll,
    NodeGraphMoveSelected([f32; 2]),
    NodeGraphAddNode {
        title: String,
        position: [f32; 2],
        sockets: Vec<NodeSocket>,
    },
    NodeGraphCreateLink {
        from_node: NodeId,
        from_socket: usize,
        to_node: NodeId,
        to_socket: usize,
    },
    NodeGraphReconnectLink {
        link_id: u64,
        endpoint: LinkEndpoint,
        node_id: NodeId,
        socket_index: usize,
    },
    NodeGraphRemoveLink(u64),
    NodeGraphDeleteSelection,
    NodeGraphToggleCollapse,
    NodeGraphPanBy([f32; 2]),
    NodeGraphZoomBy(f32),
    CanvasSelectObject {
        id: u64,
        additive: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetAction {
    pub target: Entity,
    pub action: WidgetActionKind,
}

impl WidgetAction {
    pub fn new(target: Entity, action: WidgetActionKind) -> Self {
        Self { target, action }
    }
}

#[derive(Resource, Debug, Default)]
pub struct WidgetActionQueue(VecDeque<WidgetAction>);

impl WidgetActionQueue {
    pub fn push(&mut self, action: WidgetAction) {
        self.0.push_back(action);
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(super) fn drain(&mut self) -> impl Iterator<Item = WidgetAction> + '_ {
        self.0.drain(..)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetValue {
    Bool(bool),
    Number(f64),
    Text,
    Option(Option<usize>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckboxPaintChange {
    pub entity: Entity,
    pub before: CheckboxValue,
    pub after: CheckboxValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WidgetEvent {
    Activated {
        target: Entity,
    },
    ValueChanged {
        target: Entity,
        before: WidgetValue,
        after: WidgetValue,
    },
    TextChanged {
        target: Entity,
        text: String,
    },
    TextCommitted {
        target: Entity,
        window: Entity,
        before: String,
        after: String,
        before_selection: TextSelection,
        after_selection: TextSelection,
    },
    CheckboxPaintCommitted {
        changes: Vec<CheckboxPaintChange>,
    },
    CheckboxPaintCancelled {
        changes: Vec<CheckboxPaintChange>,
    },
    NumericCommitted {
        target: Entity,
        before: Vec<f64>,
        after: Vec<f64>,
    },
    NumericGroupCommitted {
        changes: Vec<(Entity, f64, f64)>,
    },
    NumericExpressionError {
        target: Entity,
        error: String,
    },
    ColorRampCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    HsvColorCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    CurveEditorCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    CanvasCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    CollectionCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    CollectionItemActivated {
        target: Entity,
        id: u64,
    },
    DataTableCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    NodeGraphCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    PanelGroupCommitted {
        changes: Vec<(Entity, bool, bool)>,
    },
    PanelOrderCommitted {
        group: Entity,
        persistence_key: String,
        before: Vec<String>,
        after: Vec<String>,
    },
    PanelOrderRestoreFailed {
        group: Entity,
        reason: String,
    },
    NodeGraphLinkFailed {
        target: Entity,
        error: NodeGraphError,
    },
    DataTableCellActivated {
        target: Entity,
        row_id: u64,
        column: usize,
    },
    NumericCancelled {
        target: Entity,
        restored: Vec<f64>,
    },
    ClipboardWrite {
        target: Entity,
        text: String,
    },
    ClipboardRead {
        target: Entity,
    },
    ImePreedit {
        target: Entity,
        text: String,
        cursor_range: Option<[usize; 2]>,
    },
    MenuCommand {
        target: Entity,
        command_id: String,
    },
    FileActivated {
        target: Entity,
        path: PathBuf,
    },
    FileSelected {
        target: Entity,
        path: PathBuf,
        selected: bool,
    },
    FileSelectionChanged {
        target: Entity,
        selected: Vec<PathBuf>,
    },
    FileBookmarksChanged {
        target: Entity,
        bookmarks: Vec<PathBuf>,
    },
    FileDirectoryChanged {
        target: Entity,
        path: PathBuf,
    },
    /// A reversible change to FileBrowser-owned UI state. Filesystem entries are refreshed when
    /// this snapshot is restored; actual file operations use `FileOperationCommitted` instead.
    FileBrowserCommitted {
        target: Entity,
        before: String,
        after: String,
    },
    /// A successful filesystem mutation is an external side effect and is not part of UI undo.
    FileOperationCommitted {
        target: Entity,
        operation: &'static str,
        paths: Vec<PathBuf>,
        irreversible: bool,
    },
    FileBrowserRestoreFailed {
        target: Entity,
        path: PathBuf,
        message: String,
    },
    FileOperationFailed {
        target: Entity,
        operation: &'static str,
        path: PathBuf,
        message: String,
        /// Paths changed before a non-atomic filesystem operation failed.
        partial_effects: Vec<PathBuf>,
    },
}

#[derive(Resource, Debug, Default)]
pub struct WidgetEventQueue(pub Vec<WidgetEvent>, u64);

impl WidgetEventQueue {
    pub fn take(&mut self) -> Vec<WidgetEvent> {
        self.1 = self.1.wrapping_add(1);
        std::mem::take(&mut self.0)
    }
    pub fn generation(&self) -> u64 {
        self.1
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardRequest {
    Write(String),
    Read { target: Entity },
}

/// One-shot platform clipboard requests, separate from the app-visible event history.
#[derive(Resource, Debug, Default)]
pub struct ClipboardRequestQueue(pub VecDeque<ClipboardRequest>);

impl ClipboardRequestQueue {
    pub fn push(&mut self, request: ClipboardRequest) {
        self.0.push_back(request);
    }
    pub fn drain(&mut self) -> impl Iterator<Item = ClipboardRequest> + '_ {
        self.0.drain(..)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorCaptureRequest {
    Begin {
        window: Entity,
        target: Entity,
        hide_cursor: bool,
        continuous: bool,
    },
    End {
        window: Entity,
        target: Entity,
        restore_position: bool,
    },
}

#[derive(Resource, Debug, Default)]
pub struct CursorCaptureRequestQueue(pub VecDeque<CursorCaptureRequest>);

impl CursorCaptureRequestQueue {
    pub fn drain(&mut self) -> impl Iterator<Item = CursorCaptureRequest> + '_ {
        self.0.drain(..)
    }
}

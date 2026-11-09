use super::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorRampEditorState {
    pub active_channel: ColorChannel,
    pub editing_color: bool,
}

impl Default for ColorRampEditorState {
    fn default() -> Self {
        Self {
            active_channel: ColorChannel::Red,
            editing_color: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FileBrowserViewState {
    pub scroll_rows: usize,
    pub row_height: f32,
    pub header_height: f32,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct FileBrowserItem {
    pub owner: Entity,
    pub path: PathBuf,
    pub name: String,
    pub index: usize,
    pub directory: bool,
    pub size: u64,
    pub selected: bool,
    pub preview: crate::file_browser::PreviewKind,
}

/// Stable virtual child for a single Dropdown option row.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct DropdownOptionControl {
    pub owner: Entity,
    pub index: usize,
    pub label: String,
    pub selected: bool,
    pub highlighted: bool,
    pub enabled: bool,
}

/// Geometry and scroll window for a Dropdown popup, shared with accessibility adapters.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DropdownPopupState {
    pub popup: PopupState,
    pub viewport: Rect,
    pub first_visible: usize,
    pub visible_count: usize,
}

/// Stable virtual child for a single RadioGroup option row.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct RadioOptionControl {
    pub owner: Entity,
    pub index: usize,
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
}

#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct NodeGraphEditorState {
    pub gesture: Option<NodeGraphGesture>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeGraphGesture {
    Pan {
        start_pointer: [f32; 2],
        start_pan: [f32; 2],
        before: String,
    },
    MoveNodes {
        start_pointer: [f32; 2],
        starts: Vec<(NodeId, [f32; 2])>,
        before: String,
    },
    BoxSelect {
        start_pointer: [f32; 2],
        current_pointer: [f32; 2],
        additive: bool,
        before: String,
    },
    LinkDrag {
        start_node: NodeId,
        start_socket: usize,
        start_direction: SocketDirection,
        reconnect: Option<(u64, LinkEndpoint)>,
        current_pointer: [f32; 2],
        before: String,
    },
    CutLinks {
        points: Vec<[f32; 2]>,
        before: String,
    },
    Selection {
        before: String,
    },
    SelectLink {
        before: String,
    },
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct NodeGraphNodeControl {
    pub owner: Entity,
    pub node_id: NodeId,
    pub label: String,
    pub selected: bool,
    pub collapsed: bool,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct NodeGraphSocketControl {
    pub owner: Entity,
    pub node_id: NodeId,
    pub socket_index: usize,
    pub direction: SocketDirection,
    pub label: String,
    pub socket_type: crate::SocketType,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeGraphLinkControl {
    pub owner: Entity,
    pub link_id: u64,
    pub from_node: NodeId,
    pub from_socket: usize,
    pub to_node: NodeId,
    pub to_socket: usize,
    pub selected: bool,
}

impl Default for FileBrowserViewState {
    fn default() -> Self {
        Self {
            scroll_rows: 0,
            row_height: 22.0,
            header_height: 48.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionItemKind {
    List,
    Tree,
    PreviewGrid,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct CollectionItemControl {
    pub owner: Entity,
    pub id: u64,
    pub kind: CollectionItemKind,
    pub index: usize,
    pub depth: usize,
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
    pub expanded: bool,
    pub has_children: bool,
    pub asset_id: Option<u64>,
    pub tint: [f32; 4],
}

/// A virtualized table header. `column` is the stable column identity within
/// the table declaration, while `owner` identifies the live model entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct DataTableHeaderControl {
    pub owner: Entity,
    pub column: usize,
    pub label: String,
    pub width: f32,
    pub sortable: bool,
    pub sort_ascending: Option<bool>,
    pub active: bool,
}

/// A virtualized table cell carrying stable row and column identity for input
/// routing and accessibility adapters.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct DataTableCellControl {
    pub owner: Entity,
    pub row_id: u64,
    pub row_index: usize,
    pub column: usize,
    pub column_label: String,
    pub value: String,
    pub selected: bool,
    pub active: bool,
}

#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct CollectionEditorState {
    pub rename_id: Option<u64>,
    pub rename_buffer: String,
    pub rename_original: String,
    pub rename_cursor: usize,
    pub rename_select_all: bool,
}

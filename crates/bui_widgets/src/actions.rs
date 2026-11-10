//! Bevy-facing event handlers and transaction records for reusable widgets.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use crate::{
    Button, Canvas, Checkbox, CheckboxValue, ColorChannel, ColorRamp, CurveEditor, DataTable,
    Dropdown, DropdownOptionControl, DropdownPopupState, FileBrowser, FileSort, FileView,
    HsvColorEditor, HsvColorEditorState, HsvColorField, IconButton, IconButtonAsset, Interpolation,
    Label, LinkEndpoint, ListModel, MenuControl, MenuDisplay, MenuItemControl, NodeCategory,
    NodeGraph, NodeGraphError, NodeId, NodeSocket, NumericExpressionVariables, NumericField,
    NumericFieldKind, Panel, PanelGroup, PreviewGrid, Progress, RadioGroup, Slider,
    SocketDirection, SocketType, TextArea, TextField, Tooltip, TreeModel,
    declarative::{
        CollectionEditorState, CollectionItemControl, CollectionItemKind, ColorRampEditorState,
        DataTableCellControl, DataTableHeaderControl, FileBrowserItem, FileBrowserViewState,
        NodeGraphEditorState, NodeGraphGesture, NodeGraphLinkControl, NodeGraphNodeControl,
        NodeGraphSocketControl, RadioOptionControl,
    },
    gestures::{NumericGesture, NumericModifiers, NumericTarget},
    menus::{PopupSide, PopupState},
};
use bevy_app::{App, Update};
use bevy_asset::Assets;
use bevy_ecs::change_detection::Mut;
use bevy_ecs::change_detection::Tick;
use bevy_ecs::component::ComponentId;
use bevy_ecs::lifecycle::{Add, Remove};
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_text::{Font, FontCx, FontSize, LayoutCx, LineBreak, LineHeight, TextFont, TextPipeline};
use bui_core::{
    TextSelection, UiChildren, UiDirty, UiDirtyBatch, UiDirtyDomain, UiEnabled, UiFocus,
    UiFocusable, UiHitBox, UiKey, UiKeyCode, UiLiveRegion, UiModifiers, UiNode, UiParent,
    UiPointerButton, UiRect, UiRoutedEvent, UiRoutedEvents, UiSystems, UiVisible,
    detach_from_parent, is_interactable, is_interactable_in_window, set_parent,
    ui_components_changed_since,
};
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiWheelHandled};
use bui_render::{
    ClipRect, DisplayList, DrawItem, Primitive, TextRenderStyle, WidgetFragmentBatchGroup,
    WidgetRenderFragment,
};
use bui_style::{
    EditorRole, Rgba, Theme, ThemeColorToken, ThemeMotionToken, UiClasses, UiComputedStyle,
    UiInlineStyle, UiRole, UiStylePaintOnlyOutputs, UiStyleProperty, UiStyleSheet, UiStyleState,
    UiStyleValue, set_ui_style_state,
};
use bui_text::{
    TextBuffer, TextLayout, UiReadOnly, UiTextBuffer, UiTextLayout, resolve_text_font,
    ui_default_text_font,
};

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
    fn drain(&mut self) -> impl Iterator<Item = WidgetAction> + '_ {
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

#[derive(Component, Debug, Clone, PartialEq, Eq)]
struct TextEditDraft {
    window: Entity,
    before: String,
    before_selection: TextSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextSnapshot {
    text: String,
    selection: TextSelection,
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

#[derive(Debug, Default)]
struct PaintTransaction {
    target: bool,
    visited: HashSet<Entity>,
    changes: Vec<CheckboxPaintChange>,
}

impl PaintTransaction {
    fn begin(entity: Entity, checkbox: &mut Checkbox) -> Option<Self> {
        if !checkbox.enabled || checkbox.read_only {
            return None;
        }
        let before = checkbox.value;
        let target = before != CheckboxValue::On;
        let after = if target {
            CheckboxValue::On
        } else {
            CheckboxValue::Off
        };
        if before != after {
            checkbox.value = after;
        }
        Some(Self {
            target,
            visited: [entity].into(),
            changes: (before != after)
                .then_some(CheckboxPaintChange {
                    entity,
                    before,
                    after,
                })
                .into_iter()
                .collect(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumericScrubPhase {
    Selecting,
    Scrubbing,
}

#[derive(Debug)]
struct NumericScrub {
    entity: Entity,
    targets: Vec<Entity>,
    target_specs: Vec<NumericTarget>,
    starts: Vec<f64>,
    last_values: Vec<f64>,
    gesture: NumericGesture,
    phase: NumericScrubPhase,
    last_position: [f32; 2],
    pending_x: f64,
    pending_moves: Vec<(f64, NumericModifiers)>,
    kind: NumericFieldKind,
    unit: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct SliderDrag {
    target: Entity,
    before: f64,
    min: f64,
    max: f64,
    track_left: f32,
    track_right: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorRampDragKind {
    Stop(u64),
    Channel { stop: u64, channel: ColorChannel },
}

#[derive(Debug)]
struct ColorRampDrag {
    target: Entity,
    before: ColorRamp,
    kind: ColorRampDragKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HsvColorDragKind {
    Wheel,
    Value,
    Alpha,
}

#[derive(Debug)]
struct HsvColorDrag {
    target: Entity,
    before: HsvColorEditor,
    kind: HsvColorDragKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurveEditorDragKind {
    Point(u64),
    LeftHandle(u64),
    RightHandle(u64),
    Pan,
}

#[derive(Debug)]
struct CurveEditorDrag {
    target: Entity,
    before: CurveEditor,
    kind: CurveEditorDragKind,
}

#[derive(Debug)]
struct CollectionDrag {
    owner: Entity,
    id: u64,
    kind: CollectionItemKind,
    source: Entity,
    start: [f32; 2],
    before: String,
    dragging: bool,
}

#[derive(Debug)]
struct DataTableResizeDrag {
    owner: Entity,
    source: Entity,
    column: usize,
    start_x: f32,
    start_width: f32,
    before: String,
    changed: bool,
}

#[derive(Debug)]
struct PanelReorderGesture {
    group: Entity,
    source: Entity,
    persistence_key: String,
    moving_key: String,
    before: Vec<String>,
    current: Vec<String>,
    slots: Vec<(String, f32)>,
}

#[derive(Resource, Debug, Default)]
pub struct WidgetInteractionState {
    modifiers: HashMap<Entity, UiModifiers>,
    checkbox_paints: HashMap<Entity, PaintTransaction>,
    numeric_scrubs: HashMap<Entity, NumericScrub>,
    slider_drags: HashMap<Entity, SliderDrag>,
    color_ramp_drags: HashMap<Entity, ColorRampDrag>,
    hsv_color_drags: HashMap<Entity, HsvColorDrag>,
    curve_editor_drags: HashMap<Entity, CurveEditorDrag>,
    collection_drags: HashMap<Entity, CollectionDrag>,
    data_table_resize_drags: HashMap<Entity, DataTableResizeDrag>,
    panel_reorders: HashMap<Entity, PanelReorderGesture>,
}

impl WidgetInteractionState {
    fn paint_targets(&self) -> HashSet<Entity> {
        let mut targets = HashSet::new();
        for transaction in self.checkbox_paints.values() {
            targets.extend(transaction.visited.iter().copied());
        }
        for scrub in self.numeric_scrubs.values() {
            targets.extend(scrub.targets.iter().copied());
            targets.insert(scrub.entity);
        }
        targets.extend(self.slider_drags.values().map(|drag| drag.target));
        targets.extend(self.color_ramp_drags.values().map(|drag| drag.target));
        targets.extend(self.hsv_color_drags.values().map(|drag| drag.target));
        targets.extend(self.curve_editor_drags.values().map(|drag| drag.target));
        for drag in self.collection_drags.values() {
            targets.extend([drag.owner, drag.source]);
        }
        for drag in self.data_table_resize_drags.values() {
            targets.extend([drag.owner, drag.source]);
        }
        for drag in self.panel_reorders.values() {
            targets.extend([drag.group, drag.source]);
        }
        targets
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FocusTargetSnapshot {
    window: Entity,
    focused: Option<Entity>,
    hovered: Option<Entity>,
    captured: Option<Entity>,
}

struct PaintFragmentGuard<'a> {
    fragment_entities: &'a mut HashSet<Entity>,
    changed_outputs: &'a mut HashMap<Entity, WidgetRenderFragment>,
    removed_fragments: &'a mut HashSet<Entity>,
    entity: Entity,
    fragment: Option<WidgetRenderFragment>,
}

#[derive(Debug, Clone, PartialEq)]
struct NodeGraphChromeStamp {
    graph_counts: (usize, usize),
    pan: [u32; 2],
    zoom: u32,
    gesture: NodeGraphEditorState,
    label: String,
    rect: [u32; 4],
    clip: Option<[u32; 4]>,
    z: i32,
    dpi_scale: u32,
    camera: Option<Entity>,
    theme: String,
    canvas_surface: [u32; 4],
}

#[derive(Debug, Clone)]
struct NodeGraphOutputLabelMeasurements {
    signature: String,
    widths: HashMap<usize, f32>,
}

fn node_graph_chrome_stamp(
    graph: &NodeGraph,
    state: &NodeGraphEditorState,
    label: &str,
    layout: UiLayoutResult,
    camera: Option<Entity>,
    theme: &Theme,
    canvas_surface: Rgba,
) -> NodeGraphChromeStamp {
    let rect_bits = |rect: Rect| {
        [
            rect.x.to_bits(),
            rect.y.to_bits(),
            rect.width.to_bits(),
            rect.height.to_bits(),
        ]
    };
    NodeGraphChromeStamp {
        graph_counts: (graph.nodes.len(), graph.links.len()),
        pan: [graph.pan[0].to_bits(), graph.pan[1].to_bits()],
        zoom: graph.zoom.to_bits(),
        gesture: state.clone(),
        label: label.to_owned(),
        rect: rect_bits(layout.rect),
        clip: layout.clip.map(rect_bits),
        z: layout.z,
        dpi_scale: layout.dpi_scale.to_bits(),
        camera,
        theme: format!("{theme:?}"),
        canvas_surface: [
            canvas_surface.0.to_bits(),
            canvas_surface.1.to_bits(),
            canvas_surface.2.to_bits(),
            canvas_surface.3.to_bits(),
        ],
    }
}

impl Drop for PaintFragmentGuard<'_> {
    fn drop(&mut self) {
        let Some(fragment) = self.fragment.take() else {
            return;
        };
        if !fragment.items.is_empty() {
            self.removed_fragments.remove(&self.entity);
            self.fragment_entities.insert(self.entity);
            self.changed_outputs.insert(self.entity, fragment);
        }
    }
}

#[derive(Resource, Debug, Default)]
struct WidgetPaintCache {
    prepared: bool,
    focus_active_window: Option<Entity>,
    focus_targets: Vec<FocusTargetSnapshot>,
    interaction_targets: HashSet<Entity>,
    node_graph_chrome_stamps: HashMap<Entity, NodeGraphChromeStamp>,
    node_graph_output_measurements:
        HashMap<Entity, HashMap<NodeId, NodeGraphOutputLabelMeasurements>>,
    text_measurements: HashMap<Entity, (String, f32)>,
    fragments: HashSet<Entity>,
    checkbox_static_stamps: HashMap<Entity, CheckboxPaintStaticStamp>,
    paint_ranks: HashMap<Entity, u64>,
    paint_order_keys: BTreeMap<u64, Entity>,
    paint_children: HashMap<Entity, Vec<Entity>>,
    paint_roots: BTreeMap<u32, Entity>,
    paint_parents: HashMap<Entity, Entity>,
    pending_parent_changes: HashSet<Entity>,
    pending_children_changes: HashSet<Entity>,
    skipped_frames: u64,
    rebuilt_frames: u64,
    repainted_entities: usize,
    recolored_fragments: usize,
    published_fragments: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckboxPaintStaticStamp {
    checkbox: Tick,
    layout: Tick,
    theme: Tick,
    effective_enabled: bool,
    order: u64,
    camera: Option<Entity>,
}

#[derive(Resource, Debug, Default)]
struct WidgetPaintSummaryGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
}

#[derive(Resource, Debug, Default)]
struct WidgetFileBrowserItems {
    items: HashMap<Entity, HashMap<PathBuf, Entity>>,
    stamps: HashMap<Entity, FileBrowserVirtualStamp>,
}

#[derive(Resource, Debug, Default)]
struct WidgetFileBrowserFilterEdits {
    active: HashMap<Entity, String>,
    redo: HashMap<Entity, String>,
}

#[derive(Resource, Debug, Default)]
struct WidgetCollectionItems {
    items: HashMap<Entity, HashMap<u64, Entity>>,
    stamps: HashMap<Entity, CollectionVirtualStamp>,
}

#[derive(Resource, Debug, Default)]
struct WidgetControlOptionItems {
    dropdowns: HashMap<Entity, Vec<Entity>>,
    radios: HashMap<Entity, Vec<Entity>>,
    dropdown_stamps: HashMap<Entity, ControlOptionStamp>,
    radio_stamps: HashMap<Entity, ControlOptionStamp>,
}

#[derive(Resource, Debug, Default)]
struct WidgetMenuPopupStamps {
    stamps: HashMap<Entity, MenuPopupSyncStamp>,
}

#[derive(Resource, Default)]
struct WidgetMenuPopupGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
}

#[derive(Resource, Debug, Default)]
struct WidgetPanelSyncCache {
    stamps: HashMap<Entity, PanelSyncStamp>,
    clips: HashMap<Entity, PanelDescendantClip>,
}

#[derive(Resource, Debug, Default)]
struct WidgetPanelSyncInvalidations {
    panels: HashSet<Entity>,
    topology_changed: bool,
}

#[derive(Resource, Default)]
struct WidgetPanelSyncGate {
    component: Option<ComponentId>,
    last: Tick,
    initialized: bool,
}

#[derive(Resource, Default)]
struct WidgetPanelInvalidationGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
}

#[derive(Resource, Default)]
struct WidgetPanelAnimationGate {
    component: Option<ComponentId>,
    last: Tick,
    initialized: bool,
    active: HashSet<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DataTableItemKey {
    Header(usize),
    Cell { row_id: u64, column: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualViewStamp {
    model_changed: u32,
    layout_changed: Option<u32>,
    theme_changed: Option<u32>,
    style_changed: Option<u32>,
    key_changed: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileBrowserVirtualStamp {
    model_changed: u32,
    layout_changed: Option<u32>,
    view_changed: Option<u32>,
    theme_changed: Option<u32>,
    key_changed: Option<u32>,
    visible_changed: Option<u32>,
    enabled_changed: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CollectionVirtualStamp {
    kind: u8,
    stamp: VirtualViewStamp,
    visible_changed: Option<u32>,
    enabled_changed: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ControlOptionStamp {
    kind: u8,
    model_changed: u32,
    layout_changed: Option<u32>,
    popup_changed: Option<u32>,
    theme_changed: Option<u32>,
    viewport_changed: Option<u32>,
    viewport: Rect,
    owner_visible: bool,
    owner_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct MenuPopupSyncStamp {
    menu_changed: u32,
    layout_changed: u32,
    viewport_changed: Option<u32>,
    viewport: Rect,
    theme_changed: Option<u32>,
    owner_visible: bool,
    owner_enabled: bool,
    item_ticks: HashMap<Entity, Option<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PanelSyncStamp {
    panel_changed: u32,
    children_changed: Option<u32>,
    layout_changed: Option<u32>,
    theme_changed: Option<u32>,
    progress_bits: u32,
}

#[derive(Debug, Default, Clone, Copy)]
struct PanelDescendantClip {
    base_layout: Option<Rect>,
    applied_layout: Option<Option<Rect>>,
    base_hit: Option<UiRect>,
    applied_hit: Option<Option<UiRect>>,
}

#[derive(Resource, Debug, Default)]
struct WidgetDataTableItems {
    items: HashMap<Entity, HashMap<DataTableItemKey, Entity>>,
    stamps: HashMap<Entity, VirtualViewStamp>,
    viewport_ticks: HashMap<Entity, u32>,
    active_row_positions: HashMap<Entity, Option<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NodeGraphItemKey {
    Node(NodeId),
    Socket(NodeId, usize),
    Link(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeGraphNodeGeometrySignature {
    position: [u32; 2],
    size: [u32; 2],
    collapsed: bool,
    socket_layout: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeGraphNodeSignature {
    paint: u64,
    geometry: NodeGraphNodeGeometrySignature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeGraphLinkSignature {
    from_node: NodeId,
    from_socket: usize,
    to_node: NodeId,
    to_socket: usize,
    from_geometry: Option<NodeGraphNodeGeometrySignature>,
    to_geometry: Option<NodeGraphNodeGeometrySignature>,
    source_socket_type: Option<SocketType>,
    selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeGraphTransformSignature {
    pan: [u32; 2],
    zoom: u32,
}

struct NodeGraphSignatureSnapshot {
    node_indices: HashMap<NodeId, usize>,
    node_signatures: HashMap<NodeId, NodeGraphNodeSignature>,
    node_order: Vec<NodeId>,
    link_indices: HashMap<u64, usize>,
    link_signatures: HashMap<u64, NodeGraphLinkSignature>,
    link_order: Vec<u64>,
    incident_links: HashMap<NodeId, Vec<u64>>,
    transform: NodeGraphTransformSignature,
}

type GraphSpatialCells = HashMap<(u8, i32, i32), Vec<NodeGraphItemKey>>;
type GraphSpatialMembership = HashMap<NodeGraphItemKey, Vec<(u8, i32, i32)>>;

#[derive(Resource, Debug, Default)]
struct WidgetNodeGraphItems {
    items: HashMap<Entity, HashMap<NodeGraphItemKey, Entity>>,
    stamps: HashMap<Entity, VirtualViewStamp>,
    node_indices: HashMap<Entity, HashMap<NodeId, usize>>,
    link_indices: HashMap<Entity, HashMap<u64, usize>>,
    incident_links: HashMap<Entity, HashMap<NodeId, Vec<u64>>>,
    node_signatures: HashMap<Entity, HashMap<NodeId, NodeGraphNodeSignature>>,
    node_order: HashMap<Entity, Vec<NodeId>>,
    link_signatures: HashMap<Entity, HashMap<u64, NodeGraphLinkSignature>>,
    link_order: HashMap<Entity, Vec<u64>>,
    transforms: HashMap<Entity, NodeGraphTransformSignature>,
    journaled_ticks: HashMap<Entity, u32>,
    viewport_ticks: HashMap<Entity, u32>,
    spatial_cells: HashMap<Entity, GraphSpatialCells>,
    spatial_overflow: HashMap<Entity, Vec<NodeGraphItemKey>>,
    spatial_membership: HashMap<Entity, GraphSpatialMembership>,
    spatial_levels: HashMap<Entity, HashSet<u8>>,
}

#[derive(Resource, Debug, Default)]
struct WidgetNodeGraphDeltas(HashMap<Entity, HashSet<NodeId>>, HashSet<Entity>);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
struct NodeGraphLinkPaintGeometry {
    points: [[u32; 2]; 4],
    source_socket_type: Option<SocketType>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
struct NodeGraphNodePaintState {
    signature: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TooltipHover {
    trigger: Entity,
    owner: Entity,
    elapsed_seconds: f32,
}

#[derive(Resource, Debug, Default)]
struct WidgetTooltips(HashMap<Entity, TooltipHover>);

#[derive(Debug, Clone, Copy, PartialEq)]
struct MenuAnimationState {
    /// Raw transition progress from the trigger bounds to the popup bounds.
    progress: f32,
    target_open: bool,
    anchor: Rect,
    popup: Rect,
}

#[derive(Resource, Debug, Default, Clone, PartialEq)]
struct WidgetMenuAnimations(HashMap<Entity, MenuAnimationState>);

#[derive(Resource, Default)]
struct WidgetMenuAnimationGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
    active: HashSet<Entity>,
}

struct NodeGraphViewItem {
    key: NodeGraphItemKey,
    node: Option<NodeGraphNodeControl>,
    node_paint_state: Option<NodeGraphNodePaintState>,
    socket: Option<NodeGraphSocketControl>,
    link: Option<NodeGraphLinkControl>,
    link_geometry: Option<NodeGraphLinkPaintGeometry>,
    rect: Rect,
    clip: Rect,
    z: i32,
}

struct NodeGraphView {
    owner: Entity,
    items: Vec<NodeGraphViewItem>,
    replaced_keys: HashSet<NodeGraphItemKey>,
    replace_all: bool,
}

struct DataTableViewItem {
    key: DataTableItemKey,
    header: Option<DataTableHeaderControl>,
    cell: Option<DataTableCellControl>,
    rect: Rect,
    clip: Rect,
}

struct DataTableView {
    owner: Entity,
    items: Vec<DataTableViewItem>,
}

struct CollectionView {
    owner: Entity,
    stamp: CollectionVirtualStamp,
    items: Vec<(CollectionItemControl, Rect)>,
}

mod install;
pub use install::install_widget_systems;
use install::*;

/// Handle routed pointer/key/text/IME events and update Bevy widget components.
mod events;
#[cfg(test)]
use events::advance_tooltips_by;
pub use events::{advance_tooltips, process_routed_widget_events};

mod color_ramp;
use color_ramp::*;

mod hsv_color;
use hsv_color::*;

mod curve_editor;
use curve_editor::*;

mod data_table;
pub use data_table::set_data_table_viewport;
use data_table::*;

mod collections;
use collections::*;

mod menus;
pub(crate) use menus::refresh_text_edit_draft_after_history_restore;
use menus::*;

mod file_click;
use file_click::*;

mod options;
use options::*;

mod file_browser;
use file_browser::*;
pub(crate) use file_browser::{redo_file_browser_filter_draft, undo_file_browser_filter_draft};

mod numeric_gestures;
use numeric_gestures::*;

mod keyboard;
use keyboard::*;

mod apply;
pub use apply::apply_widget_actions;
use apply::*;

mod graph_geometry;
use graph_geometry::*;

mod graph_math;
use graph_math::*;

mod node_graph;
use node_graph::*;
pub use node_graph::{move_node_graph_node, pan_node_graph};

mod panels;
use panels::*;
pub use panels::{advance_menu_animations, advance_panel_animations, sync_panel_children};

mod menu_sync;
pub use menu_sync::sync_menu_popups;

mod option_sync;
pub use option_sync::sync_control_option_views;
use option_sync::*;

mod file_sync;
pub use file_sync::sync_file_browser_items;

mod collection_sync;
pub use collection_sync::sync_collection_views;
use collection_sync::*;

mod table_sync;
pub use table_sync::sync_data_table_views;

mod graph_sync;
pub use graph_sync::sync_node_graph_views;

mod text_sync;
pub use text_sync::sync_text_controls;
use text_sync::*;

mod paint;
pub use paint::collect_widget_paint;
use paint::*;

#[cfg(test)]
#[path = "actions/tests.rs"]
mod tests;

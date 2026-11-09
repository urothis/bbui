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
    SocketDirection, TextArea, TextField, Tooltip, TreeModel,
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
};
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiWheelHandled};
use bui_render::{
    ClipRect, DisplayList, DrawItem, Primitive, TextRenderStyle, WidgetRenderFragment,
};
use bui_style::{
    Rgba, Theme, ThemeColorToken, ThemeMotionToken, UiClasses, UiComputedStyle, UiInlineStyle,
    UiRole, UiStyleProperty, UiStyleSheet, UiStyleState, UiStyleValue, set_ui_style_state,
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
    input_stamp: Option<u64>,
    prepared: bool,
    focus_active_window: Option<Entity>,
    focus_targets: Vec<FocusTargetSnapshot>,
    interaction_targets: HashSet<Entity>,
    fragments: HashSet<Entity>,
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
    published_fragments: usize,
}

#[derive(Resource, Debug, Default)]
struct WidgetFileBrowserItems(HashMap<Entity, HashMap<PathBuf, Entity>>);

#[derive(Resource, Debug, Default)]
struct WidgetFileBrowserFilterEdits {
    active: HashMap<Entity, String>,
    redo: HashMap<Entity, String>,
}

#[derive(Resource, Debug, Default)]
struct WidgetCollectionItems(HashMap<Entity, HashMap<u64, Entity>>);

#[derive(Resource, Debug, Default)]
struct WidgetControlOptionItems {
    dropdowns: HashMap<Entity, Vec<Entity>>,
    radios: HashMap<Entity, Vec<Entity>>,
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
    key_changed: Option<u32>,
}

#[derive(Resource, Debug, Default)]
struct WidgetDataTableItems {
    items: HashMap<Entity, HashMap<DataTableItemKey, Entity>>,
    stamps: HashMap<Entity, VirtualViewStamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NodeGraphItemKey {
    Node(NodeId),
    Socket(NodeId, usize),
    Link(u64),
}

#[derive(Resource, Debug, Default)]
struct WidgetNodeGraphItems {
    items: HashMap<Entity, HashMap<NodeGraphItemKey, Entity>>,
    stamps: HashMap<Entity, VirtualViewStamp>,
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

struct NodeGraphViewItem {
    key: NodeGraphItemKey,
    node: Option<NodeGraphNodeControl>,
    socket: Option<NodeGraphSocketControl>,
    link: Option<NodeGraphLinkControl>,
    rect: Rect,
    clip: Rect,
    z: i32,
}

struct NodeGraphView {
    owner: Entity,
    items: Vec<NodeGraphViewItem>,
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
    items: Vec<(CollectionItemControl, Rect)>,
}

/// Installs the routed input bridge in the ordered widget phase.
pub fn install_widget_systems(app: &mut App) {
    app.init_resource::<WidgetActionQueue>()
        .init_resource::<WidgetEventQueue>()
        .init_resource::<ClipboardRequestQueue>()
        .init_resource::<CursorCaptureRequestQueue>()
        .init_resource::<WidgetInteractionState>()
        .init_resource::<WidgetPaintCache>()
        .init_resource::<UiDirty>()
        .init_resource::<WidgetFileBrowserItems>()
        .init_resource::<WidgetFileBrowserFilterEdits>()
        .init_resource::<WidgetCollectionItems>()
        .init_resource::<WidgetControlOptionItems>()
        .init_resource::<WidgetDataTableItems>()
        .init_resource::<WidgetNodeGraphItems>()
        .init_resource::<WidgetTooltips>()
        .init_resource::<WidgetMenuAnimations>()
        .init_resource::<UiRoutedEvents>()
        .init_resource::<DisplayList>()
        .init_resource::<Theme>()
        .add_systems(
            Update,
            capture_focused_text_edit_drafts
                .in_set(UiSystems::Text)
                .before(bui_text::process_text_events),
        )
        .add_systems(
            Update,
            (
                process_routed_widget_events,
                advance_tooltips,
                apply_widget_actions,
                advance_panel_animations,
                sync_panel_children,
                sync_menu_popups,
                advance_menu_animations,
                sync_control_option_views,
                sync_file_browser_items,
                sync_collection_views,
                sync_data_table_views,
                sync_node_graph_views,
                sync_text_controls,
                refresh_text_layouts_after_sync,
                collect_widget_paint_changes,
                collect_widget_paint,
            )
                .chain()
                .in_set(UiSystems::Widgets),
        );
    macro_rules! register_paint_removals {
        ($($component:ty),+ $(,)?) => {
            $(app.add_observer(mark_paint_component_removed::<$component>);)+
        };
    }
    register_paint_removals!(
        UiNode,
        UiLayoutResult,
        UiComputedStyle,
        UiRole,
        UiTextLayout,
        TextBuffer,
        UiFocusable,
        UiLiveRegion,
        UiReadOnly,
        UiKey,
        Button,
        Checkbox,
        IconButton,
        Label,
        RadioGroup,
        NumericField,
        Slider,
        TextField,
        TextArea,
        Dropdown,
        Panel,
        Progress,
        MenuControl,
        MenuItemControl,
        DropdownOptionControl,
        FileBrowser,
        FileBrowserItem,
        ListModel<String>,
        TreeModel<String>,
        PreviewGrid,
        CollectionItemControl,
        DataTable,
        DataTableHeaderControl,
        DataTableCellControl,
        ColorRamp,
        HsvColorEditor,
        CurveEditor,
        NodeGraph,
        Canvas,
        Tooltip,
        IconButtonAsset,
        DropdownPopupState,
        CollectionEditorState,
        HsvColorEditorState,
        ColorRampEditorState,
        NodeGraphEditorState,
        FileBrowserViewState,
        crate::canvas::CanvasObjectControl,
        crate::IconButtonAssetState,
        NodeGraphNodeControl,
        NodeGraphSocketControl,
        NodeGraphLinkControl,
        crate::PanelGroup,
        RadioOptionControl,
    );
    app.add_observer(mark_widget_parent_removed);
    app.add_observer(mark_widget_children_removed);
    app.add_observer(mark_widget_layout_root_removed);
    app.add_observer(remove_widget_paint_layout_rank);
    app.add_observer(prune_widget_paint_root);
    app.add_observer(mark_inherited_paint_component_removed::<UiEnabled>);
    app.add_observer(mark_inherited_paint_component_removed::<UiVisible>);
    app.add_observer(mark_widget_paint_camera_added);
    app.add_observer(mark_widget_paint_camera_removed);
}

fn mark_paint_component_removed<T: Component>(on: On<Remove<T>>, mut dirty: ResMut<UiDirty>) {
    dirty.mark(UiDirtyDomain::Paint, on.entity);
    dirty.mark(UiDirtyDomain::Accessibility, on.entity);
}

fn mark_inherited_paint_component_removed<T: Component>(
    on: On<Remove<T>>,
    children: Query<'_, '_, &'static UiChildren>,
    mut dirty: ResMut<UiDirty>,
) {
    mark_widget_paint_subtree(on.entity, &children, &mut dirty);
}

fn mark_widget_parent_removed(
    on: On<Remove<UiParent>>,
    children: Query<'_, '_, &'static UiChildren>,
    mut dirty: ResMut<UiDirty>,
    mut cache: ResMut<WidgetPaintCache>,
) {
    cache.pending_parent_changes.insert(on.entity);
    mark_widget_paint_subtree(on.entity, &children, &mut dirty);
}

#[expect(
    clippy::type_complexity,
    reason = "the observer checks the remaining hierarchy component set"
)]
fn mark_widget_children_removed(
    on: On<Remove<UiChildren>>,
    children: Query<'_, '_, &'static UiChildren>,
    remaining: Query<
        Entity,
        Or<(
            With<UiNode>,
            With<UiParent>,
            With<UiChildren>,
            With<UiLayoutResult>,
        )>,
    >,
    mut dirty: ResMut<UiDirty>,
    mut cache: ResMut<WidgetPaintCache>,
) {
    cache.pending_children_changes.insert(on.entity);
    dirty.mark(UiDirtyDomain::Paint, on.entity);
    dirty.mark(UiDirtyDomain::Accessibility, on.entity);
    if let Some(previous_children) = cache.paint_children.get(&on.entity) {
        for child in previous_children {
            mark_widget_paint_subtree(*child, &children, &mut dirty);
        }
    }
    if remaining.get(on.entity).is_err() {
        prune_widget_order_cache(on.entity, &mut cache);
    }
}

fn mark_widget_layout_root_removed(
    on: On<Remove<UiLayoutRoot>>,
    children: Query<'_, '_, &'static UiChildren>,
    mut dirty: ResMut<UiDirty>,
) {
    mark_widget_paint_subtree(on.entity, &children, &mut dirty);
}

fn remove_widget_paint_layout_rank(
    on: On<Remove<UiLayoutResult>>,
    mut cache: ResMut<WidgetPaintCache>,
) {
    if let Some(rank) = cache.paint_ranks.remove(&on.entity) {
        cache.paint_order_keys.remove(&rank);
    }
}

#[expect(
    clippy::type_complexity,
    reason = "the observer checks the remaining hierarchy component set"
)]
fn prune_widget_paint_root(
    on: On<Remove<UiNode>>,
    remaining: Query<
        Entity,
        Or<(
            With<UiNode>,
            With<UiParent>,
            With<UiChildren>,
            With<UiLayoutResult>,
        )>,
    >,
    mut cache: ResMut<WidgetPaintCache>,
) {
    if remaining.get(on.entity).is_ok() {
        return;
    }
    prune_widget_order_cache(on.entity, &mut cache);
}

fn prune_widget_order_cache(entity: Entity, cache: &mut WidgetPaintCache) {
    if cache.paint_roots.get(&entity.index_u32()) == Some(&entity) {
        cache.paint_roots.remove(&entity.index_u32());
    }
    if let Some(rank) = cache.paint_ranks.remove(&entity) {
        cache.paint_order_keys.remove(&rank);
    }
}

fn mark_widget_paint_subtree(
    root: Entity,
    children: &Query<'_, '_, &'static UiChildren>,
    dirty: &mut UiDirty,
) {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            continue;
        }
        dirty.mark(UiDirtyDomain::Paint, entity);
        dirty.mark(UiDirtyDomain::Accessibility, entity);
        if let Ok(children) = children.get(entity) {
            pending.extend(children.0.iter().copied());
        }
    }
}

fn widget_paint_subtree(world: &World, root: Entity) -> HashSet<Entity> {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            continue;
        }
        if let Some(children) = world.get::<UiChildren>(entity) {
            pending.extend(children.0.iter().copied());
        }
    }
    visited
}

fn mark_menu_paint_subtree(world: &mut World, owner: Entity) {
    let mut affected = widget_paint_subtree(world, owner);
    if let Some(menu) = world.get::<MenuControl>(owner) {
        for item in &menu.items {
            affected.extend(widget_paint_subtree(world, *item));
        }
    }
    if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
        for entity in affected {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
}

fn mark_widget_paint_camera_removed(
    on: On<Remove<bevy_camera::Camera>>,
    layouts: Query<(Entity, &UiLayoutResult)>,
    mut dirty: ResMut<UiDirty>,
) {
    for (entity, layout) in &layouts {
        if layout.window == on.entity {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
}

fn mark_widget_paint_camera_added(
    on: On<Add<bevy_camera::Camera>>,
    layouts: Query<(Entity, &UiLayoutResult)>,
    mut dirty: ResMut<UiDirty>,
) {
    for (entity, layout) in &layouts {
        if layout.window == on.entity {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
}

/// Recompute text geometry after widget synchronization and before paint.
///
/// Text editing/layout normally runs in `UiSystems::Text`, before widget
/// systems. `sync_text_controls` can update a text layout's label prefix,
/// placeholder, or wrapping mode, so refresh its cached geometry here to keep
/// same-frame paint, caret hit testing, and scrolling in agreement.
fn refresh_text_layouts_after_sync(world: &mut World) {
    if world.contains_resource::<bevy_text::TextPipeline>() {
        bui_text::collect_text_widget_changes(world);
        bui_text::update_text_layouts_if_dirty(world);
    }
}

/// Handle routed pointer/key/text/IME events and update Bevy widget components.
pub fn process_routed_widget_events(world: &mut World) {
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    let mut suppress_checkbox_clicks = HashSet::new();
    let mut suppress_menu_clicks = HashSet::new();
    let mut suppress_color_ramp_clicks = HashSet::new();
    let mut suppress_hsv_color_clicks = HashSet::new();
    let mut suppress_collection_clicks = HashSet::new();
    let mut suppress_data_table_clicks = HashSet::new();
    let mut suppress_panel_clicks = HashSet::new();
    let mut emitted = Vec::new();

    for event in events {
        if process_node_graph_event(world, &event, &mut emitted) {
            continue;
        }
        match event {
            UiRoutedEvent::PointerEnter { window, target } => {
                set_tooltip_hover(world, window, Some(target));
                let position = world
                    .get_resource::<UiFocus>()
                    .and_then(|focus| focus.state(window))
                    .and_then(|state| state.pointer_position)
                    .unwrap_or([0.0, 0.0]);
                update_menu_pointer(world, target, window, position, false);
                update_dropdown_pointer(world, target);
            }
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position,
                dragging: false,
                ..
            } => {
                set_tooltip_hover(world, window, Some(target));
                update_menu_pointer(world, target, window, position, false);
                update_dropdown_pointer(world, target);
            }
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
            } => {
                let modifiers = world
                    .resource::<WidgetInteractionState>()
                    .modifiers
                    .get(&window)
                    .copied()
                    .unwrap_or_default();
                place_text_caret(world, target, position, modifiers.shift);
                begin_color_ramp_pointer(world, window, target, position);
                begin_hsv_color_pointer(world, window, target, position);
                begin_curve_editor_pointer(world, window, target, position, modifiers);
                begin_collection_drag(world, window, target, position);
                begin_data_table_resize(world, window, target, position);
                begin_panel_reorder(world, window, target, position);
                if let Some(item) = world.get::<MenuItemControl>(target).cloned()
                    && let Some(mut menu) = world.get_mut::<MenuControl>(item.owner)
                {
                    menu.gesture.press(Some(item.index));
                    select_menu_item(&mut menu, &item);
                }
                if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                    && menu.display != MenuDisplay::Context
                {
                    menu.press_position = Some(position);
                }
                let paint = if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
                    PaintTransaction::begin(target, &mut checkbox)
                } else {
                    None
                };
                if let Some(transaction) = paint {
                    world
                        .resource_mut::<WidgetInteractionState>()
                        .checkbox_paints
                        .insert(window, transaction);
                }
                begin_numeric_scrub(world, window, target, position);
                begin_slider_drag(world, window, target, position);
            }
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Secondary,
                position,
            } => {
                close_menus_outside(world, window, target);
                let focus_restore_key = world.get::<UiKey>(target).map(|key| key.0.clone());
                let opened = if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                    && menu.display == MenuDisplay::Context
                {
                    menu.open_at(position, focus_restore_key);
                    menu.popup.open = true;
                    true
                } else {
                    false
                };
                if opened {
                    sync_menu_popups(world);
                }
            }
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Middle,
                position,
            } if world.get::<CurveEditor>(target).is_some() => {
                begin_curve_editor_pan(world, window, target, position);
            }
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position,
                delta,
                dragging,
                ..
            } if dragging => {
                update_menu_pointer(world, target, window, position, true);
                let modifiers = world
                    .resource::<WidgetInteractionState>()
                    .modifiers
                    .get(&window)
                    .copied()
                    .unwrap_or_default();
                place_text_caret(world, target, position, modifiers.shift);
                update_color_ramp_drag(world, window, position);
                update_hsv_color_drag(world, window, position);
                update_curve_editor_drag(world, window, position, delta);
                update_collection_drag(world, window, position);
                update_data_table_resize(world, window, position);
                update_panel_reorder(world, window, position);
                apply_checkbox_paint_at(world, window, position);
                let modifiers = world
                    .resource::<WidgetInteractionState>()
                    .modifiers
                    .get(&window)
                    .copied()
                    .unwrap_or_default();
                update_numeric_scrub(world, window, position, delta[0], modifiers);
                update_slider_drag(world, window, position[0]);
            }
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
                cancelled,
                ..
            } => {
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .color_ramp_drags
                    .remove(&window)
                {
                    suppress_color_ramp_clicks.insert(drag.target);
                    finish_color_ramp_drag(world, drag, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .hsv_color_drags
                    .remove(&window)
                {
                    suppress_hsv_color_clicks.insert(drag.target);
                    finish_hsv_color_drag(world, drag, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .curve_editor_drags
                    .remove(&window)
                {
                    finish_curve_editor_drag(world, drag, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .collection_drags
                    .remove(&window)
                {
                    if drag.dragging {
                        suppress_collection_clicks.insert(drag.source);
                    }
                    finish_collection_drag(world, drag, position, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .data_table_resize_drags
                    .remove(&window)
                {
                    if drag.changed {
                        suppress_data_table_clicks.insert(drag.source);
                        suppress_data_table_clicks.insert(target);
                    }
                    finish_data_table_resize(world, drag, cancelled, &mut emitted);
                }
                if let Some(gesture) = world
                    .resource_mut::<WidgetInteractionState>()
                    .panel_reorders
                    .remove(&window)
                {
                    let moved = gesture.current != gesture.before;
                    if moved {
                        suppress_panel_clicks.insert(gesture.source);
                    }
                    finish_panel_reorder(world, gesture, cancelled, &mut emitted);
                }
                if let Some(item) = world.get::<MenuItemControl>(target).cloned() {
                    if cancelled {
                        if let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
                            menu.gesture.cancel();
                        }
                    } else {
                        let released = menu_item_at_position(world, item.owner, window, position);
                        if let Some(released) = released {
                            select_menu_item_for_release(world, &released, &mut emitted);
                            suppress_menu_clicks.insert(target);
                        }
                    }
                } else if world.get::<MenuControl>(target).is_some() {
                    let (opened_by_pointer, display) = {
                        let Some(mut menu) = world.get_mut::<MenuControl>(target) else {
                            continue;
                        };
                        let opened_by_pointer = menu.opened_by_pointer;
                        let display = menu.display;
                        menu.press_position = None;
                        menu.opened_by_pointer = false;
                        (opened_by_pointer, display)
                    };
                    if opened_by_pointer {
                        if cancelled {
                            close_menu_and_restore(world, target);
                        } else {
                            let released = if display == MenuDisplay::Pie {
                                update_pie_menu_pointer(world, target, position);
                                selected_menu_item(world, target)
                            } else {
                                menu_item_at_position(world, target, window, position)
                            };
                            if let Some(released) = released {
                                select_menu_item_for_release(world, &released, &mut emitted);
                            } else {
                                close_menu_and_restore(world, target);
                            }
                        }
                        suppress_menu_clicks.insert(target);
                    }
                }
                let paint = {
                    world
                        .resource_mut::<WidgetInteractionState>()
                        .checkbox_paints
                        .remove(&window)
                };
                if let Some(paint) = paint {
                    if cancelled {
                        restore_checkboxes(world, &paint.changes);
                        emitted.push(WidgetEvent::CheckboxPaintCancelled {
                            changes: paint.changes,
                        });
                    } else {
                        suppress_checkbox_clicks.insert(target);
                        if !paint.changes.is_empty() {
                            emitted.push(WidgetEvent::CheckboxPaintCommitted {
                                changes: paint.changes,
                            });
                        }
                    }
                }
                let scrub = {
                    world
                        .resource_mut::<WidgetInteractionState>()
                        .numeric_scrubs
                        .remove(&window)
                };
                if let Some(scrub) = scrub {
                    finish_numeric_scrub(world, window, scrub, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .slider_drags
                    .remove(&window)
                {
                    finish_slider_drag(world, drag, cancelled, &mut emitted);
                }
            }
            UiRoutedEvent::PointerUp {
                window,
                button: UiPointerButton::Middle,
                cancelled,
                ..
            } => {
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .curve_editor_drags
                    .remove(&window)
                {
                    finish_curve_editor_drag(world, drag, cancelled, &mut emitted);
                }
            }
            UiRoutedEvent::PointerCancel { window, .. } => {
                cancel_gestures(world, window, &mut emitted)
            }
            UiRoutedEvent::PointerLeave { window, target } => {
                clear_tooltip_hover(world, window, target);
            }
            UiRoutedEvent::Click {
                window,
                target,
                button: UiPointerButton::Primary,
                click_count,
                position,
            } => {
                if suppress_checkbox_clicks.remove(&target) {
                    continue;
                }
                if suppress_menu_clicks.remove(&target) {
                    continue;
                }
                if suppress_color_ramp_clicks.remove(&target) {
                    continue;
                }
                if suppress_hsv_color_clicks.remove(&target) {
                    continue;
                }
                if suppress_collection_clicks.remove(&target) {
                    continue;
                }
                if suppress_data_table_clicks.remove(&target) {
                    continue;
                }
                if suppress_panel_clicks.remove(&target) {
                    continue;
                }
                close_dropdowns_outside(world, window, target);
                if let Some(header) = world.get::<DataTableHeaderControl>(target).cloned() {
                    edit_data_table(world, header.owner, &mut emitted, |table| {
                        let column_changed = table.set_active_column(header.column);
                        let sorted = table.toggle_sort(header.column);
                        column_changed || sorted
                    });
                } else if let Some(cell) = world.get::<DataTableCellControl>(target).cloned() {
                    let modifiers = world
                        .resource::<WidgetInteractionState>()
                        .modifiers
                        .get(&window)
                        .copied()
                        .unwrap_or_default();
                    select_data_table_cell(
                        world,
                        &cell,
                        modifiers.control || modifiers.super_key,
                        modifiers.shift,
                        click_count,
                        &mut emitted,
                    );
                } else if let Some(item) = world.get::<MenuItemControl>(target).cloned() {
                    select_menu_item_for_release(world, &item, &mut emitted);
                } else if let Some(option) = world.get::<DropdownOptionControl>(target).cloned() {
                    select_dropdown_option(world, option.owner, option.index, &mut emitted);
                } else if let Some(option) = world.get::<RadioOptionControl>(target).cloned() {
                    select_radio_option(world, option.owner, option.index, &mut emitted);
                } else if let Some(item) = world.get::<FileBrowserItem>(target).cloned() {
                    select_file_browser_item(world, &item, click_count, window, &mut emitted);
                } else if let Some(item) = world.get::<CollectionItemControl>(target).cloned() {
                    let modifiers = world
                        .resource::<WidgetInteractionState>()
                        .modifiers
                        .get(&window)
                        .copied()
                        .unwrap_or_default();
                    click_collection_item(
                        world,
                        &item,
                        position,
                        click_count,
                        modifiers,
                        &mut emitted,
                    );
                } else if world.get::<MenuControl>(target).is_some() {
                    close_menus_outside(world, window, target);
                    if world
                        .get::<MenuControl>(target)
                        .is_some_and(|menu| menu.display != MenuDisplay::Context)
                    {
                        toggle_menu(world, target);
                    }
                } else if world.get::<FileBrowser>(target).is_some() {
                    close_menus_outside(world, window, target);
                    click_file_browser(world, target, position, click_count, window, &mut emitted);
                } else if world.get::<ColorRamp>(target).is_some() {
                    click_color_ramp(world, target, position, click_count, &mut emitted);
                } else if world.get::<HsvColorEditor>(target).is_some() {
                    click_hsv_color_editor(world, target, position);
                } else if world.get::<RadioGroup>(target).is_some() {
                    click_radio_group(world, target, position, &mut emitted);
                } else {
                    close_menus_outside(world, window, target);
                    if world.get::<Panel>(target).is_some() {
                        if panel_header_contains(world, target, position) {
                            let modifiers = world
                                .resource::<WidgetInteractionState>()
                                .modifiers
                                .get(&window)
                                .copied()
                                .unwrap_or_default();
                            if modifiers.control {
                                toggle_exclusive_panel(world, target, &mut emitted);
                            } else {
                                activate_target(world, target, click_count, &mut emitted);
                            }
                        }
                    } else {
                        activate_target(world, target, click_count, &mut emitted);
                    }
                }
            }
            UiRoutedEvent::Key {
                window,
                target,
                key,
                pressed,
                repeat: _,
                modifiers,
            } => {
                world
                    .resource_mut::<WidgetInteractionState>()
                    .modifiers
                    .insert(window, modifiers);
                if pressed
                    && key == UiKeyCode::Escape
                    && (cancel_panel_reorder(world, window, &mut emitted)
                        || cancel_numeric_scrub(world, window, &mut emitted)
                        || cancel_slider_drag(world, window, &mut emitted))
                {
                    continue;
                }
                if pressed && !handle_menu_key(world, target, key, modifiers, &mut emitted) {
                    handle_key(world, target, key, modifiers, &mut emitted);
                }
                if pressed
                    && key == UiKeyCode::Enter
                    && world.get::<TextField>(target).is_some()
                    && world.get::<TextArea>(target).is_none()
                {
                    finish_text_edit_on_focus_departure(world, target, &mut emitted);
                }
            }
            UiRoutedEvent::Wheel { target, delta, .. }
                if dropdown_owner(world, target).is_some() =>
            {
                let owner = dropdown_owner(world, target).unwrap_or(target);
                scroll_dropdown_options(world, owner, delta[1]);
            }
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                ..
            } if data_table_owner(world, target).is_some() => {
                scroll_data_table(world, target, delta, unit);
            }
            UiRoutedEvent::Wheel {
                target,
                position,
                delta,
                ..
            } if world.get::<CurveEditor>(target).is_some() => {
                zoom_curve_editor_at(world, target, position, delta[1], &mut emitted);
            }
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                ..
            } if collection_owner(world, target).is_some() => {
                scroll_collection(world, target, delta[1], unit);
            }
            UiRoutedEvent::Wheel {
                window,
                target,
                delta,
                modifiers,
                ..
            } if modifiers.control => {
                if let Some(mut slider) = world.get_mut::<Slider>(target) {
                    let before = slider.value;
                    let steps = if delta[1] > 0.0 {
                        1
                    } else if delta[1] < 0.0 {
                        -1
                    } else {
                        0
                    };
                    slider.step_by(steps);
                    if before != slider.value {
                        emitted.push(WidgetEvent::ValueChanged {
                            target,
                            before: WidgetValue::Number(before),
                            after: WidgetValue::Number(slider.value),
                        });
                    }
                } else if let Some(mut field) = world.get_mut::<NumericField>(target) {
                    let before = field.value;
                    let steps = if delta[1] > 0.0 {
                        1
                    } else if delta[1] < 0.0 {
                        -1
                    } else {
                        0
                    };
                    field.step_by(steps);
                    let after = field.value;
                    if before != after {
                        emitted.push(WidgetEvent::ValueChanged {
                            target,
                            before: WidgetValue::Number(before),
                            after: WidgetValue::Number(after),
                        });
                    }
                }
                world
                    .resource_mut::<WidgetInteractionState>()
                    .modifiers
                    .insert(window, modifiers);
            }
            UiRoutedEvent::Wheel { target, delta, .. }
                if world.get::<FileBrowser>(target).is_some() =>
            {
                scroll_file_browser(world, target, delta[1]);
            }
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                ..
            } if world.get::<TextArea>(target).is_some() => {
                scroll_text_area(world, target, delta[1], unit);
            }
            UiRoutedEvent::Text { target, text, .. }
            | UiRoutedEvent::ImeCommit { target, text, .. } => {
                if let Some(owner) = collection_owner(world, target)
                    && collection_is_renaming(world, owner)
                {
                    collection_rename_text(world, owner, &text);
                } else if world.get::<HsvColorEditor>(target).is_some() {
                    hsv_color_text_input(world, target, &text);
                } else if world.get::<MenuControl>(target).is_some() {
                    if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                        && menu.session.open
                    {
                        menu.query.push_str(&text);
                    }
                } else if world.get::<FileBrowser>(target).is_some() {
                    begin_file_browser_filter_edit(world, target);
                    if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                        let mut filter = browser.filter.clone();
                        filter.push_str(&text);
                        browser.set_filter(filter);
                    }
                    if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                        view.scroll_rows = 0;
                    }
                } else if let Some(buffer) = world.get::<TextBuffer>(target) {
                    emitted.push(WidgetEvent::TextChanged {
                        target,
                        text: buffer.as_str().to_owned(),
                    });
                } else {
                    insert_text(world, target, &text, &mut emitted);
                }
            }
            UiRoutedEvent::ImePreedit {
                target,
                text,
                cursor_range,
                ..
            } => emitted.push(WidgetEvent::ImePreedit {
                target,
                text,
                cursor_range,
            }),
            UiRoutedEvent::FocusChanged {
                window,
                previous,
                current: _,
            } => {
                if let Some(previous) = previous {
                    handle_widget_focus_departure(world, previous, &mut emitted);
                }
                world
                    .resource_mut::<WidgetInteractionState>()
                    .modifiers
                    .entry(window)
                    .or_default();
            }
            _ => {}
        }
    }
    {
        let mut requests = world.resource_mut::<ClipboardRequestQueue>();
        for event in &emitted {
            match event {
                WidgetEvent::ClipboardWrite { text, .. } => {
                    requests.push(ClipboardRequest::Write(text.clone()))
                }
                WidgetEvent::ClipboardRead { target } => {
                    requests.push(ClipboardRequest::Read { target: *target })
                }
                _ => {}
            }
        }
    }
    world.resource_mut::<WidgetEventQueue>().0.extend(emitted);
}

fn tooltip_host(world: &World, target: Entity) -> Option<Entity> {
    let mut current = Some(target);
    let mut visited = HashSet::new();
    while let Some(entity) = current {
        if !visited.insert(entity) {
            return None;
        }
        if world.get::<Tooltip>(entity).is_some() {
            return Some(entity);
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    None
}

fn set_tooltip_hover(world: &mut World, window: Entity, target: Option<Entity>) {
    let next = target.and_then(|trigger| {
        tooltip_host(world, trigger).map(|owner| TooltipHover {
            trigger,
            owner,
            elapsed_seconds: 0.0,
        })
    });
    let previous = world
        .get_resource::<WidgetTooltips>()
        .and_then(|tooltips| tooltips.0.get(&window).copied());
    if previous.is_some_and(|previous| {
        next.is_some_and(|next| previous.trigger == next.trigger && previous.owner == next.owner)
    }) {
        return;
    }
    if let Some(previous) = previous
        && let Some(mut tooltip) = world.get_mut::<Tooltip>(previous.owner)
    {
        tooltip.visible = false;
    }
    if let Some(next) = next {
        if let Some(mut tooltip) = world.get_mut::<Tooltip>(next.owner) {
            tooltip.visible = false;
        }
        world
            .resource_mut::<WidgetTooltips>()
            .0
            .insert(window, next);
    } else {
        world.resource_mut::<WidgetTooltips>().0.remove(&window);
    }
}

fn clear_tooltip_hover(world: &mut World, window: Entity, target: Entity) {
    let hover = world
        .get_resource::<WidgetTooltips>()
        .and_then(|tooltips| tooltips.0.get(&window).copied());
    let Some(hover) = hover else {
        return;
    };
    if hover.trigger != target && hover.owner != target {
        return;
    }
    world.resource_mut::<WidgetTooltips>().0.remove(&window);
    if let Some(mut tooltip) = world.get_mut::<Tooltip>(hover.owner) {
        tooltip.visible = false;
    }
}

/// Advance tooltip dwell timers using Bevy's frame delta.
pub fn advance_tooltips(world: &mut World) {
    let delta_seconds = world
        .get_resource::<bevy_time::Time>()
        .map_or(1.0 / 60.0, bevy_time::Time::delta_secs);
    advance_tooltips_by(world, delta_seconds);
}

fn advance_tooltips_by(world: &mut World, delta_seconds: f32) {
    if !delta_seconds.is_finite() || delta_seconds <= 0.0 {
        return;
    }
    let hovers = {
        let mut tooltips = world.resource_mut::<WidgetTooltips>();
        for hover in tooltips.0.values_mut() {
            hover.elapsed_seconds += delta_seconds;
        }
        tooltips.0.values().copied().collect::<Vec<_>>()
    };
    for hover in hovers {
        let should_show = world
            .get::<Tooltip>(hover.owner)
            .is_some_and(|tooltip| hover.elapsed_seconds * 1000.0 >= tooltip.delay_millis as f32);
        if should_show && let Some(mut tooltip) = world.get_mut::<Tooltip>(hover.owner) {
            tooltip.visible = true;
        }
    }
}

fn color_ramp_spacing(world: &World) -> bui_style::ThemeColorRampSpacing {
    world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.color_ramp,
        |theme| theme.spacing.color_ramp,
    )
}

fn color_ramp_track_rect(rect: Rect, spacing: bui_style::ThemeColorRampSpacing) -> Rect {
    Rect {
        x: rect.x + spacing.track_x,
        y: rect.y + spacing.track_y,
        width: (rect.width - spacing.track_width_inset).max(1.0),
        height: spacing.track_height,
    }
}

fn color_ramp_channel_rect(
    rect: Rect,
    index: usize,
    spacing: bui_style::ThemeColorRampSpacing,
) -> Rect {
    Rect {
        x: rect.x + spacing.channel_x,
        y: rect.y + spacing.channel_y + index as f32 * spacing.channel_step,
        width: (rect.width - spacing.channel_width_inset).max(1.0),
        height: spacing.channel_height,
    }
}

fn normalized_x(rect: Rect, x: f32) -> f32 {
    ((x - rect.x) / rect.width.max(1.0)).clamp(0.0, 1.0)
}

fn begin_color_ramp_pointer(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let (Some(ramp), Some(layout)) = (
        world.get::<ColorRamp>(target).cloned(),
        world.get::<UiLayoutResult>(target).copied(),
    ) else {
        return;
    };
    let rect = layout.rect;
    let spacing = color_ramp_spacing(world);
    let track = color_ramp_track_rect(rect, spacing);
    let marker_y = track.y + track.height;
    if position[1] >= marker_y && position[1] <= marker_y + spacing.marker_hit_extension {
        let nearest = ramp.stops.iter().min_by(|a, b| {
            let ax = track.x + a.position * track.width;
            let bx = track.x + b.position * track.width;
            (ax - position[0])
                .abs()
                .total_cmp(&(bx - position[0]).abs())
        });
        if let Some(stop) = nearest
            && (track.x + stop.position * track.width - position[0]).abs()
                <= spacing.marker_handle_radius * 2.0
        {
            let stop_id = stop.id;
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_stop(stop_id);
            }
            let before = world
                .get::<ColorRamp>(target)
                .cloned()
                .unwrap_or_else(|| ramp.clone());
            world
                .resource_mut::<WidgetInteractionState>()
                .color_ramp_drags
                .insert(
                    window,
                    ColorRampDrag {
                        target,
                        before,
                        kind: ColorRampDragKind::Stop(stop_id),
                    },
                );
            update_color_ramp_drag(world, window, position);
            return;
        }
    }
    let Some(index) = (0..ColorChannel::ALL.len()).find(|index| {
        let row = color_ramp_channel_rect(rect, *index, spacing);
        position[1] >= row.y - spacing.channel_selection_hit_slop
            && position[1] <= row.y + row.height + spacing.channel_selection_hit_slop
    }) else {
        return;
    };
    let channel = ColorChannel::from_index(index);
    if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
        state.active_channel = channel;
        state.editing_color = true;
    }
    let Some(stop) = ramp.selected else {
        return;
    };
    let row = color_ramp_channel_rect(rect, index, spacing);
    if position[0] < row.x - spacing.channel_label_hit_slop {
        return;
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .color_ramp_drags
        .insert(
            window,
            ColorRampDrag {
                target,
                before: ramp,
                kind: ColorRampDragKind::Channel { stop, channel },
            },
        );
    update_color_ramp_drag(world, window, position);
}

fn update_color_ramp_drag(world: &mut World, window: Entity, position: [f32; 2]) {
    let Some((target, kind)) = world
        .resource::<WidgetInteractionState>()
        .color_ramp_drags
        .get(&window)
        .map(|drag| (drag.target, drag.kind))
    else {
        return;
    };
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = color_ramp_spacing(world);
    let changed = if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
        match kind {
            ColorRampDragKind::Stop(id) => ramp.move_stop(
                id,
                normalized_x(color_ramp_track_rect(rect, spacing), position[0]),
            ),
            ColorRampDragKind::Channel { stop, channel } => {
                let row = color_ramp_channel_rect(rect, channel.index(), spacing);
                ramp.set_color_channel(stop, channel, normalized_x(row, position[0]))
            }
        }
    } else {
        false
    };
    if changed
        && let Some(mut state) = world.get_mut::<ColorRampEditorState>(target)
        && let ColorRampDragKind::Channel { channel, .. } = kind
    {
        state.active_channel = channel;
        state.editing_color = true;
    }
}

fn finish_color_ramp_drag(
    world: &mut World,
    drag: ColorRampDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Some(mut ramp) = world.get_mut::<ColorRamp>(drag.target) {
            *ramp = drag.before;
        }
        return;
    }
    let (Ok(before), Some(ramp)) = (drag.before.to_json(), world.get::<ColorRamp>(drag.target))
    else {
        return;
    };
    if let Ok(after) = ramp.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::ColorRampCommitted {
            target: drag.target,
            before,
            after,
        });
    }
}

fn edit_color_ramp(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut ColorRamp) -> bool,
) {
    let Some(before_ramp) = world.get::<ColorRamp>(target).cloned() else {
        return;
    };
    let Ok(before) = before_ramp.to_json() else {
        return;
    };
    let changed = world
        .get_mut::<ColorRamp>(target)
        .is_some_and(|mut ramp| edit(&mut ramp));
    if !changed {
        return;
    }
    if let Some(ramp) = world.get::<ColorRamp>(target)
        && let Ok(after) = ramp.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::ColorRampCommitted {
            target,
            before,
            after,
        });
    }
}

fn click_color_ramp(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    click_count: u32,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = color_ramp_spacing(world);
    let add_x = rect.x + rect.width - spacing.button_right_inset;
    let remove_x = add_x + spacing.button_step;
    let interpolation_x = remove_x + spacing.button_step;
    if position[1] >= rect.y && position[1] <= rect.y + spacing.header_hit_height {
        if position[0] >= add_x && position[0] < add_x + spacing.button_width {
            edit_color_ramp(world, target, emitted, |ramp| {
                let at = suggested_stop_position(ramp);
                let color = ramp.sample(at);
                ramp.add_stop(at, color).is_some()
            });
        } else if position[0] >= remove_x && position[0] < interpolation_x {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected.is_some_and(|id| ramp.remove_stop(id))
            });
        } else if position[0] >= interpolation_x {
            edit_color_ramp(world, target, emitted, |ramp| {
                let next = match ramp.interpolation {
                    Interpolation::Constant => Interpolation::Linear,
                    Interpolation::Linear => Interpolation::Smooth,
                    Interpolation::Smooth => Interpolation::Constant,
                };
                ramp.set_interpolation(next)
            });
        } else {
            if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
                state.editing_color = !state.editing_color;
            }
        }
        return;
    }
    let track = color_ramp_track_rect(rect, spacing);
    if position[1] >= track.y - spacing.track_hit_top_inset
        && position[1] <= track.y + track.height + spacing.marker_hit_extension
    {
        let at = normalized_x(track, position[0]);
        if click_count >= 2 {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.add_stop(at, ramp.sample(at)).is_some()
            });
        } else if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
            let nearest = ramp
                .stops
                .iter()
                .min_by(|a, b| (a.position - at).abs().total_cmp(&(b.position - at).abs()))
                .map(|stop| stop.id);
            if let Some(nearest) = nearest {
                ramp.select_stop(nearest);
            }
        }
        return;
    }
    for index in 0..ColorChannel::ALL.len() {
        let row = color_ramp_channel_rect(rect, index, spacing);
        if position[1] >= row.y - spacing.channel_click_hit_slop
            && position[1] <= row.y + row.height + spacing.channel_click_hit_slop
        {
            let channel = ColorChannel::from_index(index);
            if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
                state.active_channel = channel;
                state.editing_color = true;
            }
            if position[0] < row.x - spacing.channel_label_hit_slop {
                return;
            }
            let value = normalized_x(row, position[0]);
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected
                    .is_some_and(|id| ramp.set_color_channel(id, channel, value))
            });
            break;
        }
    }
}

fn suggested_stop_position(ramp: &ColorRamp) -> f32 {
    let Some(selected) = ramp.selected_stop() else {
        return 0.5;
    };
    let index = ramp
        .stops
        .iter()
        .position(|stop| stop.id == selected.id)
        .unwrap_or(0);
    if let Some(next) = ramp.stops.get(index + 1) {
        (selected.position + next.position) * 0.5
    } else if index > 0 {
        (ramp.stops[index - 1].position + selected.position) * 0.5
    } else {
        0.5
    }
}

fn hsv_spacing(world: &World) -> bui_style::ThemeHsvSpacing {
    world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().spacing.hsv, |theme| theme.spacing.hsv)
}

fn hsv_wheel_geometry(rect: Rect, spacing: bui_style::ThemeHsvSpacing) -> ([f32; 2], f32, f32) {
    (
        [
            rect.x + spacing.wheel_center_x,
            rect.y + spacing.wheel_center_y,
        ],
        spacing.wheel_outer_radius,
        spacing.wheel_inner_radius,
    )
}

fn hsv_value_rect(rect: Rect, spacing: bui_style::ThemeHsvSpacing) -> Rect {
    Rect {
        x: rect.x + spacing.controls_x,
        y: rect.y + spacing.value_track_y,
        width: (rect.width - spacing.value_track_width_inset).max(1.0),
        height: spacing.value_track_height,
    }
}

fn hsv_alpha_rect(rect: Rect, spacing: bui_style::ThemeHsvSpacing) -> Rect {
    Rect {
        x: rect.x + spacing.controls_x,
        y: rect.y + spacing.alpha_track_y,
        width: (rect.width - spacing.alpha_track_width_inset).max(1.0),
        height: spacing.value_track_height,
    }
}

fn hsv_field_rect(rect: Rect, field: HsvColorField, spacing: bui_style::ThemeHsvSpacing) -> Rect {
    let origin_x = rect.x + spacing.controls_x;
    let cell_width = spacing.field_cell_width;
    let gap = spacing.field_gap;
    match field {
        HsvColorField::Hue => Rect {
            x: origin_x,
            y: rect.y + spacing.field_first_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Saturation => Rect {
            x: origin_x + cell_width + gap,
            y: rect.y + spacing.field_first_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Value => Rect {
            x: origin_x + (cell_width + gap) * 2.0,
            y: rect.y + spacing.field_first_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Red => Rect {
            x: origin_x,
            y: rect.y + spacing.field_second_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Green => Rect {
            x: origin_x + cell_width + gap,
            y: rect.y + spacing.field_second_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Blue => Rect {
            x: origin_x + (cell_width + gap) * 2.0,
            y: rect.y + spacing.field_second_row_y,
            width: cell_width,
            height: spacing.field_height,
        },
        HsvColorField::Alpha => Rect {
            x: rect.x + rect.width - spacing.alpha_field_right_inset,
            y: rect.y + spacing.alpha_field_y,
            width: spacing.alpha_field_width,
            height: spacing.field_height,
        },
        HsvColorField::Hex => Rect {
            x: origin_x,
            y: rect.y + spacing.field_hex_y,
            width: (rect.width - spacing.field_hex_width_inset).max(1.0),
            height: spacing.field_height,
        },
    }
}

fn hsv_field_text(editor: HsvColorEditor, field: HsvColorField) -> String {
    match field {
        HsvColorField::Hue => format!("H: {}°", hsv_field_input_text(editor, field)),
        HsvColorField::Saturation => format!("S: {}%", hsv_field_input_text(editor, field)),
        HsvColorField::Value => format!("V: {}%", hsv_field_input_text(editor, field)),
        HsvColorField::Red => format!("R: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Green => format!("G: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Blue => format!("B: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Alpha => format!("A: {}", hsv_field_input_text(editor, field)),
        HsvColorField::Hex => hsv_field_input_text(editor, field),
    }
}

fn hsv_field_input_text(editor: HsvColorEditor, field: HsvColorField) -> String {
    match field {
        HsvColorField::Hue => format!("{:.0}", editor.hue_degrees),
        HsvColorField::Saturation => format!("{:.0}", editor.saturation * 100.0),
        HsvColorField::Value => format!("{:.0}", editor.value * 100.0),
        HsvColorField::Red => format!("{:.0}", editor.color().0 * 255.0),
        HsvColorField::Green => format!("{:.0}", editor.color().1 * 255.0),
        HsvColorField::Blue => format!("{:.0}", editor.color().2 * 255.0),
        HsvColorField::Alpha => format!("{:.0}", editor.alpha * 255.0),
        HsvColorField::Hex => editor.hex(),
    }
}

fn hsv_numeric_field_value(editor: HsvColorEditor, field: HsvColorField) -> Option<f32> {
    let color = editor.color();
    match field {
        HsvColorField::Hue => Some(editor.hue_degrees),
        HsvColorField::Saturation => Some(editor.saturation * 100.0),
        HsvColorField::Value => Some(editor.value * 100.0),
        HsvColorField::Red => Some(color.0 * 255.0),
        HsvColorField::Green => Some(color.1 * 255.0),
        HsvColorField::Blue => Some(color.2 * 255.0),
        HsvColorField::Alpha => Some(editor.alpha * 255.0),
        HsvColorField::Hex => None,
    }
}

fn set_hsv_numeric_field(editor: &mut HsvColorEditor, field: HsvColorField, text: &str) -> bool {
    if field == HsvColorField::Hex {
        return editor.set_hex(text).unwrap_or(false);
    }
    let Ok(value) = text
        .trim()
        .trim_end_matches('%')
        .trim_end_matches('°')
        .parse::<f32>()
    else {
        return false;
    };
    match field {
        HsvColorField::Hue => editor.set_hsv(field, value),
        HsvColorField::Saturation | HsvColorField::Value => editor.set_hsv(field, value / 100.0),
        HsvColorField::Red | HsvColorField::Green | HsvColorField::Blue => {
            editor.set_rgb(field, value / 255.0)
        }
        HsvColorField::Alpha => editor.set_hsv(field, value / 255.0),
        HsvColorField::Hex => false,
    }
}

fn edit_hsv_color(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut HsvColorEditor) -> bool,
) {
    let Some(before_editor) = world.get::<HsvColorEditor>(target).copied() else {
        return;
    };
    let Ok(before) = before_editor.to_json() else {
        return;
    };
    let changed = world
        .get_mut::<HsvColorEditor>(target)
        .is_some_and(|mut editor| edit(&mut editor));
    if !changed {
        return;
    }
    if let Some(editor) = world.get::<HsvColorEditor>(target)
        && let Ok(after) = editor.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::HsvColorCommitted {
            target,
            before,
            after,
        });
    }
}

fn begin_hsv_color_pointer(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let (Some(editor), Some(layout)) = (
        world.get::<HsvColorEditor>(target).copied(),
        world.get::<UiLayoutResult>(target).copied(),
    ) else {
        return;
    };
    let rect = layout.rect;
    let spacing = hsv_spacing(world);
    let (center, outer_radius, _) = hsv_wheel_geometry(rect, spacing);
    let dx = position[0] - center[0];
    let dy = position[1] - center[1];
    if dx.hypot(dy) <= outer_radius + 2.0 {
        world
            .resource_mut::<WidgetInteractionState>()
            .hsv_color_drags
            .insert(
                window,
                HsvColorDrag {
                    target,
                    before: editor,
                    kind: HsvColorDragKind::Wheel,
                },
            );
        update_hsv_color_drag(world, window, position);
        return;
    }
    let value_rect = hsv_value_rect(rect, spacing);
    if value_rect.contains(position[0], position[1]) {
        world
            .resource_mut::<WidgetInteractionState>()
            .hsv_color_drags
            .insert(
                window,
                HsvColorDrag {
                    target,
                    before: editor,
                    kind: HsvColorDragKind::Value,
                },
            );
        update_hsv_color_drag(world, window, position);
        return;
    }
    let alpha_rect = hsv_alpha_rect(rect, spacing);
    if alpha_rect.contains(position[0], position[1]) {
        world
            .resource_mut::<WidgetInteractionState>()
            .hsv_color_drags
            .insert(
                window,
                HsvColorDrag {
                    target,
                    before: editor,
                    kind: HsvColorDragKind::Alpha,
                },
            );
        update_hsv_color_drag(world, window, position);
        return;
    }
    for field in HsvColorField::ALL {
        if hsv_field_rect(rect, field, spacing).contains(position[0], position[1])
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.active_field = field;
            state.editing = false;
            state.edit_buffer.clear();
            return;
        }
    }
}

fn update_hsv_color_drag(world: &mut World, window: Entity, position: [f32; 2]) {
    let Some((target, kind)) = world
        .resource::<WidgetInteractionState>()
        .hsv_color_drags
        .get(&window)
        .map(|drag| (drag.target, drag.kind))
    else {
        return;
    };
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = hsv_spacing(world);
    if let Some(mut editor) = world.get_mut::<HsvColorEditor>(target) {
        match kind {
            HsvColorDragKind::Wheel => {
                let (center, _, inner_radius) = hsv_wheel_geometry(rect, spacing);
                let dx = position[0] - center[0];
                let dy = position[1] - center[1];
                let distance = dx.hypot(dy).min(inner_radius);
                let hue = dy.atan2(dx).to_degrees().rem_euclid(360.0);
                editor.set_hsv(HsvColorField::Hue, hue);
                if distance <= inner_radius {
                    editor.set_hsv(HsvColorField::Saturation, distance / inner_radius);
                }
            }
            HsvColorDragKind::Value => {
                editor.set_hsv(
                    HsvColorField::Value,
                    normalized_x(hsv_value_rect(rect, spacing), position[0]),
                );
            }
            HsvColorDragKind::Alpha => {
                editor.set_hsv(
                    HsvColorField::Alpha,
                    normalized_x(hsv_alpha_rect(rect, spacing), position[0]),
                );
            }
        }
    }
}

fn finish_hsv_color_drag(
    world: &mut World,
    drag: HsvColorDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Some(mut editor) = world.get_mut::<HsvColorEditor>(drag.target) {
            *editor = drag.before;
        }
        return;
    }
    let (Ok(before), Some(editor)) = (
        drag.before.to_json(),
        world.get::<HsvColorEditor>(drag.target),
    ) else {
        return;
    };
    if let Ok(after) = editor.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::HsvColorCommitted {
            target: drag.target,
            before,
            after,
        });
    }
}

fn curve_plot_rect(rect: Rect, theme: &Theme) -> Rect {
    let spacing = theme.spacing.editor;
    let left = spacing.gutter_width + spacing.padding_x;
    let right = spacing.padding_x;
    let vertical = theme.control_height + spacing.padding_y;
    Rect {
        x: rect.x + left,
        y: rect.y + vertical,
        width: (rect.width - left - right).max(1.0),
        height: (rect.height - vertical - spacing.padding_y).max(1.0),
    }
}

fn curve_data_position(editor: &CurveEditor, plot: Rect, position: [f32; 2]) -> [f32; 2] {
    editor.view_to_curve([
        (position[0] - plot.x) / plot.width,
        1.0 - (position[1] - plot.y) / plot.height,
    ])
}

fn curve_screen_position(editor: &CurveEditor, plot: Rect, position: [f32; 2]) -> [f32; 2] {
    let view = editor.curve_to_view(position);
    [
        plot.x + view[0] * plot.width,
        plot.y + (1.0 - view[1]) * plot.height,
    ]
}

fn begin_curve_editor_pointer(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
    modifiers: UiModifiers,
) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let (before, kind) = {
        let (Some(layout), Some(mut editor)) = (
            world.get::<UiLayoutResult>(target).copied(),
            world.get_mut::<CurveEditor>(target),
        ) else {
            return;
        };
        let plot = curve_plot_rect(layout.rect, &theme);
        if !plot.contains(position[0], position[1]) {
            return;
        }
        let before = editor.clone();
        let selected = editor.selected;
        let mut kind = selected.and_then(|id| {
            (editor.interpolation == Interpolation::Smooth)
                .then(|| editor.points.iter().find(|point| point.id == id))
                .flatten()
                .and_then(|point| {
                    let point_position = curve_screen_position(&editor, plot, point.position);
                    let left = curve_screen_position(
                        &editor,
                        plot,
                        [
                            point.position[0] + point.left_handle[0],
                            point.position[1] + point.left_handle[1],
                        ],
                    );
                    let right = curve_screen_position(
                        &editor,
                        plot,
                        [
                            point.position[0] + point.right_handle[0],
                            point.position[1] + point.right_handle[1],
                        ],
                    );
                    if distance(left, position) <= 8.0 {
                        Some(CurveEditorDragKind::LeftHandle(id))
                    } else if distance(right, position) <= 8.0 {
                        Some(CurveEditorDragKind::RightHandle(id))
                    } else if distance(point_position, position) <= 9.0 {
                        Some(CurveEditorDragKind::Point(id))
                    } else {
                        None
                    }
                })
        });
        if kind.is_none() {
            kind = editor
                .points
                .iter()
                .map(|point| {
                    (
                        distance(
                            curve_screen_position(&editor, plot, point.position),
                            position,
                        ),
                        point.id,
                    )
                })
                .filter(|(distance, _)| *distance <= 9.0)
                .min_by(|left, right| left.0.total_cmp(&right.0))
                .map(|(_, id)| CurveEditorDragKind::Point(id));
        }
        if let Some(CurveEditorDragKind::Point(id))
        | Some(CurveEditorDragKind::LeftHandle(id))
        | Some(CurveEditorDragKind::RightHandle(id)) = kind
        {
            editor.selected = Some(id);
        } else if kind.is_none() && modifiers.shift {
            let data = curve_data_position(&editor, plot, position);
            kind = editor
                .add_point(data[0], data[1])
                .map(CurveEditorDragKind::Point);
        }
        let Some(kind) = kind else {
            return;
        };
        (before, kind)
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .curve_editor_drags
        .insert(
            window,
            CurveEditorDrag {
                target,
                before,
                kind,
            },
        );
}

fn begin_curve_editor_pan(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let (Some(layout), Some(editor)) = (
        world.get::<UiLayoutResult>(target).copied(),
        world.get::<CurveEditor>(target).cloned(),
    ) else {
        return;
    };
    if !curve_plot_rect(layout.rect, &theme).contains(position[0], position[1]) {
        return;
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .curve_editor_drags
        .insert(
            window,
            CurveEditorDrag {
                target,
                before: editor,
                kind: CurveEditorDragKind::Pan,
            },
        );
}

fn update_curve_editor_drag(
    world: &mut World,
    window: Entity,
    position: [f32; 2],
    delta: [f32; 2],
) {
    let Some((target, kind)) = world
        .resource::<WidgetInteractionState>()
        .curve_editor_drags
        .get(&window)
        .map(|drag| (drag.target, drag.kind))
    else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let Some(plot) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| curve_plot_rect(layout.rect, &theme))
    else {
        return;
    };
    if let Some(mut editor) = world.get_mut::<CurveEditor>(target) {
        match kind {
            CurveEditorDragKind::Point(id) => {
                let data = curve_data_position(&editor, plot, position);
                editor.move_point(id, data);
            }
            CurveEditorDragKind::LeftHandle(id) | CurveEditorDragKind::RightHandle(id) => {
                let data = curve_data_position(&editor, plot, position);
                if let Some(point) = editor.points.iter().find(|point| point.id == id).copied() {
                    let offset = [data[0] - point.position[0], data[1] - point.position[1]];
                    let (left, right) = if kind == CurveEditorDragKind::LeftHandle(id) {
                        (offset, point.right_handle)
                    } else {
                        (point.left_handle, offset)
                    };
                    editor.set_handles(id, left, right);
                }
            }
            CurveEditorDragKind::Pan => {
                editor.pan_by([delta[0] / plot.width, -delta[1] / plot.height]);
            }
        }
    }
}

fn finish_curve_editor_drag(
    world: &mut World,
    drag: CurveEditorDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Some(mut editor) = world.get_mut::<CurveEditor>(drag.target) {
            *editor = drag.before;
        }
        return;
    }
    let Some(editor) = world.get::<CurveEditor>(drag.target) else {
        return;
    };
    if !curve_editor_content_changed(&drag.before, editor) {
        return;
    }
    let (Ok(before), Ok(after)) = (drag.before.to_json(), editor.to_json()) else {
        return;
    };
    emitted.push(WidgetEvent::CurveEditorCommitted {
        target: drag.target,
        before,
        after,
    });
}

fn curve_editor_content_changed(before: &CurveEditor, after: &CurveEditor) -> bool {
    before.points != after.points
        || before.interpolation != after.interpolation
        || before.pan != after.pan
        || before.zoom != after.zoom
}

fn edit_curve_editor(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut CurveEditor) -> bool,
) {
    let Some(before_editor) = world.get::<CurveEditor>(target).cloned() else {
        return;
    };
    let changed = world
        .get_mut::<CurveEditor>(target)
        .is_some_and(|mut editor| edit(&mut editor));
    if !changed
        || !world
            .get::<CurveEditor>(target)
            .is_some_and(|after| curve_editor_content_changed(&before_editor, after))
    {
        return;
    }
    if let (Ok(before), Some(after)) = (before_editor.to_json(), world.get::<CurveEditor>(target))
        && let Ok(after) = after.to_json()
    {
        emitted.push(WidgetEvent::CurveEditorCommitted {
            target,
            before,
            after,
        });
    }
}

fn zoom_curve_editor_at(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    delta_y: f32,
    emitted: &mut Vec<WidgetEvent>,
) {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return;
    }
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let plot = curve_plot_rect(layout.rect, &theme);
    let anchor = [
        (position[0] - plot.x) / plot.width,
        1.0 - (position[1] - plot.y) / plot.height,
    ];
    let factor = 1.1_f32.powf(delta_y.clamp(-8.0, 8.0));
    edit_curve_editor(world, target, emitted, |editor| {
        let before = (editor.pan, editor.zoom);
        editor.zoom_by(factor, anchor);
        before != (editor.pan, editor.zoom)
    });
}

fn distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    (left[0] - right[0]).hypot(left[1] - right[1])
}

fn collection_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<ListModel<String>>(target).is_some()
        || world.get::<TreeModel<String>>(target).is_some()
        || world.get::<PreviewGrid>(target).is_some()
    {
        Some(target)
    } else {
        world
            .get::<CollectionItemControl>(target)
            .map(|item| item.owner)
    }
}

fn data_table_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<DataTable>(target).is_some() {
        Some(target)
    } else {
        world
            .get::<DataTableCellControl>(target)
            .map(|cell| cell.owner)
            .or_else(|| {
                world
                    .get::<DataTableHeaderControl>(target)
                    .map(|header| header.owner)
            })
    }
}

fn data_table_json(world: &World, owner: Entity) -> Option<String> {
    world
        .get::<DataTable>(owner)
        .and_then(|table| table.to_json().ok())
}

fn edit_data_table(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut DataTable) -> bool,
) {
    let Some(owner) = data_table_owner(world, target) else {
        return;
    };
    let Some(before) = data_table_json(world, owner) else {
        return;
    };
    if world
        .get_mut::<DataTable>(owner)
        .is_none_or(|mut table| !edit(&mut table))
    {
        return;
    }
    if let Some(after) = data_table_json(world, owner)
        && before != after
    {
        emitted.push(WidgetEvent::DataTableCommitted {
            target: owner,
            before,
            after,
        });
    }
}

fn select_data_table_cell(
    world: &mut World,
    cell: &DataTableCellControl,
    ctrl: bool,
    shift: bool,
    click_count: u32,
    emitted: &mut Vec<WidgetEvent>,
) {
    edit_data_table(world, cell.owner, emitted, |table| {
        table.select_cell(cell.row_id, cell.column, ctrl, shift)
    });
    if click_count >= 2 {
        emitted.push(WidgetEvent::DataTableCellActivated {
            target: cell.owner,
            row_id: cell.row_id,
            column: cell.column,
        });
    }
}

fn begin_data_table_resize(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let Some(header) = world.get::<DataTableHeaderControl>(target).cloned() else {
        return;
    };
    let Some(hit_box) = world.get::<UiHitBox>(target) else {
        return;
    };
    if position[0] < hit_box.rect.x + hit_box.rect.width - 7.0 {
        return;
    }
    let Some(before) = data_table_json(world, header.owner) else {
        return;
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .data_table_resize_drags
        .insert(
            window,
            DataTableResizeDrag {
                owner: header.owner,
                source: target,
                column: header.column,
                start_x: position[0],
                start_width: header.width,
                before,
                changed: false,
            },
        );
}

fn update_data_table_resize(world: &mut World, window: Entity, position: [f32; 2]) {
    let Some((owner, column, start_x, start_width)) = world
        .resource::<WidgetInteractionState>()
        .data_table_resize_drags
        .get(&window)
        .map(|drag| (drag.owner, drag.column, drag.start_x, drag.start_width))
    else {
        return;
    };
    let changed = world
        .get_mut::<DataTable>(owner)
        .is_some_and(|mut table| table.resize_column(column, start_width + position[0] - start_x));
    if changed {
        let mut interaction = world.resource_mut::<WidgetInteractionState>();
        if let Some(drag) = interaction.data_table_resize_drags.get_mut(&window) {
            drag.changed = true;
        }
    }
}

fn finish_data_table_resize(
    world: &mut World,
    drag: DataTableResizeDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Ok(before) = DataTable::from_json(&drag.before)
            && let Some(mut table) = world.get_mut::<DataTable>(drag.owner)
        {
            *table = before;
        }
        return;
    }
    if !drag.changed {
        return;
    }
    if let Some(after) = data_table_json(world, drag.owner)
        && after != drag.before
    {
        emitted.push(WidgetEvent::DataTableCommitted {
            target: drag.owner,
            before: drag.before,
            after,
        });
    }
}

fn scroll_data_table(
    world: &mut World,
    target: Entity,
    delta: [f32; 2],
    unit: bui_core::UiWheelUnit,
) {
    let Some(owner) = data_table_owner(world, target) else {
        return;
    };
    let table_row_height = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.table.row_height,
        |theme| theme.spacing.table.row_height,
    );
    let row_height = world
        .get::<DataTable>(owner)
        .map_or(table_row_height, |table| {
            if table.row_height == 24 {
                table_row_height
            } else {
                table.row_height.max(1) as f32
            }
        });
    let rows = |dy: f32| match unit {
        bui_core::UiWheelUnit::Lines => dy.abs().ceil() as usize,
        bui_core::UiWheelUnit::Pixels => (dy.abs() / row_height).ceil().max(1.0) as usize,
    };
    if let Some(mut table) = world.get_mut::<DataTable>(owner) {
        if delta[1].is_finite() && delta[1] != 0.0 {
            let amount = rows(delta[1]);
            table.scroll_rows = if delta[1] > 0.0 {
                table.scroll_rows.saturating_sub(amount)
            } else {
                table.scroll_rows.saturating_add(amount)
            };
        }
        if delta[0].is_finite() {
            table.scroll_x = (table.scroll_x - delta[0]).max(0.0);
        }
    }
}

fn scroll_data_table_by_rows(world: &mut World, target: Entity, delta: i32) {
    if let Some(owner) = data_table_owner(world, target)
        && let Some(mut table) = world.get_mut::<DataTable>(owner)
    {
        table.scroll_rows = table.scroll_rows.saturating_add_signed(delta as isize);
    }
}

fn move_data_table_active_row(
    world: &mut World,
    owner: Entity,
    delta: i32,
    extend: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    edit_data_table(world, owner, emitted, |table| {
        let before = table.selection.clone();
        for _ in 0..delta.unsigned_abs() {
            table.move_active(delta.signum(), extend);
        }
        before != table.selection
    });
}

fn data_table_page_size(world: &World, owner: Entity) -> usize {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let header_heights = theme.control_height + theme.spacing.table.header_height;
    let height = world
        .get::<UiLayoutResult>(owner)
        .map_or(240.0, |layout| layout.rect.height - header_heights);
    let row_height =
        world
            .get::<DataTable>(owner)
            .map_or(theme.spacing.table.row_height, |table| {
                if table.row_height == 24 {
                    theme.spacing.table.row_height
                } else {
                    table.row_height.max(1) as f32
                }
            });
    (height.max(row_height) / row_height).floor() as usize
}

fn activate_data_table_cell(world: &World, owner: Entity) -> Option<(u64, usize)> {
    let table = world.get::<DataTable>(owner)?;
    let row_id = table.selection.active?;
    table
        .rows
        .iter()
        .any(|row| row.id == row_id)
        .then_some((row_id, table.active_column))
}

fn handle_data_table_key(
    world: &mut World,
    owner: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && key == UiKeyCode::A {
        edit_data_table(world, owner, emitted, DataTable::select_all);
        return;
    }
    match key {
        UiKeyCode::ArrowUp => {
            move_data_table_active_row(world, owner, -1, modifiers.shift, emitted)
        }
        UiKeyCode::ArrowDown => {
            move_data_table_active_row(world, owner, 1, modifiers.shift, emitted)
        }
        UiKeyCode::ArrowLeft => edit_data_table(world, owner, emitted, |table| {
            let before = table.active_column;
            table.move_active_column(-1);
            before != table.active_column
        }),
        UiKeyCode::ArrowRight => edit_data_table(world, owner, emitted, |table| {
            let before = table.active_column;
            table.move_active_column(1);
            before != table.active_column
        }),
        UiKeyCode::PageUp | UiKeyCode::PageDown => {
            let amount = data_table_page_size(world, owner).max(1) as i32;
            let direction = if key == UiKeyCode::PageUp {
                -amount
            } else {
                amount
            };
            move_data_table_active_row(world, owner, direction, modifiers.shift, emitted);
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let end = key == UiKeyCode::End;
            edit_data_table(world, owner, emitted, |table| {
                let row_id = if end {
                    table.rows.last().map(|row| row.id)
                } else {
                    table.rows.first().map(|row| row.id)
                };
                row_id.is_some_and(|row_id| table.set_active_row(row_id, modifiers.shift))
            });
        }
        UiKeyCode::Enter => {
            if let Some((row_id, column)) = activate_data_table_cell(world, owner) {
                emitted.push(WidgetEvent::DataTableCellActivated {
                    target: owner,
                    row_id,
                    column,
                });
            }
        }
        _ => {}
    }
}

fn collection_json(world: &World, owner: Entity) -> Option<String> {
    world
        .get::<ListModel<String>>(owner)
        .and_then(|model| model.to_json().ok())
        .or_else(|| {
            world
                .get::<TreeModel<String>>(owner)
                .and_then(|model| model.to_json().ok())
        })
        .or_else(|| {
            world
                .get::<PreviewGrid>(owner)
                .and_then(|model| model.to_json().ok())
        })
}

fn edit_collection(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut World, Entity) -> bool,
) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let Some(before) = collection_json(world, owner) else {
        return;
    };
    edit_collection_from_snapshot(world, owner, before, emitted, edit);
}

fn edit_collection_from_snapshot(
    world: &mut World,
    owner: Entity,
    before: String,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut World, Entity) -> bool,
) {
    if !edit(world, owner) {
        return;
    }
    if let Some(after) = collection_json(world, owner)
        && before != after
    {
        emitted.push(WidgetEvent::CollectionCommitted {
            target: owner,
            before,
            after,
        });
    }
}

fn collection_is_renaming(world: &World, owner: Entity) -> bool {
    world
        .get::<CollectionEditorState>(owner)
        .is_some_and(|state| state.rename_id.is_some())
}

fn collection_value(world: &World, owner: Entity, id: u64) -> Option<String> {
    world
        .get::<ListModel<String>>(owner)
        .and_then(|model| model.items.iter().find(|item| item.id == id))
        .map(|item| item.value.clone())
        .or_else(|| {
            world
                .get::<TreeModel<String>>(owner)
                .and_then(|model| model.nodes.iter().find(|node| node.id == id))
                .map(|node| node.value.clone())
        })
        .or_else(|| {
            world
                .get::<PreviewGrid>(owner)
                .and_then(|model| model.items.iter().find(|item| item.id == id))
                .map(|item| item.label.clone())
        })
}

fn begin_collection_rename(world: &mut World, target: Entity, id: u64) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let Some(value) = collection_value(world, owner, id) else {
        return;
    };
    if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
        state.rename_id = Some(id);
        state.rename_original.clone_from(&value);
        state.rename_buffer = value;
        state.rename_cursor = state.rename_buffer.chars().count();
        state.rename_select_all = true;
    }
}

fn cancel_collection_rename(world: &mut World, target: Entity) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
        *state = CollectionEditorState::default();
    }
}

fn collection_rename_text(world: &mut World, owner: Entity, text: &str) {
    if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner)
        && state.rename_id.is_some()
    {
        if state.rename_select_all {
            state.rename_buffer.clear();
            state.rename_cursor = 0;
            state.rename_select_all = false;
        }
        let byte_index = state
            .rename_buffer
            .char_indices()
            .nth(state.rename_cursor)
            .map_or(state.rename_buffer.len(), |(index, _)| index);
        state.rename_buffer.insert_str(byte_index, text);
        state.rename_cursor += text.chars().count();
    }
}

fn commit_collection_rename(world: &mut World, target: Entity, emitted: &mut Vec<WidgetEvent>) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let Some((id, value)) = world
        .get::<CollectionEditorState>(owner)
        .and_then(|state| state.rename_id.map(|id| (id, state.rename_buffer.clone())))
    else {
        return;
    };
    cancel_collection_rename(world, owner);
    edit_collection(world, owner, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            list.rename(id, value)
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            tree.rename(id, value)
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            grid.rename(id, value)
        } else {
            false
        }
    });
}

fn click_collection_item(
    world: &mut World,
    item: &CollectionItemControl,
    position: [f32; 2],
    click_count: u32,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let owner = item.owner;
    let layout = world.get::<UiLayoutResult>(item.owner).copied();
    let list_spacing = world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().spacing.list, |theme| theme.spacing.list);
    let in_disclosure = item.kind == CollectionItemKind::Tree
        && item.has_children
        && layout.is_some_and(|layout| {
            position[0]
                < layout.rect.x
                    + list_spacing.padding_x
                    + list_spacing.indent * (item.depth as f32 + 1.0)
        });
    edit_collection(world, owner, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            let before = list.selection.clone();
            list.select(item.id, modifiers.control, modifiers.shift);
            before != list.selection
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            let before = tree.selection.clone();
            tree.select(item.id, modifiers.control, modifiers.shift);
            let selected = before != tree.selection;
            let expanded = (in_disclosure || click_count >= 2) && tree.toggle_expanded(item.id);
            selected || expanded
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            let before = grid.selection.clone();
            grid.select(item.id, modifiers.control, modifiers.shift);
            before != grid.selection
        } else {
            false
        }
    });
    if click_count >= 2 && item.kind != CollectionItemKind::Tree {
        emitted.push(WidgetEvent::CollectionItemActivated {
            target: owner,
            id: item.id,
        });
    }
}

fn handle_collection_key(
    world: &mut World,
    owner: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if collection_is_renaming(world, owner) {
        match key {
            UiKeyCode::Enter => commit_collection_rename(world, owner, emitted),
            UiKeyCode::Escape => cancel_collection_rename(world, owner),
            UiKeyCode::Backspace => {
                if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
                    if state.rename_select_all {
                        state.rename_buffer.clear();
                        state.rename_cursor = 0;
                        state.rename_select_all = false;
                    } else if state.rename_cursor > 0 {
                        let from = state
                            .rename_buffer
                            .char_indices()
                            .nth(state.rename_cursor - 1)
                            .map_or(0, |(index, _)| index);
                        let to = state
                            .rename_buffer
                            .char_indices()
                            .nth(state.rename_cursor)
                            .map_or(state.rename_buffer.len(), |(index, _)| index);
                        state.rename_buffer.replace_range(from..to, "");
                        state.rename_cursor -= 1;
                    }
                }
            }
            UiKeyCode::Delete => {
                if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
                    if state.rename_select_all {
                        state.rename_buffer.clear();
                        state.rename_cursor = 0;
                        state.rename_select_all = false;
                    } else if let Some((from, ch)) =
                        state.rename_buffer.char_indices().nth(state.rename_cursor)
                    {
                        state
                            .rename_buffer
                            .replace_range(from..from + ch.len_utf8(), "");
                    }
                }
            }
            UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight | UiKeyCode::Home | UiKeyCode::End => {
                if let Some(mut state) = world.get_mut::<CollectionEditorState>(owner) {
                    state.rename_select_all = false;
                    state.rename_cursor = match key {
                        UiKeyCode::ArrowLeft => state.rename_cursor.saturating_sub(1),
                        UiKeyCode::ArrowRight => {
                            (state.rename_cursor + 1).min(state.rename_buffer.chars().count())
                        }
                        UiKeyCode::Home => 0,
                        UiKeyCode::End => state.rename_buffer.chars().count(),
                        _ => state.rename_cursor,
                    };
                }
            }
            _ => {}
        }
        return;
    }
    if key == UiKeyCode::F2 {
        if let Some(id) = collection_active(world, owner) {
            begin_collection_rename(world, owner, id);
        }
        return;
    }
    if (modifiers.control || modifiers.super_key) && key == UiKeyCode::A {
        edit_collection(world, owner, emitted, |world, owner| {
            if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                let before = list.selection.clone();
                list.selection.selected = list
                    .items
                    .iter()
                    .filter(|item| item.enabled)
                    .map(|item| item.id)
                    .collect();
                list.selection.active = list.selection.selected.iter().next_back().copied();
                list.selection.anchor = list.selection.active;
                before != list.selection
            } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                let before = tree.selection.clone();
                let visible = tree.visible_ids();
                tree.selection.selected = visible
                    .into_iter()
                    .filter(|id| tree.nodes.iter().any(|node| node.id == *id && node.enabled))
                    .collect();
                tree.selection.active = tree.selection.selected.iter().next_back().copied();
                tree.selection.anchor = tree.selection.active;
                before != tree.selection
            } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                let before = grid.selection.clone();
                grid.selection.selected = grid
                    .items
                    .iter()
                    .filter(|item| item.enabled)
                    .map(|item| item.id)
                    .collect();
                grid.selection.active = grid.selection.selected.iter().next_back().copied();
                grid.selection.anchor = grid.selection.active;
                before != grid.selection
            } else {
                false
            }
        });
        return;
    }
    match key {
        UiKeyCode::ArrowUp if world.get::<PreviewGrid>(owner).is_some() => {
            let columns = world
                .get::<PreviewGrid>(owner)
                .map_or(1, |grid| grid.columns.max(1)) as i32;
            move_collection_active(world, owner, -columns, modifiers.shift, emitted);
        }
        UiKeyCode::ArrowDown if world.get::<PreviewGrid>(owner).is_some() => {
            let columns = world
                .get::<PreviewGrid>(owner)
                .map_or(1, |grid| grid.columns.max(1)) as i32;
            move_collection_active(world, owner, columns, modifiers.shift, emitted);
        }
        UiKeyCode::ArrowUp => move_collection_active(world, owner, -1, modifiers.shift, emitted),
        UiKeyCode::ArrowDown => move_collection_active(world, owner, 1, modifiers.shift, emitted),
        UiKeyCode::ArrowLeft if world.get::<PreviewGrid>(owner).is_some() => {
            move_collection_active(world, owner, -1, modifiers.shift, emitted)
        }
        UiKeyCode::ArrowRight if world.get::<PreviewGrid>(owner).is_some() => {
            move_collection_active(world, owner, 1, modifiers.shift, emitted)
        }
        UiKeyCode::PageUp | UiKeyCode::PageDown => {
            let amount = collection_page_size(world, owner).max(1) as i32;
            let direction = if key == UiKeyCode::PageUp {
                -amount
            } else {
                amount
            };
            let delta = if let Some(grid) = world.get::<PreviewGrid>(owner) {
                direction * grid.columns.max(1) as i32
            } else {
                direction
            };
            move_collection_active(world, owner, delta, modifiers.shift, emitted);
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let end = key == UiKeyCode::End;
            edit_collection(world, owner, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    let Some(id) = (if end {
                        list.items.iter().rev().find(|item| item.enabled)
                    } else {
                        list.items.iter().find(|item| item.enabled)
                    })
                    .map(|item| item.id) else {
                        return false;
                    };
                    list.select(id, false, modifiers.shift)
                } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                    let visible = tree.visible_ids();
                    let id = (if end {
                        visible.iter().rev().find(|id| {
                            tree.nodes
                                .iter()
                                .any(|node| node.id == **id && node.enabled)
                        })
                    } else {
                        visible.iter().find(|id| {
                            tree.nodes
                                .iter()
                                .any(|node| node.id == **id && node.enabled)
                        })
                    })
                    .copied();
                    id.is_some_and(|id| tree.select(id, false, modifiers.shift))
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    let id = (if end {
                        grid.items.iter().rev().find(|item| item.enabled)
                    } else {
                        grid.items.iter().find(|item| item.enabled)
                    })
                    .map(|item| item.id);
                    id.is_some_and(|id| grid.select(id, false, modifiers.shift))
                } else {
                    false
                }
            });
        }
        UiKeyCode::Enter => {
            if let Some(active) = collection_active(world, owner) {
                if world.get::<TreeModel<String>>(owner).is_some() {
                    edit_collection(world, owner, emitted, |world, owner| {
                        world
                            .get_mut::<TreeModel<String>>(owner)
                            .is_some_and(|mut tree| tree.toggle_expanded(active))
                    });
                } else {
                    emitted.push(WidgetEvent::CollectionItemActivated {
                        target: owner,
                        id: active,
                    });
                }
            }
        }
        UiKeyCode::ArrowRight if world.get::<TreeModel<String>>(owner).is_some() => {
            move_tree_horizontal(world, owner, true, emitted);
        }
        UiKeyCode::ArrowLeft if world.get::<TreeModel<String>>(owner).is_some() => {
            move_tree_horizontal(world, owner, false, emitted);
        }
        UiKeyCode::Escape => cancel_collection_rename(world, owner),
        _ => {}
    }
}

fn collection_active(world: &World, owner: Entity) -> Option<u64> {
    world
        .get::<ListModel<String>>(owner)
        .and_then(|model| model.selection.active)
        .or_else(|| {
            world
                .get::<TreeModel<String>>(owner)
                .and_then(|model| model.selection.active)
        })
        .or_else(|| {
            world
                .get::<PreviewGrid>(owner)
                .and_then(|model| model.selection.active)
        })
}

fn move_collection_active(
    world: &mut World,
    owner: Entity,
    delta: i32,
    extend: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    edit_collection(world, owner, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            let before = list.selection.clone();
            for _ in 0..delta.unsigned_abs() {
                list.move_active(delta.signum(), extend);
            }
            before != list.selection
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            let before = tree.selection.clone();
            for _ in 0..delta.unsigned_abs() {
                tree.move_active(delta.signum(), extend);
            }
            before != tree.selection
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            let before = grid.selection.clone();
            for _ in 0..delta.unsigned_abs() {
                grid.move_active(delta.signum(), extend);
            }
            before != grid.selection
        } else {
            false
        }
    });
}

fn move_tree_horizontal(
    world: &mut World,
    owner: Entity,
    expand: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(active) = collection_active(world, owner) else {
        return;
    };
    edit_collection(world, owner, emitted, |world, owner| {
        let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) else {
            return false;
        };
        let Some(node) = tree.nodes.iter().find(|node| node.id == active).cloned() else {
            return false;
        };
        if expand {
            if !node.children.is_empty() && !node.expanded {
                tree.toggle_expanded(active)
            } else if let Some(child) = node.children.first().copied() {
                tree.select(child, false, false)
            } else {
                false
            }
        } else if node.expanded && !node.children.is_empty() {
            tree.toggle_expanded(active)
        } else if let Some(parent) = node.parent {
            tree.select(parent, false, false)
        } else {
            false
        }
    });
}

fn collection_page_size(world: &World, owner: Entity) -> usize {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let height = world.get::<UiLayoutResult>(owner).map_or(240.0, |layout| {
        layout.rect.height - theme.spacing.collection.header_height
    });
    let row_height = collection_row_height(world, owner, &theme).max(1.0);
    (height.max(row_height) / row_height).floor() as usize
}

fn collection_row_height(world: &World, owner: Entity, theme: &Theme) -> f32 {
    if let Some(model) = world.get::<ListModel<String>>(owner) {
        if model.row_height == 22.0 {
            theme.spacing.list.row_height
        } else {
            model.row_height
        }
    } else if let Some(model) = world.get::<TreeModel<String>>(owner) {
        if model.row_height == 22 {
            theme.spacing.list.row_height
        } else {
            model.row_height.max(1) as f32
        }
    } else if let Some(model) = world.get::<PreviewGrid>(owner) {
        let height = if model.cell_height == 104.0 {
            theme.spacing.collection.preview_cell_height
        } else {
            model.cell_height
        };
        height + theme.spacing.collection.item_gap
    } else {
        theme.spacing.list.row_height
    }
}

fn begin_collection_drag(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let Some(item) = world.get::<CollectionItemControl>(target).cloned() else {
        return;
    };
    let Some(before) = collection_json(world, item.owner) else {
        return;
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .collection_drags
        .insert(
            window,
            CollectionDrag {
                owner: item.owner,
                id: item.id,
                kind: item.kind,
                source: target,
                start: position,
                before,
                dragging: false,
            },
        );
}

fn update_collection_drag(world: &mut World, window: Entity, position: [f32; 2]) {
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .collection_drags
        .get_mut(&window)
        && distance(drag.start, position) >= 5.0
    {
        drag.dragging = true;
    }
}

fn collection_item_at_position(
    world: &mut World,
    owner: Entity,
    window: Entity,
    position: [f32; 2],
) -> Option<(CollectionItemControl, Rect)> {
    let mut query = world.query::<(Entity, &CollectionItemControl, &UiHitBox)>();
    query
        .iter(world)
        .filter(|(_, item, hit)| {
            item.owner == owner
                && hit.window.is_none_or(|candidate| candidate == window)
                && hit.contains(position)
        })
        .max_by_key(|(_, _, hit)| hit.z_index)
        .map(|(_, item, hit)| (item.clone(), hit.rect))
}

fn finish_collection_drag(
    world: &mut World,
    drag: CollectionDrag,
    position: [f32; 2],
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled || !drag.dragging {
        return;
    }
    let window = world
        .get::<UiLayoutResult>(drag.source)
        .map_or(Entity::PLACEHOLDER, |layout| layout.window);
    let Some((drop_target, drop_rect)) =
        collection_item_at_position(world, drag.owner, window, position)
    else {
        return;
    };
    if drop_target.id == drag.id || drop_target.kind != drag.kind {
        return;
    }
    let insert_after = position[1] >= drop_rect.y + drop_rect.height * 0.5;
    let requested_index = drop_target.index + usize::from(insert_after);
    let list_spacing = world
        .get_resource::<Theme>()
        .map_or_else(|| Theme::default().spacing.list, |theme| theme.spacing.list);
    let tree_into_target = drag.kind == CollectionItemKind::Tree
        && position[0]
            > drop_rect.x
                + list_spacing.indent * 2.0
                + drop_target.depth as f32 * list_spacing.indent;
    edit_collection_from_snapshot(world, drag.owner, drag.before, emitted, |world, owner| {
        if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
            let selected = list.select(drag.id, false, false);
            let source_index = list.items.iter().position(|item| item.id == drag.id);
            let target_index = list.items.iter().position(|item| item.id == drop_target.id);
            let requested =
                target_index.map_or(requested_index, |index| index + usize::from(insert_after));
            let destination = match source_index {
                Some(source) if source < requested => requested.saturating_sub(1),
                _ => requested,
            };
            list.reorder(drag.id, destination) || selected
        } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
            let node_parent = tree
                .nodes
                .iter()
                .find(|node| node.id == drop_target.id)
                .and_then(|node| node.parent);
            let destination = if tree_into_target {
                Some(drop_target.id)
            } else {
                node_parent
            };
            let requested = if destination == Some(drop_target.id) {
                tree.nodes
                    .iter()
                    .find(|node| node.id == drop_target.id)
                    .map_or(0, |node| node.children.len())
            } else if let Some(parent) = destination {
                tree.nodes
                    .iter()
                    .find(|node| node.id == parent)
                    .and_then(|node| node.children.iter().position(|id| *id == drop_target.id))
                    .unwrap_or(0)
            } else {
                tree.nodes
                    .iter()
                    .filter(|node| node.parent.is_none())
                    .position(|node| node.id == drop_target.id)
                    .unwrap_or(0)
            };
            let requested = if destination == node_parent && insert_after {
                requested.saturating_add(1)
            } else {
                requested
            };
            let source_sibling_index = if let Some(parent) = destination {
                tree.nodes
                    .iter()
                    .find(|node| node.id == parent)
                    .and_then(|node| node.children.iter().position(|id| *id == drag.id))
            } else {
                tree.nodes
                    .iter()
                    .filter(|node| node.parent.is_none())
                    .position(|node| node.id == drag.id)
            };
            let index = if destination
                == tree
                    .nodes
                    .iter()
                    .find(|node| node.id == drag.id)
                    .and_then(|node| node.parent)
                && source_sibling_index.is_some_and(|source| source < requested)
            {
                requested.saturating_sub(1)
            } else {
                requested
            };
            let selected = tree.select(drag.id, false, false);
            let moved = tree.move_node(drag.id, destination, index);
            if moved
                && tree_into_target
                && let Some(parent) = tree.nodes.iter_mut().find(|node| node.id == drop_target.id)
            {
                parent.expanded = true;
            }
            moved || selected
        } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
            let selected = grid.select(drag.id, false, false);
            let source_index = grid.items.iter().position(|item| item.id == drag.id);
            let target_index = grid.items.iter().position(|item| item.id == drop_target.id);
            let requested =
                target_index.map_or(requested_index, |index| index + usize::from(insert_after));
            let destination = match source_index {
                Some(source) if source < requested => requested.saturating_sub(1),
                _ => requested,
            };
            grid.reorder(drag.id, destination) || selected
        } else {
            false
        }
    });
}

fn scroll_collection(world: &mut World, target: Entity, delta_y: f32, unit: bui_core::UiWheelUnit) {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return;
    }
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let row_height = collection_row_height(world, owner, &theme).max(1.0);
    let rows = match unit {
        bui_core::UiWheelUnit::Lines => delta_y.abs().ceil() as usize,
        bui_core::UiWheelUnit::Pixels => (delta_y.abs() / row_height).ceil().max(1.0) as usize,
    };
    if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
        let distance = rows as f32 * row_height;
        list.scroll_offset = if delta_y > 0.0 {
            (list.scroll_offset - distance).max(0.0)
        } else {
            list.scroll_offset + distance
        };
    } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
        tree.scroll_rows = if delta_y > 0.0 {
            tree.scroll_rows.saturating_sub(rows)
        } else {
            tree.scroll_rows.saturating_add(rows)
        };
    } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
        grid.scroll_rows = if delta_y > 0.0 {
            grid.scroll_rows.saturating_sub(rows)
        } else {
            grid.scroll_rows.saturating_add(rows)
        };
    }
}

fn scroll_collection_by_rows(world: &mut World, target: Entity, delta: i32) {
    let Some(owner) = collection_owner(world, target) else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let row_height = collection_row_height(world, owner, &theme);
    if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
        list.scroll_offset = (list.scroll_offset + delta as f32 * row_height).max(0.0);
    } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
        tree.scroll_rows = tree.scroll_rows.saturating_add_signed(delta as isize);
    } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
        grid.scroll_rows = grid.scroll_rows.saturating_add_signed(delta as isize);
    }
}

fn click_hsv_color_editor(world: &mut World, target: Entity, position: [f32; 2]) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = hsv_spacing(world);
    let value_rect = hsv_value_rect(rect, spacing);
    let alpha_rect = hsv_alpha_rect(rect, spacing);
    if value_rect.contains(position[0], position[1])
        || alpha_rect.contains(position[0], position[1])
    {
        return;
    }
    for field in HsvColorField::ALL {
        if hsv_field_rect(rect, field, spacing).contains(position[0], position[1])
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.active_field = field;
            state.editing = false;
            state.edit_buffer.clear();
            return;
        }
    }
}

fn hsv_color_text_input(world: &mut World, target: Entity, text: &str) {
    let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) else {
        return;
    };
    if !state.editing {
        state.edit_buffer.clear();
        state.editing = true;
    }
    state.edit_buffer.push_str(text);
}

fn toggle_menu(world: &mut World, target: Entity) {
    let was_open = world
        .get::<MenuControl>(target)
        .is_some_and(|menu| menu.session.open);
    let focus_restore_key = menu_focus_key(world, target);
    let closed = if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
        menu.toggle(focus_restore_key);
        menu.popup.open = menu.session.open;
        was_open && !menu.session.open
    } else {
        false
    };
    if closed {
        close_menu_and_restore(world, target);
    }
}

fn menu_focus_key(world: &World, target: Entity) -> Option<String> {
    let window = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.window);
    window
        .and_then(|window| {
            world
                .get_resource::<UiFocus>()
                .and_then(|focus| focus.state(window))
                .and_then(|state| state.focused)
        })
        .and_then(|focused| world.get::<UiKey>(focused).map(|key| key.0.clone()))
        .or_else(|| world.get::<UiKey>(target).map(|key| key.0.clone()))
}

fn restore_menu_focus(world: &mut World, owner: Entity, key: Option<String>) {
    let Some(window) = world
        .get::<UiLayoutResult>(owner)
        .map(|layout| layout.window)
    else {
        return;
    };
    let keyed = key.and_then(|key| {
        let mut query = world.query::<(Entity, &UiKey)>();
        query
            .iter(world)
            .find(|(entity, candidate)| {
                candidate.0 == key
                    && world.get::<UiFocusable>(*entity).is_some()
                    && is_interactable_in_window(world, *entity, window)
            })
            .map(|(entity, _)| entity)
    });
    let focus_target = keyed.or_else(|| {
        (world.get::<UiFocusable>(owner).is_some()
            && is_interactable_in_window(world, owner, window))
        .then_some(owner)
    });
    if let Some(target) = focus_target
        && let Some(mut focus) = world.get_resource_mut::<UiFocus>()
    {
        focus.state_mut(window).focused = Some(target);
        focus.set_active(window);
    }
}

fn close_menu_and_restore(world: &mut World, owner: Entity) {
    let restore_key = if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
        let restore_key = menu.session.focus_restore_key.clone();
        menu.close();
        menu.popup.open = false;
        restore_key
    } else {
        None
    };
    restore_menu_focus(world, owner, restore_key);
}

fn handle_widget_focus_departure(
    world: &mut World,
    previous: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    finish_text_edit_on_focus_departure(world, previous, emitted);
    let menu_owner = world
        .get::<MenuControl>(previous)
        .is_some()
        .then_some(previous)
        .or_else(|| {
            world
                .get::<MenuItemControl>(previous)
                .map(|item| item.owner)
        });
    finish_file_browser_filter_edit(world, previous, emitted);
    if let Some(owner) = menu_owner
        && world
            .get::<MenuControl>(owner)
            .is_some_and(|menu| menu.session.open)
        && let Some(mut menu) = world.get_mut::<MenuControl>(owner)
    {
        // Tab has already moved focus. Close the popup without restoring focus
        // to its trigger; Escape and command activation use the restoring path.
        menu.close();
        menu.popup.open = false;
    }
    if let Some(mut dropdown) = world.get_mut::<Dropdown>(previous) {
        dropdown.close();
    }
    finish_numeric_edit_on_focus_departure(world, previous, emitted);
    finish_hsv_edit_on_focus_departure(world, previous, emitted);
}

fn capture_focused_text_edit_drafts(world: &mut World) {
    let focused: Vec<_> = world
        .get_resource::<UiFocus>()
        .map(|focus| {
            let mut focused: Vec<_> = focus
                .windows
                .iter()
                .filter_map(|(window, state)| state.focused.map(|target| (*window, target)))
                .collect();
            if let (Some(window), Some(target)) = (focus.active_window, focus.focused)
                && !focused.contains(&(window, target))
            {
                focused.push((window, target));
            }
            focused
        })
        .unwrap_or_default();
    for (window, target) in focused {
        begin_text_edit_draft(world, window, target);
    }
}

fn begin_text_edit_draft(world: &mut World, window: Entity, target: Entity) {
    if world.get::<TextEditDraft>(target).is_some() || !text_editable(world, target) {
        return;
    }
    let Some(snapshot) = text_snapshot(world, target) else {
        return;
    };
    world.entity_mut(target).insert(TextEditDraft {
        window,
        before: snapshot.text,
        before_selection: snapshot.selection,
    });
}

/// Rebase a live text-edit draft after application history restores its value.
/// The history adapter calls this after restoring focus and selection.
pub(crate) fn refresh_text_edit_draft_after_history_restore(world: &mut World, target: Entity) {
    let focused_window = world.get_resource::<UiFocus>().and_then(|focus| {
        focus
            .windows
            .iter()
            .find_map(|(window, state)| (state.focused == Some(target)).then_some(*window))
            .or_else(|| {
                (focus.focused == Some(target))
                    .then_some(focus.active_window)
                    .flatten()
            })
    });
    if let Some(window) = focused_window {
        world.entity_mut(target).remove::<TextEditDraft>();
        begin_text_edit_draft(world, window, target);
    } else if world.get_entity(target).is_ok() {
        world.entity_mut(target).remove::<TextEditDraft>();
    }
}

fn text_editable(world: &World, target: Entity) -> bool {
    world.get::<UiReadOnly>(target).is_none()
        && world
            .get::<TextField>(target)
            .is_none_or(|field| !field.read_only)
        && world
            .get::<TextArea>(target)
            .is_none_or(|area| !area.field.read_only)
}

fn text_snapshot(world: &World, target: Entity) -> Option<TextSnapshot> {
    if let Some(buffer) = world.get::<TextBuffer>(target) {
        return Some(TextSnapshot {
            text: buffer.as_str().to_owned(),
            selection: TextSelection {
                anchor: buffer.anchor(),
                caret: buffer.caret(),
            },
        });
    }
    if let Some(buffer) = world.get::<UiTextBuffer>(target) {
        return Some(TextSnapshot {
            text: buffer.0.as_str().to_owned(),
            selection: TextSelection {
                anchor: buffer.0.anchor(),
                caret: buffer.0.caret(),
            },
        });
    }
    if let Some(field) = world.get::<TextField>(target) {
        return Some(text_field_snapshot(
            &field.text,
            field.caret,
            field.selection.as_ref(),
        ));
    }
    world.get::<TextArea>(target).map(|area| {
        text_field_snapshot(
            &area.field.text,
            area.field.caret,
            area.field.selection.as_ref(),
        )
    })
}

fn text_field_snapshot(
    text: &str,
    caret: usize,
    selection: Option<&std::ops::Range<usize>>,
) -> TextSnapshot {
    let byte_index = |char_index: usize| {
        text.char_indices()
            .nth(char_index)
            .map_or(text.len(), |(byte, _)| byte)
    };
    let (anchor, caret) = selection.map_or_else(
        || {
            let caret = byte_index(caret);
            (caret, caret)
        },
        |range| (byte_index(range.start), byte_index(range.end)),
    );
    TextSnapshot {
        text: text.to_owned(),
        selection: TextSelection { anchor, caret },
    }
}

fn finish_text_edit_on_focus_departure(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(mut buffer) = world.get_mut::<TextBuffer>(target) {
        buffer.cancel_composition();
        buffer.break_undo_group();
    } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(target) {
        buffer.0.cancel_composition();
        buffer.0.break_undo_group();
    }
    let Some(draft) = world.get::<TextEditDraft>(target).cloned() else {
        return;
    };
    let Some(after) = text_snapshot(world, target) else {
        world.entity_mut(target).remove::<TextEditDraft>();
        return;
    };
    if draft.before != after.text {
        if let Some(mut buffer) = world.get_mut::<TextBuffer>(target) {
            buffer.clear_history();
        } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(target) {
            buffer.0.clear_history();
        }
        emitted.push(WidgetEvent::TextCommitted {
            target,
            window: draft.window,
            before: draft.before,
            after: after.text,
            before_selection: draft.before_selection,
            after_selection: after.selection,
        });
    }
    world.entity_mut(target).remove::<TextEditDraft>();
}

fn finish_numeric_edit_on_focus_departure(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let variables = world
        .get::<NumericExpressionVariables>(target)
        .map(|variables| variables.0.clone())
        .unwrap_or_default();
    let Some(mut field) = world.get_mut::<NumericField>(target) else {
        return;
    };
    if field.editing.is_none() {
        return;
    }
    if !field.enabled || field.read_only {
        field.cancel_edit();
        return;
    }
    let before = field.value;
    match field.commit_edit(&variables) {
        Ok(true) => emitted.push(WidgetEvent::ValueChanged {
            target,
            before: WidgetValue::Number(before),
            after: WidgetValue::Number(field.value),
        }),
        Ok(false) => {}
        Err(error) => {
            field.editing = None;
            field.replace_on_input = false;
            field.expression_error = Some(error.clone());
            emitted.push(WidgetEvent::NumericExpressionError {
                target,
                error: error.to_string(),
            });
        }
    }
}

fn finish_hsv_edit_on_focus_departure(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(state) = world.get::<HsvColorEditorState>(target).cloned() else {
        return;
    };
    if !state.editing {
        return;
    }
    edit_hsv_color(world, target, emitted, |editor| {
        set_hsv_numeric_field(editor, state.active_field, &state.edit_buffer)
    });
    if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
        state.editing = false;
        state.edit_buffer.clear();
    }
}

fn place_text_caret(world: &mut World, target: Entity, position: [f32; 2], extend: bool) {
    let Some(layout) = world.get::<UiTextLayout>(target).cloned() else {
        return;
    };
    if let Some(mut buffer) = world.get_mut::<TextBuffer>(target) {
        layout.place_caret(&mut buffer, position, extend);
    }
}

fn handle_menu_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    _modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    if world.get::<MenuControl>(target).is_none() {
        return false;
    }
    let focus_restore_key = world
        .get::<bui_core::UiKey>(target)
        .map(|key| key.0.clone());
    let mut command = None;
    let mut restore_focus = false;
    let mut restore_key = None;
    if matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
        && world.get::<MenuControl>(target).is_some_and(|menu| {
            menu.session.open
                && menu.session.path.len() <= 1
                && (key != UiKeyCode::ArrowRight
                    || !menu
                        .session
                        .selected(&menu.tree)
                        .is_some_and(|entry| !entry.children.is_empty()))
        })
        && let Some(sibling) = menu_bar_sibling(
            world,
            target,
            if key == UiKeyCode::ArrowRight { 1 } else { -1 },
        )
    {
        let next_key = world.get::<UiKey>(sibling).map(|key| key.0.clone());
        if let Some(mut current) = world.get_mut::<MenuControl>(target) {
            current.close();
            current.popup.open = false;
        }
        if let Some(mut next) = world.get_mut::<MenuControl>(sibling) {
            next.open(next_key);
            next.popup.open = true;
        }
        if let Some(window) = world
            .get::<UiLayoutResult>(sibling)
            .map(|layout| layout.window)
            && let Some(mut focus) = world.get_resource_mut::<UiFocus>()
        {
            focus.state_mut(window).focused = Some(sibling);
            focus.set_active(window);
        }
        return true;
    }
    if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
        let tree = menu.tree.clone();
        if !menu.session.open {
            if matches!(
                key,
                UiKeyCode::Enter | UiKeyCode::Space | UiKeyCode::ArrowDown
            ) {
                menu.open(focus_restore_key);
                menu.popup.open = true;
            }
            return true;
        }
        match key {
            UiKeyCode::Escape if !menu.query.is_empty() => menu.query.clear(),
            UiKeyCode::Escape => {
                if menu.session.path.len() > 1 {
                    menu.session.close_submenu();
                } else {
                    restore_focus = true;
                    restore_key = menu.session.focus_restore_key.clone();
                    menu.close();
                }
                menu.popup.open = menu.session.open;
            }
            UiKeyCode::Backspace if !menu.query.is_empty() => {
                menu.query.pop();
            }
            UiKeyCode::ArrowDown => {
                if menu.display == MenuDisplay::Pie {
                    let parent_path = if menu.session.path.len() > 1 {
                        menu.session.path[..menu.session.path.len() - 1].to_vec()
                    } else {
                        Vec::new()
                    };
                    let selectable: Vec<_> = tree
                        .entries_at(&parent_path)
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.enabled && !entry.separator)
                        .map(|(index, _)| index)
                        .collect();
                    let next = menu.pie.and_then(|pie| pie.selected).map_or(0, |at| at + 1);
                    let count = selectable.len();
                    if count > 0 {
                        let selected = next % count;
                        if let Some(pie) = &mut menu.pie {
                            pie.select_keyboard(selected, count);
                        }
                        menu.session.path.clone_from(&parent_path);
                        menu.session.path.push(selectable[selected]);
                    }
                } else {
                    menu.session.move_selection(&tree, 1);
                }
            }
            UiKeyCode::ArrowUp => {
                if menu.display == MenuDisplay::Pie {
                    let parent_path = if menu.session.path.len() > 1 {
                        menu.session.path[..menu.session.path.len() - 1].to_vec()
                    } else {
                        Vec::new()
                    };
                    let selectable: Vec<_> = tree
                        .entries_at(&parent_path)
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.enabled && !entry.separator)
                        .map(|(index, _)| index)
                        .collect();
                    let count = selectable.len();
                    if count > 0 {
                        let selected = menu
                            .pie
                            .and_then(|pie| pie.selected)
                            .map_or(count - 1, |at| (at + count - 1) % count);
                        if let Some(pie) = &mut menu.pie {
                            pie.select_keyboard(selected, count);
                        }
                        menu.session.path.clone_from(&parent_path);
                        menu.session.path.push(selectable[selected]);
                    }
                } else {
                    menu.session.move_selection(&tree, -1);
                }
            }
            UiKeyCode::ArrowRight => {
                menu.session.open_submenu(&tree);
            }
            UiKeyCode::ArrowLeft => {
                menu.session.close_submenu();
            }
            UiKeyCode::Enter | UiKeyCode::Space => {
                let selected = menu.session.selected(&tree).cloned();
                if selected
                    .as_ref()
                    .is_some_and(|entry| !entry.children.is_empty())
                {
                    menu.session.open_submenu(&tree);
                } else {
                    command = menu.session.activate(&tree);
                    if let Some(id) = &command {
                        menu.search.pick(id.clone());
                        restore_focus = true;
                        restore_key = menu.session.focus_restore_key.clone();
                        menu.close();
                    }
                    menu.popup.open = menu.session.open;
                }
            }
            _ => {}
        }
    }
    if let Some(command_id) = command {
        emitted.push(WidgetEvent::MenuCommand { target, command_id });
    }
    if restore_focus {
        restore_menu_focus(world, target, restore_key);
    }
    true
}

fn menu_bar_sibling(world: &mut World, target: Entity, direction: i32) -> Option<Entity> {
    let parent = world.get::<UiParent>(target).map(|parent| parent.0)?;
    if !world
        .get::<UiRole>(parent)
        .is_some_and(|role| role.0 == "menu-bar")
    {
        return None;
    }
    let children = world.get::<bui_core::UiChildren>(parent)?.0.clone();
    let menus: Vec<_> = children
        .into_iter()
        .filter(|child| world.get::<MenuControl>(*child).is_some())
        .collect();
    if menus.len() < 2 {
        return None;
    }
    let index = menus.iter().position(|entity| *entity == target)? as i64;
    let next = (index + i64::from(direction)).rem_euclid(menus.len() as i64) as usize;
    menus.get(next).copied()
}

fn select_menu_item(menu: &mut MenuControl, item: &MenuItemControl) {
    if !menu.session.open || !item.enabled || item.separator {
        return;
    }
    menu.session.path.clone_from(&item.parent_path);
    menu.session.path.push(item.index);
}

fn select_menu_item_for_release(
    world: &mut World,
    item: &MenuItemControl,
    emitted: &mut Vec<WidgetEvent>,
) {
    if !item.enabled || item.separator {
        return;
    }
    let mut command = None;
    let mut restore_focus = None;
    if let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
        if !menu.session.open {
            return;
        }
        select_menu_item(&mut menu, item);
        if !item.has_submenu {
            command = Some(item.id.clone());
            menu.search.pick(item.id.clone());
            restore_focus = menu.session.focus_restore_key.clone();
            menu.close();
            menu.popup.open = false;
        }
        menu.gesture.pressed = false;
    }
    if command.is_some() {
        restore_menu_focus(world, item.owner, restore_focus);
    }
    if let Some(command_id) = command {
        emitted.push(WidgetEvent::MenuCommand {
            target: item.owner,
            command_id,
        });
    }
}

fn menu_item_at_position(
    world: &mut World,
    owner: Entity,
    window: Entity,
    position: [f32; 2],
) -> Option<MenuItemControl> {
    let mut query = world.query::<(Entity, &MenuItemControl, &UiHitBox)>();
    query
        .iter(world)
        .filter(|(entity, item, hit)| {
            item.owner == owner
                && is_interactable_in_window(world, *entity, window)
                && hit.window.is_none_or(|hit_window| hit_window == window)
                && hit.contains(position)
        })
        .max_by_key(|(_, _, hit)| hit.z_index)
        .map(|(_, item, _)| item.clone())
}

fn update_menu_pointer(
    world: &mut World,
    target: Entity,
    window: Entity,
    position: [f32; 2],
    dragging: bool,
) {
    if let Some(item) = world.get::<MenuItemControl>(target).cloned() {
        let display = world
            .get::<MenuControl>(item.owner)
            .map_or(MenuDisplay::Dropdown, |menu| menu.display);
        if display == MenuDisplay::Pie {
            update_pie_menu_pointer(world, item.owner, position);
        } else {
            let hovered = if dragging {
                menu_item_at_position(world, item.owner, window, position)
            } else {
                Some(item.clone())
            };
            if let Some(hovered) = hovered {
                if let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
                    select_menu_item(&mut menu, &hovered);
                    if dragging {
                        menu.gesture.drag_over(Some(hovered.index));
                    }
                }
            } else if dragging && let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
                menu.gesture.drag_over(None);
                menu.session.path.clear();
            }
        }
        return;
    }

    let drag_open = world.get::<MenuControl>(target).is_some_and(|menu| {
        dragging
            && !menu.session.open
            && menu.display != MenuDisplay::Context
            && menu.press_position.is_some()
    });
    if drag_open {
        let start = world
            .get::<MenuControl>(target)
            .and_then(|menu| menu.press_position);
        let focus_restore_key = menu_focus_key(world, target);
        if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
            menu.open(focus_restore_key);
            menu.opened_by_pointer = true;
            menu.popup.open = true;
            if let (Some(pie), Some(start)) = (&mut menu.pie, start) {
                pie.center = start;
            }
        }
        sync_menu_popups(world);
    }

    let display = world.get::<MenuControl>(target).map(|menu| menu.display);
    if display == Some(MenuDisplay::Pie)
        && world
            .get::<MenuControl>(target)
            .is_some_and(|menu| menu.session.open)
    {
        update_pie_menu_pointer(world, target, position);
    } else if dragging
        && world
            .get::<MenuControl>(target)
            .is_some_and(|menu| menu.session.open)
        && let Some(item) = menu_item_at_position(world, target, window, position)
        && let Some(mut menu) = world.get_mut::<MenuControl>(target)
    {
        select_menu_item(&mut menu, &item);
        menu.gesture.drag_over(Some(item.index));
    }
}

fn update_pie_menu_pointer(world: &mut World, owner: Entity, position: [f32; 2]) {
    let Some(menu) = world.get::<MenuControl>(owner) else {
        return;
    };
    if !menu.session.open || menu.display != MenuDisplay::Pie {
        return;
    }
    let path = if menu.session.path.len() > 1 {
        menu.session.path[..menu.session.path.len() - 1].to_vec()
    } else {
        Vec::new()
    };
    let Some(entries) = menu.tree.entries_at(&path) else {
        return;
    };
    let selectable: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.enabled && !entry.separator)
        .map(|(index, _)| index)
        .collect();
    let Some(mut pie) = menu.pie else {
        return;
    };
    let selected = pie.update_pointer(position, selectable.len());
    if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
        menu.pie = Some(pie);
        if let Some(index) = selected.and_then(|at| selectable.get(at).copied()) {
            menu.session.path.clone_from(&path);
            menu.session.path.push(index);
        } else {
            menu.session.path.clear();
        }
    }
}

fn selected_menu_item(world: &World, owner: Entity) -> Option<MenuItemControl> {
    let menu = world.get::<MenuControl>(owner)?;
    let (&index, parent_path) = menu.session.path.split_last()?;
    menu.items.iter().find_map(|entity| {
        world
            .get::<MenuItemControl>(*entity)
            .filter(|item| item.index == index && item.parent_path == parent_path)
            .cloned()
    })
}

fn close_menus_outside(world: &mut World, window: Entity, target: Entity) {
    if world.get::<MenuItemControl>(target).is_some() {
        return;
    }
    let mut query = world.query::<(Entity, &MenuControl, &UiLayoutResult)>();
    let open_menus: Vec<_> = query
        .iter(world)
        .filter(|(_, menu, layout)| menu.session.open && layout.window == window)
        .map(|(entity, _, _)| entity)
        .collect();
    for owner in open_menus {
        if owner == target {
            continue;
        }
        if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
            menu.close();
            menu.popup.open = false;
        }
    }
}

fn click_file_browser(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    click_count: u32,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let (view, spacing, header_height, row_height) = file_browser_metrics(world, target);
    if layout.window != window {
        return;
    }
    let local_y = position[1] - layout.rect.y;
    if local_y < header_height * 0.5 {
        let breadcrumbs = world
            .get::<FileBrowser>(target)
            .map(|browser| browser.breadcrumbs())
            .unwrap_or_default();
        let mut x = layout.rect.x + spacing.padding_x;
        let mut destination = None;
        for crumb in breadcrumbs {
            let width = (crumb.name.chars().count() as f32 * 8.0 + 18.0).max(32.0);
            if position[0] >= x && position[0] <= x + width {
                destination = Some(crumb.path);
                break;
            }
            x += width;
        }
        if let Some(destination) = destination {
            navigate_file_browser(world, target, destination, emitted);
        }
        return;
    }
    if local_y < header_height {
        let local_x = position[0] - layout.rect.x;
        let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
        let view_switch_width = theme.control_height * 3.0;
        let sort_control_width = theme.control_height * 3.25;
        if local_x < view_switch_width {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.view = if browser.view == FileView::List {
                    FileView::Grid
                } else {
                    FileView::List
                };
            }
            emit_file_browser_commit(world, target, before, emitted);
            return;
        }
        if local_x < view_switch_width + sort_control_width {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let sort = browser.sort;
                let ascending = if browser.sort == FileSort::Name {
                    !browser.ascending
                } else {
                    true
                };
                browser.set_sort(sort, ascending);
            }
            emit_file_browser_commit(world, target, before, emitted);
            return;
        }
        let bookmarks = world
            .get::<FileBrowser>(target)
            .map(|browser| browser.bookmarks.clone())
            .unwrap_or_default();
        let mut x = view_switch_width + sort_control_width + theme.spacing.control.gap * 1.5;
        for bookmark in bookmarks {
            let width = (bookmark
                .file_name()
                .map_or(8, |name| name.to_string_lossy().chars().count())
                as f32
                * 8.0
                + 14.0)
                .max(36.0);
            if local_x >= x && local_x <= x + width {
                navigate_file_browser(world, target, bookmark, emitted);
                return;
            }
            x += width;
        }
        return;
    }
    let Some(browser_view) = world.get::<FileBrowser>(target).map(|browser| browser.view) else {
        return;
    };
    let row_index = match browser_view {
        FileView::List => {
            view.scroll_rows + ((local_y - header_height).max(0.0) / row_height).floor() as usize
        }
        FileView::Grid => {
            let cell_stride = spacing.grid_cell_width + spacing.grid_gap;
            let columns = (layout.rect.width / cell_stride).floor().max(1.0) as usize;
            let row_stride = spacing.grid_cell_height + spacing.grid_gap;
            let col = ((position[0] - layout.rect.x) / cell_stride)
                .floor()
                .max(0.0) as usize;
            let row = ((local_y - header_height).max(0.0) / row_stride).floor() as usize;
            (view.scroll_rows + row) * columns + col
        }
    };
    let Some(item) = world.get::<FileBrowser>(target).and_then(|browser| {
        browser
            .visible_entries()
            .get(row_index)
            .map(|entry| FileBrowserItem {
                owner: target,
                path: entry.path.clone(),
                name: entry.name.clone(),
                index: row_index,
                directory: entry.directory,
                size: entry.size,
                selected: browser.selected.contains(&entry.path),
                preview: entry.preview,
            })
    }) else {
        return;
    };
    select_file_browser_item(world, &item, click_count, window, emitted);
}

fn click_radio_group(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let row_height = theme.control_height.max(1.0);
    let index = ((position[1] - layout.rect.y - row_height) / row_height).floor();
    let count = world
        .get::<RadioGroup>(target)
        .map_or(0, |group| group.options.len());
    if index < 0.0 || index >= count as f32 {
        return;
    }
    select_radio_option(world, target, index as usize, emitted);
}

fn select_radio_option(
    world: &mut World,
    target: Entity,
    index: usize,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(mut group) = world.get_mut::<RadioGroup>(target) {
        let before = group.selected;
        if group.select(index) {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Option(before),
                after: WidgetValue::Option(group.selected),
            });
        }
    }
}

fn select_dropdown_option(
    world: &mut World,
    owner: Entity,
    index: usize,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(mut dropdown) = world.get_mut::<Dropdown>(owner) {
        let before = dropdown.selected;
        if dropdown.choose(index) {
            emitted.push(WidgetEvent::ValueChanged {
                target: owner,
                before: WidgetValue::Option(before),
                after: WidgetValue::Option(dropdown.selected),
            });
        }
    }
}

fn dropdown_owner(world: &World, target: Entity) -> Option<Entity> {
    world
        .get::<DropdownOptionControl>(target)
        .map(|option| option.owner)
        .or_else(|| world.get::<Dropdown>(target).map(|_| target))
}

fn update_dropdown_pointer(world: &mut World, target: Entity) {
    let Some(option) = world.get::<DropdownOptionControl>(target).cloned() else {
        return;
    };
    if !option.enabled {
        return;
    }
    if let Some(mut dropdown) = world.get_mut::<Dropdown>(option.owner)
        && dropdown.open
        && dropdown.highlighted != Some(option.index)
    {
        dropdown.highlighted = Some(option.index);
    }
}

fn close_dropdowns_outside(world: &mut World, window: Entity, target: Entity) {
    let keep_open = dropdown_owner(world, target);
    let mut query = world.query::<(Entity, &Dropdown, &UiLayoutResult)>();
    let owners: Vec<_> = query
        .iter(world)
        .filter(|(owner, dropdown, layout)| {
            dropdown.open && layout.window == window && Some(*owner) != keep_open
        })
        .map(|(owner, _, _)| owner)
        .collect();
    for owner in owners {
        if let Some(mut dropdown) = world.get_mut::<Dropdown>(owner) {
            dropdown.close();
        }
    }
}

fn scroll_dropdown_options(world: &mut World, owner: Entity, delta_y: f32) {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return;
    }
    let Some((open, option_count)) = world
        .get::<Dropdown>(owner)
        .map(|dropdown| (dropdown.open, dropdown.options.len()))
    else {
        return;
    };
    if !open || option_count == 0 {
        return;
    }
    let count = world
        .get::<DropdownPopupState>(owner)
        .map_or(1, |state| state.visible_count.max(1));
    if let Some(mut state) = world.get_mut::<DropdownPopupState>(owner) {
        let max_first = option_count.saturating_sub(count);
        state.first_visible = if delta_y > 0.0 {
            state.first_visible.saturating_sub(3)
        } else {
            state.first_visible.saturating_add(3).min(max_first)
        };
    }
}

fn file_browser_metrics(
    world: &World,
    target: Entity,
) -> (
    FileBrowserViewState,
    bui_style::ThemeFileBrowserSpacing,
    f32,
    f32,
) {
    let view = world
        .get::<FileBrowserViewState>(target)
        .copied()
        .unwrap_or_default();
    let spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.file_browser,
        |theme| theme.spacing.file_browser,
    );
    let header_height = if view.header_height == 48.0 {
        spacing.header_height
    } else {
        view.header_height
    }
    .max(1.0);
    let row_height = if view.row_height == 22.0 {
        spacing.list_row_height
    } else {
        view.row_height
    }
    .max(1.0);
    (view, spacing, header_height, row_height)
}

fn file_browser_snapshot(world: &World, target: Entity) -> Option<String> {
    world
        .get::<FileBrowser>(target)
        .and_then(|browser| browser.snapshot_json().ok())
}

fn emit_file_browser_commit(
    world: &World,
    target: Entity,
    before: Option<String>,
    emitted: &mut Vec<WidgetEvent>,
) {
    let (Some(before), Some(after)) = (before, file_browser_snapshot(world, target)) else {
        return;
    };
    if before != after {
        emitted.push(WidgetEvent::FileBrowserCommitted {
            target,
            before,
            after,
        });
    }
}

fn begin_file_browser_filter_edit(world: &mut World, target: Entity) {
    if let Some(snapshot) = file_browser_snapshot(world, target) {
        let mut edits = world.resource_mut::<WidgetFileBrowserFilterEdits>();
        edits.redo.remove(&target);
        edits.active.entry(target).or_insert(snapshot);
    }
}

fn finish_file_browser_filter_edit(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let before = {
        let mut edits = world.resource_mut::<WidgetFileBrowserFilterEdits>();
        edits.redo.remove(&target);
        edits.active.remove(&target)
    };
    emit_file_browser_commit(world, target, before, emitted);
}

pub(crate) fn undo_file_browser_filter_draft(world: &mut World, target: Entity) -> bool {
    let before = world
        .get_resource_mut::<WidgetFileBrowserFilterEdits>()
        .and_then(|mut edits| edits.active.remove(&target));
    let Some(before) = before else {
        return false;
    };
    let after = file_browser_snapshot(world, target);
    let result = world
        .get_mut::<FileBrowser>(target)
        .map(|mut browser| browser.restore_json(&before));
    match result {
        Some(Ok(())) => {
            if let Some(after) = after {
                world
                    .resource_mut::<WidgetFileBrowserFilterEdits>()
                    .redo
                    .insert(target, after);
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        Some(Err(error)) => {
            if let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>() {
                let message = error.to_string();
                events.0.push(WidgetEvent::FileBrowserRestoreFailed {
                    target,
                    path: error.path,
                    message,
                });
            }
        }
        None => return false,
    }
    true
}

pub(crate) fn redo_file_browser_filter_draft(world: &mut World, target: Entity) -> bool {
    let (active, after) = {
        let Some(mut edits) = world.get_resource_mut::<WidgetFileBrowserFilterEdits>() else {
            return false;
        };
        (
            edits.active.contains_key(&target),
            edits.redo.remove(&target),
        )
    };
    let Some(after) = after else {
        return active;
    };
    let before = file_browser_snapshot(world, target);
    let result = world
        .get_mut::<FileBrowser>(target)
        .map(|mut browser| browser.restore_json(&after));
    match result {
        Some(Ok(())) => {
            if let Some(before) = before {
                world
                    .resource_mut::<WidgetFileBrowserFilterEdits>()
                    .active
                    .insert(target, before);
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        Some(Err(error)) => {
            if let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>() {
                let message = error.to_string();
                events.0.push(WidgetEvent::FileBrowserRestoreFailed {
                    target,
                    path: error.path,
                    message,
                });
            }
        }
        None => return false,
    }
    true
}

fn navigate_file_browser(
    world: &mut World,
    target: Entity,
    path: PathBuf,
    emitted: &mut Vec<WidgetEvent>,
) {
    let before = file_browser_snapshot(world, target);
    if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
        match browser.navigate(&path) {
            Ok(()) => emitted.push(WidgetEvent::FileDirectoryChanged { target, path }),
            Err(error) => emit_file_error(target, error, emitted),
        }
    }
    if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
        view.scroll_rows = 0;
    }
    emit_file_browser_commit(world, target, before, emitted);
}

fn scroll_file_browser(world: &mut World, target: Entity, delta_y: f32) {
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let (_view, spacing, header_height, row_height) = file_browser_metrics(world, target);
    let Some((browser_view, visible_count)) = world
        .get::<FileBrowser>(target)
        .map(|browser| (browser.view, browser.visible_count()))
    else {
        return;
    };
    let rows = match browser_view {
        FileView::List => ((layout.rect.height - header_height) / row_height)
            .floor()
            .max(1.0) as usize,
        FileView::Grid => ((layout.rect.height - header_height)
            / (spacing.grid_cell_height + spacing.grid_gap))
            .floor()
            .max(1.0) as usize,
    };
    let columns = if browser_view == FileView::Grid {
        (layout.rect.width / (spacing.grid_cell_width + spacing.grid_gap))
            .floor()
            .max(1.0) as usize
    } else {
        1
    };
    let total_rows = visible_count.div_ceil(columns);
    let max_scroll = total_rows.saturating_sub(rows);
    if let Some(mut state) = world.get_mut::<FileBrowserViewState>(target) {
        state.scroll_rows = if delta_y > 0.0 {
            state.scroll_rows.saturating_sub(3)
        } else if delta_y < 0.0 {
            state.scroll_rows.saturating_add(3).min(max_scroll)
        } else {
            state.scroll_rows.min(max_scroll)
        };
    }
}

fn ensure_file_cursor_visible(
    world: &mut World,
    target: Entity,
    view: FileBrowserViewState,
    height: f32,
) {
    let Some(browser) = world.get::<FileBrowser>(target) else {
        return;
    };
    let Some(cursor) = browser.cursor.as_ref() else {
        return;
    };
    let Some(index) = browser
        .visible_entries()
        .iter()
        .position(|entry| &entry.path == cursor)
    else {
        return;
    };
    let spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.file_browser,
        |theme| theme.spacing.file_browser,
    );
    let header_height = if view.header_height == 48.0 {
        spacing.header_height
    } else {
        view.header_height
    }
    .max(1.0);
    let row_height = if browser.view == FileView::List {
        if view.row_height == 22.0 {
            spacing.list_row_height.max(1.0)
        } else {
            view.row_height.max(1.0)
        }
    } else {
        (spacing.grid_cell_height + spacing.grid_gap).max(1.0)
    };
    let visible_rows = ((height - header_height) / row_height).floor().max(1.0) as usize;
    let columns = if browser.view == FileView::Grid {
        world.get::<UiLayoutResult>(target).map_or(1, |layout| {
            (layout.rect.width / (spacing.grid_cell_width + spacing.grid_gap))
                .floor()
                .max(1.0) as usize
        })
    } else {
        1
    };
    let row = index / columns;
    if let Some(mut state) = world.get_mut::<FileBrowserViewState>(target) {
        if row < state.scroll_rows {
            state.scroll_rows = row;
        } else if row >= state.scroll_rows + visible_rows {
            state.scroll_rows = row + 1 - visible_rows;
        }
    }
}

fn emit_file_error(target: Entity, error: crate::FileBrowserError, emitted: &mut Vec<WidgetEvent>) {
    emit_file_error_with_effects(target, error, Vec::new(), emitted);
}

fn emit_file_error_with_effects(
    target: Entity,
    error: crate::FileBrowserError,
    partial_effects: Vec<PathBuf>,
    emitted: &mut Vec<WidgetEvent>,
) {
    emitted.push(WidgetEvent::FileOperationFailed {
        target,
        operation: error.operation,
        path: error.path,
        message: error.source.to_string(),
        partial_effects,
    });
}

fn select_file_browser_item(
    world: &mut World,
    item: &FileBrowserItem,
    click_count: u32,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let modifiers = world
        .get_resource::<WidgetInteractionState>()
        .and_then(|state| state.modifiers.get(&window).copied())
        .unwrap_or_default();
    let before = file_browser_snapshot(world, item.owner);
    let result = {
        let Some(mut browser) = world.get_mut::<FileBrowser>(item.owner) else {
            return;
        };
        if !browser.select(item.index, modifiers.control, modifiers.shift) {
            return;
        }
        let selected = browser.selected.contains(&item.path);
        let opened = if click_count >= 2 {
            let old_directory = browser.directory.clone();
            match browser.open_cursor() {
                Ok(Some(path)) => Some(Ok(Some(path))),
                Ok(None) if browser.directory != old_directory => Some(Ok(None)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }
        } else {
            None
        };
        (selected, opened, browser.directory.clone())
    };
    emit_file_browser_commit(world, item.owner, before, emitted);
    emitted.push(WidgetEvent::FileSelected {
        target: item.owner,
        path: item.path.clone(),
        selected: result.0,
    });
    if let Some(opened) = result.1 {
        match opened {
            Ok(Some(path)) => emitted.push(WidgetEvent::FileActivated {
                target: item.owner,
                path,
            }),
            Ok(None) => {
                emitted.push(WidgetEvent::FileDirectoryChanged {
                    target: item.owner,
                    path: result.2,
                });
                if let Some(mut view) = world.get_mut::<FileBrowserViewState>(item.owner) {
                    view.scroll_rows = 0;
                }
            }
            Err(error) => emit_file_error(item.owner, error, emitted),
        }
    } else {
        let view = world
            .get::<FileBrowserViewState>(item.owner)
            .copied()
            .unwrap_or_default();
        let height = world
            .get::<UiLayoutResult>(item.owner)
            .map_or(0.0, |layout| layout.rect.height);
        ensure_file_cursor_visible(world, item.owner, view, height);
    }
}

fn handle_file_browser_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let current_filter = world
        .get::<FileBrowser>(target)
        .map(|browser| browser.filter.clone())
        .unwrap_or_default();
    let filter_edit =
        !current_filter.is_empty() && matches!(key, UiKeyCode::Backspace | UiKeyCode::Escape);
    if filter_edit {
        begin_file_browser_filter_edit(world, target);
    }
    let has_filter_edit = world
        .resource::<WidgetFileBrowserFilterEdits>()
        .active
        .contains_key(&target);
    let before = (!filter_edit && !has_filter_edit)
        .then(|| file_browser_snapshot(world, target))
        .flatten();

    handle_file_browser_key_inner(world, target, key, modifiers, emitted);

    if (filter_edit && key == UiKeyCode::Escape) || (key == UiKeyCode::Enter && has_filter_edit) {
        finish_file_browser_filter_edit(world, target, emitted);
    } else if !filter_edit && !has_filter_edit {
        emit_file_browser_commit(world, target, before, emitted);
    }
}

fn handle_file_browser_key_inner(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let view = world
        .get::<FileBrowserViewState>(target)
        .copied()
        .unwrap_or_default();
    let height = world
        .get::<UiLayoutResult>(target)
        .map_or(0.0, |layout| layout.rect.height);
    if (modifiers.control || modifiers.super_key) && key == UiKeyCode::A {
        let changed = world
            .get_mut::<FileBrowser>(target)
            .is_some_and(|mut browser| browser.select_all_visible());
        if changed {
            let selected = world
                .get::<FileBrowser>(target)
                .map(|browser| browser.selected.iter().cloned().collect())
                .unwrap_or_default();
            emitted.push(WidgetEvent::FileSelectionChanged { target, selected });
            ensure_file_cursor_visible(world, target, view, height);
        }
        return;
    }
    let (_, spacing, header_height, list_row_height) = file_browser_metrics(world, target);
    let browser_view = world
        .get::<FileBrowser>(target)
        .map_or(FileView::List, |browser| browser.view);
    let layout_width = world
        .get::<UiLayoutResult>(target)
        .map_or(0.0, |layout| layout.rect.width);
    let columns = if browser_view == FileView::Grid {
        (layout_width / (spacing.grid_cell_width + spacing.grid_gap))
            .floor()
            .max(1.0) as usize
    } else {
        1
    };
    let row_height = if browser_view == FileView::Grid {
        spacing.grid_cell_height + spacing.grid_gap
    } else {
        list_row_height
    }
    .max(1.0);
    let visible_rows = ((height - header_height) / row_height).floor().max(1.0) as usize;
    match key {
        UiKeyCode::ArrowDown
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::PageDown
        | UiKeyCode::PageUp => {
            let delta = match key {
                UiKeyCode::ArrowDown if browser_view == FileView::Grid => columns as i32,
                UiKeyCode::ArrowUp if browser_view == FileView::Grid => -(columns as i32),
                UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => 1,
                UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => -1,
                UiKeyCode::PageDown => (visible_rows * columns) as i32,
                UiKeyCode::PageUp => -((visible_rows * columns) as i32),
                _ => 0,
            };
            let delta = if browser_view == FileView::List
                && matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
            {
                0
            } else {
                delta
            };
            if delta != 0
                && let Some(mut browser) = world.get_mut::<FileBrowser>(target)
            {
                browser.move_cursor(delta, modifiers.shift);
            }
            ensure_file_cursor_visible(world, target, view, height);
        }
        UiKeyCode::Enter => {
            let result = if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let old_directory = browser.directory.clone();
                browser.open_cursor().map(|opened| {
                    (
                        opened,
                        browser.directory != old_directory,
                        browser.directory.clone(),
                    )
                })
            } else {
                return;
            };
            match result {
                Ok((Some(path), _, _)) => emitted.push(WidgetEvent::FileActivated { target, path }),
                Ok((None, true, path)) => {
                    emitted.push(WidgetEvent::FileDirectoryChanged { target, path });
                    if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                        view.scroll_rows = 0;
                    }
                }
                Ok((None, false, _)) => {}
                Err(error) => emit_file_error(target, error, emitted),
            }
        }
        UiKeyCode::Backspace => {
            let filter = world
                .get::<FileBrowser>(target)
                .map(|browser| browser.filter.clone())
                .unwrap_or_default();
            if filter.is_empty() {
                if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                    match browser.up() {
                        Ok(true) => emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        }),
                        Ok(false) => {}
                        Err(error) => emit_file_error(target, error, emitted),
                    }
                }
            } else if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let mut filter = filter;
                filter.pop();
                browser.set_filter(filter);
                if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                    view.scroll_rows = 0;
                }
            }
        }
        UiKeyCode::Escape => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.set_filter(String::new());
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let count = world
                .get::<FileBrowser>(target)
                .map_or(0, FileBrowser::visible_count);
            if count > 0 {
                let index = if key == UiKeyCode::Home { 0 } else { count - 1 };
                if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                    browser.select(index, modifiers.control, modifiers.shift);
                }
                ensure_file_cursor_visible(world, target, view, height);
            }
        }
        _ => {}
    }
}

const NUMERIC_SCRUB_LOCK_DISTANCE: f64 = 3.0;

fn numeric_target(field: &NumericField) -> NumericTarget {
    NumericTarget {
        start: field.value,
        min: field.min.unwrap_or(-f64::MAX),
        max: field.max.unwrap_or(f64::MAX),
        step: field.step,
    }
}

fn slider_track_x(rect: Rect, theme: &Theme) -> (f32, f32) {
    let padding = theme.spacing.control.padding_x.max(0.0);
    let handle_radius = theme.spacing.editor.handle_radius;
    let handle_radius = if handle_radius.is_finite() {
        handle_radius.clamp(3.0, 9.0)
    } else {
        6.0
    };
    let left = rect.x + padding + handle_radius;
    let right = rect.x + rect.width - padding - handle_radius;
    if left <= right {
        (left, right)
    } else {
        let center = rect.x + rect.width * 0.5;
        (center, center)
    }
}

fn slider_value_at_x(x: f32, left: f32, right: f32) -> f64 {
    if right <= left {
        0.5
    } else {
        f64::from(((x - left) / (right - left)).clamp(0.0, 1.0))
    }
}

fn begin_slider_drag(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let Some(slider) = world.get::<Slider>(target).cloned() else {
        return;
    };
    if !slider.enabled || slider.read_only {
        return;
    }
    let (Some(min), Some(max)) = (slider.min, slider.max) else {
        return;
    };
    if !min.is_finite() || !max.is_finite() || !(max - min).is_finite() {
        return;
    }
    let rect = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
        .or_else(|| {
            world
                .get::<UiHitBox>(target)
                .map(|hit| Rect::new(hit.rect.x, hit.rect.y, hit.rect.width, hit.rect.height))
        });
    let Some(rect) = rect else {
        return;
    };
    let base_theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let theme = styled_theme(world, target, &base_theme);
    let (track_left, track_right) = slider_track_x(rect, &theme);
    let before = slider.value;
    if let Some(mut slider) = world.get_mut::<Slider>(target) {
        let _ =
            slider.set_normalized_value(slider_value_at_x(position[0], track_left, track_right));
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .slider_drags
        .insert(
            window,
            SliderDrag {
                target,
                before,
                min,
                max,
                track_left,
                track_right,
            },
        );
}

fn update_slider_drag(world: &mut World, window: Entity, x: f32) {
    let Some(drag) = world
        .resource::<WidgetInteractionState>()
        .slider_drags
        .get(&window)
        .copied()
    else {
        return;
    };
    if let Some(mut slider) = world.get_mut::<Slider>(drag.target)
        && slider.enabled
        && !slider.read_only
        && slider.min == Some(drag.min)
        && slider.max == Some(drag.max)
    {
        let _ =
            slider.set_normalized_value(slider_value_at_x(x, drag.track_left, drag.track_right));
    }
}

fn finish_slider_drag(
    world: &mut World,
    drag: SliderDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(mut slider) = world.get_mut::<Slider>(drag.target) else {
        return;
    };
    if cancelled {
        let _ = slider.restore_value(drag.before);
        emitted.push(WidgetEvent::NumericCancelled {
            target: drag.target,
            restored: vec![drag.before],
        });
        return;
    }
    let after = slider.value;
    if drag.before != after {
        emitted.push(WidgetEvent::NumericCommitted {
            target: drag.target,
            before: vec![drag.before],
            after: vec![after],
        });
    }
}

fn cancel_slider_drag(world: &mut World, window: Entity, emitted: &mut Vec<WidgetEvent>) -> bool {
    let drag = world
        .resource_mut::<WidgetInteractionState>()
        .slider_drags
        .remove(&window);
    if let Some(drag) = drag {
        finish_slider_drag(world, drag, true, emitted);
        true
    } else {
        false
    }
}

fn begin_numeric_scrub(world: &mut World, window: Entity, target: Entity, position: [f32; 2]) {
    let Some(field) = world.get::<NumericField>(target).cloned() else {
        return;
    };
    if !field.enabled || field.read_only {
        return;
    }
    let target_data = numeric_target(&field);
    let Ok(gesture) = NumericGesture::begin(vec![target_data]) else {
        return;
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .numeric_scrubs
        .insert(
            window,
            NumericScrub {
                entity: target,
                targets: vec![target],
                target_specs: vec![target_data],
                starts: vec![field.value],
                last_values: vec![field.value],
                gesture,
                phase: NumericScrubPhase::Selecting,
                last_position: position,
                pending_x: 0.0,
                pending_moves: Vec::new(),
                kind: field.kind,
                unit: field.unit,
            },
        );
    world
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::Begin {
            window,
            target,
            hide_cursor: true,
            continuous: true,
        });
}

fn numeric_targets_crossed(
    world: &mut World,
    window: Entity,
    from: [f32; 2],
    to: [f32; 2],
    kind: NumericFieldKind,
    unit: Option<&str>,
) -> Vec<(Entity, NumericTarget)> {
    let delta_y = to[1] - from[1];
    if delta_y.abs() < f32::EPSILON {
        return Vec::new();
    }
    let min_y = from[1].min(to[1]);
    let max_y = from[1].max(to[1]);
    let mut query = world.query::<(Entity, &NumericField, &UiHitBox)>();
    let mut crossed: Vec<_> = query
        .iter(world)
        .filter_map(|(entity, field, hit)| {
            if field.kind != kind
                || field.unit.as_deref() != unit
                || !field.enabled
                || field.read_only
                || hit.window.is_some_and(|hit_window| hit_window != window)
                || !is_interactable(world, entity)
            {
                return None;
            }
            let center_y = hit.rect.y + hit.rect.height * 0.5;
            if center_y < min_y || center_y > max_y {
                return None;
            }
            let t = (center_y - from[1]) / delta_y;
            let x = from[0] + (to[0] - from[0]) * t;
            let point = [x, center_y];
            if !hit.contains(point)
                || hit
                    .clip
                    .is_some_and(|clip| !clip.contains(point[0], point[1]))
            {
                return None;
            }
            Some((t, hit.z_index, entity, numeric_target(field)))
        })
        .collect();
    crossed.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.index().cmp(&b.2.index()))
    });
    crossed
        .into_iter()
        .map(|(_, _, entity, target)| (entity, target))
        .collect()
}

fn update_numeric_scrub(
    world: &mut World,
    window: Entity,
    position: [f32; 2],
    delta_x: f32,
    modifiers: UiModifiers,
) {
    let scrub_info = world
        .resource::<WidgetInteractionState>()
        .numeric_scrubs
        .get(&window)
        .map(|scrub| {
            (
                scrub.phase,
                scrub.last_position,
                scrub.kind,
                scrub.unit.clone(),
            )
        });
    let Some((phase, last_position, kind, unit)) = scrub_info else {
        return;
    };
    let additions = if phase == NumericScrubPhase::Selecting {
        numeric_targets_crossed(
            world,
            window,
            last_position,
            position,
            kind,
            unit.as_deref(),
        )
    } else {
        Vec::new()
    };
    let modifiers = NumericModifiers {
        precision: modifiers.shift,
        snap: modifiers.control,
    };
    let updates = {
        let mut state = world.resource_mut::<WidgetInteractionState>();
        let Some(scrub) = state.numeric_scrubs.get_mut(&window) else {
            return;
        };
        if scrub.phase == NumericScrubPhase::Selecting {
            for (entity, target) in additions {
                if !scrub.targets.contains(&entity) {
                    scrub.targets.push(entity);
                    scrub.target_specs.push(target);
                    scrub.starts.push(target.start);
                    scrub.last_values.push(target.start);
                    scrub.gesture = NumericGesture::begin(scrub.target_specs.clone())
                        .expect("eligible numeric fields have validated gesture targets");
                }
            }
        }
        scrub.last_position = position;
        if scrub.phase == NumericScrubPhase::Selecting {
            let dx = f64::from(delta_x);
            scrub.pending_x += dx;
            scrub.pending_moves.push((dx, modifiers));
            if scrub.pending_x.abs() >= NUMERIC_SCRUB_LOCK_DISTANCE {
                scrub.phase = NumericScrubPhase::Scrubbing;
                let mut values = scrub.last_values.clone();
                for (movement, segment_modifiers) in std::mem::take(&mut scrub.pending_moves) {
                    values = scrub.gesture.update(movement, segment_modifiers).to_vec();
                }
                Some(
                    scrub
                        .targets
                        .iter()
                        .copied()
                        .zip(values)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        } else {
            let values = scrub.gesture.update(f64::from(delta_x), modifiers).to_vec();
            Some(
                scrub
                    .targets
                    .iter()
                    .copied()
                    .zip(values)
                    .collect::<Vec<_>>(),
            )
        }
    };
    let Some(updates) = updates else {
        return;
    };
    let actual_values: Vec<_> = updates
        .into_iter()
        .map(|(entity, value)| {
            let actual = if let Some(mut field) = world.get_mut::<NumericField>(entity) {
                if field.enabled && !field.read_only {
                    let _ = field.set_value(value);
                }
                field.value
            } else {
                value
            };
            (entity, actual)
        })
        .collect();
    let mut state = world.resource_mut::<WidgetInteractionState>();
    if let Some(scrub) = state.numeric_scrubs.get_mut(&window) {
        for (index, entity) in scrub.targets.iter().enumerate() {
            if let Some((_, value)) = actual_values.iter().find(|(target, _)| target == entity) {
                scrub.last_values[index] = *value;
            }
        }
    }
}

fn finish_numeric_scrub(
    world: &mut World,
    window: Entity,
    scrub: NumericScrub,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    world
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::End {
            window,
            target: scrub.entity,
            restore_position: true,
        });
    if cancelled {
        for (entity, start) in scrub
            .targets
            .iter()
            .copied()
            .zip(scrub.starts.iter().copied())
        {
            if let Some(mut field) = world.get_mut::<NumericField>(entity) {
                field.value = start;
            }
        }
        emitted.push(WidgetEvent::NumericCancelled {
            target: scrub.entity,
            restored: scrub.starts,
        });
        return;
    }
    let changes: Vec<_> = scrub
        .targets
        .iter()
        .copied()
        .zip(scrub.starts.iter().copied())
        .filter_map(|(entity, before)| {
            let after = world.get::<NumericField>(entity)?.value;
            (before != after).then_some((entity, before, after))
        })
        .collect();
    match changes.as_slice() {
        [] => {}
        [(target, before, after)] => emitted.push(WidgetEvent::NumericCommitted {
            target: *target,
            before: vec![*before],
            after: vec![*after],
        }),
        _ => emitted.push(WidgetEvent::NumericGroupCommitted { changes }),
    }
}

fn cancel_numeric_scrub(world: &mut World, window: Entity, emitted: &mut Vec<WidgetEvent>) -> bool {
    let scrub = world
        .resource_mut::<WidgetInteractionState>()
        .numeric_scrubs
        .remove(&window);
    if let Some(scrub) = scrub {
        finish_numeric_scrub(world, window, scrub, true, emitted);
        true
    } else {
        false
    }
}

fn apply_checkbox_paint_at(world: &mut World, window: Entity, position: [f32; 2]) {
    let mut query = world.query::<(Entity, &UiHitBox, &Checkbox)>();
    let hits: Vec<_> = query
        .iter(world)
        .filter(|(entity, hit, checkbox)| {
            checkbox.enabled
                && !checkbox.read_only
                && hit.window.is_none_or(|hit_window| hit_window == window)
                && is_interactable(world, *entity)
                && hit.contains(position)
        })
        .map(|(entity, hit, _)| (hit.z_index, entity))
        .collect();
    let Some((_, target)) = hits
        .into_iter()
        .max_by_key(|(z, entity)| (*z, entity.index()))
    else {
        return;
    };
    let target_value = {
        let state = world.resource::<WidgetInteractionState>();
        let Some(transaction) = state.checkbox_paints.get(&window) else {
            return;
        };
        transaction.target
    };
    let should_visit = {
        let mut state = world.resource_mut::<WidgetInteractionState>();
        let Some(transaction) = state.checkbox_paints.get_mut(&window) else {
            return;
        };
        transaction.visited.insert(target)
    };
    if !should_visit {
        return;
    }
    let change = {
        let Some(mut checkbox) = world.get_mut::<Checkbox>(target) else {
            return;
        };
        if !checkbox.enabled || checkbox.read_only {
            return;
        }
        let before = checkbox.value;
        let after = if target_value {
            CheckboxValue::On
        } else {
            CheckboxValue::Off
        };
        checkbox.value = after;
        (before != after).then_some(CheckboxPaintChange {
            entity: target,
            before,
            after,
        })
    };
    if let Some(change) = change
        && let Some(transaction) = world
            .resource_mut::<WidgetInteractionState>()
            .checkbox_paints
            .get_mut(&window)
    {
        transaction.changes.push(change);
    }
}

fn restore_checkboxes(world: &mut World, changes: &[CheckboxPaintChange]) {
    for change in changes {
        if let Some(mut checkbox) = world.get_mut::<Checkbox>(change.entity) {
            checkbox.value = change.before;
        }
    }
}

fn cancel_gestures(world: &mut World, window: Entity, emitted: &mut Vec<WidgetEvent>) {
    let paint = {
        world
            .resource_mut::<WidgetInteractionState>()
            .checkbox_paints
            .remove(&window)
    };
    if let Some(paint) = paint {
        restore_checkboxes(world, &paint.changes);
        emitted.push(WidgetEvent::CheckboxPaintCancelled {
            changes: paint.changes,
        });
    }
    if let Some(scrub) = world
        .resource_mut::<WidgetInteractionState>()
        .numeric_scrubs
        .remove(&window)
    {
        finish_numeric_scrub(world, window, scrub, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .slider_drags
        .remove(&window)
    {
        finish_slider_drag(world, drag, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .color_ramp_drags
        .remove(&window)
    {
        finish_color_ramp_drag(world, drag, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .hsv_color_drags
        .remove(&window)
    {
        finish_hsv_color_drag(world, drag, true, emitted);
    }
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .curve_editor_drags
        .remove(&window)
    {
        finish_curve_editor_drag(world, drag, true, emitted);
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .collection_drags
        .remove(&window);
    if let Some(drag) = world
        .resource_mut::<WidgetInteractionState>()
        .data_table_resize_drags
        .remove(&window)
    {
        finish_data_table_resize(world, drag, true, emitted);
    }
    if let Some(gesture) = world
        .resource_mut::<WidgetInteractionState>()
        .panel_reorders
        .remove(&window)
    {
        finish_panel_reorder(world, gesture, true, emitted);
    }
}

fn activate_target(
    world: &mut World,
    target: Entity,
    click_count: u32,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(cell) = world.get::<DataTableCellControl>(target).cloned() {
        select_data_table_cell(world, &cell, false, false, 1, emitted);
    } else if let Some(owner) = data_table_owner(world, target) {
        if let Some((row_id, column)) = activate_data_table_cell(world, owner) {
            emitted.push(WidgetEvent::DataTableCellActivated {
                target: owner,
                row_id,
                column,
            });
        }
    } else if world.get::<HsvColorEditor>(target).is_some() {
        if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
            state.editing = true;
            state.edit_buffer.clear();
        }
        emitted.push(WidgetEvent::Activated { target });
    } else if world.get::<CurveEditor>(target).is_some() {
        emitted.push(WidgetEvent::Activated { target });
    } else if world.get::<MenuControl>(target).is_some() {
        toggle_menu(world, target);
    } else if let Some(menu_item) = world.get::<MenuItemControl>(target).cloned() {
        select_menu_item_for_release(world, &menu_item, emitted);
    } else if let Some(option) = world.get::<DropdownOptionControl>(target).cloned() {
        select_dropdown_option(world, option.owner, option.index, emitted);
    } else if let Some(option) = world.get::<RadioOptionControl>(target).cloned() {
        select_radio_option(world, option.owner, option.index, emitted);
    } else if let Some(file_item) = world.get::<FileBrowserItem>(target).cloned() {
        let window = world
            .get::<UiLayoutResult>(target)
            .map_or(Entity::PLACEHOLDER, |layout| layout.window);
        select_file_browser_item(world, &file_item, 1, window, emitted);
    } else if let Some(item) = world.get::<CollectionItemControl>(target).cloned() {
        edit_collection(world, item.owner, emitted, |world, owner| {
            if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                list.select(item.id, false, false)
            } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                tree.select(item.id, false, false)
            } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                grid.select(item.id, false, false)
            } else {
                false
            }
        });
    } else if let Some(mut button) = world.get_mut::<Button>(target) {
        if button.activate() {
            emitted.push(WidgetEvent::Activated { target });
        }
    } else if let Some(mut button) = world.get_mut::<IconButton>(target) {
        if button.activate() {
            emitted.push(WidgetEvent::Activated { target });
        }
    } else if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
        let before = checkbox.value == CheckboxValue::On;
        if checkbox.toggle() {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Bool(before),
                after: WidgetValue::Bool(checkbox.is_checked()),
            });
        }
    } else if let Some(mut panel) = world.get_mut::<Panel>(target) {
        let before = panel.state.target_expanded;
        if panel.toggle() {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Bool(before),
                after: WidgetValue::Bool(panel.state.target_expanded),
            });
        }
    } else if let Some(mut dropdown) = world.get_mut::<Dropdown>(target) {
        if dropdown.open {
            dropdown.close();
        } else {
            dropdown.open();
        }
    } else if let Some(mut field) = world.get_mut::<NumericField>(target)
        && click_count >= 2
    {
        field.begin_edit();
    }
}

fn handle_slider_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(mut slider) = world.get_mut::<Slider>(target) else {
        return;
    };
    if !slider.enabled || slider.read_only {
        return;
    }
    let before = slider.value;
    let changed = match key {
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowDown
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::PageUp
        | UiKeyCode::PageDown => {
            let direction = match key {
                UiKeyCode::ArrowLeft | UiKeyCode::ArrowDown | UiKeyCode::PageDown => -1.0,
                _ => 1.0,
            };
            let multiplier =
                if matches!(key, UiKeyCode::PageUp | UiKeyCode::PageDown) || modifiers.control {
                    10.0
                } else if modifiers.shift && slider.kind == NumericFieldKind::Float {
                    0.1
                } else {
                    1.0
                };
            let step = slider.step;
            slider
                .set_value(before + direction * step * multiplier)
                .unwrap_or(false)
        }
        UiKeyCode::Home => slider
            .min
            .is_some_and(|min| slider.set_value(min).unwrap_or(false)),
        UiKeyCode::End => slider
            .max
            .is_some_and(|max| slider.set_value(max).unwrap_or(false)),
        UiKeyCode::Backspace => slider.reset(),
        _ => return,
    };
    if changed {
        emitted.push(WidgetEvent::ValueChanged {
            target,
            before: WidgetValue::Number(before),
            after: WidgetValue::Number(slider.value),
        });
    }
}

fn handle_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if world.get::<Panel>(target).is_some()
        && (modifiers.control || modifiers.super_key)
        && matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown)
    {
        reorder_panel_step(
            world,
            target,
            if key == UiKeyCode::ArrowUp { -1 } else { 1 },
            emitted,
        );
        return;
    }
    if let Some(owner) = data_table_owner(world, target) {
        handle_data_table_key(world, owner, key, modifiers, emitted);
        return;
    }
    if let Some(owner) = collection_owner(world, target) {
        handle_collection_key(world, owner, key, modifiers, emitted);
        return;
    }
    if world.get::<HsvColorEditor>(target).is_some() {
        handle_hsv_color_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<CurveEditor>(target).is_some() {
        handle_curve_editor_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<ColorRamp>(target).is_some() {
        handle_color_ramp_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<FileBrowser>(target).is_some() {
        handle_file_browser_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<Slider>(target).is_some() {
        handle_slider_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<TextBuffer>(target).is_some() {
        if modifiers.control || modifiers.super_key {
            match key {
                UiKeyCode::C => {
                    if let Some(buffer) = world.get::<TextBuffer>(target)
                        && let Some(text) = buffer.copy()
                    {
                        emitted.push(WidgetEvent::ClipboardWrite { target, text });
                    }
                }
                UiKeyCode::X if world.get::<UiReadOnly>(target).is_none() => {
                    if let Some(mut buffer) = world.get_mut::<TextBuffer>(target)
                        && let Some(text) = buffer.cut()
                    {
                        emitted.push(WidgetEvent::ClipboardWrite { target, text });
                        emitted.push(WidgetEvent::TextChanged {
                            target,
                            text: buffer.as_str().to_owned(),
                        });
                    }
                }
                UiKeyCode::V if world.get::<UiReadOnly>(target).is_none() => {
                    emitted.push(WidgetEvent::ClipboardRead { target });
                }
                _ => {}
            }
        } else if matches!(
            key,
            UiKeyCode::Backspace | UiKeyCode::Delete | UiKeyCode::Enter
        ) && let Some(buffer) = world.get::<TextBuffer>(target)
        {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: buffer.as_str().to_owned(),
            });
        }
        return;
    }
    if (key == UiKeyCode::Space || key == UiKeyCode::Enter)
        && world.get::<NumericField>(target).is_none()
        && world.get::<TextField>(target).is_none()
        && world.get::<TextArea>(target).is_none()
        && world.get::<Dropdown>(target).is_none()
    {
        if let Some(mut radio) = world.get_mut::<RadioGroup>(target) {
            let before = radio.selected;
            radio.move_selection(1);
            if before != radio.selected {
                emitted.push(WidgetEvent::ValueChanged {
                    target,
                    before: WidgetValue::Option(before),
                    after: WidgetValue::Option(radio.selected),
                });
            }
        } else {
            activate_target(world, target, 1, emitted);
        }
        return;
    }
    let expression_variables = if key == UiKeyCode::Enter {
        world
            .get::<NumericExpressionVariables>(target)
            .map(|variables| variables.0.clone())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    if let Some(mut field) = world.get_mut::<NumericField>(target) {
        match key {
            UiKeyCode::A if modifiers.control || modifiers.super_key => {
                field.edit_select_all();
            }
            UiKeyCode::ArrowUp | UiKeyCode::ArrowDown => {
                let sign = if key == UiKeyCode::ArrowUp { 1.0 } else { -1.0 };
                let factor = if modifiers.control {
                    10.0
                } else if modifiers.shift && field.kind == NumericFieldKind::Float {
                    0.1
                } else {
                    1.0
                };
                let before = field.value;
                let step = field.step;
                let _ = field.set_value(before + step * sign * factor);
                if before != field.value {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            }
            UiKeyCode::Backspace if field.editing.is_none() => {
                let before = field.value;
                field.reset();
                if before != field.value {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            }
            UiKeyCode::Backspace if field.editing.is_some() => field.edit_backspace(),
            UiKeyCode::Escape => field.cancel_edit(),
            UiKeyCode::Enter if field.editing.is_some() => {
                let before = field.value;
                match field.commit_edit(&expression_variables) {
                    Ok(true) => emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    }),
                    Ok(false) => {}
                    Err(error) => emitted.push(WidgetEvent::NumericExpressionError {
                        target,
                        error: error.to_string(),
                    }),
                }
            }
            UiKeyCode::C if modifiers.control => emitted.push(WidgetEvent::ClipboardWrite {
                target,
                text: field.clipboard_value(),
            }),
            UiKeyCode::V if modifiers.control => {
                emitted.push(WidgetEvent::ClipboardRead { target })
            }
            _ => {}
        }
        return;
    }
    if let Some(mut field) = world.get_mut::<TextField>(target) {
        if modifiers.control && key == UiKeyCode::C {
            emitted.push(WidgetEvent::ClipboardWrite {
                target,
                text: field.selected_text().unwrap_or_else(|| field.text.clone()),
            });
            return;
        }
        if modifiers.control && key == UiKeyCode::V {
            emitted.push(WidgetEvent::ClipboardRead { target });
            return;
        }
        handle_text_key(&mut field, key, modifiers, target, emitted);
        return;
    }
    if let Some(mut field) = world.get_mut::<TextArea>(target) {
        if modifiers.control && key == UiKeyCode::C {
            emitted.push(WidgetEvent::ClipboardWrite {
                target,
                text: field
                    .field
                    .selected_text()
                    .unwrap_or_else(|| field.field.text.clone()),
            });
            return;
        }
        if modifiers.control && key == UiKeyCode::V {
            emitted.push(WidgetEvent::ClipboardRead { target });
            return;
        }
        if matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown) {
            let before = field.field.text.clone();
            let current = field.field.caret;
            let prefix: String = field.field.text.chars().take(current).collect();
            let line_start = prefix
                .rfind('\n')
                .map_or(0, |byte| prefix[..byte].chars().count() + 1);
            let column = current.saturating_sub(line_start);
            field.move_vertical(if key == UiKeyCode::ArrowUp { -1 } else { 1 }, column);
            if before != field.field.text {
                emitted.push(WidgetEvent::TextChanged {
                    target,
                    text: field.field.text.clone(),
                });
            }
        } else {
            handle_text_key(&mut field.field, key, modifiers, target, emitted);
        }
        return;
    }
    if world.get::<Dropdown>(target).is_some() {
        let page = world
            .get::<DropdownPopupState>(target)
            .map_or(8, |state| state.visible_count.max(1))
            .min(i32::MAX as usize) as i32;
        let Some(mut dropdown) = world.get_mut::<Dropdown>(target) else {
            return;
        };
        match key {
            UiKeyCode::Space => {
                if dropdown.open {
                    dropdown.close();
                } else {
                    dropdown.open();
                }
            }
            UiKeyCode::ArrowDown => {
                if !dropdown.open {
                    dropdown.open();
                } else {
                    dropdown.move_highlight(1);
                }
            }
            UiKeyCode::ArrowUp => {
                if !dropdown.open {
                    dropdown.open();
                } else {
                    dropdown.move_highlight(-1);
                }
            }
            UiKeyCode::PageDown if dropdown.open => {
                dropdown.move_highlight(page);
            }
            UiKeyCode::PageUp if dropdown.open => {
                dropdown.move_highlight(-page);
            }
            UiKeyCode::Home if dropdown.open && !dropdown.options.is_empty() => {
                dropdown.highlighted = Some(0);
            }
            UiKeyCode::End if dropdown.open && !dropdown.options.is_empty() => {
                dropdown.highlighted = Some(dropdown.options.len() - 1);
            }
            UiKeyCode::Escape => dropdown.close(),
            UiKeyCode::Enter => {
                if !dropdown.open {
                    dropdown.open();
                } else if let Some(index) = dropdown.highlighted {
                    let before = dropdown.selected;
                    if dropdown.choose(index) {
                        emitted.push(WidgetEvent::ValueChanged {
                            target,
                            before: WidgetValue::Option(before),
                            after: WidgetValue::Option(dropdown.selected),
                        });
                    }
                }
            }
            _ => {}
        }
        return;
    }
    if let Some(mut radio) = world.get_mut::<RadioGroup>(target) {
        let before = radio.selected;
        match key {
            UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => {
                radio.move_selection(1);
            }
            UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => {
                radio.move_selection(-1);
            }
            _ => {}
        }
        if before != radio.selected {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Option(before),
                after: WidgetValue::Option(radio.selected),
            });
        }
    }
}

fn handle_curve_editor_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    if (modifiers.control || modifiers.super_key)
        && matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
    {
        if let Some(mut editor) = world.get_mut::<CurveEditor>(target)
            && !editor.points.is_empty()
        {
            let current = editor
                .selected
                .and_then(|id| editor.points.iter().position(|point| point.id == id));
            let index = match (current, key) {
                (Some(index), UiKeyCode::ArrowRight) => (index + 1) % editor.points.len(),
                (Some(index), UiKeyCode::ArrowLeft) => {
                    (index + editor.points.len() - 1) % editor.points.len()
                }
                (None, UiKeyCode::ArrowRight) => 0,
                (None, UiKeyCode::ArrowLeft) => editor.points.len() - 1,
                _ => return,
            };
            editor.selected = Some(editor.points[index].id);
        }
        return;
    }
    match key {
        UiKeyCode::A => edit_curve_editor(world, target, emitted, add_curve_point_near_selection),
        UiKeyCode::Delete | UiKeyCode::Backspace => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.selected.is_some_and(|id| editor.remove_point(id))
            });
        }
        UiKeyCode::C => edit_curve_editor(world, target, emitted, |editor| {
            editor.interpolation = match editor.interpolation {
                Interpolation::Constant => Interpolation::Linear,
                Interpolation::Linear => Interpolation::Smooth,
                Interpolation::Smooth => Interpolation::Constant,
            };
            true
        }),
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowDown
        | UiKeyCode::Home
        | UiKeyCode::End => {
            let step = if modifiers.control || modifiers.super_key {
                0.001
            } else if modifiers.shift {
                0.1
            } else {
                0.01
            };
            edit_curve_editor(world, target, emitted, |editor| {
                let Some(point) = editor
                    .selected
                    .and_then(|id| editor.points.iter().find(|point| point.id == id))
                    .or_else(|| editor.points.first())
                    .copied()
                else {
                    return false;
                };
                editor.selected = Some(point.id);
                let mut position = point.position;
                match key {
                    UiKeyCode::ArrowLeft => position[0] -= step,
                    UiKeyCode::ArrowRight => position[0] += step,
                    UiKeyCode::ArrowUp => position[1] += step,
                    UiKeyCode::ArrowDown => position[1] -= step,
                    UiKeyCode::Home => position[0] = 0.0,
                    UiKeyCode::End => position[0] = 1.0,
                    _ => return false,
                }
                editor.move_point(point.id, position)
            });
        }
        _ => {}
    }
}

fn add_curve_point_near_selection(editor: &mut CurveEditor) -> bool {
    let pair = editor
        .selected
        .and_then(|id| editor.points.iter().position(|point| point.id == id))
        .and_then(|index| {
            if index + 1 < editor.points.len() {
                Some((editor.points[index], editor.points[index + 1]))
            } else if index > 0 {
                Some((editor.points[index - 1], editor.points[index]))
            } else {
                None
            }
        })
        .or_else(|| {
            editor
                .points
                .first()
                .zip(editor.points.last())
                .map(|(a, b)| (*a, *b))
        });
    let Some((left, right)) = pair else {
        return false;
    };
    let x = (left.position[0] + right.position[0]) * 0.5;
    editor.add_point(x, editor.evaluate(x)).is_some()
}

fn handle_hsv_color_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    let state = world
        .get::<HsvColorEditorState>(target)
        .cloned()
        .unwrap_or_default();
    if state.editing {
        match key {
            UiKeyCode::Backspace => {
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.edit_buffer.pop();
                }
            }
            UiKeyCode::Delete => {
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.edit_buffer.clear();
                }
            }
            UiKeyCode::Enter => {
                let field = state.active_field;
                let buffer = state.edit_buffer;
                edit_hsv_color(world, target, emitted, |editor| {
                    set_hsv_numeric_field(editor, field, &buffer)
                });
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.editing = false;
                    state.edit_buffer.clear();
                }
            }
            UiKeyCode::Escape => {
                if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
                    state.editing = false;
                    state.edit_buffer.clear();
                }
            }
            _ => {}
        }
        return;
    }
    if matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown) {
        if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
            let index = HsvColorField::ALL
                .iter()
                .position(|field| *field == state.active_field)
                .unwrap_or(0);
            let delta = if key == UiKeyCode::ArrowDown { 1 } else { -1 };
            state.active_field = HsvColorField::ALL
                [(index as i32 + delta).rem_euclid(HsvColorField::ALL.len() as i32) as usize];
        }
        return;
    }
    if key == UiKeyCode::Enter || key == UiKeyCode::Space {
        let editor = world.get::<HsvColorEditor>(target).copied();
        if let Some(editor) = editor
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.editing = true;
            state.edit_buffer = hsv_field_input_text(editor, state.active_field);
        }
        return;
    }
    if matches!(key, UiKeyCode::Backspace | UiKeyCode::Delete) {
        let editor = world.get::<HsvColorEditor>(target).copied();
        if let Some(editor) = editor
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.editing = true;
            state.edit_buffer = hsv_field_input_text(editor, state.active_field);
            state.edit_buffer.pop();
        }
        return;
    }
    let field = state.active_field;
    let Some(current) = world
        .get::<HsvColorEditor>(target)
        .and_then(|editor| hsv_numeric_field_value(*editor, field))
    else {
        return;
    };
    let step = if modifiers.control || modifiers.super_key {
        10.0
    } else if modifiers.shift {
        0.1
    } else {
        1.0
    };
    let next = match key {
        UiKeyCode::ArrowRight | UiKeyCode::ArrowUp => Some(current + step),
        UiKeyCode::ArrowLeft | UiKeyCode::ArrowDown => Some(current - step),
        UiKeyCode::Home => Some(0.0),
        UiKeyCode::End => Some(match field {
            HsvColorField::Hue => 359.0,
            HsvColorField::Saturation | HsvColorField::Value => 100.0,
            HsvColorField::Red
            | HsvColorField::Green
            | HsvColorField::Blue
            | HsvColorField::Alpha => 255.0,
            HsvColorField::Hex => return,
        }),
        _ => None,
    };
    if let Some(next) = next {
        edit_hsv_color(world, target, emitted, |editor| {
            set_hsv_numeric_field(editor, field, &next.to_string())
        });
    }
}

fn handle_color_ramp_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    let state = world
        .get::<ColorRampEditorState>(target)
        .copied()
        .unwrap_or_default();
    if key == UiKeyCode::C || key == UiKeyCode::Space {
        if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
            state.editing_color = !state.editing_color;
        }
        return;
    }
    if key == UiKeyCode::Enter && state.editing_color {
        if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
            state.editing_color = false;
        }
        return;
    }
    if key == UiKeyCode::A {
        edit_color_ramp(world, target, emitted, |ramp| {
            let position = suggested_stop_position(ramp);
            ramp.add_stop(position, ramp.sample(position)).is_some()
        });
        return;
    }
    if matches!(key, UiKeyCode::Delete | UiKeyCode::Backspace) {
        edit_color_ramp(world, target, emitted, |ramp| {
            ramp.selected.is_some_and(|id| ramp.remove_stop(id))
        });
        return;
    }
    if !state.editing_color && key == UiKeyCode::Enter {
        edit_color_ramp(world, target, emitted, |ramp| {
            let next = match ramp.interpolation {
                Interpolation::Constant => Interpolation::Linear,
                Interpolation::Linear => Interpolation::Smooth,
                Interpolation::Smooth => Interpolation::Constant,
            };
            ramp.set_interpolation(next)
        });
        return;
    }
    if state.editing_color {
        if matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown) {
            if let Some(mut editor) = world.get_mut::<ColorRampEditorState>(target) {
                let delta = if key == UiKeyCode::ArrowDown { 1 } else { -1 };
                editor.active_channel = ColorChannel::from_index(
                    (editor.active_channel.index() as i32 + delta).rem_euclid(4) as usize,
                );
            }
            return;
        }
        let step = if modifiers.control || modifiers.super_key {
            0.1
        } else if modifiers.shift {
            0.001
        } else {
            0.01
        };
        let direction = match key {
            UiKeyCode::ArrowRight | UiKeyCode::ArrowUp => Some(1.0),
            UiKeyCode::ArrowLeft | UiKeyCode::ArrowDown => Some(-1.0),
            _ => None,
        };
        if let Some(direction) = direction {
            let channel = state.active_channel;
            edit_color_ramp(world, target, emitted, |ramp| {
                let selected = ramp.selected_stop().map(|stop| (stop.id, stop.color));
                selected.is_some_and(|(stop_id, color)| {
                    let current = match channel {
                        ColorChannel::Red => color.0,
                        ColorChannel::Green => color.1,
                        ColorChannel::Blue => color.2,
                        ColorChannel::Alpha => color.3,
                    };
                    ramp.set_color_channel(
                        stop_id,
                        channel,
                        (current + direction * step).clamp(0.0, 1.0),
                    )
                })
            });
        } else if matches!(key, UiKeyCode::Home | UiKeyCode::End) {
            let channel = state.active_channel;
            let value = if key == UiKeyCode::Home { 0.0 } else { 1.0 };
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected
                    .is_some_and(|id| ramp.set_color_channel(id, channel, value))
            });
        }
        return;
    }
    match key {
        UiKeyCode::ArrowUp => {
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_relative(-1);
            }
        }
        UiKeyCode::ArrowDown => {
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_relative(1);
            }
        }
        UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight => {
            let step = if modifiers.control || modifiers.super_key {
                0.1
            } else if modifiers.shift {
                0.001
            } else {
                0.01
            };
            let delta = if key == UiKeyCode::ArrowLeft {
                -step
            } else {
                step
            };
            edit_color_ramp(world, target, emitted, |ramp| ramp.move_selected_by(delta));
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let position = if key == UiKeyCode::Home { 0.0 } else { 1.0 };
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected.is_some_and(|id| ramp.move_stop(id, position))
            });
        }
        _ => {}
    }
}

fn handle_text_key(
    field: &mut TextField,
    key: UiKeyCode,
    modifiers: UiModifiers,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let before = field.text.clone();
    match key {
        UiKeyCode::ArrowLeft => field.move_caret(-1, modifiers.shift),
        UiKeyCode::ArrowRight => field.move_caret(1, modifiers.shift),
        UiKeyCode::Home => field.select(0..0),
        UiKeyCode::End => {
            let end = field.text.chars().count();
            field.select(end..end);
        }
        UiKeyCode::A if modifiers.control => field.select_all(),
        UiKeyCode::Backspace => {
            field.backspace();
        }
        UiKeyCode::Delete => {
            if let Some(range) = field.selection.clone() {
                field.select(range);
                field.backspace();
            } else if field.caret < field.text.chars().count() {
                field.select(field.caret..field.caret + 1);
                field.backspace();
            }
        }
        _ => return,
    }
    if before != field.text {
        emitted.push(WidgetEvent::TextChanged {
            target,
            text: field.text.clone(),
        });
    }
}

fn insert_text(world: &mut World, target: Entity, text: &str, emitted: &mut Vec<WidgetEvent>) {
    if world.get::<TextBuffer>(target).is_some() {
        if world.get::<UiReadOnly>(target).is_some() {
            return;
        }
        if let Some(mut buffer) = world.get_mut::<TextBuffer>(target)
            && buffer.insert_text(text)
        {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: buffer.as_str().to_owned(),
            });
        }
    } else if let Some(mut field) = world.get_mut::<TextField>(target) {
        if field.insert(text) {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: field.text.clone(),
            });
        }
    } else if let Some(mut field) = world.get_mut::<TextArea>(target) {
        if field.insert_text(text) {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: field.field.text.clone(),
            });
        }
    } else if let Some(mut field) = world.get_mut::<NumericField>(target) {
        field.edit_insert(text);
    }
}

/// Apply actions requested by application code through `WidgetActionQueue`.
pub fn apply_widget_actions(world: &mut World) {
    let actions: Vec<_> = world
        .get_resource_mut::<WidgetActionQueue>()
        .map(|mut queue| queue.drain().collect())
        .unwrap_or_default();
    let mut emitted = Vec::new();
    for action in actions {
        apply_action(world, action, &mut emitted);
    }
    world.resource_mut::<WidgetEventQueue>().0.extend(emitted);
}

fn apply_action(world: &mut World, action: WidgetAction, emitted: &mut Vec<WidgetEvent>) {
    let target = action.target;
    match action.action {
        WidgetActionKind::Activate => activate_target(world, target, 1, emitted),
        WidgetActionKind::Toggle => {
            if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
                let before = checkbox.is_checked();
                if checkbox.toggle() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Bool(before),
                        after: WidgetValue::Bool(checkbox.is_checked()),
                    });
                }
            } else if let Some(mut panel) = world.get_mut::<Panel>(target) {
                let before = panel.state.target_expanded;
                if panel.toggle() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Bool(before),
                        after: WidgetValue::Bool(panel.state.target_expanded),
                    });
                }
            }
        }
        WidgetActionKind::SetBool(value) => {
            if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
                let before = checkbox.is_checked();
                if checkbox.set(value) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Bool(before),
                        after: WidgetValue::Bool(value),
                    });
                }
            }
        }
        WidgetActionKind::SetNumber(value) => {
            if let Some(mut field) = world.get_mut::<NumericField>(target) {
                let before = field.value;
                if field.set_value(value).unwrap_or(false) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            } else if let Some(mut slider) = world.get_mut::<Slider>(target) {
                let before = slider.value;
                if slider.set_value(value).unwrap_or(false) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(slider.value),
                    });
                }
            }
        }
        WidgetActionKind::StepNumber(steps) => {
            if let Some(mut field) = world.get_mut::<NumericField>(target) {
                let before = field.value;
                if field.step_by(steps) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            } else if let Some(mut slider) = world.get_mut::<Slider>(target) {
                let before = slider.value;
                if slider.step_by(steps) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(slider.value),
                    });
                }
            }
        }
        WidgetActionKind::ResetNumber => {
            if let Some(mut field) = world.get_mut::<NumericField>(target) {
                let before = field.value;
                if field.reset() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            } else if let Some(mut slider) = world.get_mut::<Slider>(target) {
                let before = slider.value;
                if slider.reset() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(slider.value),
                    });
                }
            }
        }
        WidgetActionKind::InsertText(text) => insert_text(world, target, &text, emitted),
        WidgetActionKind::ChooseOption(index) => {
            if let Some(option) = world.get::<DropdownOptionControl>(target).cloned() {
                select_dropdown_option(world, option.owner, option.index, emitted);
            } else if let Some(option) = world.get::<RadioOptionControl>(target).cloned() {
                select_radio_option(world, option.owner, option.index, emitted);
            } else if world.get::<Dropdown>(target).is_some() {
                select_dropdown_option(world, target, index, emitted);
            } else if let Some(mut radio) = world.get_mut::<RadioGroup>(target) {
                let before = radio.selected;
                if radio.select(index) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Option(before),
                        after: WidgetValue::Option(radio.selected),
                    });
                }
            }
        }
        WidgetActionKind::FileNavigateUp => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.up() {
                    Ok(true) => emitted.push(WidgetEvent::FileDirectoryChanged {
                        target,
                        path: browser.directory.clone(),
                    }),
                    Ok(false) => {}
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileNavigateTo(path) => {
            navigate_file_browser(world, target, path, emitted);
        }
        WidgetActionKind::FileRefresh => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let directory = browser.directory.clone();
                match browser.refresh() {
                    Ok(()) => emitted.push(WidgetEvent::FileDirectoryChanged {
                        target,
                        path: directory,
                    }),
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileSetFilter(filter) => {
            // This action mirrors edits from an external TextField. Its local TextBuffer owns
            // text undo, so browser snapshots deliberately do not create one transaction per key.
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.set_filter(filter);
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        WidgetActionKind::FileSetView(view) => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.view = view;
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileSetSort(sort, ascending) => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.set_sort(sort, ascending);
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileOpenIndex(index) => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target)
                && browser.select(index, false, false)
            {
                match browser.open_cursor() {
                    Ok(Some(path)) => emitted.push(WidgetEvent::FileActivated { target, path }),
                    Ok(None) => emitted.push(WidgetEvent::FileDirectoryChanged {
                        target,
                        path: browser.directory.clone(),
                    }),
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileBookmarkCurrent => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let directory = browser.directory.clone();
                match browser.bookmark(directory) {
                    Ok(true) => emitted.push(WidgetEvent::FileBookmarksChanged {
                        target,
                        bookmarks: browser.bookmarks.clone(),
                    }),
                    Ok(false) => {}
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileRemoveBookmarkCurrent => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let directory = browser.directory.clone();
                if browser.remove_bookmark(directory) {
                    emitted.push(WidgetEvent::FileBookmarksChanged {
                        target,
                        bookmarks: browser.bookmarks.clone(),
                    });
                }
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileCreateDirectory(name) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.create_directory(&name) {
                    Ok(path) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "create directory",
                            paths: vec![path],
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileRenameSelected(name) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target)
                && let Some(path) = browser.selected.iter().next().cloned()
            {
                match browser.rename(path, &name) {
                    Ok(destination) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "rename",
                            paths: vec![destination],
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileDeleteSelected => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let requested: Vec<_> = browser.selected.iter().cloned().collect();
                let present_before: HashSet<_> = requested
                    .iter()
                    .filter(|path| std::fs::symlink_metadata(path).is_ok())
                    .cloned()
                    .collect();
                match browser.delete_selected() {
                    Ok(_) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "delete",
                            paths: requested,
                            irreversible: true,
                        });
                    }
                    Err(error) => {
                        let partial_effects = present_before
                            .into_iter()
                            .filter(|path| {
                                std::fs::symlink_metadata(path).is_err_and(|error| {
                                    error.kind() == std::io::ErrorKind::NotFound
                                })
                            })
                            .collect();
                        emit_file_error_with_effects(target, error, partial_effects, emitted);
                    }
                }
            }
        }
        WidgetActionKind::FileCopySelectedTo(directory) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.copy_selected_to(directory) {
                    Ok(paths) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "copy",
                            paths,
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileMoveSelectedTo(directory) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.move_selected_to(directory) {
                    Ok(paths) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "move",
                            paths,
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::TogglePanel => {
            if let Some(mut panel) = world.get_mut::<Panel>(target) {
                let before = panel.state.target_expanded;
                if panel.toggle() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Bool(before),
                        after: WidgetValue::Bool(panel.state.target_expanded),
                    });
                }
            }
        }
        WidgetActionKind::PanelReorderStep(delta) => {
            reorder_panel_step(world, target, delta, emitted);
        }
        WidgetActionKind::ChooseMenuCommand(command_id) => {
            let restore_key = if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                && menu.tree.flattened().iter().any(|(_, _, entry)| {
                    entry.id == command_id && entry.enabled && !entry.separator
                }) {
                menu.search.pick(command_id.clone());
                let restore_key = menu.session.focus_restore_key.clone();
                menu.close();
                menu.popup.open = false;
                Some(restore_key)
            } else {
                None
            };
            if let Some(restore_key) = restore_key {
                restore_menu_focus(world, target, restore_key);
                emitted.push(WidgetEvent::MenuCommand { target, command_id });
            }
        }
        WidgetActionKind::ClearMenuHistory => {
            if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
                menu.clear_search_history();
            }
        }
        WidgetActionKind::ColorRampAddStop { position } => {
            edit_color_ramp(world, target, emitted, |ramp| {
                let position = position.clamp(0.0, 1.0);
                ramp.add_stop(position, ramp.sample(position)).is_some()
            });
        }
        WidgetActionKind::ColorRampRemoveSelectedStop => {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected.is_some_and(|id| ramp.remove_stop(id))
            });
        }
        WidgetActionKind::ColorRampSelectStop(id) => {
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_stop(id);
            }
        }
        WidgetActionKind::ColorRampMoveStop { id, position } => {
            edit_color_ramp(world, target, emitted, |ramp| ramp.move_stop(id, position));
        }
        WidgetActionKind::ColorRampSetChannel { channel, value } => {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected
                    .is_some_and(|id| ramp.set_color_channel(id, channel, value))
            });
        }
        WidgetActionKind::ColorRampSetInterpolation(interpolation) => {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.set_interpolation(interpolation)
            });
        }
        WidgetActionKind::HsvColorSet(color) => {
            edit_hsv_color(world, target, emitted, |editor| editor.set_rgba(color));
        }
        WidgetActionKind::HsvColorSetHsv { field, value } => {
            edit_hsv_color(world, target, emitted, |editor| {
                editor.set_hsv(field, value)
            });
        }
        WidgetActionKind::HsvColorSetRgb { field, value } => {
            edit_hsv_color(world, target, emitted, |editor| {
                editor.set_rgb(field, value)
            });
        }
        WidgetActionKind::HsvColorSetHex(hex) => {
            edit_hsv_color(world, target, emitted, |editor| {
                editor.set_hex(&hex).unwrap_or(false)
            });
        }
        WidgetActionKind::CurveEditorAddPoint { position } => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.add_point(position[0], position[1]).is_some()
            });
        }
        WidgetActionKind::CurveEditorRemoveSelectedPoint => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.selected.is_some_and(|id| editor.remove_point(id))
            });
        }
        WidgetActionKind::CurveEditorSelectPoint(id) => {
            if let Some(mut editor) = world.get_mut::<CurveEditor>(target) {
                editor.select_point(id);
            }
        }
        WidgetActionKind::CurveEditorMovePoint { id, position } => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.move_point(id, position)
            });
        }
        WidgetActionKind::CurveEditorSetInterpolation(interpolation) => {
            edit_curve_editor(world, target, emitted, |editor| {
                if editor.interpolation == interpolation {
                    false
                } else {
                    editor.interpolation = interpolation;
                    true
                }
            });
        }
        WidgetActionKind::CurveEditorResetView => {
            edit_curve_editor(world, target, emitted, |editor| {
                if editor.pan == [0.0, 0.0] && editor.zoom == 1.0 {
                    false
                } else {
                    editor.reset_view();
                    true
                }
            });
        }
        WidgetActionKind::CollectionSelect { id, ctrl, shift } => {
            edit_collection(world, target, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    let before = list.selection.clone();
                    list.select(id, ctrl, shift);
                    before != list.selection
                } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                    let before = tree.selection.clone();
                    tree.select(id, ctrl, shift);
                    before != tree.selection
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    let before = grid.selection.clone();
                    grid.select(id, ctrl, shift);
                    before != grid.selection
                } else {
                    false
                }
            });
        }
        WidgetActionKind::CollectionSelectAll => {
            edit_collection(world, target, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    let before = list.selection.clone();
                    list.selection.selected = list
                        .items
                        .iter()
                        .filter(|item| item.enabled)
                        .map(|item| item.id)
                        .collect();
                    list.selection.active = list.selection.selected.iter().next_back().copied();
                    before != list.selection
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    let before = grid.selection.clone();
                    grid.selection.selected = grid
                        .items
                        .iter()
                        .filter(|item| item.enabled)
                        .map(|item| item.id)
                        .collect();
                    grid.selection.active = grid.selection.selected.iter().next_back().copied();
                    before != grid.selection
                } else {
                    false
                }
            });
        }
        WidgetActionKind::CollectionBeginRename(id) => begin_collection_rename(world, target, id),
        WidgetActionKind::CollectionCommitRename => {
            commit_collection_rename(world, target, emitted)
        }
        WidgetActionKind::CollectionCancelRename => cancel_collection_rename(world, target),
        WidgetActionKind::CollectionMove { id, index } => {
            edit_collection(world, target, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    list.reorder(id, index)
                } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                    tree.reorder_sibling(id, index)
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    grid.reorder(id, index)
                } else {
                    false
                }
            });
        }
        WidgetActionKind::CollectionScroll(rows) => scroll_collection_by_rows(world, target, rows),
        WidgetActionKind::TreeToggleExpanded(id) => {
            edit_collection(world, target, emitted, |world, owner| {
                world
                    .get_mut::<TreeModel<String>>(owner)
                    .is_some_and(|mut tree| tree.toggle_expanded(id))
            });
        }
        WidgetActionKind::TreeMoveNode { id, parent, index } => {
            edit_collection(world, target, emitted, |world, owner| {
                world
                    .get_mut::<TreeModel<String>>(owner)
                    .is_some_and(|mut tree| tree.move_node(id, parent, index))
            });
        }
        WidgetActionKind::DataTableSelectCell {
            row_id,
            column,
            ctrl,
            shift,
        } => {
            edit_data_table(world, target, emitted, |table| {
                table.select_cell(row_id, column, ctrl, shift)
            });
        }
        WidgetActionKind::DataTableSelectAll => {
            edit_data_table(world, target, emitted, DataTable::select_all);
        }
        WidgetActionKind::DataTableSortColumn(column) => {
            edit_data_table(world, target, emitted, |table| {
                let changed = table.set_active_column(column);
                table.toggle_sort(column) || changed
            });
        }
        WidgetActionKind::DataTableResizeColumn { column, width } => {
            edit_data_table(world, target, emitted, |table| {
                table.resize_column(column, width)
            });
        }
        WidgetActionKind::DataTableMoveActiveRow { delta, extend } => {
            move_data_table_active_row(world, target, delta, extend, emitted);
        }
        WidgetActionKind::DataTableMoveActiveColumn(delta) => {
            edit_data_table(world, target, emitted, |table| {
                let before = table.active_column;
                table.move_active_column(delta);
                before != table.active_column
            });
        }
        WidgetActionKind::DataTableSetActiveColumn(column) => {
            edit_data_table(world, target, emitted, |table| {
                table.set_active_column(column)
            });
        }
        WidgetActionKind::DataTableScrollRows(rows) => {
            scroll_data_table_by_rows(world, target, rows)
        }
        WidgetActionKind::DataTableActivateCell => {
            if let Some(owner) = data_table_owner(world, target)
                && let Some((row_id, column)) = activate_data_table_cell(world, owner)
            {
                emitted.push(WidgetEvent::DataTableCellActivated {
                    target: owner,
                    row_id,
                    column,
                });
            }
        }
        WidgetActionKind::NodeGraphSelectNode { id, additive } => {
            edit_node_graph(world, target, emitted, |graph| graph.select(id, additive));
        }
        WidgetActionKind::NodeGraphSelectAll => {
            edit_node_graph(world, target, emitted, NodeGraph::select_all);
        }
        WidgetActionKind::NodeGraphSelectLink(link_id) => {
            edit_node_graph(world, target, emitted, |graph| graph.select_link(link_id));
        }
        WidgetActionKind::NodeGraphMoveSelected(delta) => {
            edit_node_graph(world, target, emitted, |graph| {
                graph.move_selected_by(delta)
            });
        }
        WidgetActionKind::NodeGraphAddNode {
            title,
            position,
            sockets,
        } => {
            edit_node_graph(world, target, emitted, |graph| {
                if !position.into_iter().all(f32::is_finite) {
                    return false;
                }
                graph.add_node(title, position, sockets);
                true
            });
        }
        WidgetActionKind::NodeGraphCreateLink {
            from_node,
            from_socket,
            to_node,
            to_socket,
        } => {
            edit_node_graph_result(world, target, emitted, |graph| {
                graph
                    .create_link(from_node, from_socket, to_node, to_socket)
                    .map(|_| true)
            });
        }
        WidgetActionKind::NodeGraphReconnectLink {
            link_id,
            endpoint,
            node_id,
            socket_index,
        } => {
            edit_node_graph_result(world, target, emitted, |graph| {
                graph
                    .reconnect_link(link_id, endpoint, node_id, socket_index)
                    .map(|()| true)
            });
        }
        WidgetActionKind::NodeGraphRemoveLink(link_id) => {
            edit_node_graph(world, target, emitted, |graph| graph.remove_link(link_id));
        }
        WidgetActionKind::NodeGraphDeleteSelection => {
            edit_node_graph(world, target, emitted, |graph| {
                if let Some(link) = graph.selected_link {
                    graph.remove_link(link)
                } else {
                    let selected: Vec<_> = graph.selected.iter().copied().collect();
                    let mut changed = false;
                    for id in selected {
                        changed |= graph.remove_node(id);
                    }
                    changed
                }
            });
        }
        WidgetActionKind::NodeGraphToggleCollapse => {
            edit_node_graph(world, target, emitted, NodeGraph::toggle_selected_collapsed);
        }
        WidgetActionKind::NodeGraphPanBy(delta) => {
            edit_node_graph(world, target, emitted, |graph| {
                let before = graph.pan;
                graph.pan_by(delta);
                graph.pan != before
            });
        }
        WidgetActionKind::NodeGraphZoomBy(factor) => {
            edit_node_graph(world, target, emitted, |graph| {
                let before = graph.zoom;
                graph.zoom_by(factor);
                graph.zoom != before
            });
        }
        WidgetActionKind::CanvasSelectObject { id, additive } => {
            select_canvas_object(world, target, id, additive, emitted);
        }
    }
}

fn select_canvas_object(
    world: &mut World,
    target: Entity,
    id: u64,
    additive: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world.get_mut::<Canvas>(target).is_some_and(|mut canvas| {
        if !canvas
            .objects
            .iter()
            .any(|object| object.id == id && object.visible && !object.locked)
        {
            return false;
        }
        let old = canvas.selected.clone();
        if !additive {
            canvas.selected.clear();
        }
        if additive && !canvas.selected.insert(id) {
            canvas.selected.remove(&id);
        } else if !additive {
            canvas.selected.insert(id);
        }
        old != canvas.selected
    });
    if changed
        && let Some(after) = world
            .get::<Canvas>(target)
            .and_then(|canvas| canvas.to_json().ok())
        && before != after
    {
        emitted.push(WidgetEvent::CanvasCommitted {
            target,
            before,
            after,
        });
    }
}

fn edit_node_graph(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut NodeGraph) -> bool,
) {
    let Some(owner) = node_graph_owner(world, target) else {
        return;
    };
    let Some(before) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    let changed = world
        .get_mut::<NodeGraph>(owner)
        .is_some_and(|mut graph| edit(&mut graph));
    if !changed {
        return;
    }
    let Some(after) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    emit_node_graph_commit(owner, before, after, emitted);
}

fn edit_node_graph_result(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut NodeGraph) -> Result<bool, NodeGraphError>,
) {
    let Some(owner) = node_graph_owner(world, target) else {
        return;
    };
    let Some(before) = world
        .get::<NodeGraph>(owner)
        .and_then(|graph| graph.to_json().ok())
    else {
        return;
    };
    let result = world
        .get_mut::<NodeGraph>(owner)
        .map(|mut graph| edit(&mut graph));
    match result {
        Some(Ok(true)) => {
            if let Some(after) = world
                .get::<NodeGraph>(owner)
                .and_then(|graph| graph.to_json().ok())
            {
                emit_node_graph_commit(owner, before, after, emitted);
            }
        }
        Some(Err(error)) => emitted.push(WidgetEvent::NodeGraphLinkFailed {
            target: owner,
            error,
        }),
        Some(Ok(false)) | None => {}
    }
}

fn emit_node_graph_commit(
    target: Entity,
    before: String,
    after: String,
    emitted: &mut Vec<WidgetEvent>,
) {
    if before != after {
        emitted.push(WidgetEvent::NodeGraphCommitted {
            target,
            before,
            after,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeGraphHit {
    Empty,
    NodeHeader(NodeId),
    NodeBody(NodeId),
    Socket {
        node: NodeId,
        socket: usize,
        direction: SocketDirection,
    },
    Link(u64),
}

fn node_graph_screen_position(
    graph: &NodeGraph,
    rect: Rect,
    point: [f32; 2],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> [f32; 2] {
    [
        rect.x + graph.pan[0] + point[0] * graph.zoom,
        rect.y + spacing.header_height + graph.pan[1] + point[1] * graph.zoom,
    ]
}

fn node_graph_data_position(
    graph: &NodeGraph,
    rect: Rect,
    point: [f32; 2],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> [f32; 2] {
    [
        (point[0] - rect.x - graph.pan[0]) / graph.zoom,
        (point[1] - rect.y - spacing.header_height - graph.pan[1]) / graph.zoom,
    ]
}

fn node_graph_node_rect(
    graph: &NodeGraph,
    rect: Rect,
    node: &crate::NodeModel,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Rect {
    let input_count = node
        .sockets
        .iter()
        .filter(|socket| socket.direction == SocketDirection::Input)
        .count();
    let output_count = node
        .sockets
        .iter()
        .filter(|socket| socket.direction == SocketDirection::Output)
        .count();
    let socket_rows = input_count.max(output_count) as f32;
    let graph_height = if node.collapsed {
        spacing.header_height
    } else {
        node.size[1].max(
            spacing.header_height + socket_rows * spacing.socket_spacing + spacing.body_padding,
        )
    };
    let origin = node_graph_screen_position(graph, rect, node.position, spacing);
    Rect {
        x: origin[0],
        y: origin[1],
        width: node.size[0] * graph.zoom,
        height: graph_height * graph.zoom,
    }
}

fn node_graph_socket_position(
    graph: &NodeGraph,
    rect: Rect,
    node: &crate::NodeModel,
    socket_index: usize,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Option<[f32; 2]> {
    let socket = node.sockets.get(socket_index)?;
    let row = node.sockets[..socket_index]
        .iter()
        .filter(|previous| previous.direction == socket.direction)
        .count();
    let node_rect = node_graph_node_rect(graph, rect, node, spacing);
    let y = if node.collapsed {
        node_rect.y + spacing.header_height * graph.zoom * 0.5
    } else {
        node_rect.y
            + (spacing.header_height
                + row as f32 * spacing.socket_spacing
                + spacing.socket_spacing * 0.5)
                * graph.zoom
    };
    Some([
        if socket.direction == SocketDirection::Input {
            node_rect.x
        } else {
            node_rect.x + node_rect.width
        },
        y,
    ])
}

fn cache_node_graph_socket_positions(
    zoom: f32,
    node: &crate::NodeModel,
    node_rect: Rect,
    spacing: &bui_style::ThemeNodeGraphSpacing,
    positions: &mut HashMap<(NodeId, usize), [f32; 2]>,
) {
    let mut input_row = 0usize;
    let mut output_row = 0usize;
    for (socket_index, socket) in node.sockets.iter().enumerate() {
        let row = match socket.direction {
            SocketDirection::Input => {
                let row = input_row;
                input_row = input_row.saturating_add(1);
                row
            }
            SocketDirection::Output => {
                let row = output_row;
                output_row = output_row.saturating_add(1);
                row
            }
        };
        let y = if node.collapsed {
            node_rect.y + spacing.header_height * zoom * 0.5
        } else {
            node_rect.y
                + (spacing.header_height
                    + row as f32 * spacing.socket_spacing
                    + spacing.socket_spacing * 0.5)
                    * zoom
        };
        let x = if socket.direction == SocketDirection::Input {
            node_rect.x
        } else {
            node_rect.x + node_rect.width
        };
        positions.insert((node.id, socket_index), [x, y]);
    }
}

fn node_graph_link_points(
    graph: &NodeGraph,
    rect: Rect,
    link: crate::NodeLink,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Option<[[f32; 2]; 4]> {
    let from = graph.nodes.iter().find(|node| node.id == link.from_node)?;
    let to = graph.nodes.iter().find(|node| node.id == link.to_node)?;
    node_graph_link_points_between(graph, rect, link, from, to, spacing)
}

fn node_graph_link_points_between(
    graph: &NodeGraph,
    rect: Rect,
    link: crate::NodeLink,
    from: &crate::NodeModel,
    to: &crate::NodeModel,
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> Option<[[f32; 2]; 4]> {
    let p0 = node_graph_socket_position(graph, rect, from, link.from_socket, spacing)?;
    let p3 = node_graph_socket_position(graph, rect, to, link.to_socket, spacing)?;
    let tangent = ((p3[0] - p0[0]).abs() * 0.5).max(spacing.connection_tangent_min * graph.zoom);
    let direction = if p3[0] >= p0[0] { 1.0 } else { -1.0 };
    Some([
        p0,
        [p0[0] + tangent * direction, p0[1]],
        [p3[0] - tangent * direction, p3[1]],
        p3,
    ])
}

fn node_graph_bezier_bounds(points: [[f32; 2]; 4], padding: f32) -> Rect {
    let min_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min);
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let min_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min);
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max);
    Rect::new(
        min_x - padding,
        min_y - padding,
        max_x - min_x + padding * 2.0,
        max_y - min_y + padding * 2.0,
    )
}

fn node_graph_hit(
    graph: &NodeGraph,
    rect: Rect,
    position: [f32; 2],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> NodeGraphHit {
    for node in graph.nodes.iter().rev() {
        for (socket_index, socket) in node.sockets.iter().enumerate() {
            if let Some(center) =
                node_graph_socket_position(graph, rect, node, socket_index, spacing)
                && distance(center, position) <= spacing.socket_radius + spacing.socket_hit_slop
            {
                return NodeGraphHit::Socket {
                    node: node.id,
                    socket: socket_index,
                    direction: socket.direction,
                };
            }
        }
    }
    for node in graph.nodes.iter().rev() {
        let node_rect = node_graph_node_rect(graph, rect, node, spacing);
        if node_rect.contains(position[0], position[1]) {
            return if position[1] <= node_rect.y + spacing.header_height * graph.zoom {
                NodeGraphHit::NodeHeader(node.id)
            } else {
                NodeGraphHit::NodeBody(node.id)
            };
        }
    }
    let nodes_by_id: HashMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    for link in graph.links.iter().rev() {
        let (Some(from), Some(to)) = (
            nodes_by_id.get(&link.from_node),
            nodes_by_id.get(&link.to_node),
        ) else {
            continue;
        };
        if let Some(points) = node_graph_link_points_between(graph, rect, *link, from, to, spacing)
            && node_graph_bezier_bounds(points, 7.0).contains(position[0], position[1])
            && bezier_distance(points, position) <= 7.0
        {
            return NodeGraphHit::Link(link.id);
        }
    }
    NodeGraphHit::Empty
}

fn node_graph_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<NodeGraph>(target).is_some() {
        return Some(target);
    }
    world
        .get::<NodeGraphNodeControl>(target)
        .map(|control| control.owner)
        .or_else(|| {
            world
                .get::<NodeGraphSocketControl>(target)
                .map(|control| control.owner)
        })
        .or_else(|| {
            world
                .get::<NodeGraphLinkControl>(target)
                .map(|control| control.owner)
        })
}

fn has_annotated_node_graph_down(
    world: &World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
) -> bool {
    world
        .get_resource::<UiRoutedEvents>()
        .is_some_and(|events| {
            events.events.iter().any(|event| {
                matches!(event, UiRoutedEvent::PointerDownWithModifiers {
                    window: annotated_window,
                    target: annotated_target,
                    button: annotated_button,
                    position: annotated_position,
                    ..
                } if *annotated_window == window
                    && *annotated_target == target
                    && *annotated_button == button
                    && *annotated_position == position)
            })
        })
}

fn process_node_graph_event(
    world: &mut World,
    event: &UiRoutedEvent,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    match event {
        UiRoutedEvent::PointerDown {
            window,
            target,
            button,
            position,
        } if let Some(owner) = node_graph_owner(world, *target) => {
            if !has_annotated_node_graph_down(world, *window, *target, *button, *position) {
                begin_node_graph_gesture(world, *window, owner, *button, *position, None, emitted);
            }
            true
        }
        UiRoutedEvent::PointerDownWithModifiers {
            window,
            target,
            button,
            position,
            modifiers,
        } if let Some(owner) = node_graph_owner(world, *target) => {
            begin_node_graph_gesture(
                world,
                *window,
                owner,
                *button,
                *position,
                Some(*modifiers),
                emitted,
            );
            true
        }
        UiRoutedEvent::PointerMoved {
            target,
            position,
            dragging: true,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            update_node_graph_gesture(world, owner, *position);
            true
        }
        UiRoutedEvent::PointerUp {
            target,
            position,
            cancelled,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            finish_node_graph_gesture(world, owner, *position, *cancelled, emitted);
            true
        }
        UiRoutedEvent::PointerCancel { target, .. }
            if let Some(owner) = node_graph_owner(world, *target) =>
        {
            finish_node_graph_gesture(world, owner, [0.0, 0.0], true, emitted);
            true
        }
        UiRoutedEvent::Click { target, .. } if node_graph_owner(world, *target).is_some() => true,
        UiRoutedEvent::ClickWithModifiers { target, .. }
            if node_graph_owner(world, *target).is_some() =>
        {
            true
        }
        UiRoutedEvent::Wheel {
            target,
            position,
            delta,
            unit,
            modifiers,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            node_graph_wheel(world, owner, *position, *delta, *unit, *modifiers, emitted);
            true
        }
        UiRoutedEvent::Key {
            target,
            key,
            pressed: true,
            modifiers,
            ..
        } if let Some(owner) = node_graph_owner(world, *target) => {
            node_graph_key(world, owner, *key, *modifiers, emitted);
            true
        }
        _ => false,
    }
}

fn begin_node_graph_gesture(
    world: &mut World,
    window: Entity,
    target: Entity,
    button: UiPointerButton,
    position: [f32; 2],
    event_modifiers: Option<UiModifiers>,
    emitted: &mut Vec<WidgetEvent>,
) {
    let (Some(layout), Some(graph)) = (
        world.get::<UiLayoutResult>(target).copied(),
        world.get::<NodeGraph>(target).cloned(),
    ) else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    if !layout.rect.contains(position[0], position[1]) {
        return;
    }
    let Ok(before) = graph.to_json() else {
        return;
    };
    let modifiers = event_modifiers.unwrap_or_else(|| {
        world
            .get_resource::<WidgetInteractionState>()
            .and_then(|state| state.modifiers.get(&window).copied())
            .unwrap_or_default()
    });

    if matches!(button, UiPointerButton::Middle | UiPointerButton::Secondary) {
        set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::Pan {
                start_pointer: position,
                start_pan: graph.pan,
                before,
            }),
        );
        return;
    }
    if button != UiPointerButton::Primary {
        return;
    }
    if modifiers.alt {
        set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::CutLinks {
                points: vec![position],
                before,
            }),
        );
        return;
    }

    match node_graph_hit(&graph, layout.rect, position, spacing) {
        NodeGraphHit::Socket {
            node,
            socket,
            direction,
        } => {
            let reconnect = graph.links.iter().find_map(|link| {
                (direction == SocketDirection::Output
                    && link.from_node == node
                    && link.from_socket == socket)
                    .then_some((link.id, LinkEndpoint::From))
                    .or_else(|| {
                        (direction == SocketDirection::Input
                            && link.to_node == node
                            && link.to_socket == socket)
                            .then_some((link.id, LinkEndpoint::To))
                    })
            });
            set_node_graph_gesture(
                world,
                target,
                Some(NodeGraphGesture::LinkDrag {
                    start_node: node,
                    start_socket: socket,
                    start_direction: direction,
                    reconnect,
                    current_pointer: position,
                    before,
                }),
            );
        }
        NodeGraphHit::NodeHeader(id) | NodeGraphHit::NodeBody(id) => {
            let additive = modifiers.control || modifiers.shift;
            let was_selected = graph.selected.contains(&id);
            if additive {
                if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                    graph.select(id, true);
                }
                if was_selected {
                    set_node_graph_gesture(
                        world,
                        target,
                        Some(NodeGraphGesture::Selection { before }),
                    );
                    return;
                }
            } else if !was_selected {
                if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                    graph.select(id, false);
                }
            } else if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                graph.selected_link = None;
            }

            if matches!(
                node_graph_hit(&graph, layout.rect, position, spacing),
                NodeGraphHit::NodeHeader(_)
            ) {
                let node = graph.nodes.iter().find(|node| node.id == id).unwrap();
                let node_rect = node_graph_node_rect(&graph, layout.rect, node, spacing);
                if position[0] <= node_rect.x + 22.0 * graph.zoom {
                    if let Some(mut current) = world.get_mut::<NodeGraph>(target)
                        && let Some(node) = current.nodes.iter_mut().find(|node| node.id == id)
                    {
                        node.collapsed = !node.collapsed;
                    }
                    if let Some(after) = world
                        .get::<NodeGraph>(target)
                        .and_then(|graph| graph.to_json().ok())
                    {
                        emit_node_graph_commit(target, before, after, emitted);
                    }
                    return;
                }
            }
            let starts = world
                .get::<NodeGraph>(target)
                .map(|graph| {
                    graph
                        .nodes
                        .iter()
                        .filter(|node| graph.selected.contains(&node.id))
                        .map(|node| (node.id, node.position))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            set_node_graph_gesture(
                world,
                target,
                Some(NodeGraphGesture::MoveNodes {
                    start_pointer: position,
                    starts,
                    before,
                }),
            );
        }
        NodeGraphHit::Link(id) => {
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                graph.select_link(id);
            }
            set_node_graph_gesture(world, target, Some(NodeGraphGesture::SelectLink { before }));
        }
        NodeGraphHit::Empty => set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::BoxSelect {
                start_pointer: position,
                current_pointer: position,
                additive: modifiers.control || modifiers.shift,
                before,
            }),
        ),
    }
}

fn set_node_graph_gesture(world: &mut World, target: Entity, gesture: Option<NodeGraphGesture>) {
    if let Some(mut state) = world.get_mut::<NodeGraphEditorState>(target) {
        state.gesture = gesture;
    }
}

fn update_node_graph_gesture(world: &mut World, target: Entity, position: [f32; 2]) {
    let Some(gesture) = world
        .get::<NodeGraphEditorState>(target)
        .and_then(|state| state.gesture.clone())
    else {
        return;
    };
    match gesture {
        NodeGraphGesture::Pan {
            start_pointer,
            start_pan,
            ..
        } => {
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                graph.pan = [
                    start_pan[0] + position[0] - start_pointer[0],
                    start_pan[1] + position[1] - start_pointer[1],
                ];
            }
        }
        NodeGraphGesture::MoveNodes {
            start_pointer,
            starts,
            ..
        } => {
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                let delta = [
                    (position[0] - start_pointer[0]) / graph.zoom,
                    (position[1] - start_pointer[1]) / graph.zoom,
                ];
                for (id, start) in starts {
                    graph.move_node(id, [start[0] + delta[0], start[1] + delta[1]]);
                }
            }
        }
        NodeGraphGesture::BoxSelect {
            start_pointer,
            additive,
            before,
            ..
        } => set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::BoxSelect {
                start_pointer,
                current_pointer: position,
                additive,
                before,
            }),
        ),
        NodeGraphGesture::LinkDrag {
            start_node,
            start_socket,
            start_direction,
            reconnect,
            before,
            ..
        } => set_node_graph_gesture(
            world,
            target,
            Some(NodeGraphGesture::LinkDrag {
                start_node,
                start_socket,
                start_direction,
                reconnect,
                current_pointer: position,
                before,
            }),
        ),
        NodeGraphGesture::CutLinks { mut points, before } => {
            if points
                .last()
                .is_none_or(|last| distance(*last, position) >= 3.0)
            {
                points.push(position);
            }
            set_node_graph_gesture(
                world,
                target,
                Some(NodeGraphGesture::CutLinks { points, before }),
            );
        }
        NodeGraphGesture::Selection { .. } | NodeGraphGesture::SelectLink { .. } => {}
    }
}

fn finish_node_graph_gesture(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if !cancelled {
        update_node_graph_gesture(world, target, position);
    }
    let gesture = world
        .get_mut::<NodeGraphEditorState>(target)
        .and_then(|mut state| state.gesture.take());
    let Some(gesture) = gesture else {
        return;
    };
    let before = node_graph_gesture_before(&gesture).to_owned();
    if cancelled {
        if let Ok(graph) = NodeGraph::from_json(&before)
            && let Some(mut current) = world.get_mut::<NodeGraph>(target)
        {
            *current = graph;
        }
        return;
    }

    match gesture {
        NodeGraphGesture::BoxSelect {
            start_pointer,
            current_pointer,
            additive,
            ..
        } => {
            let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
                return;
            };
            let Some(graph) = world.get::<NodeGraph>(target).cloned() else {
                return;
            };
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let box_rect = Rect::new(
                start_pointer[0].min(current_pointer[0]),
                start_pointer[1].min(current_pointer[1]),
                (start_pointer[0] - current_pointer[0]).abs(),
                (start_pointer[1] - current_pointer[1]).abs(),
            );
            if let Some(mut current) = world.get_mut::<NodeGraph>(target) {
                if !additive {
                    current.selected.clear();
                }
                current.selected_link = None;
                for node in &graph.nodes {
                    if node_graph_node_rect(&graph, layout.rect, node, spacing)
                        .intersection(box_rect)
                        .is_some()
                    {
                        current.selected.insert(node.id);
                    }
                }
            }
        }
        NodeGraphGesture::LinkDrag {
            start_node,
            start_socket,
            start_direction,
            reconnect,
            current_pointer,
            ..
        } => {
            let (Some(layout), Some(graph)) = (
                world.get::<UiLayoutResult>(target).copied(),
                world.get::<NodeGraph>(target).cloned(),
            ) else {
                return;
            };
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let NodeGraphHit::Socket {
                node,
                socket,
                direction,
            } = node_graph_hit(&graph, layout.rect, current_pointer, spacing)
            else {
                return;
            };
            let result = if let Some((link_id, endpoint)) = reconnect {
                if direction != start_direction {
                    return;
                }
                world
                    .get_mut::<NodeGraph>(target)
                    .map(|mut graph| graph.reconnect_link(link_id, endpoint, node, socket))
            } else {
                if direction == start_direction {
                    return;
                }
                let (from_node, from_socket, to_node, to_socket) =
                    if start_direction == SocketDirection::Output {
                        (start_node, start_socket, node, socket)
                    } else {
                        (node, socket, start_node, start_socket)
                    };
                world.get_mut::<NodeGraph>(target).map(|mut graph| {
                    graph
                        .create_link(from_node, from_socket, to_node, to_socket)
                        .map(|_| ())
                })
            };
            if let Some(Err(error)) = result {
                emitted.push(WidgetEvent::NodeGraphLinkFailed { target, error });
            }
        }
        NodeGraphGesture::CutLinks { points, .. } => {
            let (Some(layout), Some(graph)) = (
                world.get::<UiLayoutResult>(target).copied(),
                world.get::<NodeGraph>(target).cloned(),
            ) else {
                return;
            };
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let removed: Vec<_> = graph
                .links
                .iter()
                .filter(|link| {
                    node_graph_link_crossed(&graph, layout.rect, **link, &points, spacing)
                })
                .map(|link| link.id)
                .collect();
            if let Some(mut graph) = world.get_mut::<NodeGraph>(target) {
                for id in removed {
                    graph.remove_link(id);
                }
            }
        }
        NodeGraphGesture::Pan { .. }
        | NodeGraphGesture::MoveNodes { .. }
        | NodeGraphGesture::Selection { .. }
        | NodeGraphGesture::SelectLink { .. } => {}
    }
    if let Some(after) = world
        .get::<NodeGraph>(target)
        .and_then(|graph| graph.to_json().ok())
    {
        emit_node_graph_commit(target, before, after, emitted);
    }
}

fn node_graph_gesture_before(gesture: &NodeGraphGesture) -> &str {
    match gesture {
        NodeGraphGesture::Pan { before, .. }
        | NodeGraphGesture::MoveNodes { before, .. }
        | NodeGraphGesture::BoxSelect { before, .. }
        | NodeGraphGesture::LinkDrag { before, .. }
        | NodeGraphGesture::CutLinks { before, .. }
        | NodeGraphGesture::Selection { before }
        | NodeGraphGesture::SelectLink { before } => before,
    }
}

fn node_graph_wheel(
    world: &mut World,
    target: Entity,
    pointer: [f32; 2],
    delta: [f32; 2],
    unit: bui_core::UiWheelUnit,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    edit_node_graph(world, target, emitted, |graph| {
        if modifiers.control || modifiers.super_key {
            let anchor = node_graph_data_position(graph, rect, pointer, spacing);
            let scale = if unit == bui_core::UiWheelUnit::Lines {
                1.0
            } else {
                0.25
            };
            let before = graph.zoom;
            graph.zoom_by((delta[1] * scale * 0.12).exp());
            if graph.zoom == before {
                return false;
            }
            graph.pan = [
                pointer[0] - rect.x - anchor[0] * graph.zoom,
                pointer[1] - rect.y - spacing.header_height - anchor[1] * graph.zoom,
            ];
        } else {
            let scale = if unit == bui_core::UiWheelUnit::Lines {
                36.0
            } else {
                1.0
            };
            graph.pan_by([delta[0] * scale, delta[1] * scale]);
        }
        true
    });
}

fn node_graph_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && matches!(key, UiKeyCode::Z | UiKeyCode::Y) {
        return;
    }
    match key {
        UiKeyCode::Escape => {
            finish_node_graph_gesture(world, target, [0.0, 0.0], true, emitted);
        }
        UiKeyCode::A if modifiers.control || modifiers.super_key => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphSelectAll),
                emitted,
            );
        }
        UiKeyCode::Delete | UiKeyCode::Backspace | UiKeyCode::X => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphDeleteSelection),
                emitted,
            );
        }
        UiKeyCode::C => {
            apply_action(
                world,
                WidgetAction::new(target, WidgetActionKind::NodeGraphToggleCollapse),
                emitted,
            );
        }
        UiKeyCode::N => {
            let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
            let spacing = &theme.spacing.node_graph;
            let position = world
                .get::<UiLayoutResult>(target)
                .zip(world.get::<NodeGraph>(target))
                .map(|(layout, graph)| {
                    node_graph_data_position(
                        graph,
                        layout.rect,
                        [
                            layout.rect.x + layout.rect.width * 0.5,
                            layout.rect.y + spacing.header_height + 80.0,
                        ],
                        spacing,
                    )
                })
                .unwrap_or([0.0, 0.0]);
            apply_action(
                world,
                WidgetAction::new(
                    target,
                    WidgetActionKind::NodeGraphAddNode {
                        title: "Node".to_owned(),
                        position,
                        sockets: vec![
                            NodeSocket::input("Input", crate::SocketType::Any),
                            NodeSocket::output("Output", crate::SocketType::Any),
                        ],
                    },
                ),
                emitted,
            );
        }
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowDown => {
            if modifiers.control || modifiers.super_key {
                let factor = match key {
                    UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => 1.15,
                    UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => 1.0 / 1.15,
                    _ => unreachable!(),
                };
                apply_action(
                    world,
                    WidgetAction::new(target, WidgetActionKind::NodeGraphZoomBy(factor)),
                    emitted,
                );
            } else {
                let step = if modifiers.shift { 40.0 } else { 10.0 };
                let delta = match key {
                    UiKeyCode::ArrowLeft => [-step, 0.0],
                    UiKeyCode::ArrowRight => [step, 0.0],
                    UiKeyCode::ArrowUp => [0.0, -step],
                    UiKeyCode::ArrowDown => [0.0, step],
                    _ => unreachable!(),
                };
                apply_action(
                    world,
                    WidgetAction::new(target, WidgetActionKind::NodeGraphMoveSelected(delta)),
                    emitted,
                );
            }
        }
        UiKeyCode::Home => {
            edit_node_graph(world, target, emitted, |graph| {
                let changed = graph.pan != [0.0, 0.0] || graph.zoom != 1.0;
                graph.pan = [0.0, 0.0];
                graph.zoom = 1.0;
                changed
            });
        }
        _ => {}
    }
}

fn bezier_point(points: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    [
        u.powi(3) * points[0][0]
            + 3.0 * u * u * t * points[1][0]
            + 3.0 * u * t * t * points[2][0]
            + t.powi(3) * points[3][0],
        u.powi(3) * points[0][1]
            + 3.0 * u * u * t * points[1][1]
            + 3.0 * u * t * t * points[2][1]
            + t.powi(3) * points[3][1],
    ]
}

fn bezier_distance(points: [[f32; 2]; 4], point: [f32; 2]) -> f32 {
    let mut previous = points[0];
    let mut closest = f32::INFINITY;
    for index in 1..=24 {
        let current = bezier_point(points, index as f32 / 24.0);
        closest = closest.min(point_segment_distance(point, previous, current));
        previous = current;
    }
    closest
}

fn point_segment_distance(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let denominator = delta[0] * delta[0] + delta[1] * delta[1];
    if denominator <= f32::EPSILON {
        return distance(point, start);
    }
    let amount = (((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1])
        / denominator)
        .clamp(0.0, 1.0);
    distance(
        point,
        [start[0] + amount * delta[0], start[1] + amount * delta[1]],
    )
}

fn node_graph_link_crossed(
    graph: &NodeGraph,
    rect: Rect,
    link: crate::NodeLink,
    stroke: &[[f32; 2]],
    spacing: &bui_style::ThemeNodeGraphSpacing,
) -> bool {
    if stroke.len() < 2 {
        return false;
    }
    let Some(points) = node_graph_link_points(graph, rect, link, spacing) else {
        return false;
    };
    let mut curve_previous = points[0];
    for sample in 1..=24 {
        let curve_current = bezier_point(points, sample as f32 / 24.0);
        if stroke.windows(2).any(|segment| {
            segments_close(curve_previous, curve_current, segment[0], segment[1], 4.0)
        }) {
            return true;
        }
        curve_previous = curve_current;
    }
    false
}

fn segments_close(
    first_start: [f32; 2],
    first_end: [f32; 2],
    second_start: [f32; 2],
    second_end: [f32; 2],
    tolerance: f32,
) -> bool {
    point_segment_distance(first_start, second_start, second_end) <= tolerance
        || point_segment_distance(first_end, second_start, second_end) <= tolerance
        || point_segment_distance(second_start, first_start, first_end) <= tolerance
        || point_segment_distance(second_end, first_start, first_end) <= tolerance
}

/// Advance animated panel state using Bevy's frame delta (or a stable fallback in bare apps).
pub fn advance_panel_animations(world: &mut World) {
    let delta = world
        .get_resource::<bevy_time::Time>()
        .map_or(1.0 / 60.0, bevy_time::Time::delta_secs);
    let (motion_scale, duration_seconds) =
        world.get_resource::<Theme>().map_or((1.0, 0.16), |theme| {
            (
                theme.motion_scale,
                theme
                    .motion
                    .duration(ThemeMotionToken::PanelExpand)
                    .as_secs_f32(),
            )
        });
    let reduced_motion = world
        .get_resource::<bui_style::UiReducedMotion>()
        .is_some_and(|reduced| reduced.0);
    let mut query = world.query::<&mut Panel>();
    for mut panel in query.iter_mut(world) {
        let target = if panel.state.target_expanded {
            1.0
        } else {
            0.0
        };
        let needs_advance = !panel.state.progress.is_finite()
            || (panel.state.progress - target).abs() > f32::EPSILON;
        if !needs_advance {
            continue;
        }
        panel.state.animation_seconds = duration_seconds;
        panel
            .state
            .advance_with_motion(delta, motion_scale, reduced_motion);
    }
}

/// Animate popup surfaces independently from their hit boxes. `sync_menu_popups` keeps
/// interactive rows at their final geometry immediately; this system only moves the visual
/// surface between the trigger and popup bounds.
pub fn advance_menu_animations(world: &mut World) {
    let delta = world
        .get_resource::<bevy_time::Time>()
        .map_or(1.0 / 60.0, bevy_time::Time::delta_secs);
    advance_menu_animations_by(world, delta);
}

fn advance_menu_animations_by(world: &mut World, delta_seconds: f32) {
    world.init_resource::<WidgetMenuAnimations>();
    let mut query = world.query::<(Entity, &MenuControl, &UiLayoutResult)>();
    let menus: Vec<_> = query
        .iter(world)
        .map(|(entity, menu, layout)| (entity, menu.session.open, layout.rect, menu.popup.bounds))
        .collect();
    let live: HashSet<_> = menus.iter().map(|(entity, ..)| *entity).collect();
    let theme = world.resource::<Theme>();
    let spec = theme.motion.transition(ThemeMotionToken::MenuOpen);
    let duration = spec.duration.as_secs_f32();
    let motion_scale = theme.motion_scale;
    let reduced_motion = world
        .get_resource::<bui_style::UiReducedMotion>()
        .is_some_and(|reduced| reduced.0);
    let snap = reduced_motion
        || !duration.is_finite()
        || duration <= 0.0
        || !motion_scale.is_finite()
        || motion_scale <= 0.0;
    let step = if snap {
        1.0
    } else {
        (delta_seconds.max(0.0) * motion_scale / duration).clamp(0.0, 1.0)
    };

    let previous = world.resource::<WidgetMenuAnimations>().clone();
    let mut next = previous.clone();
    next.0.retain(|entity, _| live.contains(entity));
    for (entity, open, anchor, popup) in menus {
        let Some(state) = next.0.get_mut(&entity) else {
            if open {
                next.0.insert(
                    entity,
                    MenuAnimationState {
                        progress: if snap { 1.0 } else { step },
                        target_open: true,
                        anchor,
                        popup,
                    },
                );
            }
            continue;
        };

        if state.target_open != open {
            state.target_open = open;
        }
        if open {
            state.anchor = anchor;
            state.popup = popup;
        }
        let target = if state.target_open { 1.0 } else { 0.0 };
        if snap {
            state.progress = target;
        } else {
            let remaining = target - state.progress;
            state.progress += remaining.signum() * step.min(remaining.abs());
        }
    }
    next.0
        .retain(|_, state| state.target_open || state.progress > f32::EPSILON);
    if previous != next {
        let changed_owners: HashSet<_> = previous
            .0
            .keys()
            .chain(next.0.keys())
            .copied()
            .filter(|owner| previous.0.get(owner) != next.0.get(owner))
            .collect();
        for owner in changed_owners {
            mark_menu_paint_subtree(world, owner);
        }
        *world.resource_mut::<WidgetMenuAnimations>() = next;
    }
}

fn menu_animation_progress(world: &World, owner: Entity, theme: &Theme) -> Option<f32> {
    let progress = world
        .get_resource::<WidgetMenuAnimations>()?
        .0
        .get(&owner)?
        .progress;
    let easing = theme.motion.transition(ThemeMotionToken::MenuOpen).easing;
    Some(match easing {
        bui_style::UiEasing::Linear => progress.clamp(0.0, 1.0),
        bui_style::UiEasing::EaseIn => progress.clamp(0.0, 1.0).powi(2),
        bui_style::UiEasing::EaseOut => 1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(2),
        bui_style::UiEasing::EaseInOut if progress < 0.5 => 2.0 * progress.clamp(0.0, 1.0).powi(2),
        bui_style::UiEasing::EaseInOut => {
            1.0 - (-2.0 * progress.clamp(0.0, 1.0) + 2.0).powi(2) * 0.5
        }
    })
}

fn interpolate_rect(from: Rect, to: Rect, progress: f32) -> Rect {
    let mix = |start: f32, end: f32| start + (end - start) * progress;
    Rect {
        x: mix(from.x, to.x),
        y: mix(from.y, to.y),
        width: mix(from.width, to.width).max(0.0),
        height: mix(from.height, to.height).max(0.0),
    }
}

fn with_opacity(color: Rgba, opacity: f32) -> Rgba {
    Rgba(color.0, color.1, color.2, color.3 * opacity.clamp(0.0, 1.0))
}

fn intersect_clip_rect(clip: ClipRect, rect: Rect) -> Option<ClipRect> {
    Rect::new(clip.x, clip.y, clip.width, clip.height)
        .intersection(rect)
        .map(ClipRect::from)
}

fn toggle_exclusive_panel(world: &mut World, target: Entity, emitted: &mut Vec<WidgetEvent>) {
    let Some(clicked) = world.get::<Panel>(target).cloned() else {
        return;
    };
    let clicked_key = clicked
        .persistence_key
        .clone()
        .unwrap_or_else(|| clicked.title.clone());
    let mut ancestors = vec![target];
    let mut current = target;
    while let Some(parent) = world.get::<UiParent>(current).map(|parent| parent.0) {
        ancestors.push(parent);
        current = parent;
    }
    let group_scope = ancestors.into_iter().find_map(|entity| {
        world
            .get::<PanelGroup>(entity)
            .filter(|group| group.panel_keys.iter().any(|key| key == &clicked_key))
            .map(|group| {
                (
                    entity,
                    group.panel_keys.iter().cloned().collect::<HashSet<_>>(),
                )
            })
    });
    let fallback_parent = world.get::<UiParent>(target).map(|parent| parent.0);
    let mut keys = group_scope
        .as_ref()
        .map_or_else(HashSet::new, |(_, keys)| keys.clone());
    if keys.is_empty()
        && let Some(parent) = fallback_parent
    {
        let mut panels = world.query::<(&Panel, &UiParent)>();
        keys.extend(
            panels
                .iter(world)
                .filter(|(panel, ui_parent)| ui_parent.0 == parent && panel.exclusive)
                .map(|(panel, _)| {
                    panel
                        .persistence_key
                        .clone()
                        .unwrap_or_else(|| panel.title.clone())
                }),
        );
    }
    keys.insert(clicked_key.clone());
    if let Some((group_entity, _)) = group_scope.as_ref()
        && let Some(mut group) = world.get_mut::<PanelGroup>(*group_entity)
    {
        group.exclusive_open = Some(clicked_key.clone());
    }
    let mut panels = world.query::<(Entity, &Panel)>();
    let changes: Vec<_> =
        panels
            .iter(world)
            .filter_map(|(entity, panel)| {
                let key = panel.persistence_key.as_deref().unwrap_or(&panel.title);
                let in_scope = if let Some((group, _)) = group_scope.as_ref() {
                    entity == target || has_ancestor(world, entity, *group)
                } else if let Some(parent) = fallback_parent {
                    entity == target
                        || world
                            .get::<UiParent>(entity)
                            .is_some_and(|ui_parent| ui_parent.0 == parent)
                } else {
                    entity == target
                };
                (in_scope && (entity == target || (panel.exclusive && keys.contains(key))))
                    .then_some((entity, panel.state.target_expanded, key == clicked_key))
            })
            .collect();
    let mut committed = Vec::new();
    for (entity, before, expanded) in changes {
        if let Some(mut panel) = world.get_mut::<Panel>(entity)
            && panel.set_expanded(expanded)
        {
            committed.push((entity, before, expanded));
        }
    }
    if !committed.is_empty() {
        emitted.push(WidgetEvent::PanelGroupCommitted { changes: committed });
    }
}

fn panel_header_contains(world: &World, target: Entity, position: [f32; 2]) -> bool {
    let Some(layout) = world.get::<UiLayoutResult>(target) else {
        // Standalone panels without measured layout retain their routed behavior.
        return true;
    };
    let header_height = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.panel.header_height,
        |theme| theme.spacing.panel.header_height,
    );
    let header = Rect {
        height: layout.rect.height.min(header_height),
        ..layout.rect
    };
    header.contains(position[0], position[1])
}

fn panel_reorder_context(
    world: &World,
    panel_entity: Entity,
) -> Option<(Entity, String, String, Vec<String>)> {
    let panel = world.get::<Panel>(panel_entity)?;
    let panel_key = panel
        .persistence_key
        .clone()
        .or_else(|| world.get::<UiKey>(panel_entity).map(|key| key.0.clone()))?;
    let group_entity = world.get::<UiParent>(panel_entity)?.0;
    let group = world.get::<PanelGroup>(group_entity)?;
    if !group.reorderable || !group.panel_keys.contains(&panel_key) {
        return None;
    }
    let persistence_key = group
        .persistence_key
        .clone()
        .or_else(|| world.get::<UiKey>(group_entity).map(|key| key.0.clone()))?;
    crate::panels::validate_panel_order(world, group_entity, &group.panel_keys).ok()?;
    Some((
        group_entity,
        persistence_key,
        panel_key,
        group.panel_keys.clone(),
    ))
}

fn reorder_panel_step(
    world: &mut World,
    panel_entity: Entity,
    delta: i32,
    emitted: &mut Vec<WidgetEvent>,
) {
    if delta == 0 {
        return;
    }
    let Some((group, persistence_key, panel_key, before)) =
        panel_reorder_context(world, panel_entity)
    else {
        return;
    };
    let Some(current_index) = before.iter().position(|key| key == &panel_key) else {
        return;
    };
    let destination = (current_index as i64 + i64::from(delta))
        .clamp(0, before.len().saturating_sub(1) as i64) as usize;
    if destination == current_index {
        return;
    }
    let mut after = before.clone();
    let key = after.remove(current_index);
    after.insert(destination, key);
    if crate::panels::restore_panel_order(world, group, &after) {
        emitted.push(WidgetEvent::PanelOrderCommitted {
            group,
            persistence_key,
            before,
            after,
        });
    }
}

fn begin_panel_reorder(
    world: &mut World,
    window: Entity,
    panel_entity: Entity,
    position: [f32; 2],
) {
    if !panel_header_contains(world, panel_entity, position) {
        return;
    }
    let Some((group, persistence_key, moving_key, order)) =
        panel_reorder_context(world, panel_entity)
    else {
        return;
    };
    let Some(children) = world.get::<bui_core::UiChildren>(group) else {
        return;
    };
    let mut keyed_positions = HashMap::new();
    for child in &children.0 {
        let Some(panel) = world.get::<Panel>(*child) else {
            continue;
        };
        let Some(key) = panel
            .persistence_key
            .clone()
            .or_else(|| world.get::<UiKey>(*child).map(|key| key.0.clone()))
        else {
            continue;
        };
        let center_y = world
            .get::<UiLayoutResult>(*child)
            .map_or(f32::NAN, |layout| layout.rect.y + layout.rect.height * 0.5);
        keyed_positions.insert(key, center_y);
    }
    let slots: Vec<_> = order
        .iter()
        .enumerate()
        .map(|(index, key)| {
            (
                key.clone(),
                keyed_positions
                    .get(key)
                    .copied()
                    .filter(|center| center.is_finite())
                    .unwrap_or(index as f32),
            )
        })
        .collect();
    world
        .resource_mut::<WidgetInteractionState>()
        .panel_reorders
        .insert(
            window,
            PanelReorderGesture {
                group,
                source: panel_entity,
                persistence_key,
                moving_key,
                before: order.clone(),
                current: order,
                slots,
            },
        );
}

fn update_panel_reorder(world: &mut World, window: Entity, position: [f32; 2]) {
    let snapshot = world
        .resource::<WidgetInteractionState>()
        .panel_reorders
        .get(&window)
        .map(|gesture| {
            (
                gesture.group,
                gesture.moving_key.clone(),
                gesture.before.clone(),
                gesture.current.clone(),
                gesture.slots.clone(),
            )
        });
    let Some((group, moving_key, before, current, slots)) = snapshot else {
        return;
    };
    let insert_at = slots
        .iter()
        .filter(|(key, center)| key != &moving_key && position[1] >= *center)
        .count()
        .min(before.len().saturating_sub(1));
    let mut after = before;
    let Some(current_index) = after.iter().position(|key| key == &moving_key) else {
        return;
    };
    let key = after.remove(current_index);
    after.insert(insert_at, key);
    if after == current {
        return;
    }
    if crate::panels::preview_panel_order(world, group, &after)
        && let Some(gesture) = world
            .resource_mut::<WidgetInteractionState>()
            .panel_reorders
            .get_mut(&window)
    {
        gesture.current = after;
    }
}

fn finish_panel_reorder(
    world: &mut World,
    gesture: PanelReorderGesture,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        let _ = crate::panels::restore_panel_order(world, gesture.group, &gesture.before);
        return;
    }
    if gesture.current == gesture.before {
        return;
    }
    if crate::panels::restore_panel_order(world, gesture.group, &gesture.current) {
        emitted.push(WidgetEvent::PanelOrderCommitted {
            group: gesture.group,
            persistence_key: gesture.persistence_key,
            before: gesture.before,
            after: gesture.current,
        });
    } else {
        emitted.push(WidgetEvent::PanelOrderRestoreFailed {
            group: gesture.group,
            reason: "panel order no longer matches the live group".into(),
        });
    }
}

fn cancel_panel_reorder(world: &mut World, window: Entity, emitted: &mut Vec<WidgetEvent>) -> bool {
    let gesture = world
        .resource_mut::<WidgetInteractionState>()
        .panel_reorders
        .remove(&window);
    if let Some(gesture) = gesture {
        finish_panel_reorder(world, gesture, true, emitted);
        true
    } else {
        false
    }
}

/// Keep collapsed panel children out of layout and hit testing.
pub fn sync_panel_children(world: &mut World) {
    let mut query = world.query::<(Entity, &Panel, &bui_core::UiChildren)>();
    let panels: Vec<_> = query
        .iter(world)
        .map(|(entity, panel, children)| {
            (
                entity,
                panel.state.progress,
                panel.state.target_expanded,
                children.0.clone(),
            )
        })
        .collect();
    for (panel, progress, expanded, children) in panels {
        for child in children {
            if world.get::<crate::WidgetElement>(child).is_none() {
                continue;
            }
            if let Some(mut visible) = world.get_mut::<UiVisible>(child) {
                visible.0 = expanded || progress > 0.0;
            } else {
                world
                    .entity_mut(child)
                    .insert(UiVisible(expanded || progress > 0.0));
            }
        }
        apply_panel_clip(world, panel, progress);
    }
}

fn apply_panel_clip(world: &mut World, panel: Entity, progress: f32) {
    if progress >= 1.0 {
        return;
    }
    let Some(panel_layout) = world.get::<UiLayoutResult>(panel).copied() else {
        return;
    };
    let header_spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.panel.header_height,
        |theme| theme.spacing.panel.header_height,
    );
    let header_height = panel_layout.rect.height.min(header_spacing);
    let content_height = (panel_layout.rect.height - header_height).max(0.0) * progress;
    let mut reveal = Rect {
        x: panel_layout.rect.x,
        y: panel_layout.rect.y + header_height,
        width: panel_layout.rect.width,
        height: content_height,
    };
    if let Some(parent_clip) = panel_layout.clip {
        let Some(intersection) = reveal.intersection(parent_clip) else {
            reveal.width = 0.0;
            reveal.height = 0.0;
            set_descendant_clip(world, panel, panel_layout.window, reveal);
            return;
        };
        reveal = intersection;
    }
    set_descendant_clip(world, panel, panel_layout.window, reveal);
}

fn set_descendant_clip(world: &mut World, panel: Entity, window: Entity, clip: Rect) {
    let mut query = world.query_filtered::<Entity, With<UiLayoutResult>>();
    let descendants: Vec<_> = query
        .iter(world)
        .filter(|entity| *entity != panel && has_ancestor(world, *entity, panel))
        .collect();
    for entity in descendants {
        let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
            continue;
        };
        if layout.window != window {
            continue;
        }
        let clip = layout
            .clip
            .and_then(|old| clip.intersection(old))
            .unwrap_or(clip);
        if let Some(mut layout) = world.get_mut::<UiLayoutResult>(entity) {
            layout.clip = Some(clip);
        }
        if let Some(mut hit) = world.get_mut::<UiHitBox>(entity) {
            hit.clip = Some(
                hit.clip
                    .and_then(|old| clip.intersection(old))
                    .unwrap_or(clip),
            );
        }
    }
}

fn has_ancestor(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    while let Some(parent) = world.get::<UiParent>(entity) {
        if parent.0 == ancestor {
            return true;
        }
        entity = parent.0;
    }
    false
}

/// Recompute popup item visibility, geometry, and hitboxes after widget state changes.
pub fn sync_menu_popups(world: &mut World) {
    let mut menu_query = world.query_filtered::<Entity, With<MenuControl>>();
    let menus: Vec<_> = menu_query.iter(world).collect();
    for owner in menus {
        let Some(owner_layout) = world.get::<UiLayoutResult>(owner).copied() else {
            continue;
        };
        let menu_spacing = world
            .get_resource::<Theme>()
            .map_or_else(|| Theme::default().spacing.menu, |theme| theme.spacing.menu);
        let viewport = {
            let mut roots = world.query::<&UiLayoutRoot>();
            roots
                .iter(world)
                .find(|root| root.window == owner_layout.window)
                .map_or(
                    Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 1280.0,
                        height: 720.0,
                    },
                    |root| root.viewport,
                )
        };
        let (
            open,
            display,
            query_text,
            path,
            width,
            row_height,
            item_entities,
            tree,
            search_order_ids,
            context_position,
            press_position,
            opened_by_pointer,
        ) = {
            let Some(menu) = world.get::<MenuControl>(owner) else {
                continue;
            };
            (
                menu.session.open,
                menu.display,
                menu.query.clone(),
                menu.session.path.clone(),
                menu.width,
                if menu.row_height == 24.0 {
                    menu_spacing.item_height
                } else {
                    menu.row_height
                },
                menu.items.clone(),
                menu.tree.clone(),
                if menu.query.is_empty() {
                    Vec::new()
                } else {
                    menu.tree
                        .search(&menu.search, &menu.query)
                        .into_iter()
                        .map(|entry| entry.id.clone())
                        .collect()
                },
                menu.context_position,
                menu.press_position,
                menu.opened_by_pointer,
            )
        };
        let search_ids: HashSet<String> = search_order_ids.iter().cloned().collect();
        let maximum_menu_depth = tree
            .flattened()
            .into_iter()
            .map(|(parent_path, _, _)| parent_path.len())
            .max()
            .unwrap_or(0);
        let width = width.min(
            ((viewport.width - menu_spacing.popup_padding * 2.0).max(0.0)
                / (maximum_menu_depth + 1) as f32)
                .max(1.0),
        );

        let count = if query_text.is_empty() {
            tree.entries_at(&[]).map_or(0, <[_]>::len)
        } else {
            search_ids.len()
        };
        let (bounds, popup_anchor, pie_center, popup_side) = if display == MenuDisplay::Pie {
            let diameter = menu_spacing
                .pie_popup_diameter
                .min(viewport.width)
                .min(viewport.height);
            let half = diameter * 0.5;
            let requested_center = if opened_by_pointer {
                press_position
            } else {
                context_position
            }
            .unwrap_or([
                owner_layout.rect.x + owner_layout.rect.width * 0.5,
                owner_layout.rect.y + owner_layout.rect.height * 0.5,
            ]);
            let center = [
                requested_center[0].clamp(viewport.x + half, viewport.x + viewport.width - half),
                requested_center[1].clamp(viewport.y + half, viewport.y + viewport.height - half),
            ];
            let bounds = Rect {
                x: center[0] - half,
                y: center[1] - half,
                width: diameter,
                height: diameter,
            };
            (
                bounds,
                owner_layout.rect,
                center,
                crate::menus::PopupSide::Below,
            )
        } else {
            let anchor = context_position.map_or(owner_layout.rect, |position| Rect {
                x: position[0],
                y: position[1],
                width: 1.0,
                height: 1.0,
            });
            let mut popup = world
                .get::<MenuControl>(owner)
                .map_or_else(|| crate::PopupState::new(anchor), |menu| menu.popup);
            popup.anchor = anchor;
            popup.open = open;
            let bounds = popup.place(
                viewport,
                [width, count.max(1) as f32 * row_height],
                crate::menus::PopupSide::Below,
                menu_spacing.popup_padding,
            );
            (bounds, anchor, [0.0, 0.0], popup.side)
        };
        if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
            menu.popup.anchor = popup_anchor;
            menu.popup.bounds = bounds;
            menu.popup.open = open;
            menu.popup.side = popup_side;
            if display == MenuDisplay::Pie {
                if menu.pie.is_none() && open {
                    menu.pie = Some(crate::PieMenu::new(
                        [0.0, 0.0],
                        menu_spacing.pie_inner_radius,
                        menu_spacing.pie_outer_radius,
                    ));
                }
                if let Some(pie) = &mut menu.pie {
                    pie.center = pie_center;
                    pie.dead_zone = menu_spacing.pie_inner_radius;
                    pie.radius = (bounds.width.min(bounds.height) * 0.5
                        - menu_spacing.pie_inner_radius)
                        .max(0.0);
                }
            }
        }
        let owner_hit_rect = if open { bounds } else { owner_layout.rect };
        let owner_hit_z = if open {
            owner_layout.z.saturating_add(90)
        } else {
            owner_layout.z
        };
        world.entity_mut(owner).insert(UiHitBox {
            window: Some(owner_layout.window),
            rect: owner_hit_rect,
            clip: if open {
                Some(viewport)
            } else {
                owner_layout.clip
            },
            z_index: owner_hit_z,
        });

        let item_components: Vec<_> = item_entities
            .iter()
            .filter_map(|entity| {
                world
                    .get::<MenuItemControl>(*entity)
                    .cloned()
                    .map(|item| (*entity, item))
            })
            .collect();
        let pie_parent_path = if path.len() > 1 {
            path[..path.len() - 1].to_vec()
        } else {
            Vec::new()
        };
        let selectable_pie_indices: Vec<_> = tree
            .entries_at(&pie_parent_path)
            .unwrap_or_default()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.enabled && !entry.separator)
            .map(|(index, _)| index)
            .collect();
        let mut search_order: Vec<_> = item_components
            .iter()
            .filter(|(_, item)| search_ids.contains(&item.id))
            .map(|(entity, item)| (*entity, item.clone()))
            .collect();
        search_order.sort_by_key(|(_, item)| {
            search_order_ids
                .iter()
                .position(|id| id == &item.id)
                .unwrap_or(usize::MAX)
        });
        let max_depth = maximum_menu_depth;
        let left_space = bounds.x - viewport.x;
        let right_space = viewport.x + viewport.width - (bounds.x + width);
        let left_fits = left_space >= width * max_depth as f32;
        let right_fits = right_space >= width * max_depth as f32;
        let cascade_right = right_fits || !left_fits;
        let mut search_row = 0usize;
        for (entity, item) in item_components {
            let in_search = !query_text.is_empty();
            let pie_index = selectable_pie_indices
                .iter()
                .position(|index| *index == item.index);
            let visible = if !open {
                false
            } else if in_search {
                search_ids.contains(&item.id)
            } else if display == MenuDisplay::Pie {
                item.parent_path == pie_parent_path && pie_index.is_some()
            } else if item.parent_path.is_empty() {
                true
            } else {
                item.parent_path.len() <= path.len()
                    && path
                        .iter()
                        .take(item.parent_path.len())
                        .eq(item.parent_path.iter())
                    && tree
                        .entries_at(&item.parent_path)
                        .is_some_and(|entries| !entries.is_empty())
            };
            if let Some(mut visible_component) = world.get_mut::<UiVisible>(entity) {
                visible_component.0 = visible;
            } else {
                world.entity_mut(entity).insert(UiVisible(visible));
            }
            if let Some(mut enabled) = world.get_mut::<bui_core::UiEnabled>(entity) {
                enabled.0 = item.enabled && !item.separator;
            }
            if !visible {
                continue;
            }
            let (rect, z) = if display == MenuDisplay::Pie && !in_search {
                let ring_index = pie_index.unwrap_or(0);
                let count = selectable_pie_indices.len().max(1);
                let angle = std::f32::consts::TAU * ring_index as f32 / count as f32
                    - std::f32::consts::FRAC_PI_2;
                let center = pie_center;
                let available_radius = world
                    .get::<MenuControl>(owner)
                    .and_then(|menu| menu.pie.as_ref().map(|pie| pie.radius))
                    .unwrap_or(menu_spacing.pie_outer_radius);
                let desired_radius = if pie_parent_path.is_empty() {
                    menu_spacing.pie_outer_item_radius
                } else {
                    menu_spacing.pie_nested_item_radius
                };
                let radius = desired_radius.min(available_radius);
                (
                    Rect {
                        x: center[0] + angle.cos() * radius - width * 0.5,
                        y: center[1] + angle.sin() * radius - row_height * 0.5,
                        width: width.min(menu_spacing.pie_item_max_width),
                        height: row_height,
                    },
                    owner_layout.z.saturating_add(120),
                )
            } else if in_search {
                let row = search_order
                    .iter()
                    .position(|(search_entity, _)| *search_entity == entity)
                    .unwrap_or_else(|| {
                        let row = search_row;
                        search_row += 1;
                        row
                    });
                (
                    Rect {
                        x: bounds.x,
                        y: bounds.y + row as f32 * row_height,
                        width: bounds.width,
                        height: row_height,
                    },
                    owner_layout.z.saturating_add(100),
                )
            } else {
                let depth = item.parent_path.len();
                let panel_x = if depth == 0 {
                    bounds.x
                } else if cascade_right {
                    bounds.x + depth as f32 * width
                } else {
                    bounds.x - depth as f32 * width
                };
                let panel_y = (item.parent_path.iter().sum::<usize>() + item.index) as f32
                    * row_height
                    + bounds.y;
                (
                    Rect {
                        x: panel_x,
                        y: panel_y,
                        width,
                        height: row_height,
                    },
                    owner_layout
                        .z
                        .saturating_add(100 + item.parent_path.len() as i32 * 10),
                )
            };
            let clip = Some(viewport);
            world.entity_mut(entity).insert((
                UiLayoutResult {
                    window: owner_layout.window,
                    rect,
                    clip,
                    z,
                    content_size: [rect.width, rect.height],
                    dpi_scale: owner_layout.dpi_scale,
                },
                UiHitBox {
                    window: Some(owner_layout.window),
                    rect,
                    clip,
                    z_index: z,
                },
            ));
        }
        if let Some(mut menu) = world.get_mut::<MenuControl>(owner)
            && !open
        {
            menu.query.clear();
        }
    }
}

/// Materialize stable option rows for Dropdown and RadioGroup owners.
pub fn sync_control_option_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let mut root_query = world.query::<&UiLayoutRoot>();
    let viewports: HashMap<_, _> = root_query
        .iter(world)
        .map(|root| (root.window, root.viewport))
        .collect();

    let mut dropdown_query = world.query::<(Entity, &Dropdown, &UiLayoutResult)>();
    let dropdown_views: Vec<_> = dropdown_query
        .iter(world)
        .map(|(owner, dropdown, layout)| {
            (
                owner,
                dropdown.clone(),
                world
                    .get::<Label>(owner)
                    .map_or_else(String::new, |label| label.text.clone()),
                *layout,
                world
                    .get::<UiEnabled>(owner)
                    .is_none_or(|enabled| enabled.0),
                is_visible(world, owner),
            )
        })
        .collect();
    let mut radio_query = world.query::<(Entity, &RadioGroup, &UiLayoutResult)>();
    let radio_views: Vec<_> = radio_query
        .iter(world)
        .map(|(owner, group, layout)| {
            (
                owner,
                group.clone(),
                *layout,
                world
                    .get::<UiEnabled>(owner)
                    .is_none_or(|enabled| enabled.0),
                is_visible(world, owner),
            )
        })
        .collect();

    let current_dropdowns: HashSet<_> = dropdown_views.iter().map(|(owner, ..)| *owner).collect();
    let current_radios: HashSet<_> = radio_views.iter().map(|(owner, ..)| *owner).collect();
    let mut items = world
        .remove_resource::<WidgetControlOptionItems>()
        .unwrap_or_default();
    let stale_dropdowns: Vec<_> = items
        .dropdowns
        .keys()
        .copied()
        .filter(|owner| !current_dropdowns.contains(owner))
        .collect();
    for owner in stale_dropdowns {
        if let Some(entities) = items.dropdowns.remove(&owner) {
            despawn_option_entities(world, owner, entities);
        }
        if world.get_entity(owner).is_ok() {
            world.entity_mut(owner).remove::<DropdownPopupState>();
        }
    }
    let stale_radios: Vec<_> = items
        .radios
        .keys()
        .copied()
        .filter(|owner| !current_radios.contains(owner))
        .collect();
    for owner in stale_radios {
        if let Some(entities) = items.radios.remove(&owner) {
            despawn_option_entities(world, owner, entities);
        }
    }

    let row_height = theme.spacing.menu.item_height.max(1.0);
    let popup_margin = theme.spacing.menu.popup_padding.max(0.0);
    for (owner, dropdown, _label, layout, owner_enabled, owner_visible) in dropdown_views {
        let entities = items.dropdowns.entry(owner).or_default();
        resize_dropdown_option_entities(world, owner, entities, dropdown.options.len());
        let viewport = viewports.get(&layout.window).copied().unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        });
        let max_rows = (viewport.height / row_height).floor().max(1.0) as usize;
        let mut visible_count = dropdown.options.len().min(8).min(max_rows);
        let mut popup_state =
            world
                .get::<DropdownPopupState>(owner)
                .copied()
                .unwrap_or(DropdownPopupState {
                    popup: PopupState::new(layout.rect),
                    viewport,
                    first_visible: 0,
                    visible_count,
                });
        popup_state.popup.anchor = layout.rect;
        popup_state.popup.open = dropdown.open;
        popup_state.viewport = viewport;
        if dropdown.open && visible_count > 0 {
            let requested_height = row_height * visible_count as f32;
            let below_space = (viewport.y + viewport.height
                - (layout.rect.y + layout.rect.height)
                - popup_margin)
                .max(0.0);
            let above_space = (layout.rect.y - viewport.y - popup_margin).max(0.0);
            let side = if below_space >= requested_height {
                PopupSide::Below
            } else if above_space >= requested_height {
                PopupSide::Above
            } else if below_space >= above_space {
                PopupSide::Below
            } else {
                PopupSide::Above
            };
            let available = match side {
                PopupSide::Below => below_space,
                PopupSide::Above => above_space,
                PopupSide::Left | PopupSide::Right => viewport.height,
            };
            visible_count = dropdown
                .options
                .len()
                .min(8)
                .min((available / row_height).floor() as usize)
                .max(1);
            let requested_height = row_height * visible_count as f32;
            popup_state.visible_count = visible_count;
            popup_state.popup.place(
                viewport,
                [layout.rect.width.max(160.0), requested_height],
                side,
                popup_margin,
            );
        } else {
            popup_state.visible_count = visible_count;
            popup_state.popup.bounds = Rect::default();
        }
        let max_first = dropdown.options.len().saturating_sub(visible_count);
        popup_state.first_visible = popup_state.first_visible.min(max_first);
        if dropdown.open
            && let Some(highlighted) = dropdown.highlighted
        {
            if highlighted < popup_state.first_visible {
                popup_state.first_visible = highlighted;
            } else if visible_count > 0 && highlighted >= popup_state.first_visible + visible_count
            {
                popup_state.first_visible =
                    highlighted.saturating_add(1).saturating_sub(visible_count);
            }
        }
        set_dropdown_popup_state(world, owner, popup_state);
        // Keep the trigger hitbox on the trigger. Popup rows have their own hitboxes
        // above it; stretching the owner across the popup makes the header untargetable
        // while open and lets the trigger steal clicks intended for an option.
        set_hitbox(
            world,
            owner,
            UiHitBox {
                window: Some(layout.window),
                rect: to_ui_rect(layout.rect),
                clip: layout.clip.map(to_ui_rect),
                z_index: layout.z,
            },
        );
        for (index, entity) in entities.iter().copied().enumerate() {
            let enabled = owner_enabled && dropdown.enabled;
            let control = DropdownOptionControl {
                owner,
                index,
                label: dropdown.options[index].clone(),
                selected: dropdown.selected == Some(index),
                highlighted: dropdown.highlighted == Some(index),
                enabled,
            };
            set_dropdown_option_control(world, entity, control);
            let visible = owner_visible
                && dropdown.open
                && index >= popup_state.first_visible
                && index < popup_state.first_visible.saturating_add(visible_count);
            set_visible(world, entity, visible);
            set_enabled(world, entity, enabled);
            if visible {
                let row = Rect {
                    x: popup_state.popup.bounds.x,
                    y: popup_state.popup.bounds.y
                        + (index - popup_state.first_visible) as f32 * row_height,
                    width: popup_state.popup.bounds.width,
                    height: row_height,
                };
                let z = layout.z.saturating_add(81);
                set_layout_result(
                    world,
                    entity,
                    UiLayoutResult {
                        window: layout.window,
                        rect: row,
                        clip: Some(popup_state.popup.bounds),
                        z,
                        content_size: [row.width, row.height],
                        dpi_scale: layout.dpi_scale,
                    },
                );
                set_hitbox(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(layout.window),
                        rect: to_ui_rect(row),
                        clip: Some(to_ui_rect(popup_state.popup.bounds)),
                        z_index: z,
                    },
                );
            }
        }
    }

    let control_height = theme.control_height.max(1.0);
    for (owner, group, layout, owner_enabled, owner_visible) in radio_views {
        let entities = items.radios.entry(owner).or_default();
        resize_radio_option_entities(world, owner, entities, group.options.len());
        let clip = layout
            .clip
            .and_then(|parent| layout.rect.intersection(parent))
            .unwrap_or(layout.rect);
        for (index, entity) in entities.iter().copied().enumerate() {
            let option = &group.options[index];
            let enabled = owner_enabled && group.enabled && option.enabled;
            let control = RadioOptionControl {
                owner,
                index,
                label: option.label.clone(),
                enabled,
                selected: group.selected == Some(index),
            };
            set_radio_option_control(world, entity, control);
            let row_y = layout.rect.y + control_height * (index as f32 + 1.0);
            let row = Rect {
                x: layout.rect.x,
                y: row_y,
                width: layout.rect.width,
                height: control_height,
            };
            let visible = owner_visible
                && row_y < layout.rect.y + layout.rect.height
                && row.intersection(clip).is_some();
            set_visible(world, entity, visible);
            set_enabled(world, entity, enabled);
            if visible {
                let z = layout.z.saturating_add(1 + index as i32 * 2);
                set_layout_result(
                    world,
                    entity,
                    UiLayoutResult {
                        window: layout.window,
                        rect: row,
                        clip: Some(clip),
                        z,
                        content_size: [row.width, row.height],
                        dpi_scale: layout.dpi_scale,
                    },
                );
                set_hitbox(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(layout.window),
                        rect: to_ui_rect(row),
                        clip: Some(to_ui_rect(clip)),
                        z_index: z,
                    },
                );
            }
        }
    }
    world.insert_resource(items);
}

fn resize_dropdown_option_entities(
    world: &mut World,
    owner: Entity,
    items: &mut Vec<Entity>,
    count: usize,
) {
    while items.len() > count {
        let entity = items.pop().expect("length was checked");
        if world.get_entity(entity).is_ok() {
            detach_from_parent(world, entity);
            world.despawn(entity);
        }
    }
    while items.len() < count {
        let entity = world
            .spawn((
                UiNode,
                UiVisible(false),
                UiEnabled(false),
                UiRole::new("listbox-option"),
                UiWheelHandled,
            ))
            .id();
        let _ = set_parent(world, entity, owner);
        items.push(entity);
    }
}

fn resize_radio_option_entities(
    world: &mut World,
    owner: Entity,
    items: &mut Vec<Entity>,
    count: usize,
) {
    while items.len() > count {
        let entity = items.pop().expect("length was checked");
        if world.get_entity(entity).is_ok() {
            detach_from_parent(world, entity);
            world.despawn(entity);
        }
    }
    while items.len() < count {
        let entity = world
            .spawn((
                UiNode,
                UiVisible(true),
                UiEnabled(false),
                UiRole::new("radio-button"),
            ))
            .id();
        let _ = set_parent(world, entity, owner);
        items.push(entity);
    }
}

fn despawn_option_entities(world: &mut World, owner: Entity, entities: Vec<Entity>) {
    for entity in entities {
        if world.get_entity(entity).is_ok() {
            detach_from_parent(world, entity);
            world.despawn(entity);
        }
    }
    if world.get_entity(owner).is_ok() {
        world.entity_mut(owner).remove::<DropdownPopupState>();
    }
}

fn set_dropdown_popup_state(world: &mut World, entity: Entity, value: DropdownPopupState) {
    if world.get::<DropdownPopupState>(entity) != Some(&value) {
        world.entity_mut(entity).insert(value);
    }
}

fn set_dropdown_option_control(world: &mut World, entity: Entity, value: DropdownOptionControl) {
    if world.get::<DropdownOptionControl>(entity) != Some(&value) {
        world.entity_mut(entity).insert(value);
    }
}

fn set_radio_option_control(world: &mut World, entity: Entity, value: RadioOptionControl) {
    if world.get::<RadioOptionControl>(entity) != Some(&value) {
        world.entity_mut(entity).insert(value);
    }
}

fn set_visible(world: &mut World, entity: Entity, visible: bool) {
    if let Some(mut current) = world.get_mut::<UiVisible>(entity) {
        if current.0 != visible {
            current.0 = visible;
        }
    } else {
        world.entity_mut(entity).insert(UiVisible(visible));
    }
}

fn set_enabled(world: &mut World, entity: Entity, enabled: bool) {
    if let Some(mut current) = world.get_mut::<UiEnabled>(entity) {
        if current.0 != enabled {
            current.0 = enabled;
        }
    } else {
        world.entity_mut(entity).insert(UiEnabled(enabled));
    }
}

fn set_layout_result(world: &mut World, entity: Entity, layout: UiLayoutResult) {
    if world.get::<UiLayoutResult>(entity) != Some(&layout) {
        world.entity_mut(entity).insert(layout);
    }
}

fn set_hitbox(world: &mut World, entity: Entity, hitbox: UiHitBox) {
    if world.get::<UiHitBox>(entity) != Some(&hitbox) {
        world.entity_mut(entity).insert(hitbox);
    }
}

fn to_ui_rect(rect: Rect) -> UiRect {
    UiRect::new(rect.x, rect.y, rect.width, rect.height)
}

/// Spawn and update only file rows that intersect the current list/grid viewport.
pub fn sync_file_browser_items(world: &mut World) {
    let spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.file_browser,
        |theme| theme.spacing.file_browser,
    );
    let mut query = world.query::<(
        Entity,
        &FileBrowser,
        &UiLayoutResult,
        Option<&FileBrowserViewState>,
    )>();
    let owners: Vec<_> = query
        .iter(world)
        .map(|(owner, browser, layout, view)| {
            let view = view.copied().unwrap_or_default();
            let header_height = if view.header_height == 48.0 {
                spacing.header_height
            } else {
                view.header_height
            }
            .max(1.0);
            let list_row_height = if view.row_height == 22.0 {
                spacing.list_row_height
            } else {
                view.row_height
            }
            .max(1.0);
            let width = layout.rect.width;
            let height = layout.rect.height;
            let (columns, row_height, start_index, max_visible) = if browser.view == FileView::List
            {
                let rows = ((height - header_height) / list_row_height).ceil().max(1.0) as usize;
                (1, list_row_height, view.scroll_rows, rows)
            } else {
                let cell_stride = spacing.grid_cell_width + spacing.grid_gap;
                let row_stride = spacing.grid_cell_height + spacing.grid_gap;
                let columns = (width / cell_stride).floor().max(1.0) as usize;
                let rows = ((height - header_height) / row_stride).ceil().max(1.0) as usize;
                (
                    columns,
                    row_stride,
                    view.scroll_rows.saturating_mul(columns),
                    rows.saturating_mul(columns),
                )
            };
            let entries: Vec<_> = browser
                .visible_entries()
                .into_iter()
                .enumerate()
                .skip(start_index)
                .take(max_visible)
                .enumerate()
                .map(|(slot, (index, entry))| {
                    let (x, y, item_width, item_height) = if browser.view == FileView::List {
                        (
                            layout.rect.x,
                            layout.rect.y + header_height + slot as f32 * row_height,
                            width,
                            row_height,
                        )
                    } else {
                        let col = slot % columns;
                        let row = slot / columns;
                        (
                            layout.rect.x
                                + col as f32 * (spacing.grid_cell_width + spacing.grid_gap),
                            layout.rect.y + header_height + row as f32 * row_height,
                            spacing.grid_cell_width,
                            spacing.grid_cell_height,
                        )
                    };
                    (
                        FileBrowserItem {
                            owner,
                            path: entry.path.clone(),
                            name: entry.name.clone(),
                            index,
                            directory: entry.directory,
                            size: entry.size,
                            selected: browser.selected.contains(&entry.path),
                            preview: entry.preview,
                        },
                        Rect {
                            x,
                            y,
                            width: item_width,
                            height: item_height,
                        },
                        layout.window,
                        layout.clip,
                        layout.dpi_scale,
                        layout.z,
                    )
                })
                .collect();
            (owner, entries)
        })
        .collect();

    for (owner, rows) in owners {
        let previous = world
            .resource::<WidgetFileBrowserItems>()
            .0
            .get(&owner)
            .cloned()
            .unwrap_or_default();
        let mut cache = previous.clone();
        let mut visible_paths = HashSet::new();
        for (item, rect, window, inherited_clip, dpi_scale, base_z) in rows {
            visible_paths.insert(item.path.clone());
            let entity = cache
                .get(&item.path)
                .copied()
                .filter(|entity| world.get_entity(*entity).is_ok())
                .unwrap_or_else(|| {
                    world
                        .spawn((UiNode, UiEnabled(true), UiVisible(true), item.clone()))
                        .id()
                });
            cache.insert(item.path.clone(), entity);
            let clip = inherited_clip
                .and_then(|clip| clip.intersection(rect))
                .unwrap_or(rect);
            let z = base_z.saturating_add(20);
            world.entity_mut(entity).insert((
                UiEnabled(true),
                UiVisible(true),
                item,
                UiLayoutResult {
                    window,
                    rect,
                    clip: Some(clip),
                    z,
                    content_size: [rect.width, rect.height],
                    dpi_scale,
                },
                UiHitBox {
                    window: Some(window),
                    rect,
                    clip: Some(clip),
                    z_index: z,
                },
            ));
            let _ = set_parent(world, entity, owner);
        }
        let stale: Vec<_> = cache
            .keys()
            .filter(|path| !visible_paths.contains(*path))
            .cloned()
            .collect();
        for path in stale {
            let Some(entity) = cache.get(&path).copied() else {
                continue;
            };
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
                cache.remove(&path);
            } else {
                cache.remove(&path);
            }
        }
        world
            .resource_mut::<WidgetFileBrowserItems>()
            .0
            .insert(owner, cache);
    }
}

/// Mount real, clipped Bevy item entities for the rows and preview tiles that
/// intersect each collection viewport. Model ids remain the stable item keys
/// even when virtualization despawns an offscreen entity.
pub fn sync_collection_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let collection_spacing = theme.spacing.collection;
    let list_spacing = theme.spacing.list;
    let header_height = collection_spacing.header_height.max(1.0);
    let item_gap = collection_spacing.item_gap.max(0.0);

    let mut query = world.query::<(
        Entity,
        Option<&ListModel<String>>,
        Option<&TreeModel<String>>,
        Option<&PreviewGrid>,
        Option<&UiLayoutResult>,
    )>();
    let mut views = Vec::new();
    let mut live_owners = HashSet::new();
    let owners: Vec<_> = query
        .iter(world)
        .filter(|(_, list, tree, grid, _)| list.is_some() || tree.is_some() || grid.is_some())
        .map(|(owner, list, tree, grid, layout)| {
            live_owners.insert(owner);
            (
                owner,
                list.cloned(),
                tree.cloned(),
                grid.cloned(),
                layout.copied(),
            )
        })
        .collect();

    for (owner, mut list, mut tree, mut grid, layout) in owners {
        let Some(layout) = layout else {
            views.push(CollectionView {
                owner,
                items: Vec::new(),
            });
            continue;
        };
        let viewport = Rect {
            x: layout.rect.x,
            y: layout.rect.y + header_height,
            width: layout.rect.width.max(0.0),
            height: (layout.rect.height - header_height).max(0.0),
        };
        let viewport_clip = layout
            .clip
            .and_then(|clip| clip.intersection(viewport))
            .unwrap_or(viewport);
        let mut items = Vec::new();

        if let Some(model) = list.as_mut() {
            let row_height = if model.row_height == 22.0 {
                list_spacing.row_height
            } else {
                model.row_height
            }
            .max(1.0);
            let content_height = model.items.len() as f32 * row_height;
            let max_offset = (content_height - viewport.height).max(0.0);
            model.scroll_offset = model.scroll_offset.clamp(0.0, max_offset);
            if viewport.height > 0.0
                && let Some(active) = model.selection.active
                && let Some(index) = model.items.iter().position(|item| item.id == active)
            {
                let top = index as f32 * row_height;
                let bottom = top + row_height;
                if top < model.scroll_offset {
                    model.scroll_offset = top;
                } else if bottom > model.scroll_offset + viewport.height {
                    model.scroll_offset = (bottom - viewport.height).min(max_offset);
                }
            }
            let actual_offset = model.scroll_offset;
            let start = (actual_offset / row_height).floor() as usize;
            let end = start
                .saturating_add((viewport.height / row_height).ceil() as usize + 1)
                .min(model.items.len());
            for index in start..end {
                let item = &model.items[index];
                let control = CollectionItemControl {
                    owner,
                    id: item.id,
                    kind: CollectionItemKind::List,
                    index,
                    depth: 0,
                    label: item.value.clone(),
                    enabled: item.enabled,
                    selected: model.selection.contains(item.id),
                    expanded: false,
                    has_children: false,
                    asset_id: None,
                    tint: [0.34, 0.38, 0.46, 1.0],
                };
                let rect = Rect {
                    x: viewport.x,
                    y: viewport.y + index as f32 * row_height - actual_offset,
                    width: viewport.width,
                    height: row_height,
                };
                if rect.intersection(viewport_clip).is_some() {
                    items.push((control, rect));
                }
            }
            if world
                .get::<ListModel<String>>(owner)
                .is_some_and(|current| current.scroll_offset != actual_offset)
                && let Some(mut current) = world.get_mut::<ListModel<String>>(owner)
            {
                current.scroll_offset = actual_offset;
            }
        } else if let Some(model) = tree.as_mut() {
            let row_height = if model.row_height == 22 {
                list_spacing.row_height
            } else {
                model.row_height.max(1) as f32
            }
            .max(1.0);
            let visible_ids = model.visible_ids();
            let page_rows = (viewport.height / row_height).ceil().max(1.0) as usize;
            if let Some(active_index) = model
                .selection
                .active
                .and_then(|active| visible_ids.iter().position(|id| *id == active))
            {
                if active_index < model.scroll_rows {
                    model.scroll_rows = active_index;
                } else if active_index >= model.scroll_rows.saturating_add(page_rows) {
                    model.scroll_rows = active_index + 1 - page_rows;
                }
            }
            let visible_rows = page_rows.saturating_add(1);
            let max_scroll = visible_ids.len().saturating_sub(page_rows);
            model.scroll_rows = model.scroll_rows.min(max_scroll);
            let start = model.scroll_rows;
            let end = start.saturating_add(visible_rows).min(visible_ids.len());
            for (slot, id) in visible_ids[start..end].iter().copied().enumerate() {
                let Some(node) = model.nodes.iter().find(|node| node.id == id) else {
                    continue;
                };
                let control = CollectionItemControl {
                    owner,
                    id,
                    kind: CollectionItemKind::Tree,
                    index: start + slot,
                    depth: model.depth(id),
                    label: node.value.clone(),
                    enabled: node.enabled,
                    selected: model.selection.contains(id),
                    expanded: node.expanded,
                    has_children: !node.children.is_empty(),
                    asset_id: None,
                    tint: [0.34, 0.38, 0.46, 1.0],
                };
                let rect = Rect {
                    x: viewport.x,
                    y: viewport.y + slot as f32 * row_height,
                    width: viewport.width,
                    height: row_height,
                };
                if rect.intersection(viewport_clip).is_some() {
                    items.push((control, rect));
                }
            }
            let scroll_rows = model.scroll_rows;
            if world
                .get::<TreeModel<String>>(owner)
                .is_some_and(|current| current.scroll_rows != scroll_rows)
                && let Some(mut current) = world.get_mut::<TreeModel<String>>(owner)
            {
                current.scroll_rows = scroll_rows;
            }
        } else if let Some(model) = grid.as_mut() {
            let cell_width = if model.cell_width == 112.0 {
                collection_spacing.preview_cell_width
            } else {
                model.cell_width
            }
            .max(collection_spacing.minimum_preview_cell_size);
            let cell_height = if model.cell_height == 104.0 {
                collection_spacing.preview_cell_height
            } else {
                model.cell_height
            }
            .max(collection_spacing.minimum_preview_cell_size);
            let columns =
                (((viewport.width + item_gap) / (cell_width + item_gap)).floor() as usize).max(1);
            model.columns = columns;
            let viewport_rows = (viewport.height / (cell_height + item_gap)).ceil() as usize;
            let total_rows = model.items.len().div_ceil(columns);
            let page_rows = viewport_rows.max(1);
            if let Some(active_row) = model
                .selection
                .active
                .and_then(|active| model.items.iter().position(|item| item.id == active))
                .map(|index| index / columns)
            {
                if active_row < model.scroll_rows {
                    model.scroll_rows = active_row;
                } else if active_row >= model.scroll_rows.saturating_add(page_rows) {
                    model.scroll_rows = active_row + 1 - page_rows;
                }
            }
            model.scroll_rows = model.scroll_rows.min(total_rows.saturating_sub(page_rows));
            let first = model.scroll_rows.saturating_mul(columns);
            let end = first
                .saturating_add(viewport_rows.saturating_add(1).saturating_mul(columns))
                .min(model.items.len());
            for index in first..end {
                let item = &model.items[index];
                let col = index % columns;
                let row = index / columns - model.scroll_rows;
                let control = CollectionItemControl {
                    owner,
                    id: item.id,
                    kind: CollectionItemKind::PreviewGrid,
                    index,
                    depth: 0,
                    label: item.label.clone(),
                    enabled: item.enabled,
                    selected: model.selection.contains(item.id),
                    expanded: false,
                    has_children: false,
                    asset_id: item.asset_id,
                    tint: item.tint,
                };
                let rect = Rect {
                    x: viewport.x + col as f32 * (cell_width + item_gap),
                    y: viewport.y + row as f32 * (cell_height + item_gap),
                    width: cell_width
                        .min((viewport.width - col as f32 * (cell_width + item_gap)).max(0.0)),
                    height: cell_height,
                };
                if rect.intersection(viewport_clip).is_some() {
                    items.push((control, rect));
                }
            }
            let columns = model.columns;
            let scroll_rows = model.scroll_rows;
            if world.get::<PreviewGrid>(owner).is_some_and(|current| {
                current.columns != columns || current.scroll_rows != scroll_rows
            }) && let Some(mut current) = world.get_mut::<PreviewGrid>(owner)
            {
                current.columns = columns;
                current.scroll_rows = scroll_rows;
            }
        }

        views.push(CollectionView { owner, items });
    }

    let previous = world.resource::<WidgetCollectionItems>().0.clone();
    let mut next = HashMap::new();
    for view in views {
        let old_items = previous.get(&view.owner).cloned().unwrap_or_default();
        let owner_layout = world.get::<UiLayoutResult>(view.owner).copied();
        let mut current_items = HashMap::new();
        if let Some(owner_layout) = owner_layout {
            let owner_clip = owner_layout
                .clip
                .unwrap_or(owner_layout.rect)
                .intersection(Rect {
                    x: owner_layout.rect.x,
                    y: owner_layout.rect.y + header_height,
                    width: owner_layout.rect.width.max(0.0),
                    height: (owner_layout.rect.height - header_height).max(0.0),
                })
                .unwrap_or(Rect::default());
            for (control, rect) in view.items {
                let entity = old_items
                    .get(&control.id)
                    .copied()
                    .filter(|entity| world.get_entity(*entity).is_ok())
                    .unwrap_or_else(|| {
                        world
                            .spawn((UiNode, UiEnabled(control.enabled), UiVisible(true)))
                            .id()
                    });
                let z = owner_layout.z.saturating_add(20);
                let clip = rect.intersection(owner_clip).unwrap_or(owner_clip);
                world.entity_mut(entity).insert((
                    UiEnabled(control.enabled),
                    UiVisible(true),
                    control.clone(),
                    UiLayoutResult {
                        window: owner_layout.window,
                        rect,
                        clip: Some(clip),
                        z,
                        content_size: [rect.width, rect.height],
                        dpi_scale: owner_layout.dpi_scale,
                    },
                    UiHitBox {
                        window: Some(owner_layout.window),
                        rect,
                        clip: Some(clip),
                        z_index: z,
                    },
                ));
                if world
                    .get::<UiParent>(entity)
                    .is_none_or(|parent| parent.0 != view.owner)
                {
                    let _ = set_parent(world, entity, view.owner);
                }
                current_items.insert(control.id, entity);
            }
        }
        for (id, entity) in old_items {
            if !current_items.contains_key(&id) && world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
        next.insert(view.owner, current_items);
    }
    for (owner, items) in previous {
        if live_owners.contains(&owner) {
            continue;
        }
        for entity in items.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
    }
    world.resource_mut::<WidgetCollectionItems>().0 = next;
}

fn insert_component_if_changed<T: Component + PartialEq>(
    world: &mut World,
    entity: Entity,
    component: T,
) {
    if world.get::<T>(entity) != Some(&component) {
        world.entity_mut(entity).insert(component);
    }
}

fn remove_component_if_present<T: Component>(world: &mut World, entity: Entity) {
    if world.get::<T>(entity).is_some() {
        world.entity_mut(entity).remove::<T>();
    }
}

fn virtual_view_stamp<T: Component>(world: &World, owner: Entity) -> Option<VirtualViewStamp> {
    Some(VirtualViewStamp {
        model_changed: entity_component_changed_tick::<T>(world, owner)?,
        layout_changed: entity_component_changed_tick::<UiLayoutResult>(world, owner),
        theme_changed: world
            .get_resource_change_ticks::<Theme>()
            .map(|ticks| ticks.changed.get()),
        key_changed: entity_component_changed_tick::<UiKey>(world, owner),
    })
}

fn entity_component_changed_tick<T: Component>(world: &World, entity: Entity) -> Option<u32> {
    let component_id = world.components().component_id::<T>()?;
    world
        .get_entity(entity)
        .ok()?
        .get_change_ticks_by_id(component_id)
        .map(|ticks| ticks.changed.get())
}

/// Materialize only table columns and rows intersecting the laid out viewport.
/// Child entities remain keyed by column index and stable row ID as sorting and
/// scrolling change their positions.
pub fn sync_data_table_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let title_height = theme.control_height.max(1.0);
    let column_header_height = theme.spacing.table.header_height.max(1.0);

    let mut query = world.query::<(Entity, &DataTable, Option<&UiLayoutResult>)>();
    let owners = query
        .iter(world)
        .filter_map(|(owner, _, layout)| {
            virtual_view_stamp::<DataTable>(world, owner)
                .map(|stamp| (owner, layout.copied(), stamp))
        })
        .collect::<Vec<_>>();
    let live_owners: HashSet<_> = owners.iter().map(|(owner, _, _)| *owner).collect();
    let mut previous = {
        let mut items = world.resource_mut::<WidgetDataTableItems>();
        std::mem::take(&mut items.items)
    };
    let mut stamps = {
        let mut cache = world.resource_mut::<WidgetDataTableItems>();
        std::mem::take(&mut cache.stamps)
    };
    let mut views = Vec::with_capacity(owners.len());
    for (owner, layout, stamp) in owners.iter().copied() {
        if stamps.get(&owner) == Some(&stamp) && previous.contains_key(&owner) {
            continue;
        }
        let Some(layout) = layout else {
            views.push(DataTableView {
                owner,
                items: Vec::new(),
            });
            continue;
        };
        let Some(table) = world.get::<DataTable>(owner) else {
            continue;
        };
        let rect = layout.rect;
        let clip = layout
            .clip
            .unwrap_or(rect)
            .intersection(rect)
            .unwrap_or_default();
        let header_y = rect.y + title_height;
        let body = Rect {
            x: rect.x,
            y: header_y + column_header_height,
            width: rect.width.max(0.0),
            height: (rect.height - title_height - column_header_height).max(0.0),
        };
        let body_clip = clip.intersection(body).unwrap_or_default();
        let row_height = if table.row_height == 24 {
            theme.spacing.table.row_height
        } else {
            table.row_height.max(1) as f32
        };
        let page_rows = (body.height / row_height).floor().max(1.0) as usize;
        let max_scroll = table.rows.len().saturating_sub(page_rows);
        let mut scroll_rows = table.scroll_rows.min(max_scroll);
        if let Some(active_row) = table
            .selection
            .active
            .and_then(|active| table.rows.iter().position(|row| row.id == active))
        {
            if active_row < scroll_rows {
                scroll_rows = active_row;
            } else if active_row >= scroll_rows.saturating_add(page_rows) {
                scroll_rows = active_row + 1 - page_rows;
            }
        }

        let content_width: f32 = table.columns.iter().map(|column| column.width).sum();
        let max_scroll_x = (content_width - body.width).max(0.0);
        let scroll_x = if table.scroll_x.is_finite() {
            table.scroll_x.clamp(0.0, max_scroll_x)
        } else {
            0.0
        };

        let visible_columns: Vec<_> = table
            .columns
            .iter()
            .enumerate()
            .scan(rect.x - scroll_x, |x, (column_index, column)| {
                let item_x = *x;
                *x += column.width;
                let item_rect = Rect {
                    x: item_x,
                    y: header_y,
                    width: column.width,
                    height: column_header_height,
                };
                Some((column_index, item_x, item_rect))
            })
            .filter(|(_, _, item_rect)| item_rect.intersection(clip).is_some())
            .map(|(column_index, item_x, _)| (column_index, item_x))
            .collect();
        let mut items = Vec::new();
        for (column_index, x) in visible_columns {
            let column = &table.columns[column_index];
            let header = DataTableHeaderControl {
                owner,
                column: column_index,
                label: column.label.clone(),
                width: column.width,
                sortable: column.sortable,
                sort_ascending: table
                    .sort
                    .filter(|(sorted, _)| *sorted == column_index)
                    .map(|(_, ascending)| ascending),
                active: table.active_column == column_index,
            };
            items.push(DataTableViewItem {
                key: DataTableItemKey::Header(column_index),
                header: Some(header),
                cell: None,
                rect: Rect {
                    x,
                    y: header_y,
                    width: column.width,
                    height: column_header_height,
                },
                clip: clip
                    .intersection(Rect {
                        x: rect.x,
                        y: header_y,
                        width: rect.width,
                        height: column_header_height,
                    })
                    .unwrap_or_default(),
            });
        }

        let visible_count = (body.height / row_height).ceil() as usize + 1;
        let start = scroll_rows.min(table.rows.len());
        let end = start.saturating_add(visible_count).min(table.rows.len());
        for row_index in start..end {
            let row = &table.rows[row_index];
            let y = body.y + (row_index - start) as f32 * row_height;
            for (column_index, x) in table
                .columns
                .iter()
                .enumerate()
                .scan(rect.x - scroll_x, |x, (column_index, column)| {
                    let item_x = *x;
                    *x += column.width;
                    let item_rect = Rect {
                        x: item_x,
                        y,
                        width: column.width,
                        height: row_height,
                    };
                    Some((column_index, item_x, item_rect))
                })
                .filter(|(_, _, item_rect)| item_rect.intersection(body_clip).is_some())
                .map(|(column_index, item_x, _)| (column_index, item_x))
            {
                let column = &table.columns[column_index];
                let Some(value) = row.cells.get(column_index) else {
                    continue;
                };
                let cell = DataTableCellControl {
                    owner,
                    row_id: row.id,
                    row_index,
                    column: column_index,
                    column_label: column.label.clone(),
                    value: value.clone(),
                    selected: table.selection.contains(row.id),
                    active: table.selection.active == Some(row.id)
                        && table.active_column == column_index,
                };
                items.push(DataTableViewItem {
                    key: DataTableItemKey::Cell {
                        row_id: row.id,
                        column: column_index,
                    },
                    header: None,
                    cell: Some(cell),
                    rect: Rect {
                        x,
                        y,
                        width: column.width,
                        height: row_height,
                    },
                    clip: body_clip,
                });
            }
        }

        if world.get::<DataTable>(owner).is_some_and(|current| {
            current.scroll_rows != scroll_rows || current.scroll_x != scroll_x
        }) && let Some(mut current) = world.get_mut::<DataTable>(owner)
        {
            current.scroll_rows = scroll_rows;
            current.scroll_x = scroll_x;
        }
        views.push(DataTableView { owner, items });
    }

    for view in views {
        let mut current_items = previous.remove(&view.owner).unwrap_or_default();
        let mut stale_keys: Vec<_> = current_items.keys().copied().collect();
        let owner_layout = world.get::<UiLayoutResult>(view.owner).copied();
        if let Some(owner_layout) = owner_layout {
            for view_item in view.items {
                stale_keys.retain(|key| *key != view_item.key);
                let entity = current_items
                    .get(&view_item.key)
                    .copied()
                    .filter(|entity| world.get_entity(*entity).is_ok())
                    .unwrap_or_else(|| {
                        world.spawn((UiNode, UiEnabled(true), UiVisible(true))).id()
                    });
                let clip = view_item
                    .clip
                    .intersection(owner_layout.clip.unwrap_or(owner_layout.rect))
                    .unwrap_or_default();
                let z = owner_layout.z.saturating_add(20);
                insert_component_if_changed(world, entity, UiEnabled(true));
                insert_component_if_changed(world, entity, UiVisible(true));
                insert_component_if_changed(
                    world,
                    entity,
                    UiLayoutResult {
                        window: owner_layout.window,
                        rect: view_item.rect,
                        clip: Some(clip),
                        z,
                        content_size: [view_item.rect.width, view_item.rect.height],
                        dpi_scale: owner_layout.dpi_scale,
                    },
                );
                insert_component_if_changed(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(owner_layout.window),
                        rect: view_item.rect,
                        clip: Some(clip),
                        z_index: z,
                    },
                );
                if let Some(header) = view_item.header {
                    insert_component_if_changed(world, entity, header.clone());
                    let header_role = UiRole::new("table-header");
                    insert_component_if_changed(world, entity, header_role);
                    set_ui_style_state(world, entity, UiStyleState::ACTIVE, header.active);
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, false);
                    remove_component_if_present::<DataTableCellControl>(world, entity);
                } else if let Some(cell) = view_item.cell {
                    insert_component_if_changed(world, entity, cell.clone());
                    let cell_role = UiRole::new("table-cell");
                    insert_component_if_changed(world, entity, cell_role);
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, cell.selected);
                    set_ui_style_state(world, entity, UiStyleState::ACTIVE, cell.active);
                    remove_component_if_present::<DataTableHeaderControl>(world, entity);
                }
                let _ = set_parent(world, entity, view.owner);
                current_items.insert(view_item.key, entity);
            }
        }
        for key in stale_keys {
            if let Some(entity) = current_items.remove(&key)
                && world.get_entity(entity).is_ok()
            {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
        previous.insert(view.owner, current_items);
    }
    let stale_owners: Vec<_> = previous
        .keys()
        .filter(|owner| !live_owners.contains(owner))
        .copied()
        .collect();
    for owner in stale_owners {
        let Some(items) = previous.remove(&owner) else {
            continue;
        };
        for entity in items.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
    }
    for (owner, _, stamp) in owners {
        stamps.insert(owner, stamp);
    }
    stamps.retain(|owner, _| live_owners.contains(owner));
    let mut cache = world.resource_mut::<WidgetDataTableItems>();
    cache.items = previous;
    cache.stamps = stamps;
}

/// Materialize clipped, stable node/socket/link accessibility and hit-test children.
pub fn sync_node_graph_views(world: &mut World) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let spacing = &theme.spacing.node_graph;
    let mut query = world.query::<(Entity, &NodeGraph, Option<&UiLayoutResult>)>();
    let owners = query
        .iter(world)
        .filter_map(|(owner, _, layout)| {
            virtual_view_stamp::<NodeGraph>(world, owner)
                .map(|stamp| (owner, layout.copied(), stamp))
        })
        .collect::<Vec<_>>();
    let live_owners: HashSet<_> = owners.iter().map(|(owner, _, _)| *owner).collect();
    let mut previous = {
        let mut items = world.resource_mut::<WidgetNodeGraphItems>();
        std::mem::take(&mut items.items)
    };
    let mut stamps = {
        let mut cache = world.resource_mut::<WidgetNodeGraphItems>();
        std::mem::take(&mut cache.stamps)
    };
    let mut views = Vec::new();

    for (owner, layout, stamp) in owners.iter().copied() {
        if stamps.get(&owner) == Some(&stamp) && previous.contains_key(&owner) {
            continue;
        }
        let Some(layout) = layout else {
            views.push(NodeGraphView {
                owner,
                items: Vec::new(),
            });
            continue;
        };
        let Some(graph) = world.get::<NodeGraph>(owner) else {
            continue;
        };
        let viewport = layout
            .clip
            .unwrap_or(layout.rect)
            .intersection(layout.rect)
            .and_then(|clip| {
                clip.intersection(Rect {
                    y: layout.rect.y + spacing.header_height,
                    height: (layout.rect.height - spacing.header_height).max(0.0),
                    ..layout.rect
                })
            })
            .unwrap_or_default();
        let mut items = Vec::new();
        let socket_count = graph.nodes.iter().map(|node| node.sockets.len()).sum();
        let mut socket_positions = HashMap::with_capacity(socket_count);
        for node in &graph.nodes {
            let rect = node_graph_node_rect(graph, layout.rect, node, spacing);
            let Some(clip) = rect.intersection(viewport) else {
                // Socket positions are still needed to resolve links whose nodes sit outside
                // the viewport while their curves cross it.
                cache_node_graph_socket_positions(
                    graph.zoom,
                    node,
                    rect,
                    spacing,
                    &mut socket_positions,
                );
                continue;
            };
            items.push(NodeGraphViewItem {
                key: NodeGraphItemKey::Node(node.id),
                node: Some(NodeGraphNodeControl {
                    owner,
                    node_id: node.id,
                    label: node.title.clone(),
                    selected: graph.selected.contains(&node.id),
                    collapsed: node.collapsed,
                }),
                socket: None,
                link: None,
                rect,
                clip,
                z: layout.z.saturating_add(4),
            });
            cache_node_graph_socket_positions(
                graph.zoom,
                node,
                rect,
                spacing,
                &mut socket_positions,
            );
            for (socket_index, socket) in node.sockets.iter().enumerate() {
                let Some(center) = socket_positions.get(&(node.id, socket_index)).copied() else {
                    continue;
                };
                let socket_rect = Rect::new(center[0] - 8.0, center[1] - 8.0, 16.0, 16.0);
                let Some(socket_clip) = socket_rect.intersection(viewport) else {
                    continue;
                };
                items.push(NodeGraphViewItem {
                    key: NodeGraphItemKey::Socket(node.id, socket_index),
                    node: None,
                    socket: Some(NodeGraphSocketControl {
                        owner,
                        node_id: node.id,
                        socket_index,
                        direction: socket.direction,
                        label: socket.name.clone(),
                        socket_type: socket.socket_type,
                    }),
                    link: None,
                    rect: socket_rect,
                    clip: socket_clip,
                    z: layout.z.saturating_add(6),
                });
            }
        }
        for link in &graph.links {
            let Some(from) = socket_positions.get(&(link.from_node, link.from_socket)) else {
                continue;
            };
            let Some(to) = socket_positions.get(&(link.to_node, link.to_socket)) else {
                continue;
            };
            let tangent =
                ((to[0] - from[0]).abs() * 0.5).max(spacing.connection_tangent_min * graph.zoom);
            let direction = if to[0] >= from[0] { 1.0 } else { -1.0 };
            let points = [
                *from,
                [from[0] + tangent * direction, from[1]],
                [to[0] - tangent * direction, to[1]],
                *to,
            ];
            let min_x = points
                .iter()
                .map(|point| point[0])
                .fold(f32::INFINITY, f32::min);
            let max_x = points
                .iter()
                .map(|point| point[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let min_y = points
                .iter()
                .map(|point| point[1])
                .fold(f32::INFINITY, f32::min);
            let max_y = points
                .iter()
                .map(|point| point[1])
                .fold(f32::NEG_INFINITY, f32::max);
            let rect = Rect::new(
                min_x - 8.0,
                min_y - 8.0,
                (max_x - min_x).max(16.0) + 16.0,
                (max_y - min_y).max(16.0) + 16.0,
            );
            let Some(clip) = rect.intersection(viewport) else {
                continue;
            };
            items.push(NodeGraphViewItem {
                key: NodeGraphItemKey::Link(link.id),
                node: None,
                socket: None,
                link: Some(NodeGraphLinkControl {
                    owner,
                    link_id: link.id,
                    from_node: link.from_node,
                    from_socket: link.from_socket,
                    to_node: link.to_node,
                    to_socket: link.to_socket,
                    selected: graph.selected_link == Some(link.id),
                }),
                rect,
                clip,
                z: layout.z.saturating_add(2),
            });
        }
        views.push(NodeGraphView { owner, items });
    }

    for view in views {
        let mut current_items = previous.remove(&view.owner).unwrap_or_default();
        let mut stale_keys: Vec<_> = current_items.keys().copied().collect();
        let owner_layout = world.get::<UiLayoutResult>(view.owner).copied();
        let owner_key = world.get::<UiKey>(view.owner).map_or_else(
            || format!("entity:{:016x}", view.owner.to_bits()),
            |key| key.0.clone(),
        );
        if let Some(owner_layout) = owner_layout {
            for view_item in view.items {
                stale_keys.retain(|key| *key != view_item.key);
                let entity = current_items
                    .get(&view_item.key)
                    .copied()
                    .filter(|entity| world.get_entity(*entity).is_ok())
                    .unwrap_or_else(|| {
                        world.spawn((UiNode, UiEnabled(true), UiVisible(true))).id()
                    });
                insert_component_if_changed(world, entity, UiEnabled(true));
                insert_component_if_changed(world, entity, UiVisible(true));
                insert_component_if_changed(
                    world,
                    entity,
                    UiLayoutResult {
                        window: owner_layout.window,
                        rect: view_item.rect,
                        clip: Some(view_item.clip),
                        z: view_item.z,
                        content_size: [view_item.rect.width, view_item.rect.height],
                        dpi_scale: owner_layout.dpi_scale,
                    },
                );
                insert_component_if_changed(
                    world,
                    entity,
                    UiHitBox {
                        window: Some(owner_layout.window),
                        rect: view_item.rect,
                        clip: Some(view_item.clip),
                        z_index: view_item.z,
                    },
                );
                let (key_suffix, role) = match view_item.key {
                    NodeGraphItemKey::Node(id) => (format!("node:{}", id.0), "node-graph-node"),
                    NodeGraphItemKey::Socket(id, socket) => {
                        (format!("socket:{}:{socket}", id.0), "node-graph-socket")
                    }
                    NodeGraphItemKey::Link(id) => (format!("link:{id}"), "node-graph-link"),
                };
                insert_component_if_changed(
                    world,
                    entity,
                    UiKey(format!("{owner_key}/{key_suffix}")),
                );
                insert_component_if_changed(world, entity, UiRole::new(role));
                if let Some(node) = view_item.node {
                    insert_component_if_changed(world, entity, node.clone());
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, node.selected);
                    remove_component_if_present::<NodeGraphSocketControl>(world, entity);
                    remove_component_if_present::<NodeGraphLinkControl>(world, entity);
                } else if let Some(socket) = view_item.socket {
                    insert_component_if_changed(world, entity, socket);
                    remove_component_if_present::<NodeGraphNodeControl>(world, entity);
                    remove_component_if_present::<NodeGraphLinkControl>(world, entity);
                } else if let Some(link) = view_item.link {
                    insert_component_if_changed(world, entity, link);
                    set_ui_style_state(world, entity, UiStyleState::SELECTED, link.selected);
                    remove_component_if_present::<NodeGraphNodeControl>(world, entity);
                    remove_component_if_present::<NodeGraphSocketControl>(world, entity);
                }
                if world
                    .get::<UiParent>(entity)
                    .is_none_or(|parent| parent.0 != view.owner)
                {
                    let _ = set_parent(world, entity, view.owner);
                }
                current_items.insert(view_item.key, entity);
            }
        }
        for key in stale_keys {
            if let Some(entity) = current_items.remove(&key)
                && world.get_entity(entity).is_ok()
            {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
        previous.insert(view.owner, current_items);
    }
    let stale_owners: Vec<_> = previous
        .keys()
        .filter(|owner| !live_owners.contains(owner))
        .copied()
        .collect();
    for owner in stale_owners {
        let Some(items) = previous.remove(&owner) else {
            continue;
        };
        for entity in items.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
        }
    }
    for (owner, _, stamp) in owners {
        stamps.insert(owner, stamp);
    }
    stamps.retain(|owner, _| live_owners.contains(owner));
    let mut cache = world.resource_mut::<WidgetNodeGraphItems>();
    cache.items = previous;
    cache.stamps = stamps;
}

/// Keep legacy display models synchronized with the shared byte-indexed text buffer.
pub fn sync_text_controls(world: &mut World) {
    let batch = bui_text::take_text_sync(world);
    if batch.as_ref().is_some_and(bui_core::UiDirtyBatch::is_empty) {
        return;
    }
    let mut query = world.query::<(
        Entity,
        &TextBuffer,
        Option<&Label>,
        Option<&TextField>,
        Option<&TextArea>,
    )>();
    let buffers: Vec<_> = query
        .iter(world)
        .filter(|(entity, _, _, _, _)| {
            batch
                .as_ref()
                .is_none_or(|batch| batch.all || batch.entities.contains(entity))
        })
        .map(|(entity, buffer, label, field, area)| {
            let label = label.map_or_else(String::new, |label| label.text.clone());
            let placeholder = field
                .map(|field| field.placeholder.clone())
                .or_else(|| area.map(|area| area.field.placeholder.clone()))
                .unwrap_or_default();
            let wrap = area.is_some_and(|area| area.wrap);
            let scroll_line = area.map_or(0, |area| area.scroll_line);
            (
                entity,
                buffer.as_str().to_owned(),
                buffer.caret(),
                buffer.selection(),
                world.get::<UiReadOnly>(entity).is_some(),
                label,
                placeholder,
                wrap,
                scroll_line,
            )
        })
        .collect();
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let mut changed_text_layouts = Vec::new();
    for (entity, text, caret, selection, read_only, label, placeholder, wrap, scroll_line) in
        buffers
    {
        let entity_theme = styled_theme(world, entity, &theme);
        let caret_chars = text[..caret.min(text.len())].chars().count();
        let selection_chars = (selection.start != selection.end).then(|| {
            text[..selection.start.min(text.len())].chars().count()
                ..text[..selection.end.min(text.len())].chars().count()
        });
        let mut display_scroll_line = scroll_line;
        if let Some(layout) = world.get::<UiTextLayout>(entity)
            && layout.multiline
        {
            let line_height = text_line_height_pixels(layout, &entity_theme);
            display_scroll_line = (layout.scroll_offset[1].max(0.0) / line_height).round() as usize;
        }
        let field_changed = world.get::<TextField>(entity).is_some_and(|field| {
            field.text != text
                || field.caret != caret_chars
                || field.selection != selection_chars
                || field.read_only != read_only
        });
        if field_changed && let Some(mut field) = world.get_mut::<TextField>(entity) {
            field.text.clone_from(&text);
            field.caret = caret_chars;
            field.selection = selection_chars.clone();
            field.read_only = read_only;
        }
        let area_changed = world.get::<TextArea>(entity).is_some_and(|area| {
            area.field.text != text
                || area.field.caret != caret_chars
                || area.field.selection != selection_chars
                || area.field.read_only != read_only
                || area.scroll_line != display_scroll_line
        });
        if area_changed && let Some(mut area) = world.get_mut::<TextArea>(entity) {
            area.field.text.clone_from(&text);
            area.field.caret = caret_chars;
            area.field.selection = selection_chars;
            area.field.read_only = read_only;
            area.scroll_line = display_scroll_line;
        }
        let prefix = if label.is_empty() {
            String::new()
        } else {
            format!("{label}: ")
        };
        let layout_changed = world.get::<UiTextLayout>(entity).is_some_and(|layout| {
            layout.prefix != prefix
                || layout.placeholder != placeholder
                || (layout.multiline && layout.wrap != wrap)
        });
        if layout_changed && let Some(mut layout) = world.get_mut::<UiTextLayout>(entity) {
            layout.prefix = prefix;
            layout.placeholder = placeholder;
            if layout.multiline {
                layout.wrap = wrap;
            }
        }
        if field_changed || area_changed || layout_changed {
            changed_text_layouts.push(entity);
        }
    }
    if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
        for entity in changed_text_layouts {
            dirty.mark(UiDirtyDomain::TextLayout, entity);
        }
    }
}

fn scroll_text_area(world: &mut World, target: Entity, delta_y: f32, unit: bui_core::UiWheelUnit) {
    if delta_y == 0.0 {
        return;
    }
    let base_theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let theme = styled_theme(world, target, &base_theme);
    let line_height = world
        .get::<UiTextLayout>(target)
        .map_or(theme.font_size * theme.line_height, |layout| {
            text_line_height_pixels(layout, &theme)
        });
    let steps = match unit {
        bui_core::UiWheelUnit::Lines => delta_y.abs().ceil(),
        bui_core::UiWheelUnit::Pixels => (delta_y.abs() / line_height).ceil().max(1.0),
    } as usize;
    let scroll_line = {
        let Some(mut area) = world.get_mut::<TextArea>(target) else {
            return;
        };
        area.scroll_line = if delta_y > 0.0 {
            area.scroll_line.saturating_sub(steps)
        } else {
            area.scroll_line.saturating_add(steps)
        };
        area.scroll_line
    };
    if let Some(mut layout) = world.get_mut::<UiTextLayout>(target)
        && layout.multiline
    {
        layout.scroll_offset[1] = line_height * scroll_line as f32;
    }
}

fn text_line_height_pixels(layout: &UiTextLayout, theme: &Theme) -> f32 {
    let font_size = layout
        .font
        .as_ref()
        .map_or(theme.font_size, |font| match font.font_size {
            FontSize::Px(size) => size,
            FontSize::Rem(size) => size * theme.font_size,
            _ => theme.font_size,
        });
    match layout
        .line_height
        .unwrap_or(LineHeight::RelativeToFont(theme.line_height))
    {
        LineHeight::Px(pixels) => pixels.max(1.0),
        LineHeight::RelativeToFont(relative) => (font_size * relative).max(1.0),
    }
}

fn styled_theme(world: &World, entity: Entity, base: &Theme) -> Theme {
    let mut theme = base.clone();
    let Some(style) = world.get::<UiComputedStyle>(entity) else {
        return theme;
    };
    if let Some(background) = style.color(UiStyleProperty::Background) {
        theme.background = background;
        theme.surface = background;
        theme.surface_hover = background;
        theme.surface_pressed = background;
    }
    if let Some(foreground) = style.color(UiStyleProperty::Foreground) {
        theme.text = foreground;
        theme.text_muted = foreground;
        theme.disabled = foreground;
    }
    if let Some(muted) = style.color(UiStyleProperty::MutedForeground) {
        theme.text_muted = muted;
    }
    if let Some(border) = style.color(UiStyleProperty::BorderColor) {
        theme.border = border;
    }
    if let Some(accent) = style.color(UiStyleProperty::AccentColor) {
        theme.accent = accent;
    }
    if let Some(selection) = style.color(UiStyleProperty::SelectionColor) {
        theme.selection = selection;
    }
    if let Some(focus) = style.color(UiStyleProperty::FocusColor) {
        theme.focus = focus;
    }
    if let Some(disabled) = style.color(UiStyleProperty::DisabledColor) {
        theme.disabled = disabled;
    }
    if let Some(error) = style.color(UiStyleProperty::ErrorColor) {
        theme.error = error;
    }
    if let Some(radius) = style.scalar(UiStyleProperty::CornerRadius)
        && radius.is_finite()
    {
        theme.corner_radius = radius.max(0.0);
    }
    if let Some(font_size) = style.scalar(UiStyleProperty::FontSize)
        && font_size.is_finite()
        && font_size > 0.0
    {
        theme.font_size = font_size;
    }
    if let Some(line_height) = style.scalar(UiStyleProperty::LineHeight)
        && line_height.is_finite()
        && line_height > 0.0
    {
        theme.line_height = line_height;
    }
    let opacity = style
        .scalar(UiStyleProperty::Opacity)
        .filter(|opacity| opacity.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    for color in [
        &mut theme.background,
        &mut theme.surface,
        &mut theme.surface_hover,
        &mut theme.surface_pressed,
        &mut theme.text,
        &mut theme.text_muted,
        &mut theme.accent,
        &mut theme.border,
        &mut theme.selection,
        &mut theme.disabled,
        &mut theme.error,
        &mut theme.focus,
        &mut theme.disabled_surface,
        &mut theme.disabled_border,
    ] {
        color.3 *= opacity;
    }
    theme
}

#[expect(
    clippy::type_complexity,
    reason = "Bevy change filters enumerate inherited paint state"
)]
#[derive(SystemParam)]
struct WidgetPaintInvalidations<'w, 's> {
    parent_changes: Query<'w, 's, Entity, Changed<UiParent>>,
    children_changes: Query<'w, 's, Entity, Changed<UiChildren>>,
    root_changes: Query<'w, 's, Entity, Changed<UiLayoutRoot>>,
    inherited_state: Query<'w, 's, Entity, Or<(Changed<UiEnabled>, Changed<UiVisible>)>>,
    children: Query<'w, 's, &'static UiChildren>,
    canvases: Query<'w, 's, Entity, With<Canvas>>,
}

/// Rebuild the shared draw list from current widget state and the completed layout pass.
#[expect(
    clippy::type_complexity,
    reason = "Bevy change queries enumerate widget inputs"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects the independent widget queries"
)]
fn collect_widget_paint_changes(
    invalidations: WidgetPaintInvalidations,
    base: Query<
        Entity,
        Or<(
            Changed<UiNode>,
            Changed<UiParent>,
            Changed<UiChildren>,
            Changed<UiEnabled>,
            Changed<UiVisible>,
            Changed<UiLayoutResult>,
            Changed<UiLayoutRoot>,
            Changed<UiComputedStyle>,
            Changed<UiRole>,
            Changed<UiTextLayout>,
            Changed<TextBuffer>,
        )>,
    >,
    semantic_base: Query<
        Entity,
        Or<(
            Changed<UiNode>,
            Changed<UiParent>,
            Changed<UiChildren>,
            Changed<UiEnabled>,
            Changed<UiVisible>,
            Changed<UiLayoutResult>,
            Changed<TextBuffer>,
        )>,
    >,
    controls: Query<
        Entity,
        Or<(
            Changed<Button>,
            Changed<Checkbox>,
            Changed<IconButton>,
            Changed<Label>,
            Changed<RadioGroup>,
            Changed<NumericField>,
            Changed<Slider>,
            Changed<TextField>,
            Changed<TextArea>,
            Changed<Dropdown>,
            Changed<Panel>,
            Changed<Progress>,
            Changed<MenuControl>,
            Changed<MenuItemControl>,
        )>,
    >,
    models_a: Query<
        Entity,
        Or<(
            Changed<DropdownOptionControl>,
            Changed<FileBrowser>,
            Changed<FileBrowserItem>,
            Changed<ListModel<String>>,
            Changed<TreeModel<String>>,
            Changed<PreviewGrid>,
            Changed<CollectionItemControl>,
            Changed<DataTable>,
        )>,
    >,
    models_b: Query<
        Entity,
        Or<(
            Changed<DataTableHeaderControl>,
            Changed<DataTableCellControl>,
            Changed<ColorRamp>,
            Changed<HsvColorEditor>,
            Changed<CurveEditor>,
            Changed<NodeGraph>,
            Changed<Canvas>,
            Changed<Tooltip>,
        )>,
    >,
    states: Query<
        Entity,
        Or<(
            Changed<IconButtonAsset>,
            Changed<DropdownPopupState>,
            Changed<CollectionEditorState>,
            Changed<HsvColorEditorState>,
            Changed<ColorRampEditorState>,
            Changed<NodeGraphEditorState>,
            Changed<FileBrowserViewState>,
        )>,
    >,
    accessibility: Query<
        Entity,
        Or<(
            Changed<UiFocusable>,
            Changed<UiLiveRegion>,
            Changed<UiReadOnly>,
            Changed<UiKey>,
            Changed<crate::canvas::CanvasObjectControl>,
            Changed<crate::IconButtonAssetState>,
            Changed<NodeGraphNodeControl>,
            Changed<NodeGraphSocketControl>,
            Changed<NodeGraphLinkControl>,
            Changed<crate::PanelGroup>,
            Changed<RadioOptionControl>,
        )>,
    >,
    theme: Res<Theme>,
    interaction: Res<WidgetInteractionState>,
    focus: Option<Res<UiFocus>>,
    canvas_images: Option<Res<crate::canvas::CanvasImageAssets>>,
    mut cache: ResMut<WidgetPaintCache>,
    mut dirty: ResMut<UiDirty>,
) {
    let active_window = focus.as_ref().and_then(|focus| focus.active_window);
    let mut focus_targets: Vec<_> = focus
        .iter()
        .flat_map(|focus| &focus.windows)
        .map(|(window, state)| FocusTargetSnapshot {
            window: *window,
            focused: state.focused,
            hovered: state.hovered,
            captured: state.captured,
        })
        .collect();
    focus_targets.sort_unstable_by_key(|targets| targets.window);
    let focus_changed =
        cache.focus_active_window != active_window || cache.focus_targets != focus_targets;
    if focus_changed {
        for target in cache
            .focus_targets
            .iter()
            .chain(focus_targets.iter())
            .flat_map(|targets| [targets.focused, targets.hovered, targets.captured])
            .flatten()
        {
            dirty.mark(UiDirtyDomain::Paint, target);
        }
        cache.focus_active_window = active_window;
        cache.focus_targets = focus_targets;
    }
    if interaction.is_changed() {
        let targets = interaction.paint_targets();
        for target in cache.interaction_targets.union(&targets).copied() {
            dirty.mark(UiDirtyDomain::Paint, target);
        }
        cache.interaction_targets = targets;
    }
    let topology_changed = !invalidations.parent_changes.is_empty()
        || !invalidations.children_changes.is_empty()
        || !invalidations.root_changes.is_empty();
    if topology_changed {
        cache
            .pending_parent_changes
            .extend(invalidations.parent_changes.iter());
        cache
            .pending_children_changes
            .extend(invalidations.children_changes.iter());
    }
    for entity in &invalidations.parent_changes {
        mark_widget_paint_subtree(entity, &invalidations.children, &mut dirty);
    }
    for entity in &invalidations.root_changes {
        mark_widget_paint_subtree(entity, &invalidations.children, &mut dirty);
    }
    if theme.is_changed() {
        dirty.mark_all(UiDirtyDomain::Paint);
    }
    for entity in &invalidations.inherited_state {
        mark_widget_paint_subtree(entity, &invalidations.children, &mut dirty);
    }
    if canvas_images
        .as_ref()
        .is_some_and(|images| images.is_changed())
    {
        for entity in &invalidations.canvases {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
    for entity in &base {
        dirty.mark(UiDirtyDomain::Paint, entity);
    }
    for entity in &semantic_base {
        dirty.mark(UiDirtyDomain::Accessibility, entity);
    }
    for entity in controls
        .iter()
        .chain(&models_a)
        .chain(&models_b)
        .chain(&states)
    {
        dirty.mark(UiDirtyDomain::Paint, entity);
        dirty.mark(UiDirtyDomain::Accessibility, entity);
    }
    for entity in &accessibility {
        dirty.mark(UiDirtyDomain::Accessibility, entity);
    }
    cache.prepared = true;
}

pub fn collect_widget_paint(world: &mut World) {
    world.init_resource::<WidgetPaintCache>();
    if std::mem::take(&mut world.resource_mut::<WidgetPaintCache>().prepared) {
        let batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
        if batch.is_empty() {
            world.resource_mut::<WidgetPaintCache>().skipped_frames += 1;
            return;
        }
        collect_widget_paint_full(world, None, Some(batch));
        return;
    }
    let input_stamp = widget_paint_input_stamp(world);
    if world.resource::<WidgetPaintCache>().input_stamp == Some(input_stamp) {
        world.resource_mut::<WidgetPaintCache>().skipped_frames += 1;
        return;
    }

    collect_widget_paint_full(world, Some(input_stamp), None);
}

fn widget_paint_order(world: &World, layout_entities: &[Entity]) -> Vec<Entity> {
    let layout_entities: HashSet<_> = layout_entities.iter().copied().collect();
    let mut nodes: Vec<_> = world
        .iter_entities()
        .filter(|entity| {
            entity.contains::<UiNode>()
                || entity.contains::<UiParent>()
                || entity.contains::<UiChildren>()
                || entity.contains::<UiLayoutResult>()
        })
        .map(|entity| entity.id())
        .collect();
    nodes.sort_unstable_by_key(|entity| entity.index());
    nodes.dedup();
    let node_set: HashSet<_> = nodes.iter().copied().collect();

    let mut children_by_parent: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut assigned_parent = HashSet::new();
    for parent in &nodes {
        let Some(children) = world.get::<UiChildren>(*parent) else {
            continue;
        };
        for child in &children.0 {
            if node_set.contains(child) && assigned_parent.insert(*child) {
                children_by_parent.entry(*parent).or_default().push(*child);
            }
        }
    }
    for child in &nodes {
        let Some(parent) = world.get::<UiParent>(*child).map(|parent| parent.0) else {
            continue;
        };
        if node_set.contains(&parent) && assigned_parent.insert(*child) {
            children_by_parent.entry(parent).or_default().push(*child);
        }
    }

    let roots: Vec<_> = nodes
        .iter()
        .copied()
        .filter(|entity| !assigned_parent.contains(entity))
        .collect();
    let mut ordered = Vec::with_capacity(layout_entities.len());
    let mut visited = HashSet::with_capacity(nodes.len());
    for start in roots.iter().chain(nodes.iter()) {
        if visited.contains(start) {
            continue;
        }
        let mut pending = vec![*start];
        while let Some(entity) = pending.pop() {
            if !visited.insert(entity) {
                continue;
            }
            if layout_entities.contains(&entity) {
                ordered.push(entity);
            }
            if let Some(children) = children_by_parent.get(&entity) {
                pending.extend(children.iter().rev().copied());
            }
        }
    }
    ordered
}

fn widget_root_order(world: &World) -> BTreeMap<u32, Entity> {
    let mut nodes: Vec<_> = world
        .iter_entities()
        .filter(|entity| {
            entity.contains::<UiNode>()
                || entity.contains::<UiParent>()
                || entity.contains::<UiChildren>()
                || entity.contains::<UiLayoutResult>()
        })
        .map(|entity| entity.id())
        .collect();
    nodes.sort_unstable_by_key(|entity| entity.index());
    nodes.dedup();
    let node_set: HashSet<_> = nodes.iter().copied().collect();
    let mut assigned_parent = HashSet::new();
    for parent in &nodes {
        if let Some(children) = world.get::<UiChildren>(*parent) {
            assigned_parent.extend(
                children
                    .0
                    .iter()
                    .copied()
                    .filter(|child| node_set.contains(child)),
            );
        }
    }
    for child in &nodes {
        if let Some(parent) = world.get::<UiParent>(*child).map(|parent| parent.0)
            && node_set.contains(&parent)
        {
            assigned_parent.insert(*child);
        }
    }
    nodes
        .into_iter()
        .filter(|entity| !assigned_parent.contains(entity))
        .map(|entity| (entity.index_u32(), entity))
        .collect()
}

fn widget_subtree_paint_order(world: &World, root: Entity) -> Vec<Entity> {
    let mut ordered = Vec::new();
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            continue;
        }
        if world.get::<UiLayoutResult>(entity).is_some() {
            ordered.push(entity);
        }
        if let Some(children) = world.get::<UiChildren>(entity) {
            pending.extend(children.0.iter().rev().copied());
        }
    }
    ordered
}

fn widget_children_snapshot(world: &World) -> HashMap<Entity, Vec<Entity>> {
    let mut children: HashMap<_, _> = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiChildren>()
                .map(|children| (entity.id(), children.0.clone()))
        })
        .collect();
    let mut assigned = HashSet::new();
    for list in children.values() {
        assigned.extend(list.iter().copied());
    }
    let mut fallback: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut parented: Vec<_> = world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiParent>()
                .map(|parent| (entity.id(), parent.0))
        })
        .collect();
    parented.sort_unstable_by_key(|(child, _)| child.index());
    for (child, parent) in parented {
        if !assigned.contains(&child) {
            fallback.entry(parent).or_default().push(child);
        }
    }
    for (parent, children_without_list) in fallback {
        let list = children.entry(parent).or_default();
        list.extend(children_without_list);
    }
    children
}

fn widget_parent_snapshot(world: &World) -> HashMap<Entity, Entity> {
    world
        .iter_entities()
        .filter_map(|entity| {
            entity
                .get::<UiParent>()
                .map(|parent| (entity.id(), parent.0))
        })
        .collect()
}

fn widget_subtree_rank_bounds(
    world: &World,
    root: Entity,
    ranks: &HashMap<Entity, u64>,
) -> Option<(u64, u64)> {
    widget_subtree_paint_order(world, root)
        .iter()
        .filter_map(|entity| ranks.get(entity).copied())
        .fold(None, |bounds, rank| {
            Some(bounds.map_or((rank, rank), |(start, end): (u64, u64)| {
                (start.min(rank), end.max(rank))
            }))
        })
}

fn assign_widget_rank_block(
    subtree: &[Entity],
    previous_ranks: &HashMap<Entity, u64>,
    lower: u64,
    upper: u64,
    ranks: &mut HashMap<Entity, u64>,
    order_keys: &mut BTreeMap<u64, Entity>,
) -> HashSet<Entity> {
    let mut left_selected = Vec::new();
    let mut right_selected = Vec::new();
    let mut left = order_keys
        .range(..=lower)
        .next_back()
        .map(|(rank, entity)| (*rank, *entity));
    let mut right = order_keys
        .range(upper..)
        .next()
        .map(|(rank, entity)| (*rank, *entity));
    let mut floor = left.map_or(0, |(rank, _)| rank);
    let mut ceiling = right.map_or(u64::MAX, |(rank, _)| rank);

    loop {
        let needed = left_selected.len() + subtree.len() + right_selected.len();
        let gap = (ceiling as u128)
            .checked_sub(floor as u128)
            .unwrap_or_else(|| {
                panic!("invalid widget order interval: lower={floor}, upper={ceiling}")
            });
        if gap > needed as u128 {
            break;
        }
        if let Some((rank, entity)) = left {
            left_selected.push((rank, entity));
            left = order_keys
                .range(..rank)
                .next_back()
                .map(|(rank, entity)| (*rank, *entity));
            floor = left.map_or(0, |(rank, _)| rank);
        } else if let Some((rank, entity)) = right {
            right_selected.push((rank, entity));
            right = order_keys
                .range((std::ops::Bound::Excluded(rank), std::ops::Bound::Unbounded))
                .next()
                .map(|(rank, entity)| (*rank, *entity));
            ceiling = right.map_or(u64::MAX, |(rank, _)| rank);
        } else {
            unreachable!("u64 rank interval can always fit a finite widget set");
        }
    }

    let mut relabel: Vec<_> = left_selected.iter().rev().copied().collect();
    relabel.extend(subtree.iter().map(|entity| (0, *entity)));
    relabel.extend(right_selected.iter().copied());
    for (rank, entity) in left_selected.iter().chain(right_selected.iter()) {
        order_keys.remove(rank);
        ranks.remove(entity);
    }
    let step = ((ceiling as u128 - floor as u128) / (relabel.len() as u128 + 1)) as u64;
    debug_assert!(step > 0);

    let mut changed = HashSet::new();
    for (index, (_, entity)) in relabel.iter().enumerate() {
        let rank = floor.saturating_add(step.saturating_mul(index as u64 + 1));
        debug_assert!(!order_keys.contains_key(&rank));
        ranks.insert(*entity, rank);
        order_keys.insert(rank, *entity);
        if previous_ranks.get(entity) != Some(&rank) {
            changed.insert(*entity);
        }
    }
    changed
}

fn move_widget_paint_subtree(
    world: &World,
    root: Entity,
    parent: Option<Entity>,
    ranks: &mut HashMap<Entity, u64>,
    order_keys: &mut BTreeMap<u64, Entity>,
    children_snapshot: &HashMap<Entity, Vec<Entity>>,
    roots: &mut BTreeMap<u32, Entity>,
) -> HashSet<Entity> {
    let subtree = widget_subtree_paint_order(world, root);
    if subtree.is_empty() {
        return HashSet::new();
    }
    let previous_ranks: HashMap<_, _> = subtree
        .iter()
        .filter_map(|entity| ranks.get(entity).map(|rank| (*entity, *rank)))
        .collect();
    for entity in &subtree {
        if let Some(rank) = ranks.remove(entity) {
            order_keys.remove(&rank);
        }
    }

    if parent.is_some() {
        if roots.get(&root.index_u32()) == Some(&root) {
            roots.remove(&root.index_u32());
        }
    } else {
        roots.insert(root.index_u32(), root);
    }
    let (previous_sibling, next_sibling) = if let Some(parent) = parent {
        let fallback_siblings;
        let siblings = if let Some(children) = world.get::<UiChildren>(parent) {
            children.0.as_slice()
        } else {
            fallback_siblings = children_snapshot
                .get(&parent)
                .map(Vec::as_slice)
                .unwrap_or_default();
            fallback_siblings
        };
        let sibling_index = siblings.iter().position(|sibling| *sibling == root);
        let previous = sibling_index.and_then(|index| {
            siblings[..index]
                .iter()
                .rev()
                .find_map(|sibling| widget_subtree_rank_bounds(world, *sibling, ranks))
        });
        let next = sibling_index.and_then(|index| {
            siblings[index + 1..]
                .iter()
                .find_map(|sibling| widget_subtree_rank_bounds(world, *sibling, ranks))
        });
        (previous, next)
    } else {
        let previous = roots
            .range(..root.index_u32())
            .rev()
            .find_map(|(_, sibling)| widget_subtree_rank_bounds(world, *sibling, ranks));
        let next = roots
            .range((
                std::ops::Bound::Excluded(root.index_u32()),
                std::ops::Bound::Unbounded,
            ))
            .find_map(|(_, sibling)| widget_subtree_rank_bounds(world, *sibling, ranks));
        (previous, next)
    };

    let (lower, upper) = match (previous_sibling, next_sibling) {
        (Some((_, end)), Some((start, _))) => (end, start),
        (Some((_, end)), None) => {
            let upper = order_keys
                .range((std::ops::Bound::Excluded(end), std::ops::Bound::Unbounded))
                .next()
                .map(|(rank, _)| *rank)
                .unwrap_or(u64::MAX);
            (end, upper)
        }
        (None, Some((start, _))) => {
            let lower =
                if let Some(parent_rank) = parent.and_then(|parent| ranks.get(&parent).copied()) {
                    parent_rank
                } else if let Some(parent) = parent {
                    let mut top = parent;
                    while let Some(ancestor) = world.get::<UiParent>(top).map(|parent| parent.0) {
                        top = ancestor;
                    }
                    roots
                        .range(..top.index_u32())
                        .rev()
                        .find_map(|(_, root)| widget_subtree_rank_bounds(world, *root, ranks))
                        .map_or(0, |(_, end)| end)
                } else {
                    0
                };
            (lower, start)
        }
        (None, None) => {
            let parent_rank = parent.and_then(|parent| ranks.get(&parent).copied());
            if let Some(parent_rank) = parent_rank {
                let upper = order_keys
                    .range((
                        std::ops::Bound::Excluded(parent_rank),
                        std::ops::Bound::Unbounded,
                    ))
                    .next()
                    .map(|(rank, _)| *rank)
                    .unwrap_or(u64::MAX);
                (parent_rank, upper)
            } else if let Some(parent) = parent {
                let mut top = parent;
                while let Some(ancestor) = world.get::<UiParent>(top).map(|parent| parent.0) {
                    top = ancestor;
                }
                let previous_root = roots
                    .range(..top.index_u32())
                    .rev()
                    .find_map(|(_, root)| widget_subtree_rank_bounds(world, *root, ranks));
                let next_root = roots
                    .range((
                        std::ops::Bound::Excluded(top.index_u32()),
                        std::ops::Bound::Unbounded,
                    ))
                    .find_map(|(_, root)| widget_subtree_rank_bounds(world, *root, ranks));
                match (previous_root, next_root) {
                    (Some((_, end)), Some((start, _))) => (end, start),
                    (Some((_, end)), None) => {
                        let upper = order_keys
                            .range((std::ops::Bound::Excluded(end), std::ops::Bound::Unbounded))
                            .next()
                            .map(|(rank, _)| *rank)
                            .unwrap_or(u64::MAX);
                        (end, upper)
                    }
                    (None, Some((start, _))) => {
                        let lower = order_keys
                            .range(..start)
                            .next_back()
                            .map(|(rank, _)| *rank)
                            .unwrap_or(0);
                        (lower, start)
                    }
                    (None, None) => (0, u64::MAX),
                }
            } else {
                (0, u64::MAX)
            }
        }
    };
    assert!(
        lower <= upper,
        "widget rank interval reversed for {root:?} under {parent:?}: {lower}..{upper}; ui_parent={:?}; ui_children={:?}; ranks={:?}",
        world.get::<UiParent>(root),
        parent
            .and_then(|parent| world.get::<UiChildren>(parent))
            .map(|children| &children.0),
        parent
            .and_then(|parent| world.get::<UiChildren>(parent))
            .map(|children| children
                .0
                .iter()
                .map(|sibling| (*sibling, widget_subtree_rank_bounds(world, *sibling, ranks)))
                .collect::<Vec<_>>())
    );
    assign_widget_rank_block(&subtree, &previous_ranks, lower, upper, ranks, order_keys)
}

fn changed_sibling_roots(old: &[Entity], current: &[Entity]) -> HashSet<Entity> {
    let current_set: HashSet<_> = current.iter().copied().collect();
    let old_positions: HashMap<_, _> = old
        .iter()
        .enumerate()
        .map(|(position, entity)| (*entity, position))
        .collect();
    let mut tails: Vec<usize> = Vec::new();
    let mut tail_indices: Vec<usize> = Vec::new();
    let mut previous = vec![None; current.len()];
    for (index, entity) in current.iter().enumerate() {
        let Some(position) = old_positions.get(entity).copied() else {
            continue;
        };
        let insertion = tails.partition_point(|tail| *tail < position);
        if insertion > 0 {
            previous[index] = Some(tail_indices[insertion - 1]);
        }
        if insertion == tails.len() {
            tails.push(position);
            tail_indices.push(index);
        } else {
            tails[insertion] = position;
            tail_indices[insertion] = index;
        }
    }
    let mut retained = vec![false; current.len()];
    let mut cursor = tail_indices.last().copied();
    while let Some(index) = cursor {
        retained[index] = true;
        cursor = previous[index];
    }
    current
        .iter()
        .enumerate()
        .filter_map(|(index, entity)| (!retained[index]).then_some(*entity))
        .chain(
            old.iter()
                .filter(|entity| !current_set.contains(entity))
                .copied(),
        )
        .collect()
}

fn current_widget_parent(
    world: &World,
    entity: Entity,
    previous_parent: Option<Entity>,
) -> Option<Entity> {
    let parent = world.get::<UiParent>(entity).map(|parent| parent.0);
    if let Some(parent) = parent
        && world
            .get::<UiChildren>(parent)
            .is_some_and(|children| children.0.contains(&entity))
    {
        return Some(parent);
    }
    if let Some(previous_parent) = previous_parent
        && world
            .get::<UiChildren>(previous_parent)
            .is_some_and(|children| children.0.contains(&entity))
    {
        return Some(previous_parent);
    }
    parent.filter(|parent| world.get_entity(*parent).is_ok())
}

fn current_widget_children(
    world: &World,
    parent: Entity,
    previous: &[Entity],
    parent_snapshot: &HashMap<Entity, Entity>,
) -> Vec<Entity> {
    let mut current = world
        .get::<UiChildren>(parent)
        .map(|children| children.0.clone())
        .unwrap_or_default();
    let mut fallback: Vec<_> = previous
        .iter()
        .copied()
        .filter(|child| parent_snapshot.get(child) == Some(&parent) && !current.contains(child))
        .collect();
    fallback.sort_unstable_by_key(|child| child.index());
    current.extend(fallback);
    current
}

fn update_parent_child_snapshot(
    world: &World,
    parent: Entity,
    child: Entity,
    add: bool,
    children_snapshot: &mut HashMap<Entity, Vec<Entity>>,
) {
    if let Some(children) = world.get::<UiChildren>(parent) {
        children_snapshot.insert(parent, children.0.clone());
        return;
    }
    let siblings = children_snapshot.entry(parent).or_default();
    if add {
        if !siblings.contains(&child) {
            let insertion = siblings.partition_point(|sibling| sibling.index() < child.index());
            siblings.insert(insertion, child);
        }
    } else {
        siblings.retain(|sibling| *sibling != child);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the local order update mutates separate caches without scanning peers"
)]
fn reconcile_widget_paint_order_local(
    world: &World,
    parent_changes: &HashSet<Entity>,
    children_changes: &HashSet<Entity>,
    missing_entities: &[Entity],
    ranks: &mut HashMap<Entity, u64>,
    order_keys: &mut BTreeMap<u64, Entity>,
    children_snapshot: &mut HashMap<Entity, Vec<Entity>>,
    roots: &mut BTreeMap<u32, Entity>,
    parent_snapshot: &mut HashMap<Entity, Entity>,
) -> HashSet<Entity> {
    let mut changed = HashSet::new();
    let moved_roots: HashSet<_> = parent_changes
        .iter()
        .copied()
        .chain(missing_entities.iter().copied())
        .collect();
    let mut moved_roots_ordered: Vec<_> = moved_roots.iter().copied().collect();
    moved_roots_ordered.sort_by_key(|root| {
        let parent = current_widget_parent(world, *root, parent_snapshot.get(root).copied());
        let sibling_position = parent
            .and_then(|parent| children_snapshot.get(&parent))
            .and_then(|children| children.iter().position(|sibling| sibling == root))
            .unwrap_or(usize::MAX);
        (
            parent.map_or(root.index(), Entity::index),
            sibling_position,
            root.index(),
        )
    });
    // Free the full moved set before assigning any one rank. This makes a
    // multi-sibling permutation allocate into the final shared interval instead
    // of treating another soon-to-move sibling's stale rank as an anchor.
    for root in &moved_roots_ordered {
        for entity in widget_subtree_paint_order(world, *root) {
            if let Some(rank) = ranks.remove(&entity) {
                order_keys.remove(&rank);
            }
        }
    }
    for root in &moved_roots_ordered {
        let old_parent = parent_snapshot.get(root).copied();
        let new_parent = current_widget_parent(world, *root, old_parent);
        if old_parent != new_parent {
            if let Some(parent) = old_parent {
                update_parent_child_snapshot(world, parent, *root, false, children_snapshot);
            }
            if let Some(parent) = new_parent {
                update_parent_child_snapshot(world, parent, *root, true, children_snapshot);
                parent_snapshot.insert(*root, parent);
            } else {
                parent_snapshot.remove(root);
            }
        }
        changed.extend(move_widget_paint_subtree(
            world,
            *root,
            new_parent,
            ranks,
            order_keys,
            children_snapshot,
            roots,
        ));
    }

    let mut ordered_parents: Vec<_> = children_changes.iter().copied().collect();
    ordered_parents.sort_unstable_by_key(|parent| parent.index());
    for parent in ordered_parents {
        let old = children_snapshot.get(&parent).cloned().unwrap_or_default();
        let current = current_widget_children(world, parent, &old, parent_snapshot);
        let mut reordered_roots: Vec<_> =
            changed_sibling_roots(&old, &current).into_iter().collect();
        reordered_roots.sort_by_key(|root| {
            current
                .iter()
                .position(|candidate| candidate == root)
                .or_else(|| old.iter().position(|candidate| candidate == root))
                .unwrap_or(usize::MAX)
        });
        for root in reordered_roots
            .iter()
            .filter(|root| !moved_roots.contains(root))
        {
            for entity in widget_subtree_paint_order(world, *root) {
                if let Some(rank) = ranks.remove(&entity) {
                    order_keys.remove(&rank);
                }
            }
        }
        for root in reordered_roots {
            if !moved_roots.contains(&root) {
                changed.extend(move_widget_paint_subtree(
                    world,
                    root,
                    current_widget_parent(world, root, parent_snapshot.get(&root).copied()),
                    ranks,
                    order_keys,
                    children_snapshot,
                    roots,
                ));
            }
        }
        if current.is_empty() {
            children_snapshot.remove(&parent);
        } else {
            children_snapshot.insert(parent, current);
        }
    }
    changed
}

fn assign_even_paint_ranks(order: &[Entity]) -> HashMap<Entity, u64> {
    let step = u64::MAX / (order.len() as u64 + 1);
    order
        .iter()
        .enumerate()
        .map(|(index, entity)| {
            let rank = step.saturating_mul(index as u64 + 1);
            (*entity, rank)
        })
        .collect()
}

/// Reconcile hierarchy order while retaining ranks for the longest sequence
/// that kept its relative order. Reparent/reorder changes therefore republish
/// only moved fragments plus local neighbors if their rank gap is exhausted.
fn collect_widget_paint_full(
    world: &mut World,
    input_stamp: Option<u64>,
    batch: Option<UiDirtyBatch>,
) {
    let base_theme = world.resource::<Theme>().clone();
    let (
        mut fragments,
        mut paint_ranks,
        mut paint_order_keys,
        mut paint_children,
        mut paint_roots,
        mut paint_parents,
        parent_changes,
        children_changes,
    ) = {
        let mut cache = world.resource_mut::<WidgetPaintCache>();
        (
            std::mem::take(&mut cache.fragments),
            std::mem::take(&mut cache.paint_ranks),
            std::mem::take(&mut cache.paint_order_keys),
            std::mem::take(&mut cache.paint_children),
            std::mem::take(&mut cache.paint_roots),
            std::mem::take(&mut cache.paint_parents),
            std::mem::take(&mut cache.pending_parent_changes),
            std::mem::take(&mut cache.pending_children_changes),
        )
    };
    let repaint_all = batch.as_ref().is_none_or(|batch| batch.all) || paint_ranks.is_empty();
    let mut entities: Vec<_> = if repaint_all {
        let mut query = world.query_filtered::<Entity, With<UiLayoutResult>>();
        query.iter(world).collect()
    } else {
        batch
            .as_ref()
            .map(|batch| batch.entities.iter().copied().collect())
            .unwrap_or_default()
    };
    let mut removed_fragments = HashSet::new();
    if repaint_all {
        removed_fragments = std::mem::take(&mut fragments);
        let order = widget_paint_order(world, &entities);
        paint_ranks = assign_even_paint_ranks(&order);
        paint_order_keys = paint_ranks
            .iter()
            .map(|(entity, rank)| (*rank, *entity))
            .collect();
        paint_children = widget_children_snapshot(world);
        paint_roots = widget_root_order(world);
        paint_parents = widget_parent_snapshot(world);
    } else {
        let missing_entities: Vec<_> = batch
            .as_ref()
            .into_iter()
            .flat_map(|batch| batch.entities.iter().copied())
            .filter(|entity| {
                world.get::<UiLayoutResult>(*entity).is_some() && !paint_ranks.contains_key(entity)
            })
            .collect();
        if !parent_changes.is_empty()
            || !children_changes.is_empty()
            || !missing_entities.is_empty()
        {
            let changed_order = reconcile_widget_paint_order_local(
                world,
                &parent_changes,
                &children_changes,
                &missing_entities,
                &mut paint_ranks,
                &mut paint_order_keys,
                &mut paint_children,
                &mut paint_roots,
                &mut paint_parents,
            );
            let mut included: HashSet<_> = entities.iter().copied().collect();
            for entity in changed_order {
                if fragments.contains(&entity) && included.insert(entity) {
                    entities.push(entity);
                }
            }
        }
    }
    let mut changed_outputs = HashMap::new();
    let mut repainted_entities = 0;

    for &entity in &entities {
        if fragments.remove(&entity) {
            removed_fragments.insert(entity);
        }
        if world.get::<UiLayoutResult>(entity).is_none() {
            if let Some(rank) = paint_ranks.remove(&entity) {
                paint_order_keys.remove(&rank);
            }
            continue;
        }
        if !is_visible(world, entity) {
            continue;
        }
        let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
            continue;
        };
        let is_box = world
            .get::<UiRole>(entity)
            .is_some_and(|role| role.0 == "box");
        if !is_box
            && world.get::<Checkbox>(entity).is_none()
            && world.get::<Button>(entity).is_none()
            && world.get::<IconButton>(entity).is_none()
            && world.get::<Label>(entity).is_none()
            && world.get::<RadioGroup>(entity).is_none()
            && world.get::<NumericField>(entity).is_none()
            && world.get::<Slider>(entity).is_none()
            && world.get::<TextField>(entity).is_none()
            && world.get::<TextArea>(entity).is_none()
            && world.get::<Dropdown>(entity).is_none()
            && world.get::<Panel>(entity).is_none()
            && world.get::<Progress>(entity).is_none()
            && world.get::<MenuControl>(entity).is_none()
            && world.get::<MenuItemControl>(entity).is_none()
            && world.get::<DropdownOptionControl>(entity).is_none()
            && world.get::<FileBrowser>(entity).is_none()
            && world.get::<FileBrowserItem>(entity).is_none()
            && world.get::<ListModel<String>>(entity).is_none()
            && world.get::<TreeModel<String>>(entity).is_none()
            && world.get::<PreviewGrid>(entity).is_none()
            && world.get::<CollectionItemControl>(entity).is_none()
            && world.get::<DataTable>(entity).is_none()
            && world.get::<DataTableHeaderControl>(entity).is_none()
            && world.get::<DataTableCellControl>(entity).is_none()
            && world.get::<ColorRamp>(entity).is_none()
            && world.get::<HsvColorEditor>(entity).is_none()
            && world.get::<CurveEditor>(entity).is_none()
            && world.get::<NodeGraph>(entity).is_none()
            && world.get::<Canvas>(entity).is_none()
            && world.get::<Tooltip>(entity).is_none()
        {
            continue;
        }
        let clip_rect = match layout.clip {
            Some(parent_clip) => {
                let Some(intersection) = parent_clip.intersection(layout.rect) else {
                    continue;
                };
                intersection
            }
            None => layout.rect,
        };
        let clip = ClipRect::from(clip_rect);
        let rect = layout.rect;
        let z = layout.z;
        let entity_theme = styled_theme(world, entity, &base_theme);
        let theme = &entity_theme;
        let camera = world
            .get::<bevy_camera::Camera>(layout.window)
            .map(|_| layout.window);
        let order = paint_ranks.get(&entity).copied().unwrap_or_default();
        let mut fragment_guard = PaintFragmentGuard {
            fragment_entities: &mut fragments,
            changed_outputs: &mut changed_outputs,
            removed_fragments: &mut removed_fragments,
            entity,
            fragment: Some(WidgetRenderFragment {
                entity,
                order,
                target_camera: camera,
                items: Vec::new(),
            }),
        };
        let camera_items = &mut fragment_guard
            .fragment
            .as_mut()
            .expect("paint fragment is initialized")
            .items;
        repainted_entities += 1;
        let enabled = is_interactable(world, entity);

        if let Some(tooltip) = world.get::<Tooltip>(entity)
            && tooltip.visible
        {
            paint_tooltip_overlay(world, entity, tooltip, layout.window, theme, camera_items);
        }

        if is_box {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            let border_width = world
                .get::<UiComputedStyle>(entity)
                .and_then(|style| style.scalar(UiStyleProperty::BorderWidth))
                .filter(|width| width.is_finite())
                .unwrap_or(1.0)
                .clamp(0.0, 8.0);
            if border_width > 0.0 {
                push_rect_outline_with_width(
                    rect,
                    theme.border,
                    clip,
                    z.saturating_add(1),
                    border_width,
                    camera_items,
                );
            }
            continue;
        }

        if let Some(mut checkbox) = world.get::<Checkbox>(entity).cloned() {
            checkbox.enabled &= enabled;
            camera_items.extend(checkbox.draw_items_clipped(rect, layout.clip, theme, z));
            continue;
        }
        if let Some(group) = world.get::<RadioGroup>(entity) {
            paint_radio_group(
                RadioGroupPaint {
                    group,
                    label: world
                        .get::<Label>(entity)
                        .map_or("Options", |label| label.text.as_str()),
                    rect,
                    clip,
                    z,
                    enabled,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(ramp) = world.get::<ColorRamp>(entity) {
            paint_color_ramp(
                ColorRampPaint {
                    ramp,
                    state: world
                        .get::<ColorRampEditorState>(entity)
                        .copied()
                        .unwrap_or_default(),
                    label: world
                        .get::<Label>(entity)
                        .map_or("Color ramp", |label| label.text.as_str()),
                    rect,
                    clip,
                    z,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(editor) = world.get::<HsvColorEditor>(entity).copied() {
            paint_hsv_color_editor(
                HsvColorPaint {
                    editor,
                    state: world
                        .get::<HsvColorEditorState>(entity)
                        .cloned()
                        .unwrap_or_default(),
                    label: world
                        .get::<Label>(entity)
                        .map_or("Color", |label| label.text.as_str()),
                    rect,
                    clip,
                    z,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(editor) = world.get::<CurveEditor>(entity) {
            paint_curve_editor(
                CurveEditorPaint {
                    editor,
                    label: world
                        .get::<Label>(entity)
                        .map_or("Curve", |label| label.text.as_str()),
                    rect,
                    clip,
                    z,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(graph) = world.get::<NodeGraph>(entity).cloned() {
            let font = resolve_text_font(world, entity, ui_default_text_font(theme.font_size));
            let output_label_widths =
                measure_node_graph_output_label_widths(world, &graph, &font, theme);
            let canvas_surface = node_graph_canvas_surface(world, entity, &base_theme);
            paint_node_graph(
                NodeGraphPaint {
                    graph: &graph,
                    state: world
                        .get::<NodeGraphEditorState>(entity)
                        .cloned()
                        .unwrap_or_default(),
                    output_label_widths,
                    canvas_surface,
                    label: world
                        .get::<Label>(entity)
                        .map_or("Node graph", |label| label.text.as_str()),
                    rect,
                    clip,
                    z,
                    theme,
                    font,
                },
                camera_items,
            );
            continue;
        }
        if let Some(canvas) = world.get::<Canvas>(entity) {
            crate::canvas::paint_canvas(
                canvas,
                rect,
                clip,
                z,
                theme,
                world.get_resource::<crate::canvas::CanvasImageAssets>(),
                camera_items,
            );
            continue;
        }
        if let Some(header) = world.get::<DataTableHeaderControl>(entity).cloned() {
            paint_data_table_header(&header, rect, clip, z, theme, camera_items);
            continue;
        }
        if let Some(cell) = world.get::<DataTableCellControl>(entity).cloned() {
            paint_data_table_cell(&cell, rect, clip, z, theme, camera_items);
            continue;
        }
        if let Some(table) = world.get::<DataTable>(entity) {
            let label = world
                .get::<Label>(entity)
                .map_or("Table", |label| label.text.as_str());
            paint_data_table_root(table, label, rect, clip, z, theme, camera_items);
            continue;
        }
        if let Some(collection_item) = world.get::<CollectionItemControl>(entity).cloned() {
            let mut display_item = collection_item.clone();
            if let Some(state) = world.get::<CollectionEditorState>(collection_item.owner)
                && state.rename_id == Some(collection_item.id)
            {
                let cursor = state
                    .rename_buffer
                    .char_indices()
                    .nth(state.rename_cursor)
                    .map_or(state.rename_buffer.len(), |(index, _)| index);
                display_item.label = format!(
                    "{}│{}",
                    &state.rename_buffer[..cursor],
                    &state.rename_buffer[cursor..]
                );
            }
            let owner_theme = styled_theme(world, collection_item.owner, theme);
            paint_collection_item(
                &display_item,
                rect,
                clip,
                z,
                enabled,
                &owner_theme,
                camera_items,
            );
            continue;
        }
        if let Some(button) = world.get::<Button>(entity) {
            let background = if !enabled || !button.enabled {
                theme.color(ThemeColorToken::ButtonDisabledSurface)
            } else if button.status == crate::WidgetStatus::Hovered {
                theme.color(ThemeColorToken::ButtonHoverSurface)
            } else {
                theme.surface
            };
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: background,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.control.padding_x,
                            rect.y + theme.spacing.control.padding_y,
                        ],
                        text: button.label.clone(),
                    },
                    if !enabled || !button.enabled {
                        theme.color(ThemeColorToken::DisabledText)
                    } else {
                        theme.text
                    },
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(button) = world.get::<IconButton>(entity) {
            let enabled = enabled && button.enabled;
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: if enabled {
                        theme.surface
                    } else {
                        theme.color(ThemeColorToken::ButtonDisabledSurface)
                    },
                },
            ));
            if world.get::<IconButtonAsset>(entity).is_none() {
                let text_width = button.icon.chars().count() as f32 * theme.font_size * 0.56;
                let text_height = theme.font_size * theme.line_height;
                camera_items.push(item(
                    clip,
                    z.saturating_add(1),
                    styled_text_primitive_for_entity(
                        world,
                        entity,
                        clip,
                        TextPlacement {
                            origin: [
                                rect.x + (rect.width - text_width).max(0.0) * 0.5,
                                rect.y + (rect.height - text_height).max(0.0) * 0.5,
                            ],
                            text: button.icon.clone(),
                        },
                        if enabled {
                            theme.text
                        } else {
                            theme.color(ThemeColorToken::DisabledText)
                        },
                        theme,
                        false,
                    ),
                ));
            }
            continue;
        }
        if let Some(menu) = world.get::<MenuControl>(entity) {
            let background = if menu.session.open {
                theme.color(ThemeColorToken::MenuOpenSurface)
            } else {
                theme.surface
            };
            let animation = world
                .get_resource::<WidgetMenuAnimations>()
                .and_then(|animations| animations.0.get(&entity))
                .copied();
            let progress = menu_animation_progress(world, entity, theme)
                .unwrap_or(if menu.session.open { 1.0 } else { 0.0 });
            let visual_popup = animation.map_or(menu.popup.bounds, |animation| {
                interpolate_rect(animation.anchor, animation.popup, progress)
            });
            if menu.session.open || progress > f32::EPSILON {
                let popup = visual_popup;
                let popup_clip_rect = layout
                    .clip
                    .and_then(|parent| popup.intersection(parent))
                    .unwrap_or(popup);
                let popup_clip = ClipRect::from(popup_clip_rect);
                let popup_z = z.saturating_add(90);
                let shadow_clip = popup_viewport_clip(world, layout.window, layout.clip, popup);
                if menu.display == MenuDisplay::Pie {
                    push_popup_shadow(
                        popup,
                        popup.width.min(popup.height) * 0.5,
                        Rgba(0.0, 0.0, 0.0, 0.0),
                        shadow_clip,
                        popup_z.saturating_sub(1),
                        theme,
                        camera_items,
                    );
                    camera_items.push(item(
                        popup_clip,
                        popup_z,
                        Primitive::Ellipse {
                            rect: ClipRect::from(popup),
                            color: theme.surface,
                        },
                    ));
                    let inset = theme
                        .spacing
                        .menu
                        .pie_inner_inset
                        .min(popup.width.min(popup.height) * 0.25);
                    camera_items.push(item(
                        popup_clip,
                        popup_z.saturating_add(1),
                        Primitive::Ellipse {
                            rect: ClipRect::from(Rect {
                                x: popup.x + inset,
                                y: popup.y + inset,
                                width: (popup.width - inset * 2.0).max(0.0),
                                height: (popup.height - inset * 2.0).max(0.0),
                            }),
                            color: theme.background,
                        },
                    ));
                } else {
                    push_popup_shadow(
                        popup,
                        theme.corner_radius,
                        theme.surface,
                        shadow_clip,
                        popup_z.saturating_sub(1),
                        theme,
                        camera_items,
                    );
                    camera_items.push(item(
                        popup_clip,
                        popup_z,
                        Primitive::RoundedRect {
                            rect: ClipRect::from(popup),
                            radius: theme.corner_radius,
                            color: theme.surface,
                        },
                    ));
                    let border = theme.color(ThemeColorToken::MenuBorder);
                    let right = popup.x + popup.width;
                    let bottom = popup.y + popup.height;
                    for (from, to) in [
                        ([popup.x, popup.y], [right, popup.y]),
                        ([right, popup.y], [right, bottom]),
                        ([right, bottom], [popup.x, bottom]),
                        ([popup.x, bottom], [popup.x, popup.y]),
                    ] {
                        camera_items.push(item(
                            popup_clip,
                            popup_z.saturating_add(1),
                            Primitive::Line {
                                from,
                                to,
                                width: 1.0,
                                color: border,
                            },
                        ));
                    }
                }
            }
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: background,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.menu.padding_x,
                            rect.y + theme.spacing.menu.padding_y,
                        ],
                        text: format!(
                            "{} {}",
                            menu.label,
                            if menu.session.open { "▴" } else { "▾" }
                        ),
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(menu_item) = world.get::<MenuItemControl>(entity) {
            let Some(menu) = world.get::<MenuControl>(menu_item.owner) else {
                continue;
            };
            let opacity = menu_animation_progress(world, menu_item.owner, theme).unwrap_or(1.0);
            let clip = if let Some(animation) = world
                .get_resource::<WidgetMenuAnimations>()
                .and_then(|animations| animations.0.get(&menu_item.owner))
            {
                let popup = interpolate_rect(animation.anchor, animation.popup, opacity);
                let Some(animated_clip) = intersect_clip_rect(clip, popup) else {
                    continue;
                };
                animated_clip
            } else {
                clip
            };
            let mut selected_path = menu_item.parent_path.clone();
            selected_path.push(menu_item.index);
            let selected = menu.session.path == selected_path;
            let color = if !enabled || !menu_item.enabled {
                theme.color(ThemeColorToken::MenuDisabledSurface)
            } else if selected {
                theme.color(ThemeColorToken::MenuSelectedSurface)
            } else {
                theme.surface
            };
            if !menu_item.separator {
                camera_items.push(item(
                    clip,
                    z,
                    Primitive::RoundedRect {
                        rect: ClipRect::from(rect),
                        radius: theme.corner_radius,
                        color: with_opacity(color, opacity),
                    },
                ));
                let suffix = if menu_item.has_submenu {
                    "  ›".to_owned()
                } else {
                    menu_item
                        .shortcut
                        .as_ref()
                        .map_or_else(String::new, |shortcut| format!("  {shortcut}"))
                };
                camera_items.push(item(
                    clip,
                    z.saturating_add(1),
                    styled_text_primitive_for_entity(
                        world,
                        entity,
                        clip,
                        TextPlacement {
                            origin: [
                                rect.x + theme.spacing.menu.padding_x,
                                rect.y + theme.spacing.menu.padding_y,
                            ],
                            text: format!("{}{suffix}", menu_item.label),
                        },
                        with_opacity(
                            if !enabled || !menu_item.enabled {
                                theme.color(ThemeColorToken::MenuDisabledText)
                            } else {
                                theme.text
                            },
                            opacity,
                        ),
                        theme,
                        false,
                    ),
                ));
            }
            continue;
        }
        if let Some(file_item) = world.get::<FileBrowserItem>(entity) {
            let file_view = world
                .get::<FileBrowser>(file_item.owner)
                .map_or(FileView::List, |browser| browser.view);
            let background = if file_item.selected {
                theme.color(ThemeColorToken::FileBrowserSelectedSurface)
            } else {
                theme.surface
            };
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: background,
                },
            ));
            let glyph = file_preview_glyph(file_item.preview);
            let text = if file_view == FileView::List {
                let size = if file_item.directory {
                    String::new()
                } else {
                    format!("  {}", format_file_size(file_item.size))
                };
                format!("{glyph}  {}{size}", file_item.name)
            } else {
                format!("{glyph}  {}", file_item.name)
            };
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.file_browser.padding_x,
                            rect.y
                                + (rect.height - theme.font_size * theme.line_height).max(0.0)
                                    * 0.5,
                        ],
                        text,
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(browser) = world.get::<FileBrowser>(entity) {
            let header_height = theme.spacing.file_browser.header_height.min(rect.height);
            let header_row_height = header_height * 0.5;
            let header_text_offset =
                (header_row_height - theme.font_size * theme.line_height).max(0.0) * 0.5;
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.background,
                },
            ));
            let breadcrumbs = browser
                .breadcrumbs()
                .into_iter()
                .map(|crumb| crumb.name)
                .collect::<Vec<_>>()
                .join("  ›  ");
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.file_browser.padding_x,
                            rect.y + header_text_offset,
                        ],
                        text: breadcrumbs,
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            let view = if browser.view == FileView::List {
                "List"
            } else {
                "Grid"
            };
            let sort = match browser.sort {
                FileSort::Name => "Name",
                FileSort::Kind => "Kind",
                FileSort::Size => "Size",
                FileSort::Modified => "Modified",
            };
            let bookmark_names = browser
                .bookmarks
                .iter()
                .filter_map(|path| path.file_name().map(|name| name.to_string_lossy()))
                .map(|name| name.into_owned())
                .collect::<Vec<_>>()
                .join("  ");
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.file_browser.padding_x,
                            rect.y + header_row_height + header_text_offset,
                        ],
                        text: format!(
                            "{view}  •  Sort: {sort} {}  •  Filter: {}  •  ★ {bookmark_names}",
                            if browser.ascending { "↑" } else { "↓" },
                            if browser.filter.is_empty() {
                                "type to search"
                            } else {
                                &browser.filter
                            }
                        ),
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(label) = world.get::<Label>(entity)
            && (world.get::<ListModel<String>>(entity).is_some()
                || world.get::<TreeModel<String>>(entity).is_some()
                || world.get::<PreviewGrid>(entity).is_some())
        {
            let count = world
                .get::<ListModel<String>>(entity)
                .map(|model| model.items.len())
                .or_else(|| {
                    world
                        .get::<TreeModel<String>>(entity)
                        .map(|model| model.nodes.len())
                })
                .or_else(|| {
                    world
                        .get::<PreviewGrid>(entity)
                        .map(|model| model.items.len())
                })
                .unwrap_or(0);
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.background,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: theme.spacing.collection.header_height.min(rect.height),
                    },
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(2),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.collection.padding,
                            rect.y + theme.spacing.editor.padding_y,
                        ],
                        text: format!("{}  ·  {count}", label.text),
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(2),
                Primitive::Line {
                    from: [rect.x, rect.y + theme.spacing.collection.header_height],
                    to: [
                        rect.x + rect.width,
                        rect.y + theme.spacing.collection.header_height,
                    ],
                    width: 1.0,
                    color: theme.border,
                },
            ));
            continue;
        }
        if let Some(label) = world.get::<Label>(entity) {
            let composite = world.get::<NumericField>(entity).is_some()
                || world.get::<Slider>(entity).is_some()
                || world.get::<TextField>(entity).is_some()
                || world.get::<TextArea>(entity).is_some()
                || world.get::<Dropdown>(entity).is_some();
            if !composite {
                camera_items.push(item(
                    clip,
                    z,
                    styled_text_primitive_for_entity(
                        world,
                        entity,
                        clip,
                        TextPlacement {
                            origin: [rect.x, rect.y],
                            text: label.text.clone(),
                        },
                        theme.text,
                        theme,
                        false,
                    ),
                ));
                continue;
            }
        }
        if let Some(slider) = world.get::<Slider>(entity) {
            let label = world
                .get::<Label>(entity)
                .map_or_else(String::new, |label| label.text.clone());
            let focused = world
                .get_resource::<UiFocus>()
                .and_then(|focus| focus.state(layout.window))
                .is_some_and(|state| state.focused == Some(entity));
            let dragging = world
                .get_resource::<WidgetInteractionState>()
                .and_then(|state| state.slider_drags.get(&layout.window))
                .is_some_and(|drag| drag.target == entity);
            paint_slider(
                SliderPaint {
                    slider,
                    label: &label,
                    rect,
                    clip,
                    z,
                    enabled: enabled && slider.enabled,
                    focused,
                    dragging,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(field) = world.get::<NumericField>(entity) {
            let scrub_selected =
                world
                    .get_resource::<WidgetInteractionState>()
                    .is_some_and(|state| {
                        state
                            .numeric_scrubs
                            .values()
                            .any(|scrub| scrub.targets.contains(&entity))
                    });
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: if scrub_selected {
                        theme.color(ThemeColorToken::EditorActiveFieldSurface)
                    } else {
                        theme.surface
                    },
                },
            ));
            let label = world
                .get::<Label>(entity)
                .map_or_else(String::new, |label| label.text.clone());
            let content = if label.is_empty() {
                field.display()
            } else {
                format!("{label}: {}", field.display())
            };
            let content = if let Some(error) = &field.expression_error {
                format!("{content}  ({error})")
            } else {
                content
            };
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.control.padding_x,
                            rect.y + theme.spacing.control.padding_y,
                        ],
                        text: content,
                    },
                    if field.expression_error.is_some() {
                        theme.error
                    } else {
                        theme.text
                    },
                    theme,
                    false,
                ),
            ));
            if field.expression_error.is_some() {
                let width = theme.spacing.control.error_outline_width;
                if width.is_finite() && width > 0.0 {
                    for (from, to) in [
                        ([rect.x, rect.y], [rect.x + rect.width, rect.y]),
                        (
                            [rect.x + rect.width, rect.y],
                            [rect.x + rect.width, rect.y + rect.height],
                        ),
                        (
                            [rect.x + rect.width, rect.y + rect.height],
                            [rect.x, rect.y + rect.height],
                        ),
                        ([rect.x, rect.y + rect.height], [rect.x, rect.y]),
                    ] {
                        camera_items.push(item(
                            clip,
                            z.saturating_add(2),
                            Primitive::Line {
                                from,
                                to,
                                width,
                                color: theme.error,
                            },
                        ));
                    }
                }
            }
            continue;
        }
        if let Some(field) = world.get::<TextField>(entity) {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            let live_text = world
                .get::<TextBuffer>(entity)
                .map_or(field.text.as_str(), TextBuffer::as_str);
            let content = if live_text.is_empty() {
                field.placeholder.clone()
            } else {
                live_text.to_owned()
            };
            let label = world
                .get::<Label>(entity)
                .map_or_else(String::new, |label| label.text.clone());
            let fallback = if label.is_empty() {
                content
            } else {
                format!("{label}: {content}")
            };
            paint_text_editor(
                world,
                entity,
                TextEditorPaint {
                    clip,
                    z: z.saturating_add(1),
                    widget_rect: rect,
                    fallback_text: fallback,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(area) = world.get::<TextArea>(entity) {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            let live_text = world
                .get::<TextBuffer>(entity)
                .map_or(area.field.text.as_str(), TextBuffer::as_str);
            let label = world
                .get::<Label>(entity)
                .map_or_else(String::new, |label| label.text.clone());
            let fallback = if label.is_empty() {
                live_text.to_owned()
            } else {
                format!("{label}: {live_text}")
            };
            paint_text_editor(
                world,
                entity,
                TextEditorPaint {
                    clip,
                    z: z.saturating_add(1),
                    widget_rect: rect,
                    fallback_text: fallback,
                    theme,
                },
                camera_items,
            );
            continue;
        }
        if let Some(option) = world.get::<DropdownOptionControl>(entity) {
            let color = if !option.enabled {
                theme.color(ThemeColorToken::MenuDisabledSurface)
            } else if option.selected || option.highlighted {
                theme.color(ThemeColorToken::MenuSelectedSurface)
            } else {
                theme.surface
            };
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: 2.0,
                    color,
                },
            ));
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.menu.padding_x,
                            rect.y + theme.spacing.menu.padding_y,
                        ],
                        text: option.label.clone(),
                    },
                    if option.enabled {
                        theme.text
                    } else {
                        theme.color(ThemeColorToken::MenuDisabledText)
                    },
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(dropdown) = world.get::<Dropdown>(entity) {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            let label = world
                .get::<Label>(entity)
                .map_or_else(String::new, |label| label.text.clone());
            let choice = dropdown
                .selected
                .and_then(|index| dropdown.options.get(index))
                .cloned()
                .unwrap_or_default();
            if dropdown.open
                && let Some(popup_state) = world.get::<DropdownPopupState>(entity)
                && !popup_state.popup.bounds.is_empty()
            {
                let popup = popup_state.popup.bounds;
                let popup_clip = ClipRect::from(popup_state.viewport);
                let popup_z = z.saturating_add(80);
                let shadow_clip =
                    popup_viewport_clip(world, layout.window, layout.clip, popup_state.viewport);
                push_popup_shadow(
                    popup,
                    theme.corner_radius,
                    theme.surface,
                    shadow_clip,
                    popup_z.saturating_sub(1),
                    theme,
                    camera_items,
                );
                camera_items.push(item(
                    popup_clip,
                    popup_z,
                    Primitive::RoundedRect {
                        rect: ClipRect::from(popup),
                        radius: theme.corner_radius,
                        color: theme.surface,
                    },
                ));
                let right = popup.x + popup.width;
                let bottom = popup.y + popup.height;
                for (from, to) in [
                    ([popup.x, popup.y], [right, popup.y]),
                    ([right, popup.y], [right, bottom]),
                    ([right, bottom], [popup.x, bottom]),
                    ([popup.x, bottom], [popup.x, popup.y]),
                ] {
                    camera_items.push(item(
                        popup_clip,
                        popup_z.saturating_add(3),
                        Primitive::Line {
                            from,
                            to,
                            width: 1.0,
                            color: theme.color(ThemeColorToken::MenuBorder),
                        },
                    ));
                }
            }
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.control.padding_x,
                            rect.y + theme.spacing.control.padding_y,
                        ],
                        text: if label.is_empty() {
                            choice
                        } else {
                            format!("{label}: {choice} ▾")
                        },
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(panel) = world.get::<Panel>(entity) {
            let header_height = rect.height.min(theme.spacing.panel.header_height);
            let panel_rect = Rect {
                height: header_height
                    + (rect.height - header_height).max(0.0) * panel.state.progress,
                ..rect
            };
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(panel_rect),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            let arrow = if panel.state.target_expanded {
                "▾"
            } else {
                "▸"
            };
            camera_items.push(item(
                clip,
                z.saturating_add(1),
                styled_text_primitive_for_entity(
                    world,
                    entity,
                    clip,
                    TextPlacement {
                        origin: [
                            rect.x + theme.spacing.panel.content_padding,
                            rect.y + theme.spacing.panel.content_padding,
                        ],
                        text: format!("{arrow} {}", panel.title),
                    },
                    theme.text,
                    theme,
                    false,
                ),
            ));
            continue;
        }
        if let Some(progress) = world.get::<Progress>(entity) {
            camera_items.push(item(
                clip,
                z,
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect),
                    radius: theme.corner_radius,
                    color: theme.surface,
                },
            ));
            if progress.value > 0.0 {
                camera_items.push(item(
                    clip,
                    z.saturating_add(1),
                    Primitive::RoundedRect {
                        rect: ClipRect {
                            width: rect.width * progress.value,
                            ..ClipRect::from(rect)
                        },
                        radius: theme.corner_radius,
                        color: theme.accent,
                    },
                ));
            }
        }
    }

    let mut display_list = world.resource_mut::<DisplayList>();
    let mut published_fragments = 0;
    for (_, fragment) in changed_outputs {
        published_fragments += usize::from(display_list.upsert_widget_fragment(fragment));
    }
    for entity in removed_fragments {
        published_fragments += usize::from(display_list.remove_widget_fragment(entity));
    }
    let mut cache = world.resource_mut::<WidgetPaintCache>();
    cache.fragments = fragments;
    cache.paint_ranks = paint_ranks;
    cache.paint_order_keys = paint_order_keys;
    cache.paint_children = paint_children;
    cache.paint_roots = paint_roots;
    cache.paint_parents = paint_parents;
    cache.input_stamp = input_stamp;
    cache.rebuilt_frames = cache.rebuilt_frames.saturating_add(1);
    cache.repainted_entities = repainted_entities;
    cache.published_fragments = published_fragments;
}

fn widget_paint_input_stamp(world: &World) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_resource_ticks::<Theme>(world, &mut hasher);
    hash_resource_ticks::<WidgetInteractionState>(world, &mut hasher);
    hash_resource_ticks::<WidgetMenuAnimations>(world, &mut hasher);
    hash_resource_ticks::<crate::canvas::CanvasImageAssets>(world, &mut hasher);

    if let Some(focus) = world.get_resource::<UiFocus>() {
        focus.active_window.hash(&mut hasher);
        for (window, state) in &focus.windows {
            window.hash(&mut hasher);
            state.focused.hash(&mut hasher);
            state.hovered.hash(&mut hasher);
            state.captured.hash(&mut hasher);
        }
    }
    if let Some(tooltips) = world.get_resource::<WidgetTooltips>() {
        tooltips.0.len().hash(&mut hasher);
        for (window, hover) in &tooltips.0 {
            window.hash(&mut hasher);
            hover.trigger.hash(&mut hasher);
            hover.owner.hash(&mut hasher);
        }
    }

    for entity in world.iter_entities() {
        if !entity.contains::<UiNode>()
            && !entity.contains::<UiParent>()
            && !entity.contains::<bui_core::UiChildren>()
            && !entity.contains::<UiLayoutResult>()
            && !entity.contains::<UiLayoutRoot>()
        {
            continue;
        }
        entity.id().hash(&mut hasher);
        entity.archetype().components().len().hash(&mut hasher);
        for component in entity.archetype().components() {
            component.index().hash(&mut hasher);
            if let Some(ticks) = entity.get_change_ticks_by_id(*component) {
                ticks.added.get().hash(&mut hasher);
                ticks.changed.get().hash(&mut hasher);
            }
        }
        if let Some(layout) = entity.get::<UiLayoutResult>() {
            let has_camera = world
                .get_entity(layout.window)
                .is_ok_and(|window| window.contains::<bevy_camera::Camera>());
            layout.window.hash(&mut hasher);
            has_camera.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn hash_resource_ticks<R: Resource>(world: &World, hasher: &mut impl Hasher) {
    if let Some(ticks) = world.get_resource_change_ticks::<R>() {
        true.hash(hasher);
        ticks.added.get().hash(hasher);
        ticks.changed.get().hash(hasher);
    } else {
        false.hash(hasher);
    }
}

fn paint_tooltip_overlay(
    world: &World,
    owner: Entity,
    tooltip: &Tooltip,
    window: Entity,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let Some(hover) = world
        .get_resource::<WidgetTooltips>()
        .and_then(|tooltips| tooltips.0.get(&window))
        .filter(|hover| hover.owner == owner)
    else {
        return;
    };
    let Some(anchor) = world.get::<UiLayoutResult>(hover.trigger).copied() else {
        return;
    };
    let viewport = world
        .iter_entities()
        .find_map(|entity| {
            entity
                .get::<UiLayoutRoot>()
                .filter(|root| root.window == window)
                .map(|root| root.viewport)
        })
        .unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        });
    let spacing = theme.spacing.tooltip;
    let padding_x = spacing.padding_x.max(4.0);
    let padding_y = spacing.padding_y.max(3.0);
    let max_content_width = spacing
        .maximum_width
        .min((viewport.width - padding_x * 2.0).max(1.0));
    let estimated_text_width = tooltip.text.chars().count() as f32 * theme.font_size * 0.56;
    let content_width = estimated_text_width.clamp(1.0, max_content_width);
    let line_height = theme.font_size * theme.line_height;
    let line_count = (estimated_text_width / max_content_width).ceil().max(1.0);
    let height = line_height * line_count + padding_y * 2.0;
    let width = content_width + padding_x * 2.0;
    let margin = spacing
        .viewport_margin
        .min(viewport.width * 0.5)
        .min(viewport.height * 0.5);
    let below = anchor.rect.y + anchor.rect.height + spacing.anchor_gap;
    let y = if below + height <= viewport.y + viewport.height - margin {
        below
    } else {
        anchor.rect.y - height - spacing.anchor_gap
    }
    .clamp(
        viewport.y + margin,
        (viewport.y + viewport.height - height - margin).max(viewport.y + margin),
    );
    let rect = Rect {
        x: anchor.rect.x.clamp(
            viewport.x + margin,
            (viewport.x + viewport.width - width - margin).max(viewport.x + margin),
        ),
        y,
        width,
        height: height.min(viewport.height - margin * 2.0),
    };
    let clip = ClipRect::from(viewport);
    let z = anchor
        .z
        .max(
            world
                .get::<UiLayoutResult>(owner)
                .map_or(0, |layout| layout.z),
        )
        .saturating_add(200);
    let shadow_clip = popup_viewport_clip(
        world,
        window,
        world
            .get::<UiLayoutResult>(owner)
            .and_then(|layout| layout.clip),
        viewport,
    );
    push_popup_shadow(
        rect,
        theme.corner_radius,
        theme.surface,
        shadow_clip,
        z.saturating_sub(1),
        theme,
        output,
    );
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    push_rect_outline(rect, theme.border, clip, z.saturating_add(1), output);
    let content_clip = ClipRect::from(Rect {
        x: rect.x + padding_x,
        y: rect.y + padding_y,
        width: (rect.width - padding_x * 2.0).max(1.0),
        height: (rect.height - padding_y * 2.0).max(1.0),
    });
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive_for_entity(
            world,
            owner,
            content_clip,
            TextPlacement {
                origin: [rect.x + padding_x, rect.y + padding_y],
                text: tooltip.text.clone(),
            },
            theme.text,
            theme,
            true,
        ),
    ));
}

struct TextEditorPaint<'a> {
    clip: ClipRect,
    z: i32,
    widget_rect: Rect,
    fallback_text: String,
    theme: &'a Theme,
}

fn paint_text_editor(
    world: &World,
    target: Entity,
    paint: TextEditorPaint<'_>,
    output: &mut Vec<DrawItem>,
) {
    let TextEditorPaint {
        clip,
        z,
        widget_rect,
        fallback_text,
        theme,
    } = paint;
    let Some(text_layout) = world.get::<UiTextLayout>(target) else {
        let origin = [
            widget_rect.x + theme.spacing.editor.padding_x,
            widget_rect.y + theme.spacing.editor.padding_y,
        ];
        let wrap = world.get::<TextArea>(target).is_some_and(|area| area.wrap);
        output.push(item(
            clip,
            z,
            styled_text_primitive_for_entity(
                world,
                target,
                clip,
                TextPlacement {
                    origin,
                    text: fallback_text,
                },
                theme.text,
                theme,
                wrap,
            ),
        ));
        return;
    };
    let text = text_layout
        .display_text()
        .map_or(fallback_text, str::to_owned);
    let origin = text_layout.text_origin().unwrap_or([
        widget_rect.x + theme.spacing.editor.padding_x,
        widget_rect.y + theme.spacing.editor.padding_y,
    ]);
    let horizontal_padding = world
        .get::<UiLayoutNode>(target)
        .map_or(0.0, |node| node.padding.left + node.padding.right);
    let max_width =
        (widget_rect.width - horizontal_padding - text_layout.inset[0].max(0.0) * 2.0).max(0.0);
    if let Some(buffer) = world.get::<TextBuffer>(target) {
        let selection_color = theme.color(ThemeColorToken::TextSelectionOverlay);
        for selection in text_layout.selection_rects(buffer) {
            let selection = Rect {
                x: selection.x,
                y: selection.y,
                width: selection.width,
                height: selection.height,
            };
            if let Some(selection) = selection.intersection(widget_rect) {
                output.push(item(
                    clip,
                    z,
                    Primitive::RoundedRect {
                        rect: ClipRect::from(selection),
                        radius: 1.0,
                        color: selection_color,
                    },
                ));
            }
        }
    }
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::TextWrapped {
            origin,
            text,
            color: theme.text,
            max_width,
            wrap: text_layout.multiline && text_layout.wrap,
            style: Some(TextRenderStyle {
                font: Some(resolve_text_font(
                    world,
                    target,
                    text_layout
                        .font
                        .clone()
                        .unwrap_or_else(|| ui_default_text_font(theme.font_size)),
                )),
                line_height: text_layout
                    .line_height
                    .or(Some(LineHeight::RelativeToFont(theme.line_height))),
                letter_spacing: text_layout.letter_spacing,
                justify: text_layout.justify,
                line_break: text_layout.line_break.unwrap_or(
                    if text_layout.multiline && text_layout.wrap {
                        LineBreak::WordBoundary
                    } else {
                        LineBreak::NoWrap
                    },
                ),
            }),
        },
    ));
    for preedit in text_layout.preedit_rects() {
        let y = preedit.y + preedit.height.max(1.0) - 1.0;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [preedit.x, y],
                to: [preedit.x + preedit.width, y],
                width: 1.0,
                color: theme.color(ThemeColorToken::EditorImeUnderline),
            },
        ));
    }
    let focused = world
        .get_resource::<UiFocus>()
        .and_then(|focus| focus.state(world.get::<UiLayoutResult>(target)?.window))
        .is_some_and(|state| state.focused == Some(target));
    if focused
        && let Some(buffer) = world.get::<TextBuffer>(target)
        && let Some(caret) = text_layout.caret_rect(buffer)
    {
        output.push(item(
            clip,
            z.saturating_add(3),
            Primitive::Line {
                from: [caret.x, caret.y],
                to: [caret.x, caret.y + caret.height],
                width: 1.0,
                color: theme.color(ThemeColorToken::EditorCaret),
            },
        ));
    }
}

fn file_preview_glyph(preview: crate::PreviewKind) -> &'static str {
    match preview {
        crate::PreviewKind::Directory => "▸",
        crate::PreviewKind::Image => "▧",
        crate::PreviewKind::Svg => "◇",
        crate::PreviewKind::AnimatedImage => "◉",
        crate::PreviewKind::Video => "▶",
        crate::PreviewKind::Text => "≡",
        crate::PreviewKind::Other => "·",
    }
}

fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn item(clip: ClipRect, z: i32, primitive: Primitive) -> DrawItem {
    DrawItem { clip, z, primitive }
}

fn popup_viewport_clip(
    world: &World,
    window: Entity,
    parent_clip: Option<Rect>,
    fallback: Rect,
) -> ClipRect {
    let viewport = world.iter_entities().find_map(|entity| {
        entity
            .get::<UiLayoutRoot>()
            .filter(|root| root.window == window)
            .map(|root| root.viewport)
    });
    let clipped = match (parent_clip, viewport) {
        (Some(parent), Some(viewport)) => parent.intersection(viewport).unwrap_or(Rect {
            x: parent.x,
            y: parent.y,
            width: 0.0,
            height: 0.0,
        }),
        (Some(parent), None) => parent,
        (None, Some(viewport)) => viewport,
        (None, None) => fallback,
    };
    ClipRect::from(clipped)
}

fn push_popup_shadow(
    rect: Rect,
    radius: f32,
    fill: Rgba,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    for shadow in &theme.effects.popup_shadow.shadows {
        let color = match shadow.color {
            bui_style::UiShadowColor::Color(color) => color,
            bui_style::UiShadowColor::Token(token) => theme.color(token),
        };
        if !radius.is_finite()
            || !shadow.x_offset.is_finite()
            || !shadow.y_offset.is_finite()
            || !shadow.spread_radius.is_finite()
            || !shadow.blur_radius.is_finite()
            || shadow.blur_radius < 0.0
        {
            continue;
        }
        output.push(item(
            clip,
            z,
            Primitive::Shadow {
                rect: ClipRect::from(rect),
                radius: radius.max(0.0),
                color: fill,
                shadow: color,
                offset: [shadow.x_offset, shadow.y_offset],
                blur: shadow.blur_radius,
                spread: shadow.spread_radius,
            },
        ));
    }
}

fn paint_data_table_root(
    table: &DataTable,
    label: &str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let title_height = theme.control_height.max(1.0).min(rect.height.max(0.0));
    let column_height = theme
        .spacing
        .table
        .header_height
        .max(1.0)
        .min((rect.height - title_height).max(0.0));
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.background,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect {
                height: title_height,
                ..ClipRect::from(rect)
            },
            radius: theme.corner_radius,
            color: theme.color(ThemeColorToken::DataTableTitleSurface),
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect {
                x: rect.x,
                y: rect.y + title_height,
                width: rect.width,
                height: column_height,
            },
            radius: 0.0,
            color: theme.color(ThemeColorToken::DataTableHeaderSurface),
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.control.padding_x,
                rect.y + theme.spacing.control.padding_y,
            ],
            format!(
                "{label}  ·  {} rows × {} columns",
                table.rows.len(),
                table.columns.len()
            ),
            theme.text,
            theme,
            false,
        ),
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::Line {
            from: [rect.x, rect.y + title_height + column_height],
            to: [rect.x + rect.width, rect.y + title_height + column_height],
            width: 1.0,
            color: theme.border,
        },
    ));
}

fn paint_data_table_header(
    header: &DataTableHeaderControl,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let background = if header.active {
        theme.color(ThemeColorToken::DataTableSelectedHeaderSurface)
    } else {
        theme.color(ThemeColorToken::DataTableHeaderSurface)
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: 0.0,
            color: background,
        },
    ));
    let sort_glyph = match header.sort_ascending {
        Some(true) => "  ↑",
        Some(false) => "  ↓",
        None => "",
    };
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.table.cell_padding_x,
                rect.y + theme.spacing.table.cell_padding_y,
            ],
            format!("{}{sort_glyph}", header.label),
            theme.text,
            theme,
            false,
        ),
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::Line {
            from: [
                rect.x + rect.width - 1.0,
                rect.y + theme.spacing.table.cell_padding_y.min(rect.height * 0.5),
            ],
            to: [
                rect.x + rect.width - 1.0,
                rect.y + rect.height - theme.spacing.table.cell_padding_y.min(rect.height * 0.5),
            ],
            width: 1.0,
            color: if header.active {
                theme.focus
            } else {
                theme.border
            },
        },
    ));
}

fn paint_data_table_cell(
    cell: &DataTableCellControl,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let background = if cell.active {
        theme.color(ThemeColorToken::DataTableActiveCellSurface)
    } else if cell.selected {
        theme.color(ThemeColorToken::DataTableSelectedCellSurface)
    } else {
        theme.surface
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: 0.0,
            color: background,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.table.cell_padding_x,
                rect.y + theme.spacing.table.cell_padding_y,
            ],
            cell.value.clone(),
            theme.text,
            theme,
            false,
        ),
    ));
    output.push(item(
        clip,
        z.saturating_add(if cell.active { 3 } else { 2 }),
        Primitive::Line {
            from: [rect.x, rect.y + rect.height - 1.0],
            to: [rect.x + rect.width, rect.y + rect.height - 1.0],
            width: 1.0,
            color: if cell.active {
                theme.focus
            } else {
                theme.border
            },
        },
    ));
    if cell.active {
        output.push(item(
            clip,
            z.saturating_add(3),
            Primitive::Line {
                from: [rect.x + 1.0, rect.y + 1.0],
                to: [rect.x + 1.0, rect.y + rect.height - 1.0],
                width: 2.0,
                color: theme.focus,
            },
        ));
    }
}

fn paint_collection_item(
    control: &CollectionItemControl,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    enabled: bool,
    theme: &Theme,
    output: &mut Vec<DrawItem>,
) {
    let enabled = enabled && control.enabled;
    let background = if !enabled {
        theme.color(ThemeColorToken::CollectionDisabledSurface)
    } else if control.selected {
        theme.color(ThemeColorToken::CollectionSelectedSurface)
    } else {
        theme.surface
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: if control.kind == CollectionItemKind::PreviewGrid {
                theme.corner_radius.max(2.0)
            } else {
                theme.corner_radius.min(3.0)
            },
            color: background,
        },
    ));

    let foreground = if enabled {
        theme.text
    } else {
        theme.color(ThemeColorToken::DisabledText)
    };
    if control.kind == CollectionItemKind::PreviewGrid {
        let spacing = theme.spacing.collection;
        let padding = spacing.preview_padding;
        let preview_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: (rect.width - padding * 2.0).max(0.0),
            height: (rect.height - spacing.preview_caption_height - padding * 2.0).max(0.0),
        };
        if let Some(asset_id) = control.asset_id {
            output.push(item(
                clip,
                z.saturating_add(1),
                Primitive::Image {
                    rect: ClipRect::from(preview_rect),
                    asset_id,
                },
            ));
        } else {
            output.push(item(
                clip,
                z.saturating_add(1),
                Primitive::RoundedRect {
                    rect: ClipRect::from(preview_rect),
                    radius: theme.corner_radius.min(4.0),
                    color: theme.color(ThemeColorToken::PreviewPlaceholder),
                },
            ));
            let swatch = Rect {
                x: preview_rect.x + spacing.padding,
                y: preview_rect.y + spacing.padding,
                width: (preview_rect.width - spacing.padding * 2.0).max(0.0),
                height: (preview_rect.height - spacing.padding * 2.0).max(0.0),
            };
            if swatch.width > 8.0 && swatch.height > 8.0 {
                output.push(item(
                    clip,
                    z.saturating_add(2),
                    Primitive::RoundedRect {
                        rect: ClipRect::from(swatch),
                        radius: theme.corner_radius.min(4.0),
                        color: Rgba(control.tint[0], control.tint[1], control.tint[2], 1.0),
                    },
                ));
            }
        }
        if control.selected {
            output.push(item(
                clip,
                z.saturating_add(3),
                Primitive::RoundedRect {
                    rect: ClipRect::from(rect.inset(1.0, 1.0, 1.0, 1.0)),
                    radius: theme.corner_radius.max(2.0),
                    color: theme.color(ThemeColorToken::CollectionSelectionOverlay),
                },
            ));
        }
        output.push(item(
            clip,
            z.saturating_add(4),
            styled_text_primitive(
                clip,
                [
                    rect.x + theme.spacing.collection.padding,
                    rect.y + rect.height - spacing.preview_caption_height
                        + theme.spacing.list.padding_y,
                ],
                control.label.clone(),
                foreground,
                theme,
                false,
            ),
        ));
        return;
    }

    let label_x = rect.x
        + theme.spacing.list.padding_x
        + if control.kind == CollectionItemKind::Tree {
            control.depth as f32 * theme.spacing.list.indent
        } else {
            0.0
        };
    let text_x = if control.kind == CollectionItemKind::Tree {
        let disclosure = if control.has_children {
            if control.expanded { "▾" } else { "▸" }
        } else {
            "·"
        };
        output.push(item(
            clip,
            z.saturating_add(1),
            styled_text_primitive(
                clip,
                [label_x, rect.y + theme.spacing.list.padding_y],
                disclosure.to_owned(),
                if control.has_children {
                    foreground
                } else {
                    theme.text_muted
                },
                theme,
                false,
            ),
        ));
        label_x + theme.spacing.list.indent
    } else {
        label_x
    };
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [text_x, rect.y + theme.spacing.list.padding_y],
            control.label.clone(),
            foreground,
            theme,
            false,
        ),
    ));
}

struct SliderPaint<'a> {
    slider: &'a Slider,
    label: &'a str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    enabled: bool,
    focused: bool,
    dragging: bool,
    theme: &'a Theme,
}

fn paint_slider(paint: SliderPaint<'_>, output: &mut Vec<DrawItem>) {
    let SliderPaint {
        slider,
        label,
        rect,
        clip,
        z,
        enabled,
        focused,
        dragging,
        theme,
    } = paint;
    let active = focused || dragging;
    let radius = theme.spacing.editor.handle_radius;
    let radius = if radius.is_finite() {
        radius.clamp(3.0, 9.0)
    } else {
        6.0
    };
    let (left, right) = slider_track_x(rect, theme);
    let center_y = rect.y + rect.height - theme.spacing.control.padding_y.max(0.0) - radius;
    let position = slider.normalized_value() as f32;
    let handle_x = left + (right - left) * position;
    let track_height = theme.spacing.control.slider_track_height.max(2.0);
    let track_rect = ClipRect {
        x: left,
        y: center_y - track_height * 0.5,
        width: (right - left).max(0.0),
        height: track_height,
    };
    let background = if !enabled {
        theme.color(ThemeColorToken::ControlDisabledSurface)
    } else if active {
        theme.color(ThemeColorToken::EditorActiveFieldSurface)
    } else {
        theme.surface
    };
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: background,
        },
    ));
    let text = if label.is_empty() {
        slider.display()
    } else {
        format!("{label}: {}", slider.display())
    };
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.control.padding_x,
                rect.y + theme.spacing.control.padding_y,
            ],
            text,
            if enabled {
                theme.text
            } else {
                theme.color(ThemeColorToken::ControlDisabledText)
            },
            theme,
            false,
        ),
    ));
    let track_color = if enabled {
        theme.border
    } else {
        theme.color(ThemeColorToken::ControlDisabledMark)
    };
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: track_rect,
            radius: track_height * 0.5,
            color: track_color,
        },
    ));
    let fill_width = (handle_x - left).max(0.0);
    if fill_width > 0.0 {
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    width: fill_width,
                    ..track_rect
                },
                radius: track_height * 0.5,
                color: if enabled {
                    theme.accent
                } else {
                    theme.color(ThemeColorToken::ControlDisabledMark)
                },
            },
        ));
    }
    if active && enabled {
        output.push(item(
            clip,
            z.saturating_add(3),
            Primitive::Ellipse {
                rect: ClipRect {
                    x: handle_x - radius - 2.0,
                    y: center_y - radius - 2.0,
                    width: (radius + 2.0) * 2.0,
                    height: (radius + 2.0) * 2.0,
                },
                color: theme.focus,
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(4),
        Primitive::Ellipse {
            rect: ClipRect {
                x: handle_x - radius,
                y: center_y - radius,
                width: radius * 2.0,
                height: radius * 2.0,
            },
            color: if enabled {
                theme.accent
            } else {
                theme.color(ThemeColorToken::ControlDisabledMark)
            },
        },
    ));
}

struct RadioGroupPaint<'a> {
    group: &'a RadioGroup,
    label: &'a str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    enabled: bool,
    theme: &'a Theme,
}

struct ColorRampPaint<'a> {
    ramp: &'a ColorRamp,
    state: ColorRampEditorState,
    label: &'a str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &'a Theme,
}

fn paint_radio_group(paint: RadioGroupPaint<'_>, output: &mut Vec<DrawItem>) {
    let RadioGroupPaint {
        group,
        label,
        rect,
        clip,
        z,
        enabled,
        theme,
    } = paint;
    let control_height = theme.control_height.max(1.0);
    let label_y = rect.y + (control_height - theme.font_size * theme.line_height).max(0.0) * 0.5;
    output.push(item(
        clip,
        z,
        styled_text_primitive(
            clip,
            [rect.x + theme.spacing.control.padding_x, label_y],
            label.to_owned(),
            if enabled && group.enabled {
                theme.text
            } else {
                theme.color(ThemeColorToken::ControlDisabledText)
            },
            theme,
            false,
        ),
    ));
    for (index, option) in group.options.iter().enumerate() {
        let row_y = rect.y + control_height * (index as f32 + 1.0);
        if row_y >= rect.y + rect.height {
            break;
        }
        let option_enabled = enabled && group.enabled && option.enabled;
        let diameter = (control_height - 8.0).clamp(8.0, 16.0);
        let circle = Rect {
            x: rect.x + theme.spacing.control.padding_x,
            y: row_y + (control_height - diameter) * 0.5,
            width: diameter,
            height: diameter,
        };
        let color = if option_enabled {
            theme.border
        } else {
            theme.color(ThemeColorToken::ControlDisabledMark)
        };
        output.push(item(
            clip,
            z.saturating_add(1 + index as i32 * 2),
            Primitive::Ellipse {
                rect: ClipRect::from(circle),
                color,
            },
        ));
        if group.selected == Some(index) {
            let inset = (diameter * 0.28).max(2.0);
            output.push(item(
                clip,
                z.saturating_add(2 + index as i32 * 2),
                Primitive::Ellipse {
                    rect: ClipRect::from(circle.inset(inset, inset, inset, inset)),
                    color: if option_enabled {
                        theme.accent
                    } else {
                        theme.color(ThemeColorToken::ControlDisabledMark)
                    },
                },
            ));
        }
        output.push(item(
            clip,
            z.saturating_add(2 + index as i32 * 2),
            styled_text_primitive(
                clip,
                [
                    circle.x + circle.width + theme.spacing.control.checkbox_label_gap,
                    row_y + (control_height - theme.font_size * theme.line_height).max(0.0) * 0.5,
                ],
                option.label.clone(),
                if option_enabled {
                    theme.text
                } else {
                    theme.color(ThemeColorToken::ControlDisabledText)
                },
                theme,
                false,
            ),
        ));
    }
}

struct HsvColorPaint<'a> {
    editor: HsvColorEditor,
    state: HsvColorEditorState,
    label: &'a str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &'a Theme,
}

struct CurveEditorPaint<'a> {
    editor: &'a CurveEditor,
    label: &'a str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &'a Theme,
}

struct NodeGraphPaint<'a> {
    graph: &'a NodeGraph,
    state: NodeGraphEditorState,
    output_label_widths: HashMap<(NodeId, usize), f32>,
    canvas_surface: Rgba,
    label: &'a str,
    rect: Rect,
    clip: ClipRect,
    z: i32,
    theme: &'a Theme,
    font: TextFont,
}

fn paint_hsv_color_editor(paint: HsvColorPaint<'_>, output: &mut Vec<DrawItem>) {
    let HsvColorPaint {
        editor,
        state,
        label,
        rect,
        clip,
        z,
        theme,
    } = paint;
    let spacing = theme.spacing.hsv;
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [
                rect.x + theme.spacing.editor.padding_x,
                rect.y + theme.spacing.editor.padding_y,
            ],
            label.to_owned(),
            theme.text,
            theme,
            false,
        ),
    ));
    let preview = ClipRect {
        x: rect.x + rect.width - spacing.preview_right_inset,
        y: rect.y + spacing.preview_top_inset,
        width: spacing.preview_width,
        height: spacing.preview_height,
    };
    for index in 0..4 {
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: preview.x + index as f32 * 9.5,
                    y: preview.y,
                    width: 10.0,
                    height: preview.height,
                },
                radius: 1.0,
                color: if index % 2 == 0 {
                    theme.background
                } else {
                    theme.surface_hover
                },
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::RoundedRect {
            rect: preview,
            radius: 2.0,
            color: editor.color(),
        },
    ));

    let (center, outer_radius, _) = hsv_wheel_geometry(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(2),
        Primitive::HsvWheel {
            rect: ClipRect {
                x: center[0] - outer_radius,
                y: center[1] - outer_radius,
                width: outer_radius * 2.0,
                height: outer_radius * 2.0,
            },
            hue_degrees: editor.hue_degrees,
            saturation: editor.saturation,
            value: editor.value,
        },
    ));

    let value_rect = hsv_value_rect(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.controls_x, rect.y + spacing.value_label_y],
            "Value".to_owned(),
            theme.text_muted,
            theme,
            false,
        ),
    ));
    const CONTROL_SEGMENTS: usize = 32;
    for segment in 0..CONTROL_SEGMENTS {
        let value = (segment as f32 + 0.5) / CONTROL_SEGMENTS as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: value_rect.x + value_rect.width * segment as f32 / CONTROL_SEGMENTS as f32,
                    y: value_rect.y,
                    width: value_rect.width / CONTROL_SEGMENTS as f32 + 0.6,
                    height: value_rect.height,
                },
                radius: if segment == 0 || segment + 1 == CONTROL_SEGMENTS {
                    3.0
                } else {
                    0.0
                },
                color: crate::hsv_to_rgb([editor.hue_degrees, editor.saturation, value], 1.0),
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(4),
        Primitive::Line {
            from: [
                value_rect.x + value_rect.width * editor.value,
                value_rect.y - 2.0,
            ],
            to: [
                value_rect.x + value_rect.width * editor.value,
                value_rect.y + value_rect.height + 2.0,
            ],
            width: 2.0,
            color: theme.text,
        },
    ));
    let alpha_rect = hsv_alpha_rect(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.controls_x, rect.y + spacing.alpha_label_y],
            "Alpha".to_owned(),
            theme.text_muted,
            theme,
            false,
        ),
    ));
    for cell in 0..8 {
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: alpha_rect.x + alpha_rect.width * cell as f32 / 8.0,
                    y: alpha_rect.y,
                    width: alpha_rect.width / 8.0 + 0.5,
                    height: alpha_rect.height,
                },
                radius: 0.0,
                color: if cell % 2 == 0 {
                    theme.background
                } else {
                    theme.surface_hover
                },
            },
        ));
    }
    let color = editor.color();
    for segment in 0..CONTROL_SEGMENTS {
        let alpha = (segment as f32 + 0.5) / CONTROL_SEGMENTS as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: alpha_rect.x + alpha_rect.width * segment as f32 / CONTROL_SEGMENTS as f32,
                    y: alpha_rect.y,
                    width: alpha_rect.width / CONTROL_SEGMENTS as f32 + 0.6,
                    height: alpha_rect.height,
                },
                radius: 0.0,
                color: Rgba(color.0, color.1, color.2, alpha),
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(4),
        Primitive::Line {
            from: [
                alpha_rect.x + alpha_rect.width * editor.alpha,
                alpha_rect.y - 2.0,
            ],
            to: [
                alpha_rect.x + alpha_rect.width * editor.alpha,
                alpha_rect.y + alpha_rect.height + 2.0,
            ],
            width: 2.0,
            color: theme.text,
        },
    ));

    for field in HsvColorField::ALL {
        let field_rect = hsv_field_rect(rect, field, spacing);
        let active = state.active_field == field;
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect::from(field_rect),
                radius: 3.0,
                color: if active {
                    theme.color(ThemeColorToken::EditorActiveFieldSurface)
                } else {
                    theme.background
                },
            },
        ));
        for (from, to) in [
            (
                [field_rect.x, field_rect.y],
                [field_rect.x + field_rect.width, field_rect.y],
            ),
            (
                [field_rect.x, field_rect.y + field_rect.height],
                [
                    field_rect.x + field_rect.width,
                    field_rect.y + field_rect.height,
                ],
            ),
        ] {
            output.push(item(
                clip,
                z.saturating_add(2),
                Primitive::Line {
                    from,
                    to,
                    width: if active { 1.5 } else { 0.75 },
                    color: if active { theme.accent } else { theme.border },
                },
            ));
        }
        let text = if active && state.editing {
            if field == HsvColorField::Hex {
                state.edit_buffer.clone()
            } else {
                format!("{}: {}", field.label(), state.edit_buffer)
            }
        } else {
            hsv_field_text(editor, field)
        };
        output.push(item(
            clip,
            z.saturating_add(3),
            styled_text_primitive(
                clip,
                [
                    field_rect.x + spacing.field_text_inset_x,
                    field_rect.y + spacing.field_text_inset_y,
                ],
                text,
                if active { theme.text } else { theme.text_muted },
                theme,
                false,
            ),
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.controls_x, rect.y + spacing.help_y],
            "Enter edit · arrows adjust · Home/End set limits".to_owned(),
            theme.text_muted,
            theme,
            false,
        ),
    ));
}

fn paint_curve_editor(paint: CurveEditorPaint<'_>, output: &mut Vec<DrawItem>) {
    let CurveEditorPaint {
        editor,
        label,
        rect,
        clip,
        z,
        theme,
    } = paint;
    let spacing = theme.spacing.editor;
    let curve_spacing = theme.spacing.curve;
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(1),
        styled_text_primitive(
            clip,
            [rect.x + spacing.padding_x, rect.y + spacing.padding_y],
            label.to_owned(),
            theme.text,
            theme,
            false,
        ),
    ));
    if let Some(point) = editor
        .selected
        .and_then(|id| editor.points.iter().find(|point| point.id == id))
    {
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width - curve_spacing.selected_status_width - spacing.padding_x,
                    rect.y + spacing.padding_y,
                ],
                format!(
                    "x {:.3}   y {:.3}   {:?}",
                    point.position[0], point.position[1], editor.interpolation
                ),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    } else {
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width
                        - curve_spacing.interpolation_status_width
                        - spacing.padding_x,
                    rect.y + spacing.padding_y,
                ],
                format!("{:?}", editor.interpolation),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    }
    let plot = curve_plot_rect(rect, theme);
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect::from(plot),
            radius: 2.0,
            color: theme.color(ThemeColorToken::CurvePlotSurface),
        },
    ));
    let x_grid_steps = (plot.width / spacing.grid_spacing.max(1.0))
        .ceil()
        .clamp(2.0, 8.0) as usize;
    let y_grid_steps = (plot.height / spacing.grid_spacing.max(1.0))
        .ceil()
        .clamp(2.0, 8.0) as usize;
    let grid = theme.color(ThemeColorToken::CurveGridLine);
    for index in 0..=x_grid_steps {
        let x = plot.x + plot.width * index as f32 / x_grid_steps as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [x, plot.y],
                to: [x, plot.y + plot.height],
                width: 1.0,
                color: grid,
            },
        ));
    }
    for index in 0..=y_grid_steps {
        let y = plot.y + plot.height * index as f32 / y_grid_steps as f32;
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::Line {
                from: [plot.x, y],
                to: [plot.x + plot.width, y],
                width: 1.0,
                color: grid,
            },
        ));
    }
    let axis = theme.color(ThemeColorToken::CurveAxisLine);
    for pair in editor.points.windows(2) {
        let left = pair[0];
        let right = pair[1];
        let p0 = curve_screen_position(editor, plot, left.position);
        let p3 = curve_screen_position(editor, plot, right.position);
        match editor.interpolation {
            Interpolation::Constant => {
                let corner =
                    curve_screen_position(editor, plot, [right.position[0], left.position[1]]);
                output.push(item(
                    clip,
                    z.saturating_add(3),
                    Primitive::Line {
                        from: p0,
                        to: corner,
                        width: 2.0,
                        color: theme.accent,
                    },
                ));
                output.push(item(
                    clip,
                    z.saturating_add(3),
                    Primitive::Line {
                        from: corner,
                        to: p3,
                        width: 2.0,
                        color: theme.accent,
                    },
                ));
            }
            Interpolation::Linear => output.push(item(
                clip,
                z.saturating_add(3),
                Primitive::Line {
                    from: p0,
                    to: p3,
                    width: 2.0,
                    color: theme.accent,
                },
            )),
            Interpolation::Smooth => {
                let c1 = curve_screen_position(
                    editor,
                    plot,
                    [
                        left.position[0] + left.right_handle[0],
                        left.position[1] + left.right_handle[1],
                    ],
                );
                let c2 = curve_screen_position(
                    editor,
                    plot,
                    [
                        right.position[0] + right.left_handle[0],
                        right.position[1] + right.left_handle[1],
                    ],
                );
                output.push(item(
                    clip,
                    z.saturating_add(3),
                    Primitive::Bezier {
                        points: [p0, c1, c2, p3],
                        width: 2.0,
                        color: theme.accent,
                    },
                ));
            }
        }
    }
    for point in &editor.points {
        let center = curve_screen_position(editor, plot, point.position);
        let selected = editor.selected == Some(point.id);
        if selected && editor.interpolation == Interpolation::Smooth {
            for handle in [point.left_handle, point.right_handle] {
                let end = curve_screen_position(
                    editor,
                    plot,
                    [point.position[0] + handle[0], point.position[1] + handle[1]],
                );
                output.push(item(
                    clip,
                    z.saturating_add(4),
                    Primitive::Line {
                        from: center,
                        to: end,
                        width: 1.0,
                        color: theme.text_muted,
                    },
                ));
                output.push(item(
                    clip,
                    z.saturating_add(5),
                    Primitive::RoundedRect {
                        rect: ClipRect {
                            x: end[0] - spacing.handle_radius,
                            y: end[1] - spacing.handle_radius,
                            width: spacing.handle_radius * 2.0,
                            height: spacing.handle_radius * 2.0,
                        },
                        radius: spacing.handle_radius,
                        color: theme.text_muted,
                    },
                ));
            }
        }
        let radius = spacing.handle_radius * if selected { 1.5 } else { 1.0 };
        output.push(item(
            clip,
            z.saturating_add(6),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: center[0] - radius,
                    y: center[1] - radius,
                    width: radius * 2.0,
                    height: radius * 2.0,
                },
                radius,
                color: if selected { theme.accent } else { theme.text },
            },
        ));
    }
    // The plot border is drawn last to distinguish the editable viewport edge from its grid.
    output.push(item(
        clip,
        z.saturating_add(7),
        Primitive::Line {
            from: [plot.x, plot.y],
            to: [plot.x + plot.width, plot.y],
            width: 1.0,
            color: axis,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(7),
        Primitive::Line {
            from: [plot.x, plot.y + plot.height],
            to: [plot.x + plot.width, plot.y + plot.height],
            width: 1.0,
            color: axis,
        },
    ));
}

fn paint_node_graph(paint: NodeGraphPaint<'_>, output: &mut Vec<DrawItem>) {
    let NodeGraphPaint {
        graph,
        state,
        output_label_widths,
        canvas_surface,
        label,
        rect,
        clip,
        z,
        theme,
        font,
    } = paint;
    let spacing = &theme.spacing.node_graph;
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: canvas_surface,
        },
    ));
    let header = Rect {
        height: spacing.header_height.min(rect.height),
        ..rect
    };
    if let Some(header_clip) = intersect_clip_rect(clip, header) {
        output.push(item(
            header_clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: ClipRect::from(header),
                radius: theme.corner_radius,
                color: theme.color(ThemeColorToken::NodeGraphHeaderSurface),
            },
        ));
        let inset = theme.spacing.control.padding_x.min(rect.width * 0.08);
        let summary_width = spacing.header_hint_width.min(rect.width * 0.44).max(1.0);
        let summary_right = rect.x + rect.width - inset;
        let summary_left = (summary_right - summary_width).max(rect.x + inset);
        let title_x = rect.x + inset;
        let title_width = (summary_left - title_x - inset).max(1.0);
        let summary = format!(
            "{} {} · {} {}",
            graph.nodes.len(),
            if graph.nodes.len() == 1 {
                "node"
            } else {
                "nodes"
            },
            graph.links.len(),
            if graph.links.len() == 1 {
                "link"
            } else {
                "links"
            },
        );
        let text_y = rect.y
            + spacing
                .title_padding_y
                .min((header.height - theme.font_size).max(0.0));
        output.push(item(
            header_clip,
            z.saturating_add(2),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [title_x, text_y],
                    text: label.to_owned(),
                },
                theme.text,
                title_width,
                bevy_text::Justify::Left,
                &font,
                theme,
                1.0,
            ),
        ));
        output.push(item(
            header_clip,
            z.saturating_add(2),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [summary_left, text_y],
                    text: summary,
                },
                theme.text_muted,
                summary_right - summary_left,
                bevy_text::Justify::Right,
                &font,
                theme,
                1.0,
            ),
        ));
    }

    let viewport = Rect {
        y: rect.y + header.height,
        height: (rect.height - header.height).max(0.0),
        ..rect
    };
    let Some(viewport_clip) = intersect_clip_rect(clip, viewport) else {
        return;
    };
    let grid_spacing = (spacing.grid_spacing * graph.zoom)
        .clamp(spacing.grid_spacing_min, spacing.grid_spacing_max);
    let grid = theme
        .color(ThemeColorToken::NodeGraphGridLine)
        .mix(canvas_surface, 0.6);
    let dot_size = (2.0 * graph.zoom.sqrt()).clamp(1.5, 2.6);
    let start_x = viewport.x + graph.pan[0].rem_euclid(grid_spacing);
    let start_y = viewport.y + graph.pan[1].rem_euclid(grid_spacing);
    let mut dot_x = start_x;
    let mut columns = 0;
    let mut dot_count = 0;
    while dot_x <= viewport.x + viewport.width && columns < 48 && dot_count < 1024 {
        let mut dot_y = start_y;
        let mut rows = 0;
        while dot_y <= viewport.y + viewport.height && rows < 48 && dot_count < 1024 {
            output.push(item(
                viewport_clip,
                z.saturating_add(1),
                Primitive::Ellipse {
                    rect: ClipRect {
                        x: dot_x - dot_size * 0.5,
                        y: dot_y - dot_size * 0.5,
                        width: dot_size,
                        height: dot_size,
                    },
                    color: grid,
                },
            ));
            dot_y += grid_spacing;
            rows += 1;
            dot_count += 1;
        }
        dot_x += grid_spacing;
        columns += 1;
    }

    let nodes_by_id: HashMap<_, _> = graph.nodes.iter().map(|node| (node.id, node)).collect();
    for link in &graph.links {
        let (Some(from), Some(to)) = (
            nodes_by_id.get(&link.from_node),
            nodes_by_id.get(&link.to_node),
        ) else {
            continue;
        };
        let Some(points) = node_graph_link_points_between(graph, rect, *link, from, to, spacing)
        else {
            continue;
        };
        if node_graph_bezier_bounds(points, 8.0)
            .intersection(viewport)
            .is_none()
        {
            continue;
        }
        let selected = graph.selected_link == Some(link.id);
        output.push(item(
            viewport_clip,
            z.saturating_add(3),
            Primitive::Bezier {
                points,
                width: if selected { 3.5 } else { 2.5 },
                color: if selected {
                    theme.accent
                } else {
                    from.sockets
                        .get(link.from_socket)
                        .map_or(theme.text_muted, |socket| {
                            socket_type_color(socket.socket_type, theme)
                        })
                },
            },
        ));
    }

    for node in &graph.nodes {
        let node_rect = node_graph_node_rect(graph, rect, node, spacing);
        let Some(node_clip) = intersect_clip_rect(viewport_clip, node_rect) else {
            continue;
        };
        let selected = graph.selected.contains(&node.id);
        output.push(item(
            node_clip,
            z.saturating_add(4),
            Primitive::RoundedRect {
                rect: ClipRect::from(node_rect),
                radius: spacing.node_radius,
                color: if selected {
                    theme.surface.mix(theme.accent, 0.08)
                } else {
                    theme.surface
                },
            },
        ));
        let title_height = (spacing.header_height * graph.zoom).min(node_rect.height);
        let node_header = Rect {
            height: title_height,
            ..node_rect
        };
        let title_clip = intersect_clip_rect(node_clip, node_header).unwrap_or(node_clip);
        output.push(item(
            title_clip,
            z.saturating_add(5),
            Primitive::RoundedRect {
                rect: ClipRect::from(node_header),
                radius: spacing.node_radius,
                color: node_graph_category_color(node.category, theme),
            },
        ));
        let zoom = graph.zoom.max(0.01);
        let title_x = node_rect.x + (spacing.title_padding_x + 8.0) * zoom;
        let title_width =
            (node_rect.x + node_rect.width - spacing.body_padding * zoom - title_x).max(1.0);
        output.push(item(
            title_clip,
            z.saturating_add(6),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [title_x, node_rect.y + spacing.title_padding_y * zoom],
                    text: node.title.clone(),
                },
                theme.text,
                title_width,
                bevy_text::Justify::Left,
                &font,
                theme,
                zoom,
            ),
        ));
        output.push(item(
            title_clip,
            z.saturating_add(6),
            node_graph_text_primitive(
                TextPlacement {
                    origin: [
                        node_rect.x + spacing.collapse_toggle_inset_x * zoom,
                        node_rect.y + spacing.title_padding_y * zoom,
                    ],
                    text: if node.collapsed { "▸" } else { "▾" }.to_owned(),
                },
                theme.text,
                (spacing.title_padding_x - spacing.collapse_toggle_inset_x).max(1.0) * zoom,
                bevy_text::Justify::Left,
                &font,
                theme,
                zoom,
            ),
        ));

        for (socket_index, socket) in node.sockets.iter().enumerate() {
            let Some(center) = node_graph_socket_position(graph, rect, node, socket_index, spacing)
            else {
                continue;
            };
            let color = socket_type_color(socket.socket_type, theme);
            let radius = (spacing.socket_radius * graph.zoom).max(0.0);
            output.push(item(
                viewport_clip,
                z.saturating_add(8),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        x: center[0] - radius,
                        y: center[1] - radius,
                        width: radius * 2.0,
                        height: radius * 2.0,
                    },
                    radius,
                    color,
                },
            ));
            let label_row = node.sockets[..socket_index]
                .iter()
                .filter(|previous| previous.direction == socket.direction)
                .count();
            let opposite_direction = match socket.direction {
                SocketDirection::Input => SocketDirection::Output,
                SocketDirection::Output => SocketDirection::Input,
            };
            let has_counterpart = node
                .sockets
                .iter()
                .filter(|candidate| candidate.direction == opposite_direction)
                .nth(label_row)
                .is_some();
            let content_left = node_rect.x + spacing.socket_label_inset_x * zoom;
            let content_right = node_rect.x + node_rect.width - spacing.socket_label_inset_x * zoom;
            let content_width = (content_right - content_left).max(1.0);
            let split = content_left + content_width * 0.5;
            let (text_x, max_width, text_clip) = match socket.direction {
                SocketDirection::Input if has_counterpart => (
                    content_left,
                    (split - content_left).max(1.0),
                    intersect_clip_rect(
                        node_clip,
                        Rect::new(
                            content_left,
                            node_rect.y,
                            split - content_left,
                            node_rect.height,
                        ),
                    )
                    .unwrap_or(node_clip),
                ),
                SocketDirection::Input => (content_left, content_width, node_clip),
                SocketDirection::Output => {
                    let lane_left = if has_counterpart { split } else { content_left };
                    let lane_width = (content_right - lane_left).max(1.0);
                    let label_width = (output_label_widths
                        .get(&(node.id, socket_index))
                        .copied()
                        .unwrap_or(lane_width)
                        + 1.0)
                        .clamp(1.0, lane_width);
                    let clip = if has_counterpart {
                        intersect_clip_rect(
                            node_clip,
                            Rect::new(lane_left, node_rect.y, lane_width, node_rect.height),
                        )
                        .unwrap_or(node_clip)
                    } else {
                        node_clip
                    };
                    (content_right - label_width, label_width, clip)
                }
            };
            output.push(item(
                text_clip,
                z.saturating_add(8),
                node_graph_text_primitive(
                    TextPlacement {
                        origin: [text_x, center[1] - spacing.socket_label_inset_y * zoom],
                        text: socket.name.clone(),
                    },
                    theme.text_muted,
                    max_width,
                    bevy_text::Justify::Left,
                    &font,
                    theme,
                    zoom,
                ),
            ));
        }
        push_rect_outline_with_width(
            node_rect,
            if selected {
                theme.accent
            } else {
                theme.border.mix(theme.surface, 0.2)
            },
            node_clip,
            z.saturating_add(7),
            if selected { 2.0 } else { 1.0 },
            output,
        );
    }

    if let Some(gesture) = state.gesture {
        match gesture {
            NodeGraphGesture::LinkDrag {
                start_node,
                start_socket,
                start_direction,
                current_pointer,
                ..
            } => {
                if let Some(node) = graph.nodes.iter().find(|node| node.id == start_node)
                    && let Some(start) =
                        node_graph_socket_position(graph, rect, node, start_socket, spacing)
                {
                    let control = ((current_pointer[0] - start[0]).abs() * 0.5)
                        .max(spacing.connection_tangent_min);
                    let sign = if start_direction == SocketDirection::Output {
                        1.0
                    } else {
                        -1.0
                    };
                    output.push(item(
                        viewport_clip,
                        z.saturating_add(9),
                        Primitive::Bezier {
                            points: [
                                start,
                                [start[0] + control * sign, start[1]],
                                [current_pointer[0] - control * sign, current_pointer[1]],
                                current_pointer,
                            ],
                            width: 2.0,
                            color: theme.accent,
                        },
                    ));
                }
            }
            NodeGraphGesture::BoxSelect {
                start_pointer,
                current_pointer,
                ..
            } => {
                let selection = Rect::new(
                    start_pointer[0].min(current_pointer[0]),
                    start_pointer[1].min(current_pointer[1]),
                    (start_pointer[0] - current_pointer[0]).abs(),
                    (start_pointer[1] - current_pointer[1]).abs(),
                );
                let outline = theme.accent;
                output.push(item(
                    viewport_clip,
                    z.saturating_add(9),
                    Primitive::RoundedRect {
                        rect: ClipRect::from(selection),
                        radius: 0.0,
                        color: theme.color(ThemeColorToken::NodeGraphSelectionOverlay),
                    },
                ));
                push_rect_outline(
                    selection,
                    outline,
                    viewport_clip,
                    z.saturating_add(10),
                    output,
                );
            }
            NodeGraphGesture::CutLinks { points, .. } => {
                for pair in points.windows(2) {
                    output.push(item(
                        viewport_clip,
                        z.saturating_add(10),
                        Primitive::Line {
                            from: pair[0],
                            to: pair[1],
                            width: 2.0,
                            color: theme.error,
                        },
                    ));
                }
            }
            NodeGraphGesture::Pan { .. }
            | NodeGraphGesture::MoveNodes { .. }
            | NodeGraphGesture::Selection { .. }
            | NodeGraphGesture::SelectLink { .. } => {}
        }
    }
}

fn node_graph_text_primitive(
    placement: TextPlacement,
    color: Rgba,
    max_width: f32,
    justify: bevy_text::Justify,
    font: &TextFont,
    theme: &Theme,
    zoom: f32,
) -> Primitive {
    Primitive::TextWrapped {
        origin: placement.origin,
        text: placement.text,
        color,
        max_width: max_width.max(1.0),
        wrap: false,
        style: Some(TextRenderStyle {
            font: Some(font.clone().with_font_size(theme.font_size * zoom)),
            line_height: Some(LineHeight::RelativeToFont(theme.line_height)),
            letter_spacing: bevy_text::LetterSpacing::default(),
            justify,
            line_break: LineBreak::NoWrap,
        }),
    }
}

fn node_graph_category_color(category: NodeCategory, theme: &Theme) -> Rgba {
    let surface = theme.color(ThemeColorToken::NodeGraphHeaderSurface);
    let hue = match category {
        NodeCategory::General => return surface,
        NodeCategory::Input | NodeCategory::Converter => {
            theme.color(ThemeColorToken::NodeGraphTextSocket)
        }
        NodeCategory::Texture => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        NodeCategory::Color | NodeCategory::Output => {
            theme.color(ThemeColorToken::NodeGraphColorSocket)
        }
        NodeCategory::Geometry => theme.color(ThemeColorToken::NodeGraphVectorSocket),
    };
    surface.mix(hue, 0.5)
}

fn node_graph_canvas_surface(world: &World, entity: Entity, theme: &Theme) -> Rgba {
    let token_surface = theme.color(ThemeColorToken::NodeGraphSurface);
    let inline_background = world
        .get::<UiInlineStyle>(entity)
        .and_then(|style| style.0.get(&UiStyleProperty::Background));
    let inline_override = inline_background.is_some();

    let role = world.get::<UiRole>(entity);
    let classes = world.get::<UiClasses>(entity);
    let mut state = world
        .get::<UiStyleState>(entity)
        .copied()
        .unwrap_or_default();
    if world
        .get::<UiEnabled>(entity)
        .is_some_and(|enabled| !enabled.0)
    {
        state = state.union(UiStyleState::DISABLED);
    }
    if let Some(focus) = world.get_resource::<UiFocus>() {
        for window_state in focus.windows.values() {
            if window_state.hovered == Some(entity) {
                state = state.union(UiStyleState::HOVERED);
            }
            if window_state.captured == Some(entity) {
                state = state.union(UiStyleState::PRESSED);
            }
            if window_state.focused == Some(entity) {
                state = state.union(UiStyleState::FOCUSED);
            }
        }
    }
    let stylesheet_override = world.get_resource::<UiStyleSheet>().is_some_and(|sheet| {
        sheet.rules.iter().any(|rule| {
            let selector = &rule.selector;
            rule.declaration
                .0
                .contains_key(&UiStyleProperty::Background)
                && selector
                    .role
                    .as_ref()
                    .is_none_or(|wanted| role.is_some_and(|actual| &actual.0 == wanted))
                && selector
                    .class
                    .as_ref()
                    .is_none_or(|wanted| classes.is_some_and(|actual| actual.contains(wanted)))
                && state.contains(selector.required_state)
                && !state.intersects(selector.forbidden_state)
        })
    });
    if !inline_override && !stylesheet_override {
        return token_surface;
    }

    world
        .get::<UiComputedStyle>(entity)
        .and_then(|style| style.color(UiStyleProperty::Background))
        .or_else(|| match inline_background {
            Some(UiStyleValue::Color(color)) => Some(*color),
            Some(UiStyleValue::ColorToken(token)) => Some(theme.color(*token)),
            Some(_) | None => None,
        })
        .unwrap_or(token_surface)
}

fn measure_node_graph_output_label_widths(
    world: &mut World,
    graph: &NodeGraph,
    font: &TextFont,
    theme: &Theme,
) -> HashMap<(NodeId, usize), f32> {
    let labels = graph
        .nodes
        .iter()
        .flat_map(|node| {
            node.sockets
                .iter()
                .enumerate()
                .filter(|(_, socket)| socket.direction == SocketDirection::Output)
                .map(|(index, socket)| ((node.id, index), socket.name.clone()))
        })
        .collect::<Vec<_>>();
    if labels.is_empty()
        || !world.contains_resource::<TextPipeline>()
        || !world.contains_resource::<Assets<Font>>()
        || !world.contains_resource::<FontCx>()
        || !world.contains_resource::<LayoutCx>()
    {
        return HashMap::new();
    }

    let font = font
        .clone()
        .with_font_size(theme.font_size * graph.zoom.max(0.01));
    world.resource_scope(|world, mut pipeline: Mut<TextPipeline>| {
        world.resource_scope(|world, fonts: Mut<Assets<Font>>| {
            world.resource_scope(|world, mut font_cx: Mut<FontCx>| {
                world.resource_scope(|_, mut layout_cx: Mut<LayoutCx>| {
                    labels
                        .into_iter()
                        .filter_map(|(key, text)| {
                            let layout = TextLayout::shape_with_bevy(
                                &text,
                                f32::INFINITY,
                                &font,
                                &mut pipeline,
                                &fonts,
                                &mut font_cx,
                                &mut layout_cx,
                            )
                            .ok()?;
                            Some((key, layout.bounds().width.max(0.0)))
                        })
                        .collect()
                })
            })
        })
    })
}

fn socket_type_color(socket_type: crate::SocketType, theme: &Theme) -> Rgba {
    match socket_type {
        crate::SocketType::Any => theme.text_muted,
        crate::SocketType::Boolean => theme.color(ThemeColorToken::NodeGraphBooleanSocket),
        crate::SocketType::Float => theme.color(ThemeColorToken::NodeGraphFloatSocket),
        crate::SocketType::Vector2 | crate::SocketType::Vector3 => {
            theme.color(ThemeColorToken::NodeGraphVectorSocket)
        }
        crate::SocketType::Color => theme.color(ThemeColorToken::NodeGraphColorSocket),
        crate::SocketType::Text => theme.color(ThemeColorToken::NodeGraphTextSocket),
    }
}

fn push_rect_outline(rect: Rect, color: Rgba, clip: ClipRect, z: i32, output: &mut Vec<DrawItem>) {
    push_rect_outline_with_width(rect, color, clip, z, 1.0, output);
}

fn push_rect_outline_with_width(
    rect: Rect,
    color: Rgba,
    clip: ClipRect,
    z: i32,
    width: f32,
    output: &mut Vec<DrawItem>,
) {
    let left = rect.x;
    let top = rect.y;
    let right = rect.x + rect.width;
    let bottom = rect.y + rect.height;
    for (from, to) in [
        ([left, top], [right, top]),
        ([right, top], [right, bottom]),
        ([right, bottom], [left, bottom]),
        ([left, bottom], [left, top]),
    ] {
        output.push(item(
            clip,
            z,
            Primitive::Line {
                from,
                to,
                width,
                color,
            },
        ));
    }
}

fn paint_color_ramp(paint: ColorRampPaint<'_>, output: &mut Vec<DrawItem>) {
    let ColorRampPaint {
        ramp,
        state,
        label,
        rect,
        clip,
        z,
        theme,
    } = paint;
    let spacing = theme.spacing.color_ramp;
    output.push(item(
        clip,
        z,
        Primitive::RoundedRect {
            rect: ClipRect::from(rect),
            radius: theme.corner_radius,
            color: theme.surface,
        },
    ));
    let mode = match ramp.interpolation {
        Interpolation::Constant => "Constant",
        Interpolation::Linear => "Linear",
        Interpolation::Smooth => "Smooth",
    };
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.title_x, rect.y + spacing.title_y],
            format!("{label} · {mode}"),
            theme.text,
            theme,
            false,
        ),
    ));
    for (index, (title, color)) in [
        ("+", theme.accent),
        ("−", theme.surface_hover),
        (mode, theme.surface_hover),
    ]
    .into_iter()
    .enumerate()
    {
        let width = spacing.button_width;
        let button_rect = ClipRect {
            x: rect.x + rect.width - spacing.button_right_inset
                + index as f32 * spacing.button_step,
            y: rect.y + spacing.button_top_inset,
            width,
            height: spacing.button_height,
        };
        output.push(item(
            clip,
            z.saturating_add(1),
            Primitive::RoundedRect {
                rect: button_rect,
                radius: 3.0,
                color,
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    button_rect.x + spacing.button_label_x,
                    button_rect.y + spacing.button_label_y,
                ],
                title.to_owned(),
                theme.text,
                theme,
                false,
            ),
        ));
    }

    let track = color_ramp_track_rect(rect, spacing);
    output.push(item(
        clip,
        z.saturating_add(1),
        Primitive::RoundedRect {
            rect: ClipRect::from(track),
            radius: 2.0,
            color: theme.background,
        },
    ));
    const RAMP_SEGMENTS: usize = 48;
    for index in 0..RAMP_SEGMENTS {
        let t = (index as f32 + 0.5) / RAMP_SEGMENTS as f32;
        let color = ramp.sample(t);
        output.push(item(
            clip,
            z.saturating_add(2),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: track.x + track.width * index as f32 / RAMP_SEGMENTS as f32,
                    y: track.y,
                    width: track.width / RAMP_SEGMENTS as f32 + 0.8,
                    height: track.height,
                },
                radius: if index == 0 || index + 1 == RAMP_SEGMENTS {
                    2.0
                } else {
                    0.0
                },
                color,
            },
        ));
    }
    output.push(item(
        clip,
        z.saturating_add(3),
        Primitive::Line {
            from: [track.x, track.y],
            to: [track.x + track.width, track.y],
            width: 1.0,
            color: theme.border,
        },
    ));
    output.push(item(
        clip,
        z.saturating_add(3),
        Primitive::Line {
            from: [track.x, track.y + track.height],
            to: [track.x + track.width, track.y + track.height],
            width: 1.0,
            color: theme.border,
        },
    ));
    for stop in &ramp.stops {
        let x = track.x + stop.position * track.width;
        output.push(item(
            clip,
            z.saturating_add(4),
            Primitive::Line {
                from: [x, track.y + track.height],
                to: [x, track.y + track.height + spacing.marker_line_extension],
                width: if ramp.selected == Some(stop.id) {
                    2.0
                } else {
                    1.0
                },
                color: if ramp.selected == Some(stop.id) {
                    theme.accent
                } else {
                    theme.border
                },
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(5),
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: x - spacing.marker_handle_radius,
                    y: track.y + track.height + spacing.marker_handle_top_inset,
                    width: spacing.marker_handle_radius * 2.0,
                    height: spacing.marker_handle_radius * 2.0,
                },
                radius: spacing.marker_handle_radius * 0.4,
                color: if ramp.selected == Some(stop.id) {
                    theme.accent
                } else {
                    stop.color
                },
            },
        ));
    }

    let Some(selected) = ramp.selected_stop() else {
        return;
    };
    let channels = [
        selected.color.0,
        selected.color.1,
        selected.color.2,
        selected.color.3,
    ];
    output.push(item(
        clip,
        z.saturating_add(2),
        styled_text_primitive(
            clip,
            [rect.x + spacing.details_x, rect.y + spacing.details_y],
            format!(
                "Stop {:.3}   RGBA {:.2}, {:.2}, {:.2}, {:.2}",
                selected.position, channels[0], channels[1], channels[2], channels[3]
            ),
            theme.text_muted,
            theme,
            false,
        ),
    ));
    for (index, value) in channels.into_iter().enumerate() {
        let channel = ColorChannel::from_index(index);
        let row = color_ramp_channel_rect(rect, index, spacing);
        output.push(item(
            clip,
            z.saturating_add(3),
            styled_text_primitive(
                clip,
                [
                    rect.x + spacing.channel_label_x,
                    row.y + spacing.channel_label_y,
                ],
                channel.label().to_owned(),
                if state.active_channel == channel {
                    theme.accent
                } else {
                    theme.text_muted
                },
                theme,
                false,
            ),
        ));
        const CHANNEL_SEGMENTS: usize = 24;
        for segment in 0..CHANNEL_SEGMENTS {
            let sample = (segment as f32 + 0.5) / CHANNEL_SEGMENTS as f32;
            output.push(item(
                clip,
                z.saturating_add(2),
                Primitive::RoundedRect {
                    rect: ClipRect {
                        x: row.x + row.width * segment as f32 / CHANNEL_SEGMENTS as f32,
                        y: row.y,
                        width: row.width / CHANNEL_SEGMENTS as f32 + 0.5,
                        height: row.height,
                    },
                    radius: if segment == 0 || segment + 1 == CHANNEL_SEGMENTS {
                        2.0
                    } else {
                        0.0
                    },
                    color: color_ramp_channel_color(selected.color, channel, sample),
                },
            ));
        }
        output.push(item(
            clip,
            z.saturating_add(4),
            Primitive::Line {
                from: [
                    row.x + row.width * value,
                    row.y - spacing.channel_label_y - 1.0,
                ],
                to: [
                    row.x + row.width * value,
                    row.y + row.height + spacing.channel_label_y + 1.0,
                ],
                width: if state.active_channel == channel {
                    2.0
                } else {
                    1.0
                },
                color: if state.active_channel == channel {
                    theme.accent
                } else {
                    theme.text
                },
            },
        ));
        output.push(item(
            clip,
            z.saturating_add(3),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width - spacing.channel_value_right_inset,
                    row.y + spacing.channel_label_y,
                ],
                format!("{value:.2}"),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    }
    if state.editing_color {
        output.push(item(
            clip,
            z.saturating_add(2),
            styled_text_primitive(
                clip,
                [
                    rect.x + rect.width - spacing.editing_right_inset,
                    rect.y + rect.height - spacing.editing_bottom_inset,
                ],
                "Editing color · arrows adjust".to_owned(),
                theme.text_muted,
                theme,
                false,
            ),
        ));
    }
}

fn color_ramp_channel_color(color: Rgba, channel: ColorChannel, value: f32) -> Rgba {
    match channel {
        ColorChannel::Red => Rgba(value, color.1, color.2, color.3),
        ColorChannel::Green => Rgba(color.0, value, color.2, color.3),
        ColorChannel::Blue => Rgba(color.0, color.1, value, color.3),
        ColorChannel::Alpha => Rgba(color.0, color.1, color.2, value),
    }
}

fn styled_text_primitive(
    clip: ClipRect,
    origin: [f32; 2],
    text: String,
    color: Rgba,
    theme: &Theme,
    wrap: bool,
) -> Primitive {
    Primitive::TextWrapped {
        origin,
        text,
        color,
        max_width: (clip.width - (origin[0] - clip.x)).max(1.0),
        wrap,
        style: Some(TextRenderStyle {
            font: Some(ui_default_text_font(theme.font_size)),
            line_height: Some(LineHeight::RelativeToFont(theme.line_height)),
            letter_spacing: bevy_text::LetterSpacing::default(),
            justify: bevy_text::Justify::default(),
            line_break: if wrap {
                LineBreak::WordBoundary
            } else {
                LineBreak::NoWrap
            },
        }),
    }
}

struct TextPlacement {
    origin: [f32; 2],
    text: String,
}

fn styled_text_primitive_for_entity(
    world: &World,
    entity: Entity,
    clip: ClipRect,
    placement: TextPlacement,
    color: Rgba,
    theme: &Theme,
    wrap: bool,
) -> Primitive {
    let mut primitive =
        styled_text_primitive(clip, placement.origin, placement.text, color, theme, wrap);
    if let Primitive::TextWrapped {
        style: Some(style), ..
    } = &mut primitive
        && let Some(font) = style.font.take()
    {
        style.font = Some(resolve_text_font(world, entity, font));
    }
    primitive
}

fn is_visible(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut remaining_entities = world.entities().len();
    while let Some(at) = current {
        if remaining_entities == 0 || world.get::<bui_core::UiNode>(at).is_none() {
            return false;
        }
        remaining_entities -= 1;
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetPlugin;
    use crate::{ListItem, PanelOrderStore, WidgetNode, WidgetTree};
    use bevy_app::App;
    use bui_core::{
        CorePlugin, UiChildren, UiFocusable, UiHitBox, UiInputEvent, UiInputQueue, UiKey,
        UiKeyCode, UiModifiers, UiNode, UiRect, UiRoutedEvent, UiRoutedEvents, UiTabIndex,
    };
    use bui_style::{UiStyleDeclaration, UiStyleRule, UiStyleSelector};

    fn browser_items(world: &mut World, owner: Entity) -> Vec<(Entity, FileBrowserItem)> {
        let mut query = world.query::<(Entity, &FileBrowserItem)>();
        query
            .iter(world)
            .filter(|(entity, item)| {
                item.owner == owner
                    && world
                        .get::<UiVisible>(*entity)
                        .is_some_and(|visible| visible.0)
            })
            .map(|(entity, item)| (entity, item.clone()))
            .collect()
    }

    fn collection_items(world: &mut World, owner: Entity) -> Vec<(Entity, CollectionItemControl)> {
        let mut query = world.query::<(Entity, &CollectionItemControl)>();
        query
            .iter(world)
            .filter(|(_, item)| item.owner == owner)
            .map(|(entity, item)| (entity, item.clone()))
            .collect()
    }

    fn data_table_cells(world: &mut World, owner: Entity) -> Vec<(Entity, DataTableCellControl)> {
        let mut query = world.query::<(Entity, &DataTableCellControl)>();
        query
            .iter(world)
            .filter(|(_, cell)| cell.owner == owner)
            .map(|(entity, cell)| (entity, cell.clone()))
            .collect()
    }

    fn data_table_headers(
        world: &mut World,
        owner: Entity,
    ) -> Vec<(Entity, DataTableHeaderControl)> {
        let mut query = world.query::<(Entity, &DataTableHeaderControl)>();
        query
            .iter(world)
            .filter(|(_, header)| header.owner == owner)
            .map(|(entity, header)| (entity, header.clone()))
            .collect()
    }

    fn sample_table(rows: usize) -> DataTable {
        DataTable {
            columns: vec![
                crate::TableColumn::new("Name", 104.0),
                crate::TableColumn::new("Type", 92.0),
            ],
            rows: (0..rows)
                .map(|index| crate::TableRow {
                    id: index as u64 + 1,
                    cells: vec![
                        format!("Object {index:03}"),
                        if index % 2 == 0 {
                            "Mesh".into()
                        } else {
                            "Light".into()
                        },
                    ],
                })
                .collect(),
            ..DataTable::default()
        }
    }

    #[derive(Resource, Debug, Default, PartialEq, Eq)]
    struct VirtualSyncChanges {
        table_cells: usize,
        table_headers: usize,
        graph_nodes: usize,
        graph_sockets: usize,
        graph_links: usize,
        layout_results: usize,
        hit_boxes: usize,
    }

    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn count_virtual_sync_changes(
        table_cells: Query<Entity, Changed<DataTableCellControl>>,
        table_headers: Query<Entity, Changed<DataTableHeaderControl>>,
        graph_nodes: Query<Entity, Changed<NodeGraphNodeControl>>,
        graph_sockets: Query<Entity, Changed<NodeGraphSocketControl>>,
        graph_links: Query<Entity, Changed<NodeGraphLinkControl>>,
        table_layouts: Query<Entity, (With<DataTableCellControl>, Changed<UiLayoutResult>)>,
        graph_layouts: Query<
            Entity,
            (
                Or<(
                    With<NodeGraphNodeControl>,
                    With<NodeGraphSocketControl>,
                    With<NodeGraphLinkControl>,
                )>,
                Changed<UiLayoutResult>,
            ),
        >,
        table_hits: Query<Entity, (With<DataTableCellControl>, Changed<UiHitBox>)>,
        graph_hits: Query<
            Entity,
            (
                Or<(
                    With<NodeGraphNodeControl>,
                    With<NodeGraphSocketControl>,
                    With<NodeGraphLinkControl>,
                )>,
                Changed<UiHitBox>,
            ),
        >,
        mut changes: ResMut<VirtualSyncChanges>,
    ) {
        *changes = VirtualSyncChanges {
            table_cells: table_cells.iter().count(),
            table_headers: table_headers.iter().count(),
            graph_nodes: graph_nodes.iter().count(),
            graph_sockets: graph_sockets.iter().count(),
            graph_links: graph_links.iter().count(),
            layout_results: table_layouts.iter().count() + graph_layouts.iter().count(),
            hit_boxes: table_hits.iter().count() + graph_hits.iter().count(),
        };
    }

    fn spawn_color_ramp_widget(app: &mut App, rect: UiRect) -> (Entity, Entity) {
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let entity = WidgetTree::new(WidgetNode::color_ramp_default("Gradient"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(entity).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 7,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        (window, entity)
    }

    fn spawn_hsv_color_widget(app: &mut App, rect: UiRect) -> (Entity, Entity) {
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let entity = WidgetTree::new(WidgetNode::hsv_color_default("Color"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(entity).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 9,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        (window, entity)
    }

    fn spawn_curve_editor_widget(app: &mut App, rect: UiRect) -> (Entity, Entity) {
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let entity = WidgetTree::new(WidgetNode::curve_editor_default("Curve"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(entity).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 11,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        (window, entity)
    }

    fn route_curve_key(
        app: &mut App,
        window: Entity,
        target: Entity,
        key: UiKeyCode,
        modifiers: UiModifiers,
    ) {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        app.update();
    }

    #[test]
    fn plugin_routes_keyboard_and_pointer_activations_into_value_events() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let checkbox = app.world_mut().spawn((
            UiNode,
            UiFocusable,
            UiHitBox {
                rect: UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                clip: None,
                z_index: 0,
                window: Some(window),
            },
            Checkbox::new("check", false),
        ));
        let id = checkbox.id();
        // Exercise the same routed queue consumed by the plugin without requiring CorePlugin here.
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: id,
                button: UiPointerButton::Primary,
                position: [5.0, 5.0],
            },
            UiRoutedEvent::PointerUp {
                window,
                target: id,
                button: UiPointerButton::Primary,
                position: [5.0, 5.0],
                cancelled: false,
            },
            UiRoutedEvent::Click {
                window,
                target: id,
                button: UiPointerButton::Primary,
                click_count: 1,
                position: [5.0, 5.0],
            },
        ];
        app.update();
        assert!(app.world().get::<Checkbox>(id).unwrap().is_checked());
        let events = app.world_mut().resource_mut::<WidgetEventQueue>().take();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, WidgetEvent::CheckboxPaintCommitted { .. }))
        );

        let button = app.world_mut().spawn((UiNode, Button::new("Run"))).id();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: button,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(app.world().get::<Button>(button).unwrap().activations, 1);
        assert!(
            app.world_mut()
                .resource_mut::<WidgetEventQueue>()
                .take()
                .iter()
                .any(
                    |event| matches!(event, WidgetEvent::Activated { target } if *target == button)
                )
        );
    }

    #[test]
    fn color_ramp_routes_stop_reorder_and_gesture_undo_redo() {
        let mut app = App::new();
        let rect = UiRect::new(20.0, 30.0, 360.0, 194.0);
        let (window, entity) = spawn_color_ramp_widget(&mut app, rect);
        {
            let mut ramp = app.world_mut().get_mut::<ColorRamp>(entity).unwrap();
            ramp.add_stop(0.25, Rgba(1.0, 0.0, 0.0, 1.0)).unwrap();
            ramp.add_stop(0.75, Rgba(0.0, 1.0, 0.0, 1.0)).unwrap();
        }
        let selected = app
            .world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .stops
            .iter()
            .find(|stop| (stop.position - 0.25).abs() < f32::EPSILON)
            .unwrap()
            .id;
        let track = color_ramp_track_rect(rect, Theme::default().spacing.color_ramp);
        let start = [track.x + track.width * 0.25, track.y + track.height + 10.0];
        let end = [track.x + track.width * 0.9, start[1]];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: end,
                delta: [end[0] - start[0], 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: end,
                cancelled: false,
            },
            UiRoutedEvent::Click {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                click_count: 1,
                position: end,
            },
        ];
        app.update();
        let ramp = app.world().get::<ColorRamp>(entity).unwrap();
        assert!(
            (ramp
                .stops
                .iter()
                .find(|stop| stop.id == selected)
                .unwrap()
                .position
                - 0.9)
                .abs()
                < 0.01
        );
        let order: Vec<_> = ramp.stops.iter().map(|stop| stop.id).collect();
        assert_eq!(order, vec![1, 4, selected, 2]);
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ColorRampCommitted { target, .. } if *target == entity)));

        let key = |key, modifiers| UiRoutedEvent::Key {
            window,
            target: entity,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                ..Default::default()
            },
        )];
        app.update();
        assert!(
            (app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .stops
                .iter()
                .find(|stop| stop.id == selected)
                .unwrap()
                .position
                - 0.25)
                .abs()
                < 0.01
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::Y,
            UiModifiers {
                control: true,
                ..Default::default()
            },
        )];
        app.update();
        assert!(
            (app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .stops
                .iter()
                .find(|stop| stop.id == selected)
                .unwrap()
                .position
                - 0.9)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn color_ramp_click_and_keyboard_edit_interpolation_and_rgba() {
        let mut app = App::new();
        let rect = UiRect::new(10.0, 20.0, 360.0, 194.0);
        let (window, entity) = spawn_color_ramp_widget(&mut app, rect);
        let mut spacing = Theme::default().spacing.color_ramp;
        spacing.button_step = 58.0;
        spacing.button_width = 52.0;
        spacing.track_x = 18.0;
        spacing.track_y = 42.0;
        spacing.track_width_inset = 36.0;
        spacing.channel_x = 54.0;
        spacing.channel_y = 90.0;
        spacing.channel_width_inset = 72.0;
        app.world_mut().resource_mut::<Theme>().spacing.color_ramp = spacing;
        let click = |position, click_count| UiRoutedEvent::Click {
            window,
            target: entity,
            button: UiPointerButton::Primary,
            click_count,
            position,
        };
        let add_x = rect.x + rect.width - spacing.button_right_inset;
        let remove_x = add_x + spacing.button_step;
        let interpolation_x = remove_x + spacing.button_step;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(
            [interpolation_x + spacing.button_width * 0.5, rect.y + 10.0],
            1,
        )];
        app.update();
        assert_eq!(
            app.world().get::<ColorRamp>(entity).unwrap().interpolation,
            Interpolation::Smooth
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(
            [add_x + spacing.button_width * 0.5, rect.y + 10.0],
            1,
        )];
        app.update();
        assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 3);
        let track = color_ramp_track_rect(rect, spacing);
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![click([track.x + track.width * 0.8, track.y + 8.0], 2)];
        app.update();
        assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 4);

        let channel_row = color_ramp_channel_rect(rect, ColorChannel::Red.index(), spacing);
        let start = [channel_row.x + channel_row.width * 0.2, channel_row.y + 5.0];
        let end = [channel_row.x + channel_row.width * 0.7, start[1]];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: end,
                delta: [end[0] - start[0], 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: end,
                cancelled: false,
            },
        ];
        app.update();
        let ramp = app.world().get::<ColorRamp>(entity).unwrap();
        assert!((ramp.selected_stop().unwrap().color.0 - 0.7).abs() < 0.01);
        assert!(
            app.world()
                .get::<ColorRampEditorState>(entity)
                .unwrap()
                .editing_color
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: entity,
            key: UiKeyCode::ArrowRight,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert!(
            (app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .selected_stop()
                .unwrap()
                .color
                .0
                - 0.71)
                .abs()
                < 0.01
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![click([rect.x + rect.width - 80.0, rect.y + 10.0], 1)];
        app.update();
        assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 3);
        let key = |key| UiRoutedEvent::Key {
            window,
            target: entity,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
        app.update();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
        app.update();
        assert_eq!(
            app.world().get::<ColorRamp>(entity).unwrap().interpolation,
            Interpolation::Constant
        );
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(item.primitive, Primitive::TextWrapped { .. }))
        );
    }

    #[test]
    fn color_ramp_keyboard_routes_stop_position_and_channel_editing() {
        let mut app = App::new();
        let (window, entity) =
            spawn_color_ramp_widget(&mut app, UiRect::new(0.0, 0.0, 360.0, 194.0));
        let send_key = |app: &mut App, key, modifiers| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: entity,
                key,
                pressed: true,
                repeat: false,
                modifiers,
            }];
            app.update();
        };

        send_key(&mut app, UiKeyCode::A, UiModifiers::default());
        let added_id = app
            .world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected
            .unwrap();
        assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 3);
        let start = app
            .world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .position;
        send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
        let normal_step = app
            .world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .position;
        assert!((normal_step - start - 0.01).abs() < 0.001);
        send_key(
            &mut app,
            UiKeyCode::ArrowRight,
            UiModifiers {
                shift: true,
                ..UiModifiers::default()
            },
        );
        let fine_step = app
            .world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .position;
        assert!((fine_step - normal_step - 0.001).abs() < 0.0002);
        send_key(
            &mut app,
            UiKeyCode::ArrowRight,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        let coarse_step = app
            .world()
            .get::<ColorRamp>(entity)
            .unwrap()
            .selected_stop()
            .unwrap()
            .position;
        assert!((coarse_step - fine_step - 0.1).abs() < 0.001);
        send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .stops
                .iter()
                .find(|stop| stop.id == added_id)
                .unwrap()
                .position,
            0.0
        );
        send_key(&mut app, UiKeyCode::End, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .stops
                .iter()
                .find(|stop| stop.id == added_id)
                .unwrap()
                .position,
            1.0
        );
        send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(
            app.world().get::<ColorRamp>(entity).unwrap().selected,
            Some(1)
        );
        send_key(&mut app, UiKeyCode::ArrowUp, UiModifiers::default());
        assert_eq!(
            app.world().get::<ColorRamp>(entity).unwrap().selected,
            Some(added_id)
        );
        send_key(&mut app, UiKeyCode::Delete, UiModifiers::default());
        assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 2);
        send_key(&mut app, UiKeyCode::A, UiModifiers::default());
        send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
        assert_eq!(app.world().get::<ColorRamp>(entity).unwrap().stops.len(), 2);

        app.world_mut()
            .get_mut::<ColorRamp>(entity)
            .unwrap()
            .select_stop(1);
        send_key(&mut app, UiKeyCode::C, UiModifiers::default());
        send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ColorRampEditorState>(entity)
                .unwrap()
                .active_channel,
            ColorChannel::Green
        );
        send_key(
            &mut app,
            UiKeyCode::ArrowRight,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert!(
            (app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .selected_stop()
                .unwrap()
                .color
                .1
                - 0.1)
                .abs()
                < 0.001
        );
        send_key(&mut app, UiKeyCode::End, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .selected_stop()
                .unwrap()
                .color
                .1,
            1.0
        );
        send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .selected_stop()
                .unwrap()
                .color
                .1,
            0.0
        );
        send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        assert!(
            !app.world()
                .get::<ColorRampEditorState>(entity)
                .unwrap()
                .editing_color
        );
    }

    #[test]
    fn color_ramp_painter_emits_gradient_and_channel_gpu_primitives() {
        let mut app = App::new();
        let (window, entity) =
            spawn_color_ramp_widget(&mut app, UiRect::new(0.0, 0.0, 360.0, 194.0));
        assert_ne!(window, Entity::PLACEHOLDER);
        app.update();
        let items = app.world().resource::<DisplayList>().widget_items();
        assert!(items.len() >= 140);
        assert!(
            items
                .iter()
                .filter(|item| matches!(
                    item.primitive,
                    Primitive::RoundedRect { rect, .. } if rect.height == 25.0
                ))
                .count()
                >= 48
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Line { .. }))
        );
        app.world_mut().entity_mut(entity).despawn();
    }

    #[test]
    fn color_ramp_cancelled_drag_restores_position_without_commit() {
        let mut app = App::new();
        let rect = UiRect::new(0.0, 0.0, 360.0, 194.0);
        let (window, entity) = spawn_color_ramp_widget(&mut app, rect);
        let track = color_ramp_track_rect(rect, Theme::default().spacing.color_ramp);
        let start = [track.x, track.y + track.height + 10.0];
        let end = [track.x + track.width * 0.7, start[1]];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: end,
                delta: [end[0] - start[0], 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerCancel {
                window,
                target: entity,
            },
        ];
        app.update();
        assert_eq!(
            app.world()
                .get::<ColorRamp>(entity)
                .unwrap()
                .stops
                .iter()
                .find(|stop| stop.id == 1)
                .unwrap()
                .position,
            0.0
        );
        assert!(
            !app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .any(|event| matches!(event, WidgetEvent::ColorRampCommitted { .. }))
        );
    }

    #[test]
    fn hsv_wheel_drag_is_one_undoable_gesture_and_value_alpha_tracks_route() {
        let mut app = App::new();
        let rect = UiRect::new(10.0, 20.0, 448.0, 236.0);
        let (window, entity) = spawn_hsv_color_widget(&mut app, rect);
        let mut spacing = Theme::default().spacing.hsv;
        spacing.wheel_center_x = 96.0;
        spacing.wheel_center_y = 108.0;
        spacing.wheel_outer_radius = 62.0;
        spacing.wheel_inner_radius = 50.0;
        app.world_mut().resource_mut::<Theme>().spacing.hsv = spacing;
        let (center, _, _) = hsv_wheel_geometry(rect, spacing);
        let at_green = [center[0] + 20.0, center[1] + 34.0];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: at_green,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: at_green,
                cancelled: false,
            },
        ];
        app.update();
        let editor = app.world().get::<HsvColorEditor>(entity).unwrap();
        assert!((editor.hue_degrees - 60.0).abs() < 1.0);
        assert!((editor.saturation - 1556.0_f32.sqrt() / 50.0).abs() < 0.03);
        assert!(editor.color().0 > editor.color().2);
        assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(|event| matches!(event, WidgetEvent::HsvColorCommitted { target, .. } if *target == entity))
                .count(),
            1,
            "a drag produces one committed edit"
        );
        app.world_mut().resource_mut::<WidgetEventQueue>().take();

        let key = |key| UiRoutedEvent::Key {
            window,
            target: entity,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..Default::default()
            },
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Z)];
        app.update();
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .hue_degrees)
                .abs()
                < 0.01
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Y)];
        app.update();
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .hue_degrees
                - 60.0)
                .abs()
                < 1.0
        );

        for (kind, track, value) in [
            (HsvColorDragKind::Value, hsv_value_rect(rect, spacing), 0.3),
            (HsvColorDragKind::Alpha, hsv_alpha_rect(rect, spacing), 0.65),
        ] {
            let from = [track.x + track.width * 0.8, track.y + 5.0];
            let to = [track.x + track.width * value, from[1]];
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
                UiRoutedEvent::PointerDown {
                    window,
                    target: entity,
                    button: UiPointerButton::Primary,
                    position: from,
                },
                UiRoutedEvent::PointerMoved {
                    window,
                    target: entity,
                    position: to,
                    delta: [to[0] - from[0], 0.0],
                    dragging: true,
                },
                UiRoutedEvent::PointerUp {
                    window,
                    target: entity,
                    button: UiPointerButton::Primary,
                    position: to,
                    cancelled: false,
                },
            ];
            app.update();
            let editor = app.world().get::<HsvColorEditor>(entity).unwrap();
            let actual = match kind {
                HsvColorDragKind::Value => editor.value,
                HsvColorDragKind::Alpha => editor.alpha,
                HsvColorDragKind::Wheel => unreachable!(),
            };
            assert!((actual - value).abs() < 0.01);
        }
    }

    #[test]
    fn hsv_editor_routes_hex_and_rgb_numeric_entry_and_action_updates() {
        let mut app = App::new();
        let rect = UiRect::new(0.0, 0.0, 448.0, 236.0);
        let (window, entity) = spawn_hsv_color_widget(&mut app, rect);
        let mut spacing = Theme::default().spacing.hsv;
        spacing.controls_x = 184.0;
        spacing.field_cell_width = 70.0;
        spacing.field_gap = 10.0;
        spacing.field_first_row_y = 118.0;
        spacing.field_second_row_y = 150.0;
        spacing.field_hex_y = 184.0;
        app.world_mut().resource_mut::<Theme>().spacing.hsv = spacing;
        let click_field = |field| {
            let field_rect = hsv_field_rect(rect, field, spacing);
            UiRoutedEvent::Click {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                click_count: 1,
                position: [field_rect.x + 4.0, field_rect.y + 4.0],
            }
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            click_field(HsvColorField::Hex),
            UiRoutedEvent::Text {
                window,
                target: entity,
                text: "#00FF0080".into(),
            },
            UiRoutedEvent::Key {
                window,
                target: entity,
                key: UiKeyCode::Enter,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ];
        app.update();
        let editor = app.world().get::<HsvColorEditor>(entity).unwrap();
        assert_eq!(editor.hex(), "#00FF0080");
        assert!((editor.color().1 - 1.0).abs() < 0.001);
        assert!((editor.alpha - 128.0 / 255.0).abs() < 0.001);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            click_field(HsvColorField::Red),
            UiRoutedEvent::Text {
                window,
                target: entity,
                text: "128".into(),
            },
            UiRoutedEvent::Key {
                window,
                target: entity,
                key: UiKeyCode::Enter,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ];
        app.update();
        assert!(
            (app.world().get::<HsvColorEditor>(entity).unwrap().color().0 - 128.0 / 255.0).abs()
                < 0.001
        );

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                entity,
                WidgetActionKind::HsvColorSetHex("#663399CC".into()),
            ));
        app.update();
        assert_eq!(
            app.world().get::<HsvColorEditor>(entity).unwrap().hex(),
            "#663399CC"
        );
    }

    #[test]
    fn hsv_editor_routes_navigation_modifiers_edit_commit_and_cancel() {
        let mut app = App::new();
        let (window, entity) =
            spawn_hsv_color_widget(&mut app, UiRect::new(0.0, 0.0, 448.0, 236.0));
        let route_key = |app: &mut App, key, modifiers| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: entity,
                key,
                pressed: true,
                repeat: false,
                modifiers,
            }];
            app.update();
        };

        route_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .hue_degrees
                - 1.0)
                .abs()
                < 0.01
        );
        route_key(
            &mut app,
            UiKeyCode::ArrowRight,
            UiModifiers {
                shift: true,
                ..UiModifiers::default()
            },
        );
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .hue_degrees
                - 1.1)
                .abs()
                < 0.01
        );
        route_key(
            &mut app,
            UiKeyCode::ArrowRight,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .hue_degrees
                - 11.1)
                .abs()
                < 0.01
        );

        route_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<HsvColorEditorState>(entity)
                .unwrap()
                .active_field,
            HsvColorField::Saturation
        );
        route_key(&mut app, UiKeyCode::End, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .saturation,
            1.0
        );

        route_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        assert!(
            app.world()
                .get::<HsvColorEditorState>(entity)
                .unwrap()
                .editing
        );
        route_key(&mut app, UiKeyCode::Delete, UiModifiers::default());
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: entity,
            text: "35".to_owned(),
        }];
        app.update();
        route_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .saturation
                - 0.35)
                .abs()
                < 0.01
        );
        assert!(
            !app.world()
                .get::<HsvColorEditorState>(entity)
                .unwrap()
                .editing
        );

        route_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        route_key(&mut app, UiKeyCode::Delete, UiModifiers::default());
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: entity,
            text: "90".to_owned(),
        }];
        app.update();
        route_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
        assert!(
            (app.world()
                .get::<HsvColorEditor>(entity)
                .unwrap()
                .saturation
                - 0.35)
                .abs()
                < 0.01
        );
        let state = app.world().get::<HsvColorEditorState>(entity).unwrap();
        assert!(!state.editing);
        assert!(state.edit_buffer.is_empty());
    }

    #[test]
    fn hsv_editor_paints_a_bounded_gpu_wheel_preview_and_entry_fields() {
        let mut app = App::new();
        let (_, entity) = spawn_hsv_color_widget(&mut app, UiRect::new(0.0, 0.0, 448.0, 236.0));
        app.update();
        let items = app.world().resource::<DisplayList>().widget_items();
        assert!((100..140).contains(&items.len()));
        assert_eq!(
            items
                .iter()
                .filter(|item| matches!(item.primitive, Primitive::HsvWheel { .. }))
                .count(),
            1,
            "the wheel must be a single GPU-shaded display item"
        );
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::HsvWheel {
                hue_degrees: 0.0,
                saturation: 1.0,
                value: 1.0,
                ..
            }
        )));
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, .. } if text == "#FF0000FF"
        )));
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect {
                color: Rgba(1.0, 0.0, 0.0, 1.0),
                ..
            }
        )));
        app.world_mut().entity_mut(entity).despawn();
    }

    #[test]
    fn typed_editor_spacing_drives_hsv_curve_and_color_ramp_paint() {
        let mut theme = Theme::default();
        theme.spacing.hsv.wheel_center_x = 96.0;
        theme.spacing.hsv.wheel_center_y = 108.0;
        theme.spacing.hsv.wheel_outer_radius = 62.0;
        theme.spacing.hsv.controls_x = 188.0;
        theme.spacing.curve.interpolation_status_width = 124.0;
        theme.spacing.color_ramp.track_x = 17.0;
        theme.spacing.color_ramp.track_y = 44.0;
        theme.spacing.color_ramp.track_width_inset = 38.0;
        theme.spacing.color_ramp.track_height = 21.0;
        theme.spacing.color_ramp.channel_x = 55.0;
        theme.spacing.color_ramp.channel_y = 91.0;
        theme.spacing.color_ramp.channel_width_inset = 70.0;
        theme.spacing.color_ramp.channel_height = 12.0;
        theme.spacing.node_graph.header_hint_width = 244.0;

        let rect = Rect::new(10.0, 20.0, 448.0, 236.0);
        let clip = ClipRect::from(rect);
        let mut hsv_items = Vec::new();
        paint_hsv_color_editor(
            HsvColorPaint {
                editor: HsvColorEditor::default(),
                state: HsvColorEditorState::default(),
                label: "HSV",
                rect,
                clip,
                z: 1,
                theme: &theme,
            },
            &mut hsv_items,
        );
        assert!(hsv_items.iter().any(|item| matches!(
            item.primitive,
            Primitive::HsvWheel { rect: wheel, .. }
                if wheel.x == rect.x + 96.0 - 62.0
                    && wheel.y == rect.y + 108.0 - 62.0
                    && wheel.width == 124.0
                    && wheel.height == 124.0
        )));
        assert!(hsv_items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, origin, .. }
                if text == "Value" && *origin == [rect.x + 188.0, rect.y + 27.0]
        )));

        let curve = CurveEditor::default();
        let mut curve_items = Vec::new();
        paint_curve_editor(
            CurveEditorPaint {
                editor: &curve,
                label: "Curve",
                rect,
                clip,
                z: 1,
                theme: &theme,
            },
            &mut curve_items,
        );
        let interpolation = format!("{:?}", curve.interpolation);
        assert!(curve_items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, origin, .. }
                if text == &interpolation
                    && (origin[0] - (rect.x + rect.width - 124.0 - theme.spacing.editor.padding_x)).abs() < 0.01
        )));

        let ramp = ColorRamp::default();
        let mut ramp_items = Vec::new();
        paint_color_ramp(
            ColorRampPaint {
                ramp: &ramp,
                state: ColorRampEditorState::default(),
                label: "Ramp",
                rect,
                clip,
                z: 1,
                theme: &theme,
            },
            &mut ramp_items,
        );
        assert!(ramp_items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect: track, color, .. }
                if color == theme.background
                    && track.x == rect.x + 17.0
                    && track.y == rect.y + 44.0
                    && track.width == rect.width - 38.0
                    && track.height == 21.0
        )));
        assert!(ramp_items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect: row, .. }
                if row.x == rect.x + 55.0
                    && row.y == rect.y + 91.0
                    && row.width == (rect.width - 70.0) / 24.0 + 0.5
                    && row.height == 12.0
        )));

        let graph = NodeGraph::default();
        let mut graph_items = Vec::new();
        paint_node_graph(
            NodeGraphPaint {
                graph: &graph,
                state: NodeGraphEditorState::default(),
                output_label_widths: HashMap::new(),
                canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
                label: "Graph",
                rect,
                clip,
                z: 1,
                theme: &theme,
                font: ui_default_text_font(theme.font_size),
            },
            &mut graph_items,
        );
        let header_text = graph_items
            .iter()
            .filter_map(|item| match &item.primitive {
                Primitive::TextWrapped {
                    text,
                    origin,
                    max_width,
                    ..
                } if item.z == 3 => Some((text.as_str(), origin[0], *max_width)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(header_text.len(), 2);
        assert!(header_text.iter().any(|(text, _, _)| *text == "Graph"));
        assert!(
            header_text
                .iter()
                .any(|(text, _, _)| *text == "0 nodes · 0 links")
        );
        assert!(!header_text.iter().any(|(text, _, _)| text.contains("Drag")));
        let title = header_text
            .iter()
            .find(|(text, _, _)| *text == "Graph")
            .unwrap();
        let summary = header_text
            .iter()
            .find(|(text, _, _)| *text == "0 nodes · 0 links")
            .unwrap();
        let title_right = title.1 + title.2;
        let summary_left = summary.1;
        assert!(summary_left - title_right >= 8.0);
    }

    #[test]
    fn tooltip_painter_uses_typed_bounds_padding_margin_and_anchor_gap() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        world.spawn(UiLayoutRoot::new(
            window,
            Rect::new(0.0, 0.0, 500.0, 300.0),
            1.0,
        ));
        let trigger = world
            .spawn(UiLayoutResult {
                window,
                rect: Rect::new(100.0, 40.0, 24.0, 20.0),
                clip: None,
                z: 4,
                content_size: [24.0, 20.0],
                dpi_scale: 1.0,
            })
            .id();
        let owner = world
            .spawn(Tooltip::new("A tooltip with a long caption"))
            .id();
        world.insert_resource(WidgetTooltips::default());
        world.resource_mut::<WidgetTooltips>().0.insert(
            window,
            TooltipHover {
                trigger,
                owner,
                elapsed_seconds: 1.0,
            },
        );
        let mut theme = Theme::default();
        theme.spacing.tooltip.padding_x = 10.0;
        theme.spacing.tooltip.padding_y = 6.0;
        theme.spacing.tooltip.maximum_width = 72.0;
        theme.spacing.tooltip.viewport_margin = 8.0;
        theme.spacing.tooltip.anchor_gap = 9.0;
        let tooltip = world.get::<Tooltip>(owner).unwrap().clone();
        let mut items = Vec::new();
        paint_tooltip_overlay(&world, owner, &tooltip, window, &theme, &mut items);

        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, max_width, origin, .. }
                if text == &tooltip.text && *max_width == 72.0 && *origin == [110.0, 75.0]
        )));
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect, .. }
                if rect.x == 100.0 && rect.y == 69.0
                    && rect.width == 92.0 && rect.height > 0.0
        )));
    }

    #[test]
    fn popup_widget_painters_emit_theme_dependent_shadow_primitives() {
        let mut colors_by_theme = Vec::new();
        for kind in [bui_style::ThemeKind::Dark, bui_style::ThemeKind::Light] {
            let theme = Theme::new(kind);
            let expected_color = theme.color(ThemeColorToken::PopupShadow);
            let mut app = App::new();
            app.add_plugins(WidgetPlugin);
            *app.world_mut().resource_mut::<Theme>() = theme.clone();

            let window = app.world_mut().spawn_empty().id();
            let viewport = Rect::new(0.0, 0.0, 500.0, 360.0);
            app.world_mut()
                .spawn(UiLayoutRoot::new(window, viewport, 1.0));

            let mut menu = MenuControl::new(
                "File",
                crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
                MenuDisplay::Dropdown,
            );
            menu.open(None);
            menu.popup.bounds = Rect::new(40.0, 60.0, 120.0, 72.0);
            menu.popup.open = true;
            app.world_mut().spawn((
                UiNode,
                UiLayoutResult {
                    window,
                    rect: Rect::new(40.0, 24.0, 120.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
                menu,
            ));

            let mut pie_menu = MenuControl::new(
                "Radial",
                crate::MenuTree::new(vec![crate::MenuEntry::command("one", "One")]),
                MenuDisplay::Pie,
            );
            pie_menu.open(None);
            pie_menu.popup.bounds = Rect::new(220.0, 48.0, 112.0, 112.0);
            pie_menu.popup.open = true;
            app.world_mut().spawn((
                UiNode,
                UiLayoutResult {
                    window,
                    rect: Rect::new(260.0, 8.0, 32.0, 24.0),
                    clip: None,
                    z: 2,
                    content_size: [32.0, 24.0],
                    dpi_scale: 1.0,
                },
                pie_menu,
            ));

            let dropdown_rect = Rect::new(40.0, 160.0, 160.0, 26.0);
            let dropdown_popup_rect = Rect::new(40.0, 186.0, 160.0, 56.0);
            let mut popup = crate::PopupState::new(dropdown_rect);
            popup.open = true;
            popup.bounds = dropdown_popup_rect;
            app.world_mut().spawn((
                UiNode,
                UiLayoutResult {
                    window,
                    rect: dropdown_rect,
                    clip: None,
                    z: 3,
                    content_size: [dropdown_rect.width, dropdown_rect.height],
                    dpi_scale: 1.0,
                },
                Dropdown {
                    open: true,
                    ..Dropdown::new(["First", "Second"], Some(0))
                },
                DropdownPopupState {
                    popup,
                    viewport,
                    first_visible: 0,
                    visible_count: 2,
                },
            ));

            let trigger = app
                .world_mut()
                .spawn(UiLayoutResult {
                    window,
                    rect: Rect::new(320.0, 120.0, 24.0, 24.0),
                    clip: None,
                    z: 4,
                    content_size: [24.0, 24.0],
                    dpi_scale: 1.0,
                })
                .id();
            let tooltip_owner = app
                .world_mut()
                .spawn((
                    UiNode,
                    Tooltip {
                        visible: true,
                        ..Tooltip::new("Themed popup tooltip")
                    },
                    UiLayoutResult {
                        window,
                        rect: Rect::new(320.0, 120.0, 24.0, 24.0),
                        clip: None,
                        z: 5,
                        content_size: [24.0, 24.0],
                        dpi_scale: 1.0,
                    },
                ))
                .id();
            app.world_mut().resource_mut::<WidgetTooltips>().0.insert(
                window,
                TooltipHover {
                    trigger,
                    owner: tooltip_owner,
                    elapsed_seconds: 1.0,
                },
            );

            collect_widget_paint(app.world_mut());
            let items = app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .to_vec();
            let shadows: Vec<_> = items
                .iter()
                .filter_map(|item| match &item.primitive {
                    Primitive::Shadow {
                        rect,
                        shadow,
                        offset,
                        blur,
                        spread,
                        ..
                    } => Some((*rect, *shadow, *offset, *blur, *spread, item.clip)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                shadows.len(),
                4,
                "menu, pie, dropdown, and tooltip each paint a shadow: {shadows:?}"
            );
            assert!(
                shadows
                    .iter()
                    .all(|(_, color, ..)| *color == expected_color)
            );
            assert!(shadows.iter().all(|(_, _, offset, blur, spread, _)| {
                let layer = theme.effects.popup_shadow.shadows[0];
                *offset == [layer.x_offset, layer.y_offset]
                    && *blur == layer.blur_radius
                    && *spread == layer.spread_radius
            }));
            assert!(
                shadows
                    .iter()
                    .all(|(_, _, _, _, _, clip)| *clip == ClipRect::from(viewport))
            );
            colors_by_theme.push(expected_color);
        }
        assert_ne!(colors_by_theme[0], colors_by_theme[1]);
    }

    #[test]
    fn menu_transition_uses_theme_motion_and_snaps_without_delaying_hitboxes() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut()
            .init_resource::<bui_style::UiReducedMotion>();
        app.world_mut().resource_mut::<Theme>().motion.menu_open =
            std::time::Duration::from_millis(100);
        let window = app.world_mut().spawn_empty().id();
        let viewport = Rect::new(0.0, 0.0, 500.0, 360.0);
        app.world_mut()
            .spawn(UiLayoutRoot::new(window, viewport, 1.0));

        let mut menu = MenuControl::new(
            "Menu",
            crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
            MenuDisplay::Dropdown,
        );
        menu.width = 220.0;
        let anchor = Rect::new(20.0, 10.0, 80.0, 20.0);
        let owner = app
            .world_mut()
            .spawn((
                UiNode,
                menu,
                UiLayoutResult {
                    window,
                    rect: anchor,
                    clip: None,
                    z: 1,
                    content_size: [anchor.width, anchor.height],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        let row = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                MenuItemControl {
                    owner,
                    id: "open".to_owned(),
                    label: "Open".to_owned(),
                    parent_path: Vec::new(),
                    index: 0,
                    enabled: true,
                    separator: false,
                    has_submenu: false,
                    shortcut: None,
                },
                UiLayoutResult {
                    window,
                    rect: Rect::new(0.0, 0.0, 1.0, 1.0),
                    clip: None,
                    z: 0,
                    content_size: [1.0, 1.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<MenuControl>(owner)
            .unwrap()
            .items
            .push(row);

        sync_menu_popups(app.world_mut());
        app.world_mut()
            .get_mut::<MenuControl>(owner)
            .unwrap()
            .open(None);
        sync_menu_popups(app.world_mut());
        advance_menu_animations_by(app.world_mut(), 0.05);

        let popup = app.world().get::<MenuControl>(owner).unwrap().popup.bounds;
        let owner_hit_rect = app.world().get::<UiHitBox>(owner).unwrap().rect;
        let row_rect = app.world().get::<UiHitBox>(row).unwrap().rect;
        assert_eq!(
            owner_hit_rect, popup,
            "open hit testing uses final popup bounds"
        );
        assert_eq!(
            row_rect,
            app.world().get::<UiLayoutResult>(row).unwrap().rect
        );
        let state = app
            .world()
            .resource::<WidgetMenuAnimations>()
            .0
            .get(&owner)
            .copied()
            .unwrap();
        assert!((state.progress - 0.5).abs() < 0.001);

        collect_widget_paint(app.world_mut());
        let progress =
            menu_animation_progress(app.world(), owner, app.world().resource::<Theme>()).unwrap();
        let animated_popup = interpolate_rect(anchor, popup, progress);
        let row_paint = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .find(|item| {
                matches!(
                    &item.primitive,
                    Primitive::RoundedRect { rect, .. }
                        if rect.x == row_rect.x
                            && rect.y == row_rect.y
                            && rect.width == row_rect.width
                            && rect.height == row_rect.height
                )
            })
            .unwrap();
        assert!(row_paint.clip.y >= animated_popup.y);
        assert!(
            row_paint.clip.y + row_paint.clip.height <= animated_popup.y + animated_popup.height
        );
        assert!(
            row_paint.clip.height < row_rect.height,
            "rows are clipped during opening"
        );

        app.world_mut().increment_change_tick();
        app.world_mut()
            .get_mut::<MenuControl>(owner)
            .unwrap()
            .close();
        sync_menu_popups(app.world_mut());
        assert_eq!(app.world().get::<UiHitBox>(owner).unwrap().rect, anchor);
        assert!(!app.world().get::<UiVisible>(row).unwrap().0);
        advance_menu_animations_by(app.world_mut(), 0.025);
        let state = app
            .world()
            .resource::<WidgetMenuAnimations>()
            .0
            .get(&owner)
            .copied()
            .unwrap();
        assert!((state.progress - 0.25).abs() < 0.001);
        collect_widget_paint(app.world_mut());
        assert!(
            !app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(
                    |item| matches!(&item.primitive, Primitive::Text { text, .. } if text == "Open")
                )
        );

        app.world_mut().increment_change_tick();
        app.world_mut()
            .resource_mut::<bui_style::UiReducedMotion>()
            .0 = true;
        advance_menu_animations_by(app.world_mut(), 0.0);
        assert!(
            !app.world()
                .resource::<WidgetMenuAnimations>()
                .0
                .contains_key(&owner)
        );
        collect_widget_paint(app.world_mut());
        assert!(
            !app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(&item.primitive, Primitive::Shadow { .. }))
        );
    }

    #[test]
    fn checkbox_paint_sets_each_traversed_control_once_and_commits_one_record() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let first = app
            .world_mut()
            .spawn((
                UiNode,
                UiHitBox {
                    rect: UiRect::new(0.0, 0.0, 20.0, 20.0),
                    clip: None,
                    z_index: 0,
                    window: Some(window),
                },
                Checkbox::new("first", false),
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                UiNode,
                UiHitBox {
                    rect: UiRect::new(20.0, 0.0, 20.0, 20.0),
                    clip: None,
                    z_index: 0,
                    window: Some(window),
                },
                Checkbox::new("second", false),
            ))
            .id();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [5.0, 5.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [30.0, 5.0],
                delta: [25.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [30.0, 5.0],
                delta: [0.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [30.0, 5.0],
                cancelled: false,
            },
            UiRoutedEvent::Click {
                window,
                target: first,
                button: UiPointerButton::Primary,
                click_count: 1,
                position: [5.0, 5.0],
            },
        ];
        app.update();
        assert!(app.world().get::<Checkbox>(first).unwrap().is_checked());
        assert!(app.world().get::<Checkbox>(second).unwrap().is_checked());
        let events = app.world_mut().resource_mut::<WidgetEventQueue>().take();
        let Some(WidgetEvent::CheckboxPaintCommitted { changes }) = events
            .iter()
            .find(|event| matches!(event, WidgetEvent::CheckboxPaintCommitted { .. }))
        else {
            panic!("one brush transaction should be emitted");
        };
        assert_eq!(changes.len(), 2);
        assert_ne!(changes[0].entity, changes[1].entity);
    }

    #[test]
    fn checkbox_brush_cancel_restores_and_commit_undoes_once_with_disabled_keyboard_guards() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let spawn_checkbox = |world: &mut World, label: &str, x: f32| {
            world
                .spawn((
                    UiNode,
                    Checkbox::new(label, false),
                    UiHitBox {
                        window: Some(window),
                        rect: UiRect::new(x, 0.0, 20.0, 20.0),
                        clip: None,
                        z_index: 0,
                    },
                ))
                .id()
        };
        let first = spawn_checkbox(app.world_mut(), "first", 0.0);
        let second = spawn_checkbox(app.world_mut(), "second", 20.0);
        let mut locked_checkbox = Checkbox::new("locked", false);
        locked_checkbox.read_only = true;
        let locked_checkbox = app.world_mut().spawn(locked_checkbox).id();
        let mut disabled_button = Button::new("disabled");
        disabled_button.enabled = false;
        let disabled_button = app.world_mut().spawn(disabled_button).id();
        let mut read_only_text = TextField::new("locked text");
        read_only_text.read_only = true;
        let read_only_text = app.world_mut().spawn(read_only_text).id();

        let brush_events = |cancelled| {
            vec![
                UiRoutedEvent::PointerDown {
                    window,
                    target: first,
                    button: UiPointerButton::Primary,
                    position: [5.0, 5.0],
                },
                UiRoutedEvent::PointerMoved {
                    window,
                    target: first,
                    position: [30.0, 5.0],
                    delta: [25.0, 0.0],
                    dragging: true,
                },
                UiRoutedEvent::PointerUp {
                    window,
                    target: first,
                    button: UiPointerButton::Primary,
                    position: [30.0, 5.0],
                    cancelled,
                },
            ]
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = brush_events(true);
        app.update();
        assert!(!app.world().get::<Checkbox>(first).unwrap().is_checked());
        assert!(!app.world().get::<Checkbox>(second).unwrap().is_checked());
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::CheckboxPaintCancelled { changes } if changes.len() == 2)
        ));
        app.world_mut().resource_mut::<WidgetEventQueue>().take();

        app.world_mut().resource_mut::<UiRoutedEvents>().events = brush_events(false);
        app.update();
        assert!(app.world().get::<Checkbox>(first).unwrap().is_checked());
        assert!(app.world().get::<Checkbox>(second).unwrap().is_checked());
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(|event| matches!(event, WidgetEvent::CheckboxPaintCommitted { .. }))
                .count(),
            1
        );

        let key = |target, key, modifiers| UiRoutedEvent::Key {
            window,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let ctrl = UiModifiers {
            control: true,
            ..UiModifiers::default()
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(first, UiKeyCode::Z, ctrl)];
        app.update();
        assert!(!app.world().get::<Checkbox>(first).unwrap().is_checked());
        assert!(!app.world().get::<Checkbox>(second).unwrap().is_checked());
        assert_eq!(app.world().resource::<bui_core::UiHistory>().redo_len(), 1);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            first,
            UiKeyCode::Z,
            UiModifiers {
                shift: true,
                ..ctrl
            },
        )];
        app.update();
        assert!(app.world().get::<Checkbox>(first).unwrap().is_checked());
        assert!(app.world().get::<Checkbox>(second).unwrap().is_checked());

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            key(disabled_button, UiKeyCode::Space, UiModifiers::default()),
            key(disabled_button, UiKeyCode::Enter, UiModifiers::default()),
            key(locked_checkbox, UiKeyCode::Space, UiModifiers::default()),
            key(read_only_text, UiKeyCode::Backspace, UiModifiers::default()),
            UiRoutedEvent::Text {
                window,
                target: read_only_text,
                text: " edit".to_owned(),
            },
        ];
        app.update();
        assert_eq!(
            app.world()
                .get::<Button>(disabled_button)
                .unwrap()
                .activations,
            0
        );
        assert!(
            !app.world()
                .get::<Checkbox>(locked_checkbox)
                .unwrap()
                .is_checked()
        );
        assert_eq!(
            app.world().get::<TextField>(read_only_text).unwrap().text,
            "locked text"
        );
        assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(|event| {
            matches!(event, WidgetEvent::Activated { target } if *target == disabled_button)
                || matches!(event, WidgetEvent::TextChanged { target, .. } if *target == read_only_text)
        }));
    }

    #[test]
    fn numeric_scrub_commits_and_requests_cursor_capture_once() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                NumericField::new(1.0, None, None, 0.1, NumericFieldKind::Float).unwrap(),
            ))
            .id();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [5.0, 5.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [10.0, 5.0],
                delta: [5.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [10.0, 5.0],
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(app.world().get::<NumericField>(target).unwrap().value, 1.5);
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<CursorCaptureRequestQueue>()
            .drain()
            .collect();
        assert_eq!(
            requests,
            [
                CursorCaptureRequest::Begin {
                    window,
                    target,
                    hide_cursor: true,
                    continuous: true
                },
                CursorCaptureRequest::End {
                    window,
                    target,
                    restore_position: true
                },
            ]
        );
    }

    #[test]
    fn slider_routes_drag_keyboard_wheel_cancellation_and_numeric_actions() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let rect = UiRect::new(0.0, 0.0, 260.0, 32.0);
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                UiVisible(true),
                UiEnabled(true),
                UiFocusable,
                Label::new("Opacity"),
                Slider::new(0.2, 0.0, 1.0, 0.1).unwrap(),
                UiHitBox {
                    window: Some(window),
                    rect,
                    clip: None,
                    z_index: 1,
                },
                UiLayoutResult {
                    window,
                    rect,
                    clip: None,
                    z: 1,
                    content_size: [rect.width, rect.height],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.update();
        let initial_paint = app.world().resource::<DisplayList>().widget_items();
        assert!(
            initial_paint
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Ellipse { .. }))
        );
        assert!(
            initial_paint
                .iter()
                .any(|item| matches!(item.primitive, Primitive::RoundedRect { .. }))
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [130.0, 24.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [220.0, 24.0],
                delta: [90.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [220.0, 24.0],
                cancelled: false,
            },
        ];
        app.update();
        app.world_mut().resource_mut::<UiRoutedEvents>().clear();
        let dragged_value = app.world().get::<Slider>(target).unwrap().value;
        assert!(dragged_value > 0.8 && dragged_value <= 1.0);
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::NumericCommitted { target: event_target, before, after }
                if *event_target == target && before == &[0.2] && after == &[dragged_value])
        ));
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        app.world_mut().resource_mut::<WidgetEventQueue>().take();

        let key = |key, modifiers| UiRoutedEvent::Key {
            window,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.2);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                shift: true,
                ..UiModifiers::default()
            },
        )];
        app.update();
        assert_eq!(
            app.world().get::<Slider>(target).unwrap().value,
            dragged_value
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::Home, UiModifiers::default())];
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.0);
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::ArrowRight, UiModifiers::default())];
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.1);
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::End, UiModifiers::default())];
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, 1.0);

        app.world_mut().resource_mut::<UiRoutedEvents>().clear();
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(target, WidgetActionKind::SetNumber(0.5)));
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.5);
        app.world_mut().resource_mut::<UiRoutedEvents>().clear();
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(target, WidgetActionKind::StepNumber(2)));
        app.update();
        assert!((app.world().get::<Slider>(target).unwrap().value - 0.7).abs() < 1e-9);
        app.world_mut().resource_mut::<UiRoutedEvents>().clear();
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(target, WidgetActionKind::ResetNumber));
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, 0.2);
        app.world_mut().resource_mut::<UiRoutedEvents>().clear();
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(target, WidgetActionKind::SetNumber(0.5)));
        app.update();
        app.world_mut().resource_mut::<UiRoutedEvents>().clear();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target,
            position: [130.0, 24.0],
            delta: [0.0, 1.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert!((app.world().get::<Slider>(target).unwrap().value - 0.6).abs() < 1e-9);
        app.world_mut().resource_mut::<UiRoutedEvents>().clear();

        let before_cancel = app.world().get::<Slider>(target).unwrap().value;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position: [60.0, 24.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position: [210.0, 24.0],
                delta: [150.0, 0.0],
                dragging: true,
            },
            key(UiKeyCode::Escape, UiModifiers::default()),
        ];
        app.update();
        assert_eq!(
            app.world().get::<Slider>(target).unwrap().value,
            before_cancel
        );
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::NumericCancelled { target: event_target, restored }
                if *event_target == target && restored == &[before_cancel])
        ));

        app.world_mut().get_mut::<Slider>(target).unwrap().enabled = false;
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::End, UiModifiers::default())];
        app.update();
        assert_eq!(
            app.world().get::<Slider>(target).unwrap().value,
            before_cancel
        );
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(target, WidgetActionKind::SetNumber(0.1)));
        app.update();
        assert_eq!(
            app.world().get::<Slider>(target).unwrap().value,
            before_cancel
        );

        app.world_mut().get_mut::<Slider>(target).unwrap().enabled = true;
        app.world_mut().get_mut::<Slider>(target).unwrap().read_only = true;
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(target, WidgetActionKind::StepNumber(1)));
        app.update();
        assert_eq!(
            app.world().get::<Slider>(target).unwrap().value,
            before_cancel
        );
    }

    #[test]
    fn slider_paint_uses_runtime_theme_and_disabled_semantic_tokens() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let rect = UiRect::new(0.0, 0.0, 260.0, 32.0);
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                UiVisible(true),
                UiEnabled(true),
                UiFocusable,
                Label::new("Opacity"),
                Slider::new(0.4, 0.0, 1.0, 0.1).unwrap(),
                UiHitBox {
                    window: Some(window),
                    rect,
                    clip: None,
                    z_index: 1,
                },
                UiLayoutResult {
                    window,
                    rect,
                    clip: None,
                    z: 1,
                    content_size: [rect.width, rect.height],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.update();
        let handle_color = |world: &World| {
            world
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .find_map(|item| match item.primitive {
                    Primitive::Ellipse { color, .. } => Some(color),
                    _ => None,
                })
                .expect("slider paint includes a GPU ellipse handle")
        };
        let dark = handle_color(app.world());
        app.world_mut()
            .resource_mut::<Theme>()
            .switch(bui_style::ThemeKind::Light);
        app.update();
        let light = handle_color(app.world());
        assert_ne!(dark, light, "theme switch invalidates slider handle paint");

        app.world_mut().get_mut::<Slider>(target).unwrap().enabled = false;
        app.update();
        let expected = app
            .world()
            .resource::<Theme>()
            .color(ThemeColorToken::ControlDisabledMark);
        assert_eq!(handle_color(app.world()), expected);
    }

    #[test]
    fn numeric_group_scrub_selects_compatible_rows_and_undoes_as_one_transaction() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let spawn_field = |world: &mut World, y: f32, field: NumericField| {
            world
                .spawn((
                    UiNode,
                    UiHitBox {
                        window: Some(window),
                        rect: UiRect::new(0.0, y, 100.0, 18.0),
                        clip: None,
                        z_index: 0,
                    },
                    UiLayoutResult {
                        window,
                        rect: UiRect::new(0.0, y, 100.0, 18.0),
                        clip: None,
                        z: 0,
                        content_size: [100.0, 18.0],
                        dpi_scale: 1.0,
                    },
                    field,
                ))
                .id()
        };
        let first = spawn_field(
            app.world_mut(),
            0.0,
            NumericField::new(0.3, Some(0.0), Some(1.0), 0.2, NumericFieldKind::Float).unwrap(),
        );
        let second = spawn_field(
            app.world_mut(),
            20.0,
            NumericField::new(10.2, Some(10.0), Some(10.6), 0.5, NumericFieldKind::Float).unwrap(),
        );
        let integer = spawn_field(app.world_mut(), 40.0, NumericField::integer(4));
        let mut other_unit = NumericField::float(2.0).unwrap();
        other_unit.unit = Some("m".into());
        let unit_field = spawn_field(app.world_mut(), 60.0, other_unit);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [8.0, 9.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [8.0, 69.0],
                delta: [0.0, 60.0],
                dragging: true,
            },
        ];
        app.update();
        let scrub = app
            .world()
            .resource::<WidgetInteractionState>()
            .numeric_scrubs
            .get(&window)
            .expect("the vertical drag keeps a group selection active");
        assert_eq!(scrub.targets, [first, second]);
        let selected_surface = app
            .world()
            .resource::<Theme>()
            .color(ThemeColorToken::EditorActiveFieldSurface);
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(item.primitive,
                Primitive::RoundedRect { rect, color, .. }
                    if rect.x == 0.0 && rect.y == 20.0 && color == selected_surface))
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::Key {
                window,
                target: first,
                key: UiKeyCode::Other(0),
                pressed: false,
                repeat: false,
                modifiers: UiModifiers {
                    shift: true,
                    ..UiModifiers::default()
                },
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [10.0, 69.0],
                delta: [2.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::Key {
                window,
                target: first,
                key: UiKeyCode::Other(0),
                pressed: false,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [12.0, 69.0],
                delta: [2.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::Key {
                window,
                target: first,
                key: UiKeyCode::Other(0),
                pressed: false,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [12.0, 69.0],
                delta: [0.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [12.0, 69.0],
                cancelled: false,
            },
        ];
        app.update();

        assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 0.8);
        assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 10.6);
        assert_eq!(app.world().get::<NumericField>(integer).unwrap().value, 4.0);
        assert_eq!(
            app.world().get::<NumericField>(unit_field).unwrap().value,
            2.0
        );
        let changes = app
            .world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .find_map(|event| match event {
                WidgetEvent::NumericGroupCommitted { changes } => Some(changes),
                _ => None,
            })
            .expect("one group commit is emitted for the whole drag");
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().any(|(entity, before, after)| {
            *entity == first && *before == 0.3 && *after == 0.8
        }));
        assert!(changes.iter().any(|(entity, before, after)| {
            *entity == second && *before == 10.2 && *after == 10.6
        }));
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 0.3);
        assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 10.2);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Y,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 0.8);
        assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 10.6);
    }

    #[test]
    fn numeric_group_escape_restores_every_selected_value_and_ends_capture() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let spawn_field = |world: &mut World, y: f32, value: f64| {
            world
                .spawn((
                    UiNode,
                    UiHitBox {
                        window: Some(window),
                        rect: UiRect::new(0.0, y, 100.0, 18.0),
                        clip: None,
                        z_index: 0,
                    },
                    NumericField::new(value, None, None, 1.0, NumericFieldKind::Float).unwrap(),
                ))
                .id()
        };
        let first = spawn_field(app.world_mut(), 0.0, 2.0);
        let second = spawn_field(app.world_mut(), 20.0, 8.0);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [8.0, 9.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [8.0, 29.0],
                delta: [0.0, 20.0],
                dragging: true,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: first,
                position: [13.0, 29.0],
                delta: [5.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::Key {
                window,
                target: first,
                key: UiKeyCode::Escape,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
            UiRoutedEvent::PointerUp {
                window,
                target: first,
                button: UiPointerButton::Primary,
                position: [13.0, 29.0],
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(app.world().get::<NumericField>(first).unwrap().value, 2.0);
        assert_eq!(app.world().get::<NumericField>(second).unwrap().value, 8.0);
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::NumericCancelled { target, restored }
                if *target == first && restored == &[2.0, 8.0])
        ));
        assert!(
            !app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .any(|event| matches!(event, WidgetEvent::NumericGroupCommitted { .. }))
        );
        let requests: Vec<_> = app
            .world_mut()
            .resource_mut::<CursorCaptureRequestQueue>()
            .drain()
            .collect();
        assert_eq!(requests.len(), 2);
        assert!(
            matches!(requests[0], CursorCaptureRequest::Begin { target, .. } if target == first)
        );
        assert!(matches!(requests[1], CursorCaptureRequest::End { target, .. } if target == first));
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
    }

    #[test]
    fn widget_paint_collector_uses_layout_bounds_and_current_theme() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let manual = DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            z: -1,
            primitive: Primitive::Text {
                origin: [0.0, 0.0],
                text: "manual".into(),
                color: Rgba(1.0, 1.0, 1.0, 1.0),
            },
        };
        app.world_mut()
            .resource_mut::<DisplayList>()
            .push(manual.clone());
        let entity = app
            .world_mut()
            .spawn((
                UiNode,
                Checkbox::new("visible", true),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(1.0, 2.0, 120.0, 24.0),
                    clip: Some(UiRect::new(10.0, 0.0, 80.0, 30.0)),
                    z: 7,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.update();
        let items = app.world().resource::<DisplayList>().items().to_vec();
        assert!(!items.is_empty());
        assert!(items.contains(&manual));
        assert!(items.len() > 1);
        let widget_items = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert!(widget_items.iter().all(|item| item.z >= 7));
        assert!(
            widget_items
                .iter()
                .all(|item| item.clip.x == 10.0 && item.clip.width == 80.0)
        );
        app.world_mut().entity_mut(entity).despawn();
    }

    #[test]
    fn widget_paint_cache_skips_idle_and_invalidates_visual_and_layout_inputs() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let entity = app
            .world_mut()
            .spawn((
                UiNode,
                Checkbox::new("cached", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(1.0, 2.0, 120.0, 24.0),
                    clip: None,
                    z: 3,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();

        app.update();
        let initial = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert!(!initial.is_empty());
        assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 1);

        app.update();
        assert_eq!(app.world().resource::<WidgetPaintCache>().skipped_frames, 1);
        assert_eq!(
            app.world().resource::<DisplayList>().widget_items(),
            initial
        );

        app.world_mut()
            .get_mut::<Checkbox>(entity)
            .unwrap()
            .toggle();
        app.update();
        let checked = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert_ne!(
            checked, initial,
            "a value mutation repaints in the same frame"
        );
        assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 2);

        app.world_mut().resource_mut::<Theme>().surface = Rgba(0.12, 0.34, 0.56, 1.0);
        app.update();
        let themed = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert_ne!(
            themed, checked,
            "theme changes invalidate widget paint immediately"
        );
        assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 3);

        app.world_mut()
            .get_mut::<UiLayoutResult>(entity)
            .unwrap()
            .rect
            .x += 16.0;
        app.update();
        let moved = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert_ne!(
            moved, themed,
            "layout changes repaint before the frame is presented"
        );
        assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 4);
    }

    #[test]
    fn one_changed_checkbox_repaints_only_its_fragment() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let first = app
            .world_mut()
            .spawn((
                UiNode,
                Checkbox::new("first", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                UiNode,
                Checkbox::new("second", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, 30.0, 120.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.update();
        let fragment_orders: HashMap<_, _> = app
            .world()
            .resource::<DisplayList>()
            .widget_fragments()
            .values()
            .map(|fragment| (fragment.entity, fragment.order))
            .collect();
        let overlay = item(
            ClipRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            9,
            Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                radius: 2.0,
                color: Rgba(0.2, 0.3, 0.4, 1.0),
            },
        );
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_overlay_widget_items(vec![overlay.clone()]);
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .repainted_entities,
            2
        );
        app.world_mut().get_mut::<Checkbox>(first).unwrap().toggle();
        app.world_mut().resource_mut::<WidgetPaintCache>().prepared = true;
        app.world_mut()
            .resource_mut::<UiDirty>()
            .mark(UiDirtyDomain::Paint, first);
        collect_widget_paint(app.world_mut());
        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(cache.repainted_entities, 1);
        assert_eq!(
            cache.published_fragments, 1,
            "one dirty control publishes only its own changed fragment"
        );
        assert_eq!(
            app.world()
                .resource::<DisplayList>()
                .widget_fragments()
                .get(&first)
                .map(|fragment| fragment.order),
            fragment_orders.get(&first).copied()
        );
        assert_eq!(
            app.world()
                .resource::<DisplayList>()
                .widget_fragments()
                .get(&second)
                .map(|fragment| fragment.order),
            fragment_orders.get(&second).copied(),
            "updating one entity keeps its peers at their existing paint ranks"
        );
        assert_eq!(
            app.world().resource::<DisplayList>().overlay_widget_items(),
            &[overlay],
            "per-entity widget updates preserve the post-widget overlay layer"
        );
    }

    #[test]
    fn widget_fragment_camera_migration_and_removal_update_only_that_entity() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let first_camera = app.world_mut().spawn(bevy_camera::Camera::default()).id();
        let second_camera = app.world_mut().spawn_empty().id();
        let migrating = app
            .world_mut()
            .spawn((
                UiNode,
                Checkbox::new("migrate", false),
                UiLayoutResult {
                    window: first_camera,
                    rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                    clip: None,
                    z: 2,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("stays", false),
            UiLayoutResult {
                window: first_camera,
                rect: UiRect::new(0.0, 30.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ));

        app.update();
        app.world_mut()
            .get_mut::<UiLayoutResult>(migrating)
            .unwrap()
            .window = second_camera;
        app.update();

        let has_label = |items: &[DrawItem], label: &str| {
            items.iter().any(
                |item| matches!(&item.primitive, Primitive::Text { text, .. } if text == label),
            )
        };
        let display_list = app.world().resource::<DisplayList>();
        assert!(
            display_list
                .camera_items(first_camera)
                .is_some_and(|items| has_label(items, "stays"))
        );
        assert!(has_label(display_list.widget_items(), "migrate"));
        assert!(
            display_list
                .camera_items(second_camera)
                .is_none_or(|items| items.is_empty())
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            1,
            "moving to a window without a camera updates the same entity key"
        );

        app.world_mut()
            .entity_mut(second_camera)
            .insert(bevy_camera::Camera::default());
        app.update();
        let display_list = app.world().resource::<DisplayList>();
        assert!(
            display_list
                .camera_items(second_camera)
                .is_some_and(|items| has_label(items, "migrate"))
        );
        assert!(!has_label(display_list.widget_items(), "migrate"));
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            1,
            "adding a camera retargets associated widget fragments"
        );

        app.world_mut()
            .entity_mut(second_camera)
            .remove::<bevy_camera::Camera>();
        app.update();
        let display_list = app.world().resource::<DisplayList>();
        assert!(has_label(display_list.widget_items(), "migrate"));
        assert!(
            display_list
                .camera_items(second_camera)
                .is_none_or(|items| items.is_empty())
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            1,
            "removing a camera retargets associated widget fragments to the screen"
        );

        app.world_mut().entity_mut(migrating).despawn();
        app.update();
        let display_list = app.world().resource::<DisplayList>();
        assert!(
            display_list
                .camera_items(first_camera)
                .is_some_and(|items| has_label(items, "stays"))
        );
        assert!(
            !has_label(display_list.widget_items(), "migrate"),
            "despawning a screen-migrated control removes its cached fragment"
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            1,
            "despawning a control removes only its cached fragment"
        );
    }

    #[test]
    fn topology_edits_repaint_only_new_or_reordered_widget_fragments() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let first_group = app.world_mut().spawn((UiNode, UiChildren::default())).id();
        let second_group = app.world_mut().spawn((UiNode, UiChildren::default())).id();
        let spawn_child = |world: &mut World, parent: Entity, label: &str, y: f32| {
            world
                .spawn((
                    UiNode,
                    UiParent(parent),
                    Checkbox::new(label, false),
                    UiLayoutResult {
                        window: Entity::PLACEHOLDER,
                        rect: UiRect::new(0.0, y, 120.0, 24.0),
                        clip: None,
                        z: 1,
                        content_size: [120.0, 24.0],
                        dpi_scale: 1.0,
                    },
                ))
                .id()
        };
        let first = spawn_child(app.world_mut(), first_group, "first", 0.0);
        let second = spawn_child(app.world_mut(), first_group, "second", 30.0);
        let third = spawn_child(app.world_mut(), second_group, "third", 60.0);
        let fourth = spawn_child(app.world_mut(), first_group, "fourth", 45.0);
        app.world_mut()
            .get_mut::<UiChildren>(first_group)
            .unwrap()
            .0 = vec![first, second, fourth];
        app.world_mut()
            .get_mut::<UiChildren>(second_group)
            .unwrap()
            .0 = vec![third];
        for index in 0..10_000 {
            app.world_mut().spawn((
                UiNode,
                Checkbox::new("peer", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(200.0, index as f32 * 24.0, 20.0, 20.0),
                    clip: None,
                    z: 1,
                    content_size: [20.0, 20.0],
                    dpi_scale: 1.0,
                },
            ));
        }
        app.update();

        let added = spawn_child(app.world_mut(), first_group, "added", 90.0);
        app.world_mut()
            .get_mut::<UiChildren>(first_group)
            .unwrap()
            .0
            .push(added);
        app.update();
        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(
            cache.repainted_entities, 1,
            "mounting a widget paints only it"
        );
        assert_eq!(cache.published_fragments, 1);
        let order = |world: &World, entity| {
            world
                .resource::<DisplayList>()
                .widget_fragments()
                .get(&entity)
                .unwrap()
                .order
        };
        assert!(order(app.world(), first) < order(app.world(), second));
        assert!(order(app.world(), second) < order(app.world(), added));
        assert!(order(app.world(), fourth) < order(app.world(), added));
        assert!(
            order(app.world(), added) < order(app.world(), third),
            "added rank {} should precede second group rank {}",
            order(app.world(), added),
            order(app.world(), third)
        );

        app.world_mut()
            .get_mut::<UiChildren>(first_group)
            .unwrap()
            .0 = vec![added, first, second];
        app.update();
        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(
            cache.repainted_entities, 1,
            "sibling reorder repaints the moved rank"
        );
        assert_eq!(cache.published_fragments, 1);
        assert!(order(app.world(), added) < order(app.world(), first));
        assert!(order(app.world(), first) < order(app.world(), second));

        app.world_mut()
            .get_mut::<UiChildren>(first_group)
            .unwrap()
            .0 = vec![second, fourth, added, first];
        app.update();
        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(
            cache.repainted_entities, 2,
            "a multi-sibling reorder republishes both moved fragments"
        );
        assert_eq!(cache.published_fragments, 2);
        assert!(order(app.world(), second) < order(app.world(), fourth));
        assert!(order(app.world(), fourth) < order(app.world(), added));
        assert!(order(app.world(), added) < order(app.world(), first));

        bui_core::set_parent(app.world_mut(), second, second_group).unwrap();
        app.update();
        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(
            cache.repainted_entities, 1,
            "reparenting repaints the moved subtree only"
        );
        assert_eq!(cache.published_fragments, 1);
        assert!(order(app.world(), fourth) < order(app.world(), added));
        assert!(order(app.world(), added) < order(app.world(), first));
        assert!(order(app.world(), first) < order(app.world(), third));
        assert!(order(app.world(), third) < order(app.world(), second));
        assert_eq!(
            app.world()
                .resource::<DisplayList>()
                .widget_fragments()
                .len(),
            10_005,
            "unaffected peers retain their fragments"
        );
    }

    #[test]
    fn local_paint_order_expands_exhausted_rank_gaps_without_collisions() {
        let mut world = World::new();
        let first = world.spawn_empty().id();
        let last = world.spawn_empty().id();
        let mut ranks = HashMap::from([(first, 100), (last, 101)]);
        let mut order_keys = BTreeMap::from([(100, first), (101, last)]);
        let mut inserted = Vec::new();
        let mut saw_neighbor_rebalance = false;

        for _ in 0..80 {
            let entity = world.spawn_empty().id();
            let lower = ranks[&first];
            let upper = *order_keys
                .range((std::ops::Bound::Excluded(lower), std::ops::Bound::Unbounded))
                .next()
                .expect("the last anchor remains after every insertion")
                .0;
            let changed = assign_widget_rank_block(
                &[entity],
                &HashMap::new(),
                lower,
                upper,
                &mut ranks,
                &mut order_keys,
            );
            saw_neighbor_rebalance |= changed.len() > 1;
            inserted.push(entity);
        }

        let actual: Vec<_> = order_keys.values().copied().collect();
        let expected: Vec<_> = std::iter::once(first)
            .chain(inserted.into_iter().rev())
            .chain(std::iter::once(last))
            .collect();
        assert_eq!(
            actual, expected,
            "neighbor expansion preserves requested order"
        );
        assert!(
            saw_neighbor_rebalance,
            "a saturated local gap rebalances neighbors"
        );
        assert_eq!(order_keys.len(), ranks.len());
        for (rank, entity) in &order_keys {
            assert_eq!(ranks.get(entity), Some(rank));
        }
    }

    #[test]
    fn ten_thousand_peers_only_repaint_for_inherited_state_and_active_menu_changes() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut().resource_mut::<Theme>().motion.menu_open =
            std::time::Duration::from_secs(1);
        let state_root = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiChildren::default(),
            ))
            .id();
        let visible_child = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(state_root),
                Checkbox::new("visible child", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, 0.0, 160.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [160.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        let enabled_child = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(state_root),
                Checkbox::new("enabled child", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, 30.0, 160.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [160.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.world_mut()
            .entity_mut(state_root)
            .insert(UiChildren(vec![visible_child, enabled_child]));
        for index in 0..10_000 {
            app.world_mut().spawn((
                UiNode,
                Checkbox::new("", false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(200.0, index as f32 * 24.0, 20.0, 20.0),
                    clip: None,
                    z: 1,
                    content_size: [20.0, 20.0],
                    dpi_scale: 1.0,
                },
            ));
        }

        app.update();
        assert_eq!(
            app.world().resource::<WidgetPaintCache>().fragments.len(),
            10_002
        );

        app.world_mut().get_mut::<UiVisible>(state_root).unwrap().0 = false;
        app.update();
        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(cache.repainted_entities, 0);
        assert_eq!(
            cache.published_fragments, 2,
            "hiding a parent removes only its two child fragments"
        );
        assert!(
            !app.world()
                .resource::<DisplayList>()
                .widget_fragments()
                .contains_key(&visible_child)
        );
        assert!(
            !app.world()
                .resource::<DisplayList>()
                .widget_fragments()
                .contains_key(&enabled_child)
        );

        app.world_mut().get_mut::<UiVisible>(state_root).unwrap().0 = true;
        app.update();
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .repainted_entities,
            2
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            2,
            "showing a parent republishes only its two child fragments"
        );

        app.world_mut().get_mut::<UiEnabled>(state_root).unwrap().0 = false;
        app.update();
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .repainted_entities,
            2
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            2,
            "disabling a parent refreshes only its two child fragments"
        );

        app.world_mut().get_mut::<UiEnabled>(state_root).unwrap().0 = true;
        app.update();
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .repainted_entities,
            2
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            2
        );

        let menu = app
            .world_mut()
            .spawn((
                UiNode,
                MenuControl::new(
                    "Actions",
                    crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
                    MenuDisplay::Dropdown,
                ),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(20.0, 20.0, 120.0, 24.0),
                    clip: None,
                    z: 2,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
                UiChildren::default(),
            ))
            .id();
        let menu_item = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(menu),
                UiVisible(false),
                MenuItemControl {
                    owner: menu,
                    id: "open".to_owned(),
                    label: "Open".to_owned(),
                    parent_path: Vec::new(),
                    index: 0,
                    enabled: true,
                    separator: false,
                    has_submenu: false,
                    shortcut: None,
                },
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, 0.0, 1.0, 1.0),
                    clip: None,
                    z: 0,
                    content_size: [1.0, 1.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<UiChildren>(menu)
            .unwrap()
            .0
            .push(menu_item);
        app.world_mut()
            .get_mut::<MenuControl>(menu)
            .unwrap()
            .items
            .push(menu_item);
        app.update();
        app.world_mut()
            .get_mut::<MenuControl>(menu)
            .unwrap()
            .open(None);
        app.update();
        app.update();

        let cache = app.world().resource::<WidgetPaintCache>();
        assert_eq!(
            cache.repainted_entities, 2,
            "a menu transition repaints only its owner and item subtree"
        );
        assert_eq!(
            cache.published_fragments, 2,
            "menu animation output is scoped to the active menu subtree"
        );
    }

    #[test]
    fn pointer_position_alone_does_not_repaint_widgets() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin).init_resource::<UiFocus>();
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .pointer_position = Some([4.0, 4.0]);
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("steady", false),
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z: 0,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ));
        app.update();
        let rebuilt = app.world().resource::<WidgetPaintCache>().rebuilt_frames;

        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .pointer_position = Some([5.0, 5.0]);
        app.update();
        assert_eq!(
            app.world().resource::<WidgetPaintCache>().rebuilt_frames,
            rebuilt
        );
    }

    #[test]
    fn widget_paint_culls_items_outside_the_ancestor_clip() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("visible", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 0.0, 20.0, 20.0),
                clip: Some(UiRect::new(0.0, 0.0, 20.0, 20.0)),
                z: 1,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ));
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("clipped", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(100.0, 100.0, 20.0, 20.0),
                clip: Some(UiRect::new(0.0, 0.0, 20.0, 20.0)),
                z: 1,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ));

        app.update();
        let items = app.world().resource::<DisplayList>().widget_items();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.clip
            == ClipRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            }));
    }

    #[test]
    fn runtime_theme_round_trip_keeps_tree_identity_and_refreshes_widget_paint() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let tree = WidgetTree::new(WidgetNode::column([
            WidgetNode::button("Apply").with_key("apply"),
            WidgetNode::checkbox("Visible", true).with_key("visible"),
            WidgetNode::number("Scale", 1.0, 0.0, 10.0, 0.1).with_key("scale"),
        ]));
        tree.spawn(app.world_mut(), None)
            .expect("the same declared tree remains mounted while themes switch");
        let keyed_entities: Vec<_> = app
            .world()
            .iter_entities()
            .filter_map(|entity| {
                entity
                    .get::<UiKey>()
                    .map(|key| (key.0.clone(), entity.id()))
            })
            .collect();
        for (key, entity) in &keyed_entities {
            let y = match key.as_str() {
                "apply" => 0.0,
                "visible" => 30.0,
                "scale" => 60.0,
                _ => unreachable!(),
            };
            app.world_mut().entity_mut(*entity).insert(UiLayoutResult {
                window,
                rect: UiRect::new(12.0, y, 180.0, 24.0),
                clip: None,
                z: 4,
                content_size: [180.0, 24.0],
                dpi_scale: 1.0,
            });
        }

        app.update();
        let dark_theme = app.world().resource::<Theme>().clone();
        let dark_items = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert!(dark_items.iter().any(|item| {
            matches!(item.primitive, Primitive::RoundedRect { color, .. } if color == dark_theme.surface)
        }));

        *app.world_mut().resource_mut::<Theme>() = Theme::new(bui_style::ThemeKind::Light);
        app.update();
        let light_theme = app.world().resource::<Theme>().clone();
        let light_items = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert_ne!(
            dark_items, light_items,
            "theme changes invalidate widget paint"
        );
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .published_fragments,
            3,
            "a global theme change republishes each affected control fragment"
        );
        assert!(light_items.iter().any(|item| {
            matches!(item.primitive, Primitive::RoundedRect { color, .. } if color == light_theme.surface)
        }));
        assert_eq!(
            keyed_entities,
            app.world()
                .iter_entities()
                .filter_map(|entity| {
                    entity
                        .get::<UiKey>()
                        .map(|key| (key.0.clone(), entity.id()))
                })
                .collect::<Vec<_>>(),
            "switching a theme must preserve the mounted entity identities"
        );
        for (_, entity) in &keyed_entities {
            assert_eq!(
                app.world().get::<UiLayoutResult>(*entity).unwrap().rect,
                UiRect::new(
                    12.0,
                    match app.world().get::<UiKey>(*entity).unwrap().0.as_str() {
                        "apply" => 0.0,
                        "visible" => 30.0,
                        "scale" => 60.0,
                        _ => unreachable!(),
                    },
                    180.0,
                    24.0,
                )
            );
        }

        *app.world_mut().resource_mut::<Theme>() = Theme::new(bui_style::ThemeKind::Dark);
        app.update();
        assert_eq!(
            app.world().resource::<DisplayList>().widget_items(),
            dark_items.as_slice(),
            "returning to the original theme restores its paint deterministically"
        );
    }

    #[test]
    fn widget_paint_uses_computed_colors_opacity_and_corner_radius() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut style = UiComputedStyle::default();
        style.properties.insert(
            UiStyleProperty::Background,
            bui_style::UiStyleValue::Color(Rgba(0.2, 0.3, 0.4, 1.0)),
        );
        style.properties.insert(
            UiStyleProperty::Foreground,
            bui_style::UiStyleValue::Color(Rgba(0.6, 0.7, 0.8, 1.0)),
        );
        style.properties.insert(
            UiStyleProperty::CornerRadius,
            bui_style::UiStyleValue::Scalar(11.0),
        );
        style.properties.insert(
            UiStyleProperty::Opacity,
            bui_style::UiStyleValue::Scalar(0.5),
        );
        style.properties.insert(
            UiStyleProperty::FontSize,
            bui_style::UiStyleValue::Scalar(20.0),
        );
        style.properties.insert(
            UiStyleProperty::LineHeight,
            bui_style::UiStyleValue::Scalar(1.5),
        );
        let entity = app
            .world_mut()
            .spawn((
                UiNode,
                Button::new("styled"),
                style,
                UiLayoutResult {
                    window,
                    rect: UiRect::new(4.0, 8.0, 90.0, 26.0),
                    clip: None,
                    z: 3,
                    content_size: [90.0, 26.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.update();

        let items = app.world().resource::<DisplayList>().widget_items();
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { radius: 11.0, color, .. }
                if color == Rgba(0.2, 0.3, 0.4, 0.5)
        )));
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { color, style: Some(style), .. }
                if *color == Rgba(0.6, 0.7, 0.8, 0.5)
                    && style.font.as_ref().is_some_and(|font| matches!(
                        font.font_size,
                        bevy_text::FontSize::Px(20.0)
                    ))
                    && style.line_height == Some(LineHeight::RelativeToFont(1.5))
        )));
        app.world_mut().entity_mut(entity).despawn();
    }

    #[test]
    fn widget_paint_uses_semantic_button_tokens_and_control_spacing() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut theme = Theme::new(bui_style::ThemeKind::Light);
        theme.spacing.control.padding_x = 13.0;
        theme.spacing.control.padding_y = 7.0;
        let disabled_surface = theme.color(ThemeColorToken::ButtonDisabledSurface);
        let disabled_text = theme.color(ThemeColorToken::DisabledText);
        *app.world_mut().resource_mut::<Theme>() = theme;
        let mut button = Button::new("disabled");
        button.enabled = false;
        app.world_mut().spawn((
            UiNode,
            button,
            UiLayoutResult {
                window,
                rect: UiRect::new(20.0, 30.0, 120.0, 28.0),
                clip: None,
                z: 3,
                content_size: [120.0, 28.0],
                dpi_scale: 1.0,
            },
        ));
        app.update();

        let items = app.world().resource::<DisplayList>().widget_items();
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { color, .. } if color == disabled_surface
        )));
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { origin, color, .. }
                if *origin == [33.0, 37.0] && *color == disabled_text
        )));
    }

    #[test]
    fn widget_paint_keeps_screen_and_camera_layers_separate() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let screen = app.world_mut().spawn_empty().id();
        let camera = app.world_mut().spawn(bevy_camera::Camera::default()).id();
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("screen", true),
            UiLayoutResult {
                window: screen,
                rect: UiRect::new(0.0, 0.0, 80.0, 20.0),
                clip: None,
                z: 1,
                content_size: [80.0, 20.0],
                dpi_scale: 1.0,
            },
        ));
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("world", true),
            UiLayoutResult {
                window: camera,
                rect: UiRect::new(0.0, 0.0, 80.0, 20.0),
                clip: None,
                z: 2,
                content_size: [80.0, 20.0],
                dpi_scale: 1.0,
            },
        ));
        app.update();
        let display_list = app.world().resource::<DisplayList>();
        assert!(!display_list.widget_items().is_empty());
        assert!(
            display_list
                .camera_items(camera)
                .is_some_and(|items| !items.is_empty())
        );
        assert!(display_list.items().iter().any(|item| item.z == 1));
        assert!(display_list.items().iter().any(|item| item.z == 2));
    }

    #[test]
    fn application_actions_are_applied_in_plugin_widget_phase() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let entity = app.world_mut().spawn(Checkbox::new("enabled", false)).id();
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(entity, WidgetActionKind::SetBool(true)));
        app.update();
        assert!(app.world().get::<Checkbox>(entity).unwrap().is_checked());
        assert_eq!(
            app.world_mut()
                .resource_mut::<WidgetEventQueue>()
                .take()
                .len(),
            1
        );
    }

    #[test]
    fn radio_group_and_icon_button_are_visible_and_route_pointer_keyboard_and_events() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut radio = RadioGroup::new(["Object", "Edit", "Sculpt"], Some(0));
        radio.options[2].enabled = false;
        let tree = WidgetTree::new(WidgetNode::row([
            WidgetNode::radio_group("Mode", radio),
            WidgetNode::icon_button("+", "Add object"),
            WidgetNode::button("Disabled"),
            WidgetNode::text_field("Read only", "locked"),
        ]));
        let root = tree.spawn(app.world_mut(), None).unwrap();
        let controls = app.world().get::<UiChildren>(root).unwrap().0.clone();
        let radio = controls[0];
        let icon = controls[1];
        let disabled = controls[2];
        let read_only = controls[3];
        app.world_mut().get_mut::<Button>(disabled).unwrap().enabled = false;
        app.world_mut()
            .get_mut::<TextField>(read_only)
            .unwrap()
            .read_only = true;
        app.world_mut().entity_mut(read_only).insert(UiReadOnly);
        for (entity, rect) in [
            (radio, UiRect::new(10.0, 10.0, 220.0, 96.0)),
            (icon, UiRect::new(260.0, 10.0, 32.0, 28.0)),
            (disabled, UiRect::new(300.0, 10.0, 100.0, 28.0)),
        ] {
            app.world_mut().entity_mut(entity).insert((
                UiLayoutResult {
                    window,
                    rect,
                    clip: None,
                    z: 5,
                    content_size: [rect.width, rect.height],
                    dpi_scale: 1.0,
                },
                UiHitBox {
                    window: Some(window),
                    rect,
                    clip: None,
                    z_index: 5,
                },
            ));
        }
        app.update();
        assert!(app.world().resource::<DisplayList>().widget_items().iter().any(
            |item| matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "+")
        ));
        assert!(app.world().resource::<DisplayList>().widget_items().iter().any(
            |item| matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "Edit")
        ));
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Ellipse { .. }))
        );

        let click = |target, position| UiRoutedEvent::Click {
            window,
            target,
            button: UiPointerButton::Primary,
            click_count: 1,
            position,
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(radio, [24.0, 70.0])];
        app.update();
        assert_eq!(
            app.world().get::<RadioGroup>(radio).unwrap().selected,
            Some(1)
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(0)), after: WidgetValue::Option(Some(1)) } if *target == radio)));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(radio, [24.0, 94.0])];
        app.update();
        assert_eq!(
            app.world().get::<RadioGroup>(radio).unwrap().selected,
            Some(1),
            "disabled options cannot be selected by pointer"
        );

        let key = |key| UiRoutedEvent::Key {
            window,
            target: radio,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::ArrowRight)];
        app.update();
        assert_eq!(
            app.world().get::<RadioGroup>(radio).unwrap().selected,
            Some(0)
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, .. } if *target == radio)));
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
        app.update();
        assert_eq!(
            app.world().get::<RadioGroup>(radio).unwrap().selected,
            Some(1)
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, .. } if *target == radio)));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(icon, [276.0, 24.0])];
        app.update();
        assert_eq!(app.world().get::<IconButton>(icon).unwrap().activations, 1);
        assert!(
            app.world_mut()
                .resource_mut::<WidgetEventQueue>()
                .take()
                .iter()
                .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == icon))
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![click(disabled, [320.0, 24.0])];
        app.update();
        assert_eq!(app.world().get::<Button>(disabled).unwrap().activations, 0);
        assert!(!app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == disabled)));
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                read_only,
                WidgetActionKind::InsertText(" edit".to_owned()),
            ));
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(read_only).unwrap().as_str(),
            "locked",
            "read-only text controls ignore routed insert actions"
        );
        assert!(!app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::TextChanged { target, .. } if *target == read_only)));
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: icon,
            key: UiKeyCode::Space,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(app.world().get::<IconButton>(icon).unwrap().activations, 2);
        assert!(
            app.world_mut()
                .resource_mut::<WidgetEventQueue>()
                .take()
                .iter()
                .any(|event| matches!(event, WidgetEvent::Activated { target } if *target == icon))
        );
    }

    fn send_tab(app: &mut App, window: Entity, shift: bool) {
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    shift,
                    ..UiModifiers::default()
                },
            });
        app.update();
    }

    fn send_key(app: &mut App, window: Entity, key: UiKeyCode, modifiers: UiModifiers) {
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key,
                pressed: true,
                repeat: false,
                modifiers,
            });
        app.update();
    }

    #[test]
    fn committed_text_blur_uses_global_history_after_local_undo_priority() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
        let window = app.world_mut().spawn_empty().id();
        let text = WidgetTree::new(WidgetNode::text_field("Name", "a🧊c"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(text).insert((
            UiTabIndex(0),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 0,
            },
        ));
        let other = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(1),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(120.0, 0.0, 100.0, 28.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<TextBuffer>(text)
            .unwrap()
            .set_selection(5, 1);
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(text);
        app.update();

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Text {
                window,
                text: "Z".into(),
            });
        app.update();
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "aZc");
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 1);

        let control = UiModifiers {
            control: true,
            ..UiModifiers::default()
        };
        send_key(&mut app, window, UiKeyCode::Z, control);
        assert_eq!(
            app.world().get::<TextBuffer>(text).unwrap().as_str(),
            "a🧊c"
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        send_key(&mut app, window, UiKeyCode::Y, control);
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "aZc");
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);

        send_tab(&mut app, window, false);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(other)
        );
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 0);
        assert!(app
            .world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::TextCommitted { target, before, after, before_selection, after_selection, .. }
                if *target == text
                    && before == "a🧊c"
                    && after == "aZc"
                    && *before_selection == TextSelection { anchor: 5, caret: 1 }
                    && *after_selection == TextSelection { anchor: 2, caret: 2 })));
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        send_key(&mut app, window, UiKeyCode::Z, control);
        let buffer = app.world().get::<TextBuffer>(text).unwrap();
        assert_eq!(buffer.as_str(), "a🧊c");
        assert_eq!(buffer.anchor(), 5);
        assert_eq!(buffer.caret(), 1);
        assert_eq!(buffer.undo_len(), 0);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(text)
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        assert_eq!(app.world().resource::<bui_core::UiHistory>().redo_len(), 1);

        send_key(&mut app, window, UiKeyCode::Y, control);
        let buffer = app.world().get::<TextBuffer>(text).unwrap();
        assert_eq!(buffer.as_str(), "aZc");
        assert_eq!(buffer.anchor(), 2);
        assert_eq!(buffer.caret(), 2);
        assert_eq!(
            buffer.undo_len(),
            0,
            "global redo must not leave a duplicate local edit"
        );

        send_tab(&mut app, window, false);
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(|event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == text))
                .count(),
            1,
            "redo rebases the focused draft instead of recommitting the same text"
        );
    }

    #[test]
    fn locally_cancelled_text_is_not_committed_and_enter_commits_once() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
        let window = app.world_mut().spawn_empty().id();
        let text = WidgetTree::new(WidgetNode::text_field("Name", "a"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(text).insert((
            UiTabIndex(0),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
                clip: None,
                z_index: 0,
            },
        ));
        let other = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(1),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(120.0, 0.0, 100.0, 28.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(text);
        app.update();

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Text {
                window,
                text: "b".into(),
            });
        app.update();
        send_key(
            &mut app,
            window,
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "a");
        send_tab(&mut app, window, false);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(other)
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == text)
        ));

        send_tab(&mut app, window, true);
        send_key(
            &mut app,
            window,
            UiKeyCode::Y,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "ab");
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        send_key(&mut app, window, UiKeyCode::Enter, UiModifiers::default());
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "ab");
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().undo_len(), 0);
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        assert_eq!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .filter(
                    |event| matches!(event, WidgetEvent::TextCommitted { target, before, after, .. }
                    if *target == text && before == "a" && after == "ab")
                )
                .count(),
            1
        );

        send_key(
            &mut app,
            window,
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(app.world().get::<TextBuffer>(text).unwrap().as_str(), "a");
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(text)
        );
    }

    #[test]
    fn textarea_ime_preedit_cancels_and_ime_commit_enters_global_history_once() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
        let window = app.world_mut().spawn_empty().id();
        let area = WidgetTree::new(WidgetNode::text_area("Notes", "draft"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(area).insert((
            UiTabIndex(0),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 160.0, 60.0),
                clip: None,
                z_index: 0,
            },
        ));
        let other = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiTabIndex(1),
                UiEnabled(true),
                UiVisible(true),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(180.0, 0.0, 100.0, 28.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(area);
        app.update();

        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::ImePreedit {
                window,
                text: "仮".into(),
                cursor_range: None,
            },
            UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ]);
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(area).unwrap().as_str(),
            "draft"
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        assert!(!app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::TextCommitted { target, .. } if *target == area)
        ));
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(other)
        );

        send_tab(&mut app, window, true);
        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::ImePreedit {
                window,
                text: "かな".into(),
                cursor_range: None,
            },
            UiInputEvent::ImeCommit {
                window,
                text: "かな".into(),
            },
            UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ]);
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(area).unwrap().as_str(),
            "draftかな"
        );
        assert_eq!(app.world().get::<TextBuffer>(area).unwrap().undo_len(), 0);
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        send_key(
            &mut app,
            window,
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(
            app.world().get::<TextBuffer>(area).unwrap().as_str(),
            "draft"
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 0);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(area)
        );
    }

    #[test]
    fn tab_leaves_open_popovers_and_dropdowns_without_restoring_trigger_focus() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, WidgetPlugin));
        let window = app.world_mut().spawn_empty().id();
        let spawn_focusable = |world: &mut World, tab_index: i32, x: f32| {
            world
                .spawn((
                    UiNode,
                    UiEnabled(true),
                    UiVisible(true),
                    UiFocusable,
                    UiTabIndex(tab_index),
                    UiHitBox {
                        window: Some(window),
                        rect: UiRect::new(x, 0.0, 80.0, 24.0),
                        clip: None,
                        z_index: tab_index,
                    },
                ))
                .id()
        };
        let before = spawn_focusable(app.world_mut(), 0, 0.0);
        let mut popover = MenuControl::new(
            "Tools",
            crate::MenuTree::new(vec![crate::MenuEntry::command("apply", "Apply")]),
            MenuDisplay::Popover,
        );
        popover.open(Some("tools-popover".into()));
        popover.popup.open = true;
        let menu = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(1),
                UiKey("tools-popover".into()),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(90.0, 0.0, 80.0, 24.0),
                    clip: None,
                    z_index: 1,
                },
                UiLayoutResult {
                    window,
                    rect: UiRect::new(90.0, 0.0, 80.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [80.0, 24.0],
                    dpi_scale: 1.0,
                },
                popover,
            ))
            .id();
        let dropdown = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(2),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(180.0, 0.0, 80.0, 24.0),
                    clip: None,
                    z_index: 2,
                },
                Dropdown::new(["A", "B"], Some(0)),
            ))
            .id();
        let after = spawn_focusable(app.world_mut(), 3, 270.0);
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(menu);

        app.update();
        assert!(app.world().get::<MenuControl>(menu).unwrap().session.open);
        send_tab(&mut app, window, false);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(dropdown)
        );
        assert!(!app.world().get::<MenuControl>(menu).unwrap().session.open);
        assert!(!app.world().get::<MenuControl>(menu).unwrap().popup.open);

        app.world_mut()
            .get_mut::<Dropdown>(dropdown)
            .unwrap()
            .open();
        send_tab(&mut app, window, false);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(after)
        );
        assert!(!app.world().get::<Dropdown>(dropdown).unwrap().open);

        {
            let mut menu = app.world_mut().get_mut::<MenuControl>(menu).unwrap();
            menu.open(Some("tools-popover".into()));
            menu.popup.open = true;
        }
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(menu);
        app.update();
        send_tab(&mut app, window, true);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(before)
        );
        assert!(!app.world().get::<MenuControl>(menu).unwrap().session.open);
    }

    #[test]
    fn dropdown_popup_clamps_to_viewport_keeps_trigger_hitbox_and_routes_options() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let viewport = Rect::new(0.0, 0.0, 320.0, 220.0);
        app.world_mut()
            .spawn(UiLayoutRoot::new(window, viewport, 1.0));
        let options: Vec<_> = (0..12).map(|index| format!("Option {index}")).collect();
        let owner = WidgetTree::new(WidgetNode::dropdown("Quality", options, Some(0)))
            .spawn(app.world_mut(), None)
            .unwrap();
        let trigger = Rect::new(42.0, 155.0, 132.0, 24.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: trigger,
            clip: None,
            z: 7,
            content_size: [trigger.width, trigger.height],
            dpi_scale: 1.0,
        });
        app.world_mut().get_mut::<Dropdown>(owner).unwrap().open();
        app.update();

        let popup_state = *app.world().get::<DropdownPopupState>(owner).unwrap();
        assert!(popup_state.popup.open);
        assert_eq!(popup_state.popup.side, PopupSide::Above);
        assert!(popup_state.popup.bounds.x >= viewport.x);
        assert!(popup_state.popup.bounds.y >= viewport.y);
        assert!(
            popup_state.popup.bounds.x + popup_state.popup.bounds.width
                <= viewport.x + viewport.width
        );
        assert!(
            popup_state.popup.bounds.y + popup_state.popup.bounds.height
                <= viewport.y + viewport.height
        );
        assert!(popup_state.popup.bounds.y + popup_state.popup.bounds.height <= trigger.y);

        let owner_hit = app.world().get::<UiHitBox>(owner).unwrap();
        assert_eq!(
            owner_hit.rect, trigger,
            "open popup must not steal trigger hitbox"
        );
        let mut option_query = app.world_mut().query::<(Entity, &DropdownOptionControl)>();
        let options: Vec<_> = option_query
            .iter(app.world())
            .filter(|(_, option)| option.owner == owner)
            .map(|(entity, option)| (entity, option.clone()))
            .collect();
        assert_eq!(options.len(), 12);
        for (entity, option) in &options {
            let expected_visible = option.index >= popup_state.first_visible
                && option.index
                    < popup_state
                        .first_visible
                        .saturating_add(popup_state.visible_count);
            assert_eq!(
                app.world()
                    .get::<UiVisible>(*entity)
                    .is_some_and(|visible| visible.0),
                expected_visible,
                "option row visibility follows the popup's virtual range"
            );
            if expected_visible {
                assert!(app.world().get::<UiHitBox>(*entity).is_some());
            }
        }
        assert!(app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "Option 0")));

        let click = |target| UiRoutedEvent::Click {
            window,
            target,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [trigger.x + 4.0, trigger.y + 4.0],
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(owner)];
        app.update();
        assert!(!app.world().get::<Dropdown>(owner).unwrap().open);
        assert_eq!(app.world().get::<UiHitBox>(owner).unwrap().rect, trigger);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![click(owner)];
        app.update();
        assert!(app.world().get::<Dropdown>(owner).unwrap().open);
        let option_entity = options
            .iter()
            .find_map(|(entity, option)| (option.index == 2).then_some(*entity))
            .unwrap();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: option_entity,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [trigger.x + 4.0, trigger.y + 4.0],
        }];
        app.update();
        let dropdown = app.world().get::<Dropdown>(owner).unwrap();
        assert_eq!(dropdown.selected, Some(2));
        assert!(!dropdown.open);
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(0)), after: WidgetValue::Option(Some(2)) } if *target == owner)));

        let key = |key| UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
        app.update();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::PageDown)];
        app.update();
        assert_eq!(
            app.world().get::<Dropdown>(owner).unwrap().highlighted,
            Some(8)
        );
        assert_eq!(
            app.world()
                .get::<DropdownPopupState>(owner)
                .unwrap()
                .first_visible,
            3
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: owner,
            position: [trigger.x + 8.0, trigger.y + 8.0],
            delta: [0.0, -1.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<DropdownPopupState>(owner)
                .unwrap()
                .first_visible,
            6,
            "wheel scrolling advances the virtual option viewport"
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::End)];
        app.update();
        assert_eq!(
            app.world().get::<Dropdown>(owner).unwrap().highlighted,
            Some(11)
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(UiKeyCode::Enter)];
        app.update();
        assert_eq!(
            app.world().get::<Dropdown>(owner).unwrap().selected,
            Some(11)
        );
        assert!(!app.world().get::<Dropdown>(owner).unwrap().open);
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(2)), after: WidgetValue::Option(Some(11)) } if *target == owner)));
    }

    #[test]
    fn radio_group_materializes_stable_option_controls_for_pointer_and_accessibility() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let owner = WidgetTree::new(WidgetNode::radio_group(
            "Mode",
            RadioGroup::new(["Object", "Edit", "Sculpt"], Some(0)),
        ))
        .spawn(app.world_mut(), None)
        .unwrap();
        let rect = Rect::new(12.0, 18.0, 180.0, 96.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 2,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();
        let mut query = app.world_mut().query::<(Entity, &RadioOptionControl)>();
        let before: Vec<_> = query
            .iter(app.world())
            .filter(|(_, option)| option.owner == owner)
            .map(|(entity, option)| (entity, option.clone()))
            .collect();
        assert_eq!(before.len(), 3);
        assert_eq!(before[0].1.label, "Object");
        assert!(before[0].1.selected);
        assert_eq!(before[1].1.label, "Edit");
        assert!(!before[1].1.selected);
        assert!(before.iter().all(|(entity, _)| {
            app.world().get::<UiHitBox>(*entity).is_some()
                && app
                    .world()
                    .get::<UiLayoutResult>(*entity)
                    .is_some_and(|layout| layout.window == window)
        }));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: before[1].0,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [24.0, 55.0],
        }];
        app.update();
        assert_eq!(
            app.world().get::<RadioGroup>(owner).unwrap().selected,
            Some(1)
        );
        let mut query = app.world_mut().query::<(Entity, &RadioOptionControl)>();
        let after: Vec<_> = query
            .iter(app.world())
            .filter(|(_, option)| option.owner == owner)
            .map(|(entity, option)| (entity, option.clone()))
            .collect();
        assert_eq!(
            after.iter().map(|(entity, _)| *entity).collect::<Vec<_>>(),
            before.iter().map(|(entity, _)| *entity).collect::<Vec<_>>()
        );
        assert!(after[1].1.selected);
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::ValueChanged { target, before: WidgetValue::Option(Some(0)), after: WidgetValue::Option(Some(1)) } if *target == owner)));
    }

    #[test]
    fn focus_departure_commits_composite_edits_and_text_ime_keeps_local_undo() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, bui_text::TextPlugin, WidgetPlugin));
        let window = app.world_mut().spawn_empty().id();
        let input_hit_box = |tab_index| {
            (
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiFocusable,
                UiTabIndex(tab_index),
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(tab_index as f32 * 100.0, 0.0, 90.0, 28.0),
                    clip: None,
                    z_index: tab_index,
                },
            )
        };
        let mut number = NumericField::float(1.0).unwrap();
        number.edit_insert("5");
        let number = app.world_mut().spawn((input_hit_box(0), number)).id();
        let editor = HsvColorEditor::from_rgba(Rgba(1.0, 0.0, 0.0, 1.0)).unwrap();
        let hsv = app
            .world_mut()
            .spawn((
                input_hit_box(1),
                editor,
                HsvColorEditorState {
                    active_field: HsvColorField::Hue,
                    editing: true,
                    edit_buffer: "180".into(),
                },
            ))
            .id();
        let text = WidgetTree::new(WidgetNode::text_field("Name", "draft"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(text).insert((
            UiTabIndex(2),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(200.0, 0.0, 90.0, 28.0),
                clip: None,
                z_index: 2,
            },
        ));
        let area = WidgetTree::new(WidgetNode::text_area("Notes", "draft"))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(area).insert((
            UiTabIndex(3),
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(300.0, 0.0, 90.0, 28.0),
                clip: None,
                z_index: 3,
            },
        ));
        let after = app.world_mut().spawn(input_hit_box(4)).id();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(number);

        send_tab(&mut app, window, false);
        assert_eq!(app.world().get::<NumericField>(number).unwrap().value, 5.0);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(hsv)
        );
        send_tab(&mut app, window, false);
        let editor = app.world().get::<HsvColorEditor>(hsv).unwrap();
        assert!((editor.hue_degrees - 180.0).abs() < 0.01);
        let edit_state = app.world().get::<HsvColorEditorState>(hsv).unwrap();
        assert!(!edit_state.editing);
        assert!(edit_state.edit_buffer.is_empty());
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(text)
        );

        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::Text {
                window,
                text: "x".into(),
            },
            UiInputEvent::ImePreedit {
                window,
                text: "に".into(),
                cursor_range: None,
            },
            UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ]);
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(text).unwrap().as_str(),
            "draftx"
        );
        assert!(
            app.world()
                .get::<TextBuffer>(text)
                .unwrap()
                .composition()
                .is_none()
        );
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(area)
        );
        let app_undo_depth = app.world().resource::<bui_core::UiHistory>().undo_len();

        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::Text {
                window,
                text: "y".into(),
            },
            UiInputEvent::ImePreedit {
                window,
                text: "かな".into(),
                cursor_range: None,
            },
            UiInputEvent::Key {
                window,
                key: UiKeyCode::Tab,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ]);
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(after)
        );
        let buffer = app.world().get::<TextBuffer>(area).unwrap();
        assert_eq!(buffer.as_str(), "drafty");
        assert!(
            buffer.composition().is_none(),
            "Tab cancels outgoing TextArea IME preedit"
        );

        send_tab(&mut app, window, true);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(area)
        );
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Z,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            });
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(area).unwrap().as_str(),
            "draft"
        );
        assert_eq!(
            app.world().resource::<bui_core::UiHistory>().undo_len(),
            app_undo_depth,
            "the blurred TextArea edit is committed and undone through shared history"
        );

        send_tab(&mut app, window, true);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(text)
        );
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::Z,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            });
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(text).unwrap().as_str(),
            "draft"
        );
        assert_eq!(
            app.world().resource::<bui_core::UiHistory>().undo_len(),
            app_undo_depth - 1,
            "the previously blurred TextField edit is the next shared undo step"
        );
    }

    #[test]
    fn widget_tree_menu_opens_searches_routes_items_and_emits_commands() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let tree = WidgetTree::new(WidgetNode::menu(
            "File",
            [
                crate::MenuEntry::command("open", "Open"),
                crate::MenuEntry::submenu(
                    "more",
                    "More",
                    vec![crate::MenuEntry::command("advanced", "Advanced")],
                ),
            ],
        ));
        let owner = tree.spawn(app.world_mut(), None).unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(12.0, 18.0, 90.0, 24.0),
            clip: None,
            z: 4,
            content_size: [90.0, 24.0],
            dpi_scale: 1.0,
        });
        let items = app.world().get::<MenuControl>(owner).unwrap().items.clone();
        let more = items
            .iter()
            .copied()
            .find(|entity| app.world().get::<MenuItemControl>(*entity).unwrap().id == "more")
            .unwrap();
        let advanced = items
            .iter()
            .copied()
            .find(|entity| app.world().get::<MenuItemControl>(*entity).unwrap().id == "advanced")
            .unwrap();

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [20.0, 20.0],
        }];
        app.update();
        assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert!(app.world().get::<UiVisible>(more).unwrap().0);
        assert!(!app.world().get::<UiVisible>(advanced).unwrap().0);
        assert!(app.world().get::<UiHitBox>(more).is_some());

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerEnter {
                window,
                target: more,
            }];
        app.update();
        assert!(app.world().get::<UiVisible>(advanced).unwrap().0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: owner,
            text: "adv".into(),
        }];
        app.update();
        assert!(app.world().get::<UiVisible>(advanced).unwrap().0);
        assert!(!app.world().get::<UiVisible>(more).unwrap().0);
        let hit = *app.world().get::<UiHitBox>(advanced).unwrap();
        let position = [hit.rect.x + 2.0, hit.rect.y + 2.0];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: advanced,
                button: UiPointerButton::Primary,
                position,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: advanced,
                button: UiPointerButton::Primary,
                position,
                cancelled: false,
            },
            UiRoutedEvent::Click {
                window,
                target: advanced,
                button: UiPointerButton::Primary,
                click_count: 1,
                position,
            },
        ];
        app.update();
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::MenuCommand { command_id, .. } if command_id == "advanced")));
    }

    #[test]
    fn menu_keyboard_opens_submenus_activates_commands_and_cancels() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let owner = WidgetTree::new(WidgetNode::menu(
            "File",
            [
                crate::MenuEntry::submenu(
                    "more",
                    "More",
                    vec![crate::MenuEntry::command("advanced", "Advanced")],
                ),
                crate::MenuEntry::command("open", "Open"),
            ],
        ))
        .spawn(app.world_mut(), None)
        .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 100.0, 28.0),
            clip: None,
            z: 1,
            content_size: [100.0, 28.0],
            dpi_scale: 1.0,
        });
        let send_key = |app: &mut App, key| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: owner,
                key,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            }];
            app.update();
        };

        send_key(&mut app, UiKeyCode::ArrowDown);
        assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
        send_key(&mut app, UiKeyCode::ArrowDown);
        assert_eq!(
            app.world().get::<MenuControl>(owner).unwrap().session.path,
            [0]
        );
        send_key(&mut app, UiKeyCode::ArrowRight);
        assert_eq!(
            app.world().get::<MenuControl>(owner).unwrap().session.path,
            [0, 0]
        );
        send_key(&mut app, UiKeyCode::Escape);
        assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert_eq!(
            app.world().get::<MenuControl>(owner).unwrap().session.path,
            [0],
            "Escape closes the submenu before closing the popup"
        );
        send_key(&mut app, UiKeyCode::ArrowRight);
        send_key(&mut app, UiKeyCode::Enter);
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::MenuCommand { target, command_id } if *target == owner && command_id == "advanced")));

        send_key(&mut app, UiKeyCode::Space);
        assert!(app.world().get::<MenuControl>(owner).unwrap().session.open);
        send_key(&mut app, UiKeyCode::Escape);
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
    }

    #[test]
    fn menu_bar_arrow_keys_switch_open_menu_and_focus() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut().init_resource::<UiFocus>();
        let window = app.world_mut().spawn_empty().id();
        let root = WidgetTree::new(WidgetNode::menu_bar([
            WidgetNode::menu("File", [crate::MenuEntry::command("open", "Open")]),
            WidgetNode::menu("Edit", [crate::MenuEntry::command("copy", "Copy")]),
        ]))
        .spawn(app.world_mut(), None)
        .unwrap();
        let menus = app.world().get::<UiChildren>(root).unwrap().0.clone();
        assert_eq!(menus.len(), 2);
        for (index, menu) in menus.iter().enumerate() {
            app.world_mut().entity_mut(*menu).insert(UiLayoutResult {
                window,
                rect: UiRect::new(index as f32 * 90.0, 0.0, 90.0, 28.0),
                clip: None,
                z: 1,
                content_size: [90.0, 28.0],
                dpi_scale: 1.0,
            });
        }
        let send_key = |app: &mut App, target, key| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target,
                key,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            }];
            app.update();
        };

        send_key(&mut app, menus[0], UiKeyCode::ArrowDown);
        assert!(
            app.world()
                .get::<MenuControl>(menus[0])
                .unwrap()
                .session
                .open
        );
        send_key(&mut app, menus[0], UiKeyCode::ArrowRight);
        assert!(
            !app.world()
                .get::<MenuControl>(menus[0])
                .unwrap()
                .session
                .open
        );
        assert!(
            app.world()
                .get::<MenuControl>(menus[1])
                .unwrap()
                .session
                .open
        );
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(menus[1])
        );
        send_key(&mut app, menus[1], UiKeyCode::ArrowLeft);
        assert!(
            app.world()
                .get::<MenuControl>(menus[0])
                .unwrap()
                .session
                .open
        );
        assert!(
            !app.world()
                .get::<MenuControl>(menus[1])
                .unwrap()
                .session
                .open
        );
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(menus[0])
        );
    }

    #[test]
    fn menu_drag_open_picks_under_release_and_restores_trigger_focus() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let owner = WidgetTree::new(
            WidgetNode::menu(
                "File",
                [
                    crate::MenuEntry::command("open", "Open"),
                    crate::MenuEntry::command("save", "Save"),
                ],
            )
            .with_key("file-menu"),
        )
        .spawn(app.world_mut(), None)
        .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(20.0, 20.0, 100.0, 28.0),
            clip: None,
            z: 2,
            content_size: [100.0, 28.0],
            dpi_scale: 1.0,
        });
        app.world_mut().entity_mut(owner).insert(UiHitBox {
            window: Some(window),
            rect: UiRect::new(20.0, 20.0, 100.0, 28.0),
            clip: None,
            z_index: 2,
        });
        app.world_mut().insert_resource(UiFocus::default());
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(owner);

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: [28.0, 28.0],
            }];
        app.update();
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: [34.0, 48.0],
                delta: [6.0, 20.0],
                dragging: true,
            }];
        app.update();
        let menu = app.world().get::<MenuControl>(owner).unwrap();
        assert!(menu.session.open);
        assert!(menu.opened_by_pointer);
        let save = menu
            .items
            .iter()
            .copied()
            .find(|entity| {
                app.world()
                    .get::<MenuItemControl>(*entity)
                    .is_some_and(|item| item.id == "save")
            })
            .unwrap();
        let hit = *app.world().get::<UiHitBox>(save).unwrap();
        let position = [
            hit.rect.x + hit.rect.width * 0.5,
            hit.rect.y + hit.rect.height * 0.5,
        ];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position,
                delta: [position[0] - 34.0, position[1] - 48.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position,
                cancelled: false,
            },
        ];
        app.update();

        let menu = app.world().get::<MenuControl>(owner).unwrap();
        assert!(!menu.session.open);
        assert!(!menu.opened_by_pointer);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .and_then(|state| state.focused),
            Some(owner)
        );
        assert!(
            app.world_mut()
                .resource_mut::<WidgetEventQueue>()
                .take()
                .iter()
                .any(
                    |event| matches!(event, WidgetEvent::MenuCommand { target, command_id }
                if *target == owner && command_id == "save")
                )
        );
    }

    #[test]
    fn context_menu_anchors_at_pointer_flips_and_outside_click_closes_without_stealing_focus() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(UiLayoutRoot::new(
            window,
            Rect::new(0.0, 0.0, 320.0, 220.0),
            1.0,
        ));
        let owner = WidgetTree::new(
            WidgetNode::context_menu(
                "Object",
                [
                    crate::MenuEntry::command("duplicate", "Duplicate"),
                    crate::MenuEntry::submenu(
                        "convert",
                        "Convert",
                        vec![crate::MenuEntry::command("mesh", "Mesh")],
                    ),
                ],
            )
            .with_key("object-context"),
        )
        .spawn(app.world_mut(), None)
        .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(40.0, 30.0, 100.0, 28.0),
            clip: None,
            z: 2,
            content_size: [100.0, 28.0],
            dpi_scale: 1.0,
        });
        app.world_mut().entity_mut(owner).insert(UiHitBox {
            window: Some(window),
            rect: UiRect::new(40.0, 30.0, 100.0, 28.0),
            clip: None,
            z_index: 2,
        });
        app.world_mut().insert_resource(UiFocus::default());

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Secondary,
                position: [310.0, 210.0],
            }];
        app.update();
        let (popup, items) = {
            let menu = app.world().get::<MenuControl>(owner).unwrap();
            (menu.popup, menu.items.clone())
        };
        assert_eq!(popup.anchor, Rect::new(310.0, 210.0, 1.0, 1.0));
        assert_eq!(popup.side, crate::menus::PopupSide::Above);
        assert!(popup.bounds.x >= 0.0);
        assert!(popup.bounds.y >= 0.0);
        assert!(popup.bounds.x + popup.bounds.width <= 320.0);
        assert!(popup.bounds.y + popup.bounds.height <= 220.0);
        let convert = items
            .iter()
            .copied()
            .find(|entity| {
                app.world()
                    .get::<MenuItemControl>(*entity)
                    .is_some_and(|item| item.id == "convert")
            })
            .unwrap();
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerEnter {
                window,
                target: convert,
            }];
        app.update();
        let mesh = items
            .iter()
            .copied()
            .find(|entity| {
                app.world()
                    .get::<MenuItemControl>(*entity)
                    .is_some_and(|item| item.id == "mesh")
            })
            .unwrap();
        let mesh_rect = app.world().get::<UiLayoutResult>(mesh).unwrap().rect;
        assert!(mesh_rect.x >= 0.0 && mesh_rect.x + mesh_rect.width <= 320.0);

        let outside = app
            .world_mut()
            .spawn((UiNode, UiFocusable, UiKey("outside".into())))
            .id();
        app.world_mut().entity_mut(outside).insert(UiLayoutResult {
            window,
            rect: UiRect::new(8.0, 8.0, 20.0, 20.0),
            clip: None,
            z: 20,
            content_size: [20.0, 20.0],
            dpi_scale: 1.0,
        });
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(outside);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: outside,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [10.0, 10.0],
        }];
        app.update();
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .and_then(|state| state.focused),
            Some(outside)
        );
    }

    #[test]
    fn pie_menu_pointer_dead_zone_keyboard_and_cancel_are_routed() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut()
            .resource_mut::<Theme>()
            .spacing
            .menu
            .pie_inner_inset = 23.0;
        app.world_mut()
            .resource_mut::<Theme>()
            .spacing
            .menu
            .pie_popup_diameter = 180.0;
        app.world_mut()
            .resource_mut::<Theme>()
            .spacing
            .menu
            .pie_inner_radius = 14.0;
        app.world_mut()
            .resource_mut::<Theme>()
            .spacing
            .menu
            .pie_outer_radius = 78.0;
        app.world_mut()
            .resource_mut::<Theme>()
            .spacing
            .menu
            .pie_outer_item_radius = 66.0;
        app.world_mut()
            .resource_mut::<Theme>()
            .spacing
            .menu
            .pie_item_max_width = 110.0;
        let window = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(UiLayoutRoot::new(
            window,
            Rect::new(0.0, 0.0, 320.0, 220.0),
            1.0,
        ));
        let owner = WidgetTree::new(
            WidgetNode::pie_menu(
                "Add",
                [
                    crate::MenuEntry::command("cube", "Cube"),
                    crate::MenuEntry::command("sphere", "Sphere"),
                    crate::MenuEntry::command("cone", "Cone"),
                    crate::MenuEntry::command("torus", "Torus"),
                ],
            )
            .with_key("add-pie"),
        )
        .spawn(app.world_mut(), None)
        .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(100.0, 80.0, 40.0, 40.0),
            clip: None,
            z: 2,
            content_size: [40.0, 40.0],
            dpi_scale: 1.0,
        });
        app.world_mut().entity_mut(owner).insert(UiHitBox {
            window: Some(window),
            rect: UiRect::new(100.0, 80.0, 40.0, 40.0),
            clip: None,
            z_index: 2,
        });
        app.world_mut().insert_resource(UiFocus::default());
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(owner);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: owner,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [110.0, 90.0],
        }];
        app.update();
        advance_menu_animations_by(app.world_mut(), 1.0);
        app.world_mut().increment_change_tick();
        collect_widget_paint(app.world_mut());
        let center = app
            .world()
            .get::<MenuControl>(owner)
            .unwrap()
            .pie
            .unwrap()
            .center;
        let pie_bounds = app.world().get::<MenuControl>(owner).unwrap().popup.bounds;
        let background = app.world().resource::<Theme>().background;
        assert_eq!(pie_bounds.width, 180.0);
        assert_eq!(
            app.world()
                .get::<MenuControl>(owner)
                .unwrap()
                .pie
                .unwrap()
                .radius,
            76.0
        );
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(
                    item.primitive,
                    Primitive::Ellipse { rect, color }
                        if color == background
                            && rect.x == pie_bounds.x + 23.0
                            && rect.y == pie_bounds.y + 23.0
                            && rect.width == pie_bounds.width - 46.0
                            && rect.height == pie_bounds.height - 46.0
                ))
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: [center[0], center[1] - 32.0],
                delta: [0.0, 0.0],
                dragging: false,
            }];
        app.update();
        assert_eq!(
            app.world()
                .get::<MenuControl>(owner)
                .unwrap()
                .pie
                .unwrap()
                .selected,
            Some(0)
        );
        assert_eq!(
            app.world().get::<MenuControl>(owner).unwrap().session.path,
            [0]
        );
        let root_item = app
            .world()
            .get::<MenuControl>(owner)
            .unwrap()
            .items
            .iter()
            .copied()
            .find(|entity| {
                app.world()
                    .get::<MenuItemControl>(*entity)
                    .is_some_and(|item| item.id == "cube")
            })
            .unwrap();
        let root_item_rect = app.world().get::<UiLayoutResult>(root_item).unwrap().rect;
        assert_eq!(root_item_rect.width, 110.0);
        assert!((root_item_rect.y + root_item_rect.height * 0.5 - (center[1] - 66.0)).abs() < 0.01);
        assert!(app.world().get::<UiVisible>(root_item).unwrap().0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: center,
                delta: [0.0, 0.0],
                dragging: false,
            }];
        app.update();
        assert_eq!(
            app.world()
                .get::<MenuControl>(owner)
                .unwrap()
                .pie
                .unwrap()
                .selected,
            None
        );
        assert!(
            app.world()
                .get::<MenuControl>(owner)
                .unwrap()
                .session
                .path
                .is_empty()
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowDown,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world().get::<MenuControl>(owner).unwrap().session.path,
            [0]
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Escape,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .and_then(|state| state.focused),
            Some(owner)
        );
    }

    #[test]
    fn clear_menu_history_action_resets_recency_without_affecting_other_menus() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let entries = [
            crate::MenuEntry::command("a", "Select"),
            crate::MenuEntry::command("z", "Select"),
        ];
        let first = app
            .world_mut()
            .spawn(MenuControl::new(
                "First",
                crate::MenuTree::new(entries.to_vec()),
                MenuDisplay::Dropdown,
            ))
            .id();
        let second = app
            .world_mut()
            .spawn(MenuControl::new(
                "Second",
                crate::MenuTree::new(entries.to_vec()),
                MenuDisplay::Dropdown,
            ))
            .id();
        app.world_mut()
            .get_mut::<MenuControl>(first)
            .unwrap()
            .search
            .pick("z");
        app.world_mut()
            .get_mut::<MenuControl>(second)
            .unwrap()
            .search
            .pick("z");
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(first, WidgetActionKind::ClearMenuHistory));
        app.update();
        let first_rank = app
            .world()
            .get::<MenuControl>(first)
            .unwrap()
            .search
            .rank("select", [("z", "Select"), ("a", "Select")]);
        let second_rank = app
            .world()
            .get::<MenuControl>(second)
            .unwrap()
            .search
            .rank("select", [("z", "Select"), ("a", "Select")]);
        assert_eq!(first_rank, ["a", "z"]);
        assert_eq!(second_rank, ["z", "a"]);
    }

    #[test]
    fn tooltip_waits_for_pointer_dwell_then_paints_and_hides_on_leave() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(UiLayoutRoot::new(
            window,
            Rect::new(0.0, 0.0, 180.0, 100.0),
            1.0,
        ));
        let host = WidgetTree::new(WidgetNode::tooltip(
            "Applies the current settings",
            WidgetNode::button("Apply").with_key("apply-button"),
        ))
        .spawn(app.world_mut(), None)
        .unwrap();
        let child = app.world().get::<UiChildren>(host).unwrap().0[0];
        app.world_mut().entity_mut(host).insert(UiLayoutResult {
            window,
            rect: UiRect::new(140.0, 72.0, 32.0, 20.0),
            clip: None,
            z: 2,
            content_size: [32.0, 20.0],
            dpi_scale: 1.0,
        });
        app.world_mut().entity_mut(child).insert(UiLayoutResult {
            window,
            rect: UiRect::new(140.0, 72.0, 32.0, 20.0),
            clip: None,
            z: 3,
            content_size: [32.0, 20.0],
            dpi_scale: 1.0,
        });
        app.world_mut()
            .get_mut::<Tooltip>(host)
            .unwrap()
            .delay_millis = 200;
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerEnter {
                window,
                target: child,
            }];
        app.update();
        assert!(!app.world().get::<Tooltip>(host).unwrap().visible);

        advance_tooltips_by(app.world_mut(), 0.25);
        assert!(app.world().get::<Tooltip>(host).unwrap().visible);
        collect_widget_paint(app.world_mut());
        let painted = app.world().resource::<DisplayList>().widget_items();
        assert!(painted.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, .. } if text == "Applies the current settings"
        )));
        assert!(painted.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect, .. }
                if rect.x >= 0.0 && rect.x + rect.width <= 180.0
        )));

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerLeave {
                window,
                target: child,
            }];
        app.update();
        assert!(!app.world().get::<Tooltip>(host).unwrap().visible);
        assert!(
            !app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(
                    &item.primitive,
                    Primitive::TextWrapped { text, .. } if text == "Applies the current settings"
                ))
        );
    }

    #[test]
    fn ctrl_click_exclusive_panel_collapses_siblings_and_advances_animation() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let tree = WidgetTree::new(WidgetNode::column([
            WidgetNode::panel("One", false, [WidgetNode::button("One child")]).with_key("one"),
            WidgetNode::panel("Two", false, [WidgetNode::button("Two child")]).with_key("two"),
        ]));
        let root = tree.spawn(app.world_mut(), None).unwrap();
        app.world_mut()
            .entity_mut(root)
            .insert(PanelGroup::with_panels(["one", "two"]));
        let panels = app
            .world()
            .get::<bui_core::UiChildren>(root)
            .unwrap()
            .0
            .clone();
        let child = app
            .world()
            .get::<bui_core::UiChildren>(panels[1])
            .unwrap()
            .0[0];
        for (index, panel_entity) in panels.iter().copied().enumerate() {
            {
                let mut panel = app.world_mut().get_mut::<Panel>(panel_entity).unwrap();
                panel.exclusive = true;
                panel.persistence_key = Some(if index == 0 { "one" } else { "two" }.into());
                if index == 0 {
                    panel.state.set_expanded(true);
                }
            }
            app.world_mut().entity_mut(panel_entity).insert((
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, index as f32 * 120.0, 180.0, 120.0),
                    clip: None,
                    z: index as i32,
                    content_size: [180.0, 120.0],
                    dpi_scale: 1.0,
                },
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(0.0, index as f32 * 120.0, 180.0, 120.0),
                    clip: None,
                    z_index: index as i32,
                },
            ));
        }
        app.world_mut().entity_mut(child).insert((
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 144.0, 180.0, 80.0),
                clip: None,
                z: 3,
                content_size: [180.0, 80.0],
                dpi_scale: 1.0,
            },
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 144.0, 180.0, 80.0),
                clip: None,
                z_index: 3,
            },
        ));
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::Key {
                window,
                target: panels[1],
                key: UiKeyCode::Other(17),
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..Default::default()
                },
            },
            UiRoutedEvent::Click {
                window,
                target: panels[1],
                button: UiPointerButton::Primary,
                click_count: 1,
                position: [8.0, 128.0],
            },
        ];
        app.update();

        assert!(
            !app.world()
                .get::<Panel>(panels[0])
                .unwrap()
                .state
                .target_expanded
        );
        let expanded = app.world().get::<Panel>(panels[1]).unwrap();
        assert!(expanded.state.target_expanded);
        assert!(expanded.state.progress > 0.0 && expanded.state.progress < 1.0);
        assert!(app.world().get::<UiVisible>(child).unwrap().0);
        let child_hit = app.world().get::<UiHitBox>(child).unwrap();
        assert!(
            child_hit
                .clip
                .is_some_and(|clip| clip.height < child_hit.rect.height)
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelGroupCommitted { changes } if changes.len() == 2 && changes.iter().any(|(entity, before, after)| *entity == panels[0] && *before && !*after) && changes.iter().any(|(entity, before, after)| *entity == panels[1] && !*before && *after))));
    }

    #[test]
    fn exclusive_panels_with_duplicate_keys_stay_inside_their_group() {
        let mut world = World::new();
        let first_root = world
            .spawn((UiNode, PanelGroup::with_panels(["shared", "first-local"])))
            .id();
        let second_root = world
            .spawn((UiNode, PanelGroup::with_panels(["shared", "second-local"])))
            .id();
        let first_shared = world.spawn((UiNode, Panel::new("Shared", false))).id();
        let first_local = world.spawn((UiNode, Panel::new("First local", true))).id();
        let second_shared = world.spawn((UiNode, Panel::new("Shared", false))).id();
        let second_local = world.spawn((UiNode, Panel::new("Second local", true))).id();
        for (entity, key) in [
            (first_shared, "shared"),
            (first_local, "first-local"),
            (second_shared, "shared"),
            (second_local, "second-local"),
        ] {
            let mut panel = world.get_mut::<Panel>(entity).unwrap();
            panel.exclusive = true;
            panel.persistence_key = Some(key.to_owned());
        }
        world
            .get_mut::<PanelGroup>(second_root)
            .unwrap()
            .exclusive_open = Some("second-local".to_owned());
        bui_core::set_parent(&mut world, first_shared, first_root).unwrap();
        bui_core::set_parent(&mut world, first_local, first_root).unwrap();
        bui_core::set_parent(&mut world, second_shared, second_root).unwrap();
        bui_core::set_parent(&mut world, second_local, second_root).unwrap();
        let mut events = Vec::new();

        toggle_exclusive_panel(&mut world, first_shared, &mut events);

        assert!(
            world
                .get::<Panel>(first_shared)
                .unwrap()
                .state
                .target_expanded
        );
        assert!(
            !world
                .get::<Panel>(first_local)
                .unwrap()
                .state
                .target_expanded
        );
        assert!(
            !world
                .get::<Panel>(second_shared)
                .unwrap()
                .state
                .target_expanded
        );
        assert!(
            world
                .get::<Panel>(second_local)
                .unwrap()
                .state
                .target_expanded
        );
        assert_eq!(
            world
                .get::<PanelGroup>(second_root)
                .unwrap()
                .exclusive_open
                .as_deref(),
            Some("second-local")
        );
        assert!(matches!(
            events.as_slice(),
            [WidgetEvent::PanelGroupCommitted { changes }]
                if changes.len() == 2
                    && changes.iter().any(|(entity, before, after)| *entity == first_shared && !*before && *after)
                    && changes.iter().any(|(entity, before, after)| *entity == first_local && *before && !*after)
        ));
    }

    #[test]
    fn ordinary_panel_toggle_is_one_reversible_value_edit() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let panel = app.world_mut().spawn(Panel::new("Details", false)).id();
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
        app.update();
        assert!(
            app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        let route_history = |app: &mut App, redo: bool| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window: Entity::PLACEHOLDER,
                target: panel,
                key: if redo { UiKeyCode::Y } else { UiKeyCode::Z },
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            }];
            app.update();
        };
        route_history(&mut app, false);
        assert!(
            !app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );
        route_history(&mut app, true);
        assert!(
            app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window: Entity::PLACEHOLDER,
            target: panel,
            key: UiKeyCode::Space,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert!(
            !app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window: Entity::PLACEHOLDER,
            target: panel,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert!(
            app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );
    }

    #[test]
    fn reorderable_panel_group_routes_keyboard_drag_cancel_and_persists_order() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, bui_layout::LayoutPlugin, WidgetPlugin));
        let window = app.world_mut().spawn_empty().id();
        let make_group = |world: &mut World, order: [&str; 3]| {
            let group = world
                .spawn((
                    UiNode,
                    UiKey("settings".into()),
                    UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 240.0, 300.0), 1.0),
                    UiLayoutNode::column(4.0),
                    PanelGroup::with_panels(order).with_reordering("settings"),
                ))
                .id();
            let mut panels = HashMap::new();
            for key in order {
                let mut panel = Panel::new(key, false);
                panel.persistence_key = Some(key.to_owned());
                let entity = world
                    .spawn((
                        UiNode,
                        UiVisible(true),
                        UiEnabled(true),
                        UiFocusable,
                        UiKey(key.to_owned()),
                        UiLayoutNode::leaf(
                            bui_layout::UiLength::Fill(1.0),
                            bui_layout::UiLength::Px(80.0),
                        ),
                        panel,
                    ))
                    .id();
                set_parent(world, entity, group).unwrap();
                panels.insert(key.to_owned(), entity);
            }
            (group, panels)
        };
        let (group, panels) = make_group(app.world_mut(), ["alpha", "beta", "gamma"]);
        app.update();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(panels["alpha"]);

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Key {
                window,
                key: UiKeyCode::ArrowDown,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            });
        app.update();
        app.update();
        let after_keyboard = vec!["beta".to_owned(), "alpha".to_owned(), "gamma".to_owned()];
        assert_eq!(
            app.world().get::<PanelGroup>(group).unwrap().panel_keys,
            after_keyboard
        );
        assert_eq!(
            app.world().get::<UiChildren>(group).unwrap().0,
            [panels["beta"], panels["alpha"], panels["gamma"]]
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelOrderCommitted { group: target, persistence_key, before, after } if *target == group && persistence_key == "settings" && before == &["alpha", "beta", "gamma"] && after == &after_keyboard)));
        assert_eq!(
            app.world().resource::<PanelOrderStore>().get("settings"),
            Some(after_keyboard.as_slice())
        );

        let alpha_y = app
            .world()
            .get::<UiLayoutResult>(panels["alpha"])
            .unwrap()
            .rect
            .y;
        let gamma_y = app
            .world()
            .get::<UiLayoutResult>(panels["gamma"])
            .unwrap()
            .rect
            .y;
        let beta_y = app
            .world()
            .get::<UiLayoutResult>(panels["beta"])
            .unwrap()
            .rect
            .y;
        assert!(
            beta_y < alpha_y && alpha_y < gamma_y,
            "sibling layout should follow keyed order: beta={beta_y}, alpha={alpha_y}, gamma={gamma_y}"
        );

        let start = [8.0, alpha_y + 8.0];
        let end = [8.0, 290.0];
        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: start,
                time_seconds: 1.0,
            },
            UiInputEvent::PointerMove {
                window,
                position: end,
                time_seconds: 1.2,
            },
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: false,
                position: end,
                time_seconds: 1.3,
            },
        ]);
        app.update();
        let after_drag = vec!["beta".to_owned(), "gamma".to_owned(), "alpha".to_owned()];
        assert_eq!(
            app.world().get::<PanelGroup>(group).unwrap().panel_keys,
            after_drag
        );
        assert!(
            !app.world()
                .get::<Panel>(panels["alpha"])
                .unwrap()
                .state
                .target_expanded
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::PanelOrderCommitted { group: target, before, after, .. } if *target == group && before == &after_keyboard && after == &after_drag)));
        assert_eq!(
            app.world().resource::<PanelOrderStore>().get("settings"),
            Some(after_drag.as_slice())
        );

        let beta = panels["beta"];
        let beta_y = app.world().get::<UiLayoutResult>(beta).unwrap().rect.y;
        app.world_mut().resource_mut::<UiInputQueue>().extend([
            UiInputEvent::PointerButton {
                window,
                button: UiPointerButton::Primary,
                pressed: true,
                position: [8.0, beta_y + 8.0],
                time_seconds: 2.0,
            },
            UiInputEvent::PointerMove {
                window,
                position: [8.0, 250.0],
                time_seconds: 2.2,
            },
        ]);
        app.update();
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerCancel { window });
        app.update();
        assert_eq!(
            app.world().get::<PanelGroup>(group).unwrap().panel_keys,
            after_drag
        );
        assert_eq!(
            app.world().resource::<PanelOrderStore>().get("settings"),
            Some(after_drag.as_slice()),
            "cancel restores the persisted committed order"
        );

        for entity in [panels["alpha"], panels["beta"], panels["gamma"], group] {
            app.world_mut().despawn(entity);
        }
        let (rebuilt, rebuilt_panels) = make_group(app.world_mut(), ["alpha", "beta", "gamma"]);
        app.update();
        app.update();
        assert_eq!(
            app.world().get::<PanelGroup>(rebuilt).unwrap().panel_keys,
            after_drag
        );
        assert_eq!(
            app.world().get::<UiChildren>(rebuilt).unwrap().0,
            [
                rebuilt_panels["beta"],
                rebuilt_panels["gamma"],
                rebuilt_panels["alpha"]
            ]
        );
        assert_eq!(
            app.world().get::<UiKey>(rebuilt).unwrap().0,
            "settings",
            "the reorder group's stable UiKey survives tree reconstruction"
        );
    }

    #[test]
    fn panel_widget_system_snaps_for_zero_scale_and_reduced_motion() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut()
            .init_resource::<bui_style::UiReducedMotion>();
        let panel = app.world_mut().spawn(Panel::new("Motion", false)).id();

        app.world_mut().resource_mut::<Theme>().motion_scale = 0.0;
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
        app.update();
        let state = &app.world().get::<Panel>(panel).unwrap().state;
        assert!(state.target_expanded);
        assert_eq!(state.progress, 1.0);

        app.world_mut().resource_mut::<Theme>().motion_scale = 1.0;
        app.world_mut()
            .resource_mut::<bui_style::UiReducedMotion>()
            .0 = true;
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
        app.update();
        let state = &app.world().get::<Panel>(panel).unwrap().state;
        assert!(!state.target_expanded);
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn panel_widget_uses_typed_theme_duration_and_leaves_settled_state_unchanged() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, WidgetPlugin))
            .init_resource::<bevy_time::Time>()
            .init_resource::<bui_style::UiReducedMotion>();
        let panel = app.world_mut().spawn(Panel::new("Motion", false)).id();
        app.world_mut().resource_mut::<Theme>().motion.panel_expand =
            std::time::Duration::from_millis(400);
        app.update();

        let panel_id = app.world().components().component_id::<Panel>().unwrap();
        let changed_before = app
            .world()
            .get_entity(panel)
            .unwrap()
            .get_change_ticks_by_id(panel_id)
            .unwrap()
            .changed
            .get();
        app.update();
        let changed_after = app
            .world()
            .get_entity(panel)
            .unwrap()
            .get_change_ticks_by_id(panel_id)
            .unwrap()
            .changed
            .get();
        assert_eq!(changed_after, changed_before, "settled panels stay clean");

        app.world_mut()
            .resource_mut::<bevy_time::Time>()
            .advance_by(std::time::Duration::from_millis(200));
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
        app.update();
        let state = &app.world().get::<Panel>(panel).unwrap().state;
        assert_eq!(state.animation_seconds, 0.4);
        assert!((state.progress - 0.5).abs() < 0.001);

        app.world_mut()
            .resource_mut::<bui_style::UiReducedMotion>()
            .0 = true;
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(panel, WidgetActionKind::TogglePanel));
        app.update();
        let state = &app.world().get::<Panel>(panel).unwrap().state;
        assert_eq!(state.progress, 0.0, "reduced motion keeps toggling usable");
    }

    #[test]
    fn menu_and_node_graph_painters_consume_semantic_theme_tokens() {
        for kind in [bui_style::ThemeKind::Dark, bui_style::ThemeKind::Light] {
            let theme = Theme::new(kind);
            let rect = UiRect::new(20.0, 30.0, 360.0, 240.0);
            let clip = ClipRect::from(rect);
            let mut graph_paint = Vec::new();
            paint_node_graph(
                NodeGraphPaint {
                    graph: &NodeGraph::default(),
                    state: NodeGraphEditorState {
                        gesture: Some(NodeGraphGesture::BoxSelect {
                            start_pointer: [60.0, 100.0],
                            current_pointer: [140.0, 160.0],
                            additive: false,
                            before: String::new(),
                        }),
                    },
                    output_label_widths: HashMap::new(),
                    canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
                    label: "Graph",
                    rect,
                    clip,
                    z: 1,
                    theme: &theme,
                    font: ui_default_text_font(theme.font_size),
                },
                &mut graph_paint,
            );
            assert!(graph_paint.iter().any(|item| matches!(
                item.primitive,
                Primitive::Ellipse { color, .. }
                    if color == theme.color(ThemeColorToken::NodeGraphGridLine).mix(theme.color(ThemeColorToken::NodeGraphSurface), 0.6)
            )));
            assert!(graph_paint.iter().any(|item| matches!(
                item.primitive,
                Primitive::RoundedRect { color, .. }
                    if color == theme.color(ThemeColorToken::NodeGraphSelectionOverlay)
            )));

            let mut app = App::new();
            app.add_plugins(WidgetPlugin);
            *app.world_mut().resource_mut::<Theme>() = theme.clone();
            let window = app.world_mut().spawn_empty().id();
            let mut menu = MenuControl::new(
                "File",
                crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
                MenuDisplay::Dropdown,
            );
            menu.open(None);
            menu.popup.open = true;
            menu.popup.bounds = UiRect::new(8.0, 32.0, 100.0, 44.0);
            app.world_mut().spawn((
                UiNode,
                UiLayoutResult {
                    window,
                    rect: UiRect::new(8.0, 4.0, 100.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [100.0, 24.0],
                    dpi_scale: 1.0,
                },
                menu,
            ));
            collect_widget_paint(app.world_mut());
            assert!(app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(
                    item.primitive,
                    Primitive::Line { color, .. } if color == theme.color(ThemeColorToken::MenuBorder)
                )));
        }
    }

    #[test]
    fn node_graph_painter_keeps_category_headers_and_clips_zoomed_text() {
        let theme = Theme::new(bui_style::ThemeKind::Dark);
        let mut graph = NodeGraph::default();
        let node_id = graph.add_node_with_category(
            "A deliberately long texture node title",
            [18.0, 20.0],
            vec![
                NodeSocket::input("A deliberately long input label", crate::SocketType::Float),
                NodeSocket::output("A deliberately long output label", crate::SocketType::Color),
            ],
            NodeCategory::Texture,
        );
        graph.zoom = 1.5;
        graph.selected.insert(node_id);
        let rect = Rect::new(20.0, 16.0, 360.0, 240.0);
        let clip = ClipRect::from(rect);
        let node = graph.nodes.iter().find(|node| node.id == node_id).unwrap();
        let node_rect = node_graph_node_rect(&graph, rect, node, &theme.spacing.node_graph);
        let mut items = Vec::new();
        paint_node_graph(
            NodeGraphPaint {
                graph: &graph,
                state: NodeGraphEditorState::default(),
                output_label_widths: HashMap::from([((node_id, 1), 44.0)]),
                canvas_surface: theme.color(ThemeColorToken::NodeGraphSurface),
                label: "Texture graph",
                rect,
                clip,
                z: 1,
                theme: &theme,
                font: ui_default_text_font(theme.font_size),
            },
            &mut items,
        );

        let category_header = node_graph_category_color(NodeCategory::Texture, &theme);
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect: header, color, .. }
                if (header.x - node_rect.x).abs() < 0.01
                    && (header.y - node_rect.y).abs() < 0.01
                    && (header.height - theme.spacing.node_graph.header_height * graph.zoom).abs() < 0.01
                    && color == category_header
        )));
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::Line { color, width, .. }
                if color == theme.accent && (width - 2.0).abs() < 0.01
        )));

        for text in [
            "A deliberately long texture node title",
            "A deliberately long input label",
            "A deliberately long output label",
        ] {
            let draw = items
                .iter()
                .find(|item| matches!(&item.primitive, Primitive::TextWrapped { text: candidate, .. } if candidate == text))
                .unwrap_or_else(|| panic!("missing node graph label {text:?}"));
            assert!(draw.clip.x >= node_rect.x - 0.01);
            assert!(draw.clip.x + draw.clip.width <= node_rect.x + node_rect.width + 0.01);
            assert!(draw.clip.y >= node_rect.y - 0.01);
            assert!(draw.clip.y + draw.clip.height <= node_rect.y + node_rect.height + 0.01);
            let Primitive::TextWrapped {
                max_width, style, ..
            } = &draw.primitive
            else {
                unreachable!();
            };
            assert!(*max_width <= node_rect.width);
            let font_size = style.as_ref().unwrap().font.as_ref().unwrap().font_size;
            assert!(
                matches!(font_size, FontSize::Px(size) if (size - theme.font_size * graph.zoom).abs() < 0.01)
            );
        }
        let output = items
            .iter()
            .find(|item| {
                matches!(&item.primitive, Primitive::TextWrapped { text, .. } if text == "A deliberately long output label")
            })
            .unwrap();
        let Primitive::TextWrapped {
            origin, max_width, ..
        } = &output.primitive
        else {
            unreachable!();
        };
        let content_right = node_rect.x + node_rect.width
            - theme.spacing.node_graph.socket_label_inset_x * graph.zoom;
        assert!((origin[0] - (content_right - 45.0)).abs() < 0.01);
        assert!((*max_width - 45.0).abs() < 0.01);
        assert!(output.clip.x >= node_rect.x + node_rect.width * 0.49);
    }

    #[test]
    fn node_graph_default_height_tracks_socket_rows_and_preserves_explicit_size() {
        let theme = Theme::new(bui_style::ThemeKind::Dark);
        let mut graph = NodeGraph::default();
        let one_row = graph.add_node(
            "One row",
            [10.0, 10.0],
            vec![NodeSocket::input("Input", crate::SocketType::Float)],
        );
        let two_rows = graph.add_node(
            "Two rows",
            [220.0, 10.0],
            vec![
                NodeSocket::input("First", crate::SocketType::Float),
                NodeSocket::input("Second", crate::SocketType::Float),
            ],
        );
        let rect = Rect::new(0.0, 0.0, 500.0, 300.0);
        let node = graph.nodes.iter().find(|node| node.id == one_row).unwrap();
        assert_eq!(node.size[1], 58.0);
        assert_eq!(
            node_graph_node_rect(&graph, rect, node, &theme.spacing.node_graph).height,
            58.0
        );
        let node = graph.nodes.iter().find(|node| node.id == two_rows).unwrap();
        assert_eq!(
            node_graph_node_rect(&graph, rect, node, &theme.spacing.node_graph).height,
            80.0
        );
        graph.nodes[0].collapsed = true;
        assert_eq!(
            node_graph_node_rect(&graph, rect, &graph.nodes[0], &theme.spacing.node_graph).height,
            theme.spacing.node_graph.header_height
        );
        graph.nodes[1].size[1] = 126.0;
        assert_eq!(
            node_graph_node_rect(&graph, rect, &graph.nodes[1], &theme.spacing.node_graph).height,
            126.0
        );
    }

    #[test]
    fn node_graph_canvas_uses_its_token_and_respects_explicit_background_styles() {
        let theme = Theme::new(bui_style::ThemeKind::Dark);
        let mut world = World::new();
        let entity = world.spawn((UiNode, UiRole::new("node-graph"))).id();
        let token_surface = theme.color(ThemeColorToken::NodeGraphSurface);
        assert_eq!(
            node_graph_canvas_surface(&world, entity, &theme),
            token_surface
        );
        assert_ne!(token_surface, theme.surface);

        let custom_color = Rgba(0.36, 0.12, 0.18, 1.0);
        let mut inline = UiInlineStyle::default();
        inline.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(custom_color),
        );
        world.entity_mut(entity).insert((
            inline,
            UiComputedStyle {
                properties: HashMap::from([(
                    UiStyleProperty::Background,
                    UiStyleValue::Color(custom_color),
                )]),
            },
        ));
        assert_eq!(
            node_graph_canvas_surface(&world, entity, &theme),
            custom_color
        );

        world.entity_mut(entity).remove::<UiInlineStyle>();
        let mut declaration = UiStyleDeclaration::default();
        declaration.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(custom_color),
        );
        world.insert_resource(UiStyleSheet {
            rules: vec![UiStyleRule {
                selector: UiStyleSelector {
                    role: Some("node-graph".to_owned()),
                    ..Default::default()
                },
                declaration,
                priority: 0,
            }],
        });
        assert_eq!(
            node_graph_canvas_surface(&world, entity, &theme),
            custom_color
        );
    }

    #[test]
    fn panel_pointer_click_toggles_only_from_header() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let panel = app.world_mut().spawn(Panel::new("Details", false)).id();
        app.world_mut().entity_mut(panel).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 180.0, 100.0),
            clip: None,
            z: 1,
            content_size: [180.0, 100.0],
            dpi_scale: 1.0,
        });

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: panel,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [12.0, 45.0],
        }];
        app.update();
        assert!(
            !app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: panel,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [12.0, 12.0],
        }];
        app.update();
        assert!(
            app.world()
                .get::<Panel>(panel)
                .unwrap()
                .state
                .target_expanded
        );
    }

    #[test]
    fn tree_text_field_uses_shared_buffer_and_external_clipboard_requests() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let owner = WidgetTree::new(WidgetNode::text_field("Name", "old"))
            .spawn(app.world_mut(), None)
            .unwrap();
        assert!(
            app.world()
                .get::<bui_text::UiExternalClipboard>(owner)
                .is_some()
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: owner,
            text: "!".into(),
        }];
        bui_text::process_text_events(app.world_mut());
        sync_text_controls(app.world_mut());
        assert_eq!(
            app.world().get::<TextBuffer>(owner).unwrap().as_str(),
            "old!"
        );
        assert_eq!(app.world().get::<TextField>(owner).unwrap().text, "old!");

        app.world_mut()
            .resource_mut::<UiRoutedEvents>()
            .events
            .clear();
        app.world_mut()
            .get_mut::<TextBuffer>(owner)
            .unwrap()
            .select_all();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::X,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..Default::default()
            },
        }];
        process_routed_widget_events(app.world_mut());
        assert_eq!(app.world().get::<TextBuffer>(owner).unwrap().as_str(), "");
        assert!(
            app.world_mut()
                .resource_mut::<ClipboardRequestQueue>()
                .drain()
                .any(|request| request == ClipboardRequest::Write("old!".into()))
        );
    }

    #[test]
    fn curve_editor_paints_gpu_curve_grid_points_and_bezier_handles() {
        let mut app = App::new();
        let (window, entity) =
            spawn_curve_editor_widget(&mut app, UiRect::new(0.0, 0.0, 360.0, 240.0));
        {
            let mut editor = app.world_mut().get_mut::<CurveEditor>(entity).unwrap();
            editor.interpolation = Interpolation::Smooth;
            let id = editor.add_point(0.5, 0.72).unwrap();
            editor.selected = Some(id);
            editor.set_handles(id, [-0.11, -0.08], [0.13, 0.12]);
        }
        app.update();

        let items = app.world().resource::<DisplayList>().widget_items();
        assert!(items.len() >= 24);
        assert!(items.iter().all(|item| item.z >= 11));
        assert!(
            items
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Bezier { .. }))
        );
        assert!(
            items
                .iter()
                .filter(|item| matches!(item.primitive, Primitive::Line { .. }))
                .count()
                >= 12
        );
        assert!(items.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { rect, .. }
                if rect.width
                    == curve_plot_rect(UiRect::new(0.0, 0.0, 360.0, 240.0), &Theme::default())
                        .width
        )));
        assert_ne!(window, Entity::PLACEHOLDER);
    }

    #[test]
    fn curve_editor_shift_drag_adds_one_undoable_point_transaction() {
        let mut app = App::new();
        let rect = UiRect::new(0.0, 0.0, 320.0, 240.0);
        let (window, entity) = spawn_curve_editor_widget(&mut app, rect);
        let plot = curve_plot_rect(rect, &Theme::default());
        let start = [plot.x + plot.width * 0.5, plot.y + plot.height * 0.5];
        let end = [plot.x + plot.width * 0.68, plot.y + plot.height * 0.28];
        app.world_mut()
            .resource_mut::<WidgetInteractionState>()
            .modifiers
            .insert(
                window,
                UiModifiers {
                    shift: true,
                    ..Default::default()
                },
            );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: end,
                delta: [end[0] - start[0], end[1] - start[1]],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Primary,
                position: end,
                cancelled: false,
            },
        ];
        app.update();

        let editor = app.world().get::<CurveEditor>(entity).unwrap();
        assert_eq!(editor.points.len(), 3);
        let selected = editor
            .points
            .iter()
            .find(|point| Some(point.id) == editor.selected)
            .unwrap();
        assert!((selected.position[0] - 0.68).abs() < 0.002);
        assert!((selected.position[1] - 0.72).abs() < 0.002);
        let commits = app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .into_iter()
            .filter(|event| matches!(event, WidgetEvent::CurveEditorCommitted { target, .. } if *target == entity))
            .count();
        assert_eq!(commits, 1);
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().points.len(),
            2
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::Z,
            UiModifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().points.len(),
            3
        );
    }

    #[test]
    fn curve_editor_keyboard_cycles_selection_and_edits_points() {
        let mut app = App::new();
        let (window, entity) =
            spawn_curve_editor_widget(&mut app, UiRect::new(0.0, 0.0, 320.0, 240.0));
        let control = UiModifiers {
            control: true,
            ..Default::default()
        };
        route_curve_key(&mut app, window, entity, UiKeyCode::ArrowRight, control);
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().selected,
            Some(1)
        );
        route_curve_key(&mut app, window, entity, UiKeyCode::ArrowRight, control);
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().selected,
            Some(2)
        );
        route_curve_key(&mut app, window, entity, UiKeyCode::ArrowLeft, control);
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().selected,
            Some(1)
        );

        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::A,
            UiModifiers::default(),
        );
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().points.len(),
            3
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::C,
            UiModifiers::default(),
        );
        assert_eq!(
            app.world()
                .get::<CurveEditor>(entity)
                .unwrap()
                .interpolation,
            Interpolation::Smooth
        );
        let before_y = app
            .world()
            .get::<CurveEditor>(entity)
            .unwrap()
            .points
            .iter()
            .find(|point| {
                Some(point.id) == app.world().get::<CurveEditor>(entity).unwrap().selected
            })
            .unwrap()
            .position[1];
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::ArrowUp,
            UiModifiers::default(),
        );
        let editor = app.world().get::<CurveEditor>(entity).unwrap();
        let selected = editor
            .points
            .iter()
            .find(|point| Some(point.id) == editor.selected)
            .unwrap();
        assert!(selected.position[1] > before_y);
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::Home,
            UiModifiers::default(),
        );
        assert_eq!(
            app.world()
                .get::<CurveEditor>(entity)
                .unwrap()
                .points
                .iter()
                .find(|point| Some(point.id)
                    == app.world().get::<CurveEditor>(entity).unwrap().selected)
                .unwrap()
                .position[0],
            0.0
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::End,
            UiModifiers::default(),
        );
        assert_eq!(
            app.world()
                .get::<CurveEditor>(entity)
                .unwrap()
                .points
                .iter()
                .find(|point| Some(point.id)
                    == app.world().get::<CurveEditor>(entity).unwrap().selected)
                .unwrap()
                .position[0],
            1.0
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::Home,
            UiModifiers::default(),
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::ArrowRight,
            UiModifiers {
                shift: true,
                ..UiModifiers::default()
            },
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::ArrowRight,
            UiModifiers::default(),
        );
        let editor = app.world().get::<CurveEditor>(entity).unwrap();
        let selected = editor
            .points
            .iter()
            .find(|point| Some(point.id) == editor.selected)
            .unwrap();
        assert!(
            (selected.position[0] - 0.11).abs() < 0.001,
            "Shift arrow precision should accumulate to 0.11, got {}",
            selected.position[0]
        );
        route_curve_key(
            &mut app,
            window,
            entity,
            UiKeyCode::Delete,
            UiModifiers::default(),
        );
        assert_eq!(
            app.world().get::<CurveEditor>(entity).unwrap().points.len(),
            2
        );
    }

    #[test]
    fn curve_editor_handle_drag_cancels_or_commits_and_view_zooms_and_pans() {
        let mut app = App::new();
        let rect = UiRect::new(0.0, 0.0, 360.0, 240.0);
        let (window, entity) = spawn_curve_editor_widget(&mut app, rect);
        let id = {
            let mut editor = app.world_mut().get_mut::<CurveEditor>(entity).unwrap();
            editor.interpolation = Interpolation::Smooth;
            let id = editor.add_point(0.5, 0.5).unwrap();
            editor.selected = Some(id);
            id
        };
        let plot = curve_plot_rect(rect, &Theme::default());
        let start = curve_screen_position(
            app.world().get::<CurveEditor>(entity).unwrap(),
            plot,
            [0.65, 0.5],
        );
        let end = [start[0] + 12.0, start[1] - 8.0];
        let original_handle = app
            .world()
            .get::<CurveEditor>(entity)
            .unwrap()
            .points
            .iter()
            .find(|point| point.id == id)
            .unwrap()
            .right_handle;

        for cancelled in [true, false] {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
                UiRoutedEvent::PointerDown {
                    window,
                    target: entity,
                    button: UiPointerButton::Primary,
                    position: start,
                },
                UiRoutedEvent::PointerMoved {
                    window,
                    target: entity,
                    position: end,
                    delta: [end[0] - start[0], end[1] - start[1]],
                    dragging: true,
                },
                UiRoutedEvent::PointerUp {
                    window,
                    target: entity,
                    button: UiPointerButton::Primary,
                    position: end,
                    cancelled,
                },
            ];
            app.update();
            let point = app
                .world()
                .get::<CurveEditor>(entity)
                .unwrap()
                .points
                .iter()
                .find(|point| point.id == id)
                .unwrap();
            if cancelled {
                assert_eq!(point.right_handle, original_handle);
                assert!(
                    app.world_mut()
                        .resource_mut::<WidgetEventQueue>()
                        .take()
                        .iter()
                        .all(|event| !matches!(event, WidgetEvent::CurveEditorCommitted { .. }))
                );
            } else {
                assert_ne!(point.right_handle, original_handle);
                assert!(app
                    .world_mut()
                    .resource_mut::<WidgetEventQueue>()
                    .take()
                    .iter()
                    .any(|event| matches!(event, WidgetEvent::CurveEditorCommitted { target, .. } if *target == entity)));
            }
        }

        let zoom_before = app.world().get::<CurveEditor>(entity).unwrap().zoom;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: entity,
            position: [plot.x + plot.width * 0.4, plot.y + plot.height * 0.6],
            delta: [0.0, 1.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert!(app.world().get::<CurveEditor>(entity).unwrap().zoom > zoom_before);

        let before_pan = app.world().get::<CurveEditor>(entity).unwrap().pan;
        let pan_start = [plot.x + plot.width * 0.5, plot.y + plot.height * 0.5];
        let pan_end = [pan_start[0] + 14.0, pan_start[1] - 9.0];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: entity,
                button: UiPointerButton::Middle,
                position: pan_start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: entity,
                position: pan_end,
                delta: [14.0, -9.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: entity,
                button: UiPointerButton::Middle,
                position: pan_end,
                cancelled: false,
            },
        ];
        app.update();
        assert_ne!(
            app.world().get::<CurveEditor>(entity).unwrap().pan,
            before_pan
        );
    }

    #[test]
    fn text_area_uses_shared_shape_for_caret_paint_and_scrolling() {
        let mut app = App::new();
        app.add_plugins((bui_style::StylePlugin, bui_text::TextPlugin, WidgetPlugin));
        app.init_resource::<UiFocus>();
        app.world_mut()
            .resource_mut::<bevy_asset::Assets<bevy_text::Font>>()
            .insert(
                bevy_asset::AssetId::default(),
                bevy_text::Font::from_bytes(bevy_text::DEFAULT_FONT_DATA.to_vec()),
            )
            .unwrap();
        let window = app.world_mut().spawn_empty().id();
        let owner = WidgetTree::new(WidgetNode::text_area(
            "Notes",
            "first line with enough text\nsecond line",
        ))
        .spawn(app.world_mut(), None)
        .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(20.0, 30.0, 120.0, 44.0),
            clip: None,
            z: 7,
            content_size: [120.0, 44.0],
            dpi_scale: 1.0,
        });
        {
            let mut layout = app.world_mut().get_mut::<UiTextLayout>(owner).unwrap();
            layout.font = Some(bevy_text::TextFont::from_font_size(18.0));
            layout.line_height = Some(LineHeight::Px(24.0));
            layout.letter_spacing = bevy_text::LetterSpacing::Px(0.5);
        }
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(owner);

        app.update();
        bui_text::update_text_layouts(app.world_mut());
        let first_origin = app
            .world()
            .get::<UiTextLayout>(owner)
            .unwrap()
            .text_origin()
            .expect("text layout should publish its origin");
        let text_layout = app.world().get::<UiTextLayout>(owner).unwrap();
        assert!(
            text_layout.layout().is_some(),
            "text area should use Bevy's shared text shaper: {:?}",
            text_layout.last_error()
        );
        let click_position = [first_origin[0] + 90.0, first_origin[1] + 5.0];
        let expected_caret = text_layout.hit_test(click_position).unwrap();
        assert!(expected_caret > 0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: click_position,
            }];
        app.update();
        assert_eq!(
            app.world().get::<TextBuffer>(owner).unwrap().caret(),
            expected_caret
        );

        app.world_mut()
            .resource_mut::<UiRoutedEvents>()
            .events
            .clear();
        app.world_mut()
            .get_mut::<TextBuffer>(owner)
            .unwrap()
            .select_all();
        app.update();
        collect_widget_paint(app.world_mut());
        let paint = app.world().resource::<DisplayList>().widget_items();
        let text = paint.iter().find_map(|item| match &item.primitive {
            Primitive::TextWrapped {
                wrap,
                max_width,
                style: Some(style),
                ..
            } => Some((*wrap, *max_width, style)),
            _ => None,
        });
        let (wrap, max_width, style) = text.expect("editable area should emit wrapped text paint");
        assert!(wrap);
        assert!(max_width > 0.0 && max_width < 120.0);
        assert_eq!(
            style.font.as_ref().unwrap().font_size,
            bevy_text::FontSize::Px(18.0)
        );
        assert_eq!(style.line_height, Some(LineHeight::Px(24.0)));
        assert_eq!(style.letter_spacing, bevy_text::LetterSpacing::Px(0.5));
        assert!(
            paint
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Line { .. }))
        );
        assert!(paint.iter().any(|item| matches!(
            item.primitive,
            Primitive::RoundedRect { color, .. } if color.3 == 0.32
        )));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::ImePreedit {
            window,
            target: owner,
            text: "xy".into(),
            cursor_range: Some([0, 2]),
        }];
        bui_text::process_text_events(app.world_mut());
        bui_text::update_text_layouts(app.world_mut());
        sync_text_controls(app.world_mut());
        collect_widget_paint(app.world_mut());
        let preedit_rects = app
            .world()
            .get::<UiTextLayout>(owner)
            .unwrap()
            .preedit_rects();
        assert!(!preedit_rects.is_empty());
        let preedit_paint = app.world().resource::<DisplayList>().widget_items();
        assert!(preedit_paint.iter().any(|item| {
            let Primitive::Line { from, to, .. } = &item.primitive else {
                return false;
            };
            preedit_rects.iter().any(|rect| {
                let y = rect.y + rect.height.max(1.0) - 1.0;
                (from[0] - rect.x).abs() < 0.01
                    && (from[1] - y).abs() < 0.01
                    && (to[0] - (rect.x + rect.width)).abs() < 0.01
                    && (to[1] - y).abs() < 0.01
            })
        }));
        for kind in [bui_style::ThemeKind::Dark, bui_style::ThemeKind::Light] {
            let theme = Theme::new(kind);
            app.world_mut()
                .resource_mut::<UiRoutedEvents>()
                .events
                .clear();
            *app.world_mut().resource_mut::<Theme>() = theme.clone();
            app.update();
            bui_style::resolve_ui_styles(app.world_mut());
            collect_widget_paint(app.world_mut());
            let layout = app.world().get::<UiTextLayout>(owner).unwrap();
            let buffer = app.world().get::<TextBuffer>(owner).unwrap();
            let caret = layout.caret_rect(buffer).unwrap();
            let preedit_rects = layout.preedit_rects();
            let paint = app.world().resource::<DisplayList>().widget_items();
            assert!(
                paint.iter().any(|item| {
                    let Primitive::Line {
                        from, to, color, ..
                    } = &item.primitive
                    else {
                        return false;
                    };
                    *color == theme.color(ThemeColorToken::EditorCaret)
                        && (from[0] - caret.x).abs() < 0.01
                        && (from[1] - caret.y).abs() < 0.01
                        && (to[0] - caret.x).abs() < 0.01
                        && (to[1] - (caret.y + caret.height)).abs() < 0.01
                }),
                "{kind:?} caret={caret:?}"
            );
            assert!(paint.iter().any(|item| {
                let Primitive::Line {
                    from, to, color, ..
                } = &item.primitive
                else {
                    return false;
                };
                *color == theme.color(ThemeColorToken::EditorImeUnderline)
                    && preedit_rects.iter().any(|rect| {
                        let y = rect.y + rect.height.max(1.0) - 1.0;
                        (from[0] - rect.x).abs() < 0.01
                            && (from[1] - y).abs() < 0.01
                            && (to[0] - (rect.x + rect.width)).abs() < 0.01
                            && (to[1] - y).abs() < 0.01
                    })
            }));
        }
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![UiRoutedEvent::ImeDisabled {
                window,
                target: owner,
            }];
        bui_text::process_text_events(app.world_mut());
        bui_text::update_text_layouts(app.world_mut());

        app.world_mut()
            .resource_mut::<UiRoutedEvents>()
            .events
            .clear();
        app.world_mut()
            .get_mut::<TextBuffer>(owner)
            .unwrap()
            .collapse_to(0);
        app.world_mut()
            .get_mut::<TextArea>(owner)
            .unwrap()
            .scroll_line = 0;
        app.world_mut()
            .get_mut::<UiTextLayout>(owner)
            .unwrap()
            .scroll_offset[1] = 0.0;
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = None;
        app.update();

        let scroll_before = app.world().get::<TextArea>(owner).unwrap().scroll_line;
        let max_scroll = app
            .world()
            .get::<UiTextLayout>(owner)
            .unwrap()
            .layout()
            .unwrap()
            .bounds()
            .height;
        let max_scroll = (max_scroll - 38.0).max(0.0);
        let max_scroll_lines = (max_scroll / 24.0).round() as usize;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: owner,
            position: [30.0, 40.0],
            delta: [0.0, -2.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        let scrolled_line = app.world().get::<TextArea>(owner).unwrap().scroll_line;
        assert_eq!(scrolled_line, (scroll_before + 2).min(max_scroll_lines));
        assert_eq!(
            app.world()
                .get::<UiTextLayout>(owner)
                .unwrap()
                .scroll_offset[1],
            scrolled_line as f32 * 24.0
        );
        let layout = app.world().get::<UiTextLayout>(owner).unwrap();
        let layout_result = app.world().get::<UiLayoutResult>(owner).unwrap();
        let padding_y = app
            .world()
            .get::<UiLayoutNode>(owner)
            .map_or(0.0, |node| node.padding.top);
        let expected_origin_y = layout_result.rect.y + padding_y + layout.inset[1].max(0.0)
            - layout.scroll_offset[1].max(0.0);
        let actual_origin_y = layout.text_origin().unwrap()[1];
        assert!(
            (actual_origin_y - expected_origin_y).abs() < 0.01,
            "origin={actual_origin_y} expected={expected_origin_y} rect_y={} padding_y={padding_y} inset_y={} scroll_y={}",
            layout_result.rect.y,
            layout.inset[1],
            layout.scroll_offset[1]
        );
        app.world_mut()
            .resource_mut::<UiRoutedEvents>()
            .events
            .clear();
        app.update();
    }

    #[test]
    fn widget_tree_file_browser_virtualizes_rows_selects_filters_and_opens_directories() {
        let directory = std::env::temp_dir().join(format!(
            "bui-widget-browser-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(directory.join("subfolder")).unwrap();
        std::fs::write(directory.join("alpha.txt"), "a").unwrap();
        std::fs::write(directory.join("beta.bin"), "b").unwrap();
        std::fs::write(directory.join("subfolder/nested.txt"), "n").unwrap();
        // FileBrowser entries use canonical paths; macOS may canonicalize
        // temp_dir's `/var` prefix to `/private/var`.
        let directory = std::fs::canonicalize(directory).unwrap();

        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let owner = WidgetTree::new(WidgetNode::file_browser(directory.clone()))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 360.0, 170.0),
            clip: None,
            z: 5,
            content_size: [360.0, 170.0],
            dpi_scale: 1.0,
        });
        app.update();

        let visible_items = browser_items(app.world_mut(), owner);
        assert_eq!(visible_items.len(), 3);
        assert!(
            visible_items
                .iter()
                .all(|(entity, _)| app.world().get::<UiHitBox>(*entity).is_some())
        );
        let alpha = visible_items
            .iter()
            .find(|(_, item)| item.name == "alpha.txt")
            .unwrap()
            .clone();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: alpha.0,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [8.0, 54.0],
        }];
        app.update();
        assert!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .selected
                .contains(&directory.join("alpha.txt"))
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileSelected { path, selected: true, .. } if path.ends_with("alpha.txt"))));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: owner,
            text: "alpha".into(),
        }];
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(owner).unwrap().filter,
            "alpha"
        );
        assert_eq!(browser_items(app.world_mut(), owner).len(), 2); // Directories stay visible while filtering.

        let subfolder = browser_items(app.world_mut(), owner)
            .into_iter()
            .find(|(_, item)| item.name == "subfolder")
            .unwrap();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: subfolder.0,
            button: UiPointerButton::Primary,
            click_count: 2,
            position: [10.0, 54.0],
        }];
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(owner).unwrap().directory,
            std::fs::canonicalize(directory.join("subfolder")).unwrap()
        );
        assert!(
            app.world()
                .get::<FileBrowserItem>(subfolder.0)
                .is_none_or(|item| !item.selected)
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn list_view_virtualizes_stable_items_and_paints_clipped_rows() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let model = ListModel {
            items: (0..2_000)
                .map(|index| ListItem {
                    id: index + 1,
                    value: format!("Asset {index:04}"),
                    enabled: index % 13 != 0,
                })
                .collect(),
            ..ListModel::default()
        };
        let owner = WidgetTree::new(WidgetNode::list("Assets", model))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(12.0, 16.0, 240.0, 130.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 4,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();

        let visible = collection_items(app.world_mut(), owner);
        assert!((5..=7).contains(&visible.len()));
        assert!(visible.iter().all(|(entity, item)| {
            item.kind == CollectionItemKind::List
                && app
                    .world()
                    .get::<UiParent>(*entity)
                    .is_some_and(|parent| parent.0 == owner)
                && app.world().get::<UiHitBox>(*entity).is_some_and(|hit| {
                    hit.window == Some(window)
                        && hit.clip.is_some_and(|clip| {
                            clip.y >= rect.y + 28.0 && clip.bottom() <= rect.bottom()
                        })
                })
        }));
        assert!(visible.len() < 2_000);
        let stable_id = visible.iter().find(|(_, item)| item.id == 5).unwrap().0;
        let first = visible.iter().map(|(_, item)| item.id).min().unwrap();
        assert_eq!(first, 1);
        assert!(app.world().resource::<DisplayList>().widget_items().iter().any(
            |draw| matches!(&draw.primitive, Primitive::TextWrapped { text, .. } if text.contains("Asset 0000"))
        ));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: owner,
            position: [30.0, 80.0],
            delta: [0.0, -1.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        let scrolled = collection_items(app.world_mut(), owner);
        assert_eq!(scrolled.iter().map(|(_, item)| item.id).min(), Some(2));
        assert_eq!(
            app.world()
                .resource::<WidgetCollectionItems>()
                .0
                .get(&owner)
                .and_then(|items| items.get(&5)),
            Some(&stable_id)
        );
    }

    #[test]
    fn list_keyboard_routes_page_edges_select_all_activate_and_rename_cancel() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let model = ListModel {
            items: (0..10)
                .map(|index| ListItem {
                    id: index + 1,
                    value: format!("Asset {index}"),
                    enabled: true,
                })
                .collect(),
            ..ListModel::default()
        };
        let owner = WidgetTree::new(WidgetNode::list("Assets", model))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 240.0, 150.0),
            clip: None,
            z: 0,
            content_size: [240.0, 150.0],
            dpi_scale: 1.0,
        });
        app.update();
        let send_key = |app: &mut App, key, modifiers| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: owner,
                key,
                pressed: true,
                repeat: false,
                modifiers,
            }];
            app.update();
        };

        send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ListModel<String>>(owner)
                .unwrap()
                .selection
                .active,
            Some(1)
        );
        send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
        let page_row = app
            .world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active
            .unwrap();
        assert!(page_row > 1);
        send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ListModel<String>>(owner)
                .unwrap()
                .selection
                .active,
            Some(1)
        );
        send_key(&mut app, UiKeyCode::End, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ListModel<String>>(owner)
                .unwrap()
                .selection
                .active,
            Some(10)
        );
        send_key(
            &mut app,
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(
            app.world()
                .get::<ListModel<String>>(owner)
                .unwrap()
                .selection
                .selected
                .len(),
            10
        );
        send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<ListModel<String>>(owner)
                .unwrap()
                .selection
                .active,
            Some(1)
        );
        send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::CollectionItemActivated { target, id: 1 } if *target == owner)));

        send_key(&mut app, UiKeyCode::F2, UiModifiers::default());
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: owner,
            text: "Cancelled rename".to_owned(),
        }];
        app.update();
        send_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
        assert_eq!(
            app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
            "Asset 0"
        );
        assert!(
            app.world()
                .get::<CollectionEditorState>(owner)
                .unwrap()
                .rename_id
                .is_none()
        );
    }

    #[test]
    fn collection_keyboard_rename_and_history_restore_serialized_model() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let model = ListModel {
            items: vec![
                ListItem {
                    id: 10,
                    value: "Oak".to_owned(),
                    enabled: true,
                },
                ListItem {
                    id: 20,
                    value: "Pine".to_owned(),
                    enabled: true,
                },
            ],
            ..ListModel::default()
        };
        let owner = WidgetTree::new(WidgetNode::list("Trees", model))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 220.0, 120.0),
            clip: None,
            z: 0,
            content_size: [220.0, 120.0],
            dpi_scale: 1.0,
        });
        app.update();

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::Key {
                window,
                target: owner,
                key: UiKeyCode::ArrowDown,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
            UiRoutedEvent::Key {
                window,
                target: owner,
                key: UiKeyCode::F2,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
            UiRoutedEvent::Text {
                window,
                target: owner,
                text: "Birch 🌿".to_owned(),
            },
            UiRoutedEvent::Key {
                window,
                target: owner,
                key: UiKeyCode::Enter,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers::default(),
            },
        ];
        app.update();
        assert_eq!(
            app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
            "Birch 🌿"
        );
        assert!(app.world().resource::<bui_core::UiHistory>().undo_len() > 0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(
            app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
            "Oak"
        );
        assert!(app.world().resource::<bui_core::UiHistory>().redo_len() > 0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Y,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(
            app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
            "Birch 🌿"
        );
    }

    #[test]
    fn tree_rows_route_expand_drag_reorder_and_single_undo() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let tree = TreeModel {
            nodes: vec![
                crate::TreeNode {
                    id: 1,
                    parent: None,
                    children: vec![2, 3],
                    value: "Scene".to_owned(),
                    expanded: false,
                    enabled: true,
                },
                crate::TreeNode {
                    id: 2,
                    parent: Some(1),
                    children: vec![],
                    value: "Camera".to_owned(),
                    expanded: false,
                    enabled: true,
                },
                crate::TreeNode {
                    id: 3,
                    parent: Some(1),
                    children: vec![],
                    value: "Light".to_owned(),
                    expanded: false,
                    enabled: true,
                },
            ],
            ..TreeModel::default()
        };
        let owner = WidgetTree::new(WidgetNode::tree("Outliner", tree))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(0.0, 0.0, 240.0, 130.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 0,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();

        let root_row = collection_items(app.world_mut(), owner)
            .into_iter()
            .find(|(_, item)| item.id == 1)
            .unwrap();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: root_row.0,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [10.0, 40.0],
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<TreeModel<String>>(owner)
                .unwrap()
                .selection
                .active,
            Some(1)
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowLeft,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<TreeModel<String>>(owner)
                .unwrap()
                .visible_ids(),
            [1]
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowRight,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<TreeModel<String>>(owner)
                .unwrap()
                .visible_ids(),
            [1, 2, 3]
        );

        let rows = collection_items(app.world_mut(), owner);
        let camera = rows.iter().find(|(_, item)| item.id == 2).unwrap().clone();
        let light = rows.iter().find(|(_, item)| item.id == 3).unwrap().clone();
        let camera_hit = app.world().get::<UiHitBox>(camera.0).unwrap().rect;
        let light_hit = app.world().get::<UiHitBox>(light.0).unwrap().rect;
        let start = [light_hit.x + 20.0, light_hit.y + light_hit.height * 0.5];
        let end = [camera_hit.x + 20.0, camera_hit.y + camera_hit.height * 0.25];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: light.0,
                button: UiPointerButton::Primary,
                position: start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: camera.0,
                position: end,
                delta: [end[0] - start[0], end[1] - start[1]],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: camera.0,
                button: UiPointerButton::Primary,
                position: end,
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(
            app.world().get::<TreeModel<String>>(owner).unwrap().nodes[0].children,
            [3, 2]
        );
        assert!(app.world().resource::<bui_core::UiHistory>().undo_len() > 0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(
            app.world().get::<TreeModel<String>>(owner).unwrap().nodes[0].children,
            [2, 3]
        );
    }

    #[test]
    fn preview_grid_selects_spatially_and_paints_registered_images_or_tints() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut grid = PreviewGrid {
            items: (0..16)
                .map(|index| {
                    let mut item =
                        crate::PreviewGridItem::new(index + 1, format!("Material {index}"));
                    item.tint = [0.2 + index as f32 * 0.035, 0.42, 0.72, 1.0];
                    if index == 0 {
                        item.asset_id = Some(901);
                    }
                    item
                })
                .collect(),
            cell_width: 112.0,
            cell_height: 104.0,
            ..PreviewGrid::default()
        };
        grid.select(3, false, false);
        let owner = WidgetTree::new(WidgetNode::preview_grid("Materials", grid))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(0.0, 0.0, 250.0, 180.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 0,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();

        let rows = collection_items(app.world_mut(), owner);
        let image_item = rows.iter().find(|(_, item)| item.id == 1).unwrap().0;
        let other_item = rows.iter().find(|(_, item)| item.id == 4).unwrap().0;
        assert_eq!(
            app.world()
                .get::<CollectionItemControl>(image_item)
                .unwrap()
                .asset_id,
            Some(901)
        );
        let other_layout = app.world().get::<UiLayoutResult>(other_item).unwrap().rect;
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|draw| matches!(draw.primitive, Primitive::Image { asset_id: 901, .. }))
        );
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|draw| matches!(draw.primitive, Primitive::RoundedRect { color, .. } if (color.0 - 0.305).abs() < 0.0001 && (color.1 - 0.42).abs() < 0.0001))
        );

        app.world_mut()
            .resource_mut::<WidgetInteractionState>()
            .modifiers
            .insert(
                window,
                UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: other_item,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [other_layout.x + 10.0, other_layout.y + 40.0],
        }];
        app.update();
        let grid = app.world().get::<PreviewGrid>(owner).unwrap();
        assert!(grid.selection.contains(3));
        assert!(grid.selection.contains(4));
        assert_eq!(grid.selection.active, Some(4));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowUp,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<PreviewGrid>(owner)
                .unwrap()
                .selection
                .active,
            Some(2)
        );
    }

    #[test]
    fn preview_grid_keyboard_routes_column_page_edge_and_range_navigation() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let grid = PreviewGrid {
            items: (1..=12)
                .map(|id| crate::PreviewGridItem::new(id, format!("Item {id}")))
                .collect(),
            columns: 3,
            cell_width: 60.0,
            cell_height: 56.0,
            ..PreviewGrid::default()
        };
        let owner = WidgetTree::new(WidgetNode::preview_grid("Items", grid))
            .spawn(app.world_mut(), None)
            .unwrap();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 240.0, 220.0),
            clip: None,
            z: 0,
            content_size: [240.0, 220.0],
            dpi_scale: 1.0,
        });
        app.update();
        app.world_mut()
            .get_mut::<PreviewGrid>(owner)
            .unwrap()
            .select(1, false, false);
        let send_key = |app: &mut App, key, modifiers| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: owner,
                key,
                pressed: true,
                repeat: false,
                modifiers,
            }];
            app.update();
        };
        let active = |app: &App| {
            app.world()
                .get::<PreviewGrid>(owner)
                .unwrap()
                .selection
                .active
                .unwrap()
        };

        send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(active(&app), 4, "Down advances one grid column");
        send_key(
            &mut app,
            UiKeyCode::ArrowRight,
            UiModifiers {
                shift: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(active(&app), 5, "Right advances one item");
        assert_eq!(
            app.world()
                .get::<PreviewGrid>(owner)
                .unwrap()
                .selection
                .selected,
            [4, 5].into(),
            "Shift extends the active item range"
        );
        send_key(&mut app, UiKeyCode::ArrowLeft, UiModifiers::default());
        assert_eq!(active(&app), 4);
        send_key(&mut app, UiKeyCode::ArrowUp, UiModifiers::default());
        assert_eq!(active(&app), 1);
        send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
        assert!(active(&app) > 1, "PageDown advances by a grid row page");
        send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
        assert_eq!(active(&app), 1);
        send_key(&mut app, UiKeyCode::End, UiModifiers::default());
        assert_eq!(active(&app), 12);
        send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
        assert_eq!(active(&app), 1);
        send_key(
            &mut app,
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(
            app.world()
                .get::<PreviewGrid>(owner)
                .unwrap()
                .selection
                .selected
                .len(),
            12
        );
        send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::CollectionItemActivated { target, id: 12 } if *target == owner)));
    }

    #[test]
    fn data_table_virtualizes_stable_cells_routes_selection_keyboard_and_scroll() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut table = sample_table(100);
        table.columns[0].width = 180.0;
        table.columns[1].width = 120.0;
        let owner = WidgetTree::new(WidgetNode::data_table("Objects", table))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(0.0, 0.0, 260.0, 150.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 4,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();

        let initial = data_table_cells(app.world_mut(), owner);
        let visible_rows: HashSet<_> = initial.iter().map(|(_, cell)| cell.row_id).collect();
        assert_eq!(visible_rows.len(), 5);
        assert_eq!(initial.len(), visible_rows.len() * 2);
        let stable_entity = initial
            .iter()
            .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
            .unwrap()
            .0;
        assert_eq!(data_table_headers(app.world_mut(), owner).len(), 2);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: owner,
            position: [20.0, 80.0],
            delta: [0.0, -1.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(app.world().get::<DataTable>(owner).unwrap().scroll_rows, 1);
        let after_scroll = data_table_cells(app.world_mut(), owner);
        assert_eq!(
            after_scroll
                .iter()
                .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
                .unwrap()
                .0,
            stable_entity,
            "a row that remains visible should keep its cell entity"
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: owner,
            position: [220.0, 80.0],
            delta: [-35.0, 0.0],
            unit: bui_core::UiWheelUnit::Pixels,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(app.world().get::<DataTable>(owner).unwrap().scroll_x, 35.0);
        assert_eq!(
            data_table_cells(app.world_mut(), owner)
                .iter()
                .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
                .unwrap()
                .0,
            stable_entity,
            "horizontal scrolling also preserves visible cell identity"
        );

        let cell = data_table_cells(app.world_mut(), owner)
            .iter()
            .find(|(_, cell)| cell.row_id == 3 && cell.column == 0)
            .unwrap()
            .0;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: cell,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [40.0, 90.0],
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<DataTable>(owner)
                .unwrap()
                .selection
                .active,
            Some(3)
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::DataTableCommitted { target, .. } if *target == owner)));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowRight,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert_eq!(
            app.world().get::<DataTable>(owner).unwrap().active_column,
            1
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::ArrowDown,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                shift: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        let table = app.world().get::<DataTable>(owner).unwrap();
        assert_eq!(table.selection.active, Some(4));
        assert_eq!(table.selection.selected, [3, 4].into());

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Enter,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers::default(),
        }];
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::DataTableCellActivated { target, row_id: 4, column: 1 } if *target == owner)));

        app.world_mut()
            .resource_mut::<WidgetInteractionState>()
            .modifiers
            .insert(
                window,
                UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            );
        let cell = data_table_cells(app.world_mut(), owner)
            .into_iter()
            .find(|(_, cell)| cell.row_id == 6 && cell.column == 0)
            .unwrap()
            .0;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: cell,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [45.0, 135.0],
        }];
        app.update();
        let table = app.world().get::<DataTable>(owner).unwrap();
        assert_eq!(table.selection.active, Some(6));
        assert_eq!(table.selection.selected, [3, 4, 6].into());

        let key = |key, modifiers| UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        app.update();
        assert_eq!(
            app.world()
                .get::<DataTable>(owner)
                .unwrap()
                .selection
                .selected
                .len(),
            100,
            "Ctrl+A selects every row"
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::Home, UiModifiers::default())];
        app.update();
        assert_eq!(
            app.world()
                .get::<DataTable>(owner)
                .unwrap()
                .selection
                .active,
            Some(1)
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::PageDown, UiModifiers::default())];
        app.update();
        let paged_row = app
            .world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .active
            .unwrap();
        assert!(paged_row > 1, "PageDown advances by the viewport row count");
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::PageUp, UiModifiers::default())];
        app.update();
        assert_eq!(
            app.world()
                .get::<DataTable>(owner)
                .unwrap()
                .selection
                .active,
            Some(1),
            "PageUp reverses the same row stride"
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::End, UiModifiers::default())];
        app.update();
        assert_eq!(
            app.world()
                .get::<DataTable>(owner)
                .unwrap()
                .selection
                .active,
            Some(100)
        );
    }

    #[test]
    fn virtual_table_and_graph_sync_skip_idle_component_changes() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, WidgetPlugin))
            .init_resource::<VirtualSyncChanges>()
            .add_systems(Update, count_virtual_sync_changes.in_set(UiSystems::Render));
        let window = app.world_mut().spawn_empty().id();
        let table = WidgetTree::new(WidgetNode::data_table("Objects", sample_table(100)))
            .spawn(app.world_mut(), None)
            .unwrap();
        let table_rect = UiRect::new(0.0, 0.0, 260.0, 150.0);
        app.world_mut().entity_mut(table).insert(UiLayoutResult {
            window,
            rect: table_rect,
            clip: None,
            z: 4,
            content_size: [table_rect.width, table_rect.height],
            dpi_scale: 1.0,
        });

        let mut graph = NodeGraph::default();
        let first = graph.add_node(
            "First",
            [40.0, 40.0],
            vec![NodeSocket::output("out", crate::SocketType::Float)],
        );
        graph.add_node(
            "Second",
            [240.0, 80.0],
            vec![NodeSocket::input("in", crate::SocketType::Float)],
        );
        let graph_owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
            .spawn(app.world_mut(), None)
            .unwrap();
        let graph_rect = UiRect::new(300.0, 0.0, 640.0, 440.0);
        app.world_mut()
            .entity_mut(graph_owner)
            .insert(UiLayoutResult {
                window,
                rect: graph_rect,
                clip: None,
                z: 8,
                content_size: [graph_rect.width, graph_rect.height],
                dpi_scale: 1.0,
            });

        app.update();
        assert!(app.world().resource::<VirtualSyncChanges>().table_cells > 0);
        assert!(app.world().resource::<VirtualSyncChanges>().graph_nodes > 0);

        app.update();
        assert_eq!(
            *app.world().resource::<VirtualSyncChanges>(),
            VirtualSyncChanges::default(),
            "an idle view refresh keeps all visible model components and geometry unchanged"
        );

        app.world_mut().get_mut::<DataTable>(table).unwrap().rows[0].cells[0] =
            "Updated visible cell".into();
        app.world_mut()
            .get_mut::<NodeGraph>(graph_owner)
            .unwrap()
            .nodes
            .iter_mut()
            .find(|node| node.id == first)
            .unwrap()
            .title = "Updated visible node".into();
        app.update();
        let changes = app.world().resource::<VirtualSyncChanges>();
        assert_eq!(changes.table_cells, 1);
        assert_eq!(changes.graph_nodes, 1);
        assert_eq!(changes.table_headers, 0);
        assert_eq!(changes.graph_sockets, 0);
        assert_eq!(changes.graph_links, 0);
        assert_eq!(changes.layout_results, 0);
        assert_eq!(changes.hit_boxes, 0);
    }

    #[test]
    fn data_table_header_sorts_and_resize_gesture_is_one_undo_transaction() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut table = sample_table(12);
        table.rows.reverse();
        let owner = WidgetTree::new(WidgetNode::data_table("Objects", table))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(10.0, 20.0, 240.0, 150.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: Some(UiRect::new(20.0, 20.0, 220.0, 140.0)),
            z: 8,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();
        let headers = data_table_headers(app.world_mut(), owner);
        let (header_entity, header) = headers
            .iter()
            .find(|(_, header)| header.column == 0)
            .unwrap();
        let header_rect = app.world().get::<UiHitBox>(*header_entity).unwrap().rect;
        assert_eq!(header.width, 104.0);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
            window,
            target: *header_entity,
            button: UiPointerButton::Primary,
            click_count: 1,
            position: [header_rect.x + 8.0, header_rect.y + 6.0],
        }];
        app.update();
        let table = app.world().get::<DataTable>(owner).unwrap();
        assert_eq!(table.sort, Some((0, true)));
        assert_eq!(table.rows[0].id, 1);

        let before_undo_count = app.world().resource::<bui_core::UiHistory>().undo_len();
        let hit_x = header_rect.x + header_rect.width - 1.0;
        let y = header_rect.y + 8.0;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: *header_entity,
                button: UiPointerButton::Primary,
                position: [hit_x, y],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: *header_entity,
                position: [hit_x + 30.0, y],
                delta: [30.0, 0.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: *header_entity,
                button: UiPointerButton::Primary,
                position: [hit_x + 30.0, y],
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(
            app.world().get::<DataTable>(owner).unwrap().columns[0].width,
            134.0
        );
        assert_eq!(
            app.world().resource::<bui_core::UiHistory>().undo_len(),
            before_undo_count + 1,
            "the entire drag should commit as one undo step"
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(
            app.world().get::<DataTable>(owner).unwrap().columns[0].width,
            104.0
        );
    }

    #[test]
    fn data_table_painter_emits_header_cell_text_and_selection_gpu_primitives() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut table = sample_table(3);
        table.select_cell(2, 1, false, false);
        let owner = WidgetTree::new(WidgetNode::data_table("Inventory", table))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(15.0, 20.0, 250.0, 144.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: Some(UiRect::new(20.0, 20.0, 220.0, 110.0)),
            z: 12,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();
        let items = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        assert!(items.len() >= 20);
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, .. } if text.contains("Inventory")
        )));
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, .. } if text == "Name" || text == "Type"
        )));
        assert!(items.iter().any(|item| matches!(
            &item.primitive,
            Primitive::TextWrapped { text, .. } if text == "Object 001"
        )));
        assert!(items.iter().all(|item| item.z >= 12));
        assert!(items.iter().all(|item| item.clip.x >= 20.0));
        assert!(
            items
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Line { .. }))
        );
    }

    #[test]
    fn data_table_mounts_columns_after_a_fully_scrolled_off_column() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut table = sample_table(4);
        table.columns = vec![
            crate::TableColumn::new("Hidden", 160.0),
            crate::TableColumn::new("Visible", 160.0),
            crate::TableColumn::new("Later", 160.0),
        ];
        for row in &mut table.rows {
            row.cells.push("Value 2".to_owned());
        }
        table.scroll_x = 170.0;
        let owner = WidgetTree::new(WidgetNode::data_table("Objects", table))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(0.0, 0.0, 250.0, 180.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 4,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });

        sync_data_table_views(app.world_mut());

        let headers = data_table_headers(app.world_mut(), owner);
        assert!(!headers.iter().any(|(_, header)| header.column == 0));
        assert!(headers.iter().any(|(_, header)| header.column == 1));
        assert!(headers.iter().any(|(_, header)| header.column == 2));
        let cells = data_table_cells(app.world_mut(), owner);
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|(_, cell)| cell.column > 0));
    }

    #[test]
    fn node_graph_connects_reconnects_cuts_links_and_undoes_each_gesture() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut graph = NodeGraph::default();
        let source_a = graph.add_node(
            "Source A",
            [40.0, 40.0],
            vec![NodeSocket::output("value", crate::SocketType::Float)],
        );
        let source_b = graph.add_node(
            "Source B",
            [40.0, 220.0],
            vec![NodeSocket::output("value", crate::SocketType::Float)],
        );
        let sink = graph.add_node(
            "Sink",
            [360.0, 120.0],
            vec![NodeSocket::input("value", crate::SocketType::Float)],
        );
        let owner = WidgetTree::new(WidgetNode::node_graph("Shader", graph))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(20.0, 30.0, 640.0, 440.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 8,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();

        let spacing = app.world().resource::<Theme>().spacing.node_graph;
        let socket_point = |node_id, socket_index| {
            let graph = app.world().get::<NodeGraph>(owner).unwrap();
            let node = graph.nodes.iter().find(|node| node.id == node_id).unwrap();
            node_graph_socket_position(graph, rect, node, socket_index, &spacing).unwrap()
        };
        let from_a = socket_point(source_a, 0);
        let from_b = socket_point(source_b, 0);
        let to_sink = socket_point(sink, 0);
        let before_history = app.world().resource::<bui_core::UiHistory>().undo_len();
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: from_a,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: to_sink,
                delta: [to_sink[0] - from_a[0], to_sink[1] - from_a[1]],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: to_sink,
                cancelled: false,
            },
        ];
        app.update();
        let link = *app
            .world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .links
            .first()
            .unwrap();
        assert_eq!((link.from_node, link.to_node), (source_a, sink));
        assert_eq!(
            app.world().resource::<bui_core::UiHistory>().undo_len(),
            before_history + 1,
            "connection drag is one history transaction"
        );
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::NodeGraphCommitted { target, .. } if *target == owner)
        ));
        assert!(
            app.world()
                .resource::<DisplayList>()
                .widget_items()
                .iter()
                .any(|item| matches!(item.primitive, Primitive::Bezier { .. }))
        );
        let link_control = app
            .world()
            .iter_entities()
            .find(|entity| {
                entity
                    .get::<NodeGraphLinkControl>()
                    .is_some_and(|control| control.owner == owner)
            })
            .expect("visible connection has an accessible virtual control");
        let stable_link_id = link_control.get::<NodeGraphLinkControl>().unwrap().link_id;

        let undo = |app: &mut App, redo: bool| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: owner,
                key: if redo { UiKeyCode::Y } else { UiKeyCode::Z },
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    ..UiModifiers::default()
                },
            }];
            app.update();
        };
        undo(&mut app, false);
        assert!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .links
                .is_empty()
        );
        undo(&mut app, true);
        let link = app.world().get::<NodeGraph>(owner).unwrap().links[0];
        assert_eq!(link.id, stable_link_id);
        assert!(app.world().iter_entities().any(|entity| {
            entity
                .get::<NodeGraphLinkControl>()
                .is_some_and(|control| control.owner == owner && control.link_id == stable_link_id)
        }));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: from_a,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: from_b,
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(
            app.world().get::<NodeGraph>(owner).unwrap().links[0].from_node,
            source_b,
            "dragging an existing output endpoint reconnects the link"
        );

        let graph = app.world().get::<NodeGraph>(owner).unwrap().clone();
        let points = node_graph_link_points(&graph, rect, graph.links[0], &spacing).unwrap();
        let middle = bezier_point(points, 0.5);
        let cut_end = [middle[0], middle[1] + 18.0];
        app.world_mut()
            .resource_mut::<WidgetInteractionState>()
            .modifiers
            .insert(
                window,
                UiModifiers {
                    alt: true,
                    ..UiModifiers::default()
                },
            );
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: middle,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: cut_end,
                delta: [0.0, 18.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: cut_end,
                cancelled: false,
            },
        ];
        app.update();
        assert!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .links
                .is_empty()
        );
        app.world_mut()
            .resource_mut::<WidgetInteractionState>()
            .modifiers
            .remove(&window);
        undo(&mut app, false);
        assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().links.len(), 1);
    }

    #[test]
    fn node_graph_routes_selection_move_collapse_box_pan_zoom_add_and_delete() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut graph = NodeGraph::default();
        let first = graph.add_node(
            "First",
            [40.0, 40.0],
            vec![NodeSocket::output("out", crate::SocketType::Float)],
        );
        let second = graph.add_node(
            "Second",
            [330.0, 80.0],
            vec![NodeSocket::input("in", crate::SocketType::Float)],
        );
        let owner = WidgetTree::new(WidgetNode::node_graph("Graph", graph))
            .spawn(app.world_mut(), None)
            .unwrap();
        let rect = UiRect::new(0.0, 0.0, 600.0, 420.0);
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect,
            clip: None,
            z: 5,
            content_size: [rect.width, rect.height],
            dpi_scale: 1.0,
        });
        app.update();
        let spacing = app.world().resource::<Theme>().spacing.node_graph;
        let first_header = [
            rect.x + 40.0 + spacing.title_padding_x + 10.0,
            rect.y + spacing.header_height + 40.0 + spacing.header_height * 0.5,
        ];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: first_header,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: first_header,
                cancelled: false,
            },
        ];
        app.update();
        assert!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .selected
                .contains(&first)
        );

        let key = |key, modifiers| UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        };
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::ArrowRight, UiModifiers::default())];
        app.update();
        assert_eq!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .nodes
                .iter()
                .find(|node| node.id == first)
                .unwrap()
                .position[0],
            50.0
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::C, UiModifiers::default())];
        app.update();
        assert!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .nodes
                .iter()
                .find(|node| node.id == first)
                .unwrap()
                .collapsed
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: [320.0, 100.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: [420.0, 170.0],
                delta: [100.0, 70.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: [420.0, 170.0],
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(
            app.world().get::<NodeGraph>(owner).unwrap().selected,
            [second].into(),
            "dragging an empty-area marquee selects intersecting nodes"
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        app.update();
        assert_eq!(
            app.world().get::<NodeGraph>(owner).unwrap().selected,
            [first, second].into()
        );
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::N, UiModifiers::default())];
        app.update();
        assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().nodes.len(), 3);
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        app.update();
        assert_eq!(
            app.world().get::<NodeGraph>(owner).unwrap().selected.len(),
            3
        );

        let start_pan = app.world().get::<NodeGraph>(owner).unwrap().pan;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Middle,
                position: [570.0, 390.0],
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: [590.0, 400.0],
                delta: [20.0, 10.0],
                dragging: true,
            },
            UiRoutedEvent::PointerUp {
                window,
                target: owner,
                button: UiPointerButton::Middle,
                position: [590.0, 400.0],
                cancelled: false,
            },
        ];
        app.update();
        assert_eq!(
            app.world().get::<NodeGraph>(owner).unwrap().pan,
            [start_pan[0] + 20.0, start_pan[1] + 10.0]
        );
        let zoom = app.world().get::<NodeGraph>(owner).unwrap().zoom;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
            window,
            target: owner,
            position: [260.0, 220.0],
            delta: [0.0, 1.0],
            unit: bui_core::UiWheelUnit::Lines,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert!(app.world().get::<NodeGraph>(owner).unwrap().zoom > zoom);
        let zoomed_by_wheel = app.world().get::<NodeGraph>(owner).unwrap().zoom;
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![key(
            UiKeyCode::ArrowUp,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        app.update();
        assert!(app.world().get::<NodeGraph>(owner).unwrap().zoom > zoomed_by_wheel);
        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::Home, UiModifiers::default())];
        app.update();
        assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().zoom, 1.0);
        assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().pan, [0.0, 0.0]);

        let graph = app.world().get::<NodeGraph>(owner).unwrap().clone();
        let node = graph.nodes.iter().find(|node| node.id == first).unwrap();
        let spacing = app.world().resource::<Theme>().spacing.node_graph;
        let position = node_graph_node_rect(&graph, rect, node, &spacing);
        let before_cancel = graph
            .nodes
            .iter()
            .find(|node| node.id == first)
            .unwrap()
            .position;
        let drag_start = [position.x + 60.0, position.y + 10.0];
        let drag_end = [drag_start[0] + 40.0, drag_start[1] + 20.0];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: drag_start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: drag_end,
                delta: [40.0, 20.0],
                dragging: true,
            },
            UiRoutedEvent::PointerCancel {
                window,
                target: owner,
            },
        ];
        app.update();
        assert_eq!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .nodes
                .iter()
                .find(|node| node.id == first)
                .unwrap()
                .position,
            before_cancel
        );

        let before_escape = app.world().get::<NodeGraph>(owner).unwrap().clone();
        let node = before_escape
            .nodes
            .iter()
            .find(|node| node.id == first)
            .unwrap();
        let node_rect = node_graph_node_rect(&before_escape, rect, node, &spacing);
        let start = [node_rect.x + 60.0, node_rect.y + 12.0];
        let moved = [start[0] + 24.0, start[1] + 16.0];
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![
            UiRoutedEvent::PointerDown {
                window,
                target: owner,
                button: UiPointerButton::Primary,
                position: start,
            },
            UiRoutedEvent::PointerMoved {
                window,
                target: owner,
                position: moved,
                delta: [24.0, 16.0],
                dragging: true,
            },
            key(UiKeyCode::Escape, UiModifiers::default()),
        ];
        app.update();
        assert_eq!(
            app.world().get::<NodeGraph>(owner).unwrap(),
            &before_escape,
            "Escape restores the graph snapshot from the active gesture"
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events =
            vec![key(UiKeyCode::Delete, UiModifiers::default())];
        app.update();
        assert!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .nodes
                .is_empty()
        );
    }

    #[test]
    fn file_navigate_to_updates_directory_reports_errors_and_resets_scroll() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bui-widgets-navigate-to-{}-{unique}",
            std::process::id()
        ));
        let nested = root.join("Nested");
        std::fs::create_dir_all(&nested).unwrap();
        let browser = FileBrowser::new(&root).unwrap();
        let owner = app.world_mut().spawn(browser).id();
        app.world_mut()
            .entity_mut(owner)
            .insert(FileBrowserViewState {
                scroll_rows: 17,
                ..FileBrowserViewState::default()
            });

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                owner,
                WidgetActionKind::FileNavigateTo(nested.clone()),
            ));
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(owner).unwrap().directory,
            std::fs::canonicalize(&nested).unwrap()
        );
        assert_eq!(
            app.world()
                .get::<FileBrowserViewState>(owner)
                .unwrap()
                .scroll_rows,
            0
        );
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::FileDirectoryChanged { target, .. } if *target == owner)
        ));

        app.world_mut()
            .get_mut::<FileBrowserViewState>(owner)
            .unwrap()
            .scroll_rows = 11;
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                owner,
                WidgetActionKind::FileNavigateTo(root.join("missing")),
            ));
        app.update();
        assert_eq!(
            app.world()
                .get::<FileBrowserViewState>(owner)
                .unwrap()
                .scroll_rows,
            0
        );
        assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::FileOperationFailed { target, .. } if *target == owner)
        ));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_browser_grid_keyboard_selection_and_bookmarks_route_end_to_end() {
        let root = std::env::temp_dir().join(format!(
            "bui-widgets-browser-routes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        for index in 0..10 {
            std::fs::write(root.join(format!("entry-{index:02}.txt")), "item").unwrap();
        }

        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let mut browser = FileBrowser::new(&root).unwrap();
        browser.view = FileView::Grid;
        let owner = app.world_mut().spawn(browser).id();
        app.world_mut().entity_mut(owner).insert(UiLayoutResult {
            window,
            rect: UiRect::new(0.0, 0.0, 250.0, 130.0),
            clip: None,
            z: 2,
            content_size: [250.0, 130.0],
            dpi_scale: 1.0,
        });
        app.world_mut()
            .get_mut::<FileBrowser>(owner)
            .unwrap()
            .select(0, false, false);

        let send_key = |app: &mut App, key, modifiers| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: owner,
                key,
                pressed: true,
                repeat: false,
                modifiers,
            }];
            app.update();
        };
        let selected_index = |app: &App| {
            let browser = app.world().get::<FileBrowser>(owner).unwrap();
            browser
                .visible_range(0, browser.visible_count())
                .iter()
                .position(|entry| Some(&entry.path) == browser.cursor.as_ref())
                .unwrap()
        };

        send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(selected_index(&app), 2, "grid down advances one column");
        send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
        assert_eq!(selected_index(&app), 3, "grid right advances one cell");
        send_key(&mut app, UiKeyCode::ArrowLeft, UiModifiers::default());
        assert_eq!(selected_index(&app), 2, "grid left reverses one cell");
        send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
        send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
        assert_eq!(
            selected_index(&app),
            5,
            "page down advances a visible grid row"
        );
        send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
        assert_eq!(
            selected_index(&app),
            3,
            "page up reverses the visible-row stride"
        );

        app.world_mut().get_mut::<FileBrowser>(owner).unwrap().view = FileView::List;
        app.world_mut()
            .get_mut::<FileBrowser>(owner)
            .unwrap()
            .select(0, false, false);
        send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
        assert_eq!(selected_index(&app), 1, "list down advances one row");
        send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
        assert_eq!(
            selected_index(&app),
            4,
            "list page down advances visible rows"
        );
        send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
        assert_eq!(
            selected_index(&app),
            1,
            "list page up reverses the row stride"
        );

        send_key(
            &mut app,
            UiKeyCode::A,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        assert_eq!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .selected
                .len(),
            10
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileSelectionChanged { target, selected } if *target == owner && selected.len() == 10)));

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                owner,
                WidgetActionKind::FileBookmarkCurrent,
            ));
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(owner).unwrap().bookmarks,
            vec![std::fs::canonicalize(&root).unwrap()]
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { target, bookmarks } if *target == owner && bookmarks.len() == 1)));

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                owner,
                WidgetActionKind::FileRemoveBookmarkCurrent,
            ));
        app.update();
        assert!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .bookmarks
                .is_empty()
        );
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { target, bookmarks } if *target == owner && bookmarks.is_empty())));

        send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .cursor
                .as_ref()
                .and_then(|path| path.file_name())
                .unwrap(),
            "entry-00.txt"
        );
        send_key(&mut app, UiKeyCode::End, UiModifiers::default());
        assert_eq!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .cursor
                .as_ref()
                .and_then(|path| path.file_name())
                .unwrap(),
            "entry-09.txt"
        );
        send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
        assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileActivated { target, path } if *target == owner && path.ends_with("entry-09.txt"))));

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: owner,
            text: "entry".to_owned(),
        }];
        app.update();
        send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
        assert_eq!(
            app.world().get::<FileBrowser>(owner).unwrap().filter,
            "entr"
        );
        send_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
        assert!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .filter
                .is_empty()
        );
        assert!(
            !app.world()
                .resource::<WidgetFileBrowserFilterEdits>()
                .active
                .contains_key(&owner),
            "Escape must close the filter draft before the next key is routed"
        );
        send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
        assert_eq!(
            app.world().get::<FileBrowser>(owner).unwrap().directory,
            std::fs::canonicalize(root.parent().unwrap()).unwrap(),
            "Backspace leaves an empty filter by navigating to the parent directory"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn direct_file_browser_filter_undo_consumes_draft_before_application_history() {
        let root = std::env::temp_dir().join(format!(
            "bui-widgets-browser-filter-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("cube.txt"), "cube").unwrap();
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut().init_resource::<UiFocus>();
        let window = app.world_mut().spawn_empty().id();
        let browser = app
            .world_mut()
            .spawn((
                FileBrowser::new(&root).unwrap(),
                UiFocusable,
                UiKey("browser".into()),
            ))
            .id();
        let checkbox = app
            .world_mut()
            .spawn((Checkbox::new("enabled", false), UiKey("enabled".into())))
            .id();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(browser);
        app.world_mut().resource_mut::<UiFocus>().set_active(window);
        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(checkbox, WidgetActionKind::SetBool(true)));
        app.update();
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window,
            target: browser,
            text: "cube".into(),
        }];
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().filter,
            "cube"
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

        let route_history_key = |app: &mut App, shift: bool| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: browser,
                key: UiKeyCode::Z,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    shift,
                    ..UiModifiers::default()
                },
            }];
            app.update();
        };
        route_history_key(&mut app, false);

        assert!(
            app.world()
                .get::<FileBrowser>(browser)
                .unwrap()
                .filter
                .is_empty()
        );
        assert_eq!(
            app.world().get::<Checkbox>(checkbox).unwrap().value,
            CheckboxValue::On,
            "the in-progress filter undo must not consume the previous application edit"
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        route_history_key(&mut app, true);
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().filter,
            "cube",
            "redo should restore a locally cancelled filter draft"
        );
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        route_history_key(&mut app, false);
        assert!(
            app.world()
                .get::<FileBrowser>(browser)
                .unwrap()
                .filter
                .is_empty()
        );
        route_history_key(&mut app, false);
        assert_eq!(
            app.world().get::<Checkbox>(checkbox).unwrap().value,
            CheckboxValue::Off,
            "once the local draft redo is consumed, Ctrl+Z should reach app history"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_browser_ui_actions_round_trip_through_widget_history() {
        let root = std::env::temp_dir().join(format!(
            "bui-widgets-browser-history-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(root.join("entry.txt"), "entry").unwrap();
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        app.world_mut().init_resource::<UiFocus>();
        let window = app.world_mut().spawn_empty().id();
        let browser = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiKey("browser-history".into()),
                FileBrowser::new(&root).unwrap(),
                FileBrowserViewState::default(),
                UiLayoutResult {
                    window,
                    rect: UiRect::new(0.0, 0.0, 280.0, 180.0),
                    clip: None,
                    z: 1,
                    content_size: [280.0, 180.0],
                    dpi_scale: 1.0,
                },
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(0.0, 0.0, 280.0, 180.0),
                    clip: None,
                    z_index: 1,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(browser);
        app.world_mut().resource_mut::<UiFocus>().set_active(window);

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                browser,
                WidgetActionKind::FileBookmarkCurrent,
            ));
        app.update();
        assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
        let route_history = |app: &mut App, shift: bool| {
            app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
                window,
                target: browser,
                key: UiKeyCode::Z,
                pressed: true,
                repeat: false,
                modifiers: UiModifiers {
                    control: true,
                    shift,
                    ..UiModifiers::default()
                },
            }];
            app.update();
        };
        route_history(&mut app, false);
        assert!(
            app.world()
                .get::<FileBrowser>(browser)
                .unwrap()
                .bookmarks
                .is_empty()
        );
        route_history(&mut app, true);
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().bookmarks,
            vec![std::fs::canonicalize(&root).unwrap()]
        );

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                browser,
                WidgetActionKind::FileSetView(FileView::Grid),
            ));
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().view,
            FileView::Grid
        );
        route_history(&mut app, false);
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().view,
            FileView::List
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: browser,
            key: UiKeyCode::A,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        }];
        app.update();
        assert_eq!(
            app.world()
                .get::<FileBrowser>(browser)
                .unwrap()
                .selected
                .len(),
            2
        );
        route_history(&mut app, false);
        assert!(
            app.world()
                .get::<FileBrowser>(browser)
                .unwrap()
                .selected
                .is_empty(),
            "selection state is part of the browser snapshot"
        );

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                browser,
                WidgetActionKind::FileNavigateTo(child.clone()),
            ));
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().directory,
            std::fs::canonicalize(&child).unwrap()
        );
        route_history(&mut app, false);
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().directory,
            std::fs::canonicalize(&root).unwrap()
        );
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .focused,
            Some(browser),
            "undo restores focus only while the browser is still interactable in its window"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_file_operations_are_reported_as_irreversible_side_effects() {
        let root = std::env::temp_dir().join(format!(
            "bui-widgets-browser-side-effect-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let path = root.join("delete-me.txt");
        std::fs::write(&path, "external file").unwrap();
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let mut browser_model = FileBrowser::new(&root).unwrap();
        browser_model.selected.insert(path.clone());
        let browser = app.world_mut().spawn(browser_model).id();

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                browser,
                WidgetActionKind::FileDeleteSelected,
            ));
        app.update();

        assert!(!path.exists());
        assert!(
            app.world()
                .resource::<WidgetEventQueue>()
                .0
                .iter()
                .any(|event| {
                    matches!(event, WidgetEvent::FileOperationCommitted {
                target,
                operation: "delete",
                paths,
                irreversible: true,
            } if *target == browser && paths.as_slice() == std::slice::from_ref(&path))
                })
        );
        assert_eq!(
            app.world().resource::<bui_core::UiHistory>().undo_len(),
            0,
            "filesystem mutations are external side effects, not UI history values"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_partial_delete_reports_only_paths_changed_by_the_operation() {
        let root = std::env::temp_dir().join(format!(
            "bui-widgets-browser-partial-delete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let first = root.join("a.txt");
        let missing = root.join("b.txt");
        std::fs::write(&first, "first").unwrap();
        std::fs::write(&missing, "missing before action").unwrap();
        let mut browser_model = FileBrowser::new(&root).unwrap();
        browser_model
            .selected
            .extend([first.clone(), missing.clone()]);
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let browser = app.world_mut().spawn(browser_model).id();
        std::fs::remove_file(&missing).unwrap();

        app.world_mut()
            .resource_mut::<WidgetActionQueue>()
            .push(WidgetAction::new(
                browser,
                WidgetActionKind::FileDeleteSelected,
            ));
        app.update();

        let failure = app
            .world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .find_map(|event| match event {
                WidgetEvent::FileOperationFailed {
                    target,
                    partial_effects,
                    message,
                    ..
                } if *target == browser => Some((partial_effects, message)),
                _ => None,
            })
            .expect("the stale second entry should report a failed operation");
        assert!(!first.exists());
        assert_eq!(failure.0.as_slice(), std::slice::from_ref(&first));
        assert!(failure.1.contains("1 earlier item(s) were deleted"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

//! Declarative widget trees that instantiate real Bevy entities and components.

use bevy_ecs::prelude::{Component, Entity, With, World};
use bui_core::{
    UiEnabled, UiFocusable, UiHierarchyError, UiKey, UiNode, UiTabIndex, UiVisible, set_parent,
};
use bui_layout::{
    Rect, UiFlexDirection, UiLayoutMode, UiLayoutNode, UiLength, UiScroll, UiWheelHandled,
};
use bui_style::{Rgba, UiInlineStyle, UiRole, UiStyleProperty, UiStyleValue};
use bui_text::{TextBuffer, UiExternalClipboard, UiMultiline, UiTextLayout};
use std::path::PathBuf;

use crate::{
    Button, Canvas, Checkbox, ColorRamp, CurveEditor, DataTable, Dropdown, FileBrowser,
    HsvColorEditor, IconButton, IconButtonAsset, IconButtonAssetState, Label, ListModel,
    MenuControl, MenuDisplay, MenuItemControl, MenuTree, NodeGraph, NumericField, NumericFieldKind,
    Panel, PreviewGrid, Progress, RadioGroup, Slider, TextArea, TextField,
    Tooltip as TooltipControl, TreeModel,
    editors::{ColorChannel, HsvColorEditorState, LinkEndpoint, NodeId, SocketDirection},
    menus::{MenuEntry, PopupState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetContainer {
    Row,
    Column,
    MenuBar,
    Overlay,
    Grid {
        columns: usize,
    },
    ResponsiveGrid {
        min_cell_width: u32,
        max_columns: usize,
    },
    Flex {
        direction: UiFlexDirection,
        wrap: bool,
    },
    Inline,
    Box,
    PropertyRow,
    Scroll,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WidgetSpec {
    Container {
        kind: WidgetContainer,
        gap: f32,
    },
    Label {
        text: String,
    },
    Button {
        label: String,
    },
    IconButton {
        icon: String,
        accessible_name: String,
    },
    IconButtonAsset {
        path: String,
        accessible_name: String,
    },
    Checkbox {
        label: String,
        checked: bool,
    },
    RadioGroup {
        label: String,
        group: RadioGroup,
    },
    Number {
        label: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        step: f64,
        kind: NumericFieldKind,
    },
    Slider {
        label: String,
        slider: Slider,
    },
    SliderRange {
        label: String,
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        kind: NumericFieldKind,
    },
    Panel {
        title: String,
        expanded: bool,
    },
    TextField {
        label: String,
        value: String,
    },
    TextArea {
        label: String,
        value: String,
    },
    Dropdown {
        label: String,
        options: Vec<String>,
        selected: Option<usize>,
    },
    Menu {
        label: String,
        tree: MenuTree,
        display: MenuDisplay,
    },
    Tooltip {
        text: String,
    },
    Progress {
        value: f32,
    },
    FileBrowser {
        directory: PathBuf,
    },
    ColorRamp {
        label: String,
        ramp: ColorRamp,
    },
    HsvColor {
        label: String,
        color: Rgba,
    },
    CurveEditor {
        label: String,
        editor: CurveEditor,
    },
    List {
        label: String,
        model: ListModel<String>,
    },
    Tree {
        label: String,
        model: TreeModel<String>,
    },
    PreviewGrid {
        label: String,
        model: PreviewGrid,
    },
    DataTable {
        label: String,
        model: DataTable,
    },
    NodeGraph {
        label: String,
        model: NodeGraph,
    },
    Canvas {
        label: String,
        model: Canvas,
    },
}

#[derive(Component)]
struct IntrinsicLabelText;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct WidgetElement {
    pub key: Option<String>,
    pub spec: WidgetSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetNode {
    pub key: Option<String>,
    pub spec: WidgetSpec,
    pub children: Vec<WidgetNode>,
    pub layout: Option<UiLayoutNode>,
    pub role: Option<String>,
    pub scrollable: bool,
    pub style: Option<UiInlineStyle>,
}

impl WidgetNode {
    pub fn new(spec: WidgetSpec) -> Self {
        Self {
            key: None,
            spec,
            children: Vec::new(),
            layout: None,
            role: None,
            scrollable: false,
            style: None,
        }
    }
    pub fn column(children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Column, children)
    }
    pub fn row(children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Row, children)
    }
    pub fn menu_bar(menus: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::MenuBar, menus)
    }
    pub fn overlay(children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Overlay, children)
    }
    pub fn grid(columns: usize, children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Grid { columns }, children)
    }
    pub fn responsive_grid(
        min_cell_width: u32,
        max_columns: usize,
        children: impl IntoIterator<Item = Self>,
    ) -> Self {
        Self::container(
            WidgetContainer::ResponsiveGrid {
                min_cell_width,
                max_columns,
            },
            children,
        )
    }
    pub fn flex(
        direction: UiFlexDirection,
        wrap: bool,
        children: impl IntoIterator<Item = Self>,
    ) -> Self {
        Self::container(WidgetContainer::Flex { direction, wrap }, children)
    }
    pub fn inline(children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Inline, children)
    }
    pub fn boxed(children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Box, children)
            .with_role("box")
            .with_width(UiLength::Fill(1.0))
    }
    pub fn property_row(label: impl Into<String>, control: Self) -> Self {
        Self::container(
            WidgetContainer::PropertyRow,
            [
                Self::label(label).with_role("property-label"),
                control.with_width(UiLength::Fill(1.0)),
            ],
        )
        .with_role("property-row")
        .with_width(UiLength::Fill(1.0))
    }
    pub fn scroll(children: impl IntoIterator<Item = Self>) -> Self {
        Self::container(WidgetContainer::Scroll, children)
            .with_role("scroll")
            .with_scroll()
            .with_width(UiLength::Fill(1.0))
    }
    pub fn tooltip(text: impl Into<String>, child: Self) -> Self {
        Self {
            key: None,
            spec: WidgetSpec::Tooltip { text: text.into() },
            children: vec![child],
            layout: None,
            role: None,
            scrollable: false,
            style: None,
        }
    }
    pub fn container(kind: WidgetContainer, children: impl IntoIterator<Item = Self>) -> Self {
        let role = match kind {
            WidgetContainer::Row => Some("layout-row"),
            WidgetContainer::Column => Some("layout-column"),
            WidgetContainer::Grid { .. } | WidgetContainer::ResponsiveGrid { .. } => {
                Some("layout-grid")
            }
            WidgetContainer::Flex { .. } => Some("layout-flex"),
            WidgetContainer::Inline => Some("layout-inline"),
            WidgetContainer::MenuBar
            | WidgetContainer::Overlay
            | WidgetContainer::Box
            | WidgetContainer::PropertyRow
            | WidgetContainer::Scroll => None,
        };
        Self {
            key: None,
            spec: WidgetSpec::Container { kind, gap: 0.0 },
            children: children.into_iter().collect(),
            layout: None,
            role: role.map(str::to_owned),
            scrollable: false,
            style: None,
        }
    }
    pub fn label(text: impl Into<String>) -> Self {
        Self::new(WidgetSpec::Label { text: text.into() })
    }
    pub fn button(label: impl Into<String>) -> Self {
        Self::new(WidgetSpec::Button {
            label: label.into(),
        })
    }
    pub fn icon_button(icon: impl Into<String>, accessible_name: impl Into<String>) -> Self {
        Self::new(WidgetSpec::IconButton {
            icon: icon.into(),
            accessible_name: accessible_name.into(),
        })
    }
    /// Declare an icon button backed by a Bevy image asset path, such as `icons/save.svg`.
    /// The visible control reports loading and failure status and releases its Bevy handle when
    /// it leaves the visible tree.
    pub fn icon_button_asset(path: impl Into<String>, accessible_name: impl Into<String>) -> Self {
        Self::new(WidgetSpec::IconButtonAsset {
            path: path.into(),
            accessible_name: accessible_name.into(),
        })
    }
    pub fn checkbox(label: impl Into<String>, checked: bool) -> Self {
        Self::new(WidgetSpec::Checkbox {
            label: label.into(),
            checked,
        })
    }
    pub fn radio_group(label: impl Into<String>, group: RadioGroup) -> Self {
        Self::new(WidgetSpec::RadioGroup {
            label: label.into(),
            group,
        })
    }
    pub fn number(label: impl Into<String>, value: f64, min: f64, max: f64, step: f64) -> Self {
        Self::new(WidgetSpec::Number {
            label: label.into(),
            value,
            min: Some(min),
            max: Some(max),
            step,
            kind: NumericFieldKind::Float,
        })
    }
    pub fn unbounded_number(label: impl Into<String>, value: f64, step: f64) -> Self {
        Self::new(WidgetSpec::Number {
            label: label.into(),
            value,
            min: None,
            max: None,
            step,
            kind: NumericFieldKind::Float,
        })
    }
    pub fn slider(label: impl Into<String>, slider: Slider) -> Self {
        Self::new(WidgetSpec::Slider {
            label: label.into(),
            slider,
        })
    }
    pub fn slider_range(
        label: impl Into<String>,
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    ) -> Self {
        Self::slider_range_kind(label, value, min, max, step, NumericFieldKind::Float)
    }
    pub fn integer_slider_range(
        label: impl Into<String>,
        value: f64,
        min: f64,
        max: f64,
        step: f64,
    ) -> Self {
        Self::slider_range_kind(label, value, min, max, step, NumericFieldKind::Integer)
    }
    fn slider_range_kind(
        label: impl Into<String>,
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        kind: NumericFieldKind,
    ) -> Self {
        Self::new(WidgetSpec::SliderRange {
            label: label.into(),
            value,
            min,
            max,
            step,
            kind,
        })
    }
    pub fn panel(
        title: impl Into<String>,
        expanded: bool,
        children: impl IntoIterator<Item = Self>,
    ) -> Self {
        Self {
            key: None,
            spec: WidgetSpec::Panel {
                title: title.into(),
                expanded,
            },
            children: children.into_iter().collect(),
            layout: None,
            role: None,
            scrollable: false,
            style: None,
        }
    }
    pub fn text_field(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(WidgetSpec::TextField {
            label: label.into(),
            value: value.into(),
        })
    }
    pub fn text_area(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(WidgetSpec::TextArea {
            label: label.into(),
            value: value.into(),
        })
    }
    pub fn dropdown(
        label: impl Into<String>,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<usize>,
    ) -> Self {
        Self::new(WidgetSpec::Dropdown {
            label: label.into(),
            options: options.into_iter().map(Into::into).collect(),
            selected,
        })
    }
    pub fn menu(label: impl Into<String>, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        Self::new(WidgetSpec::Menu {
            label: label.into(),
            tree: MenuTree::new(entries.into_iter().collect()),
            display: MenuDisplay::Dropdown,
        })
    }
    pub fn pie_menu(
        label: impl Into<String>,
        entries: impl IntoIterator<Item = MenuEntry>,
    ) -> Self {
        Self::new(WidgetSpec::Menu {
            label: label.into(),
            tree: MenuTree::new(entries.into_iter().collect()),
            display: MenuDisplay::Pie,
        })
    }
    pub fn popover(label: impl Into<String>, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
        Self::new(WidgetSpec::Menu {
            label: label.into(),
            tree: MenuTree::new(entries.into_iter().collect()),
            display: MenuDisplay::Popover,
        })
    }
    pub fn context_menu(
        label: impl Into<String>,
        entries: impl IntoIterator<Item = MenuEntry>,
    ) -> Self {
        Self::new(WidgetSpec::Menu {
            label: label.into(),
            tree: MenuTree::new(entries.into_iter().collect()),
            display: MenuDisplay::Context,
        })
    }
    pub fn progress(value: f32) -> Self {
        Self::new(WidgetSpec::Progress { value })
    }
    pub fn file_browser(directory: impl Into<PathBuf>) -> Self {
        Self::new(WidgetSpec::FileBrowser {
            directory: directory.into(),
        })
    }
    pub fn color_ramp(label: impl Into<String>, ramp: ColorRamp) -> Self {
        Self::new(WidgetSpec::ColorRamp {
            label: label.into(),
            ramp,
        })
    }
    pub fn color_ramp_default(label: impl Into<String>) -> Self {
        Self::color_ramp(label, ColorRamp::default())
    }
    pub fn hsv_color(label: impl Into<String>, color: Rgba) -> Self {
        Self::new(WidgetSpec::HsvColor {
            label: label.into(),
            color,
        })
    }
    pub fn hsv_color_default(label: impl Into<String>) -> Self {
        Self::hsv_color(label, Rgba(1.0, 0.0, 0.0, 1.0))
    }
    pub fn curve_editor(label: impl Into<String>, editor: CurveEditor) -> Self {
        Self::new(WidgetSpec::CurveEditor {
            label: label.into(),
            editor,
        })
    }
    pub fn curve_editor_default(label: impl Into<String>) -> Self {
        Self::curve_editor(label, CurveEditor::default())
    }
    pub fn list(label: impl Into<String>, model: ListModel<String>) -> Self {
        Self::new(WidgetSpec::List {
            label: label.into(),
            model,
        })
    }
    pub fn list_default(label: impl Into<String>) -> Self {
        Self::list(label, ListModel::default())
    }
    pub fn tree(label: impl Into<String>, model: TreeModel<String>) -> Self {
        Self::new(WidgetSpec::Tree {
            label: label.into(),
            model,
        })
    }
    pub fn tree_default(label: impl Into<String>) -> Self {
        Self::tree(label, TreeModel::default())
    }
    pub fn preview_grid(label: impl Into<String>, model: PreviewGrid) -> Self {
        Self::new(WidgetSpec::PreviewGrid {
            label: label.into(),
            model,
        })
    }
    pub fn preview_grid_default(label: impl Into<String>) -> Self {
        Self::preview_grid(label, PreviewGrid::default())
    }
    pub fn data_table(label: impl Into<String>, model: DataTable) -> Self {
        Self::new(WidgetSpec::DataTable {
            label: label.into(),
            model,
        })
    }
    pub fn data_table_default(label: impl Into<String>) -> Self {
        Self::data_table(
            label,
            DataTable {
                columns: vec![crate::TableColumn::new("Column", 180.0)],
                ..DataTable::default()
            },
        )
    }
    pub fn node_graph(label: impl Into<String>, model: NodeGraph) -> Self {
        Self::new(WidgetSpec::NodeGraph {
            label: label.into(),
            model,
        })
    }
    pub fn node_graph_default(label: impl Into<String>) -> Self {
        Self::node_graph(label, NodeGraph::default())
    }
    pub fn canvas(label: impl Into<String>, model: Canvas) -> Self {
        Self::new(WidgetSpec::Canvas {
            label: label.into(),
            model,
        })
    }
    pub fn canvas_default(label: impl Into<String>) -> Self {
        Self::canvas(label, Canvas::default())
    }
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn with_layout(mut self, layout: UiLayoutNode) -> Self {
        self.layout = Some(layout);
        self
    }
    pub fn with_width(mut self, width: UiLength) -> Self {
        let mut layout = self.layout.unwrap_or_else(|| layout_for(&self.spec));
        layout.size.width = width;
        self.layout = Some(layout);
        self.style
            .get_or_insert_with(UiInlineStyle::default)
            .set(UiStyleProperty::Width, UiStyleValue::Length(width));
        self
    }
    pub fn with_height(mut self, height: UiLength) -> Self {
        let mut layout = self.layout.unwrap_or_else(|| layout_for(&self.spec));
        layout.size.height = height;
        self.layout = Some(layout);
        self.style
            .get_or_insert_with(UiInlineStyle::default)
            .set(UiStyleProperty::Height, UiStyleValue::Length(height));
        self
    }
    pub fn with_gap(mut self, gap: f32) -> Self {
        let gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
        let mut layout = self.layout.unwrap_or_else(|| layout_for(&self.spec));
        layout.gap = gap;
        self.layout = Some(layout);
        self.style
            .get_or_insert_with(UiInlineStyle::default)
            .set(UiStyleProperty::Gap, UiStyleValue::Scalar(gap));
        self
    }
    pub fn with_style(mut self, style: UiInlineStyle) -> Self {
        self.style = Some(style);
        self
    }
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }
    pub fn with_scroll(mut self) -> Self {
        self.scrollable = true;
        self
    }
    pub fn with_child(mut self, child: Self) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetTree {
    pub root: WidgetNode,
}

impl WidgetTree {
    pub fn new(root: WidgetNode) -> Self {
        Self { root }
    }

    /// Spawn every declaration node as a Bevy entity, preserving tree order in `UiChildren`.
    ///
    /// The root is also attached to `parent` when supplied. Number declarations are validated
    /// before any entity is created, so invalid values cannot leave behind a partial tree.
    pub fn spawn(
        &self,
        world: &mut World,
        parent: Option<Entity>,
    ) -> Result<Entity, WidgetTreeError> {
        if let Some(parent) = parent
            && world.get::<UiNode>(parent).is_none()
        {
            return Err(WidgetTreeError::Hierarchy(UiHierarchyError::MissingParent));
        }
        validate_node(&self.root)?;
        spawn_node(world, &self.root, parent)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum WidgetTreeError {
    InvalidLayout {
        key: Option<String>,
        message: &'static str,
    },
    InvalidNumberField {
        key: Option<String>,
    },
    InvalidSlider {
        key: Option<String>,
        message: &'static str,
    },
    Hierarchy(UiHierarchyError),
    FileBrowser {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    InvalidColorRamp {
        key: Option<String>,
        message: String,
    },
    InvalidHsvColor {
        key: Option<String>,
        message: String,
    },
    InvalidCurveEditor {
        key: Option<String>,
        message: String,
    },
    InvalidCollection {
        key: Option<String>,
        message: String,
    },
    InvalidDataTable {
        key: Option<String>,
        message: String,
    },
    InvalidNodeGraph {
        key: Option<String>,
        message: String,
    },
    InvalidCanvas {
        key: Option<String>,
        message: String,
    },
    InvalidMenu {
        key: Option<String>,
        message: String,
    },
}

fn validate_node(node: &WidgetNode) -> Result<(), WidgetTreeError> {
    if let WidgetSpec::Container { kind, .. } = &node.spec {
        let invalid = match kind {
            WidgetContainer::Grid { columns: 0 } => Some("grid columns must be positive"),
            WidgetContainer::ResponsiveGrid {
                min_cell_width: 0, ..
            } => Some("responsive grid minimum cell width must be positive"),
            WidgetContainer::ResponsiveGrid { max_columns: 0, .. } => {
                Some("responsive grid maximum columns must be positive")
            }
            _ => None,
        };
        if let Some(message) = invalid {
            return Err(WidgetTreeError::InvalidLayout {
                key: node.key.clone(),
                message,
            });
        }
    }
    if let WidgetSpec::Number {
        value,
        min,
        max,
        step,
        kind,
        ..
    } = &node.spec
    {
        NumericField::new(*value, *min, *max, *step, *kind).map_err(|_| {
            WidgetTreeError::InvalidNumberField {
                key: node.key.clone(),
            }
        })?;
    }
    if let WidgetSpec::Slider { slider, .. } = &node.spec {
        Slider::from_field(slider.field.clone()).map_err(|message| {
            WidgetTreeError::InvalidSlider {
                key: node.key.clone(),
                message,
            }
        })?;
    }
    if let WidgetSpec::SliderRange {
        value,
        min,
        max,
        step,
        kind,
        ..
    } = &node.spec
    {
        Slider::with_kind(*value, *min, *max, *step, *kind).map_err(|message| {
            WidgetTreeError::InvalidSlider {
                key: node.key.clone(),
                message,
            }
        })?;
    }
    if let WidgetSpec::FileBrowser { directory } = &node.spec {
        FileBrowser::new(directory).map_err(|error| WidgetTreeError::FileBrowser {
            operation: error.operation,
            path: error.path,
            message: error.source.to_string(),
        })?;
    }
    if let WidgetSpec::ColorRamp { ramp, .. } = &node.spec {
        ramp.validate()
            .map_err(|message| WidgetTreeError::InvalidColorRamp {
                key: node.key.clone(),
                message,
            })?;
    }
    if let WidgetSpec::HsvColor { color, .. } = &node.spec {
        HsvColorEditor::from_rgba(*color).map_err(|message| WidgetTreeError::InvalidHsvColor {
            key: node.key.clone(),
            message,
        })?;
    }
    if let WidgetSpec::CurveEditor { editor, .. } = &node.spec {
        editor
            .validate()
            .map_err(|message| WidgetTreeError::InvalidCurveEditor {
                key: node.key.clone(),
                message,
            })?;
    }
    let collection_validation = match &node.spec {
        WidgetSpec::List { model, .. } => model.validate(),
        WidgetSpec::Tree { model, .. } => model.validate(),
        WidgetSpec::PreviewGrid { model, .. } => model.validate(),
        _ => Ok(()),
    };
    collection_validation.map_err(|message| WidgetTreeError::InvalidCollection {
        key: node.key.clone(),
        message,
    })?;
    if let WidgetSpec::DataTable { model, .. } = &node.spec {
        model
            .validate()
            .map_err(|message| WidgetTreeError::InvalidDataTable {
                key: node.key.clone(),
                message,
            })?;
    }
    if let WidgetSpec::NodeGraph { model, .. } = &node.spec {
        model
            .validate()
            .map_err(|message| WidgetTreeError::InvalidNodeGraph {
                key: node.key.clone(),
                message,
            })?;
    }
    if let WidgetSpec::Canvas { model, .. } = &node.spec {
        let json = model
            .to_json()
            .map_err(|error| WidgetTreeError::InvalidCanvas {
                key: node.key.clone(),
                message: error.to_string(),
            })?;
        Canvas::from_json(&json).map_err(|message| WidgetTreeError::InvalidCanvas {
            key: node.key.clone(),
            message,
        })?;
    }
    if let WidgetSpec::Menu { tree, .. } = &node.spec {
        tree.validate()
            .map_err(|message| WidgetTreeError::InvalidMenu {
                key: node.key.clone(),
                message: message.to_owned(),
            })?;
    }
    for child in &node.children {
        validate_node(child)?;
    }
    Ok(())
}

fn spawn_node(
    world: &mut World,
    node: &WidgetNode,
    parent: Option<Entity>,
) -> Result<Entity, WidgetTreeError> {
    let mut layout = node.layout.unwrap_or_else(|| layout_for(&node.spec));
    if node.scrollable {
        layout.clip_contents = true;
    }
    let mut entity = world.spawn((
        UiNode,
        UiEnabled(true),
        UiVisible(true),
        layout,
        WidgetElement {
            key: node.key.clone(),
            spec: node.spec.clone(),
        },
    ));
    if let Some(key) = &node.key {
        entity.insert(UiKey(key.clone()));
    }
    if matches!(
        &node.spec,
        WidgetSpec::Container {
            kind: WidgetContainer::MenuBar,
            ..
        }
    ) {
        entity.insert(UiRole::new("menu-bar"));
    }
    if matches!(&node.spec, WidgetSpec::Tooltip { .. }) {
        entity.insert(UiRole::new("tooltip-host"));
    }
    if let Some(role) = &node.role {
        entity.insert(UiRole::new(role.clone()));
    }
    if let Some(style) = &node.style {
        entity.insert(style.clone());
    }
    if node.scrollable {
        entity.insert(UiScroll::new());
    }
    if matches!(
        &node.spec,
        WidgetSpec::Button { .. }
            | WidgetSpec::IconButton { .. }
            | WidgetSpec::IconButtonAsset { .. }
            | WidgetSpec::Checkbox { .. }
            | WidgetSpec::RadioGroup { .. }
            | WidgetSpec::Number { .. }
            | WidgetSpec::Slider { .. }
            | WidgetSpec::SliderRange { .. }
            | WidgetSpec::Panel { .. }
            | WidgetSpec::TextField { .. }
            | WidgetSpec::TextArea { .. }
            | WidgetSpec::Dropdown { .. }
            | WidgetSpec::Menu { .. }
            | WidgetSpec::FileBrowser { .. }
            | WidgetSpec::ColorRamp { .. }
            | WidgetSpec::HsvColor { .. }
            | WidgetSpec::CurveEditor { .. }
            | WidgetSpec::List { .. }
            | WidgetSpec::Tree { .. }
            | WidgetSpec::PreviewGrid { .. }
            | WidgetSpec::DataTable { .. }
            | WidgetSpec::NodeGraph { .. }
            | WidgetSpec::Canvas { .. }
    ) {
        entity.insert((UiFocusable, UiTabIndex(0)));
    }
    let id = entity.id();
    if let Some(parent) = parent {
        set_parent(world, id, parent).map_err(WidgetTreeError::Hierarchy)?;
    }
    insert_control(world, id, &node.spec)?;
    if let WidgetSpec::Menu { tree, .. } = &node.spec {
        spawn_menu_items(world, id, tree);
    }
    for child in &node.children {
        let child_id = spawn_node(world, child, Some(id))?;
        if matches!(
            &node.spec,
            WidgetSpec::Panel {
                expanded: false,
                ..
            }
        ) {
            world.entity_mut(child_id).insert(UiVisible(false));
        }
    }
    Ok(id)
}

fn spawn_menu_items(world: &mut World, owner: Entity, tree: &MenuTree) {
    let mut item_entities = Vec::new();
    for (parent_path, index, entry) in tree.flattened() {
        let item = world
            .spawn((
                UiNode,
                UiEnabled(entry.enabled && !entry.separator),
                UiVisible(false),
                UiRole::new("menu-item"),
                MenuItemControl {
                    owner,
                    id: entry.id.clone(),
                    label: entry.label.clone(),
                    parent_path,
                    index,
                    enabled: entry.enabled,
                    separator: entry.separator,
                    has_submenu: !entry.children.is_empty(),
                    shortcut: entry.shortcut.clone(),
                },
            ))
            .id();
        let _ = set_parent(world, item, owner);
        item_entities.push(item);
    }
    if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
        menu.items = item_entities;
    }
}

fn layout_for(spec: &WidgetSpec) -> UiLayoutNode {
    match spec {
        WidgetSpec::Container { kind, gap } => UiLayoutNode {
            mode: match kind {
                WidgetContainer::Row | WidgetContainer::MenuBar => UiLayoutMode::Row,
                WidgetContainer::Column => UiLayoutMode::Column,
                WidgetContainer::Overlay => UiLayoutMode::Overlay,
                WidgetContainer::Grid { columns } => UiLayoutMode::Grid { columns: *columns },
                WidgetContainer::ResponsiveGrid {
                    min_cell_width,
                    max_columns,
                } => UiLayoutMode::ResponsiveGrid {
                    min_cell_width: *min_cell_width,
                    max_columns: *max_columns,
                },
                WidgetContainer::Flex { direction, wrap } => UiLayoutMode::Flex {
                    direction: *direction,
                    wrap: *wrap,
                },
                WidgetContainer::Inline => UiLayoutMode::Inline,
                WidgetContainer::Box | WidgetContainer::Scroll => UiLayoutMode::Column,
                WidgetContainer::PropertyRow => UiLayoutMode::Row,
            },
            gap: if gap.is_finite() { gap.max(0.0) } else { 0.0 },
            ..UiLayoutNode::default()
        },
        WidgetSpec::Label { .. } => UiLayoutNode::leaf(UiLength::Auto, UiLength::Px(24.0)),
        WidgetSpec::Tooltip { .. } => UiLayoutNode::column(0.0),
        WidgetSpec::Panel { .. } => UiLayoutNode {
            mode: UiLayoutMode::Column,
            gap: 4.0,
            ..UiLayoutNode::default()
        },
        WidgetSpec::RadioGroup { group, .. } => UiLayoutNode::leaf(
            UiLength::Px(220.0),
            UiLength::Px(24.0 * (group.options.len().max(1) as f32 + 1.0)),
        ),
        WidgetSpec::Slider { .. } | WidgetSpec::SliderRange { .. } => {
            UiLayoutNode::leaf(UiLength::Px(260.0), UiLength::Px(32.0))
        }
        WidgetSpec::IconButton { .. } => UiLayoutNode::leaf(UiLength::Px(32.0), UiLength::Px(28.0)),
        WidgetSpec::IconButtonAsset { .. } => {
            UiLayoutNode::leaf(UiLength::Px(32.0), UiLength::Px(28.0))
        }
        WidgetSpec::TextArea { .. } => UiLayoutNode::leaf(UiLength::Px(220.0), UiLength::Px(96.0)),
        WidgetSpec::FileBrowser { .. } => {
            UiLayoutNode::leaf(UiLength::Px(480.0), UiLength::Px(320.0))
        }
        WidgetSpec::ColorRamp { .. } => {
            UiLayoutNode::leaf(UiLength::Px(360.0), UiLength::Px(194.0))
        }
        WidgetSpec::HsvColor { .. } => UiLayoutNode::leaf(UiLength::Px(448.0), UiLength::Px(236.0)),
        WidgetSpec::CurveEditor { .. } => {
            UiLayoutNode::leaf(UiLength::Px(440.0), UiLength::Px(280.0))
        }
        WidgetSpec::List { .. } => UiLayoutNode::leaf(UiLength::Px(300.0), UiLength::Px(300.0)),
        WidgetSpec::Tree { .. } => UiLayoutNode::leaf(UiLength::Px(320.0), UiLength::Px(340.0)),
        WidgetSpec::PreviewGrid { .. } => {
            UiLayoutNode::leaf(UiLength::Px(440.0), UiLength::Px(340.0))
        }
        WidgetSpec::DataTable { .. } => {
            UiLayoutNode::leaf(UiLength::Px(620.0), UiLength::Px(360.0))
        }
        WidgetSpec::NodeGraph { .. } => {
            UiLayoutNode::leaf(UiLength::Px(620.0), UiLength::Px(440.0))
        }
        WidgetSpec::Canvas { .. } => UiLayoutNode::leaf(UiLength::Px(640.0), UiLength::Px(420.0)),
        _ => UiLayoutNode::leaf(UiLength::Px(160.0), UiLength::Px(24.0)),
    }
}

fn insert_control(
    world: &mut World,
    entity: Entity,
    spec: &WidgetSpec,
) -> Result<(), WidgetTreeError> {
    match spec {
        WidgetSpec::Container { .. } => {}
        WidgetSpec::Tooltip { text } => {
            world
                .entity_mut(entity)
                .insert(TooltipControl::new(text.clone()));
        }
        WidgetSpec::Label { text } => {
            let mut text_layout = UiTextLayout::default();
            // Labels paint from their layout origin, so intrinsic measurement should not reserve
            // the editor inset used by text fields.
            text_layout.inset = [0.0, 0.0];
            world.entity_mut(entity).insert((
                Label::new(text.clone()),
                TextBuffer::new(text.clone()),
                text_layout,
                IntrinsicLabelText,
            ));
        }
        WidgetSpec::Button { label } => {
            world.entity_mut(entity).insert(Button::new(label.clone()));
        }
        WidgetSpec::IconButton {
            icon,
            accessible_name,
        } => {
            world.entity_mut(entity).insert((
                IconButton::new(icon.clone(), accessible_name.clone()),
                UiRole::new("button"),
            ));
        }
        WidgetSpec::IconButtonAsset {
            path,
            accessible_name,
        } => {
            world.entity_mut(entity).insert((
                IconButton::new("", accessible_name.clone()),
                IconButtonAsset::new(path.clone()),
                IconButtonAssetState::default(),
                UiRole::new("button"),
            ));
        }
        WidgetSpec::Checkbox { label, checked } => {
            world
                .entity_mut(entity)
                .insert(Checkbox::new(label.clone(), *checked));
        }
        WidgetSpec::RadioGroup { label, group } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                group.clone(),
                UiRole::new("radio-group"),
            ));
        }
        WidgetSpec::Number {
            label,
            value,
            min,
            max,
            step,
            kind,
        } => {
            let field = NumericField::new(*value, *min, *max, *step, *kind)
                .map_err(|_| WidgetTreeError::InvalidNumberField { key: None })?;
            world
                .entity_mut(entity)
                .insert((Label::new(label.clone()), field));
        }
        WidgetSpec::Slider { label, slider } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                slider.clone(),
                UiRole::new("slider"),
            ));
        }
        WidgetSpec::SliderRange {
            label,
            value,
            min,
            max,
            step,
            kind,
        } => {
            let slider = Slider::with_kind(*value, *min, *max, *step, *kind)
                .map_err(|message| WidgetTreeError::InvalidSlider { key: None, message })?;
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                slider,
                UiRole::new("slider"),
            ));
        }
        WidgetSpec::Panel { title, expanded } => {
            world
                .entity_mut(entity)
                .insert(Panel::new(title.clone(), *expanded));
        }
        WidgetSpec::TextField { label, value } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                TextField::new(value.clone()),
                TextBuffer::new(value.clone()),
                UiExternalClipboard,
                UiTextLayout::text_field(label.clone()),
            ));
        }
        WidgetSpec::TextArea { label, value } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                TextArea::new(value.clone()),
                TextBuffer::new(value.clone()),
                UiExternalClipboard,
                UiMultiline,
                UiTextLayout::text_area(label.clone(), true),
                UiWheelHandled,
            ));
        }
        WidgetSpec::Dropdown {
            label,
            options,
            selected,
        } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                Dropdown::new(options.clone(), *selected),
                UiWheelHandled,
            ));
        }
        WidgetSpec::Menu {
            label,
            tree,
            display,
        } => {
            let role = match display {
                MenuDisplay::Dropdown => "menu-trigger",
                MenuDisplay::Popover => "popover-trigger",
                MenuDisplay::Context => "context-menu-trigger",
                MenuDisplay::Pie => "pie-menu-trigger",
            };
            world.entity_mut(entity).insert((
                MenuControl::new(label.clone(), tree.clone(), *display),
                UiRole::new(role),
            ));
        }
        WidgetSpec::Progress { value } => {
            world.entity_mut(entity).insert(Progress::new(*value));
        }
        WidgetSpec::FileBrowser { directory } => {
            let browser =
                FileBrowser::new(directory).map_err(|error| WidgetTreeError::FileBrowser {
                    operation: error.operation,
                    path: error.path,
                    message: error.source.to_string(),
                })?;
            world.entity_mut(entity).insert((
                browser,
                FileBrowserViewState::default(),
                UiWheelHandled,
            ));
        }
        WidgetSpec::ColorRamp { label, ramp } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                ramp.clone(),
                ColorRampEditorState::default(),
            ));
        }
        WidgetSpec::HsvColor { label, color } => {
            let editor = HsvColorEditor::from_rgba(*color)
                .map_err(|message| WidgetTreeError::InvalidHsvColor { key: None, message })?;
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                editor,
                HsvColorEditorState::default(),
            ));
        }
        WidgetSpec::CurveEditor { label, editor } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                editor.clone(),
                UiWheelHandled,
            ));
        }
        WidgetSpec::List { label, model } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                model.clone(),
                CollectionEditorState::default(),
                UiWheelHandled,
            ));
        }
        WidgetSpec::Tree { label, model } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                model.clone(),
                CollectionEditorState::default(),
                UiWheelHandled,
            ));
        }
        WidgetSpec::PreviewGrid { label, model } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                model.clone(),
                CollectionEditorState::default(),
                UiWheelHandled,
            ));
        }
        WidgetSpec::DataTable { label, model } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                model.clone(),
                UiRole::new("table"),
                UiWheelHandled,
            ));
        }
        WidgetSpec::NodeGraph { label, model } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                model.clone(),
                NodeGraphEditorState::default(),
                UiRole::new("node-graph"),
                UiWheelHandled,
            ));
        }
        WidgetSpec::Canvas { label, model } => {
            world.entity_mut(entity).insert((
                Label::new(label.clone()),
                model.clone(),
                UiRole::new("canvas"),
                UiWheelHandled,
            ));
        }
    }
    Ok(())
}

/// Keep the private measurement buffer for plain labels in sync before TextMeasure.
/// Label entities are deliberately not focusable, so this buffer only feeds shaping and
/// intrinsic layout; it does not make labels editable.
pub(crate) fn sync_intrinsic_label_text(world: &mut World) {
    let mut query = world.query_filtered::<(&Label, &mut TextBuffer), With<IntrinsicLabelText>>();
    for (label, mut buffer) in query.iter_mut(world) {
        if buffer.as_str() != label.text {
            *buffer = TextBuffer::new(label.text.clone());
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IconButtonAssetStatus;
    use bevy_app::App;
    use bevy_ecs::prelude::World;
    use bui_core::{UiChildren, UiFocusable, UiParent};
    use bui_layout::UiIntrinsicSize;
    use bui_style::Theme;
    use bui_text::TextPlugin;

    #[test]
    fn label_intrinsic_width_uses_shared_font_and_tracks_theme_size() {
        let text = "NodeGraph · typed sockets — Ωmega";
        let mut app = App::new();
        app.add_plugins((TextPlugin, crate::WidgetPlugin));
        let entity = WidgetTree::new(WidgetNode::label(text))
            .spawn(app.world_mut(), None)
            .unwrap();
        assert!(app.world().get::<UiFocusable>(entity).is_none());
        app.update();

        let first_width = app
            .world()
            .get::<UiIntrinsicSize>(entity)
            .expect("TextPlugin measures declarative labels")
            .0[0];
        assert!(
            first_width > 160.0,
            "the full Unicode title must not be clipped to the old 160px default: {first_width}"
        );

        app.world_mut().get_mut::<Label>(entity).unwrap().text =
            "A runtime replacement label with a longer value — Ωmega".to_owned();
        app.update();
        let replaced_width = app.world().get::<UiIntrinsicSize>(entity).unwrap().0[0];
        assert!(replaced_width > first_width);

        app.world_mut().resource_mut::<Theme>().font_size = 24.0;
        app.update();
        let larger_width = app.world().get::<UiIntrinsicSize>(entity).unwrap().0[0];
        assert!(
            larger_width > replaced_width * 1.4,
            "intrinsic label width must track the renderer's theme font size: {replaced_width} -> {larger_width}"
        );
    }

    #[test]
    fn tree_spawns_typed_bevy_controls_and_hierarchy() {
        let mut world = World::new();
        let parent = world.spawn(UiNode).id();
        let tree = WidgetTree::new(WidgetNode::column([
            WidgetNode::label("Settings").with_key("heading"),
            WidgetNode::checkbox("Visible", true).with_key("visible"),
            WidgetNode::panel(
                "Advanced",
                false,
                [WidgetNode::number("Scale", 1.0, 0.0, 10.0, 0.1)],
            ),
        ]));
        let root = tree.spawn(&mut world, Some(parent)).unwrap();
        let children = &world.get::<UiChildren>(root).unwrap().0;
        assert_eq!(children.len(), 3);
        assert!(world.get::<Label>(children[0]).is_some());
        assert!(world.get::<Checkbox>(children[1]).unwrap().is_checked());
        assert_eq!(world.get::<UiParent>(root).unwrap().0, parent);
        let panel_children = &world.get::<UiChildren>(children[2]).unwrap().0;
        assert!(!world.get::<UiVisible>(panel_children[0]).unwrap().0);
    }

    #[test]
    fn tooltip_declaration_wraps_a_live_child_control() {
        let mut world = World::new();
        let host = WidgetTree::new(WidgetNode::tooltip(
            "Apply the pending edits",
            WidgetNode::button("Apply").with_key("apply"),
        ))
        .spawn(&mut world, None)
        .unwrap();
        let child = world.get::<UiChildren>(host).unwrap().0[0];
        assert_eq!(
            world.get::<TooltipControl>(host).unwrap().text,
            "Apply the pending edits"
        );
        assert!(world.get::<Button>(child).is_some());
        assert_eq!(world.get::<UiKey>(child).unwrap().0, "apply");
        assert_eq!(world.get::<UiRole>(host).unwrap().0, "tooltip-host");
        assert_eq!(world.get::<TooltipControl>(host).unwrap().delay_millis, 400);
    }

    #[test]
    fn radio_and_icon_declarations_spawn_focusable_accessible_controls() {
        let mut group = RadioGroup::new(["Object", "Edit", "Sculpt"], Some(1));
        group.options[2].enabled = false;
        let tree = WidgetTree::new(WidgetNode::row([
            WidgetNode::radio_group("Mode", group.clone()).with_key("mode"),
            WidgetNode::icon_button("+", "Add object").with_key("add-object"),
        ]));
        let mut world = World::new();
        let root = tree.spawn(&mut world, None).unwrap();
        let [radio, icon] = world.get::<UiChildren>(root).unwrap().0.as_slice() else {
            panic!("expected two control declarations");
        };

        assert!(world.get::<UiFocusable>(*radio).is_some());
        assert_eq!(world.get::<UiRole>(*radio).unwrap().0, "radio-group");
        assert_eq!(world.get::<Label>(*radio).unwrap().text, "Mode");
        assert_eq!(world.get::<RadioGroup>(*radio).unwrap(), &group);

        assert!(world.get::<UiFocusable>(*icon).is_some());
        assert_eq!(world.get::<UiRole>(*icon).unwrap().0, "button");
        assert_eq!(
            world.get::<IconButton>(*icon).unwrap().accessible_name,
            "Add object"
        );
    }

    #[test]
    fn image_icon_declaration_keeps_asset_path_and_load_state_on_button() {
        let mut world = World::new();
        let entity = WidgetTree::new(
            WidgetNode::icon_button_asset("icons/add.svg", "Add object").with_key("add"),
        )
        .spawn(&mut world, None)
        .unwrap();

        assert_eq!(world.get::<IconButton>(entity).unwrap().icon, "");
        assert_eq!(
            world.get::<IconButton>(entity).unwrap().accessible_name,
            "Add object"
        );
        assert_eq!(
            world.get::<IconButtonAsset>(entity).unwrap().path,
            "icons/add.svg"
        );
        assert_eq!(
            world.get::<IconButtonAssetState>(entity).unwrap().status,
            IconButtonAssetStatus::Hidden
        );
        assert_eq!(world.get::<UiKey>(entity).unwrap().0, "add");
        assert!(world.get::<UiFocusable>(entity).is_some());
    }

    #[test]
    fn slider_declaration_spawns_a_focusable_bounded_control() {
        let slider = Slider::new(0.4, 0.0, 1.0, 0.05).unwrap().with_unit("m");
        let mut world = World::new();
        let entity =
            WidgetTree::new(WidgetNode::slider("Distance", slider.clone()).with_key("distance"))
                .spawn(&mut world, None)
                .unwrap();

        assert_eq!(world.get::<Slider>(entity).unwrap(), &slider);
        assert_eq!(world.get::<Label>(entity).unwrap().text, "Distance");
        assert_eq!(world.get::<UiRole>(entity).unwrap().0, "slider");
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert_eq!(world.get::<UiKey>(entity).unwrap().0, "distance");
        assert_eq!(
            world.get::<UiLayoutNode>(entity).unwrap().size.width,
            UiLength::Px(260.0)
        );
    }

    #[test]
    fn slider_range_declarations_validate_atomically_and_support_integer_values() {
        let mut world = World::new();
        let entity = WidgetTree::new(WidgetNode::integer_slider_range(
            "Count", 4.0, 0.0, 12.0, 1.0,
        ))
        .spawn(&mut world, None)
        .unwrap();
        let slider = world.get::<Slider>(entity).unwrap();
        assert_eq!(slider.kind, NumericFieldKind::Integer);
        assert_eq!(slider.value, 4.0);
        assert_eq!(slider.range(), [0.0, 12.0]);

        let mut empty_world = World::new();
        let mut query = empty_world.query::<Entity>();
        let before = query.iter(&empty_world).count();
        let invalid = WidgetTree::new(WidgetNode::column([
            WidgetNode::button("created only on success"),
            WidgetNode::slider_range("Bad", 0.5, -1.0e308, 1.0e308, 0.1),
        ]));
        assert!(matches!(
            invalid.spawn(&mut empty_world, None),
            Err(WidgetTreeError::InvalidSlider { .. })
        ));
        assert_eq!(query.iter(&empty_world).count(), before);
    }

    #[test]
    fn invalid_number_declaration_does_not_leave_partial_entities() {
        let mut world = World::new();
        let mut query = world.query::<Entity>();
        let before = query.iter(&world).count();
        let tree = WidgetTree::new(WidgetNode::column([
            WidgetNode::button("created only on success"),
            WidgetNode::number("bad", 4.0, 5.0, 1.0, 0.5),
        ]));
        assert!(tree.spawn(&mut world, None).is_err());
        assert_eq!(query.iter(&world).count(), before);
    }

    #[test]
    fn layout_containers_preserve_modes_roles_sizes_and_scroll_clips() {
        let mut world = World::new();
        let root = WidgetTree::new(WidgetNode::column([
            WidgetNode::overlay([WidgetNode::button("Overlay")]),
            WidgetNode::grid(2, [WidgetNode::button("A"), WidgetNode::button("B")]),
            WidgetNode::responsive_grid(80, 3, [WidgetNode::button("Cell")]),
            WidgetNode::flex(UiFlexDirection::Row, true, [WidgetNode::button("Wrap")]),
            WidgetNode::inline([WidgetNode::label("Inline")]),
            WidgetNode::boxed([WidgetNode::button("Boxed")]),
            WidgetNode::property_row("Scale", WidgetNode::number("Value", 1.0, 0.0, 2.0, 0.1)),
            WidgetNode::scroll([WidgetNode::button("Scroll")]).with_height(UiLength::Px(40.0)),
            WidgetNode::row([WidgetNode::button("Custom gap")])
                .with_gap(13.0)
                .with_width(UiLength::Px(320.0)),
        ]))
        .spawn(&mut world, None)
        .unwrap();
        let children = world.get::<UiChildren>(root).unwrap().0.clone();
        assert_eq!(world.get::<UiRole>(root).unwrap().0, "layout-column");
        assert!(matches!(
            world.get::<UiLayoutNode>(children[0]).unwrap().mode,
            UiLayoutMode::Overlay
        ));
        assert!(matches!(
            world.get::<UiLayoutNode>(children[1]).unwrap().mode,
            UiLayoutMode::Grid { columns: 2 }
        ));
        assert_eq!(world.get::<UiRole>(children[1]).unwrap().0, "layout-grid");
        assert!(matches!(
            world.get::<UiLayoutNode>(children[2]).unwrap().mode,
            UiLayoutMode::ResponsiveGrid {
                min_cell_width: 80,
                max_columns: 3
            }
        ));
        assert!(matches!(
            world.get::<UiLayoutNode>(children[3]).unwrap().mode,
            UiLayoutMode::Flex {
                direction: UiFlexDirection::Row,
                wrap: true
            }
        ));
        assert!(matches!(
            world.get::<UiLayoutNode>(children[4]).unwrap().mode,
            UiLayoutMode::Inline
        ));
        assert_eq!(world.get::<UiRole>(children[5]).unwrap().0, "box");
        assert_eq!(world.get::<UiRole>(children[6]).unwrap().0, "property-row");
        let property_children = &world.get::<UiChildren>(children[6]).unwrap().0;
        assert_eq!(
            world.get::<UiRole>(property_children[0]).unwrap().0,
            "property-label"
        );
        assert_eq!(
            world
                .get::<UiLayoutNode>(property_children[1])
                .unwrap()
                .size
                .width,
            UiLength::Fill(1.0)
        );
        let scroll = world.get::<UiLayoutNode>(children[7]).unwrap();
        assert!(scroll.clip_contents);
        assert_eq!(scroll.size.height, UiLength::Px(40.0));
        assert!(world.get::<UiScroll>(children[7]).is_some());
        let custom = world.get::<UiInlineStyle>(children[8]).unwrap();
        assert_eq!(
            custom.0.get(&UiStyleProperty::Gap),
            Some(&UiStyleValue::Scalar(13.0))
        );
        assert_eq!(
            custom.0.get(&UiStyleProperty::Width),
            Some(&UiStyleValue::Length(UiLength::Px(320.0)))
        );
    }

    #[test]
    fn invalid_grid_declaration_is_rejected_before_spawning_any_entities() {
        let mut world = World::new();
        let before = world.iter_entities().count();
        let tree = WidgetTree::new(WidgetNode::column([
            WidgetNode::button("Would spawn"),
            WidgetNode::grid(0, [WidgetNode::button("Invalid")]),
        ]));
        assert!(matches!(
            tree.spawn(&mut world, None),
            Err(WidgetTreeError::InvalidLayout { .. })
        ));
        assert_eq!(world.iter_entities().count(), before);
    }

    #[test]
    fn data_table_declaration_spawns_validated_focusable_model() {
        let mut world = World::new();
        let table = DataTable {
            columns: vec![crate::TableColumn::new("Name", 160.0)],
            rows: vec![crate::TableRow {
                id: 17,
                cells: vec!["Cube".into()],
            }],
            ..DataTable::default()
        };
        let entity = WidgetTree::new(WidgetNode::data_table("Objects", table.clone()))
            .spawn(&mut world, None)
            .unwrap();
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert!(world.get::<UiWheelHandled>(entity).is_some());
        assert_eq!(world.get::<DataTable>(entity).unwrap(), &table);

        let mut invalid = table;
        invalid.rows[0].cells.clear();
        let mut query = world.query::<Entity>();
        let count = query.iter(&world).count();
        assert!(
            WidgetTree::new(WidgetNode::data_table("Invalid", invalid))
                .spawn(&mut world, None)
                .is_err()
        );
        assert_eq!(query.iter(&world).count(), count);
    }

    #[test]
    fn node_graph_declaration_spawns_focusable_editor_and_validates_before_spawn() {
        let mut world = World::new();
        let mut model = NodeGraph::default();
        let source = model.add_node(
            "Source",
            [12.0, 20.0],
            vec![crate::NodeSocket::output("value", crate::SocketType::Float)],
        );
        let sink = model.add_node(
            "Sink",
            [280.0, 40.0],
            vec![crate::NodeSocket::input("value", crate::SocketType::Float)],
        );
        model.create_link(source, 0, sink, 0).unwrap();
        let entity = WidgetTree::new(WidgetNode::node_graph("Shader", model.clone()))
            .spawn(&mut world, None)
            .unwrap();
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert!(world.get::<UiWheelHandled>(entity).is_some());
        assert_eq!(world.get::<Label>(entity).unwrap().text, "Shader");
        assert_eq!(world.get::<NodeGraph>(entity).unwrap(), &model);
        assert!(world.get::<NodeGraphEditorState>(entity).is_some());

        let mut invalid = model;
        invalid.zoom = f32::NAN;
        let mut query = world.query::<Entity>();
        let before = query.iter(&world).count();
        let error = WidgetTree::new(WidgetNode::node_graph("Invalid", invalid))
            .spawn(&mut world, None)
            .unwrap_err();
        assert!(matches!(error, WidgetTreeError::InvalidNodeGraph { .. }));
        assert_eq!(query.iter(&world).count(), before);
    }

    #[test]
    fn canvas_declaration_spawns_focusable_model_and_validates_before_spawn() {
        let mut world = World::new();
        let entity = WidgetTree::new(WidgetNode::canvas_default("Scene"))
            .spawn(&mut world, None)
            .unwrap();
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert!(world.get::<UiWheelHandled>(entity).is_some());
        assert_eq!(world.get::<Label>(entity).unwrap().text, "Scene");
        assert!(world.get::<Canvas>(entity).is_some());

        let mut invalid = Canvas::default();
        invalid.zoom = f32::NAN;
        let mut query = world.query::<Entity>();
        let before = query.iter(&world).count();
        let error = WidgetTree::new(WidgetNode::canvas("Invalid", invalid))
            .spawn(&mut world, None)
            .unwrap_err();
        assert!(matches!(error, WidgetTreeError::InvalidCanvas { .. }));
        assert_eq!(query.iter(&world).count(), before);
    }

    #[test]
    fn color_ramp_declaration_spawns_focusable_editor_and_validates_before_spawn() {
        let mut world = World::new();
        let ramp = WidgetNode::color_ramp_default("Gradient").with_key("gradient");
        let entity = WidgetTree::new(ramp).spawn(&mut world, None).unwrap();
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert_eq!(world.get::<Label>(entity).unwrap().text, "Gradient");
        assert_eq!(world.get::<ColorRamp>(entity).unwrap().stops.len(), 2);
        assert_eq!(
            world
                .get::<ColorRampEditorState>(entity)
                .unwrap()
                .active_channel,
            ColorChannel::Red
        );

        let mut invalid = ColorRamp::default();
        invalid.stops[0].position = f32::NAN;
        let mut query = world.query::<Entity>();
        let before = query.iter(&world).count();
        let error = WidgetTree::new(WidgetNode::color_ramp("Invalid", invalid))
            .spawn(&mut world, None)
            .unwrap_err();
        assert!(matches!(error, WidgetTreeError::InvalidColorRamp { .. }));
        assert_eq!(query.iter(&world).count(), before);
    }

    #[test]
    fn hsv_color_declaration_spawns_editor_and_rejects_invalid_srgb() {
        let mut world = World::new();
        let entity = WidgetTree::new(WidgetNode::hsv_color("Primary", Rgba(0.2, 0.4, 0.8, 0.6)))
            .spawn(&mut world, None)
            .unwrap();
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert_eq!(world.get::<Label>(entity).unwrap().text, "Primary");
        let color = world.get::<HsvColorEditor>(entity).unwrap().color();
        assert!((color.0 - 0.2).abs() < 1.0e-5);
        assert!((color.1 - 0.4).abs() < 1.0e-5);
        assert!((color.2 - 0.8).abs() < 1.0e-5);
        assert!((color.3 - 0.6).abs() < 1.0e-5);
        assert_eq!(
            world
                .get::<HsvColorEditorState>(entity)
                .unwrap()
                .active_field,
            crate::HsvColorField::Hue
        );

        let mut query = world.query::<Entity>();
        let before = query.iter(&world).count();
        let error = WidgetTree::new(WidgetNode::hsv_color("Invalid", Rgba(1.5, 0.0, 0.0, 1.0)))
            .spawn(&mut world, None)
            .unwrap_err();
        assert!(matches!(error, WidgetTreeError::InvalidHsvColor { .. }));
        assert_eq!(query.iter(&world).count(), before);
    }

    #[test]
    fn curve_editor_declaration_is_focusable_and_validated_before_spawn() {
        let mut world = World::new();
        let entity = WidgetTree::new(WidgetNode::curve_editor_default("Ease"))
            .spawn(&mut world, None)
            .unwrap();
        assert!(world.get::<UiFocusable>(entity).is_some());
        assert_eq!(world.get::<Label>(entity).unwrap().text, "Ease");
        assert_eq!(world.get::<CurveEditor>(entity).unwrap().points.len(), 2);

        let mut invalid = CurveEditor::default();
        invalid.points[0].position[0] = f32::NAN;
        let mut query = world.query::<Entity>();
        let before = query.iter(&world).count();
        let error = WidgetTree::new(WidgetNode::curve_editor("Invalid", invalid))
            .spawn(&mut world, None)
            .unwrap_err();
        assert!(matches!(error, WidgetTreeError::InvalidCurveEditor { .. }));
        assert_eq!(query.iter(&world).count(), before);
    }
}

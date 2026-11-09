use super::*;

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
pub(crate) struct IntrinsicLabelText;

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
    /// Blender-style property split with a trailing keyframe decorator toggle.
    pub fn property_row_with_decorator(label: impl Into<String>, control: Self) -> Self {
        Self::container(
            WidgetContainer::PropertyRow,
            [
                Self::label(label).with_role("property-label"),
                control.with_width(UiLength::Fill(1.0)),
                Self::checkbox("", false)
                    .with_role("property-decorator")
                    .with_width(UiLength::Px(16.0))
                    .with_height(UiLength::Px(20.0)),
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
    pub fn with_padding(mut self, padding: UiEdges) -> Self {
        let mut layout = self.layout.unwrap_or_else(|| layout_for(&self.spec));
        layout.padding = padding;
        self.layout = Some(layout);
        self.style
            .get_or_insert_with(UiInlineStyle::default)
            .set(UiStyleProperty::Padding, UiStyleValue::Edges(padding));
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

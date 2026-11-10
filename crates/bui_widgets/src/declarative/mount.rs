use super::*;

pub(super) fn validate_node(node: &WidgetNode) -> Result<(), WidgetTreeError> {
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

pub(super) fn spawn_node(
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
    if let Some(name) = &node.accessible_name {
        entity.insert(UiAccessibleName(name.clone()));
    }
    if let Some(selected) = node.tab_selected {
        entity.insert(UiTab { selected });
    }
    if node.tab_list {
        entity.insert(UiTabList);
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
    // An explicit role describes the caller's chosen presentation. Control defaults must not
    // replace it when the node is mounted.
    if let Some(role) = &node.role {
        world.entity_mut(id).insert(UiRole::new(role.clone()));
    }
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

pub(super) fn spawn_menu_items(world: &mut World, owner: Entity, tree: &MenuTree) {
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

pub(super) fn layout_for(spec: &WidgetSpec) -> UiLayoutNode {
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
            padding: UiEdges {
                left: 8.0,
                top: 28.0,
                right: 8.0,
                bottom: 8.0,
            },
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
            UiLayoutNode::leaf(UiLength::Px(360.0), UiLength::Px(220.0))
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

pub(super) fn insert_control(
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
#[expect(
    clippy::type_complexity,
    reason = "The query combines the mount marker with direct label and buffer edits"
)]
pub(crate) fn sync_intrinsic_label_text(
    mut labels: Query<
        (Entity, &Label, &mut TextBuffer),
        (
            With<IntrinsicLabelText>,
            Or<(
                Changed<IntrinsicLabelText>,
                Changed<Label>,
                Changed<TextBuffer>,
            )>,
        ),
    >,
    mut dirty: Option<ResMut<UiDirty>>,
) {
    for (entity, label, mut buffer) in &mut labels {
        if buffer.as_str() != label.text {
            *buffer = TextBuffer::new(label.text.clone());
            if let Some(dirty) = dirty.as_deref_mut() {
                dirty.mark(UiDirtyDomain::TextMeasure, entity);
                dirty.mark(UiDirtyDomain::TextLayout, entity);
                dirty.mark(UiDirtyDomain::TextSync, entity);
            }
        }
    }
}

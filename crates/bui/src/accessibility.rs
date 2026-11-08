//! AccessKit semantics and actions for BUI's logical control entities.

use accesskit::{
    Action, ActionData, Color as AccessibleColor, HasPopup, Live, Node, Rect as AccessibleRect,
    Role, SortDirection, Toggled,
};
use bevy::a11y::{AccessibilityNode, ActionRequest};
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bui_core::{
    UiChildren, UiEnabled, UiFocus, UiFocusable, UiLivePoliteness, UiLiveRegion, UiNode, UiParent,
    UiVisible,
};
use bui_layout::UiLayoutResult;
use bui_text::UiReadOnly;
use bui_widgets::actions::{WidgetAction, WidgetActionKind, WidgetActionQueue};
use bui_widgets::{
    Button, Canvas, CanvasObjectControl, Checkbox, CheckboxValue, CollectionItemControl,
    CollectionItemKind, ColorRamp, CurveEditor, DataTable, DataTableCellControl,
    DataTableHeaderControl, Dropdown, DropdownOptionControl, FileBrowser, FileBrowserItem,
    HsvColorEditor, HsvColorField, IconButton, IconButtonAssetState, IconButtonAssetStatus, Label,
    ListModel, MenuControl, MenuItemControl, NodeGraph, NodeGraphLinkControl, NodeGraphNodeControl,
    NodeGraphSocketControl, NumericField, Panel, PanelGroup, PreviewGrid, Progress, RadioGroup,
    RadioOptionControl, Slider, SocketDirection, TextArea, TextField, Tooltip, TreeModel,
};

#[derive(Resource, Default)]
pub struct BuiFocusMirror(Option<Entity>);

pub fn sync_accessible_focus(
    focus: Res<UiFocus>,
    mut mirror: ResMut<BuiFocusMirror>,
    input_focus: Option<ResMut<InputFocus>>,
) {
    let Some(mut input_focus) = input_focus else {
        return;
    };
    if focus.active_window.is_none() {
        return;
    }
    let target = focus.focused;
    if target == mirror.0 && input_focus.get() == target {
        return;
    }
    if let Some(target) = target {
        input_focus.set(target, FocusCause::Navigated);
        mirror.0 = Some(target);
    } else {
        if input_focus.get() == mirror.0 {
            input_focus.clear();
        }
        mirror.0 = None;
    }
}

pub fn sync_accessibility(world: &mut World) {
    let stale_link = world
        .get_resource::<AccessibilityLinkSelection>()
        .and_then(|selection| selection.0.as_ref())
        .is_some_and(|(entity, socket)| {
            world.get::<NodeGraphSocketControl>(*entity) != Some(socket)
                || !visible_in_hierarchy(world, *entity)
                || !enabled_in_hierarchy(world, *entity)
        });
    if stale_link {
        world.resource_mut::<AccessibilityLinkSelection>().0 = None;
    }
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<UiNode>>()
        .iter(world)
        .collect();
    for entity in entities {
        let Some(mut node) = node_for(world, entity) else {
            if world.get::<AccessibilityNode>(entity).is_some() {
                world.entity_mut(entity).remove::<AccessibilityNode>();
            }
            continue;
        };
        if world.get::<UiFocusable>(entity).is_some() {
            node.add_action(Action::Focus);
        }
        if let Some(region) = world.get::<UiLiveRegion>(entity) {
            node.set_live(match region.politeness {
                UiLivePoliteness::Polite => Live::Polite,
                UiLivePoliteness::Assertive => Live::Assertive,
            });
            if region.atomic {
                node.set_live_atomic();
            }
        }
        if let Some(tooltip) = world.get::<Tooltip>(entity) {
            node.set_tooltip(tooltip.text.clone());
        }
        if !visible_in_hierarchy(world, entity) {
            node.set_hidden();
        }
        if !enabled_in_hierarchy(world, entity) {
            node.set_disabled();
        }
        if node.is_disabled() || node.is_hidden() {
            node.clear_actions();
        }
        if let Some(layout) = world.get::<UiLayoutResult>(entity) {
            let rect = layout.rect;
            let scale = f64::from(layout.dpi_scale);
            node.set_bounds(AccessibleRect {
                x0: f64::from(rect.x) * scale,
                y0: f64::from(rect.y) * scale,
                x1: f64::from(rect.x + rect.width) * scale,
                y1: f64::from(rect.y + rect.height) * scale,
            });
        }
        if world
            .get::<AccessibilityNode>(entity)
            .is_none_or(|old| old.0 != node)
        {
            world
                .entity_mut(entity)
                .insert(AccessibilityNode::from(node));
        }
    }
}

fn node_for(world: &World, entity: Entity) -> Option<Node> {
    let label = world.get::<Label>(entity).map(|label| label.text.clone());
    if let Some(option) = world.get::<RadioOptionControl>(entity) {
        let mut node = Node::new(Role::RadioButton);
        node.set_label(option.label.clone());
        node.set_toggled(if option.selected {
            Toggled::True
        } else {
            Toggled::False
        });
        node.set_position_in_set(option.index.saturating_add(1));
        if !option.enabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
            node.add_action(Action::Focus);
        }
        return Some(node);
    }
    if let Some(option) = world.get::<DropdownOptionControl>(entity) {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(option.label.clone());
        node.set_selected(option.selected);
        node.set_position_in_set(option.index.saturating_add(1));
        if !option.enabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
            node.add_action(Action::Focus);
        }
        return Some(node);
    }
    if let Some(header) = world.get::<DataTableHeaderControl>(entity) {
        let mut node = Node::new(Role::ColumnHeader);
        node.set_label(header.label.clone());
        node.set_column_index(header.column);
        node.set_numeric_value(f64::from(header.width));
        node.set_numeric_value_step(8.0);
        if let Some(ascending) = header.sort_ascending {
            node.set_sort_direction(if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            });
        }
        if header.sortable {
            node.add_action(Action::Click);
        }
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        node.add_action(Action::SetValue);
        return Some(node);
    }
    if let Some(cell) = world.get::<DataTableCellControl>(entity) {
        let mut node = Node::new(Role::GridCell);
        node.set_label(format!("{}, {}", cell.column_label, cell.value));
        node.set_value(cell.value.clone());
        node.set_row_index(cell.row_index);
        node.set_column_index(cell.column);
        node.set_selected(cell.selected);
        node.add_action(Action::Click);
        return Some(node);
    }
    if let Some(table) = world.get::<DataTable>(entity) {
        let mut node = Node::new(Role::Grid);
        node.set_label(label.unwrap_or_else(|| "Data table".to_owned()));
        node.set_row_count(table.rows.len());
        node.set_column_count(table.columns.len());
        if table.multi_select {
            node.set_multiselectable();
        }
        node.set_value(format!("{} rows selected", table.selection.selected.len()));
        return Some(node);
    }
    if let Some(control) = world.get::<CanvasObjectControl>(entity) {
        let mut node = Node::new(Role::GraphicsObject);
        node.set_label(control.label.clone());
        node.set_selected(control.selected);
        if control.locked {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(canvas) = world.get::<Canvas>(entity) {
        let mut node = Node::new(Role::Canvas);
        node.set_label(label.unwrap_or_else(|| "Canvas".to_owned()));
        node.set_value(format!(
            "{} objects, {} selected",
            canvas.objects.len(),
            canvas.selected.len()
        ));
        node.add_action(Action::Focus);
        return Some(node);
    }
    if let Some(control) = world.get::<NodeGraphNodeControl>(entity) {
        let mut node = Node::new(Role::Group);
        node.set_label(control.label.clone());
        node.set_selected(control.selected);
        node.set_expanded(!control.collapsed);
        node.add_action(Action::Click);
        node.add_action(if control.collapsed {
            Action::Expand
        } else {
            Action::Collapse
        });
        return Some(node);
    }
    if let Some(control) = world.get::<NodeGraphSocketControl>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(format!(
            "{} {:?} socket, {:?}",
            control.label, control.direction, control.socket_type
        ));
        let selected = world
            .get_resource::<AccessibilityLinkSelection>()
            .and_then(|selection| selection.0.as_ref())
            .is_some_and(|(selected, _)| *selected == entity);
        if selected {
            node.set_selected(true);
            node.set_description(
                "Socket selected; activate an opposite-direction socket to create a link",
            );
            node.set_live(Live::Polite);
            node.set_live_atomic();
        } else {
            node.set_description(
                "Activate an output socket, then an input socket to create a link",
            );
        }
        node.add_action(Action::Click);
        node.add_action(Action::Focus);
        return Some(node);
    }
    if let Some(control) = world.get::<NodeGraphLinkControl>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(format!(
            "Link from node {} socket {} to node {} socket {}",
            control.from_node.0,
            control.from_socket + 1,
            control.to_node.0,
            control.to_socket + 1
        ));
        node.set_selected(control.selected);
        node.add_action(Action::Click);
        return Some(node);
    }
    if let Some(graph) = world.get::<NodeGraph>(entity) {
        let mut node = Node::new(Role::GraphicsDocument);
        node.set_label(label.unwrap_or_else(|| "Node editor".to_owned()));
        node.set_value(format!(
            "{} nodes, {} links, {} selected",
            graph.nodes.len(),
            graph.links.len(),
            graph.selected.len()
        ));
        node.add_action(Action::Focus);
        return Some(node);
    }
    if let Some(item) = world.get::<CollectionItemControl>(entity) {
        let mut node = Node::new(match item.kind {
            CollectionItemKind::List => Role::ListBoxOption,
            CollectionItemKind::Tree => Role::TreeItem,
            CollectionItemKind::PreviewGrid => Role::GridCell,
        });
        node.set_label(item.label.clone());
        node.set_position_in_set(item.index.saturating_add(1));
        node.set_selected(item.selected);
        if !item.enabled {
            node.set_disabled();
        } else {
            node.add_action(Action::Click);
        }
        if item.kind == CollectionItemKind::Tree {
            node.set_level(item.depth.saturating_add(1));
            if let Some(tree) = world.get::<TreeModel<String>>(item.owner)
                && let Some(current) = tree.nodes.iter().find(|node| node.id == item.id)
            {
                let siblings: Vec<_> = if let Some(parent) = current.parent {
                    tree.nodes
                        .iter()
                        .find(|node| node.id == parent)
                        .map(|node| node.children.clone())
                        .unwrap_or_default()
                } else {
                    tree.nodes
                        .iter()
                        .filter(|node| node.parent.is_none())
                        .map(|node| node.id)
                        .collect()
                };
                if let Some(position) = siblings.iter().position(|id| *id == item.id) {
                    node.set_position_in_set(position + 1);
                    node.set_size_of_set(siblings.len());
                }
            }
            if item.has_children {
                node.set_expanded(item.expanded);
                if item.enabled {
                    node.add_action(if item.expanded {
                        Action::Collapse
                    } else {
                        Action::Expand
                    });
                }
            }
        }
        return Some(node);
    }
    if let Some(list) = world.get::<ListModel<String>>(entity) {
        let mut node = Node::new(Role::ListBox);
        node.set_label(label.unwrap_or_else(|| "List".to_owned()));
        node.set_size_of_set(list.items.len());
        if list.multi_select {
            node.set_multiselectable();
        }
        node.set_value(format!("{} selected", list.selection.selected.len()));
        return Some(node);
    }
    if let Some(tree) = world.get::<TreeModel<String>>(entity) {
        let mut node = Node::new(Role::Tree);
        node.set_label(label.unwrap_or_else(|| "Tree".to_owned()));
        node.set_size_of_set(tree.nodes.len());
        node.set_value(format!("{} selected", tree.selection.selected.len()));
        return Some(node);
    }
    if let Some(grid) = world.get::<PreviewGrid>(entity) {
        let mut node = Node::new(Role::Grid);
        node.set_label(label.unwrap_or_else(|| "Preview grid".to_owned()));
        node.set_size_of_set(grid.items.len());
        if grid.multi_select {
            node.set_multiselectable();
        }
        node.set_value(format!("{} selected", grid.selection.selected.len()));
        return Some(node);
    }
    if let Some(browser) = world.get::<FileBrowser>(entity) {
        let mut node = Node::new(Role::ListBox);
        node.set_label(format!("Files in {}", browser.directory.display()));
        node.set_value(format!("{} selected", browser.selected.len()));
        node.set_size_of_set(browser.visible_count());
        node.set_multiselectable();
        return Some(node);
    }
    if let Some(item) = world.get::<FileBrowserItem>(entity) {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(if item.directory {
            format!("{}, folder", item.name)
        } else {
            item.name.clone()
        });
        node.set_position_in_set(item.index.saturating_add(1));
        if item.selected {
            node.set_selected(true);
        }
        node.add_action(Action::Click);
        return Some(node);
    }
    if let Some(menu) = world.get::<MenuControl>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(menu.label.clone());
        node.set_has_popup(HasPopup::Menu);
        node.set_expanded(menu.session.open);
        node.add_action(Action::Click);
        node.add_action(Action::ShowContextMenu);
        return Some(node);
    }
    if let Some(item) = world.get::<MenuItemControl>(entity) {
        let mut node = Node::new(if item.separator {
            Role::Splitter
        } else {
            Role::MenuItem
        });
        node.set_label(item.label.clone());
        if let Some(shortcut) = &item.shortcut {
            node.set_description(format!("Shortcut {shortcut}"));
        }
        if let Some(owner) = world.get::<MenuControl>(item.owner) {
            let mut path = item.parent_path.clone();
            path.push(item.index);
            node.set_selected(owner.session.open && owner.session.path == path);
        }
        if item.has_submenu && !item.separator {
            node.set_has_popup(HasPopup::Menu);
            if let Some(owner) = world.get::<MenuControl>(item.owner) {
                let mut path = item.parent_path.clone();
                path.push(item.index);
                node.set_expanded(
                    owner.session.open
                        && owner.session.path.starts_with(&path)
                        && owner.session.path.len() > path.len(),
                );
            }
        }
        if !item.enabled {
            node.set_disabled();
        }
        if item.enabled && !item.separator {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(ramp) = world.get::<ColorRamp>(entity) {
        let mut node = Node::new(Role::Slider);
        node.set_label(label.unwrap_or_else(|| "Color ramp".to_owned()));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        node.set_numeric_value_step(0.01);
        if let Some(stop) = ramp.selected_stop() {
            node.set_numeric_value(f64::from(stop.position));
            node.set_value(format!(
                "Stop at {:.0}%, RGBA {:.2} {:.2} {:.2} {:.2}, {:?} interpolation",
                stop.position * 100.0,
                stop.color.0,
                stop.color.1,
                stop.color.2,
                stop.color.3,
                ramp.interpolation
            ));
        }
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        node.add_action(Action::SetValue);
        return Some(node);
    }
    if let Some(editor) = world.get::<HsvColorEditor>(entity) {
        let mut node = Node::new(Role::ColorWell);
        node.set_label(label.unwrap_or_else(|| "Color".to_owned()));
        let color = editor.color();
        let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        node.set_color_value(AccessibleColor {
            red: byte(color.0),
            green: byte(color.1),
            blue: byte(color.2),
            alpha: byte(color.3),
        });
        node.set_value(editor.hex());
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
        return Some(node);
    }
    if let Some(editor) = world.get::<CurveEditor>(entity) {
        let mut node = Node::new(Role::Slider);
        node.set_label(label.unwrap_or_else(|| "Curve editor point".to_owned()));
        node.set_numeric_value_step(0.01);
        if let Some((index, point)) = editor
            .points
            .iter()
            .enumerate()
            .find(|(_, point)| Some(point.id) == editor.selected)
        {
            node.set_numeric_value(f64::from(point.position[1]));
            node.set_value(format!(
                "Point {} of {}, x {:.3}, y {:.3}, {:?} interpolation",
                index + 1,
                editor.points.len(),
                point.position[0],
                point.position[1],
                editor.interpolation
            ));
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
        } else {
            node.set_value(format!(
                "{} points, {:?} interpolation; select a point to edit",
                editor.points.len(),
                editor.interpolation
            ));
        }
        return Some(node);
    }
    if let Some(checkbox) = world.get::<Checkbox>(entity) {
        let mut node = Node::new(Role::CheckBox);
        node.set_label(checkbox.label.clone());
        node.set_toggled(match checkbox.value {
            CheckboxValue::Off => Toggled::False,
            CheckboxValue::On => Toggled::True,
            CheckboxValue::Mixed => Toggled::Mixed,
        });
        if !checkbox.enabled {
            node.set_disabled();
        }
        if checkbox.read_only {
            node.set_read_only();
        }
        if checkbox.enabled && !checkbox.read_only {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(slider) = world.get::<Slider>(entity) {
        let mut node = Node::new(Role::Slider);
        node.set_label(label.unwrap_or_else(|| "Slider".to_owned()));
        node.set_numeric_value(slider.value);
        node.set_value(slider.display());
        let [min, max] = slider.range();
        node.set_min_numeric_value(min);
        node.set_max_numeric_value(max);
        node.set_numeric_value_step(slider.step);
        if let Some(error) = &slider.expression_error {
            node.set_description(error.to_string());
            node.set_live(Live::Assertive);
            node.set_live_atomic();
        }
        if !slider.enabled {
            node.set_disabled();
        }
        if slider.read_only {
            node.set_read_only();
        }
        if slider.enabled && !slider.read_only {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
        }
        return Some(node);
    }
    if let Some(number) = world.get::<NumericField>(entity) {
        let mut node = Node::new(Role::SpinButton);
        if let Some(label) = label {
            node.set_label(label);
        }
        node.set_numeric_value(number.value);
        node.set_value(number.display());
        if let Some(min) = number.min {
            node.set_min_numeric_value(min);
        }
        if let Some(max) = number.max {
            node.set_max_numeric_value(max);
        }
        node.set_numeric_value_step(number.step);
        if let Some(error) = &number.expression_error {
            node.set_description(error.to_string());
            node.set_live(Live::Assertive);
            node.set_live_atomic();
        }
        if !number.enabled {
            node.set_disabled();
        }
        if number.read_only {
            node.set_read_only();
        }
        if number.enabled && !number.read_only {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
            node.add_action(Action::SetValue);
        }
        return Some(node);
    }
    if let Some(field) = world.get::<TextField>(entity) {
        let mut node = Node::new(Role::TextInput);
        let read_only = field.read_only || world.get::<UiReadOnly>(entity).is_some();
        if let Some(label) = label {
            node.set_label(label);
        }
        node.set_value(field.text.clone());
        if !field.placeholder.is_empty() {
            node.set_placeholder(field.placeholder.clone());
        }
        if !field.enabled {
            node.set_disabled();
        }
        if read_only {
            node.set_read_only();
        }
        if field.enabled && !read_only {
            node.add_action(Action::ReplaceSelectedText);
        }
        return Some(node);
    }
    if let Some(area) = world.get::<TextArea>(entity) {
        let mut node = Node::new(Role::MultilineTextInput);
        let read_only = area.field.read_only || world.get::<UiReadOnly>(entity).is_some();
        if let Some(label) = label {
            node.set_label(label);
        }
        node.set_value(area.field.text.clone());
        if !area.field.enabled {
            node.set_disabled();
        }
        if read_only {
            node.set_read_only();
        }
        if area.field.enabled && !read_only {
            node.add_action(Action::ReplaceSelectedText);
        }
        return Some(node);
    }
    if let Some(dropdown) = world.get::<Dropdown>(entity) {
        let mut node = Node::new(Role::ComboBox);
        if let Some(label) = label {
            node.set_label(label);
        }
        if let Some(value) = dropdown
            .selected
            .and_then(|index| dropdown.options.get(index))
        {
            node.set_value(value.clone());
        }
        if !dropdown.enabled {
            node.set_disabled();
        }
        node.set_has_popup(HasPopup::Listbox);
        node.set_size_of_set(dropdown.options.len());
        node.set_expanded(dropdown.open);
        if dropdown.enabled {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(radio) = world.get::<RadioGroup>(entity) {
        let mut node = Node::new(Role::RadioGroup);
        node.set_size_of_set(radio.options.len());
        if let Some(label) = label {
            node.set_label(label);
        }
        if let Some(value) = radio.selected.and_then(|index| radio.options.get(index)) {
            node.set_value(value.label.clone());
        }
        if !radio.enabled {
            node.set_disabled();
        }
        if radio.enabled {
            node.add_action(Action::Increment);
            node.add_action(Action::Decrement);
        }
        return Some(node);
    }
    if let Some(button) = world.get::<Button>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(button.label.clone());
        if !button.enabled {
            node.set_disabled();
        }
        if button.enabled {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(button) = world.get::<IconButton>(entity) {
        let mut node = Node::new(Role::Button);
        node.set_label(button.accessible_name.clone());
        if let Some(state) = world.get::<IconButtonAssetState>(entity) {
            match &state.status {
                IconButtonAssetStatus::Loading => node.set_description("Icon loading"),
                IconButtonAssetStatus::Failed(reason) => {
                    node.set_description(format!("Icon failed to load: {reason}"));
                    node.set_live(Live::Assertive);
                    node.set_live_atomic();
                }
                IconButtonAssetStatus::Hidden | IconButtonAssetStatus::Ready => {}
            }
        }
        if !button.enabled {
            node.set_disabled();
        }
        if button.enabled {
            node.add_action(Action::Click);
        }
        return Some(node);
    }
    if let Some(panel) = world.get::<Panel>(entity) {
        let mut node = Node::new(Role::Group);
        node.set_label(panel.title.clone());
        node.set_expanded(panel.state.target_expanded);
        if !panel.enabled {
            node.set_disabled();
        }
        if panel.enabled {
            node.add_action(if panel.state.target_expanded {
                Action::Collapse
            } else {
                Action::Expand
            });
            if world
                .get::<UiParent>(entity)
                .and_then(|parent| world.get::<PanelGroup>(parent.0))
                .is_some_and(|group| group.reorderable)
            {
                node.add_action(Action::Increment);
                node.add_action(Action::Decrement);
            }
        }
        return Some(node);
    }
    if let Some(progress) = world.get::<Progress>(entity) {
        let mut node = Node::new(Role::ProgressIndicator);
        node.set_label(label.unwrap_or_else(|| "Progress".to_owned()));
        node.set_numeric_value(f64::from(progress.value));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(1.0);
        return Some(node);
    }
    if let Some(label) = world.get::<Label>(entity) {
        let mut node = Node::new(Role::Label);
        node.set_value(label.text.clone());
        return Some(node);
    }
    if let Some(tooltip) = world.get::<Tooltip>(entity) {
        let mut node = Node::new(Role::Tooltip);
        node.set_label(tooltip.text.clone());
        if !tooltip.visible {
            node.set_hidden();
        }
        return Some(node);
    }
    if world.get::<UiChildren>(entity).is_some() {
        return Some(Node::new(Role::Group));
    }
    None
}

fn visible_in_hierarchy(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut depth = 0;
    while let Some(at) = current {
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
        depth += 1;
        if depth > 1024 {
            return false;
        }
    }
    true
}

fn enabled_in_hierarchy(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut depth = 0;
    while let Some(at) = current {
        if world.get::<UiEnabled>(at).is_some_and(|enabled| !enabled.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
        depth += 1;
        if depth > 1024 {
            return false;
        }
    }
    true
}

type UiNodeState<'a> = (
    Option<&'a UiEnabled>,
    Option<&'a UiVisible>,
    Option<&'a UiParent>,
);

#[derive(Resource, Default)]
pub(crate) struct AccessibilityLinkSelection(Option<(Entity, NodeGraphSocketControl)>);

#[derive(SystemParam)]
pub struct AccessControlQueries<'w, 's> {
    accessible: Query<'w, 's, &'static AccessibilityNode>,
    radios: Query<'w, 's, &'static RadioGroup>,
    radio_options: Query<'w, 's, &'static RadioOptionControl>,
    dropdown_options: Query<'w, 's, &'static DropdownOptionControl>,
    ramps: Query<'w, 's, &'static ColorRamp>,
    colors: Query<'w, 's, &'static HsvColorEditor>,
    curves: Query<'w, 's, &'static CurveEditor>,
    collection_items: Query<'w, 's, &'static CollectionItemControl>,
    table_headers: Query<'w, 's, &'static DataTableHeaderControl>,
    table_cells: Query<'w, 's, &'static DataTableCellControl>,
    canvas_objects: Query<'w, 's, &'static CanvasObjectControl>,
    node_graph_nodes: Query<'w, 's, &'static NodeGraphNodeControl>,
    node_graph_sockets: Query<'w, 's, &'static NodeGraphSocketControl>,
    node_graph_links: Query<'w, 's, &'static NodeGraphLinkControl>,
    panels: Query<'w, 's, &'static Panel>,
    panel_groups: Query<'w, 's, &'static PanelGroup>,
}

pub fn route_accessibility_actions(
    mut requests: MessageReader<ActionRequest>,
    nodes: Query<UiNodeState<'_>, With<UiNode>>,
    layouts: Query<&UiLayoutResult>,
    controls: AccessControlQueries,
    link_selection: Option<ResMut<AccessibilityLinkSelection>>,
    mut focus: ResMut<UiFocus>,
    mut actions: ResMut<WidgetActionQueue>,
) {
    let mut link_selection = link_selection;
    for request in requests.read() {
        let Some(target) = Entity::try_from_bits(request.target_node.0) else {
            continue;
        };
        if let Ok(node) = controls.accessible.get(target)
            && request.action != Action::Blur
            && (node.0.is_disabled()
                || node.0.is_hidden()
                || !node.0.supports_action(request.action))
        {
            continue;
        }
        let mut current = Some(target);
        let mut allowed = true;
        let mut depth = 0;
        while let Some(at) = current {
            let Ok((enabled, visible, parent)) = nodes.get(at) else {
                allowed = false;
                break;
            };
            if enabled.is_some_and(|enabled| !enabled.0)
                || visible.is_some_and(|visible| !visible.0)
            {
                allowed = false;
                break;
            }
            current = parent.map(|parent| parent.0);
            depth += 1;
            if depth > 1024 {
                allowed = false;
                break;
            }
        }
        if !allowed {
            continue;
        }
        let focus_target = controls
            .radio_options
            .get(target)
            .map(|option| option.owner)
            .or_else(|_| {
                controls
                    .dropdown_options
                    .get(target)
                    .map(|option| option.owner)
            })
            .unwrap_or(target);
        match request.action {
            Action::Focus => {
                if let Ok(layout) = layouts.get(focus_target) {
                    focus.state_mut(layout.window).focused = Some(focus_target);
                    focus.focused = Some(focus_target);
                }
            }
            Action::Blur => {
                if focus.focused == Some(focus_target) {
                    focus.focused = None;
                }
                for state in focus.windows.values_mut() {
                    if state.focused == Some(focus_target) {
                        state.focused = None;
                    }
                }
            }
            Action::Click | Action::ShowContextMenu => {
                if let Ok(option) = controls.radio_options.get(target) {
                    if option.enabled {
                        actions.push(WidgetAction::new(
                            option.owner,
                            WidgetActionKind::ChooseOption(option.index),
                        ));
                    }
                } else if let Ok(option) = controls.dropdown_options.get(target) {
                    if option.enabled {
                        actions.push(WidgetAction::new(
                            option.owner,
                            WidgetActionKind::ChooseOption(option.index),
                        ));
                    }
                } else if let Ok(header) = controls.table_headers.get(target) {
                    if header.sortable {
                        actions.push(WidgetAction::new(
                            target,
                            WidgetActionKind::DataTableSortColumn(header.column),
                        ));
                    }
                } else if let Ok(cell) = controls.table_cells.get(target) {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::DataTableSelectCell {
                            row_id: cell.row_id,
                            column: cell.column,
                            ctrl: false,
                            shift: false,
                        },
                    ));
                } else if let Ok(socket) = controls.node_graph_sockets.get(target) {
                    let Some(selection) = link_selection.as_mut() else {
                        continue;
                    };
                    match selection.0.take() {
                        Some((_, first))
                            if first.owner == socket.owner
                                && first.node_id == socket.node_id
                                && first.socket_index == socket.socket_index
                                && first.direction == socket.direction => {}
                        Some((_, first))
                            if first.owner == socket.owner
                                && first.direction != socket.direction =>
                        {
                            let (output, input) = if first.direction == SocketDirection::Output {
                                (&first, socket)
                            } else {
                                (socket, &first)
                            };
                            actions.push(WidgetAction::new(
                                socket.owner,
                                WidgetActionKind::NodeGraphCreateLink {
                                    from_node: output.node_id,
                                    from_socket: output.socket_index,
                                    to_node: input.node_id,
                                    to_socket: input.socket_index,
                                },
                            ));
                        }
                        _ => selection.0 = Some((target, socket.clone())),
                    }
                } else if let Ok(node) = controls.node_graph_nodes.get(target) {
                    actions.push(WidgetAction::new(
                        node.owner,
                        WidgetActionKind::NodeGraphSelectNode {
                            id: node.node_id,
                            additive: false,
                        },
                    ));
                } else if let Ok(link) = controls.node_graph_links.get(target) {
                    actions.push(WidgetAction::new(
                        link.owner,
                        WidgetActionKind::NodeGraphSelectLink(link.link_id),
                    ));
                } else if let Ok(object) = controls.canvas_objects.get(target) {
                    if !object.locked {
                        actions.push(WidgetAction::new(
                            object.owner,
                            WidgetActionKind::CanvasSelectObject {
                                id: object.object_id,
                                additive: false,
                            },
                        ));
                    }
                } else {
                    actions.push(WidgetAction::new(target, WidgetActionKind::Activate));
                }
            }
            Action::Increment | Action::Decrement => {
                let step = if request.action == Action::Increment {
                    1
                } else {
                    -1
                };
                if let Ok(header) = controls.table_headers.get(target) {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::DataTableResizeColumn {
                            column: header.column,
                            width: header.width + step as f32 * 8.0,
                        },
                    ));
                } else if let Ok(editor) = controls.colors.get(target) {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::HsvColorSetHsv {
                            field: HsvColorField::Hue,
                            value: (editor.hue_degrees + step as f32).rem_euclid(360.0),
                        },
                    ));
                } else if let Ok(editor) = controls.curves.get(target) {
                    if let Some(point) = editor
                        .points
                        .iter()
                        .find(|point| Some(point.id) == editor.selected)
                    {
                        actions.push(WidgetAction::new(
                            target,
                            WidgetActionKind::CurveEditorMovePoint {
                                id: point.id,
                                position: [
                                    point.position[0],
                                    point.position[1] + step as f32 * 0.01,
                                ],
                            },
                        ));
                    }
                } else if let Ok(ramp) = controls.ramps.get(target) {
                    if let Some(stop) = ramp.selected_stop() {
                        actions.push(WidgetAction::new(
                            target,
                            WidgetActionKind::ColorRampMoveStop {
                                id: stop.id,
                                position: (stop.position + step as f32 * 0.01).clamp(0.0, 1.0),
                            },
                        ));
                    }
                } else if let Ok(radio) = controls.radios.get(target) {
                    let mut candidate = radio.clone();
                    if let Some(index) = candidate.move_selection(step) {
                        actions.push(WidgetAction::new(
                            target,
                            WidgetActionKind::ChooseOption(index),
                        ));
                    }
                } else if controls.panels.get(target).is_ok()
                    && nodes
                        .get(target)
                        .ok()
                        .and_then(|(_, _, parent)| parent)
                        .and_then(|parent| controls.panel_groups.get(parent.0).ok())
                        .is_some_and(|group| group.reorderable)
                {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::PanelReorderStep(step),
                    ));
                } else {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::StepNumber(step),
                    ));
                }
            }
            Action::Expand | Action::Collapse => {
                if let Ok(node) = controls.node_graph_nodes.get(target)
                    && node.collapsed == (request.action == Action::Expand)
                {
                    actions.push(WidgetAction::new(
                        node.owner,
                        WidgetActionKind::NodeGraphSelectNode {
                            id: node.node_id,
                            additive: false,
                        },
                    ));
                    actions.push(WidgetAction::new(
                        node.owner,
                        WidgetActionKind::NodeGraphToggleCollapse,
                    ));
                } else if let Ok(item) = controls.collection_items.get(target)
                    && item.kind == CollectionItemKind::Tree
                    && item.has_children
                    && item.expanded != (request.action == Action::Expand)
                {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::TreeToggleExpanded(item.id),
                    ));
                } else if let Ok(panel) = controls.panels.get(target)
                    && panel.state.target_expanded != (request.action == Action::Expand)
                {
                    actions.push(WidgetAction::new(target, WidgetActionKind::TogglePanel));
                }
            }
            Action::SetValue => {
                if let Some(ActionData::Value(value)) = &request.data
                    && controls.colors.get(target).is_ok()
                {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::HsvColorSetHex(value.to_string()),
                    ));
                } else if let Some(ActionData::NumericValue(value)) = &request.data {
                    if let Ok(header) = controls.table_headers.get(target) {
                        actions.push(WidgetAction::new(
                            target,
                            WidgetActionKind::DataTableResizeColumn {
                                column: header.column,
                                width: *value as f32,
                            },
                        ));
                    } else if let Ok(editor) = controls.curves.get(target) {
                        if let Some(point) = editor
                            .points
                            .iter()
                            .find(|point| Some(point.id) == editor.selected)
                        {
                            actions.push(WidgetAction::new(
                                target,
                                WidgetActionKind::CurveEditorMovePoint {
                                    id: point.id,
                                    position: [point.position[0], *value as f32],
                                },
                            ));
                        }
                    } else if let Ok(ramp) = controls.ramps.get(target) {
                        if let Some(stop) = ramp.selected_stop() {
                            actions.push(WidgetAction::new(
                                target,
                                WidgetActionKind::ColorRampMoveStop {
                                    id: stop.id,
                                    position: (*value as f32).clamp(0.0, 1.0),
                                },
                            ));
                        }
                    } else {
                        actions.push(WidgetAction::new(
                            target,
                            WidgetActionKind::SetNumber(*value),
                        ));
                    }
                }
            }
            Action::ReplaceSelectedText => {
                if let Some(ActionData::Value(value)) = &request.data {
                    actions.push(WidgetAction::new(
                        target,
                        WidgetActionKind::InsertText(value.to_string()),
                    ));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{ActionRequest as AccessKitActionRequest, NodeId, TreeId};

    #[test]
    fn keyboard_focus_is_mirrored_to_bevys_accessible_focus() {
        let mut app = App::new();
        app.init_resource::<UiFocus>()
            .init_resource::<BuiFocusMirror>()
            .init_resource::<InputFocus>()
            .add_systems(Update, sync_accessible_focus);
        let window = app.world_mut().spawn_empty().id();
        let control = app.world_mut().spawn((UiNode, UiFocusable)).id();
        {
            let mut focus = app.world_mut().resource_mut::<UiFocus>();
            focus.active_window = Some(window);
            focus.focused = Some(control);
            focus.state_mut(window).focused = Some(control);
        }
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(control));

        {
            let mut focus = app.world_mut().resource_mut::<UiFocus>();
            focus.focused = None;
            focus.state_mut(window).focused = None;
        }
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    #[test]
    fn checkbox_semantics_follow_value_and_inherited_visibility() {
        let mut world = World::new();
        let parent = world.spawn((UiNode, UiVisible(false))).id();
        let child = world
            .spawn((
                UiNode,
                UiFocusable,
                UiParent(parent),
                Checkbox::new("Visible", true),
            ))
            .id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(child).unwrap().0;
        assert_eq!(node.role(), Role::CheckBox);
        assert_eq!(node.label(), Some("Visible"));
        assert_eq!(node.toggled(), Some(Toggled::True));
        assert!(node.is_hidden());
        assert!(!node.supports_action(Action::Click));
        assert!(!node.supports_action(Action::Focus));
        world.get_mut::<UiVisible>(parent).unwrap().0 = true;
        world.get_mut::<Checkbox>(child).unwrap().value = CheckboxValue::Off;
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(child).unwrap().0;
        assert_eq!(node.toggled(), Some(Toggled::False));
        assert!(!node.is_hidden());
        assert!(node.supports_action(Action::Click));
        world.entity_mut(parent).insert(UiEnabled(false));
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(child).unwrap().0;
        assert!(node.is_disabled());
        assert!(!node.supports_action(Action::Click));
        assert!(!node.supports_action(Action::Focus));
    }

    #[test]
    fn numeric_expression_error_is_exposed_to_screen_readers() {
        let mut world = World::new();
        let mut field = NumericField::float(3.0).unwrap();
        assert!(field.evaluate_input("1/0", &Default::default()).is_err());
        let entity = world.spawn((UiNode, field)).id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert_eq!(node.role(), Role::SpinButton);
        assert_eq!(node.description(), Some("division by zero"));
        assert_eq!(node.live(), Some(Live::Assertive));
        assert_eq!(node.numeric_value(), Some(3.0));

        world.get_mut::<NumericField>(entity).unwrap().cancel_edit();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert_eq!(node.description(), None);
        assert_eq!(node.live(), None);
    }

    #[test]
    fn live_status_label_announces_changed_value_atomically() {
        let mut world = World::new();
        let status = world
            .spawn((
                UiNode,
                Label::new("Waiting"),
                UiLiveRegion::polite().atomic(),
            ))
            .id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(status).unwrap().0;
        assert_eq!(node.live(), Some(Live::Polite));
        assert!(node.is_live_atomic());
        assert_eq!(node.value(), Some("Waiting"));

        world.get_mut::<Label>(status).unwrap().text = "Saved".to_owned();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(status).unwrap().0;
        assert_eq!(node.value(), Some("Saved"));
        assert_eq!(node.live(), Some(Live::Polite));
        assert!(node.is_live_atomic());
    }

    #[test]
    fn menu_popup_and_tooltip_semantics_track_runtime_state() {
        use bui_widgets::{MenuDisplay, MenuEntry, MenuTree};

        let mut world = World::new();
        let tree = MenuTree::new(vec![MenuEntry::submenu(
            "add",
            "Add",
            vec![MenuEntry::command("cube", "Cube")],
        )]);
        let owner = world
            .spawn((
                UiNode,
                MenuControl::new("Tools", tree, MenuDisplay::Dropdown),
                Tooltip::new("Open tools"),
            ))
            .id();
        let item = world
            .spawn((
                UiNode,
                MenuItemControl {
                    owner,
                    id: "add".to_owned(),
                    label: "Add".to_owned(),
                    parent_path: Vec::new(),
                    index: 0,
                    enabled: true,
                    separator: false,
                    has_submenu: true,
                    shortcut: None,
                },
            ))
            .id();
        let separator = world
            .spawn((
                UiNode,
                MenuItemControl {
                    owner,
                    id: String::new(),
                    label: String::new(),
                    parent_path: Vec::new(),
                    index: 1,
                    enabled: false,
                    separator: true,
                    has_submenu: false,
                    shortcut: None,
                },
            ))
            .id();
        let tooltip = world.spawn((UiNode, Tooltip::new("More options"))).id();
        sync_accessibility(&mut world);
        let menu_node = &world.get::<AccessibilityNode>(owner).unwrap().0;
        assert_eq!(menu_node.has_popup(), Some(HasPopup::Menu));
        assert_eq!(menu_node.is_expanded(), Some(false));
        assert_eq!(menu_node.tooltip(), Some("Open tools"));
        let item_node = &world.get::<AccessibilityNode>(item).unwrap().0;
        assert_eq!(item_node.has_popup(), Some(HasPopup::Menu));
        assert_eq!(item_node.is_expanded(), Some(false));
        assert!(item_node.supports_action(Action::Click));
        let separator_node = &world.get::<AccessibilityNode>(separator).unwrap().0;
        assert_eq!(separator_node.role(), Role::Splitter);
        assert!(!separator_node.supports_action(Action::Click));
        world.get_mut::<MenuItemControl>(item).unwrap().enabled = false;
        sync_accessibility(&mut world);
        let item_node = &world.get::<AccessibilityNode>(item).unwrap().0;
        assert!(item_node.is_disabled());
        assert!(!item_node.supports_action(Action::Click));
        let tooltip_node = &world.get::<AccessibilityNode>(tooltip).unwrap().0;
        assert_eq!(tooltip_node.role(), Role::Tooltip);
        assert!(tooltip_node.is_hidden());

        let mut menu = world.get_mut::<MenuControl>(owner).unwrap();
        menu.open(None);
        menu.session.path = vec![0, 0];
        world.get_mut::<Tooltip>(tooltip).unwrap().visible = true;
        sync_accessibility(&mut world);
        assert_eq!(
            world
                .get::<AccessibilityNode>(owner)
                .unwrap()
                .0
                .is_expanded(),
            Some(true)
        );
        assert_eq!(
            world
                .get::<AccessibilityNode>(item)
                .unwrap()
                .0
                .is_expanded(),
            Some(true)
        );
        assert!(
            !world
                .get::<AccessibilityNode>(tooltip)
                .unwrap()
                .0
                .is_hidden()
        );
    }

    #[test]
    fn dropdown_panel_and_progress_expose_named_live_values() {
        let mut world = World::new();
        let dropdown = world
            .spawn((
                UiNode,
                Label::new("Mode"),
                Dropdown::new(vec!["Fast".to_owned(), "Careful".to_owned()], Some(0)),
            ))
            .id();
        let panel = world.spawn((UiNode, Panel::new("Settings", true))).id();
        let progress = world.spawn((UiNode, Progress::new(0.4))).id();
        sync_accessibility(&mut world);

        let dropdown_node = &world.get::<AccessibilityNode>(dropdown).unwrap().0;
        assert_eq!(dropdown_node.label(), Some("Mode"));
        assert_eq!(dropdown_node.value(), Some("Fast"));
        assert_eq!(dropdown_node.has_popup(), Some(HasPopup::Listbox));
        assert_eq!(dropdown_node.is_expanded(), Some(false));
        let panel_node = &world.get::<AccessibilityNode>(panel).unwrap().0;
        assert_eq!(panel_node.label(), Some("Settings"));
        assert_eq!(panel_node.is_expanded(), Some(true));
        let progress_node = &world.get::<AccessibilityNode>(progress).unwrap().0;
        assert_eq!(progress_node.label(), Some("Progress"));
        assert_eq!(progress_node.numeric_value(), Some(f64::from(0.4_f32)));

        world.get_mut::<Dropdown>(dropdown).unwrap().open();
        world.get_mut::<Panel>(panel).unwrap().state.target_expanded = false;
        sync_accessibility(&mut world);
        assert_eq!(
            world
                .get::<AccessibilityNode>(dropdown)
                .unwrap()
                .0
                .is_expanded(),
            Some(true)
        );
        assert_eq!(
            world
                .get::<AccessibilityNode>(panel)
                .unwrap()
                .0
                .is_expanded(),
            Some(false)
        );
    }

    #[test]
    fn removing_a_control_removes_its_stale_accessibility_node() {
        let mut world = World::new();
        let entity = world.spawn((UiNode, Button::new("Transient"))).id();
        sync_accessibility(&mut world);
        assert!(world.get::<AccessibilityNode>(entity).is_some());
        world.entity_mut(entity).remove::<Button>();
        sync_accessibility(&mut world);
        assert!(world.get::<AccessibilityNode>(entity).is_none());
    }

    #[test]
    fn screen_reader_click_queues_the_same_widget_action_as_pointer_input() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .add_systems(Update, route_accessibility_actions);
        let target = app.world_mut().spawn((UiNode, Button::new("Apply"))).id();
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 1);
    }

    #[test]
    fn screen_reader_menu_item_click_emits_the_selected_command() {
        use bui_widgets::{MenuDisplay, MenuEntry, MenuTree};

        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let mut menu = MenuControl::new(
            "Add",
            MenuTree::new(vec![MenuEntry::command("cube", "Cube")]),
            MenuDisplay::Dropdown,
        );
        menu.open(None);
        let owner = app.world_mut().spawn((UiNode, menu)).id();
        let item = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                MenuItemControl {
                    owner,
                    id: "cube".to_owned(),
                    label: "Cube".to_owned(),
                    parent_path: Vec::new(),
                    index: 0,
                    enabled: true,
                    separator: false,
                    has_submenu: false,
                    shortcut: None,
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<MenuControl>(owner)
            .unwrap()
            .session
            .path = vec![0];
        sync_accessibility(app.world_mut());
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(item)
                .unwrap()
                .0
                .is_selected(),
            Some(true)
        );
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(item.to_bits()),
                data: None,
            }));
        app.update();
        assert!(!app.world().get::<MenuControl>(owner).unwrap().session.open);
        assert!(app
            .world()
            .resource::<bui_widgets::WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, bui_widgets::WidgetEvent::MenuCommand { target, command_id }
                if *target == owner && command_id == "cube")));
    }

    #[test]
    fn radio_group_screen_reader_step_queues_a_selection_change() {
        let mut app = App::new();
        app.add_plugins(bui_widgets::WidgetPlugin)
            .add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                route_accessibility_actions.before(bui_widgets::actions::apply_widget_actions),
            );
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                Label::new("Mode"),
                RadioGroup::new(["Fast", "Careful"], Some(0)),
            ))
            .id();
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Increment,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(
            app.world().get::<RadioGroup>(target).unwrap().selected,
            Some(1)
        );
    }

    #[test]
    fn radio_and_dropdown_options_expose_and_route_individual_actions() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .add_systems(Update, route_accessibility_actions);
        let radio = app
            .world_mut()
            .spawn((UiNode, RadioGroup::new(["Fast", "Careful"], Some(0))))
            .id();
        let radio_option = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(radio),
                RadioOptionControl {
                    owner: radio,
                    index: 1,
                    label: "Careful".to_owned(),
                    enabled: true,
                    selected: false,
                },
            ))
            .id();
        let dropdown = app
            .world_mut()
            .spawn((UiNode, Dropdown::new(["One", "Two"], Some(0))))
            .id();
        let window = app.world_mut().spawn_empty().id();
        for owner in [radio, dropdown] {
            app.world_mut().entity_mut(owner).insert(UiLayoutResult {
                window,
                rect: bui_layout::Rect::new(0.0, 0.0, 120.0, 40.0),
                clip: None,
                z: 0,
                content_size: [120.0, 40.0],
                dpi_scale: 1.0,
            });
        }
        let dropdown_option = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(dropdown),
                DropdownOptionControl {
                    owner: dropdown,
                    index: 1,
                    label: "Two".to_owned(),
                    selected: false,
                    highlighted: true,
                    enabled: true,
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        let radio_node = &app
            .world()
            .get::<AccessibilityNode>(radio_option)
            .unwrap()
            .0;
        assert_eq!(radio_node.role(), Role::RadioButton);
        assert_eq!(radio_node.label(), Some("Careful"));
        assert_eq!(radio_node.toggled(), Some(Toggled::False));
        assert_eq!(radio_node.position_in_set(), Some(2));
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(radio)
                .unwrap()
                .0
                .size_of_set(),
            Some(2)
        );
        let dropdown_node = &app
            .world()
            .get::<AccessibilityNode>(dropdown_option)
            .unwrap()
            .0;
        assert_eq!(dropdown_node.role(), Role::ListBoxOption);
        assert_eq!(dropdown_node.label(), Some("Two"));
        assert_eq!(dropdown_node.is_selected(), Some(false));
        assert_eq!(dropdown_node.position_in_set(), Some(2));
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(dropdown)
                .unwrap()
                .0
                .size_of_set(),
            Some(2)
        );
        for entity in [radio_option, dropdown_option] {
            app.world_mut()
                .write_message(ActionRequest(AccessKitActionRequest {
                    action: Action::Click,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(entity.to_bits()),
                    data: None,
                }));
        }
        app.update();
        assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 2);

        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Focus,
                target_tree: TreeId::ROOT,
                target_node: NodeId(dropdown_option.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(app.world().resource::<UiFocus>().focused, Some(dropdown));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Blur,
                target_tree: TreeId::ROOT,
                target_node: NodeId(dropdown_option.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(app.world().resource::<UiFocus>().focused, None);

        app.world_mut()
            .get_mut::<RadioOptionControl>(radio_option)
            .unwrap()
            .enabled = false;
        sync_accessibility(app.world_mut());
        assert!(
            app.world()
                .get::<AccessibilityNode>(radio_option)
                .unwrap()
                .0
                .is_disabled()
        );
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(radio_option.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 2);
    }

    #[test]
    fn reorderable_panel_exposes_screen_reader_move_actions() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .add_systems(Update, route_accessibility_actions);
        let group = app
            .world_mut()
            .spawn((
                UiNode,
                PanelGroup::with_panels(["one", "two"]).with_reordering("settings"),
            ))
            .id();
        let mut panel = Panel::new("One", true);
        panel.persistence_key = Some("one".into());
        let panel = app.world_mut().spawn((UiNode, panel)).id();
        bui_core::set_parent(app.world_mut(), panel, group).unwrap();
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(panel).unwrap().0;
        assert!(node.supports_action(Action::Increment));
        assert!(node.supports_action(Action::Decrement));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Increment,
                target_tree: TreeId::ROOT,
                target_node: NodeId(panel.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 1);
    }

    #[test]
    fn icon_button_exposes_an_accessible_name_and_disabled_state() {
        let mut world = World::new();
        let mut icon = IconButton::new("plus", "Add object");
        icon.enabled = false;
        let entity = world.spawn((UiNode, icon)).id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert_eq!(node.role(), Role::Button);
        assert_eq!(node.label(), Some("Add object"));
        assert!(node.is_disabled());
        assert!(!node.supports_action(Action::Click));

        world.entity_mut(entity).insert(IconButtonAssetState {
            status: IconButtonAssetStatus::Loading,
        });
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert_eq!(node.description(), Some("Icon loading"));

        world.entity_mut(entity).insert(IconButtonAssetState {
            status: IconButtonAssetStatus::Failed("missing.svg".into()),
        });
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert_eq!(node.description(), Some("Icon failed to load: missing.svg"));
        assert_eq!(node.live(), Some(Live::Assertive));
    }

    #[test]
    fn slider_exposes_bounded_value_and_read_only_state() {
        let mut world = World::new();
        let mut slider = Slider::new(4.0, 0.0, 10.0, 0.5).unwrap().with_unit("dB");
        slider.read_only = true;
        let entity = world.spawn((UiNode, Label::new("Gain"), slider)).id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
        assert_eq!(node.role(), Role::Slider);
        assert_eq!(node.label(), Some("Gain"));
        assert_eq!(node.numeric_value(), Some(4.0));
        assert_eq!(node.value(), Some("4 dB"));
        assert_eq!(node.min_numeric_value(), Some(0.0));
        assert_eq!(node.max_numeric_value(), Some(10.0));
        assert_eq!(node.numeric_value_step(), Some(0.5));
        assert!(node.is_read_only());
        assert!(!node.supports_action(Action::Increment));
        assert!(!node.supports_action(Action::Decrement));
        assert!(!node.supports_action(Action::SetValue));
    }

    #[test]
    fn disabled_and_read_only_controls_do_not_advertise_edit_actions() {
        let mut world = World::new();
        let mut checkbox = Checkbox::new("Locked", false);
        checkbox.read_only = true;
        let checkbox = world.spawn((UiNode, checkbox)).id();
        let mut number = NumericField::float(2.0).unwrap();
        number.enabled = false;
        let number = world.spawn((UiNode, number)).id();
        let mut field = TextField::new("draft");
        field.read_only = true;
        let field = world.spawn((UiNode, field)).id();
        let component_only_read_only = world
            .spawn((UiNode, TextField::new("external"), UiReadOnly))
            .id();
        let mut area = TextArea::new("notes");
        area.field.enabled = false;
        let area = world.spawn((UiNode, area)).id();
        let mut dropdown = Dropdown::new(["One", "Two"], Some(0));
        dropdown.enabled = false;
        let dropdown = world.spawn((UiNode, dropdown)).id();
        let mut radio = RadioGroup::new(["One", "Two"], Some(0));
        radio.enabled = false;
        let radio = world.spawn((UiNode, radio)).id();
        let mut button = Button::new("Run");
        button.enabled = false;
        let button = world.spawn((UiNode, button)).id();
        sync_accessibility(&mut world);

        for (entity, forbidden) in [
            (checkbox, Action::Click),
            (number, Action::SetValue),
            (number, Action::Increment),
            (field, Action::ReplaceSelectedText),
            (component_only_read_only, Action::ReplaceSelectedText),
            (area, Action::ReplaceSelectedText),
            (dropdown, Action::Click),
            (radio, Action::Increment),
            (button, Action::Click),
        ] {
            let node = &world.get::<AccessibilityNode>(entity).unwrap().0;
            assert!(node.is_disabled() || node.is_read_only());
            assert!(
                !node.supports_action(forbidden),
                "{entity:?} advertises {forbidden:?}"
            );
        }
    }

    #[test]
    fn screen_reader_requests_cannot_activate_a_disabled_control() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .add_systems(Update, route_accessibility_actions);
        let mut button = Button::new("Unavailable");
        button.enabled = false;
        let target = app.world_mut().spawn((UiNode, UiFocusable, button)).id();
        sync_accessibility(app.world_mut());
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
        assert!(app.world().resource::<WidgetActionQueue>().is_empty());
        assert_ne!(app.world().resource::<UiFocus>().focused, Some(target));

        let mut field = TextField::new("locked");
        field.read_only = true;
        let text = app.world_mut().spawn((UiNode, UiFocusable, field)).id();
        sync_accessibility(app.world_mut());
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::ReplaceSelectedText,
                target_tree: TreeId::ROOT,
                target_node: NodeId(text.to_bits()),
                data: Some(ActionData::Value("changed".into())),
            }));
        app.update();
        assert!(app.world().resource::<WidgetActionQueue>().is_empty());
    }

    #[test]
    fn slider_screen_reader_step_and_set_value_use_widget_actions() {
        let mut app = App::new();
        app.add_plugins(bui_widgets::WidgetPlugin)
            .add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                route_accessibility_actions.before(bui_widgets::actions::apply_widget_actions),
            );
        let target = app
            .world_mut()
            .spawn((UiNode, Slider::new(4.0, 0.0, 10.0, 0.5).unwrap()))
            .id();
        for (action, data, expected) in [
            (Action::Increment, None, 4.5),
            (Action::SetValue, Some(ActionData::NumericValue(7.0)), 7.0),
        ] {
            app.world_mut()
                .write_message(ActionRequest(AccessKitActionRequest {
                    action,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(target.to_bits()),
                    data,
                }));
            app.update();
            assert_eq!(app.world().get::<Slider>(target).unwrap().value, expected);
        }
    }

    #[test]
    fn screen_reader_text_replacement_and_number_set_use_widget_actions() {
        let mut app = App::new();
        app.add_plugins(bui_widgets::WidgetPlugin)
            .add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                route_accessibility_actions.before(bui_widgets::actions::apply_widget_actions),
            );
        let mut field = TextField::new("old");
        field.select_all();
        let text = app.world_mut().spawn((UiNode, field)).id();
        let mut area = TextArea::new("note");
        area.field.select_all();
        let area = app.world_mut().spawn((UiNode, area)).id();
        let number = app
            .world_mut()
            .spawn((UiNode, NumericField::float(2.0).unwrap()))
            .id();
        for (target, action, data) in [
            (
                text,
                Action::ReplaceSelectedText,
                ActionData::Value("new".into()),
            ),
            (
                area,
                Action::ReplaceSelectedText,
                ActionData::Value("memo".into()),
            ),
            (number, Action::SetValue, ActionData::NumericValue(5.0)),
        ] {
            app.world_mut()
                .write_message(ActionRequest(AccessKitActionRequest {
                    action,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(target.to_bits()),
                    data: Some(data),
                }));
            app.update();
        }
        assert_eq!(app.world().get::<TextField>(text).unwrap().text, "new");
        assert_eq!(
            app.world().get::<TextArea>(area).unwrap().field.text,
            "memo"
        );
        assert_eq!(app.world().get::<NumericField>(number).unwrap().value, 5.0);
    }

    #[test]
    fn file_browser_row_exposes_selection_and_click_action() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let row = world
            .spawn((
                UiNode,
                FileBrowserItem {
                    owner,
                    path: std::path::PathBuf::from("notes.txt"),
                    name: "notes.txt".to_owned(),
                    index: 2,
                    directory: false,
                    size: 12,
                    selected: true,
                    preview: bui_widgets::PreviewKind::Text,
                },
            ))
            .id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(row).unwrap().0;
        assert_eq!(node.role(), Role::ListBoxOption);
        assert_eq!(node.label(), Some("notes.txt"));
        assert_eq!(node.is_selected(), Some(true));
        assert!(node.supports_action(Action::Click));
    }

    #[test]
    fn screen_reader_file_row_click_selects_the_browser_entry() {
        let root = std::env::temp_dir().join(format!(
            "bui-accessible-file-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("notes.txt");
        std::fs::write(&path, b"notes").unwrap();

        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let owner = app
            .world_mut()
            .spawn((UiNode, FileBrowser::new(&root).unwrap()))
            .id();
        let row = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                FileBrowserItem {
                    owner,
                    path: path.clone(),
                    name: "notes.txt".to_owned(),
                    index: 0,
                    directory: false,
                    size: 5,
                    selected: false,
                    preview: bui_widgets::PreviewKind::Text,
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(row.to_bits()),
                data: None,
            }));
        app.update();
        assert!(
            app.world()
                .get::<FileBrowser>(owner)
                .unwrap()
                .selected
                .contains(&path)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canvas_and_node_graph_expose_editor_summaries() {
        let mut world = World::new();
        let mut canvas = Canvas::default();
        let id = canvas
            .add(
                [0.0, 0.0],
                bui_widgets::CanvasShape::Rectangle { size: [30.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        canvas.selected.insert(id);
        let canvas_entity = world
            .spawn((UiNode, Label::new("Layout canvas"), canvas))
            .id();
        let mut graph = NodeGraph::default();
        graph.add_node("Source", [0.0, 0.0], vec![]);
        let graph_entity = world.spawn((UiNode, graph)).id();
        sync_accessibility(&mut world);
        let canvas_node = &world.get::<AccessibilityNode>(canvas_entity).unwrap().0;
        assert_eq!(canvas_node.role(), Role::Canvas);
        assert_eq!(canvas_node.label(), Some("Layout canvas"));
        assert_eq!(canvas_node.value(), Some("1 objects, 1 selected"));
        let graph_node = &world.get::<AccessibilityNode>(graph_entity).unwrap().0;
        assert_eq!(graph_node.role(), Role::GraphicsDocument);
        assert_eq!(graph_node.value(), Some("1 nodes, 0 links, 0 selected"));
    }

    #[test]
    fn canvas_object_screen_reader_click_selects_visible_unlocked_shape() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let mut canvas = Canvas::default();
        let id = canvas
            .add(
                [0.0, 0.0],
                bui_widgets::CanvasShape::Rectangle { size: [20.0, 20.0] },
                [1.0; 4],
            )
            .unwrap();
        let owner = app.world_mut().spawn((UiNode, canvas)).id();
        let control = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                CanvasObjectControl {
                    owner,
                    object_id: id,
                    label: "Rectangle 1".to_owned(),
                    selected: false,
                    locked: false,
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(control).unwrap().0;
        assert_eq!(node.role(), Role::GraphicsObject);
        assert_eq!(node.label(), Some("Rectangle 1"));
        assert!(node.supports_action(Action::Click));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(control.to_bits()),
                data: None,
            }));
        app.update();
        assert!(
            app.world()
                .get::<Canvas>(owner)
                .unwrap()
                .selected
                .contains(&id)
        );
        {
            let mut canvas = app.world_mut().get_mut::<Canvas>(owner).unwrap();
            canvas.selected.clear();
            canvas.objects[0].locked = true;
        }
        app.world_mut()
            .get_mut::<CanvasObjectControl>(control)
            .unwrap()
            .locked = true;
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(control).unwrap().0;
        assert!(node.is_disabled());
        assert!(!node.supports_action(Action::Click));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(control.to_bits()),
                data: None,
            }));
        app.update();
        assert!(
            app.world()
                .get::<Canvas>(owner)
                .unwrap()
                .selected
                .is_empty()
        );
    }

    #[test]
    fn color_ramp_exposes_selected_stop_and_routes_screen_reader_step() {
        let mut app = App::new();
        app.add_plugins(bui_widgets::WidgetPlugin)
            .add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                route_accessibility_actions.before(bui_widgets::actions::apply_widget_actions),
            );
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                Label::new("Gradient"),
                ColorRamp::default(),
            ))
            .id();
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(target).unwrap().0;
        assert_eq!(node.role(), Role::Slider);
        assert_eq!(node.label(), Some("Gradient"));
        assert_eq!(node.numeric_value(), Some(0.0));
        assert!(node.supports_action(Action::Increment));

        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Increment,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
        assert!(
            (app.world()
                .get::<ColorRamp>(target)
                .unwrap()
                .selected_stop()
                .unwrap()
                .position
                - 0.01)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn hsv_editor_exposes_color_well_and_accepts_hex_action() {
        let mut app = App::new();
        app.add_plugins(bui_widgets::WidgetPlugin)
            .add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                route_accessibility_actions.before(bui_widgets::actions::apply_widget_actions),
            );
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                Label::new("Tint"),
                HsvColorEditor::default(),
            ))
            .id();
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(target).unwrap().0;
        assert_eq!(node.role(), Role::ColorWell);
        assert_eq!(node.label(), Some("Tint"));
        assert_eq!(
            node.color_value(),
            Some(AccessibleColor {
                red: 255,
                green: 0,
                blue: 0,
                alpha: 255,
            })
        );
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::SetValue,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: Some(ActionData::Value("#00FF80".into())),
            }));
        app.update();
        assert_eq!(
            app.world().get::<HsvColorEditor>(target).unwrap().hex(),
            "#00FF80FF"
        );
    }

    #[test]
    fn curve_editor_exposes_selected_point_and_routes_screen_reader_step() {
        let mut app = App::new();
        app.add_plugins(bui_widgets::WidgetPlugin)
            .add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .add_systems(
                Update,
                route_accessibility_actions.before(bui_widgets::actions::apply_widget_actions),
            );
        let mut editor = CurveEditor::default();
        assert!(editor.select_point(2));
        let target = app
            .world_mut()
            .spawn((UiNode, UiFocusable, Label::new("Ease"), editor))
            .id();
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(target).unwrap().0;
        assert_eq!(node.role(), Role::Slider);
        assert_eq!(node.label(), Some("Ease"));
        assert_eq!(node.numeric_value(), Some(1.0));
        assert!(node.supports_action(Action::Decrement));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Decrement,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
        let point = app
            .world()
            .get::<CurveEditor>(target)
            .unwrap()
            .points
            .iter()
            .find(|point| point.id == 2)
            .unwrap();
        assert!((point.position[1] - 0.99).abs() < 0.001);
    }

    #[test]
    fn virtual_collection_items_expose_roles_selection_and_tree_expansion() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .add_systems(Update, route_accessibility_actions);
        let owner = app.world_mut().spawn(UiNode).id();
        let row = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                CollectionItemControl {
                    owner,
                    id: 7,
                    kind: CollectionItemKind::Tree,
                    index: 2,
                    depth: 1,
                    label: "Meshes".to_owned(),
                    enabled: true,
                    selected: true,
                    expanded: false,
                    has_children: true,
                    asset_id: None,
                    tint: [1.0; 4],
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        let node = &app.world().get::<AccessibilityNode>(row).unwrap().0;
        assert_eq!(node.role(), Role::TreeItem);
        assert_eq!(node.label(), Some("Meshes"));
        assert_eq!(node.level(), Some(2));
        assert_eq!(node.is_selected(), Some(true));
        assert_eq!(node.is_expanded(), Some(false));
        assert!(node.supports_action(Action::Expand));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Expand,
                target_tree: TreeId::ROOT,
                target_node: NodeId(row.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 1);
    }

    #[test]
    fn screen_reader_click_selects_a_virtual_list_item() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::actions::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let mut model = ListModel::<String>::default();
        model.items.push(bui_widgets::ListItem {
            id: 7,
            value: "Mesh".to_owned(),
            enabled: true,
        });
        let owner = app.world_mut().spawn((UiNode, model)).id();
        let row = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                CollectionItemControl {
                    owner,
                    id: 7,
                    kind: CollectionItemKind::List,
                    index: 0,
                    depth: 0,
                    label: "Mesh".to_owned(),
                    enabled: true,
                    selected: false,
                    expanded: false,
                    has_children: false,
                    asset_id: None,
                    tint: [1.0; 4],
                },
            ))
            .id();
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(row.to_bits()),
                data: None,
            }));
        app.update();
        assert!(
            app.world()
                .get::<ListModel<String>>(owner)
                .unwrap()
                .selection
                .contains(7)
        );
    }

    #[test]
    fn screen_reader_expands_tree_and_selects_preview_cell() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let tree = TreeModel::<String> {
            nodes: vec![
                bui_widgets::TreeNode {
                    id: 1,
                    parent: None,
                    children: vec![2],
                    value: "Root".to_owned(),
                    expanded: false,
                    enabled: true,
                },
                bui_widgets::TreeNode {
                    id: 2,
                    parent: Some(1),
                    children: Vec::new(),
                    value: "Child".to_owned(),
                    expanded: false,
                    enabled: true,
                },
            ],
            ..Default::default()
        };
        let tree_owner = app.world_mut().spawn((UiNode, tree)).id();
        let tree_row = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(tree_owner),
                CollectionItemControl {
                    owner: tree_owner,
                    id: 1,
                    kind: CollectionItemKind::Tree,
                    index: 0,
                    depth: 0,
                    label: "Root".to_owned(),
                    enabled: true,
                    selected: false,
                    expanded: false,
                    has_children: true,
                    asset_id: None,
                    tint: [1.0; 4],
                },
            ))
            .id();
        let mut grid = PreviewGrid::default();
        grid.items
            .push(bui_widgets::PreviewGridItem::new(7, "Material"));
        let grid_owner = app.world_mut().spawn((UiNode, grid)).id();
        let grid_cell = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(grid_owner),
                CollectionItemControl {
                    owner: grid_owner,
                    id: 7,
                    kind: CollectionItemKind::PreviewGrid,
                    index: 0,
                    depth: 0,
                    label: "Material".to_owned(),
                    enabled: true,
                    selected: false,
                    expanded: false,
                    has_children: false,
                    asset_id: None,
                    tint: [1.0; 4],
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(grid_cell)
                .unwrap()
                .0
                .role(),
            Role::GridCell
        );
        for (target, action) in [(tree_row, Action::Expand), (grid_cell, Action::Click)] {
            app.world_mut()
                .write_message(ActionRequest(AccessKitActionRequest {
                    action,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(target.to_bits()),
                    data: None,
                }));
            app.update();
        }
        assert!(
            app.world()
                .get::<TreeModel<String>>(tree_owner)
                .unwrap()
                .nodes[0]
                .expanded
        );
        assert!(
            app.world()
                .get::<PreviewGrid>(grid_owner)
                .unwrap()
                .selection
                .contains(7)
        );
    }

    #[test]
    fn tree_item_position_uses_sibling_order_after_reorder() {
        let mut world = World::new();
        let tree = TreeModel::<String> {
            nodes: vec![
                bui_widgets::TreeNode {
                    id: 1,
                    parent: None,
                    children: vec![3, 2],
                    value: "Root".to_owned(),
                    expanded: true,
                    enabled: true,
                },
                bui_widgets::TreeNode {
                    id: 2,
                    parent: Some(1),
                    children: vec![],
                    value: "Second".to_owned(),
                    expanded: false,
                    enabled: true,
                },
                bui_widgets::TreeNode {
                    id: 3,
                    parent: Some(1),
                    children: vec![],
                    value: "First".to_owned(),
                    expanded: false,
                    enabled: true,
                },
            ],
            ..Default::default()
        };
        let owner = world.spawn((UiNode, tree)).id();
        let row = world
            .spawn((
                UiNode,
                CollectionItemControl {
                    owner,
                    id: 3,
                    kind: CollectionItemKind::Tree,
                    index: 2,
                    depth: 1,
                    label: "First".to_owned(),
                    enabled: true,
                    selected: false,
                    expanded: false,
                    has_children: false,
                    asset_id: None,
                    tint: [1.0; 4],
                },
            ))
            .id();
        sync_accessibility(&mut world);
        let node = &world.get::<AccessibilityNode>(row).unwrap().0;
        assert_eq!(node.position_in_set(), Some(1));
        assert_eq!(node.size_of_set(), Some(2));
    }

    #[test]
    fn table_grid_cells_and_headers_route_accessible_selection_and_resize() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::actions::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let table = DataTable {
            columns: vec![bui_widgets::TableColumn::new("Name", 120.0)],
            rows: vec![bui_widgets::TableRow {
                id: 42,
                cells: vec!["Cube".to_owned()],
            }],
            ..Default::default()
        };
        let owner = app
            .world_mut()
            .spawn((UiNode, Label::new("Objects"), table))
            .id();
        let header = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                DataTableHeaderControl {
                    owner,
                    column: 0,
                    label: "Name".to_owned(),
                    width: 120.0,
                    sortable: true,
                    sort_ascending: Some(true),
                    active: true,
                },
            ))
            .id();
        let cell = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                DataTableCellControl {
                    owner,
                    row_id: 42,
                    row_index: 0,
                    column: 0,
                    column_label: "Name".to_owned(),
                    value: "Cube".to_owned(),
                    selected: false,
                    active: false,
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        let root = &app.world().get::<AccessibilityNode>(owner).unwrap().0;
        assert_eq!(root.role(), Role::Grid);
        assert_eq!(root.row_count(), Some(1));
        assert_eq!(root.column_count(), Some(1));
        let header_node = &app.world().get::<AccessibilityNode>(header).unwrap().0;
        assert_eq!(header_node.role(), Role::ColumnHeader);
        assert_eq!(header_node.sort_direction(), Some(SortDirection::Ascending));
        let cell_node = &app.world().get::<AccessibilityNode>(cell).unwrap().0;
        assert_eq!(cell_node.role(), Role::GridCell);
        assert_eq!(cell_node.label(), Some("Name, Cube"));
        assert_eq!(cell_node.row_index(), Some(0));
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(cell.to_bits()),
                data: None,
            }));
        app.update();
        assert!(
            app.world()
                .get::<DataTable>(owner)
                .unwrap()
                .selection
                .contains(42)
        );
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Increment,
                target_tree: TreeId::ROOT,
                target_node: NodeId(header.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(
            app.world().get::<DataTable>(owner).unwrap().columns[0].width,
            128.0
        );
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(header.to_bits()),
                data: None,
            }));
        app.update();
        assert_eq!(
            app.world().get::<DataTable>(owner).unwrap().sort,
            Some((0, true))
        );
    }

    #[test]
    fn screen_reader_can_connect_node_sockets_in_either_focus_order() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::WidgetEventQueue>()
            .init_resource::<AccessibilityLinkSelection>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let mut graph = NodeGraph::default();
        let source = graph.add_node(
            "Source",
            [0.0, 0.0],
            vec![bui_widgets::NodeSocket::output(
                "Value",
                bui_widgets::SocketType::Float,
            )],
        );
        let sink = graph.add_node(
            "Sink",
            [220.0, 0.0],
            vec![bui_widgets::NodeSocket::input(
                "Value",
                bui_widgets::SocketType::Float,
            )],
        );
        let owner = app.world_mut().spawn((UiNode, graph)).id();
        let output = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                NodeGraphSocketControl {
                    owner,
                    node_id: source,
                    socket_index: 0,
                    direction: SocketDirection::Output,
                    label: "Value".to_owned(),
                    socket_type: bui_widgets::SocketType::Float,
                },
            ))
            .id();
        let input = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                NodeGraphSocketControl {
                    owner,
                    node_id: sink,
                    socket_index: 0,
                    direction: SocketDirection::Input,
                    label: "Value".to_owned(),
                    socket_type: bui_widgets::SocketType::Float,
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        let output_node = &app.world().get::<AccessibilityNode>(output).unwrap().0;
        assert_eq!(output_node.role(), Role::Button);
        assert!(output_node.supports_action(Action::Click));
        assert!(output_node.description().unwrap().contains("create a link"));

        let click = |app: &mut App, target: Entity| {
            app.world_mut()
                .write_message(ActionRequest(AccessKitActionRequest {
                    action: Action::Click,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(target.to_bits()),
                    data: None,
                }));
            app.update();
        };
        click(&mut app, output);
        sync_accessibility(app.world_mut());
        let selected = &app.world().get::<AccessibilityNode>(output).unwrap().0;
        assert_eq!(selected.is_selected(), Some(true));
        assert_eq!(selected.live(), Some(Live::Polite));
        click(&mut app, output);
        click(&mut app, input);
        assert!(
            app.world()
                .get::<NodeGraph>(owner)
                .unwrap()
                .links
                .is_empty()
        );
        click(&mut app, output);
        let graph = app.world().get::<NodeGraph>(owner).unwrap();
        assert_eq!(graph.links.len(), 1);
        assert_eq!(graph.links[0].from_node, source);
        assert_eq!(graph.links[0].to_node, sink);
        let link_id = graph.links[0].id;
        app.world_mut()
            .get_mut::<NodeGraph>(owner)
            .unwrap()
            .remove_link(link_id);
        click(&mut app, output);
        click(&mut app, input);
        assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().links.len(), 1);
        click(&mut app, output);
        app.world_mut().entity_mut(output).despawn();
        sync_accessibility(app.world_mut());
        assert!(
            app.world()
                .resource::<AccessibilityLinkSelection>()
                .0
                .is_none()
        );
    }

    #[test]
    fn node_editor_virtual_controls_route_screen_reader_selection_and_collapse() {
        let mut app = App::new();
        app.add_message::<ActionRequest>()
            .init_resource::<UiFocus>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<bui_widgets::WidgetEventQueue>()
            .add_systems(
                Update,
                (
                    route_accessibility_actions,
                    bui_widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let mut graph = NodeGraph::default();
        let source = graph.add_node(
            "Source",
            [0.0, 0.0],
            vec![bui_widgets::NodeSocket::output(
                "Value",
                bui_widgets::SocketType::Float,
            )],
        );
        let sink = graph.add_node(
            "Sink",
            [220.0, 0.0],
            vec![bui_widgets::NodeSocket::input(
                "Value",
                bui_widgets::SocketType::Float,
            )],
        );
        let link_id = graph.create_link(source, 0, sink, 0).unwrap();
        let owner = app.world_mut().spawn((UiNode, graph)).id();
        let node = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                NodeGraphNodeControl {
                    owner,
                    node_id: source,
                    label: "Source".to_owned(),
                    selected: false,
                    collapsed: false,
                },
            ))
            .id();
        let socket = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                NodeGraphSocketControl {
                    owner,
                    node_id: source,
                    socket_index: 0,
                    direction: bui_widgets::SocketDirection::Output,
                    label: "Value".to_owned(),
                    socket_type: bui_widgets::SocketType::Float,
                },
            ))
            .id();
        let link = app
            .world_mut()
            .spawn((
                UiNode,
                UiParent(owner),
                NodeGraphLinkControl {
                    owner,
                    link_id,
                    from_node: source,
                    from_socket: 0,
                    to_node: sink,
                    to_socket: 0,
                    selected: false,
                },
            ))
            .id();
        sync_accessibility(app.world_mut());
        assert_eq!(
            app.world().get::<AccessibilityNode>(node).unwrap().0.role(),
            Role::Group
        );
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(socket)
                .unwrap()
                .0
                .role(),
            Role::Button
        );
        assert_eq!(
            app.world().get::<AccessibilityNode>(link).unwrap().0.role(),
            Role::Button
        );
        for (action, target) in [
            (Action::Click, node),
            (Action::Collapse, node),
            (Action::Click, link),
        ] {
            app.world_mut()
                .write_message(ActionRequest(AccessKitActionRequest {
                    action,
                    target_tree: TreeId::ROOT,
                    target_node: NodeId(target.to_bits()),
                    data: None,
                }));
            app.update();
        }
        let graph = app.world().get::<NodeGraph>(owner).unwrap();
        assert!(graph.nodes[0].collapsed);
        assert!(graph.selected.is_empty());
        assert_eq!(graph.selected_link, Some(link_id));
    }
}

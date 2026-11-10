//! AccessKit semantics and actions for BUI's logical control entities.

use std::collections::HashSet;

use accesskit::{
    Action, ActionData, Color as AccessibleColor, HasPopup, Live, Node, Rect as AccessibleRect,
    Role, SortDirection, Toggled,
};
use bevy::a11y::{AccessibilityNode, ActionRequest};
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bui_core::{
    UiAccessibleName, UiChildren, UiDirty, UiDirtyDomain, UiEnabled, UiFocus, UiFocusable,
    UiLivePoliteness, UiLiveRegion, UiNode, UiParent, UiTab, UiTabList, UiVisible,
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

#[derive(Resource, Default)]
pub(crate) struct AccessibilitySyncCache {
    prepared: bool,
    selected_socket: Option<Entity>,
}

pub(crate) fn collect_accessibility_resource_changes(
    selection: Res<AccessibilityLinkSelection>,
    mut cache: ResMut<AccessibilitySyncCache>,
    mut dirty: ResMut<UiDirty>,
) {
    if selection.is_changed() {
        let current = selection.0.as_ref().map(|(entity, _)| *entity);
        for entity in [cache.selected_socket, current].into_iter().flatten() {
            dirty.mark(UiDirtyDomain::Accessibility, entity);
        }
        cache.selected_socket = current;
    }
    cache.prepared = true;
}

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
    let prepared = world
        .get_resource_mut::<AccessibilitySyncCache>()
        .is_some_and(|mut cache| std::mem::take(&mut cache.prepared));
    let batch = prepared.then(|| {
        world
            .resource_mut::<UiDirty>()
            .take(UiDirtyDomain::Accessibility)
    });
    if batch.as_ref().is_some_and(bui_core::UiDirtyBatch::is_empty) {
        return;
    }
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
    let entities: Vec<_> = if let Some(batch) = batch.filter(|batch| !batch.all) {
        let mut seen = HashSet::new();
        let mut pending: Vec<_> = batch.entities.into_iter().collect();
        while let Some(entity) = pending.pop() {
            if !seen.insert(entity) {
                continue;
            }
            if let Some(children) = world.get::<UiChildren>(entity) {
                pending.extend(children.0.iter().copied());
            }
        }
        seen.into_iter().collect()
    } else {
        world
            .query_filtered::<Entity, Or<(With<UiNode>, With<AccessibilityNode>)>>()
            .iter(world)
            .collect()
    };
    for entity in entities {
        if world.get::<UiNode>(entity).is_none() {
            if world.get::<AccessibilityNode>(entity).is_some() {
                world.entity_mut(entity).remove::<AccessibilityNode>();
            }
            continue;
        }
        let Some(mut node) = node_for(world, entity) else {
            if world.get::<AccessibilityNode>(entity).is_some() {
                world.entity_mut(entity).remove::<AccessibilityNode>();
            }
            continue;
        };
        if let Some(name) = world.get::<UiAccessibleName>(entity) {
            node.set_label(name.0.clone());
        }
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

mod nodes;
use nodes::*;

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
#[path = "accessibility/tests.rs"]
mod tests;

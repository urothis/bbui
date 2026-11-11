//! AccessKit semantics and actions for BBUI's logical control entities.

use std::collections::{HashMap, HashSet};

use accesskit::{
    Action, ActionData, Color as AccessibleColor, HasPopup, Live, Node, Rect as AccessibleRect,
    Role, SortDirection, Toggled,
};
use bbui_core::{
    UiAccessibleName, UiChildren, UiDirty, UiDirtyDomain, UiEnabled, UiFocus, UiFocusable,
    UiHitBox, UiLivePoliteness, UiLiveRegion, UiNode, UiParent, UiTab, UiTabList, UiVisible,
};
use bbui_layout::UiLayoutResult;
use bbui_render::UiWorldSurface;
use bbui_text::UiReadOnly;
use bbui_widgets::actions::{WidgetAction, WidgetActionKind, WidgetActionQueue};
use bbui_widgets::{
    Button, Canvas, CanvasObjectControl, Checkbox, CheckboxValue, CollectionItemControl,
    CollectionItemKind, ColorRamp, CurveEditor, DataTable, DataTableCellControl,
    DataTableHeaderControl, Dropdown, DropdownOptionControl, FileBrowser, FileBrowserItem,
    HsvColorEditor, HsvColorField, IconButton, IconButtonAssetState, IconButtonAssetStatus, Label,
    ListModel, MenuControl, MenuItemControl, NodeGraph, NodeGraphLinkControl, NodeGraphNodeControl,
    NodeGraphSocketControl, NumericField, Panel, PanelGroup, PreviewGrid, Progress, RadioGroup,
    RadioOptionControl, Slider, SocketDirection, TextArea, TextField, Tooltip, TreeModel,
};
use bevy::a11y::{AccessibilityNode, ActionRequest};
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowRef};

#[derive(Resource, Default)]
pub struct BbuiFocusMirror(Option<Entity>);

#[derive(Resource, Default)]
pub(crate) struct AccessibilitySyncCache {
    prepared: bool,
    selected_socket: Option<Entity>,
    world_targets: HashSet<Entity>,
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

/// A moving mesh or view camera changes screen-reader geometry without changing UI layout.
#[derive(PartialEq)]
pub(crate) struct AccessibilitySceneStamp {
    surfaces: Vec<(Entity, UiWorldSurface, GlobalTransform)>,
    views: Vec<(Entity, Mat4, Mat4, Option<bevy::math::Rect>, bool, Entity)>,
    windows: Vec<(Entity, u32, u32, u32)>,
}

pub(crate) fn collect_world_accessibility_changes(
    surfaces: Query<(Entity, &UiWorldSurface, &GlobalTransform)>,
    cameras: Query<(Entity, &Camera, &RenderTarget, &GlobalTransform)>,
    windows: Query<(Entity, &Window)>,
    primary: Query<Entity, With<PrimaryWindow>>,
    mut previous: Local<Option<AccessibilitySceneStamp>>,
    mut dirty: ResMut<UiDirty>,
) {
    let surfaces: Vec<_> = surfaces
        .iter()
        .map(|(entity, surface, transform)| (entity, *surface, *transform))
        .collect();
    let old_had_surfaces = previous
        .as_ref()
        .is_some_and(|stamp| !stamp.surfaces.is_empty());
    if surfaces.is_empty() && !old_had_surfaces {
        return;
    }
    let primary = primary.iter().next();
    let views = cameras
        .iter()
        .filter_map(|(entity, camera, target, transform)| {
            let window = match target {
                RenderTarget::Window(WindowRef::Primary) => primary?,
                RenderTarget::Window(WindowRef::Entity(window)) => *window,
                _ => return None,
            };
            Some((
                entity,
                transform.to_matrix(),
                camera.computed.clip_from_view,
                camera.logical_viewport_rect(),
                camera.is_active,
                window,
            ))
        })
        .collect();
    let windows = windows
        .iter()
        .map(|(entity, window)| {
            (
                entity,
                window.resolution.physical_width(),
                window.resolution.physical_height(),
                window.scale_factor().to_bits(),
            )
        })
        .collect();
    let current = AccessibilitySceneStamp {
        surfaces,
        views,
        windows,
    };
    if previous.as_ref() != Some(&current) {
        dirty.mark_all(UiDirtyDomain::Accessibility);
        *previous = Some(current);
    }
}

struct WorldProjection {
    surfaces: HashMap<Entity, Vec<(UiWorldSurface, GlobalTransform)>>,
    views: Vec<(Camera, GlobalTransform, f32, bevy::math::Rect)>,
    previous_targets: HashSet<Entity>,
}

impl WorldProjection {
    fn collect(world: &mut World, previous_targets: HashSet<Entity>) -> Self {
        let mut surfaces: HashMap<Entity, Vec<_>> = HashMap::new();
        for (surface, transform) in world
            .query::<(&UiWorldSurface, &GlobalTransform)>()
            .iter(world)
        {
            surfaces
                .entry(surface.target_camera)
                .or_default()
                .push((*surface, *transform));
        }
        let primary = world
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .iter(world)
            .next();
        let views: Vec<_> = world
            .query::<(&Camera, &RenderTarget, &GlobalTransform)>()
            .iter(world)
            .filter_map(|(camera, target, transform)| {
                let window = match target {
                    RenderTarget::Window(WindowRef::Primary) => primary?,
                    RenderTarget::Window(WindowRef::Entity(entity)) => *entity,
                    _ => return None,
                };
                if !camera.is_active {
                    return None;
                }
                let viewport = camera.logical_viewport_rect()?;
                Some((
                    camera.clone(),
                    *transform,
                    world
                        .get::<Window>(window)
                        .map_or(1.0, Window::scale_factor),
                    viewport,
                ))
            })
            .collect();
        Self {
            surfaces,
            views,
            previous_targets,
        }
    }

    fn bounds(&self, layout: &UiLayoutResult) -> Option<Option<AccessibleRect>> {
        let Some(surfaces) = self.surfaces.get(&layout.window) else {
            return self
                .previous_targets
                .contains(&layout.window)
                .then_some(None);
        };
        let rect = layout.rect;
        let corners = [
            Vec2::new(rect.x, rect.y),
            Vec2::new(rect.x + rect.width, rect.y),
            Vec2::new(rect.x, rect.y + rect.height),
            Vec2::new(rect.x + rect.width, rect.y + rect.height),
        ];
        for (surface, transform) in surfaces {
            if surface.resolution.x == 0
                || surface.resolution.y == 0
                || surface.world_size.min_element() <= 0.0
            {
                continue;
            }
            for (camera, view_transform, scale, viewport) in &self.views {
                let mut points = [Vec2::ZERO; 4];
                let mut valid = true;
                for (point, corner) in points.iter_mut().zip(corners) {
                    let local = Vec3::new(
                        (corner.x / surface.resolution.x as f32 - 0.5) * surface.world_size.x,
                        (0.5 - corner.y / surface.resolution.y as f32) * surface.world_size.y,
                        0.0,
                    );
                    let world_point = transform.transform_point(local);
                    let Ok(screen) = camera.world_to_viewport(view_transform, world_point) else {
                        valid = false;
                        break;
                    };
                    *point = screen;
                }
                if !valid {
                    continue;
                }
                let x0 = points
                    .iter()
                    .map(|p| p.x)
                    .fold(f32::INFINITY, f32::min)
                    .max(viewport.min.x)
                    * scale;
                let y0 = points
                    .iter()
                    .map(|p| p.y)
                    .fold(f32::INFINITY, f32::min)
                    .max(viewport.min.y)
                    * scale;
                let x1 = points
                    .iter()
                    .map(|p| p.x)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .min(viewport.max.x)
                    * scale;
                let y1 = points
                    .iter()
                    .map(|p| p.y)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .min(viewport.max.y)
                    * scale;
                if x1 <= x0 || y1 <= y0 {
                    continue;
                }
                if [x0, y0, x1, y1].iter().all(|value| value.is_finite()) {
                    return Some(Some(AccessibleRect {
                        x0: f64::from(x0),
                        y0: f64::from(y0),
                        x1: f64::from(x1),
                        y1: f64::from(y1),
                    }));
                }
            }
        }
        Some(None)
    }
}

pub fn sync_accessible_focus(
    focus: Res<UiFocus>,
    mut mirror: ResMut<BbuiFocusMirror>,
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
    if batch
        .as_ref()
        .is_some_and(bbui_core::UiDirtyBatch::is_empty)
    {
        return;
    }
    let previous_targets = world
        .get_resource::<AccessibilitySyncCache>()
        .map_or_else(HashSet::new, |cache| cache.world_targets.clone());
    let projection = WorldProjection::collect(world, previous_targets);
    if let Some(mut cache) = world.get_resource_mut::<AccessibilitySyncCache>() {
        cache.world_targets = projection.surfaces.keys().copied().collect();
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
            let layout = *layout;
            match projection.bounds(&layout) {
                Some(Some(bounds)) => node.set_bounds(bounds),
                Some(None) => node.set_hidden(),
                None => {
                    let rect = layout.rect;
                    let scale = f64::from(layout.dpi_scale);
                    node.set_bounds(AccessibleRect {
                        x0: f64::from(rect.x) * scale,
                        y0: f64::from(rect.y) * scale,
                        x1: f64::from(rect.x + rect.width) * scale,
                        y1: f64::from(rect.y + rect.height) * scale,
                    });
                }
            }
        }
        if node.is_hidden() {
            node.clear_actions();
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
    Option<&'a UiHitBox>,
    Option<&'a UiLayoutResult>,
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
            let Ok((enabled, visible, parent, hit, layout)) = nodes.get(at) else {
                allowed = false;
                break;
            };
            if enabled.is_some_and(|enabled| !enabled.0)
                || visible.is_some_and(|visible| !visible.0)
                || hit.is_some_and(|hit| {
                    hit.clip
                        .is_some_and(|clip| hit.rect.intersection(clip).is_none())
                })
                || layout.is_some_and(|layout| {
                    layout
                        .clip
                        .is_some_and(|clip| layout.rect.intersection(clip).is_none())
                })
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
                        .and_then(|(_, _, parent, _, _)| parent)
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

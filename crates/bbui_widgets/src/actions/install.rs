use super::*;

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
        .init_resource::<WidgetNodeGraphDeltas>()
        .init_resource::<WidgetMenuPopupStamps>()
        .init_resource::<WidgetMenuPopupGate>()
        .init_resource::<WidgetPanelSyncCache>()
        .init_resource::<WidgetPanelSyncInvalidations>()
        .init_resource::<WidgetPanelSyncGate>()
        .init_resource::<WidgetPanelInvalidationGate>()
        .init_resource::<WidgetTooltips>()
        .init_resource::<WidgetMenuAnimations>()
        .init_resource::<WidgetMenuAnimationGate>()
        .init_resource::<UiRoutedEvents>()
        .init_resource::<DisplayList>()
        .init_resource::<Theme>()
        .add_systems(
            Update,
            capture_focused_text_edit_drafts
                .in_set(UiSystems::Text)
                .before(bbui_text::process_text_events),
        )
        .add_systems(
            Update,
            (
                process_routed_widget_events,
                advance_tooltips,
                apply_widget_actions,
                sync_panel_animations,
                collect_panel_descendant_invalidations,
                sync_panel_children,
                sync_menu_popups,
                sync_menu_animations,
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
        crate::canvas::CanvasObjectPaint,
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
        NodeGraphNodePaintState,
        NodeGraphSocketControl,
        NodeGraphLinkControl,
        NodeGraphLinkPaintGeometry,
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
    app.add_observer(mark_panel_topology_removed::<Panel>);
    app.add_observer(mark_panel_animation_removed);
    app.add_observer(mark_panel_topology_removed::<UiParent>);
    app.add_observer(mark_panel_topology_removed::<UiChildren>);
    app.add_observer(mark_menu_animation_removed);
    app.add_observer(mark_menu_popup_input_removed::<UiLayoutRoot>);
    app.add_observer(mark_menu_popup_input_removed::<MenuControl>);
    app.add_observer(mark_menu_popup_input_removed::<UiLayoutResult>);
    app.add_observer(mark_menu_popup_input_removed::<MenuItemControl>);
    app.add_observer(mark_menu_popup_input_removed::<UiVisible>);
    app.add_observer(mark_menu_popup_input_removed::<UiEnabled>);
}

pub(super) fn mark_menu_popup_input_removed<T: Component>(
    _on: On<Remove<T>>,
    mut gate: ResMut<WidgetMenuPopupGate>,
) {
    gate.initialized = false;
}

pub(super) fn mark_panel_topology_removed<T: Component>(
    on: On<Remove<T>>,
    mut invalidations: ResMut<WidgetPanelSyncInvalidations>,
) {
    invalidations.topology_changed = true;
    invalidations.panels.insert(on.entity);
}

pub(super) fn mark_menu_animation_removed(
    on: On<Remove<MenuControl>>,
    mut animations: ResMut<WidgetMenuAnimations>,
    mut gate: ResMut<WidgetMenuAnimationGate>,
    player: Option<ResMut<bbui_motion::MotionPlayer>>,
) {
    animations.0.remove(&on.entity);
    gate.initialized = false;
    if let Some(mut player) = player {
        player.cancel_binding(on.entity, menu_progress_binding().id());
    }
}

pub(super) fn mark_panel_animation_removed(
    on: On<Remove<Panel>>,
    player: Option<ResMut<bbui_motion::MotionPlayer>>,
) {
    if let Some(mut player) = player {
        player.cancel_binding(on.entity, panel_progress_binding().id());
    }
}

pub(super) fn mark_paint_component_removed<T: Component>(
    on: On<Remove<T>>,
    mut dirty: ResMut<UiDirty>,
) {
    dirty.mark(UiDirtyDomain::Paint, on.entity);
    dirty.mark(UiDirtyDomain::Accessibility, on.entity);
}

pub(super) fn mark_inherited_paint_component_removed<T: Component>(
    on: On<Remove<T>>,
    children: Query<'_, '_, &'static UiChildren>,
    mut dirty: ResMut<UiDirty>,
) {
    mark_widget_paint_subtree(on.entity, &children, &mut dirty);
}

pub(super) fn mark_widget_parent_removed(
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
pub(super) fn mark_widget_children_removed(
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

pub(super) fn mark_widget_layout_root_removed(
    on: On<Remove<UiLayoutRoot>>,
    children: Query<'_, '_, &'static UiChildren>,
    mut dirty: ResMut<UiDirty>,
) {
    mark_widget_paint_subtree(on.entity, &children, &mut dirty);
}

pub(super) fn remove_widget_paint_layout_rank(
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
pub(super) fn prune_widget_paint_root(
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

pub(super) fn prune_widget_order_cache(entity: Entity, cache: &mut WidgetPaintCache) {
    if cache.paint_roots.get(&entity.index_u32()) == Some(&entity) {
        cache.paint_roots.remove(&entity.index_u32());
    }
    if let Some(rank) = cache.paint_ranks.remove(&entity) {
        cache.paint_order_keys.remove(&rank);
    }
}

pub(super) fn mark_widget_paint_subtree(
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

pub(super) fn widget_paint_subtree(world: &World, root: Entity) -> HashSet<Entity> {
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

pub(super) fn mark_menu_paint_subtree(world: &mut World, owner: Entity) {
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

pub(super) fn mark_widget_paint_camera_removed(
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

pub(super) fn mark_widget_paint_camera_added(
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
pub(super) fn refresh_text_layouts_after_sync(world: &mut World) {
    if world.contains_resource::<bevy_text::TextPipeline>() {
        bbui_text::collect_text_widget_changes(world);
        bbui_text::update_text_layouts_if_dirty(world);
    }
}

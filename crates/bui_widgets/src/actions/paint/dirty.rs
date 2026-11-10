use super::*;

pub(in crate::actions) fn styled_theme(world: &World, entity: Entity, base: &Theme) -> Theme {
    let mut theme = base.clone();
    let Some(style) = world.get::<UiComputedStyle>(entity) else {
        return theme;
    };
    // A graph canvas owns its hover interaction, not a generic hover surface.
    // Keep explicit inline and stylesheet backgrounds available to graph users.
    let graph_background_is_interaction = world.get::<NodeGraph>(entity).is_some()
        && !node_graph_has_background_override(world, entity);
    if let Some(background) = style.color(UiStyleProperty::Background)
        && !graph_background_is_interaction
    {
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
pub(in crate::actions) struct WidgetPaintInvalidations<'w, 's> {
    parent_changes: Query<'w, 's, Entity, Changed<UiParent>>,
    children_changes: Query<'w, 's, Entity, Changed<UiChildren>>,
    root_changes: Query<'w, 's, Entity, Changed<UiLayoutRoot>>,
    style_changes: Query<'w, 's, Entity, Changed<UiComputedStyle>>,
    node_graph_roots: Query<'w, 's, Entity, With<NodeGraph>>,
    inherited_state: Query<'w, 's, Entity, Or<(Changed<UiEnabled>, Changed<UiVisible>)>>,
    children: Query<'w, 's, &'static UiChildren>,
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
pub(in crate::actions) fn collect_widget_paint_changes_query(
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
            Changed<NodeGraphNodeControl>,
            Changed<NodeGraphNodePaintState>,
            Changed<NodeGraphSocketControl>,
            Changed<NodeGraphLinkControl>,
            Changed<NodeGraphLinkPaintGeometry>,
            Changed<crate::canvas::CanvasObjectControl>,
            Changed<crate::canvas::CanvasObjectPaint>,
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
            Changed<bui_core::UiAccessibleName>,
            Changed<bui_core::UiTab>,
            Changed<bui_core::UiTabList>,
            Changed<crate::canvas::CanvasObjectControl>,
            Changed<crate::IconButtonAssetState>,
            Changed<crate::PanelGroup>,
            Changed<RadioOptionControl>,
        )>,
    >,
    theme: Res<Theme>,
    interaction: Res<WidgetInteractionState>,
    focus: Option<Res<UiFocus>>,
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
    for entity in &invalidations.style_changes {
        if invalidations.node_graph_roots.get(entity).is_ok() {
            mark_widget_paint_subtree(entity, &invalidations.children, &mut dirty);
        }
    }
    if theme.is_changed() {
        dirty.mark_all(UiDirtyDomain::Paint);
    }
    for entity in &invalidations.inherited_state {
        mark_widget_paint_subtree(entity, &invalidations.children, &mut dirty);
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

pub(in crate::actions) fn collect_widget_paint_changes(world: &mut World) {
    world.init_resource::<WidgetPaintCache>();
    world.init_resource::<WidgetPaintSummaryGate>();
    world.init_resource::<UiDirty>();
    world.init_resource::<Theme>();
    world.init_resource::<WidgetInteractionState>();
    let changed = world.resource_scope(|world, mut gate: Mut<WidgetPaintSummaryGate>| {
        if !gate.initialized {
            macro_rules! track {
                ($($component:ty),+ $(,)?) => {
                    $(gate.components.push(world.register_component::<$component>());)+
                };
            }
            track!(
                UiNode,
                UiParent,
                UiChildren,
                UiEnabled,
                UiVisible,
                UiLayoutResult,
                UiLayoutRoot,
                UiComputedStyle,
                UiRole,
                UiTextLayout,
                TextBuffer,
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
                NodeGraphNodeControl,
                NodeGraphNodePaintState,
                NodeGraphSocketControl,
                NodeGraphLinkControl,
                NodeGraphLinkPaintGeometry,
                crate::canvas::CanvasObjectControl,
                crate::canvas::CanvasObjectPaint,
                Tooltip,
                IconButtonAsset,
                DropdownPopupState,
                CollectionEditorState,
                HsvColorEditorState,
                ColorRampEditorState,
                NodeGraphEditorState,
                FileBrowserViewState,
                UiFocusable,
                UiLiveRegion,
                UiReadOnly,
                UiKey,
                bui_core::UiAccessibleName,
                bui_core::UiTab,
                bui_core::UiTabList,
                crate::IconButtonAssetState,
                crate::PanelGroup,
                RadioOptionControl,
            );
            gate.initialized = true;
            world
                .resource_mut::<UiDirty>()
                .mark_all(UiDirtyDomain::Paint);
            world
                .resource_mut::<UiDirty>()
                .mark_all(UiDirtyDomain::Accessibility);
        }
        let now = world.read_change_tick();
        let changed = ui_components_changed_since(world, &gate.components, gate.last)
            || world
                .get_resource_change_ticks::<Theme>()
                .is_some_and(|ticks| ticks.is_changed(gate.last, now))
            || world
                .get_resource_change_ticks::<WidgetInteractionState>()
                .is_some_and(|ticks| ticks.is_changed(gate.last, now))
            || world
                .get_resource_change_ticks::<UiFocus>()
                .is_some_and(|ticks| ticks.is_changed(gate.last, now));
        gate.last = now;
        changed
    });
    if changed
        && world
            .run_system_cached(collect_widget_paint_changes_query)
            .is_err()
    {
        world
            .resource_mut::<UiDirty>()
            .mark_all(UiDirtyDomain::Paint);
        world
            .resource_mut::<UiDirty>()
            .mark_all(UiDirtyDomain::Accessibility);
    }
    world.resource_mut::<WidgetPaintCache>().prepared = true;
    // Direct callers can edit the world immediately after collection.
    // Assign those edits a later tick than the one this gate observed.
    world.increment_change_tick();
}

pub fn collect_widget_paint(world: &mut World) {
    world.init_resource::<WidgetPaintCache>();
    world.init_resource::<UiDirty>();
    if !std::mem::take(&mut world.resource_mut::<WidgetPaintCache>().prepared) {
        collect_widget_paint_changes(world);
        world.resource_mut::<WidgetPaintCache>().prepared = false;
    }
    let batch = world.resource_mut::<UiDirty>().take(UiDirtyDomain::Paint);
    if batch.is_empty() {
        world.resource_mut::<WidgetPaintCache>().skipped_frames += 1;
        return;
    }
    collect_widget_paint_full(world, Some(batch));
}

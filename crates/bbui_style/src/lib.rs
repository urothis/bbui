//! Typed theme tokens and a deterministic state-aware style cascade.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use bbui_core::{
    UiChildren, UiDirty, UiDirtyBatch, UiDirtyDomain, UiEnabled, UiFocus, UiNode, UiParent,
    UiRoutedEvent, UiRoutedEvents, UiSystems, refresh_ui_hover, ui_components_changed_since,
};
use bbui_layout::{
    UiAlign, UiEdges, UiJustify, UiLayoutMode, UiLayoutNode, UiLength, compute_layout,
};
pub use bbui_motion::MotionCurve;
use bbui_motion::{MotionPlugin, MotionSystems};
use bevy_app::{App, Plugin, Update};
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::{
    change_detection::Tick, component::ComponentId, lifecycle::Remove, observer::On,
    system::SystemChangeTick,
};
use bevy_time::Time;
use bevy_ui::{BoxShadow, Val};

mod theme;
pub use theme::*;

mod editor;
pub use editor::EditorRole;

mod cascade;
pub use cascade::*;

/// Captured pre-cascade layout values. This lets state/theme changes remove a
/// previous declaration without permanently overwriting caller components.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[component(summary_tick)]
pub struct UiBaseLayoutNode(pub UiLayoutNode);

/// The unanimated cascade result used when descendants inherit style values.
#[derive(Component, Debug, Clone, PartialEq)]
struct UiTargetStyle(UiComputedStyle);

#[derive(Component, Debug, Clone, PartialEq)]
struct UiBaseBoxShadow(Option<BoxShadow>);

pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MotionPlugin)
            .init_resource::<Theme>()
            .init_resource::<UiDirty>()
            .init_resource::<UiReducedMotion>()
            .init_resource::<UiStyleSheet>()
            .init_resource::<UiStyleResolutionCache>()
            .init_resource::<UiStyleCollectorCursor>()
            .init_resource::<UiStyleResourceCursor>()
            .init_resource::<UiStyleMotionOutputs>()
            .init_resource::<UiStylePaintOnlyOutputs>()
            .init_resource::<UiRoutedEvents>()
            .add_systems(
                Update,
                (
                    begin_motion_frame,
                    collect_style_component_changes_gated,
                    collect_style_resource_changes,
                    resolve_ui_styles_dirty,
                    sync_motion_settings,
                )
                    .chain()
                    .in_set(UiSystems::Style),
            )
            .add_systems(
                Update,
                clear_style_motion_paint_outputs.in_set(MotionSystems::Prepare),
            )
            .add_systems(
                Update,
                (
                    collect_style_component_changes_gated,
                    collect_style_resource_changes,
                    resolve_ui_styles_dirty,
                )
                    .chain()
                    .in_set(UiSystems::StyleAfterInput),
            )
            .add_systems(
                Update,
                converge_hover_layout
                    .run_if(has_pointer_hover_transition)
                    .in_set(UiSystems::LayoutAfterStyle),
            );
        macro_rules! register_style_removals {
            ($($component:ty),+ $(,)?) => {
                $(app.add_observer(mark_style_component_removed::<$component>);)+
            };
        }
        register_style_removals!(
            UiNode,
            UiParent,
            UiRole,
            UiClasses,
            UiStyleState,
            UiEnabled,
            UiInlineStyle,
            UiAutoAnimations,
            UiBaseLayoutNode,
            UiLayoutNode,
            UiComputedStyle,
        );
    }
}

fn mark_style_component_removed<T: Component>(on: On<Remove<T>>, mut dirty: ResMut<UiDirty>) {
    dirty.mark(UiDirtyDomain::Style, on.entity);
}

#[derive(Resource, Debug, Default)]
struct UiStyleResolutionCache {
    focus_targets: HashMap<Entity, u8>,
    resolved_passes: u64,
    skipped_passes: u64,
}

#[derive(Resource, Debug, Default)]
struct UiStyleCollectorCursor(Tick);

#[derive(Resource, Debug, Default)]
struct UiStyleResourceCursor(Tick);

/// Exact ticks written by paint-only keyframe tracks; external computed-style edits
/// remain visible to the style collector.
#[derive(Resource, Debug, Default)]
struct UiStyleMotionOutputs(HashMap<Entity, Tick>);

/// Exact computed-style writes that only advanced a background transition.
/// Widget painting consumes these after the current paint pass.
#[derive(Resource, Debug, Default)]
pub struct UiStylePaintOnlyOutputs(HashMap<Entity, (Tick, bool)>);

impl UiStylePaintOnlyOutputs {
    /// Whether this exact computed-style write only advanced a background transition.
    pub fn background_only_at(&self, entity: Entity, changed: Tick) -> bool {
        self.0.get(&entity) == Some(&(changed, true))
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Resource, Default)]
struct UiStyleSummaryGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
}

mod collect;
use collect::*;

mod resolution;
pub use resolution::resolve_ui_styles;
use resolution::*;

mod motion;
pub use motion::style_motion_binding;
use motion::*;

mod resolve;
pub use resolve::resolve_style;

mod layout_apply;
use layout_apply::*;

mod math;
use math::*;

#[cfg(test)]
#[path = "style/tests.rs"]
mod tests;

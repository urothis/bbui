//! Typed theme tokens and a deterministic state-aware style cascade.

use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use bevy_app::{App, Plugin, Update};
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::{change_detection::Tick, lifecycle::Remove, observer::On, system::SystemChangeTick};
use bevy_time::Time;
use bevy_ui::{BoxShadow, Val};
use bui_core::{
    UiChildren, UiDirty, UiDirtyBatch, UiDirtyDomain, UiEnabled, UiFocus, UiNode, UiParent,
    UiRoutedEvent, UiRoutedEvents, UiSystems, refresh_ui_hover,
};
use bui_layout::{
    UiAlign, UiEdges, UiJustify, UiLayoutMode, UiLayoutNode, UiLength, compute_layout,
};

mod theme;
pub use theme::*;

mod cascade;
pub use cascade::*;

#[derive(Debug, Clone, PartialEq)]
struct StyleTrack {
    from: UiStyleValue,
    to: UiStyleValue,
    elapsed: Duration,
    duration: Duration,
    easing: UiEasing,
}

/// Active transitions for one entity. Tracks are kept private; consumers read
/// the interpolated values from [`UiComputedStyle`].
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct UiStyleTransitionState {
    tracks: BTreeMap<UiStyleProperty, StyleTrack>,
}

impl UiStyleTransitionState {
    pub fn is_transitioning(&self, property: UiStyleProperty) -> bool {
        self.tracks.contains_key(&property)
    }

    pub fn active_properties(&self) -> impl Iterator<Item = UiStyleProperty> + '_ {
        self.tracks.keys().copied()
    }
}

/// Captured pre-cascade layout values. This lets state/theme changes remove a
/// previous declaration without permanently overwriting caller components.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiBaseLayoutNode(pub UiLayoutNode);

#[derive(Component, Debug, Clone, PartialEq)]
struct UiBaseBoxShadow(Option<BoxShadow>);

pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Theme>()
            .init_resource::<UiDirty>()
            .init_resource::<UiReducedMotion>()
            .init_resource::<UiStyleSheet>()
            .init_resource::<UiStyleResolutionCache>()
            .init_resource::<UiStyleCollectorCursor>()
            .init_resource::<UiStyleResourceCursor>()
            .init_resource::<UiRoutedEvents>()
            .add_systems(
                Update,
                (
                    advance_ui_style_transitions,
                    collect_style_component_changes,
                    collect_style_resource_changes,
                    resolve_ui_styles_dirty,
                )
                    .chain()
                    .in_set(UiSystems::Style),
            )
            .add_systems(
                Update,
                (
                    collect_style_component_changes,
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
            UiTransitions,
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
    input_stamp: Option<u64>,
    focus_targets: HashMap<Entity, u8>,
    resolved_passes: u64,
    skipped_passes: u64,
}

#[derive(Resource, Debug, Default)]
struct UiStyleCollectorCursor(Tick);

#[derive(Resource, Debug, Default)]
struct UiStyleResourceCursor(Tick);

mod collect;
use collect::*;

mod resolution;
pub use resolution::resolve_ui_styles;
use resolution::*;

mod transitions;
pub use transitions::advance_ui_style_transitions;
use transitions::*;

mod resolve;
pub use resolve::resolve_style;

mod layout_apply;
use layout_apply::*;

mod math;
use math::*;

#[cfg(test)]
#[path = "style/tests.rs"]
mod tests;

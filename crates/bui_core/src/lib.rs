//! Shared UI identity, hierarchy, input routing, and interaction state.

use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::{lifecycle::Remove, observer::On};

pub mod geometry;
pub mod history;
pub mod input;
pub mod invalidation;
pub mod search;

pub use geometry::UiRect;
pub use history::{TextSelection, UiEditContext, UiHistory, UiHistoryRestore, UiValue};
pub use input::{
    UiInputEvent, UiInputQueue, UiKeyCode, UiModifiers, UiPointerButton, UiRoutedEvent,
    UiRoutedEvents, UiWheelUnit, process_ui_input, refresh_ui_hover,
};
pub use invalidation::{UiDirty, UiDirtyBatch, UiDirtyDomain, ui_components_changed_since};

/// Ordered phases for BUI's frame work. Plugins should put their systems in
/// these sets to make layout, input, widget updates, and rendering agree.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSystems {
    Style,
    /// Content measurement after style resolution and before layout.
    TextMeasure,
    Layout,
    Input,
    /// Geometry updates caused by routed input, before hover styling and text
    /// layout consume the frame's bounds.
    LayoutAfterInput,
    StyleAfterInput,
    /// Repeat layout only when hover styling changes geometry, then synchronize
    /// stationary-pointer hover before text layout and widget paint.
    LayoutAfterStyle,
    Text,
    Widgets,
    Render,
}

/// Marks an entity as part of the BUI hierarchy.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiNode;

/// A stable semantic identity for a control, independent of its entity ID.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
#[component(summary_tick)]
pub struct UiKey(pub String);

/// Controls whether a widget participates in interaction and layout.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiEnabled(pub bool);

impl Default for UiEnabled {
    fn default() -> Self {
        Self(true)
    }
}

/// Controls whether a node and its subtree are shown and hit-testable.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiVisible(pub bool);

impl Default for UiVisible {
    fn default() -> Self {
        Self(true)
    }
}

/// How a change in a live region should be announced to assistive technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiLivePoliteness {
    /// Announce the change at the next convenient opportunity.
    #[default]
    Polite,
    /// Announce the change as soon as possible, interrupting current speech.
    Assertive,
}

/// Marks an accessible node as a live region whose text or value can change.
///
/// The host accessibility bridge maps this to the corresponding AccessKit
/// live-region properties. `atomic` asks assistive technology to announce the
/// whole region value when a descendant changes, rather than only the changed
/// text.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiLiveRegion {
    pub politeness: UiLivePoliteness,
    pub atomic: bool,
}

/// Accessible name for a control whose visible caption is a separate entity.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiAccessibleName(pub String);

/// Marks the container that owns a set of selectable tabs.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiTabList;

/// Semantic selection state for a tab control.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiTab {
    pub selected: bool,
}

impl UiLiveRegion {
    pub const fn polite() -> Self {
        Self {
            politeness: UiLivePoliteness::Polite,
            atomic: false,
        }
    }

    pub const fn assertive() -> Self {
        Self {
            politeness: UiLivePoliteness::Assertive,
            atomic: false,
        }
    }

    pub const fn atomic(mut self) -> Self {
        self.atomic = true;
        self
    }
}

impl Default for UiLiveRegion {
    fn default() -> Self {
        Self::polite()
    }
}

/// Marks an enabled, visible node as eligible for keyboard focus.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiFocusable;

/// Optional keyboard traversal order. Negative indices opt out of Tab traversal.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiTabIndex(pub i32);

/// Explicit paint order for nodes that otherwise share layout depth.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
#[component(summary_tick)]
pub struct UiZIndex(pub i32);

/// The parent link for a node in the BUI-owned logical hierarchy.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiParent(pub Entity);

/// Children in stable insertion order. Keep this in sync using [`set_parent`].
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiChildren(pub Vec<Entity>);

/// Bounds used by pointer routing. Layout writes this from its shared result.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[component(summary_tick)]
pub struct UiHitBox {
    /// Bevy window or offscreen camera entity. `None` accepts events from any surface.
    pub window: Option<Entity>,
    pub rect: UiRect,
    pub clip: Option<UiRect>,
    pub z_index: i32,
}

impl UiHitBox {
    pub fn contains(self, position: [f32; 2]) -> bool {
        self.rect.contains(position[0], position[1])
            && self
                .clip
                .is_none_or(|clip| clip.contains(position[0], position[1]))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiSelectionSnapshot {
    pub anchor: usize,
    pub active: usize,
}

/// Focus, hover, capture, and pointer state for one Bevy window.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiFocusState {
    pub focused: Option<Entity>,
    pub hovered: Option<Entity>,
    pub captured: Option<Entity>,
    /// The button that owns the current pointer capture, if any.
    pub captured_button: Option<UiPointerButton>,
    pub pointer_position: Option<[f32; 2]>,
    pub press_position: Option<[f32; 2]>,
    pub selection: Option<UiSelectionSnapshot>,
    pub last_click: Option<UiClickRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiClickRecord {
    pub target: Entity,
    pub at_seconds: f64,
    pub position: [f32; 2],
    pub count: u32,
}

/// Per-window interaction state. The three shorthand fields mirror the last
/// window touched for compatibility with single-window apps.
#[derive(Resource, Debug, Default)]
pub struct UiFocus {
    pub focused: Option<Entity>,
    pub hovered: Option<Entity>,
    pub captured: Option<Entity>,
    pub active_window: Option<Entity>,
    pub windows: HashMap<Entity, UiFocusState>,
}

impl UiFocus {
    pub fn state(&self, window: Entity) -> Option<&UiFocusState> {
        self.windows.get(&window)
    }

    pub fn state_mut(&mut self, window: Entity) -> &mut UiFocusState {
        self.active_window = Some(window);
        self.windows.entry(window).or_default()
    }

    pub fn set_active(&mut self, window: Entity) {
        self.active_window = Some(window);
        self.sync_active();
    }

    pub(crate) fn sync_active(&mut self) {
        let active = self.active_window.and_then(|window| {
            self.windows
                .get(&window)
                .map(|state| (state.focused, state.hovered, state.captured))
        });
        (self.focused, self.hovered, self.captured) = active.unwrap_or_default();
    }

    pub fn active_window(&self) -> Option<Entity> {
        self.active_window
    }

    pub fn clear_entity(&mut self, entity: Entity) {
        for state in self.windows.values_mut() {
            if state.focused == Some(entity) {
                state.focused = None;
                state.selection = None;
            }
            if state.hovered == Some(entity) {
                state.hovered = None;
            }
            if state.captured == Some(entity) {
                state.captured = None;
                state.captured_button = None;
                state.press_position = None;
            }
            if state.last_click.is_some_and(|last| last.target == entity) {
                state.last_click = None;
            }
        }
        self.sync_active();
    }
}

/// Routed lifecycle events waiting for the next input pass. Despawn/cleanup
/// can happen after input routing; keeping these separate prevents the next
/// routing pass from clearing cancellation events before widgets consume them.
#[derive(Resource, Debug, Default)]
pub(crate) struct UiLifecycleEvents(pub(crate) Vec<UiRoutedEvent>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiHierarchyError {
    MissingChild,
    MissingParent,
    SelfParent,
    Cycle,
}

/// Attach `child` beneath `parent`, preserving deterministic child order.
pub fn set_parent(
    world: &mut World,
    child: Entity,
    parent: Entity,
) -> Result<(), UiHierarchyError> {
    if world.get::<UiNode>(child).is_none() {
        return Err(UiHierarchyError::MissingChild);
    }
    if world.get::<UiNode>(parent).is_none() {
        return Err(UiHierarchyError::MissingParent);
    }
    if child == parent {
        return Err(UiHierarchyError::SelfParent);
    }

    let mut ancestor = Some(parent);
    while let Some(entity) = ancestor {
        if entity == child {
            return Err(UiHierarchyError::Cycle);
        }
        ancestor = world
            .get::<UiParent>(entity)
            .map(|link| link.0)
            .or_else(|| world.get::<ChildOf>(entity).map(ChildOf::parent));
    }

    if world
        .get::<UiParent>(child)
        .is_some_and(|current| current.0 == parent)
    {
        world.entity_mut(child).insert(ChildOf(parent));
        return Ok(());
    }
    detach_from_parent(world, child);
    world
        .entity_mut(child)
        .insert((UiParent(parent), ChildOf(parent)));
    let mut parent_entity = world.entity_mut(parent);
    if !parent_entity.contains::<UiChildren>() {
        parent_entity.insert(UiChildren::default());
    }
    if let Some(mut children) = parent_entity.get_mut::<UiChildren>()
        && !children.0.contains(&child)
    {
        children.0.push(child);
    }
    Ok(())
}

/// Detach a node from its logical parent, if it has one.
pub fn detach_from_parent(world: &mut World, child: Entity) -> Option<Entity> {
    let linked_parent = world
        .get::<UiParent>(child)
        .map(|link| link.0)
        .or_else(|| world.get::<ChildOf>(child).map(ChildOf::parent));
    let parents = if let Some(parent) = linked_parent {
        vec![parent]
    } else {
        let mut query = world.query::<(Entity, &UiChildren)>();
        query
            .iter(world)
            .filter_map(|(parent, children)| children.0.contains(&child).then_some(parent))
            .collect()
    };
    for parent in &parents {
        if let Some(mut children) = world.get_mut::<UiChildren>(*parent) {
            children.0.retain(|entity| *entity != child);
        }
    }
    if let Ok(mut entity) = world.get_entity_mut(child) {
        entity.remove::<UiParent>();
        entity.remove::<ChildOf>();
    }
    linked_parent.or_else(|| parents.first().copied())
}

/// Detach children and clear interaction state before despawning a node. Call
/// this before `World::despawn` when child entities must outlive their parent;
/// native Bevy `ChildOf` otherwise uses Bevy's normal recursive despawn rule.
pub fn cleanup_removed_node(world: &mut World, entity: Entity) {
    detach_from_parent(world, entity);
    let children = world
        .get::<UiChildren>(entity)
        .map(|children| children.0.clone())
        .unwrap_or_default();
    for child in children {
        if (world
            .get::<UiParent>(child)
            .is_some_and(|parent| parent.0 == entity)
            || world
                .get::<ChildOf>(child)
                .is_some_and(|parent| parent.parent() == entity))
            && let Ok(mut child) = world.get_entity_mut(child)
        {
            child.remove::<UiParent>();
            child.remove::<ChildOf>();
        }
    }
    if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
        entity_mut.remove::<UiChildren>();
    }
    let cancellations = if let Some(mut focus) = world.get_resource_mut::<UiFocus>() {
        let mut events = Vec::new();
        for (window, state) in &mut focus.windows {
            if state.captured == Some(entity) {
                let button = state
                    .captured_button
                    .take()
                    .unwrap_or(UiPointerButton::Primary);
                let position = state.pointer_position.unwrap_or([0.0, 0.0]);
                events.push(UiRoutedEvent::PointerCancel {
                    window: *window,
                    target: entity,
                });
                events.push(UiRoutedEvent::PointerUp {
                    window: *window,
                    target: entity,
                    button,
                    position,
                    cancelled: true,
                });
            }
            if state.hovered == Some(entity) {
                events.push(UiRoutedEvent::PointerLeave {
                    window: *window,
                    target: entity,
                });
            }
            if state.focused == Some(entity) {
                events.push(UiRoutedEvent::ImeDisabled {
                    window: *window,
                    target: entity,
                });
                events.push(UiRoutedEvent::FocusChanged {
                    window: *window,
                    previous: Some(entity),
                    current: None,
                });
            }
        }
        focus.clear_entity(entity);
        events
    } else {
        Vec::new()
    };
    if !cancellations.is_empty() {
        world.init_resource::<UiLifecycleEvents>();
        world
            .resource_mut::<UiLifecycleEvents>()
            .0
            .extend(cancellations);
    }
}

/// Whether a node can currently be interacted with, including ancestor state.
pub fn is_interactable(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut seen = std::collections::HashSet::new();
    while let Some(at) = current {
        if !seen.insert(at) || world.get::<UiNode>(at).is_none() {
            return false;
        }
        if world.get::<UiEnabled>(at).is_some_and(|state| !state.0)
            || world.get::<UiVisible>(at).is_some_and(|state| !state.0)
        {
            return false;
        }
        let parent = world
            .get::<UiParent>(at)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(at).map(ChildOf::parent));
        current = parent.filter(|parent| world.get::<UiNode>(*parent).is_some());
    }
    true
}

/// Whether an interactable node belongs to the given input surface. A node
/// without a window-tagged hit box in its ancestry is treated as global.
pub fn is_interactable_in_window(world: &World, entity: Entity, window: Entity) -> bool {
    if !is_interactable(world, entity) {
        return false;
    }
    let mut current = Some(entity);
    let mut seen = std::collections::HashSet::new();
    while let Some(at) = current {
        if !seen.insert(at) {
            return false;
        }
        if let Some(owner) = world.get::<UiHitBox>(at).and_then(|bounds| bounds.window) {
            return owner == window;
        }
        current = world
            .get::<UiParent>(at)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(at).map(ChildOf::parent))
            .filter(|parent| world.get::<UiNode>(*parent).is_some());
    }
    true
}

/// Whether an interactable node has surface geometry that permits keyboard
/// traversal in `window`. An explicit `window: None` hit box is global; a node
/// with no hit box anywhere in its ancestry is not assigned to a tab sequence.
pub(crate) fn has_window_assignment(world: &World, entity: Entity, window: Entity) -> bool {
    if !is_interactable(world, entity) {
        return false;
    }
    let mut current = Some(entity);
    let mut seen = std::collections::HashSet::new();
    let mut has_global_bounds = false;
    while let Some(at) = current {
        if !seen.insert(at) {
            return false;
        }
        if let Some(bounds) = world.get::<UiHitBox>(at) {
            if let Some(owner) = bounds.window {
                return owner == window;
            }
            has_global_bounds = true;
        }
        current = world
            .get::<UiParent>(at)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(at).map(ChildOf::parent))
            .filter(|parent| world.get::<UiNode>(*parent).is_some());
    }
    has_global_bounds
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFocus>()
            .init_resource::<UiDirty>()
            .init_resource::<UiInputQueue>()
            .init_resource::<UiRoutedEvents>()
            .init_resource::<UiLifecycleEvents>()
            .configure_sets(
                Update,
                (
                    UiSystems::Style,
                    UiSystems::TextMeasure,
                    UiSystems::Layout,
                    UiSystems::Input,
                    UiSystems::LayoutAfterInput,
                    UiSystems::StyleAfterInput,
                    UiSystems::LayoutAfterStyle,
                    UiSystems::Text,
                    UiSystems::Widgets,
                    UiSystems::Render,
                )
                    .chain(),
            )
            .init_resource::<UiHistory>()
            .init_resource::<input::UiHoverValidation>()
            .add_systems(
                Update,
                (input::collect_hit_test_changes, process_ui_input)
                    .chain()
                    .in_set(UiSystems::Input),
            );
        macro_rules! register_hit_test_removals {
            ($($component:ty),+ $(,)?) => {
                $(app.add_observer(mark_hit_test_component_removed::<$component>);)+
            };
        }
        register_hit_test_removals!(UiNode, UiChildren);
        app.add_observer(mark_hit_test_subtree_removed::<UiEnabled>);
        app.add_observer(mark_hit_test_subtree_removed::<UiVisible>);
        app.add_observer(mark_hit_test_subtree_removed::<UiParent>);
        app.add_observer(mark_hit_box_removed);
    }
}

fn mark_hit_test_component_removed<T: Component>(_on: On<Remove<T>>, mut dirty: ResMut<UiDirty>) {
    dirty.mark_all(UiDirtyDomain::HitTest);
}

fn mark_hit_test_subtree_removed<T: Component>(
    on: On<Remove<T>>,
    children: Query<&UiChildren>,
    mut dirty: ResMut<UiDirty>,
) {
    let mut pending = vec![on.entity];
    let mut visited = std::collections::HashSet::new();
    while let Some(entity) = pending.pop() {
        if !visited.insert(entity) {
            continue;
        }
        dirty.mark(UiDirtyDomain::HitTest, entity);
        if let Ok(children) = children.get(entity) {
            pending.extend(children.0.iter().copied());
        }
    }
}

fn mark_hit_box_removed(on: On<Remove<UiHitBox>>, mut dirty: ResMut<UiDirty>) {
    dirty.mark(UiDirtyDomain::HitTest, on.entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_reparenting_preserves_order_and_rejects_cycles() {
        use bevy_ecs::hierarchy::Children as BevyChildren;

        let mut world = World::new();
        let root = world.spawn(UiNode).id();
        let child = world.spawn(UiNode).id();
        let leaf = world.spawn(UiNode).id();
        set_parent(&mut world, child, root).unwrap();
        set_parent(&mut world, leaf, child).unwrap();
        assert_eq!(world.get::<UiChildren>(root).unwrap().0, [child]);
        assert_eq!(world.get::<ChildOf>(child).unwrap().parent(), root);
        assert_eq!(world.get::<BevyChildren>(root).unwrap().len(), 1);
        assert_eq!(
            set_parent(&mut world, root, leaf),
            Err(UiHierarchyError::Cycle)
        );
        let other = world.spawn(UiNode).id();
        set_parent(&mut world, child, other).unwrap();
        assert!(world.get::<UiChildren>(root).unwrap().0.is_empty());
        assert!(world.get::<BevyChildren>(root).is_none());
        assert_eq!(world.get::<UiParent>(child).unwrap().0, other);
        assert_eq!(world.get::<ChildOf>(child).unwrap().parent(), other);
    }

    #[test]
    fn visibility_and_enabled_state_inherit_through_hierarchy() {
        let mut world = World::new();
        let root = world.spawn((UiNode, UiVisible(false))).id();
        let child = world.spawn((UiNode, UiFocusable)).id();
        set_parent(&mut world, child, root).unwrap();
        assert!(!is_interactable(&world, child));
        world.entity_mut(root).insert(UiVisible(true));
        assert!(is_interactable(&world, child));
    }

    #[test]
    fn cleanup_after_despawn_removes_stale_parent_child_links() {
        let mut world = World::new();
        let parent = world.spawn(UiNode).id();
        let child = world.spawn(UiNode).id();
        set_parent(&mut world, child, parent).unwrap();
        world.despawn(child);
        cleanup_removed_node(&mut world, child);
        assert!(world.get::<UiChildren>(parent).unwrap().0.is_empty());
    }

    #[test]
    fn cleanup_before_parent_despawn_preserves_bui_owned_children() {
        let mut world = World::new();
        let parent = world.spawn(UiNode).id();
        let child = world.spawn(UiNode).id();
        set_parent(&mut world, child, parent).unwrap();
        cleanup_removed_node(&mut world, parent);
        world.despawn(parent);
        assert!(world.get::<UiNode>(child).is_some());
        assert!(world.get::<UiParent>(child).is_none());
        assert!(world.get::<ChildOf>(child).is_none());
    }

    #[test]
    fn live_region_constructors_preserve_politeness_and_atomicity() {
        assert_eq!(UiLiveRegion::default(), UiLiveRegion::polite());
        assert_eq!(
            UiLiveRegion::assertive(),
            UiLiveRegion {
                politeness: UiLivePoliteness::Assertive,
                atomic: false,
            }
        );
        assert!(UiLiveRegion::polite().atomic().atomic);
    }
}

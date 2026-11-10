//! Collapsible panel state and grouped expansion rules.

use bevy_app::{App, Update};
use bevy_ecs::prelude::{Component, Entity, Resource, World};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bui_core::{UiChildren, UiKey, UiSystems};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelState {
    pub expanded: bool,
    pub target_expanded: bool,
    /// Animated visibility in the inclusive range 0..=1.
    pub progress: f32,
    pub animation_seconds: f32,
}

impl PanelState {
    pub fn new(expanded: bool) -> Self {
        Self {
            expanded,
            target_expanded: expanded,
            progress: if expanded { 1.0 } else { 0.0 },
            animation_seconds: 0.16,
        }
    }

    pub fn set_expanded(&mut self, expanded: bool) -> bool {
        if self.target_expanded == expanded {
            return false;
        }
        self.expanded = expanded;
        self.target_expanded = expanded;
        true
    }

    pub fn toggle(&mut self) -> bool {
        self.set_expanded(!self.target_expanded)
    }

    /// Move the visible state toward its target; returns true while another frame is needed.
    pub fn advance(&mut self, delta_seconds: f32) -> bool {
        if !delta_seconds.is_finite() || delta_seconds <= 0.0 || self.animation_seconds <= 0.0 {
            self.progress = if self.target_expanded { 1.0 } else { 0.0 };
        } else {
            let distance = (delta_seconds / self.animation_seconds).min(1.0);
            if self.target_expanded {
                self.progress = (self.progress + distance).min(1.0);
            } else {
                self.progress = (self.progress - distance).max(0.0);
            }
        }
        (self.progress - if self.target_expanded { 1.0 } else { 0.0 }).abs() > f32::EPSILON
    }

    /// Advance using the shared theme motion scale and reduced-motion setting.
    /// A scale of `1.0` uses `animation_seconds`; lower positive values finish
    /// sooner, and higher values take longer. Reduced motion or a zero/invalid
    /// scale moves directly to the target and stops requesting animation frames.
    pub fn advance_with_motion(
        &mut self,
        delta_seconds: f32,
        motion_scale: f32,
        reduced_motion: bool,
    ) -> bool {
        if reduced_motion
            || !motion_scale.is_finite()
            || motion_scale <= 0.0
            || !self.animation_seconds.is_finite()
            || self.animation_seconds <= 0.0
        {
            self.progress = if self.target_expanded { 1.0 } else { 0.0 };
            return false;
        }
        if !delta_seconds.is_finite() || delta_seconds <= 0.0 {
            return self.is_animating();
        }

        let target = if self.target_expanded { 1.0 } else { 0.0 };
        if !self.progress.is_finite() {
            self.progress = 1.0 - target;
        } else {
            self.progress = self.progress.clamp(0.0, 1.0);
        }
        let distance = ((delta_seconds / self.animation_seconds) / motion_scale).min(1.0);
        if self.target_expanded {
            self.progress = (self.progress + distance).min(1.0);
        } else {
            self.progress = (self.progress - distance).max(0.0);
        }
        self.is_animating()
    }

    fn is_animating(&self) -> bool {
        (self.progress - if self.target_expanded { 1.0 } else { 0.0 }).abs() > f32::EPSILON
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
#[component(summary_tick)]
pub struct Panel {
    pub title: String,
    pub state: PanelState,
    pub enabled: bool,
    pub exclusive: bool,
    pub persistence_key: Option<String>,
}

impl Panel {
    pub fn new(title: impl Into<String>, expanded: bool) -> Self {
        Self {
            title: title.into(),
            state: PanelState::new(expanded),
            enabled: true,
            exclusive: false,
            persistence_key: None,
        }
    }

    pub fn toggle(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        self.state.toggle()
    }

    pub fn set_expanded(&mut self, expanded: bool) -> bool {
        if !self.enabled {
            return false;
        }
        self.state.set_expanded(expanded)
    }

    /// Ctrl-click expands this panel and collapses the other panels in its group.
    pub fn ctrl_click(&mut self, group: &mut PanelGroup) {
        if !self.enabled {
            return;
        }
        self.state.set_expanded(true);
        group.open_exclusive(self.id_key());
    }

    fn id_key(&self) -> String {
        self.persistence_key
            .clone()
            .unwrap_or_else(|| self.title.clone())
    }
}

#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct PanelGroup {
    pub panel_keys: Vec<String>,
    pub exclusive_open: Option<String>,
    /// Stable storage key for order persistence and history rebinding.
    pub persistence_key: Option<String>,
    /// Enables pointer and keyboard reordering of this group's direct Panel children.
    pub reorderable: bool,
}

impl PanelGroup {
    pub fn with_panels(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            panel_keys: keys.into_iter().map(Into::into).collect(),
            exclusive_open: None,
            persistence_key: None,
            reorderable: false,
        }
    }

    /// Opt into reordering and persist the order under a stable application key.
    pub fn with_reordering(mut self, persistence_key: impl Into<String>) -> Self {
        self.reorderable = true;
        self.persistence_key = Some(persistence_key.into());
        self
    }

    pub fn open_exclusive(&mut self, key: impl Into<String>) {
        let key = key.into();
        if self.panel_keys.iter().any(|candidate| candidate == &key) {
            self.exclusive_open = Some(key);
        }
    }

    pub fn apply(&self, panels: &mut [Panel]) {
        if let Some(key) = &self.exclusive_open {
            for panel in panels {
                let panel_key = panel.persistence_key.as_deref().unwrap_or(&panel.title);
                if panel.exclusive
                    && self
                        .panel_keys
                        .iter()
                        .any(|candidate| candidate == panel_key)
                {
                    panel.state.set_expanded(panel_key == key);
                }
            }
        }
    }
}

/// Optional persistence store shared by rebuilt widget trees.
#[derive(Resource, Debug, Default, Clone)]
pub struct PanelExpansionStore(pub HashMap<String, bool>);

impl PanelExpansionStore {
    pub fn remember(&mut self, panel: &Panel) {
        if let Some(key) = &panel.persistence_key {
            self.remember_key(key, panel.state.target_expanded);
        }
    }

    pub fn restore(&self, panel: &mut Panel) -> bool {
        let Some(key) = panel.persistence_key.clone() else {
            return false;
        };
        self.restore_key(&key, panel)
    }

    /// Record expansion state under a stable application supplied key.
    pub fn remember_key(&mut self, key: impl Into<String>, expanded: bool) {
        self.0.insert(key.into(), expanded);
    }

    /// Restore expansion state by key, returning whether a stored value existed.
    pub fn restore_key(&self, key: &str, panel: &mut Panel) -> bool {
        let Some(expanded) = self.0.get(key) else {
            return false;
        };
        panel.state = PanelState::new(*expanded);
        true
    }

    /// Read a remembered expansion value.
    pub fn get(&self, key: &str) -> Option<bool> {
        self.0.get(key).copied()
    }
}

/// Reusable keyed panel orders survive panel-tree reconstruction.
#[derive(Resource, Debug, Default, Clone)]
pub struct PanelOrderStore(pub HashMap<String, Vec<String>>);

impl PanelOrderStore {
    pub fn remember(&mut self, key: impl Into<String>, order: Vec<String>) {
        self.0.insert(key.into(), order);
    }

    pub fn get(&self, key: &str) -> Option<&[String]> {
        self.0.get(key).map(Vec::as_slice)
    }
}

#[derive(Resource, Debug, Default)]
struct PanelPersistenceBindings(HashMap<Entity, (String, bool)>);

#[derive(Resource, Debug, Default)]
struct PanelOrderBindings(HashMap<Entity, (String, Vec<String>)>);

/// Register keyed panel restoration before input and persistence after actions.
pub(crate) fn install_panel_persistence(app: &mut App) {
    app.init_resource::<PanelExpansionStore>()
        .init_resource::<PanelPersistenceBindings>()
        .init_resource::<PanelOrderStore>()
        .init_resource::<PanelOrderBindings>()
        .add_systems(
            Update,
            (restore_panel_expansion, restore_panel_group_order)
                .before(crate::actions::process_routed_widget_events)
                .in_set(UiSystems::Widgets),
        )
        .add_systems(
            Update,
            (persist_panel_expansion, persist_panel_group_order)
                .after(crate::actions::apply_widget_actions)
                .in_set(UiSystems::Widgets),
        );
}

/// Validate that `order` is a permutation of the current reorderable panel children.
pub(crate) fn validate_panel_order(
    world: &World,
    group_entity: Entity,
    order: &[String],
) -> Result<(), String> {
    let Some(group) = world.get::<PanelGroup>(group_entity) else {
        return Err("panel order group no longer exists".into());
    };
    if !group.reorderable {
        return Err("panel group is not reorderable".into());
    }
    let Some(persistence_key) = panel_group_key(world, group_entity) else {
        return Err("panel group has no stable persistence key".into());
    };
    if world.iter_entities().any(|entity| {
        entity.id() != group_entity
            && entity.get::<PanelGroup>().is_some_and(|other| {
                other.reorderable
                    && panel_group_key(world, entity.id()).as_deref()
                        == Some(persistence_key.as_str())
            })
    }) {
        return Err("reorderable panel group persistence keys must be unique".into());
    }
    let Some((children, panel_entities)) = keyed_panel_children(world, group_entity) else {
        return Err("panel children require unique stable keys".into());
    };
    let expected: HashSet<_> = group.panel_keys.iter().collect();
    let requested: HashSet<_> = order.iter().collect();
    let actual: HashSet<_> = children.keys().collect();
    if expected.len() != group.panel_keys.len()
        || requested.len() != order.len()
        || expected != requested
        || expected != actual
        || panel_entities.len() != group.panel_keys.len()
    {
        return Err("panel order snapshot does not match the live panel key set".into());
    }
    if world.get::<UiChildren>(group_entity).is_none() {
        return Err("panel group has no child list".into());
    }
    Ok(())
}

/// Restore an order snapshot without emitting a widget event.
///
/// On success, both the logical child order and persistence store are updated. The
/// snapshot must match the current direct child panel keys exactly.
pub(crate) fn restore_panel_order(
    world: &mut World,
    group_entity: Entity,
    order: &[String],
) -> bool {
    if validate_panel_order(world, group_entity, order).is_err() {
        return false;
    }
    apply_panel_order(world, group_entity, order, true)
}

pub(crate) fn preview_panel_order(
    world: &mut World,
    group_entity: Entity,
    order: &[String],
) -> bool {
    apply_panel_order(world, group_entity, order, false)
}

fn apply_panel_order(
    world: &mut World,
    group_entity: Entity,
    order: &[String],
    persist: bool,
) -> bool {
    if validate_panel_order(world, group_entity, order).is_err() {
        return false;
    }
    let Some((entities_by_key, _)) = keyed_panel_children(world, group_entity) else {
        return false;
    };
    let Some(mut children) = world.get::<UiChildren>(group_entity).cloned() else {
        return false;
    };
    let positions: Vec<_> = children
        .0
        .iter()
        .enumerate()
        .filter_map(|(index, child)| world.get::<Panel>(*child).map(|_| index))
        .collect();
    if positions.len() != order.len() {
        return false;
    }
    let mut desired = Vec::with_capacity(order.len());
    for key in order {
        let Some(entity) = entities_by_key.get(key) else {
            return false;
        };
        desired.push(*entity);
    }
    for (position, entity) in positions.into_iter().zip(desired) {
        children.0[position] = entity;
    }

    if let Some(mut current_group) = world.get_mut::<PanelGroup>(group_entity) {
        current_group.panel_keys = order.to_vec();
    }
    world.entity_mut(group_entity).insert(children);
    if persist && let Some(key) = panel_group_key(world, group_entity) {
        if let Some(mut store) = world.get_resource_mut::<PanelOrderStore>() {
            store.remember(key.clone(), order.to_vec());
        }
        if let Some(mut bindings) = world.get_resource_mut::<PanelOrderBindings>() {
            bindings.0.insert(group_entity, (key, order.to_vec()));
        }
    }
    true
}

fn keyed_panel_children(
    world: &World,
    group_entity: Entity,
) -> Option<(HashMap<String, Entity>, Vec<Entity>)> {
    let children = world.get::<UiChildren>(group_entity)?;
    let mut keyed = HashMap::new();
    let mut panels = Vec::new();
    for child in &children.0 {
        let Some(panel) = world.get::<Panel>(*child) else {
            continue;
        };
        let key = panel
            .persistence_key
            .as_deref()
            .or_else(|| world.get::<UiKey>(*child).map(|key| key.0.as_str()))?;
        if keyed.insert(key.to_owned(), *child).is_some() {
            return None;
        }
        panels.push(*child);
    }
    Some((keyed, panels))
}

fn panel_group_key(world: &World, group_entity: Entity) -> Option<String> {
    let group = world.get::<PanelGroup>(group_entity)?;
    group
        .persistence_key
        .clone()
        .or_else(|| world.get::<UiKey>(group_entity).map(|key| key.0.clone()))
}

/// Restore reorderable groups from the keyed store before widget input is applied.
pub fn restore_panel_group_order(world: &mut World) {
    let groups: Vec<_> = {
        let mut query = world.query::<(Entity, &PanelGroup)>();
        query
            .iter(world)
            .filter(|(_, group)| group.reorderable)
            .filter_map(|(entity, _)| panel_group_key(world, entity).map(|key| (entity, key)))
            .collect()
    };
    let active: HashSet<_> = groups.iter().map(|(entity, _)| *entity).collect();
    if let Some(mut bindings) = world.get_resource_mut::<PanelOrderBindings>() {
        bindings.0.retain(|entity, _| active.contains(entity));
    }
    for (entity, key) in groups {
        if world.get::<UiKey>(entity).map(|key| key.0.as_str()) != Some(key.as_str()) {
            world.entity_mut(entity).insert(UiKey(key.clone()));
        }
        let prior_key = world
            .get_resource::<PanelOrderBindings>()
            .and_then(|bindings| bindings.0.get(&entity))
            .map(|(prior, _)| prior.as_str());
        if prior_key == Some(key.as_str()) {
            continue;
        }
        let Some(order) = world
            .get::<PanelGroup>(entity)
            .map(|group| group.panel_keys.clone())
        else {
            continue;
        };
        if validate_panel_order(world, entity, &order).is_err() {
            continue;
        }
        let saved = world
            .get_resource::<PanelOrderStore>()
            .and_then(|store| store.get(&key))
            .map(|order| order.to_vec());
        match saved {
            Some(saved) if !apply_panel_order(world, entity, &saved, false) => continue,
            Some(_) => {}
            None => {
                if let Some(order) = world
                    .get::<PanelGroup>(entity)
                    .map(|group| group.panel_keys.clone())
                    && let Some(mut store) = world.get_resource_mut::<PanelOrderStore>()
                {
                    store.remember(key.clone(), order);
                }
            }
        }
        if let Some(order) = world
            .get::<PanelGroup>(entity)
            .map(|group| group.panel_keys.clone())
            && let Some(mut bindings) = world.get_resource_mut::<PanelOrderBindings>()
        {
            bindings.0.insert(entity, (key, order));
        }
    }
}

/// Persist reorderable group order after widget actions and pointer gestures.
pub fn persist_panel_group_order(world: &mut World) {
    let groups: Vec<_> = {
        let mut query = world.query::<(Entity, &PanelGroup)>();
        query
            .iter(world)
            .filter(|(_, group)| group.reorderable)
            .filter_map(|(entity, group)| {
                panel_group_key(world, entity).map(|key| (entity, key, group.panel_keys.clone()))
            })
            .collect()
    };
    for (entity, key, order) in groups {
        if validate_panel_order(world, entity, &order).is_err() {
            continue;
        }
        let prior = world
            .get_resource::<PanelOrderBindings>()
            .and_then(|bindings| bindings.0.get(&entity));
        if prior.is_some_and(|(prior_key, prior_order)| prior_key == &key && prior_order == &order)
        {
            continue;
        }
        if let Some(mut store) = world.get_resource_mut::<PanelOrderStore>() {
            store.remember(key.clone(), order.clone());
        }
        if let Some(mut bindings) = world.get_resource_mut::<PanelOrderBindings>() {
            bindings.0.insert(entity, (key, order));
        }
    }
}

/// Restore a panel once when its stable key first appears or changes.
pub fn restore_panel_expansion(world: &mut World) {
    let panels: Vec<_> = {
        let mut query = world.query::<(Entity, &Panel, Option<&UiKey>)>();
        query
            .iter(world)
            .filter_map(|(entity, panel, ui_key)| {
                let key = panel
                    .persistence_key
                    .as_deref()
                    .or_else(|| ui_key.map(|key| key.0.as_str()))?;
                Some((entity, key.to_owned(), panel.state.target_expanded))
            })
            .collect()
    };
    let active: HashSet<_> = panels.iter().map(|(entity, _, _)| *entity).collect();
    if let Some(mut bindings) = world.get_resource_mut::<PanelPersistenceBindings>() {
        bindings.0.retain(|entity, _| active.contains(entity));
    }

    for (entity, key, current) in panels {
        let bound_key = world
            .get_resource::<PanelPersistenceBindings>()
            .and_then(|bindings| bindings.0.get(&entity))
            .map(|(bound_key, _)| bound_key.as_str());
        if bound_key == Some(key.as_str()) {
            continue;
        }

        let restored = world
            .get_resource::<PanelExpansionStore>()
            .and_then(|store| store.get(&key));
        let expanded = restored.unwrap_or(current);
        if restored.is_some()
            && let Some(mut panel) = world.get_mut::<Panel>(entity)
        {
            panel.state = PanelState::new(expanded);
        }
        if restored.is_none()
            && let Some(mut store) = world.get_resource_mut::<PanelExpansionStore>()
        {
            store.remember_key(key.clone(), current);
        }
        if let Some(mut bindings) = world.get_resource_mut::<PanelPersistenceBindings>() {
            bindings.0.insert(entity, (key, expanded));
        }
    }
}

/// Persist target changes after routed events and application actions have run.
pub fn persist_panel_expansion(world: &mut World) {
    let panels: Vec<_> = {
        let mut query = world.query::<(Entity, &Panel, Option<&UiKey>)>();
        query
            .iter(world)
            .filter_map(|(entity, panel, ui_key)| {
                let key = panel
                    .persistence_key
                    .as_deref()
                    .or_else(|| ui_key.map(|key| key.0.as_str()))?;
                Some((entity, key.to_owned(), panel.state.target_expanded))
            })
            .collect()
    };

    for (entity, key, expanded) in panels {
        let prior = world
            .get_resource::<PanelPersistenceBindings>()
            .and_then(|bindings| bindings.0.get(&entity))
            .cloned();
        if prior
            .as_ref()
            .is_some_and(|(prior_key, prior_value)| prior_key == &key && *prior_value == expanded)
        {
            continue;
        }

        if let Some(mut store) = world.get_resource_mut::<PanelExpansionStore>() {
            store.remember_key(key.clone(), expanded);
        }
        if let Some(mut bindings) = world.get_resource_mut::<PanelPersistenceBindings>() {
            bindings.0.insert(entity, (key, expanded));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetPlugin;
    use bevy_app::App;
    use bui_core::{UiKeyCode, UiRoutedEvent, UiRoutedEvents};

    #[test]
    fn expansion_animates_toward_target_and_keyboard_toggle_uses_same_state() {
        let mut panel = Panel::new("Transform", false);
        assert!(panel.toggle());
        assert!(panel.state.advance(0.08));
        assert!((panel.state.progress - 0.5).abs() < 0.001);
        panel.state.advance(0.2);
        assert_eq!(panel.state.progress, 1.0);
        assert!(panel.toggle());
        panel.state.advance(1.0);
        assert_eq!(panel.state.progress, 0.0);
    }

    #[test]
    fn panel_motion_scale_changes_duration_and_reduced_motion_snaps_both_directions() {
        let mut panel = PanelState::new(false);
        panel.set_expanded(true);
        assert!(panel.advance_with_motion(0.08, 1.0, false));
        assert!((panel.progress - 0.5).abs() < 0.001);

        assert!(panel.advance_with_motion(0.08, 2.0, false));
        assert!((panel.progress - 0.75).abs() < 0.001);
        panel.set_expanded(false);
        assert!(panel.advance_with_motion(0.04, 1.0, false));
        assert!((panel.progress - 0.5).abs() < 0.001);

        assert!(!panel.advance_with_motion(0.0, 1.0, true));
        assert_eq!(panel.progress, 0.0);
        panel.set_expanded(true);
        assert!(!panel.advance_with_motion(0.08, 0.0, false));
        assert_eq!(panel.progress, 1.0);
    }

    #[test]
    fn exclusive_group_and_persistence_keep_expansion_by_key() {
        let mut first = Panel::new("First", true);
        first.exclusive = true;
        first.persistence_key = Some("first".into());
        let mut second = Panel::new("Second", false);
        second.exclusive = true;
        second.persistence_key = Some("second".into());
        let mut group = PanelGroup::with_panels(["first", "second"]);
        second.ctrl_click(&mut group);
        group.apply(&mut [first.clone(), second.clone()]);
        let mut store = PanelExpansionStore::default();
        store.remember(&second);
        second.state.set_expanded(false);
        assert!(store.restore(&mut second));
        assert!(second.state.target_expanded);
    }

    #[test]
    fn duplicate_reorder_group_keys_do_not_restore_or_overwrite_each_other() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let make_group = |world: &mut World, keys: [&str; 2]| {
            let group = world
                .spawn((
                    bui_core::UiNode,
                    UiKey("shared-group".into()),
                    PanelGroup::with_panels(keys).with_reordering("shared-order"),
                ))
                .id();
            for key in keys {
                let mut panel = Panel::new(key, false);
                panel.persistence_key = Some(key.to_owned());
                let entity = world.spawn((bui_core::UiNode, panel)).id();
                bui_core::set_parent(world, entity, group).unwrap();
            }
            group
        };

        let first = make_group(app.world_mut(), ["alpha", "beta"]);
        app.update();
        assert_eq!(
            app.world()
                .resource::<PanelOrderStore>()
                .get("shared-order"),
            Some(["alpha".to_owned(), "beta".to_owned()].as_slice())
        );

        let second = make_group(app.world_mut(), ["left", "right"]);
        app.update();
        assert_eq!(
            app.world().get::<PanelGroup>(first).unwrap().panel_keys,
            ["alpha", "beta"]
        );
        assert_eq!(
            app.world().get::<PanelGroup>(second).unwrap().panel_keys,
            ["left", "right"],
            "a conflicting group must not restore another group's snapshot"
        );
        assert_eq!(
            app.world()
                .resource::<PanelOrderStore>()
                .get("shared-order"),
            Some(["alpha".to_owned(), "beta".to_owned()].as_slice()),
            "conflicting groups must not overwrite the established order"
        );
        assert!(
            validate_panel_order(app.world(), first, &["beta".into(), "alpha".into()]).is_err()
        );
        assert!(!restore_panel_order(
            app.world_mut(),
            first,
            &["beta".into(), "alpha".into()]
        ));
    }

    #[test]
    fn routed_keyboard_toggle_persists_across_panel_rebuilds() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let window = app.world_mut().spawn_empty().id();
        let first = app
            .world_mut()
            .spawn((
                Panel::new("Transform", false),
                UiKey("transform-panel".into()),
            ))
            .id();

        app.update();
        assert_eq!(
            app.world()
                .resource::<PanelExpansionStore>()
                .get("transform-panel"),
            Some(false)
        );

        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: first,
            key: UiKeyCode::Space,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }];
        app.update();
        assert!(
            app.world()
                .get::<Panel>(first)
                .unwrap()
                .state
                .target_expanded
        );
        assert_eq!(
            app.world()
                .resource::<PanelExpansionStore>()
                .get("transform-panel"),
            Some(true)
        );

        app.world_mut().entity_mut(first).despawn();
        let rebuilt = app
            .world_mut()
            .spawn((
                Panel::new("Transform", false),
                UiKey("transform-panel".into()),
            ))
            .id();
        app.update();
        let restored = app.world().get::<Panel>(rebuilt).unwrap();
        assert!(restored.state.target_expanded);
        assert_eq!(restored.state.progress, 1.0);
    }
}

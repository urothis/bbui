//! Shared change journal for work that can be limited to affected UI entities.

use std::collections::HashSet;

use bevy_ecs::{
    change_detection::Tick, component::ComponentId, entity::Entity, resource::Resource,
    world::World,
};

/// Checks table summary ticks without visiting unchanged component instances.
/// Returns true for a tracked column without a summary tick so callers remain correct.
pub fn ui_components_changed_since(world: &World, components: &[ComponentId], last: Tick) -> bool {
    let now = world.read_change_tick();
    world.storages().tables.iter().any(|table| {
        components.iter().any(|component| {
            table.has_column(*component)
                && table
                    .get_summary_tick(*component)
                    .is_none_or(|summary| summary.get().is_newer_than(last, now))
        })
    })
}

/// A subsystem that consumes UI invalidations independently of other subsystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiDirtyDomain {
    Style,
    Layout,
    TextMeasure,
    TextLayout,
    TextSync,
    Paint,
    Render,
    HitTest,
    Accessibility,
    Preview,
}

#[derive(Debug, Default)]
struct DirtySet {
    all: bool,
    entities: HashSet<Entity>,
    generation: u64,
}

/// Changes pending for one subsystem. `all` means its complete view must be rebuilt.
#[derive(Debug, Default)]
pub struct UiDirtyBatch {
    pub all: bool,
    pub entities: HashSet<Entity>,
    pub generation: u64,
}

impl UiDirtyBatch {
    pub fn is_empty(&self) -> bool {
        !self.all && self.entities.is_empty()
    }
}

/// Central change journal. A subsystem drains only its own domain, so one
/// consumer cannot hide a change from another.
///
/// Producers should mark the entity at the point of mutation. Systems that
/// accept direct public ECS component writes also collect Bevy change ticks
/// into this journal before consuming it.
#[derive(Resource, Debug, Default)]
pub struct UiDirty {
    style: DirtySet,
    layout: DirtySet,
    text_measure: DirtySet,
    text_layout: DirtySet,
    text_sync: DirtySet,
    paint: DirtySet,
    render: DirtySet,
    hit_test: DirtySet,
    accessibility: DirtySet,
    preview: DirtySet,
}

impl UiDirty {
    fn set_mut(&mut self, domain: UiDirtyDomain) -> &mut DirtySet {
        match domain {
            UiDirtyDomain::Style => &mut self.style,
            UiDirtyDomain::Layout => &mut self.layout,
            UiDirtyDomain::TextMeasure => &mut self.text_measure,
            UiDirtyDomain::TextLayout => &mut self.text_layout,
            UiDirtyDomain::TextSync => &mut self.text_sync,
            UiDirtyDomain::Paint => &mut self.paint,
            UiDirtyDomain::Render => &mut self.render,
            UiDirtyDomain::HitTest => &mut self.hit_test,
            UiDirtyDomain::Accessibility => &mut self.accessibility,
            UiDirtyDomain::Preview => &mut self.preview,
        }
    }

    pub fn mark(&mut self, domain: UiDirtyDomain, entity: Entity) {
        let set = self.set_mut(domain);
        if !set.all && set.entities.insert(entity) {
            set.generation = set.generation.wrapping_add(1);
        }
    }

    pub fn mark_all(&mut self, domain: UiDirtyDomain) {
        let set = self.set_mut(domain);
        if !set.all {
            set.all = true;
            set.entities.clear();
            set.generation = set.generation.wrapping_add(1);
        }
    }

    pub fn generation(&self, domain: UiDirtyDomain) -> u64 {
        match domain {
            UiDirtyDomain::Style => self.style.generation,
            UiDirtyDomain::Layout => self.layout.generation,
            UiDirtyDomain::TextMeasure => self.text_measure.generation,
            UiDirtyDomain::TextLayout => self.text_layout.generation,
            UiDirtyDomain::TextSync => self.text_sync.generation,
            UiDirtyDomain::Paint => self.paint.generation,
            UiDirtyDomain::Render => self.render.generation,
            UiDirtyDomain::HitTest => self.hit_test.generation,
            UiDirtyDomain::Accessibility => self.accessibility.generation,
            UiDirtyDomain::Preview => self.preview.generation,
        }
    }

    /// Whether a domain already needs this entity or a complete refresh.
    pub fn is_marked(&self, domain: UiDirtyDomain, entity: Entity) -> bool {
        let set = match domain {
            UiDirtyDomain::Style => &self.style,
            UiDirtyDomain::Layout => &self.layout,
            UiDirtyDomain::TextMeasure => &self.text_measure,
            UiDirtyDomain::TextLayout => &self.text_layout,
            UiDirtyDomain::TextSync => &self.text_sync,
            UiDirtyDomain::Paint => &self.paint,
            UiDirtyDomain::Render => &self.render,
            UiDirtyDomain::HitTest => &self.hit_test,
            UiDirtyDomain::Accessibility => &self.accessibility,
            UiDirtyDomain::Preview => &self.preview,
        };
        set.all || set.entities.contains(&entity)
    }

    pub fn take(&mut self, domain: UiDirtyDomain) -> UiDirtyBatch {
        let set = self.set_mut(domain);
        UiDirtyBatch {
            all: std::mem::take(&mut set.all),
            entities: std::mem::take(&mut set.entities),
            generation: set.generation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumers_do_not_steal_each_others_changes() {
        let mut dirty = UiDirty::default();
        let entity = Entity::from_bits(1);
        dirty.mark(UiDirtyDomain::Style, entity);
        dirty.mark(UiDirtyDomain::Paint, entity);
        dirty.mark(UiDirtyDomain::Preview, entity);
        assert_eq!(dirty.take(UiDirtyDomain::Style).entities.len(), 1);
        assert_eq!(dirty.take(UiDirtyDomain::Paint).entities.len(), 1);
        assert_eq!(dirty.take(UiDirtyDomain::Preview).entities.len(), 1);
        assert!(dirty.take(UiDirtyDomain::Style).is_empty());
    }

    #[test]
    fn global_change_subsumes_entity_changes() {
        let mut dirty = UiDirty::default();
        dirty.mark(UiDirtyDomain::Style, Entity::from_bits(1));
        dirty.mark_all(UiDirtyDomain::Style);
        let batch = dirty.take(UiDirtyDomain::Style);
        assert!(batch.all);
        assert!(batch.entities.is_empty());
    }
}

//! Platform-neutral pointer and keyboard routing for BBUI controls.

use std::collections::{HashMap, VecDeque};

use bevy_ecs::prelude::*;
use bevy_ecs::{change_detection::Tick, component::ComponentId};

type HitTestStructuralChanged<'w, 's> = Query<
    'w,
    's,
    Entity,
    Or<(
        Changed<UiNode>,
        Changed<UiEnabled>,
        Changed<UiVisible>,
        Changed<UiParent>,
        Changed<UiChildren>,
    )>,
>;

use crate::{
    UiChildren, UiClickRecord, UiDirty, UiDirtyDomain, UiEnabled, UiFocus, UiFocusable, UiHitBox,
    UiLifecycleEvents, UiNode, UiParent, UiRect, UiTabIndex, UiVisible, has_visible_focus_geometry,
    has_window_assignment, is_interactable_in_window, ui_components_changed_since,
};

const DOUBLE_CLICK_SECONDS: f64 = 0.35;
const DOUBLE_CLICK_DISTANCE: f32 = 5.0;

#[derive(Resource, Default)]
pub(crate) struct UiHoverValidation {
    windows: HashMap<Entity, ([f32; 2], u64)>,
    prepared: bool,
    #[cfg(test)]
    hit_tests: u64,
}

#[derive(Resource, Default)]
struct UiHitTestSummaryGate {
    components: Vec<ComponentId>,
    last: Tick,
    initialized: bool,
}

pub(crate) fn collect_hit_test_changes(world: &mut World) {
    world.init_resource::<UiHitTestSummaryGate>();
    world.init_resource::<UiHoverValidation>();
    world.init_resource::<UiDirty>();
    let changed = world.resource_scope(|world, mut gate: Mut<UiHitTestSummaryGate>| {
        if !gate.initialized {
            gate.components = vec![
                world.register_component::<UiNode>(),
                world.register_component::<UiEnabled>(),
                world.register_component::<UiVisible>(),
                world.register_component::<UiParent>(),
                world.register_component::<UiChildren>(),
                world.register_component::<UiHitBox>(),
            ];
            gate.initialized = true;
        }
        let changed = ui_components_changed_since(world, &gate.components, gate.last);
        gate.last = world.read_change_tick();
        changed
    });
    if changed
        && world
            .run_system_cached(collect_hit_test_changes_query)
            .is_err()
    {
        world
            .resource_mut::<UiDirty>()
            .mark_all(UiDirtyDomain::HitTest);
    }
    world.resource_mut::<UiHoverValidation>().prepared = true;
    world.increment_change_tick();
}

pub(crate) fn collect_hit_test_changes_query(
    changed: HitTestStructuralChanged,
    changed_hitboxes: Query<Entity, Changed<UiHitBox>>,
    children: Query<&UiChildren>,
    mut validation: ResMut<UiHoverValidation>,
    mut dirty: ResMut<UiDirty>,
) {
    for entity in &changed {
        let mut pending = vec![entity];
        let mut visited = std::collections::HashSet::new();
        while let Some(at) = pending.pop() {
            if !visited.insert(at) {
                continue;
            }
            dirty.mark(UiDirtyDomain::HitTest, at);
            if let Ok(descendants) = children.get(at) {
                pending.extend(descendants.0.iter().copied());
            }
        }
    }
    for entity in &changed_hitboxes {
        dirty.mark(UiDirtyDomain::HitTest, entity);
    }
    validation.prepared = true;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub super_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiPointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiWheelUnit {
    Lines,
    Pixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiKeyCode {
    Tab,
    Escape,
    Enter,
    Space,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    F2,
    A,
    C,
    N,
    V,
    X,
    Y,
    Z,
    Other(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiInputEvent {
    PointerMove {
        window: Entity,
        position: [f32; 2],
        time_seconds: f64,
    },
    PointerButton {
        window: Entity,
        button: UiPointerButton,
        pressed: bool,
        position: [f32; 2],
        time_seconds: f64,
    },
    /// Modifier-bearing pointer edge for hosts that can query keyboard state
    /// while reading mouse button messages. Routing also emits the legacy
    /// pointer event variant so existing consumers keep working.
    PointerButtonWithModifiers {
        window: Entity,
        button: UiPointerButton,
        pressed: bool,
        position: [f32; 2],
        time_seconds: f64,
        modifiers: UiModifiers,
    },
    PointerCancel {
        window: Entity,
    },
    PointerLeave {
        window: Entity,
    },
    WindowBlur {
        window: Entity,
    },
    Key {
        window: Entity,
        key: UiKeyCode,
        pressed: bool,
        repeat: bool,
        modifiers: UiModifiers,
    },
    Text {
        window: Entity,
        text: String,
    },
    ImePreedit {
        window: Entity,
        text: String,
        cursor_range: Option<[usize; 2]>,
    },
    ImeCommit {
        window: Entity,
        text: String,
    },
    ImeDisabled {
        window: Entity,
    },
    Wheel {
        window: Entity,
        position: [f32; 2],
        delta: [f32; 2],
        unit: UiWheelUnit,
        modifiers: UiModifiers,
    },
}

#[derive(Resource, Debug, Default)]
pub struct UiInputQueue {
    events: VecDeque<UiInputEvent>,
}

impl UiInputQueue {
    pub fn push(&mut self, event: UiInputEvent) {
        self.events.push_back(event);
    }

    pub fn extend(&mut self, events: impl IntoIterator<Item = UiInputEvent>) {
        self.events.extend(events);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiRoutedEvent {
    PointerEnter {
        window: Entity,
        target: Entity,
    },
    PointerLeave {
        window: Entity,
        target: Entity,
    },
    PointerMoved {
        window: Entity,
        target: Entity,
        position: [f32; 2],
        delta: [f32; 2],
        dragging: bool,
    },
    PointerDown {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
    },
    /// Modifier annotation for the matching legacy [`Self::PointerDown`].
    /// Consumers should handle only one of the two variants per pointer edge.
    PointerDownWithModifiers {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
        modifiers: UiModifiers,
    },
    PointerUp {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
        cancelled: bool,
    },
    /// Modifier annotation for a non-cancelled [`Self::PointerUp`].
    PointerUpWithModifiers {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        position: [f32; 2],
        modifiers: UiModifiers,
    },
    Click {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        click_count: u32,
        position: [f32; 2],
    },
    /// Modifier annotation for the matching legacy [`Self::Click`].
    /// Consumers should handle only one of the two variants per click.
    ClickWithModifiers {
        window: Entity,
        target: Entity,
        button: UiPointerButton,
        click_count: u32,
        position: [f32; 2],
        modifiers: UiModifiers,
    },
    PointerCancel {
        window: Entity,
        target: Entity,
    },
    Wheel {
        window: Entity,
        target: Entity,
        position: [f32; 2],
        delta: [f32; 2],
        unit: UiWheelUnit,
        modifiers: UiModifiers,
    },
    Key {
        window: Entity,
        target: Entity,
        key: UiKeyCode,
        pressed: bool,
        repeat: bool,
        modifiers: UiModifiers,
    },
    Text {
        window: Entity,
        target: Entity,
        text: String,
    },
    ImePreedit {
        window: Entity,
        target: Entity,
        text: String,
        cursor_range: Option<[usize; 2]>,
    },
    ImeCommit {
        window: Entity,
        target: Entity,
        text: String,
    },
    ImeDisabled {
        window: Entity,
        target: Entity,
    },
    FocusChanged {
        window: Entity,
        previous: Option<Entity>,
        current: Option<Entity>,
    },
}

#[derive(Resource, Debug, Default)]
pub struct UiRoutedEvents {
    /// Events are replaced on every routing pass; consumers should drain or
    /// copy them before the next update.
    pub events: Vec<UiRoutedEvent>,
}

impl UiRoutedEvents {
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

mod process;
pub use process::{process_ui_input, refresh_ui_hover};
pub use validate::clear_invalid_ui_focus;

mod pointer;
use pointer::*;

mod focus;
use focus::*;

mod validate;
use validate::*;

// Keep the rectangle type available as part of this module's routing contract.
const _: Option<UiRect> = None;

#[cfg(test)]
#[path = "input/tests.rs"]
mod tests;

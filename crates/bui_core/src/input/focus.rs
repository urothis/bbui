use super::*;

pub(super) fn route_key(
    world: &mut World,
    window: Entity,
    key: UiKeyCode,
    pressed: bool,
    repeat: bool,
    modifiers: UiModifiers,
) {
    if pressed && key == UiKeyCode::Tab {
        let previous = focused(world, window);
        let next = traverse_focus(world, window, previous, modifiers.shift);
        set_focus(world, window, next);
        if previous != next {
            if let Some(target) = previous {
                emit(world, UiRoutedEvent::ImeDisabled { window, target });
            }
            emit(
                world,
                UiRoutedEvent::FocusChanged {
                    window,
                    previous,
                    current: next,
                },
            );
        }
        return;
    }
    let target =
        focused(world, window).filter(|target| is_interactable_in_window(world, *target, window));
    if let Some(target) = target {
        emit(
            world,
            UiRoutedEvent::Key {
                window,
                target,
                key,
                pressed,
                repeat,
                modifiers,
            },
        );
    }
}

pub(super) fn traverse_focus(
    world: &mut World,
    window: Entity,
    current: Option<Entity>,
    backwards: bool,
) -> Option<Entity> {
    let mut query = world.query::<(Entity, &UiFocusable)>();
    let mut entries: Vec<(i32, Vec<usize>, Entity)> = query
        .iter(world)
        .filter_map(|(entity, _)| {
            if !has_window_assignment(world, entity, window) {
                return None;
            }
            let tab = world.get::<UiTabIndex>(entity).map_or(0, |index| index.0);
            if tab < 0 {
                return None;
            }
            let order = hierarchy_order(world, entity);
            Some((tab, order, entity))
        })
        .collect();
    entries.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    if entries.is_empty() {
        return None;
    }
    let position = entries
        .iter()
        .position(|(_, _, entity)| Some(*entity) == current);
    let next = match (position, backwards) {
        (None, false) => 0,
        (None, true) => entries.len() - 1,
        (Some(at), false) => (at + 1) % entries.len(),
        (Some(0), true) => entries.len() - 1,
        (Some(at), true) => at - 1,
    };
    Some(entries[next].2)
}

pub(super) fn hierarchy_order(world: &World, entity: Entity) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = Some(entity);
    while let Some(at) = current {
        path.push(at);
        current = world.get::<crate::UiParent>(at).map(|parent| parent.0);
    }
    path.reverse();
    let mut order = Vec::with_capacity(path.len());
    for (depth, item) in path.iter().enumerate() {
        if depth == 0 {
            order.push(item.to_bits() as usize);
        } else {
            let parent = path[depth - 1];
            let child_index = world
                .get::<crate::UiChildren>(parent)
                .and_then(|children| children.0.iter().position(|child| child == item))
                .unwrap_or(item.to_bits() as usize);
            order.push(child_index);
        }
    }
    order
}

pub(super) fn set_focus(world: &mut World, window: Entity, target: Option<Entity>) {
    let mut focus = world.resource_mut::<UiFocus>();
    let state = focus.state_mut(window);
    if state.focused != target {
        state.selection = None;
    }
    state.focused = target;
    focus.set_active(window);
}

pub(super) fn focused(world: &World, window: Entity) -> Option<Entity> {
    world
        .resource::<UiFocus>()
        .state(window)
        .and_then(|state| state.focused)
}

pub(super) fn hit_test(world: &mut World, window: Entity, position: [f32; 2]) -> Option<Entity> {
    #[cfg(test)]
    if let Some(mut validation) = world.get_resource_mut::<UiHoverValidation>() {
        validation.hit_tests += 1;
    }
    let mut query = world.query::<(Entity, &UiHitBox)>();
    let mut candidates: Vec<(i32, Vec<usize>, Entity)> = query
        .iter(world)
        .filter(|&(entity, bounds)| {
            bounds.window.is_none_or(|owner| owner == window)
                && bounds.contains(position)
                && is_interactable_in_window(world, entity, window)
        })
        .map(|(entity, bounds)| (bounds.z_index, hierarchy_order(world, entity), entity))
        .collect();
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    candidates.last().map(|(_, _, entity)| *entity)
}

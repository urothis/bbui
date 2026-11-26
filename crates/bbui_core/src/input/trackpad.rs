use super::*;

/// Keeps phased streams on their starting surface, including scroll momentum.
#[derive(Resource, Default)]
pub(super) struct UiTrackpadState(HashMap<(Entity, UiTrackpadKind), UiTrackpadOwner>);

#[derive(Clone, Copy)]
enum UiTrackpadOwner {
    Surface(Entity),
    Elsewhere,
}

fn surface_at(world: &mut World, window: Entity, position: [f32; 2]) -> Option<Entity> {
    let mut current = hit_test(world, window, position);
    let mut visited = std::collections::HashSet::new();
    while let Some(entity) = current {
        if !visited.insert(entity) {
            break;
        }
        if world.get::<UiGestureSurface>(entity).is_some()
            && is_interactable_in_window(world, entity, window)
        {
            return Some(entity);
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    None
}

pub(super) fn route_trackpad(world: &mut World, sample: UiTrackpadGesture) {
    let UiTrackpadGesture {
        window,
        position,
        delta,
        scale,
        kind,
        phase,
        phased,
    } = sample;
    world.init_resource::<UiTrackpadState>();
    let key = (window, kind);
    let prior = world.resource::<UiTrackpadState>().0.get(&key).copied();
    let ending = matches!(phase, UiGesturePhase::Ended | UiGesturePhase::Cancelled);
    if phased && ending && prior.is_none() {
        return;
    }
    let fresh = phase == UiGesturePhase::Started || (phased && prior.is_none());
    let owner = if !phased || fresh {
        surface_at(world, window, position)
            .map_or(UiTrackpadOwner::Elsewhere, UiTrackpadOwner::Surface)
    } else {
        prior.unwrap_or(UiTrackpadOwner::Elsewhere)
    };
    let target = match owner {
        UiTrackpadOwner::Surface(target) => Some(target),
        UiTrackpadOwner::Elsewhere => None,
    };

    if fresh
        && let Some(UiTrackpadOwner::Surface(prior)) = prior
        && Some(prior) != target
    {
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target: prior,
                phase: UiGesturePhase::Cancelled,
                center: position,
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
    }
    if phased {
        let mut state = world.resource_mut::<UiTrackpadState>();
        if ending {
            state.0.remove(&key);
        } else {
            state.0.insert(key, owner);
        }
    }
    let Some(target) = target else {
        // Controls without gesture surfaces still receive ordinary pixel scrolling.
        if kind == UiTrackpadKind::Scroll
            && !ending
            && delta != [0.0, 0.0]
            && (!phased || surface_at(world, window, position).is_none())
        {
            let wheel_target = hit_test(world, window, position).or_else(|| focused(world, window));
            if let Some(wheel_target) =
                wheel_target.filter(|entity| is_interactable_in_window(world, *entity, window))
            {
                emit(
                    world,
                    UiRoutedEvent::Wheel {
                        window,
                        target: wheel_target,
                        position,
                        delta,
                        unit: UiWheelUnit::Pixels,
                        modifiers: UiModifiers::default(),
                    },
                );
            }
        }
        return;
    };
    if world.get::<UiGestureSurface>(target).is_none()
        || !is_interactable_in_window(world, target, window)
    {
        world.resource_mut::<UiTrackpadState>().0.remove(&key);
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target,
                phase: UiGesturePhase::Cancelled,
                center: position,
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
        return;
    }
    if fresh {
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target,
                phase: UiGesturePhase::Started,
                center: position,
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
    }
    if !ending && (delta != [0.0, 0.0] || scale != 1.0) {
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target,
                phase: UiGesturePhase::Updated,
                center: position,
                delta,
                scale,
            },
        );
    }
    if ending {
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target,
                phase,
                center: position,
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
    }
}

pub(super) fn cancel_trackpad_window(world: &mut World, window: Entity) {
    world.init_resource::<UiTrackpadState>();
    let owners: Vec<_> = world
        .resource::<UiTrackpadState>()
        .0
        .iter()
        .filter_map(|(&(owner_window, kind), &owner)| match owner {
            UiTrackpadOwner::Surface(target) if owner_window == window => Some((kind, target)),
            _ => None,
        })
        .collect();
    for (kind, target) in owners {
        world
            .resource_mut::<UiTrackpadState>()
            .0
            .remove(&(window, kind));
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target,
                phase: UiGesturePhase::Cancelled,
                center: [0.0, 0.0],
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
    }
}

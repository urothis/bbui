use super::*;

#[derive(Debug, Clone, Copy)]
struct TouchPoint {
    position: [f32; 2],
    initial_target: Option<Entity>,
}

#[derive(Debug, Clone, Copy)]
struct ActiveGesture {
    target: Entity,
    ids: [u64; 2],
    center: [f32; 2],
    distance: f32,
}

#[derive(Default)]
struct WindowTouches {
    points: HashMap<u64, TouchPoint>,
    gesture: Option<ActiveGesture>,
}

#[derive(Resource, Default)]
pub(super) struct UiTouchState(HashMap<Entity, WindowTouches>);

pub(super) fn touch_gesture_active(world: &World, window: Entity) -> bool {
    world
        .get_resource::<UiTouchState>()
        .and_then(|state| state.0.get(&window))
        .is_some_and(|state| state.gesture.is_some())
}

fn center_and_distance(a: [f32; 2], b: [f32; 2]) -> ([f32; 2], f32) {
    let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (center, (dx * dx + dy * dy).sqrt())
}

fn gesture_surface(world: &World, first: Entity, second: Entity, window: Entity) -> Option<Entity> {
    let mut at = Some(first);
    let mut seen = std::collections::HashSet::new();
    while let Some(entity) = at {
        if !seen.insert(entity) {
            break;
        }
        if world.get::<UiGestureSurface>(entity).is_some()
            && is_interactable_in_window(world, entity, window)
        {
            let mut other = Some(second);
            let mut visited = std::collections::HashSet::new();
            while let Some(candidate) = other {
                if !visited.insert(candidate) {
                    break;
                }
                if candidate == entity {
                    return Some(entity);
                }
                other = world.get::<UiParent>(candidate).map(|parent| parent.0);
            }
        }
        at = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    None
}

pub(super) fn route_touch(
    world: &mut World,
    window: Entity,
    id: u64,
    position: [f32; 2],
    phase: UiTouchPhase,
) {
    world.init_resource::<UiTouchState>();
    let hit = (phase == UiTouchPhase::Started)
        .then(|| hit_test(world, window, position))
        .flatten();
    let (active, candidate) = {
        let mut state = world.resource_mut::<UiTouchState>();
        let touches = state.0.entry(window).or_default();
        match phase {
            UiTouchPhase::Started => {
                touches.points.insert(
                    id,
                    TouchPoint {
                        position,
                        initial_target: hit,
                    },
                );
            }
            UiTouchPhase::Moved => {
                let Some(point) = touches.points.get_mut(&id) else {
                    return;
                };
                point.position = position;
            }
            UiTouchPhase::Ended | UiTouchPhase::Cancelled => {
                if !touches.points.contains_key(&id) {
                    return;
                }
            }
        }
        let active = touches.gesture;
        let candidate =
            if phase == UiTouchPhase::Started && active.is_none() && touches.points.len() == 2 {
                let mut points = touches.points.iter();
                points.next().zip(points.next()).map(
                    |((&first_id, &first), (&second_id, &second))| {
                        ([first_id, second_id], first, second)
                    },
                )
            } else {
                None
            };
        (active, candidate)
    };

    if let Some((ids, first, second)) = candidate
        && let (Some(first_target), Some(second_target)) =
            (first.initial_target, second.initial_target)
        && let Some(target) = gesture_surface(world, first_target, second_target, window)
    {
        let (center, distance) = center_and_distance(first.position, second.position);
        // The initial press must not later commit a click or widget drag.
        cancel_pointer(world, window);
        if let Some(touches) = world.resource_mut::<UiTouchState>().0.get_mut(&window) {
            touches.gesture = Some(ActiveGesture {
                target,
                ids,
                center,
                distance,
            });
        }
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target,
                phase: UiGesturePhase::Started,
                center,
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
        return;
    }

    if let Some(mut active) = active {
        let ending = matches!(phase, UiTouchPhase::Ended | UiTouchPhase::Cancelled)
            && active.ids.contains(&id);
        let valid = world.get::<UiGestureSurface>(active.target).is_some()
            && is_interactable_in_window(world, active.target, window);
        if ending || !valid {
            emit(
                world,
                UiRoutedEvent::SurfaceGesture {
                    window,
                    target: active.target,
                    phase: if phase == UiTouchPhase::Cancelled || !valid {
                        UiGesturePhase::Cancelled
                    } else {
                        UiGesturePhase::Ended
                    },
                    center: active.center,
                    delta: [0.0, 0.0],
                    scale: 1.0,
                },
            );
            if let Some(touches) = world.resource_mut::<UiTouchState>().0.get_mut(&window) {
                touches.gesture = None;
            }
        } else if active.ids.contains(&id) {
            let state = world.resource::<UiTouchState>();
            let points = state.0.get(&window).map(|touches| &touches.points);
            let pair = points
                .and_then(|points| points.get(&active.ids[0]).zip(points.get(&active.ids[1])));
            if let Some((first, second)) = pair {
                let (center, distance) = center_and_distance(first.position, second.position);
                let delta = [center[0] - active.center[0], center[1] - active.center[1]];
                let scale = if active.distance > 0.001 {
                    distance / active.distance
                } else {
                    1.0
                };
                active.center = center;
                active.distance = distance;
                if let Some(touches) = world.resource_mut::<UiTouchState>().0.get_mut(&window) {
                    touches.gesture = Some(active);
                }
                emit(
                    world,
                    UiRoutedEvent::SurfaceGesture {
                        window,
                        target: active.target,
                        phase: UiGesturePhase::Updated,
                        center,
                        delta,
                        scale,
                    },
                );
            }
        }
    }
    if matches!(phase, UiTouchPhase::Ended | UiTouchPhase::Cancelled) {
        let mut state = world.resource_mut::<UiTouchState>();
        if let Some(touches) = state.0.get_mut(&window) {
            touches.points.remove(&id);
            if touches.points.is_empty() {
                state.0.remove(&window);
            }
        }
    }
}

pub(super) fn cancel_touch_window(world: &mut World, window: Entity) {
    if let Some(touches) = world.resource_mut::<UiTouchState>().0.remove(&window)
        && let Some(gesture) = touches.gesture
    {
        emit(
            world,
            UiRoutedEvent::SurfaceGesture {
                window,
                target: gesture.target,
                phase: UiGesturePhase::Cancelled,
                center: gesture.center,
                delta: [0.0, 0.0],
                scale: 1.0,
            },
        );
    }
}

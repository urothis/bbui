use super::*;

const WHEEL_LINE_PIXELS: f32 = 40.0;

/// Apply routed wheel and scroll-navigation keys after input routing. Wheel
/// deltas bubble from the deepest scroll ancestor outward; each ancestor
/// consumes only the distance available before its boundary. When an offset
/// changes, layout and stationary-pointer hover are refreshed before style,
/// text, and widget paint phases.
pub fn apply_routed_scroll_input(world: &mut World) {
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    let mut changed = false;
    for event in events {
        match event {
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                modifiers,
                ..
            } if !modifiers.control
                && !modifiers.super_key
                && world.get::<UiWheelHandled>(target).is_none() =>
            {
                let scale = match unit {
                    UiWheelUnit::Lines => WHEEL_LINE_PIXELS,
                    UiWheelUnit::Pixels => 1.0,
                };
                changed |= scroll_ancestors(world, target, [delta[0] * scale, -delta[1] * scale]);
            }
            UiRoutedEvent::Key {
                target,
                key,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.control && !modifiers.super_key => match key {
                UiKeyCode::PageUp => {
                    let extent = scroll_page_extent(world, target);
                    changed |= scroll_ancestors(world, target, [0.0, -extent]);
                }
                UiKeyCode::PageDown => {
                    let extent = scroll_page_extent(world, target);
                    changed |= scroll_ancestors(world, target, [0.0, extent]);
                }
                // Home and End are also caret keys. They affect a scroll view
                // only when that scroll view itself owns keyboard focus.
                UiKeyCode::Home if world.get::<UiScroll>(target).is_some() => {
                    changed |= set_scroll_vertical_edge(world, target, false);
                }
                UiKeyCode::End if world.get::<UiScroll>(target).is_some() => {
                    changed |= set_scroll_vertical_edge(world, target, true);
                }
                _ => {}
            },
            _ => {}
        }
    }
    if changed {
        compute_layout(world);
        bbui_core::refresh_ui_hover(world);
    }
}

pub(super) fn scroll_page_extent(world: &World, target: Entity) -> f32 {
    let mut current = Some(target);
    let mut visited = HashSet::new();
    while let Some(entity) = current {
        if !visited.insert(entity) {
            break;
        }
        if world.get::<UiScroll>(entity).is_some() {
            return world
                .get::<UiLayoutResult>(entity)
                .map_or(0.0, |layout| layout.rect.height);
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    0.0
}

pub(super) fn scroll_ancestors(world: &mut World, target: Entity, mut remaining: [f32; 2]) -> bool {
    let mut current = Some(target);
    let mut visited = HashSet::new();
    let mut changed = false;
    while let Some(entity) = current {
        if !visited.insert(entity) || remaining.iter().all(|delta| delta.abs() < 0.001) {
            break;
        }
        if world.get::<UiWheelHandled>(entity).is_some() {
            break;
        }
        if world.get::<UiScroll>(entity).is_some() {
            let (remaining_delta, entity_changed) = consume_scroll_delta(world, entity, remaining);
            remaining = remaining_delta;
            changed |= entity_changed;
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    changed
}

pub(super) fn consume_scroll_delta(
    world: &mut World,
    entity: Entity,
    delta: [f32; 2],
) -> ([f32; 2], bool) {
    let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
        if let Some(mut scroll) = world.get_mut::<UiScroll>(entity) {
            let before = scroll.offset;
            scroll.scroll_by(delta[0], delta[1]);
            return ([0.0, 0.0], scroll.offset != before);
        }
        return (delta, false);
    };
    let maximum = [
        (layout.content_size[0] - layout.rect.width).max(0.0),
        (layout.content_size[1] - layout.rect.height).max(0.0),
    ];
    let Some(current) = world.get::<UiScroll>(entity).copied() else {
        return (delta, false);
    };
    let mut next = current.offset;
    let mut remaining = delta;
    for axis in 0..2 {
        let before = if next[axis].is_finite() {
            next[axis].clamp(0.0, maximum[axis])
        } else {
            0.0
        };
        let requested = if delta[axis].is_finite() {
            before + delta[axis]
        } else {
            before
        };
        next[axis] = requested.clamp(0.0, maximum[axis]);
        let consumed = next[axis] - before;
        remaining[axis] -= consumed;
    }
    if next != current.offset
        && let Some(mut scroll) = world.get_mut::<UiScroll>(entity)
    {
        scroll.offset = next;
    }
    (remaining, next != current.offset)
}

pub(super) fn set_scroll_vertical_edge(world: &mut World, entity: Entity, end: bool) -> bool {
    let Some(layout) = world.get::<UiLayoutResult>(entity).copied() else {
        return false;
    };
    let maximum = (layout.content_size[1] - layout.rect.height).max(0.0);
    if let Some(mut scroll) = world.get_mut::<UiScroll>(entity) {
        let target = if end { maximum } else { 0.0 };
        let changed = scroll.offset[1] != target;
        if changed {
            scroll.offset[1] = target;
        }
        changed
    } else {
        false
    }
}

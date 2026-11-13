use super::*;

pub(super) fn select_canvas_object(
    world: &mut World,
    target: Entity,
    id: u64,
    additive: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(before) = world
        .get::<Canvas>(target)
        .and_then(|canvas| canvas.to_json().ok())
    else {
        return;
    };
    let changed = world.get_mut::<Canvas>(target).is_some_and(|mut canvas| {
        if !canvas
            .objects
            .iter()
            .any(|object| object.id == id && object.visible && !object.locked)
        {
            return false;
        }
        let old = canvas.selected.clone();
        if !additive {
            canvas.selected.clear();
        }
        if additive && !canvas.selected.insert(id) {
            canvas.selected.remove(&id);
        } else if !additive {
            canvas.selected.insert(id);
        }
        old != canvas.selected
    });
    if changed
        && let Some(after) = world
            .get::<Canvas>(target)
            .and_then(|canvas| canvas.to_json().ok())
        && before != after
    {
        emitted.push(WidgetEvent::CanvasCommitted {
            target,
            before,
            after,
        });
    }
}

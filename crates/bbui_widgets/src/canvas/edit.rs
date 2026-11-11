use super::*;

/// Edit one Canvas object while keeping its spatial index in sync. The object's ID is stable;
/// use a structural Canvas edit to add, remove, or reorder objects.
/// Returns false when the object is missing, the edit is unchanged, or it makes the object invalid.
pub fn edit_canvas_object(
    world: &mut World,
    owner: Entity,
    id: u64,
    edit: impl FnOnce(&mut CanvasObject),
) -> bool {
    if !ensure_canvas_spatial_index(world, owner) {
        return false;
    }
    let Some(position) = world
        .resource::<CanvasSpatialIndexes>()
        .0
        .get(&owner)
        .filter(|index| index.usable)
        .and_then(|index| index.objects.get(&id))
        .map(|entry| entry.index)
    else {
        return false;
    };
    let Some(original) = world
        .get::<Canvas>(owner)
        .and_then(|canvas| canvas.objects.get(position))
        .filter(|object| object.id == id)
    else {
        return false;
    };
    let mut updated = original.clone();
    edit(&mut updated);
    if updated.id != id || !updated.valid() || updated == *original {
        return false;
    }
    let Some(mut canvas) = world.get_mut::<Canvas>(owner) else {
        return false;
    };
    let Some(object) = canvas.objects.get_mut(position) else {
        return false;
    };
    *object = updated;
    note_canvas_object_edits(world, owner, &[position]);
    true
}

pub(crate) fn note_canvas_metadata_edit(world: &mut World, owner: Entity) {
    let Some(changed_tick) = canvas_component_tick(world, owner) else {
        return;
    };
    let Some(mut indexes) = world.get_resource_mut::<CanvasSpatialIndexes>() else {
        return;
    };
    let Some(index) = indexes.0.get_mut(&owner) else {
        return;
    };
    if index.usable {
        index.changed_tick = changed_tick;
    }
}

pub(super) fn note_canvas_object_edits(world: &mut World, owner: Entity, positions: &[usize]) {
    let Some(changed_tick) = canvas_component_tick(world, owner) else {
        return;
    };
    let updates = {
        let Some(canvas) = world.get::<Canvas>(owner) else {
            return;
        };
        positions
            .iter()
            .filter_map(|position| {
                canvas
                    .objects
                    .get(*position)
                    .map(|object| (*position, object.id, object.bounds()))
            })
            .collect::<Vec<_>>()
    };
    let Some(mut indexes) = world.get_resource_mut::<CanvasSpatialIndexes>() else {
        return;
    };
    let Some(index) = indexes.0.get_mut(&owner) else {
        return;
    };
    if !index.usable {
        return;
    }
    for (position, id, bounds) in updates {
        index.update_object(position, id, bounds);
    }
    index.changed_tick = changed_tick;
}

/// Keep a stable entity only for each object intersecting the visible canvas
/// clip. These entities expose per-object accessibility without scaling entity
/// count with offscreen content.
pub fn sync_canvas_object_views(world: &mut World) {
    let live_canvas_owners = {
        let mut query = world.query_filtered::<Entity, With<Canvas>>();
        query.iter(world).collect::<HashSet<_>>()
    };
    if let Some(mut indexes) = world.get_resource_mut::<CanvasSpatialIndexes>() {
        indexes
            .0
            .retain(|owner, _| live_canvas_owners.contains(owner));
    }
    let mut query = world.query::<(Entity, &Canvas, &UiLayoutResult)>();
    let owners = query
        .iter(world)
        .map(|(owner, _, layout)| (owner, *layout))
        .collect::<Vec<_>>();
    let mut previous = world
        .remove_resource::<CanvasObjectItems>()
        .unwrap_or_default();
    let mut previous_entities = std::mem::take(&mut previous.0);
    let mut next_entities = HashMap::new();
    for (owner, layout) in owners {
        if !visible_in_hierarchy(world, owner) || world.get::<UiNode>(owner).is_none() {
            continue;
        }
        ensure_canvas_spatial_index(world, owner);
        let Some(canvas) = world.get::<Canvas>(owner) else {
            continue;
        };
        let clip = layout.clip.unwrap_or(layout.rect).intersection(layout.rect);
        let mut items = Vec::new();
        let visible_indices = query_canvas_object_indices(
            world,
            owner,
            canvas,
            layout.rect,
            ClipRect::from(clip.unwrap_or_default()),
            8.0,
        )
        .unwrap_or_else(|| (0..canvas.objects.len()).collect());
        for index in visible_indices.iter().copied() {
            let Some(object) = canvas.objects.get(index) else {
                continue;
            };
            if !object.visible {
                continue;
            }
            let paint = CanvasObjectPaint(object.clone());
            let bounds = object.bounds();
            let position = canvas.world_to_screen([bounds.x, bounds.y]);
            let rect = UiRect::new(
                layout.rect.x + position[0] - 6.0,
                layout.rect.y + position[1] - 6.0,
                (bounds.width * canvas.zoom).max(1.0) + 12.0,
                (bounds.height * canvas.zoom).max(1.0) + 12.0,
            );
            let Some(visible_rect) = clip.and_then(|clip| clip.intersection(rect)) else {
                continue;
            };
            let label = match &object.shape {
                CanvasShape::Rectangle { .. } => format!("Rectangle {}", object.id),
                CanvasShape::Ellipse { .. } => format!("Ellipse {}", object.id),
                CanvasShape::Line { .. } => format!("Line {}", object.id),
                CanvasShape::Image { asset, .. } => format!("Image {}: {asset}", object.id),
            };
            let control = CanvasObjectControl {
                owner,
                object_id: object.id,
                label,
                selected: canvas.selected.contains(&object.id),
                locked: object.locked,
            };
            let child_layout = UiLayoutResult {
                window: layout.window,
                rect: visible_rect,
                clip,
                z: layout.z.saturating_add(1),
                content_size: [rect.width, rect.height],
                dpi_scale: layout.dpi_scale,
            };
            items.push((control, paint, child_layout));
        }
        let mut owned = previous_entities.remove(&owner).unwrap_or_default();
        let mut current = HashMap::new();
        let mut ordered_entities = Vec::with_capacity(items.len());
        for (control, paint, child_layout) in items {
            let object_id = control.object_id;
            let locked = control.locked;
            let entity = if let Some(entity) = owned
                .remove(&object_id)
                .filter(|entity| world.get_entity(*entity).is_ok())
            {
                entity
            } else {
                let entity = world
                    .spawn((
                        UiNode,
                        UiFocusable,
                        UiKey(format!("canvas:{owner:?}:{object_id}")),
                    ))
                    .id();
                if set_parent(world, entity, owner).is_err() {
                    world.despawn(entity);
                    continue;
                }
                entity
            };
            let mut paint_changed = false;
            let mut accessibility_changed = false;
            if world.get::<CanvasObjectControl>(entity) != Some(&control) {
                world.entity_mut(entity).insert(control);
                paint_changed = true;
                accessibility_changed = true;
            }
            if world.get::<CanvasObjectPaint>(entity) != Some(&paint) {
                world.entity_mut(entity).insert(paint);
                paint_changed = true;
            }
            if world.get::<UiLayoutResult>(entity) != Some(&child_layout) {
                world.entity_mut(entity).insert(child_layout);
                paint_changed = true;
            }
            if world
                .get::<UiEnabled>(entity)
                .is_none_or(|enabled| enabled.0 == locked)
            {
                world.entity_mut(entity).insert(UiEnabled(!locked));
                paint_changed = true;
                accessibility_changed = true;
            }
            if paint_changed || accessibility_changed {
                world.init_resource::<UiDirty>();
                let mut dirty = world.resource_mut::<UiDirty>();
                if paint_changed {
                    dirty.mark(UiDirtyDomain::Paint, entity);
                }
                if accessibility_changed {
                    dirty.mark(UiDirtyDomain::Accessibility, entity);
                }
            }
            current.insert(object_id, entity);
            ordered_entities.push(entity);
        }
        for entity in owned.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
            world.init_resource::<UiDirty>();
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Paint, entity);
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Accessibility, entity);
        }
        if let Some(children) = world
            .get::<UiChildren>(owner)
            .map(|children| children.0.clone())
        {
            let mut reordered = children
                .iter()
                .copied()
                .filter(|child| {
                    world
                        .get::<CanvasObjectControl>(*child)
                        .is_none_or(|control| control.owner != owner)
                })
                .collect::<Vec<_>>();
            reordered.extend(ordered_entities.iter().copied());
            if reordered != children {
                world.entity_mut(owner).insert(UiChildren(reordered));
            }
        }
        next_entities.insert(owner, current);
    }
    for owned in previous_entities.into_values() {
        for entity in owned.into_values() {
            if world.get_entity(entity).is_ok() {
                detach_from_parent(world, entity);
                world.despawn(entity);
            }
            world.init_resource::<UiDirty>();
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Paint, entity);
            world
                .resource_mut::<UiDirty>()
                .mark(UiDirtyDomain::Accessibility, entity);
        }
    }
    world.insert_resource(CanvasObjectItems(next_entities));
}

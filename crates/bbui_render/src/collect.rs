use super::*;

#[expect(
    clippy::type_complexity,
    reason = "Bevy change query covers render inputs"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Bevy injects each tracked resource separately"
)]
pub(super) fn collect_render_changes(
    display_list: Res<DisplayList>,
    images: Res<ImageRegistry>,
    mut cache: ResMut<RenderCache>,
    target_camera: Res<RenderTargetCamera>,
    theme: Option<Res<Theme>>,
    changed_paints: Query<Entity, Or<(Changed<UiLayoutResult>, Changed<UiPaint>)>>,
    changed_parents: Query<Entity, Changed<UiParent>>,
    changed_cameras: Query<Entity, Changed<Camera>>,
    hierarchy_query: Query<(Entity, Option<&UiParent>)>,
    mut removed_paints: RemovedComponents<UiPaint>,
    mut removed_layouts: RemovedComponents<UiLayoutResult>,
    mut removed_parents: RemovedComponents<UiParent>,
    mut removed_cameras: RemovedComponents<Camera>,
    mut dirty: ResMut<UiDirty>,
) {
    let changed_parent_entities: Vec<_> = changed_parents.iter().collect();
    let removed_parent_entities: Vec<_> = removed_parents.read().collect();
    let changed_camera_entities: Vec<_> = changed_cameras.iter().collect();
    let removed_camera_entities: Vec<_> = removed_cameras.read().collect();
    let mut parent_affected = HashSet::new();
    for entity in changed_parent_entities
        .iter()
        .chain(removed_parent_entities.iter())
    {
        parent_affected.extend(cached_subtree(&cache, *entity));
    }
    for entity in removed_parent_entities {
        update_cached_parent(&mut cache, entity, None);
    }
    for entity in changed_parent_entities {
        let parent = hierarchy_query
            .get(entity)
            .ok()
            .and_then(|(_, parent)| parent.map(|parent| parent.0));
        update_cached_parent(&mut cache, entity, parent);
    }
    let mut scoped_entities: HashSet<_> = parent_affected
        .into_iter()
        .filter(|entity| {
            cache.layout_order.contains_key(entity) || cache.fragment_keys.contains_key(entity)
        })
        .collect();
    for camera in changed_camera_entities
        .iter()
        .chain(removed_camera_entities.iter())
    {
        if let Some(entities) = cache.layout_entities_by_window.get(camera) {
            scoped_entities.extend(entities.iter().copied());
        }
        if let Some(entities) = cache.widget_fragments_by_camera.get(camera) {
            scoped_entities.extend(entities.iter().copied());
        }
    }
    let mut image_full_change = false;
    let mut image_entities = HashSet::new();
    for asset_id in images.pending_changed_ids() {
        if let Some(users) = cache.image_users.get(&asset_id) {
            for key in users {
                match key {
                    RenderKey::WidgetFragment(entity, _) | RenderKey::LayoutEntity(entity) => {
                        image_entities.insert(*entity);
                    }
                    _ => image_full_change = true,
                }
            }
        }
    }
    let full_change = display_list.has_pending_full_change()
        || image_full_change
        || target_camera.is_changed()
        || theme.as_ref().is_some_and(DetectChanges::is_changed);
    let changed_paint_entities: Vec<_> = changed_paints.iter().collect();
    let removed_paint_entities: Vec<_> = removed_paints.read().collect();
    let removed_layout_entities: Vec<_> = removed_layouts.read().collect();
    if full_change {
        dirty.mark_all(UiDirtyDomain::Render);
    } else {
        for entity in changed_paint_entities
            .into_iter()
            .chain(removed_paint_entities)
            .chain(removed_layout_entities)
            .chain(image_entities)
            .chain(scoped_entities)
            .chain(display_list.pending_widget_fragment_changes())
        {
            dirty.mark(UiDirtyDomain::Render, entity);
        }
    }
}

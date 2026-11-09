use super::*;

pub(super) fn sync_display_list(mut sync: RendererSync) {
    sync.stats.scanned_draw_keys = 0;
    sync.stats.updated_render_entries = 0;
    let _changed_image_ids = sync.images.take_changed_ids();
    let changes = sync.dirty.take(UiDirtyDomain::Render);
    let full_change = sync.display_list.take_pending_full_change();
    let mut fragment_changes = sync.display_list.take_widget_fragment_changes();
    let mut batch_group_changes = sync.display_list.take_widget_batch_group_changes();
    let mut batch_groups: HashSet<_> = batch_group_changes.iter().copied().collect();
    let mut fragment_entities: HashSet<_> = fragment_changes.iter().copied().collect();
    for entity in &changes.entities {
        if sync.cache.fragment_keys.contains_key(entity) && fragment_entities.insert(*entity) {
            fragment_changes.push(*entity);
        }
    }
    for entity in &fragment_changes {
        if let Some(group) = sync.display_list.widget_fragment_batch_group(*entity)
            && batch_groups.insert(group)
        {
            batch_group_changes.push(group);
        }
    }
    for group in &batch_group_changes {
        for entity in sync.display_list.widget_batch_group_members(*group) {
            if fragment_entities.insert(entity) {
                fragment_changes.push(entity);
            }
        }
    }
    if changes.is_empty() && !full_change && fragment_changes.is_empty() {
        return;
    }
    let direct_entities: Vec<_> = changes
        .entities
        .iter()
        .copied()
        .filter(|entity| {
            sync.paint_query
                .get(*entity)
                .is_ok_and(|(_, layout, paint)| layout.is_changed() || paint.is_changed())
                || sync
                    .cache
                    .items
                    .contains_key(&RenderKey::LayoutEntity(*entity))
        })
        .collect();
    let direct_entities_are_known = direct_entities.iter().all(|entity| {
        let key = RenderKey::LayoutEntity(*entity);
        match sync.paint_query.get(*entity) {
            Ok((_, layout, paint)) => {
                let new_item = paint.draw_item(&layout);
                if let Some(batch_key) = sync.cache.bezier_batch_for_key.get(&key) {
                    sync.cache
                        .bezier_batches
                        .get(batch_key)
                        .is_some_and(|batch| batch.keys.contains(&key))
                } else {
                    sync.cache
                        .items
                        .get(&key)
                        .is_some_and(|cached| cached.item.z == new_item.z)
                }
            }
            Err(_) => !sync.cache.bezier_batch_for_key.contains_key(&key),
        }
    });
    let mut labels_available = true;
    if !full_change && !changes.all && direct_entities_are_known && sync.cache.generation > 0 {
        if !update_layout_order_labels_for_entities(
            &mut sync.cache,
            direct_entities.iter().copied(),
        ) {
            labels_available = false;
        }
        for entity in &fragment_changes {
            if !labels_available {
                break;
            }
            let fragment = sync.display_list.widget_fragments.get(entity);
            if !update_widget_order_labels(&mut sync.cache, *entity, fragment) {
                labels_available = false;
                break;
            }
        }
    }
    if !full_change
        && !changes.all
        && (!fragment_changes.is_empty() || !direct_entities.is_empty())
        && direct_entities_are_known
        && labels_available
        && sync.cache.generation > 0
    {
        sync_widget_fragments(sync, fragment_changes, batch_group_changes, direct_entities);
    } else {
        sync_display_list_full(sync, changes);
    }
}

pub(super) fn decrement_clip_group_users(
    clip: ClipGroupKey,
    commands: &mut Commands,
    cache: &mut RenderCache,
) {
    let Some(users) = cache.clip_group_users.get_mut(&clip) else {
        return;
    };
    *users = users.saturating_sub(1);
    if *users != 0 {
        return;
    }
    cache.clip_group_users.remove(&clip);
    if let Some(group) = cache.clip_groups.remove(&clip) {
        commands.entity(group.entity).despawn();
    }
}

pub(super) fn retire_bezier_batch(
    key: BezierBatchKey,
    commands: &mut Commands,
    cache: &mut RenderCache,
    materials: &mut Option<ResMut<Assets<BezierBatchMaterial>>>,
    buffers: &mut Option<ResMut<Assets<ShaderBuffer>>>,
) -> bool {
    let Some(batch) = cache.bezier_batches.remove(&key) else {
        return false;
    };
    commands.entity(batch.entity).despawn();
    if let Some(materials) = materials.as_mut() {
        materials.remove(batch.material.id());
    }
    if let Some(buffers) = buffers.as_mut() {
        buffers.remove(batch.segments.id());
        buffers.remove(batch.curves.id());
        buffers.remove(batch.tile_indices.id());
    }
    decrement_clip_group_users(key.clip, commands, cache);
    true
}

pub(super) fn remove_cached_item(
    key: RenderKey,
    commands: &mut Commands,
    cache: &mut RenderCache,
    hsv_wheel_materials: &mut Option<ResMut<Assets<HsvWheelMaterial>>>,
    bezier_materials: &mut Option<ResMut<Assets<BezierMaterial>>>,
) -> bool {
    let Some(cached) = cache.items.remove(&key) else {
        return false;
    };
    cached.nodes.despawn(commands);
    if let Some(asset_id) = cached.image_asset_id
        && let Some(users) = cache.image_users.get_mut(&asset_id)
    {
        users.remove(&key);
        if users.is_empty() {
            cache.image_users.remove(&asset_id);
        }
    }
    if let Some(material) = cached.hsv_material
        && let Some(materials) = hsv_wheel_materials.as_mut()
    {
        materials.remove(material.id());
    }
    if let Some(material) = cached.bezier_material
        && let Some(materials) = bezier_materials.as_mut()
    {
        materials.remove(material.id());
    }
    if let Some(clip) = cached.clip_group {
        decrement_clip_group_users(clip, commands, cache);
    }
    true
}

#[expect(
    clippy::too_many_arguments,
    reason = "Renderer resources are independently injected and mutated"
)]
pub(super) fn spawn_incremental_item(
    key: RenderKey,
    desired: DesiredItem,
    item: &DrawItem,
    commands: &mut Commands,
    cache: &mut RenderCache,
    images: &ImageRegistry,
    hsv_wheel_materials: &mut Option<ResMut<Assets<HsvWheelMaterial>>>,
    bezier_materials: &mut Option<ResMut<Assets<BezierMaterial>>>,
    text_font_size: f32,
    text_line_height: f32,
    default_text_font: &TextFont,
) {
    let local_z = desired_local_z(
        &desired,
        &cache.layout_order,
        &cache.layout_order_labels,
        &cache.widget_order_labels,
    );
    let clip_group =
        valid_clip(item.clip).then(|| ClipGroupKey::new(item.clip, desired.target_camera));
    let clip_group_entity = if let Some(group_key) = clip_group {
        let entity = if let Some(group) = cache.clip_groups.get_mut(&group_key) {
            group.generation = cache.generation;
            group.entity
        } else {
            let entity = spawn_clip_group(commands, group_key);
            cache.clip_groups.insert(
                group_key,
                CachedClipGroup {
                    entity,
                    generation: cache.generation,
                },
            );
            entity
        };
        *cache.clip_group_users.entry(group_key).or_default() += 1;
        Some(entity)
    } else {
        None
    };
    let wheel_values = match (&item.primitive, clip_group_entity) {
        (Primitive::HsvWheel { rect, .. }, Some(_)) if valid_clip(*rect) => {
            hsv_wheel_parameters(&item.primitive)
        }
        _ => None,
    };
    let hsv_material = wheel_values.and_then(|(hue_degrees, saturation, value)| {
        hsv_wheel_materials.as_mut().map(|materials| {
            materials.add(HsvWheelMaterial {
                parameters: Vec4::new(hue_degrees, saturation, value, 0.0),
            })
        })
    });
    let bezier_values = match (&item.primitive, clip_group_entity) {
        (Primitive::Bezier { .. }, Some(_)) => bezier_material_values(item),
        _ => None,
    };
    let bezier_material = bezier_values.and_then(|values| {
        bezier_materials.as_mut().map(|materials| {
            materials.add(BezierMaterial {
                points_01: values.points_01,
                points_23: values.points_23,
                color: values.color,
                style: values.style,
            })
        })
    });
    let item_font_size = uses_theme_font_size(&item.primitive).then_some(text_font_size);
    let item_line_height = uses_theme_line_height(&item.primitive).then_some(text_line_height);
    let resolved_image = image_handle(images, &item.primitive).cloned();
    let image_asset_id = image_asset_id(&item.primitive);
    let mut nodes = RenderedNodes::default();
    if let Some(group) = clip_group_entity {
        commands.entity(group).with_children(|parent| {
            spawn_primitive(
                parent,
                item,
                images,
                PrimitiveRenderContext {
                    text_line_height,
                    default_text_font: default_text_font.clone(),
                    hsv_material: hsv_material.as_ref(),
                    bezier_material: bezier_material.as_ref(),
                    local_z,
                },
                &mut nodes,
            );
        });
    }
    cache.items.insert(
        key,
        CachedItem {
            item: item.clone(),
            nodes,
            local_z,
            clip_group,
            target_camera: desired.target_camera,
            text_font_size: item_font_size,
            text_line_height: item_line_height,
            image: resolved_image,
            image_asset_id,
            hsv_wheel_values: wheel_values,
            hsv_material,
            bezier_values,
            bezier_material,
            generation: cache.generation,
        },
    );
    if let Some(asset_id) = image_asset_id {
        cache.image_users.entry(asset_id).or_default().insert(key);
    }
}

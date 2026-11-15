use super::*;

pub(super) fn sync_widget_fragments(
    sync: RendererSync,
    fragment_entities: Vec<Entity>,
    mut batch_group_changes: Vec<WidgetFragmentBatchGroup>,
    direct_entities: Vec<Entity>,
) {
    let RendererSync {
        mut commands,
        display_list,
        images,
        mut hsv_wheel_materials,
        mut bezier_materials,
        mut bezier_batch_materials,
        mut shader_buffers,
        target_camera,
        theme,
        camera_query,
        paint_query,
        mut cache,
        mut stats,
        ..
    } = sync;
    let text_font_size = theme
        .as_ref()
        .map(|theme| theme.font_size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .unwrap_or(14.0);
    let text_line_height = theme
        .as_ref()
        .map(|theme| theme.line_height)
        .filter(|height| height.is_finite() && *height > 0.0)
        .unwrap_or(1.25);
    let default_text_font = ui_default_text_font(text_font_size);
    let mut old_keys = Vec::new();
    let mut old_fragment_batches = HashMap::new();
    for entity in &fragment_entities {
        update_widget_fragment_camera(
            &mut cache,
            *entity,
            display_list
                .widget_fragments
                .get(entity)
                .and_then(|fragment| fragment.target_camera),
        );
        if let Some(keys) = cache.fragment_keys.remove(entity) {
            for key in keys {
                cache.bezier_batch_for_key.remove(&key);
                old_keys.push(key);
            }
        }
        if let Some(batch_keys) = cache.fragment_batches.remove(entity) {
            old_fragment_batches.insert(*entity, batch_keys);
        }
    }
    old_keys.sort_unstable_by_key(|key| match key {
        RenderKey::WidgetFragment(entity, index) => (entity.to_bits(), *index as u64),
        RenderKey::LayoutEntity(entity) => (entity.to_bits(), u64::MAX),
        _ => (0, 0),
    });
    old_keys.dedup();
    stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(old_keys.len());
    for key in old_keys {
        if remove_cached_item(
            key,
            &mut commands,
            &mut cache,
            &mut hsv_wheel_materials,
            &mut bezier_materials,
        ) {
            stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
        }
    }

    for entity in fragment_entities {
        if let Some(fragment) = display_list.widget_fragments.get(&entity) {
            if display_list.widget_fragment_batch_group(entity).is_some()
                && bezier_batch_materials.is_some()
                && shader_buffers.is_some()
            {
                let keys: Vec<_> = (0..fragment.items.len())
                    .map(|index| RenderKey::WidgetFragment(entity, index))
                    .collect();
                cache.fragment_keys.insert(entity, keys);
                continue;
            }
            let mut desired: Vec<_> = fragment
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| DesiredItem {
                    key: RenderKey::WidgetFragment(entity, index),
                    z: item.z,
                    target_camera: fragment.target_camera,
                    hierarchy_depth: 0,
                    fragment_order: Some(fragment.order),
                })
                .collect();
            desired.sort_unstable_by(|first, second| {
                first.z.cmp(&second.z).then_with(|| {
                    first
                        .key
                        .stable_order(
                            first.fragment_order,
                            first.hierarchy_depth,
                            first.target_camera,
                        )
                        .cmp(&second.key.stable_order(
                            second.fragment_order,
                            second.hierarchy_depth,
                            second.target_camera,
                        ))
                })
            });
            let mut keys = Vec::with_capacity(desired.len());
            let mut batch_keys = Vec::new();
            let mut batched = HashSet::new();
            if let (Some(materials), Some(buffers)) =
                (bezier_batch_materials.as_mut(), shader_buffers.as_mut())
            {
                let mut start = 0;
                while start < desired.len() {
                    let fragment_item = |position: usize| {
                        let RenderKey::WidgetFragment(_, item_index) = desired[position].key else {
                            return None;
                        };
                        fragment.items.get(item_index)
                    };
                    let Some(first_item) = fragment_item(start) else {
                        start += 1;
                        continue;
                    };
                    let Some(clip) = valid_clip(first_item.clip)
                        .then(|| ClipGroupKey::new(first_item.clip, desired[start].target_camera))
                    else {
                        start += 1;
                        continue;
                    };
                    if !matches!(first_item.primitive, Primitive::Bezier { .. }) {
                        start += 1;
                        continue;
                    }
                    let mut end = start + 1;
                    while end < desired.len() && end - start < MAX_BEZIER_BATCH_CURVES {
                        let Some(next_item) = fragment_item(end) else {
                            break;
                        };
                        if desired[end].target_camera != desired[start].target_camera
                            || next_item.z != first_item.z
                            || !matches!(next_item.primitive, Primitive::Bezier { .. })
                            || !valid_clip(next_item.clip)
                            || ClipGroupKey::new(next_item.clip, desired[end].target_camera) != clip
                        {
                            break;
                        }
                        end += 1;
                    }
                    if end - start < 2 {
                        start += 1;
                        continue;
                    }
                    let signature: Vec<_> =
                        (start..end).filter_map(fragment_item).cloned().collect();
                    let key = BezierBatchKey {
                        first: desired[start].key,
                        last: desired[end - 1].key,
                        clip,
                    };
                    if let Some(values) = build_bezier_batch_values(&signature, clip.clip()) {
                        let local_z = desired_local_z(
                            &desired[start],
                            &cache.layout_order,
                            &cache.layout_order_labels,
                            &cache.widget_order_labels,
                        );
                        let reused_batch = cache.bezier_batches.contains_key(&key);
                        let generation = cache.generation;
                        let batch_member_keys =
                            desired[start..end].iter().map(|entry| entry.key).collect();
                        let updated = BezierBatchSyncContext {
                            commands: &mut commands,
                            cache: &mut cache,
                            materials,
                            buffers,
                            generation,
                        }
                        .sync(
                            key,
                            batch_member_keys,
                            signature,
                            values,
                            desired[start].z,
                            local_z,
                        );
                        if cache.bezier_batches.contains_key(&key) {
                            if !reused_batch {
                                *cache.clip_group_users.entry(clip).or_default() += 1;
                            }
                            if updated {
                                stats.updated_render_entries =
                                    stats.updated_render_entries.saturating_add(1);
                            }
                            batch_keys.push(key);
                            for entry in &desired[start..end] {
                                cache.bezier_batch_for_key.insert(entry.key, key);
                            }
                            for entry in &desired[start..end] {
                                batched.insert(entry.key);
                            }
                        }
                    }
                    start = end;
                }
            }
            for entry in desired {
                let RenderKey::WidgetFragment(_, index) = entry.key else {
                    continue;
                };
                let Some(item) = fragment.items.get(index) else {
                    continue;
                };
                if !batched.contains(&entry.key) {
                    spawn_incremental_item(
                        entry.key,
                        entry,
                        item,
                        &mut commands,
                        &mut cache,
                        &images,
                        &mut hsv_wheel_materials,
                        &mut bezier_materials,
                        text_font_size,
                        text_line_height,
                        &default_text_font,
                    );
                    stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
                }
                keys.push(entry.key);
                stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(1);
            }
            cache.fragment_keys.insert(entity, keys);
            let retained: HashSet<_> = batch_keys.iter().copied().collect();
            if !batch_keys.is_empty() {
                cache.fragment_batches.insert(entity, batch_keys);
            }
            if let Some(old_batches) = old_fragment_batches.remove(&entity) {
                for old_key in old_batches {
                    if !retained.contains(&old_key)
                        && retire_bezier_batch(
                            old_key,
                            &mut commands,
                            &mut cache,
                            &mut bezier_batch_materials,
                            &mut shader_buffers,
                        )
                    {
                        stats.updated_render_entries =
                            stats.updated_render_entries.saturating_add(1);
                    }
                }
            }
        }
    }

    if bezier_batch_materials.is_some() && shader_buffers.is_some() {
        batch_group_changes.sort_unstable_by_key(|group| (group.owner.to_bits(), group.chunk));
        batch_group_changes.dedup();
        for group in batch_group_changes {
            sync_widget_fragment_bezier_group(
                group,
                &display_list,
                &images,
                &mut commands,
                &mut cache,
                &mut stats,
                &mut bezier_batch_materials,
                &mut shader_buffers,
                &mut hsv_wheel_materials,
                &mut bezier_materials,
                text_font_size,
                text_line_height,
                &default_text_font,
            );
        }
    }

    let previous_group_batches: HashSet<_> = old_fragment_batches.into_values().flatten().collect();
    let retained_group_batches: HashSet<_> =
        cache.fragment_batches.values().flatten().copied().collect();
    for old_key in previous_group_batches
        .difference(&retained_group_batches)
        .copied()
    {
        if retire_bezier_batch(
            old_key,
            &mut commands,
            &mut cache,
            &mut bezier_batch_materials,
            &mut shader_buffers,
        ) {
            stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
        }
    }

    let mut touched_batches = HashSet::new();
    for entity in &direct_entities {
        if let Some(batch_key) = cache
            .bezier_batch_for_key
            .get(&RenderKey::LayoutEntity(*entity))
        {
            touched_batches.insert(*batch_key);
        }
    }
    let mut touched_batches: Vec<_> = touched_batches.into_iter().collect();
    touched_batches.sort_unstable_by_key(|key| (key.first, key.last));
    'batches: for old_batch_key in touched_batches {
        let Some(batch) = cache.bezier_batches.get(&old_batch_key).cloned() else {
            continue;
        };
        let mut entries: Vec<_> = batch
            .keys
            .iter()
            .copied()
            .enumerate()
            .map(|(index, key)| {
                let old_item = batch.signature.get(index).cloned();
                let (item, target_camera, fragment_order, hierarchy_depth) = match key {
                    RenderKey::LayoutEntity(entity) => match paint_query.get(entity) {
                        Ok((_, layout, paint)) => (
                            Some(paint.draw_item(&layout)),
                            camera_query.get(layout.window).ok().map(|_| layout.window),
                            None,
                            hierarchy_depth(entity, &cache.parents_by_entity),
                        ),
                        Err(_) => (
                            None,
                            old_batch_key.clip.target_camera,
                            None,
                            hierarchy_depth(entity, &cache.parents_by_entity),
                        ),
                    },
                    RenderKey::WidgetFragment(entity, _) => {
                        let fragment = display_list.widget_fragments.get(&entity);
                        (
                            display_list_item(&display_list, key).cloned(),
                            fragment.and_then(|fragment| fragment.target_camera),
                            fragment.map(|fragment| fragment.order),
                            0,
                        )
                    }
                    RenderKey::ManualDisplayListIndex(_) => (
                        display_list_item(&display_list, key).cloned(),
                        target_camera.0,
                        None,
                        0,
                    ),
                    RenderKey::CameraDisplayListIndex(camera, _)
                    | RenderKey::OverlayCameraDisplayListIndex(camera, _) => (
                        display_list_item(&display_list, key).cloned(),
                        Some(camera),
                        None,
                        0,
                    ),
                    _ => (
                        display_list_item(&display_list, key).cloned(),
                        None,
                        None,
                        0,
                    ),
                };
                let z = item
                    .as_ref()
                    .or(old_item.as_ref())
                    .map_or(batch.first_z, |item| item.z);
                let desired = DesiredItem {
                    key,
                    z,
                    target_camera,
                    hierarchy_depth,
                    fragment_order,
                };
                CurrentBezierEntry {
                    order: order_for_desired(&desired),
                    desired,
                    item,
                }
            })
            .collect();
        entries.sort_unstable_by(|first, second| {
            first
                .desired
                .z
                .cmp(&second.desired.z)
                .then_with(|| first.order.cmp(&second.order))
        });
        stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(entries.len());
        let entries_are_compatible = entries.iter().all(|entry| {
            entry.item.as_ref().is_some_and(|item| {
                matches!(item.primitive, Primitive::Bezier { .. })
                    && item.z == batch.first_z
                    && entry.desired.target_camera == old_batch_key.clip.target_camera
                    && ClipGroupKey::new(item.clip, entry.desired.target_camera)
                        == old_batch_key.clip
            })
        });
        let sorted_keys: Vec<_> = entries.iter().map(|entry| entry.desired.key).collect();
        'compatible: {
            if entries_are_compatible
                && let Some((&first_key, &last_key)) = sorted_keys.first().zip(sorted_keys.last())
            {
                let new_key = BezierBatchKey {
                    first: first_key,
                    last: last_key,
                    clip: old_batch_key.clip,
                };
                let signature: Vec<_> = entries
                    .iter()
                    .filter_map(|entry| entry.item.clone())
                    .collect();
                let first = &entries[0];
                let local_z = desired_local_z(
                    &first.desired,
                    &cache.layout_order,
                    &cache.layout_order_labels,
                    &cache.widget_order_labels,
                );
                let order_changed = sorted_keys != batch.keys || new_key != old_batch_key;
                let local_z_changed = batch.local_z != local_z;
                if order_changed || local_z_changed {
                    let Some(values) = build_bezier_batch_values(&signature, new_key.clip.clip())
                    else {
                        break 'compatible;
                    };
                    if new_key != old_batch_key {
                        if let Some(previous) = cache.bezier_batches.remove(&old_batch_key) {
                            cache.bezier_batches.insert(new_key, previous);
                        }
                        for key in &batch.keys {
                            cache.bezier_batch_for_key.insert(*key, new_key);
                        }
                        for batch_keys in cache.fragment_batches.values_mut() {
                            for key in batch_keys.iter_mut() {
                                if *key == old_batch_key {
                                    *key = new_key;
                                }
                            }
                        }
                    }
                    if let (Some(materials), Some(buffers)) =
                        (bezier_batch_materials.as_mut(), shader_buffers.as_mut())
                    {
                        let generation = cache.generation;
                        let updated = BezierBatchSyncContext {
                            commands: &mut commands,
                            cache: &mut cache,
                            materials,
                            buffers,
                            generation,
                        }
                        .sync(
                            new_key,
                            sorted_keys.clone(),
                            signature,
                            values,
                            first.desired.z,
                            local_z,
                        );
                        if updated {
                            stats.updated_render_entries =
                                stats.updated_render_entries.saturating_add(1);
                        }
                    }
                }
                continue 'batches;
            }
        }

        for key in &batch.keys {
            cache.bezier_batch_for_key.remove(key);
        }
        for batch_keys in cache.fragment_batches.values_mut() {
            batch_keys.retain(|key| *key != old_batch_key);
        }
        if retire_bezier_batch(
            old_batch_key,
            &mut commands,
            &mut cache,
            &mut bezier_batch_materials,
            &mut shader_buffers,
        ) {
            stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
        }

        let direct_keys: HashSet<_> = direct_entities
            .iter()
            .map(|entity| RenderKey::LayoutEntity(*entity))
            .collect();
        let mut start = 0;
        while start < entries.len() {
            let Some(item) = entries[start].item.as_ref() else {
                start += 1;
                continue;
            };
            let clip = ClipGroupKey::new(item.clip, entries[start].desired.target_camera);
            let first_z = item.z;
            let mut end = start + 1;
            while end < entries.len() {
                let Some(next_item) = entries[end].item.as_ref() else {
                    break;
                };
                if !matches!(next_item.primitive, Primitive::Bezier { .. })
                    || next_item.z != first_z
                    || entries[end].desired.target_camera != clip.target_camera
                    || ClipGroupKey::new(next_item.clip, entries[end].desired.target_camera) != clip
                {
                    break;
                }
                end += 1;
            }
            if !matches!(item.primitive, Primitive::Bezier { .. })
                || !valid_clip(item.clip)
                || end - start < 2
            {
                if let Some(item) = entries[start].item.as_ref()
                    && !direct_keys.contains(&entries[start].desired.key)
                {
                    spawn_incremental_item(
                        entries[start].desired.key,
                        entries[start].desired,
                        item,
                        &mut commands,
                        &mut cache,
                        &images,
                        &mut hsv_wheel_materials,
                        &mut bezier_materials,
                        text_font_size,
                        text_line_height,
                        &default_text_font,
                    );
                    stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
                }
                start += 1;
                continue;
            }
            let run = &entries[start..end];
            let signature: Vec<_> = run.iter().filter_map(|entry| entry.item.clone()).collect();
            let keys: Vec<_> = run.iter().map(|entry| entry.desired.key).collect();
            let Some((&first, &last)) = keys.first().zip(keys.last()) else {
                start = end;
                continue;
            };
            let key = BezierBatchKey { first, last, clip };
            if let (Some(values), Some(materials), Some(buffers)) = (
                build_bezier_batch_values(&signature, clip.clip()),
                bezier_batch_materials.as_mut(),
                shader_buffers.as_mut(),
            ) {
                let local_z = desired_local_z(
                    &run[0].desired,
                    &cache.layout_order,
                    &cache.layout_order_labels,
                    &cache.widget_order_labels,
                );
                let generation = cache.generation;
                let updated = BezierBatchSyncContext {
                    commands: &mut commands,
                    cache: &mut cache,
                    materials,
                    buffers,
                    generation,
                }
                .sync(key, keys.clone(), signature, values, first_z, local_z);
                if cache.bezier_batches.contains_key(&key) {
                    *cache.clip_group_users.entry(clip).or_default() += 1;
                    for member in &keys {
                        cache.bezier_batch_for_key.insert(*member, key);
                    }
                    if let Some(owner) = keys.iter().find_map(|member| match member {
                        RenderKey::WidgetFragment(entity, _) => Some(*entity),
                        _ => None,
                    }) {
                        cache.fragment_batches.entry(owner).or_default().push(key);
                    }
                }
                if updated {
                    stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
                }
            }
            start = end;
        }
    }

    for entity in direct_entities {
        if let Ok((_, layout, paint)) = paint_query.get(entity) {
            let key = RenderKey::LayoutEntity(entity);
            if !cache.layout_order.contains_key(&entity) {
                continue;
            }
            let target = camera_query.get(layout.window).ok().map(|_| layout.window);
            let item = paint.draw_item(&layout);
            update_layout_window(&mut cache, entity, Some(layout.window));
            if let Some(batch_key) = cache.bezier_batch_for_key.get(&key).copied() {
                let Some(batch) = cache.bezier_batches.get(&batch_key) else {
                    continue;
                };
                let Some(index) = batch.keys.iter().position(|member| *member == key) else {
                    continue;
                };
                let mut signature = batch.signature.clone();
                signature[index] = item;
                let keys = batch.keys.clone();
                let local_z = batch.local_z;
                let first_z = batch.first_z;
                let Some(values) = build_bezier_batch_values(&signature, batch_key.clip.clip())
                else {
                    continue;
                };
                if let (Some(materials), Some(buffers)) =
                    (bezier_batch_materials.as_mut(), shader_buffers.as_mut())
                {
                    let generation = cache.generation;
                    let updated = BezierBatchSyncContext {
                        commands: &mut commands,
                        cache: &mut cache,
                        materials,
                        buffers,
                        generation,
                    }
                    .sync(batch_key, keys, signature, values, first_z, local_z);
                    stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(1);
                    if updated {
                        stats.updated_render_entries =
                            stats.updated_render_entries.saturating_add(1);
                    }
                }
                continue;
            }
            let desired = DesiredItem {
                key,
                z: item.z,
                target_camera: target,
                hierarchy_depth: hierarchy_depth(entity, &cache.parents_by_entity),
                fragment_order: None,
            };
            let local_z = desired_local_z(
                &desired,
                &cache.layout_order,
                &cache.layout_order_labels,
                &cache.widget_order_labels,
            );
            let clip_group = valid_clip(item.clip).then(|| ClipGroupKey::new(item.clip, target));
            let resolved_image = image_handle(&images, &item.primitive).cloned();
            let resolved_image_asset_id = image_asset_id(&item.primitive);
            let item_font_size = uses_theme_font_size(&item.primitive).then_some(text_font_size);
            let item_line_height =
                uses_theme_line_height(&item.primitive).then_some(text_line_height);
            let wheel_values = match (&item.primitive, clip_group) {
                (Primitive::HsvWheel { rect, .. }, Some(_)) if valid_clip(*rect) => {
                    hsv_wheel_parameters(&item.primitive)
                }
                _ => None,
            };
            let bezier_values = match (&item.primitive, clip_group) {
                (Primitive::Bezier { .. } | Primitive::BezierGradient { .. }, Some(_)) => {
                    bezier_material_values(&item)
                }
                _ => None,
            };
            let unchanged = cache.items.get(&key).is_some_and(|cached| {
                cached.item == item
                    && cached.target_camera == target
                    && cached.clip_group == clip_group
                    && cached.text_font_size == item_font_size
                    && cached.text_line_height == item_line_height
                    && cached.image == resolved_image
                    && cached.image_asset_id == resolved_image_asset_id
                    && cached.hsv_wheel_values == wheel_values
                    && cached.bezier_values == bezier_values
            });
            if unchanged {
                let mut local_z_changed = false;
                if let Some(cached) = cache.items.get_mut(&key)
                    && cached.local_z != local_z
                {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                    local_z_changed = true;
                }
                stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(1);
                if local_z_changed {
                    stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
                }
                continue;
            }
            if remove_cached_item(
                key,
                &mut commands,
                &mut cache,
                &mut hsv_wheel_materials,
                &mut bezier_materials,
            ) {
                stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
            }
            spawn_incremental_item(
                key,
                desired,
                &item,
                &mut commands,
                &mut cache,
                &images,
                &mut hsv_wheel_materials,
                &mut bezier_materials,
                text_font_size,
                text_line_height,
                &default_text_font,
            );
            stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(1);
            stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
        } else {
            cache.layout_order.remove(&entity);
            if let Some(key) = cache.layout_entity_order_keys.remove(&entity) {
                cache.layout_order_labels.remove(&key);
            }
            update_layout_window(&mut cache, entity, None);
            let key = RenderKey::LayoutEntity(entity);
            if remove_cached_item(
                key,
                &mut commands,
                &mut cache,
                &mut hsv_wheel_materials,
                &mut bezier_materials,
            ) {
                stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
            }
        }
    }
    stats.incremental_syncs = stats.incremental_syncs.saturating_add(1);
}

use super::*;

pub(super) fn sync_display_list_full(sync: RendererSync, changes: bui_core::UiDirtyBatch) {
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
        hierarchy_query,
        dirty: _,
        mut cache,
        mut stats,
    } = sync;
    let _ = changes;
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
    let mut desired = std::mem::take(&mut cache.desired);
    desired.clear();
    desired.reserve(display_list.item_count());
    cache.layout_entities_by_window.clear();
    cache.layout_window_by_entity.clear();
    cache.widget_fragments_by_camera.clear();
    cache.widget_fragment_camera.clear();
    rebuild_cached_hierarchy(&mut cache, &hierarchy_query);
    let hierarchy_parents: HashMap<_, _> = hierarchy_query
        .iter()
        .filter_map(|(entity, parent)| parent.map(|parent| (entity, parent.0)))
        .collect();
    for (index, item) in display_list.manual_items.iter().enumerate() {
        desired.push(DesiredItem {
            key: RenderKey::ManualDisplayListIndex(index),
            z: item.z,
            target_camera: target_camera.0,
            hierarchy_depth: 0,
            fragment_order: None,
        });
    }
    for (index, item) in display_list.widget_items.iter().enumerate() {
        desired.push(DesiredItem {
            key: RenderKey::WidgetDisplayListIndex(index),
            z: item.z,
            target_camera: None,
            hierarchy_depth: 0,
            fragment_order: None,
        });
    }
    for (index, item) in display_list.overlay_widget_items.iter().enumerate() {
        desired.push(DesiredItem {
            key: RenderKey::OverlayWidgetDisplayListIndex(index),
            z: item.z,
            target_camera: None,
            hierarchy_depth: 0,
            fragment_order: None,
        });
    }
    for fragment in display_list.widget_fragments.values() {
        update_widget_fragment_camera(&mut cache, fragment.entity, fragment.target_camera);
        for (index, item) in fragment.items.iter().enumerate() {
            desired.push(DesiredItem {
                key: RenderKey::WidgetFragment(fragment.entity, index),
                z: item.z,
                target_camera: fragment.target_camera,
                hierarchy_depth: 0,
                fragment_order: Some(fragment.order),
            });
        }
    }
    for (camera, items) in &display_list.camera_items {
        for (index, item) in items.iter().enumerate() {
            desired.push(DesiredItem {
                key: RenderKey::CameraDisplayListIndex(*camera, index),
                z: item.z,
                target_camera: Some(*camera),
                hierarchy_depth: 0,
                fragment_order: None,
            });
        }
    }
    for (camera, items) in &display_list.overlay_camera_items {
        for (index, item) in items.iter().enumerate() {
            desired.push(DesiredItem {
                key: RenderKey::OverlayCameraDisplayListIndex(*camera, index),
                z: item.z,
                target_camera: Some(*camera),
                hierarchy_depth: 0,
                fragment_order: None,
            });
        }
    }
    for (entity, layout, _) in &paint_query {
        update_layout_window(&mut cache, entity, Some(layout.window));
        let camera = camera_query.get(layout.window).ok().map(|_| layout.window);
        desired.push(DesiredItem {
            key: RenderKey::LayoutEntity(entity),
            z: layout.z,
            target_camera: camera,
            hierarchy_depth: hierarchy_depth(entity, &hierarchy_parents),
            fragment_order: None,
        });
    }
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
    rebuild_widget_order_labels(&mut cache, &desired);
    cache.generation = cache.generation.wrapping_add(1);
    let generation = cache.generation;
    cache.fragment_keys.clear();
    cache.fragment_batches.clear();
    cache.bezier_batch_for_key.clear();
    cache.layout_order.clear();
    let mut next_layout_order = 0usize;
    for item in &desired {
        match item.key {
            RenderKey::WidgetFragment(entity, _) => {
                cache
                    .fragment_keys
                    .entry(entity)
                    .or_default()
                    .push(item.key);
            }
            RenderKey::LayoutEntity(entity) => {
                cache.layout_order.insert(entity, next_layout_order);
                next_layout_order = next_layout_order.saturating_add(1);
            }
            _ => {}
        }
    }
    rebuild_layout_order_labels(&mut cache, &desired);
    let mut retired_hsv_materials = Vec::new();
    let mut retired_bezier_materials = Vec::new();
    let mut retired_bezier_batches = Vec::new();
    let mut updated_render_entries = 0usize;
    let resolved_items: Vec<Option<DrawItem>> = desired
        .iter()
        .map(|desired_item| match desired_item.key {
            RenderKey::ManualDisplayListIndex(index) => {
                display_list.manual_items.get(index).cloned()
            }
            RenderKey::WidgetDisplayListIndex(index) => {
                display_list.widget_items.get(index).cloned()
            }
            RenderKey::OverlayWidgetDisplayListIndex(index) => {
                display_list.overlay_widget_items.get(index).cloned()
            }
            RenderKey::CameraDisplayListIndex(camera, index) => display_list
                .camera_items
                .get(&camera)
                .and_then(|items| items.get(index))
                .cloned(),
            RenderKey::OverlayCameraDisplayListIndex(camera, index) => display_list
                .overlay_camera_items
                .get(&camera)
                .and_then(|items| items.get(index))
                .cloned(),
            RenderKey::WidgetFragment(entity, index) => display_list
                .widget_fragments
                .get(&entity)
                .and_then(|fragment| fragment.items.get(index))
                .cloned(),
            RenderKey::LayoutEntity(entity) => paint_query
                .get(entity)
                .ok()
                .map(|(_, layout, paint)| paint.draw_item(&layout)),
        })
        .collect();
    let mut batched_keys = HashSet::new();
    if let (Some(materials), Some(buffers)) =
        (bezier_batch_materials.as_mut(), shader_buffers.as_mut())
    {
        let mut start = 0;
        while start < desired.len() {
            let Some(item) = resolved_items[start].as_ref() else {
                start += 1;
                continue;
            };
            let fragment_owner = match desired[start].key {
                RenderKey::WidgetFragment(entity, _) => Some(entity),
                _ => None,
            };
            let batch_group = match desired[start].key {
                RenderKey::WidgetFragment(entity, _) => {
                    display_list.widget_fragment_batch_group(entity)
                }
                _ => None,
            };
            let Some(clip) = valid_clip(item.clip)
                .then(|| ClipGroupKey::new(item.clip, desired[start].target_camera))
            else {
                start += 1;
                continue;
            };
            if !matches!(&item.primitive, Primitive::Bezier { .. }) {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < desired.len() {
                let Some(next_item) = resolved_items[end].as_ref() else {
                    break;
                };
                let next_fragment_owner = match desired[end].key {
                    RenderKey::WidgetFragment(entity, _) => Some(entity),
                    _ => None,
                };
                let next_batch_group = match desired[end].key {
                    RenderKey::WidgetFragment(entity, _) => {
                        display_list.widget_fragment_batch_group(entity)
                    }
                    _ => None,
                };
                let same_batch_scope = match (batch_group, next_batch_group) {
                    (Some(first), Some(next)) => first == next,
                    (None, None) => next_fragment_owner == fragment_owner,
                    _ => false,
                };
                if desired[end].target_camera != desired[start].target_camera
                    || next_item.z != item.z
                    || !same_batch_scope
                    || !matches!(&next_item.primitive, Primitive::Bezier { .. })
                    || !valid_clip(next_item.clip)
                    || ClipGroupKey::new(next_item.clip, desired[end].target_camera) != clip
                    || end - start >= MAX_BEZIER_BATCH_CURVES
                {
                    break;
                }
                end += 1;
            }
            if end - start >= 2 {
                let key = BezierBatchKey {
                    first: desired[start].key,
                    last: desired[end - 1].key,
                    clip,
                };
                let signature_unchanged = cache.bezier_batches.get(&key).is_some_and(|cached| {
                    cached.signature.len() == end - start
                        && resolved_items[start..end]
                            .iter()
                            .zip(&cached.signature)
                            .all(|(item, previous)| item.as_ref() == Some(previous))
                });
                let local_z = desired_local_z(
                    &desired[start],
                    &cache.layout_order,
                    &cache.layout_order_labels,
                    &cache.widget_order_labels,
                );
                if signature_unchanged {
                    if let Some(cached) = cache.bezier_batches.get_mut(&key) {
                        if cached.local_z != local_z || cached.first_z != desired[start].z {
                            commands
                                .entity(cached.entity)
                                .insert((GlobalZIndex(desired[start].z), ZIndex(local_z)));
                            updated_render_entries = updated_render_entries.saturating_add(1);
                            cached.local_z = local_z;
                        }
                        cached.first_z = desired[start].z;
                        cached.generation = generation;
                    }
                    if let Some(group) = cache.clip_groups.get_mut(&clip) {
                        group.generation = generation;
                    }
                    let members: HashSet<_> = desired[start..end]
                        .iter()
                        .filter_map(|entry| match entry.key {
                            RenderKey::WidgetFragment(entity, _) => Some(entity),
                            _ => None,
                        })
                        .collect();
                    for entity in members {
                        cache.fragment_batches.entry(entity).or_default().push(key);
                    }
                    for item in &desired[start..end] {
                        cache.bezier_batch_for_key.insert(item.key, key);
                    }
                    batched_keys.extend(desired[start..end].iter().map(|item| item.key));
                } else {
                    let signature: Vec<_> = resolved_items[start..end]
                        .iter()
                        .filter_map(Clone::clone)
                        .collect();
                    if let Some(values) = build_bezier_batch_values(&signature, clip.clip()) {
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
                        if updated {
                            updated_render_entries = updated_render_entries.saturating_add(1);
                        }
                        let members: HashSet<_> = desired[start..end]
                            .iter()
                            .filter_map(|entry| match entry.key {
                                RenderKey::WidgetFragment(entity, _) => Some(entity),
                                _ => None,
                            })
                            .collect();
                        for entity in members {
                            cache.fragment_batches.entry(entity).or_default().push(key);
                        }
                        for entry in &desired[start..end] {
                            cache.bezier_batch_for_key.insert(entry.key, key);
                        }
                        batched_keys.extend(desired[start..end].iter().map(|item| item.key));
                    }
                }
                start = end;
            } else {
                start += 1;
            }
        }
    }
    for (order, desired_item) in desired.drain(..).enumerate() {
        let DesiredItem {
            key,
            target_camera: target,
            z,
            hierarchy_depth,
            fragment_order,
        } = desired_item;
        if batched_keys.contains(&key) {
            continue;
        }
        let Some(item) = resolved_items[order].as_ref() else {
            continue;
        };
        let local_z = desired_local_z(
            &DesiredItem {
                key,
                z,
                target_camera: target,
                hierarchy_depth,
                fragment_order,
            },
            &cache.layout_order,
            &cache.layout_order_labels,
            &cache.widget_order_labels,
        );
        let clip_group = valid_clip(item.clip).then(|| ClipGroupKey::new(item.clip, target));
        let clip_group_entity = if let Some(group_key) = clip_group {
            if let Some(group) = cache.clip_groups.get_mut(&group_key) {
                group.generation = generation;
                Some(group.entity)
            } else {
                let entity = spawn_clip_group(&mut commands, group_key);
                cache
                    .clip_groups
                    .insert(group_key, CachedClipGroup { entity, generation });
                Some(entity)
            }
        } else {
            None
        };

        let resolved_image = image_handle(&images, &item.primitive).cloned();
        let resolved_image_asset_id = image_asset_id(&item.primitive);
        let wheel_values = match (&item.primitive, clip_group_entity) {
            (Primitive::HsvWheel { rect, .. }, Some(_)) if valid_clip(*rect) => {
                hsv_wheel_parameters(&item.primitive)
            }
            _ => None,
        };
        let bezier_values = match (&item.primitive, clip_group_entity) {
            (Primitive::Bezier { .. }, Some(_)) => bezier_material_values(item),
            _ => None,
        };
        let item_font_size = uses_theme_font_size(&item.primitive).then_some(text_font_size);
        let item_line_height = uses_theme_line_height(&item.primitive).then_some(text_line_height);
        let unchanged = cache.items.get(&key).is_some_and(|cached| {
            cached.item == *item
                && cached.target_camera == target
                && cached.clip_group == clip_group
                && cached.text_font_size == item_font_size
                && cached.text_line_height == item_line_height
                && cached.image == resolved_image
                && cached.hsv_wheel_values == wheel_values
                && cached.bezier_values == bezier_values
        });
        if unchanged {
            if let Some(cached) = cache.items.get_mut(&key) {
                if cached.local_z != local_z {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                    updated_render_entries = updated_render_entries.saturating_add(1);
                }
                cached.generation = generation;
            }
            continue;
        }

        let reusable_bezier_material = cache.items.get(&key).and_then(|cached| {
            (bezier_values.is_some()
                && cached.target_camera == target
                && cached.clip_group == clip_group
                && cached.image == resolved_image)
                .then(|| cached.bezier_material.clone())
                .flatten()
        });
        let bezier_updated_in_place = reusable_bezier_material
            .as_ref()
            .zip(bezier_values)
            .is_some_and(|(handle, values)| {
                bezier_materials.as_mut().is_some_and(|materials| {
                    materials.get_mut(handle).is_some_and(|mut material| {
                        material.points_01 = values.points_01;
                        material.points_23 = values.points_23;
                        material.color = values.color;
                        material.style = values.style;
                        true
                    })
                })
            });
        if bezier_updated_in_place {
            if let Some(cached) = cache.items.get_mut(&key) {
                cached.item = item.clone();
                cached.bezier_values = bezier_values;
                if let Some(values) = bezier_values
                    && let Some(node) = cached.nodes.first()
                {
                    commands
                        .entity(node)
                        .insert(positioned_node(values.rect, item.clip));
                }
                if cached.local_z != local_z {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                }
                cached.generation = generation;
            }
            updated_render_entries = updated_render_entries.saturating_add(1);
            continue;
        }

        let reusable_wheel_material = cache.items.get(&key).and_then(|cached| {
            (wheel_values.is_some()
                && same_hsv_wheel_geometry(&cached.item, item)
                && cached.target_camera == target
                && cached.clip_group == clip_group
                && cached.image == resolved_image
                && cached.text_font_size == item_font_size
                && cached.text_line_height == item_line_height)
                .then(|| cached.hsv_material.clone())
                .flatten()
        });
        let wheel_updated_in_place = reusable_wheel_material
            .as_ref()
            .zip(wheel_values)
            .is_some_and(|(handle, (hue_degrees, saturation, value))| {
                hsv_wheel_materials.as_mut().is_some_and(|materials| {
                    materials.get_mut(handle).is_some_and(|mut material| {
                        material.parameters = Vec4::new(hue_degrees, saturation, value, 0.0);
                        true
                    })
                })
            });
        if wheel_updated_in_place {
            if let Some(cached) = cache.items.get_mut(&key) {
                cached.item = item.clone();
                cached.hsv_wheel_values = wheel_values;
                if cached.local_z != local_z {
                    cached.nodes.set_local_z(&mut commands, local_z);
                    cached.local_z = local_z;
                }
                cached.generation = generation;
            }
            updated_render_entries = updated_render_entries.saturating_add(1);
            continue;
        }

        let mut previous_hsv_material = None;
        let mut previous_bezier_material = None;
        if let Some(previous) = cache.items.remove(&key) {
            previous_hsv_material = previous.hsv_material.clone();
            previous_bezier_material = previous.bezier_material.clone();
            previous.nodes.despawn(&mut commands);
        }

        let mut nodes = RenderedNodes::default();
        let hsv_material = wheel_values.and_then(|(hue_degrees, saturation, value)| {
            let parameters = Vec4::new(hue_degrees, saturation, value, 0.0);
            if let Some(handle) = previous_hsv_material.as_ref() {
                let materials = hsv_wheel_materials.as_mut()?;
                materials.get_mut(handle)?.parameters = parameters;
                Some(handle.clone())
            } else {
                hsv_wheel_materials
                    .as_mut()
                    .map(|materials| materials.add(HsvWheelMaterial { parameters }))
            }
        });
        if let Some(handle) = previous_hsv_material.filter(|_| hsv_material.is_none()) {
            retired_hsv_materials.push(handle);
        }
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
        if let Some(handle) = previous_bezier_material {
            retired_bezier_materials.push(handle);
        }
        if let Some(group) = clip_group_entity {
            commands.entity(group).with_children(|parent| {
                spawn_primitive(
                    parent,
                    item,
                    images.as_ref(),
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
                target_camera: target,
                text_font_size: item_font_size,
                text_line_height: item_line_height,
                image: resolved_image,
                image_asset_id: resolved_image_asset_id,
                hsv_wheel_values: wheel_values,
                hsv_material,
                bezier_values,
                bezier_material,
                generation,
            },
        );
        updated_render_entries = updated_render_entries.saturating_add(1);
    }

    cache.items.retain(|_, cached| {
        if cached.generation == generation {
            return true;
        }
        cached.nodes.for_each(|node| {
            commands.entity(node).despawn();
        });
        if let Some(material) = &cached.hsv_material {
            retired_hsv_materials.push(material.clone());
        }
        if let Some(material) = &cached.bezier_material {
            retired_bezier_materials.push(material.clone());
        }
        updated_render_entries = updated_render_entries.saturating_add(1);
        false
    });
    cache.bezier_batches.retain(|_, batch| {
        if batch.generation == generation {
            return true;
        }
        commands.entity(batch.entity).despawn();
        updated_render_entries = updated_render_entries.saturating_add(1);
        retired_bezier_batches.push((
            batch.material.clone(),
            batch.segments.clone(),
            batch.curves.clone(),
            batch.tile_indices.clone(),
        ));
        false
    });
    cache.clip_groups.retain(|_, group| {
        if group.generation == generation {
            return true;
        }
        commands.entity(group.entity).despawn();
        false
    });
    cache.clip_group_users.clear();
    let clip_users: Vec<_> = cache
        .items
        .values()
        .filter_map(|cached| cached.clip_group)
        .chain(cache.bezier_batches.keys().map(|key| key.clip))
        .collect();
    for key in clip_users {
        *cache.clip_group_users.entry(key).or_default() += 1;
    }
    cache.image_users.clear();
    let image_users: Vec<_> = cache
        .items
        .iter()
        .filter_map(|(key, cached)| cached.image_asset_id.map(|asset_id| (asset_id, *key)))
        .collect();
    for (asset_id, key) in image_users {
        cache.image_users.entry(asset_id).or_default().insert(key);
    }
    if let Some(materials) = hsv_wheel_materials.as_mut() {
        for handle in retired_hsv_materials {
            materials.remove(handle.id());
        }
    }
    if let Some(materials) = bezier_materials.as_mut() {
        for handle in retired_bezier_materials {
            materials.remove(handle.id());
        }
    }
    if let Some(materials) = bezier_batch_materials.as_mut() {
        for (material, _, _, _) in &retired_bezier_batches {
            materials.remove(material.id());
        }
    }
    if let Some(buffers) = shader_buffers.as_mut() {
        for (_, segments, curves, tile_indices) in retired_bezier_batches {
            buffers.remove(segments.id());
            buffers.remove(curves.id());
            buffers.remove(tile_indices.id());
        }
    }
    cache.desired = desired;
    cache.target_camera = target_camera.0;
    stats.scanned_draw_keys = resolved_items.len();
    stats.updated_render_entries = updated_render_entries;
    stats.full_syncs = stats.full_syncs.saturating_add(1);
}

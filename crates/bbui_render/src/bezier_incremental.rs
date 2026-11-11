use super::*;

#[expect(
    clippy::too_many_arguments,
    reason = "A graph Bezier batch is rebuilt from only one bounded fragment group"
)]
pub(super) fn sync_widget_fragment_bezier_group(
    group: WidgetFragmentBatchGroup,
    display_list: &DisplayList,
    images: &ImageRegistry,
    commands: &mut Commands,
    cache: &mut RenderCache,
    stats: &mut RenderSyncStats,
    materials: &mut Option<ResMut<Assets<BezierBatchMaterial>>>,
    buffers: &mut Option<ResMut<Assets<ShaderBuffer>>>,
    hsv_wheel_materials: &mut Option<ResMut<Assets<HsvWheelMaterial>>>,
    bezier_materials: &mut Option<ResMut<Assets<BezierMaterial>>>,
    text_font_size: f32,
    text_line_height: f32,
    default_text_font: &TextFont,
) {
    let mut entries = Vec::new();
    for entity in display_list.widget_batch_group_members(group) {
        let Some(fragment) = display_list.widget_fragments.get(&entity) else {
            continue;
        };
        for (index, item) in fragment.items.iter().enumerate() {
            let desired = DesiredItem {
                key: RenderKey::WidgetFragment(entity, index),
                z: item.z,
                target_camera: fragment.target_camera,
                hierarchy_depth: 0,
                fragment_order: Some(fragment.order),
            };
            entries.push((desired, item.clone()));
        }
    }
    entries.sort_unstable_by(|(first, _), (second, _)| {
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

    let mut start = 0;
    while start < entries.len() {
        let (first_desired, first_item) = &entries[start];
        let clip = valid_clip(first_item.clip)
            .then(|| ClipGroupKey::new(first_item.clip, first_desired.target_camera));
        let can_batch = clip.is_some() && matches!(first_item.primitive, Primitive::Bezier { .. });
        let mut end = start + 1;
        if can_batch {
            while end < entries.len() && end - start < MAX_BEZIER_BATCH_CURVES {
                let (desired, item) = &entries[end];
                if desired.target_camera != first_desired.target_camera
                    || item.z != first_item.z
                    || !matches!(item.primitive, Primitive::Bezier { .. })
                    || !valid_clip(item.clip)
                    || Some(ClipGroupKey::new(item.clip, desired.target_camera)) != clip
                {
                    break;
                }
                end += 1;
            }
        }

        let run = &entries[start..end];
        stats.scanned_draw_keys = stats.scanned_draw_keys.saturating_add(run.len());
        let run_items: Vec<_> = run.iter().map(|(_, item)| item.clone()).collect();
        let run_keys: Vec<_> = run.iter().map(|(desired, _)| desired.key).collect();
        let batch_key = run_keys
            .first()
            .zip(run_keys.last())
            .zip(clip)
            .filter(|_| run.len() >= 2)
            .map(|((&first, &last), clip)| BezierBatchKey { first, last, clip });
        let batch_values = batch_key.and_then(|key| {
            build_bezier_batch_values(&run_items, key.clip.clip()).map(|values| (key, values))
        });
        if let Some((key, values)) = batch_values
            && let (Some(materials), Some(buffers)) = (materials.as_mut(), buffers.as_mut())
        {
            let local_z = desired_local_z(
                &run[0].0,
                &cache.layout_order,
                &cache.layout_order_labels,
                &cache.widget_order_labels,
            );
            let clip = key.clip;
            let reused_batch = cache.bezier_batches.contains_key(&key);
            let generation = cache.generation;
            let updated = BezierBatchSyncContext {
                commands,
                cache,
                materials,
                buffers,
                generation,
            }
            .sync(
                key,
                run_keys.clone(),
                run_items,
                values,
                run[0].0.z,
                local_z,
            );
            if cache.bezier_batches.contains_key(&key) {
                if !reused_batch {
                    *cache.clip_group_users.entry(clip).or_default() += 1;
                }
                if updated {
                    stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
                }
                let mut member_fragments = HashSet::new();
                for member in run_keys {
                    cache.bezier_batch_for_key.insert(member, key);
                    if let RenderKey::WidgetFragment(entity, _) = member
                        && member_fragments.insert(entity)
                    {
                        cache.fragment_batches.entry(entity).or_default().push(key);
                    }
                }
                start = end;
                continue;
            }
        }

        for (desired, item) in run {
            spawn_incremental_item(
                desired.key,
                *desired,
                item,
                commands,
                cache,
                images,
                hsv_wheel_materials,
                bezier_materials,
                text_font_size,
                text_line_height,
                default_text_font,
            );
            stats.updated_render_entries = stats.updated_render_entries.saturating_add(1);
        }
        start = end;
    }
}

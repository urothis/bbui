use super::*;

pub(super) fn spawn_clip_group(commands: &mut Commands, key: ClipGroupKey) -> Entity {
    let mut group = commands.spawn((RenderedClipGroup, clip_node(key.clip())));
    if let Some(camera) = key.target_camera {
        group.insert(UiTargetCamera(camera));
    }
    group.id()
}

pub(super) fn image_handle<'a>(
    images: &'a ImageRegistry,
    primitive: &Primitive,
) -> Option<&'a Handle<Image>> {
    match primitive {
        Primitive::Image { asset_id, .. } | Primitive::Icon { asset_id, .. } => {
            images.get(*asset_id)
        }
        _ => None,
    }
}

pub(super) fn image_asset_id(primitive: &Primitive) -> Option<u64> {
    match primitive {
        Primitive::Image { asset_id, .. } | Primitive::Icon { asset_id, .. } => Some(*asset_id),
        _ => None,
    }
}

pub(super) fn hsv_wheel_parameters(primitive: &Primitive) -> Option<(f32, f32, f32)> {
    let Primitive::HsvWheel {
        hue_degrees,
        saturation,
        value,
        ..
    } = primitive
    else {
        return None;
    };
    if !hue_degrees.is_finite() || !saturation.is_finite() || !value.is_finite() {
        return None;
    }
    Some((
        hue_degrees.rem_euclid(360.0),
        saturation.clamp(0.0, 1.0),
        value.clamp(0.0, 1.0),
    ))
}

pub(super) fn bezier_material_values(item: &DrawItem) -> Option<BezierMaterialValues> {
    let (points, width, color, end_color) = match &item.primitive {
        Primitive::Bezier {
            points,
            width,
            color,
        } => (points, width, color, color),
        Primitive::BezierGradient {
            points,
            width,
            start_color,
            end_color,
        } => (points, width, start_color, end_color),
        _ => return None,
    };
    if !points.iter().flatten().all(|value| value.is_finite())
        || !width.is_finite()
        || *width <= 0.0
        || ![color.0, color.1, color.2, color.3]
            .iter()
            .all(|channel| channel.is_finite())
        || ![end_color.0, end_color.1, end_color.2, end_color.3]
            .iter()
            .all(|channel| channel.is_finite())
    {
        return None;
    }

    let padding = width * 0.5 + 1.25;
    let min_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::INFINITY, f32::min)
        - padding;
    let min_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min)
        - padding;
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max)
        + padding;
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f32::NEG_INFINITY, f32::max)
        + padding;
    let rect = ClipRect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    };
    if !valid_clip(rect) {
        return None;
    }

    let local = points.map(|point| [point[0] - rect.x, point[1] - rect.y]);
    Some(BezierMaterialValues {
        rect,
        points_01: Vec4::new(local[0][0], local[0][1], local[1][0], local[1][1]),
        points_23: Vec4::new(local[2][0], local[2][1], local[3][0], local[3][1]),
        color: Vec4::new(
            color.0.clamp(0.0, 1.0),
            color.1.clamp(0.0, 1.0),
            color.2.clamp(0.0, 1.0),
            color.3.clamp(0.0, 1.0),
        ),
        end_color: Vec4::new(
            end_color.0.clamp(0.0, 1.0),
            end_color.1.clamp(0.0, 1.0),
            end_color.2.clamp(0.0, 1.0),
            end_color.3.clamp(0.0, 1.0),
        ),
        style: Vec4::new(*width, 0.0, 0.0, 0.0),
    })
}

pub(super) fn intersect_clip(first: ClipRect, second: ClipRect) -> Option<ClipRect> {
    let left = first.x.max(second.x);
    let top = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    let rect = ClipRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    valid_clip(rect).then_some(rect)
}

pub(super) fn cubic_bezier_point(points: [[f32; 2]; 4], t: f32) -> [f32; 2] {
    let inverse = 1.0 - t;
    let weights = [
        inverse * inverse * inverse,
        3.0 * inverse * inverse * t,
        3.0 * inverse * t * t,
        t * t * t,
    ];
    [
        points
            .iter()
            .zip(weights)
            .map(|(point, weight)| point[0] * weight)
            .sum(),
        points
            .iter()
            .zip(weights)
            .map(|(point, weight)| point[1] * weight)
            .sum(),
    ]
}

pub(super) fn build_bezier_batch_values(
    items: &[DrawItem],
    clip: ClipRect,
) -> Option<BezierBatchValues> {
    let mut bounded_curves = Vec::with_capacity(items.len());
    let mut bounds: Option<ClipRect> = None;
    for item in items {
        let values = bezier_material_values(item)?;
        let Some(clipped) = intersect_clip(values.rect, clip) else {
            continue;
        };
        let next = if let Some(bounds) = bounds {
            let right = (bounds.x + bounds.width).max(clipped.x + clipped.width);
            let bottom = (bounds.y + bounds.height).max(clipped.y + clipped.height);
            ClipRect {
                x: bounds.x.min(clipped.x),
                y: bounds.y.min(clipped.y),
                width: right - bounds.x.min(clipped.x),
                height: bottom - bounds.y.min(clipped.y),
            }
        } else {
            clipped
        };
        bounds = Some(next);
        bounded_curves.push((item, values));
    }
    let rect = bounds?;
    if !valid_clip(rect) {
        return None;
    }

    let columns = (rect.width / BEZIER_BATCH_TILE_SIZE).ceil().max(1.0) as usize;
    let rows = (rect.height / BEZIER_BATCH_TILE_SIZE).ceil().max(1.0) as usize;
    let tile_count = columns.checked_mul(rows)?;
    if tile_count > MAX_BEZIER_BATCH_TILES {
        return None;
    }
    let mut tile_segments: Vec<Vec<u32>> = (0..tile_count).map(|_| Vec::new()).collect();
    let mut segments = Vec::with_capacity(bounded_curves.len().saturating_mul(32 * 4));
    let mut curves = Vec::with_capacity(bounded_curves.len().saturating_mul(8));
    for (curve_index, (item, values)) in bounded_curves.iter().enumerate() {
        let Primitive::Bezier {
            points,
            width,
            color: _,
        } = &item.primitive
        else {
            return None;
        };
        let curve_index = u32::try_from(curve_index).ok()?;
        curves.extend([
            values.color.x,
            values.color.y,
            values.color.z,
            values.color.w,
            *width,
            0.0,
            0.0,
            0.0,
        ]);
        let mut previous = cubic_bezier_point(*points, 0.0);
        for segment in 0..32u32 {
            let current = cubic_bezier_point(*points, (segment + 1) as f32 / 32.0);
            let segment_index = curve_index.checked_mul(32)?.checked_add(segment)?;
            segments.extend([
                previous[0] - rect.x,
                previous[1] - rect.y,
                current[0] - rect.x,
                current[1] - rect.y,
            ]);
            let padding = *width * 0.5 + 1.25;
            let segment_bounds = ClipRect {
                x: previous[0].min(current[0]) - padding,
                y: previous[1].min(current[1]) - padding,
                width: (previous[0] - current[0]).abs() + padding * 2.0,
                height: (previous[1] - current[1]).abs() + padding * 2.0,
            };
            if let Some(segment_bounds) = intersect_clip(segment_bounds, rect) {
                let first_x = (((segment_bounds.x - rect.x) / BEZIER_BATCH_TILE_SIZE).floor()
                    as usize)
                    .min(columns - 1);
                let first_y = (((segment_bounds.y - rect.y) / BEZIER_BATCH_TILE_SIZE).floor()
                    as usize)
                    .min(rows - 1);
                let last_x = (((segment_bounds.x + segment_bounds.width - rect.x - f32::EPSILON)
                    / BEZIER_BATCH_TILE_SIZE)
                    .floor() as usize)
                    .min(columns - 1);
                let last_y = (((segment_bounds.y + segment_bounds.height - rect.y - f32::EPSILON)
                    / BEZIER_BATCH_TILE_SIZE)
                    .floor() as usize)
                    .min(rows - 1);
                for y in first_y..=last_y {
                    for x in first_x..=last_x {
                        tile_segments[y * columns + x].push(segment_index);
                    }
                }
            }
            previous = current;
        }
    }

    let metadata_count = tile_count.checked_mul(2)?;
    let reference_count: usize = tile_segments.iter().map(Vec::len).sum();
    if reference_count > MAX_BEZIER_BATCH_TILE_REFERENCES || metadata_count > u32::MAX as usize {
        return None;
    }
    let mut tile_indices = Vec::with_capacity(metadata_count + reference_count);
    tile_indices.resize(metadata_count, 0);
    for (tile, segment_indices) in tile_segments.into_iter().enumerate() {
        let header = tile * 2;
        tile_indices[header] = u32::try_from(tile_indices.len() - metadata_count).ok()?;
        tile_indices[header + 1] = u32::try_from(segment_indices.len()).ok()?;
        tile_indices.extend(segment_indices);
    }

    Some(BezierBatchValues {
        rect,
        segments,
        curves,
        tile_indices,
        parameters: Vec4::new(
            BEZIER_BATCH_TILE_SIZE,
            columns as f32,
            rows as f32,
            metadata_count as f32,
        ),
    })
}

impl BezierBatchSyncContext<'_, '_, '_> {
    pub(super) fn sync(
        &mut self,
        key: BezierBatchKey,
        keys: Vec<RenderKey>,
        signature: Vec<DrawItem>,
        values: BezierBatchValues,
        first_z: i32,
        local_z: i32,
    ) -> bool {
        let Self {
            commands,
            cache,
            materials,
            buffers,
            generation,
        } = self;
        let generation = *generation;
        let clip = key.clip.clip();
        let group_entity = if let Some(group) = cache.clip_groups.get_mut(&key.clip) {
            group.generation = generation;
            group.entity
        } else {
            let entity = spawn_clip_group(commands, key.clip);
            cache
                .clip_groups
                .insert(key.clip, CachedClipGroup { entity, generation });
            entity
        };

        if let Some(mut cached) = cache.bezier_batches.remove(&key) {
            if cached.signature == signature {
                let changed = cached.local_z != local_z || cached.first_z != first_z;
                if changed {
                    commands
                        .entity(cached.entity)
                        .insert((GlobalZIndex(first_z), ZIndex(local_z)));
                }
                cached.first_z = first_z;
                cached.local_z = local_z;
                cached.keys = keys;
                cached.generation = generation;
                cache.bezier_batches.insert(key, cached);
                return changed;
            }
            let segments_updated = if let Some(mut buffer) = buffers.get_mut(&cached.segments) {
                buffer.clear();
                buffer.extend_from_slice(&values.segments);
                true
            } else {
                false
            };
            let curves_updated = if let Some(mut buffer) = buffers.get_mut(&cached.curves) {
                buffer.clear();
                buffer.extend_from_slice(&values.curves);
                true
            } else {
                false
            };
            let tiles_updated = if let Some(mut buffer) = buffers.get_mut(&cached.tile_indices) {
                buffer.clear();
                buffer.extend_from_slice(&values.tile_indices);
                true
            } else {
                false
            };
            let material_updated =
                materials
                    .get_mut(&cached.material)
                    .is_some_and(|mut material| {
                        material.parameters = values.parameters;
                        true
                    });
            if segments_updated && curves_updated && tiles_updated && material_updated {
                if cached.rect != values.rect
                    || cached.first_z != first_z
                    || cached.local_z != local_z
                {
                    commands.entity(cached.entity).insert((
                        positioned_node(values.rect, clip),
                        GlobalZIndex(first_z),
                        ZIndex(local_z),
                    ));
                }
                cached.signature = signature;
                cached.keys = keys;
                cached.rect = values.rect;
                cached.first_z = first_z;
                cached.local_z = local_z;
                cached.generation = generation;
                cache.bezier_batches.insert(key, cached);
                return true;
            }
            commands.entity(cached.entity).despawn();
            materials.remove(cached.material.id());
            buffers.remove(cached.segments.id());
            buffers.remove(cached.curves.id());
            buffers.remove(cached.tile_indices.id());
        }

        let segments = buffers.add(ShaderBuffer::new(
            values.segments,
            RenderAssetUsages::default(),
        ));
        let curves = buffers.add(ShaderBuffer::new(
            values.curves,
            RenderAssetUsages::default(),
        ));
        let tile_indices = buffers.add(ShaderBuffer::new(
            values.tile_indices,
            RenderAssetUsages::default(),
        ));
        let material = materials.add(BezierBatchMaterial {
            segments: segments.clone(),
            curves: curves.clone(),
            tile_indices: tile_indices.clone(),
            parameters: values.parameters,
        });
        let mut entity = None;
        commands.entity(group_entity).with_children(|parent| {
            entity = Some(spawn_item_node(
                parent,
                (
                    positioned_node(values.rect, clip),
                    MaterialNode(material.clone()),
                ),
                first_z,
                local_z,
                true,
            ));
        });
        let Some(entity) = entity else {
            return false;
        };
        cache.bezier_batches.insert(
            key,
            CachedBezierBatch {
                entity,
                material,
                segments,
                curves,
                tile_indices,
                signature,
                keys,
                rect: values.rect,
                first_z,
                local_z,
                generation,
            },
        );
        true
    }
}

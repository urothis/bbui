use super::*;

pub(super) fn desired_local_z(
    desired: &DesiredItem,
    layout_order: &HashMap<Entity, usize>,
    layout_order_labels: &BTreeMap<LayoutOrderKey, i32>,
    widget_order_labels: &BTreeMap<WidgetOrderKey, i32>,
) -> i32 {
    let (layer, order) = match desired.key {
        RenderKey::ManualDisplayListIndex(index) => (0, index as u64),
        RenderKey::LayoutEntity(entity) => {
            let _ = layout_order;
            return layout_order_labels
                .get(&LayoutOrderKey {
                    depth: desired.hierarchy_depth,
                    entity_bits: entity.to_bits(),
                })
                .copied()
                .or_else(|| {
                    layout_order
                        .get(&entity)
                        .map(|rank| LOCAL_Z_LAYER_STRIDE + *rank as i32)
                })
                .unwrap_or(LOCAL_Z_LAYER_STRIDE);
        }
        RenderKey::WidgetFragment(entity, index) => {
            let key = WidgetOrderKey {
                order: desired.fragment_order.unwrap_or_default(),
                entity_bits: entity.to_bits(),
                item_index: index,
            };
            let label = widget_order_labels
                .get(&key)
                .copied()
                .unwrap_or(WIDGET_FRAGMENT_LOCAL_Z_START);
            return if desired.target_camera.is_some() {
                let relative = i64::from(label - WIDGET_FRAGMENT_LOCAL_Z_START);
                let scaled = relative
                    * i64::from(CAMERA_FRAGMENT_LOCAL_Z_END - CAMERA_FRAGMENT_LOCAL_Z_START)
                    / i64::from(WIDGET_FRAGMENT_LOCAL_Z_END - WIDGET_FRAGMENT_LOCAL_Z_START);
                CAMERA_FRAGMENT_LOCAL_Z_START + scaled as i32
            } else {
                label
            };
        }
        RenderKey::WidgetDisplayListIndex(index) => (3, index as u64),
        RenderKey::OverlayWidgetDisplayListIndex(index) => (4, index as u64),
        RenderKey::CameraDisplayListIndex(_, index) => (5, index as u64),
        RenderKey::OverlayCameraDisplayListIndex(_, index) => (6, index as u64),
    };
    let rank = i32::try_from(order.min((LOCAL_Z_LAYER_STRIDE - 1) as u64))
        .unwrap_or(LOCAL_Z_LAYER_STRIDE - 1);
    layer * LOCAL_Z_LAYER_STRIDE + rank
}

pub(super) fn rebuild_widget_order_labels(cache: &mut RenderCache, desired: &[DesiredItem]) {
    cache.fragment_order_keys.clear();
    cache.widget_order_labels.clear();
    let mut keys: Vec<_> = desired
        .iter()
        .filter_map(|desired| {
            let RenderKey::WidgetFragment(entity, item_index) = desired.key else {
                return None;
            };
            Some((
                entity,
                WidgetOrderKey {
                    order: desired.fragment_order.unwrap_or_default(),
                    entity_bits: entity.to_bits(),
                    item_index,
                },
            ))
        })
        .collect();
    keys.sort_unstable_by_key(|(_, key)| *key);
    if keys.is_empty() {
        return;
    }
    let width = i64::from(WIDGET_FRAGMENT_LOCAL_Z_END - WIDGET_FRAGMENT_LOCAL_Z_START);
    let step = (width / (keys.len() as i64 + 1)).max(1);
    for (rank, (entity, key)) in keys.into_iter().enumerate() {
        let rank = i64::try_from(rank + 1).unwrap_or(i64::MAX);
        let label = i64::from(WIDGET_FRAGMENT_LOCAL_Z_START)
            .saturating_add(step.saturating_mul(rank))
            .min(i64::from(WIDGET_FRAGMENT_LOCAL_Z_END - 1)) as i32;
        cache.widget_order_labels.insert(key, label);
        cache
            .fragment_order_keys
            .entry(entity)
            .or_default()
            .push(key);
    }
}

pub(super) fn update_widget_order_labels(
    cache: &mut RenderCache,
    entity: Entity,
    fragment: Option<&WidgetRenderFragment>,
) -> bool {
    if let Some(previous) = cache.fragment_order_keys.remove(&entity) {
        for key in previous {
            cache.widget_order_labels.remove(&key);
        }
    }
    let Some(fragment) = fragment else {
        return true;
    };
    if fragment.items.is_empty() {
        cache.fragment_order_keys.insert(entity, Vec::new());
        return true;
    }
    let keys: Vec<_> = (0..fragment.items.len())
        .map(|item_index| WidgetOrderKey {
            order: fragment.order,
            entity_bits: entity.to_bits(),
            item_index,
        })
        .collect();
    let Some((&first, &last)) = keys.first().zip(keys.last()) else {
        return true;
    };
    let previous = cache
        .widget_order_labels
        .range(..first)
        .next_back()
        .map(|(_, label)| *label)
        .unwrap_or(WIDGET_FRAGMENT_LOCAL_Z_START);
    let next = cache
        .widget_order_labels
        .range(last..)
        .next()
        .map(|(_, label)| *label)
        .unwrap_or(WIDGET_FRAGMENT_LOCAL_Z_END);
    let gap = i64::from(next) - i64::from(previous);
    if gap <= keys.len() as i64 {
        return false;
    }
    let step = gap / (keys.len() as i64 + 1);
    let mut labels = Vec::with_capacity(keys.len());
    for (index, key) in keys.into_iter().enumerate() {
        let offset = step.saturating_mul(index as i64 + 1);
        let label = (i64::from(previous) + offset) as i32;
        cache.widget_order_labels.insert(key, label);
        labels.push(key);
    }
    cache.fragment_order_keys.insert(entity, labels);
    true
}

pub(super) fn rebuild_layout_order_labels(cache: &mut RenderCache, desired: &[DesiredItem]) {
    cache.layout_entity_order_keys.clear();
    cache.layout_order_labels.clear();
    let mut keys: Vec<_> = desired
        .iter()
        .filter_map(|desired| match desired.key {
            RenderKey::LayoutEntity(entity) => Some((
                entity,
                LayoutOrderKey {
                    depth: desired.hierarchy_depth,
                    entity_bits: entity.to_bits(),
                },
            )),
            _ => None,
        })
        .collect();
    keys.sort_unstable_by_key(|(_, key)| *key);
    let width = i64::from(LOCAL_Z_LAYER_STRIDE);
    let step = (width / (keys.len() as i64 + 1)).max(1);
    for (index, (entity, key)) in keys.into_iter().enumerate() {
        let offset = step.saturating_mul(index as i64 + 1);
        let label = (i64::from(LOCAL_Z_LAYER_STRIDE) + offset)
            .min(i64::from(LOCAL_Z_LAYER_STRIDE * 2 - 1)) as i32;
        cache.layout_entity_order_keys.insert(entity, key);
        cache.layout_order_labels.insert(key, label);
    }
}

pub(super) fn insert_layout_order_label(
    cache: &mut RenderCache,
    entity: Entity,
    key: LayoutOrderKey,
) -> bool {
    let previous = cache
        .layout_order_labels
        .range(..key)
        .next_back()
        .map(|(_, label)| *label)
        .unwrap_or(LOCAL_Z_LAYER_STRIDE);
    let next = cache
        .layout_order_labels
        .range(key..)
        .next()
        .map(|(_, label)| *label)
        .unwrap_or(LOCAL_Z_LAYER_STRIDE * 2);
    if i64::from(next) - i64::from(previous) <= 1 {
        return false;
    }
    let label = previous + (next - previous) / 2;
    cache.layout_entity_order_keys.insert(entity, key);
    cache.layout_order_labels.insert(key, label);
    true
}

pub(super) fn update_layout_order_labels_for_entities(
    cache: &mut RenderCache,
    entities: impl IntoIterator<Item = Entity>,
) -> bool {
    let entities: HashSet<_> = entities
        .into_iter()
        .filter(|entity| cache.layout_order.contains_key(entity))
        .collect();
    let mut keys = Vec::with_capacity(entities.len());
    for entity in entities {
        let key = LayoutOrderKey {
            depth: hierarchy_depth(entity, &cache.parents_by_entity),
            entity_bits: entity.to_bits(),
        };
        if cache.layout_entity_order_keys.get(&entity) == Some(&key) {
            continue;
        }
        if let Some(previous) = cache.layout_entity_order_keys.remove(&entity) {
            cache.layout_order_labels.remove(&previous);
        }
        keys.push((entity, key));
    }
    keys.sort_unstable_by_key(|(_, key)| *key);
    for (entity, key) in keys {
        if !insert_layout_order_label(cache, entity, key) {
            return false;
        }
    }
    true
}

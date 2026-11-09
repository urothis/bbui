use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasImageState {
    Loading,
    Ready,
    Failed(String),
}

impl CanvasImageAssets {
    pub fn id(&self, asset: &str) -> Option<u64> {
        self.ids.get(asset).copied()
    }

    pub fn state(&self, asset: &str) -> Option<&CanvasImageState> {
        self.states.get(asset)
    }
}

/// Keeps only images used by mounted, visible canvas objects in Bevy's image
/// registry. The asset server owns decoding; dropping the last handle lets Bevy
/// release the texture when a canvas leaves the viewport or is removed.
pub fn sync_canvas_images(world: &mut World) {
    let Some(asset_server) = world.get_resource::<AssetServer>().cloned() else {
        return;
    };
    if world.get_resource::<ImageRegistry>().is_none() {
        return;
    }
    let required = visible_canvas_image_paths(world);
    let state_is_current = world
        .get_resource::<CanvasImageAssets>()
        .is_some_and(|assets| {
            assets.ids.len() == required.len()
                && assets.states.len() == required.len()
                && required.iter().all(|asset| {
                    let Some(id) = assets.ids.get(asset) else {
                        return false;
                    };
                    let status = world
                        .resource::<ImageRegistry>()
                        .get(*id)
                        .and_then(|handle| asset_server.get_load_state(handle.id()))
                        .map(canvas_image_state);
                    assets.states.get(asset) == Some(&status.unwrap_or(CanvasImageState::Loading))
                })
        });
    if state_is_current {
        return;
    }
    let previous = world
        .get_resource::<CanvasImageAssets>()
        .cloned()
        .unwrap_or_default();
    let mut assets = world
        .remove_resource::<CanvasImageAssets>()
        .unwrap_or_default();
    let mut registry = world.resource_mut::<ImageRegistry>();
    assets.ids.retain(|asset, id| {
        if required.contains(asset) {
            true
        } else {
            registry.remove(*id);
            false
        }
    });
    assets.states.retain(|asset, _| required.contains(asset));
    for asset in required {
        if assets.ids.contains_key(&asset) {
            continue;
        }
        let Some(id) = registry.allocate_id() else {
            break;
        };
        let handle = asset_server.load::<Image>(asset.clone());
        registry.insert(id, handle);
        assets.ids.insert(asset, id);
    }
    for (asset, id) in &assets.ids {
        let state = registry
            .get(*id)
            .and_then(|handle| asset_server.get_load_state(handle.id()));
        let status = state
            .map(canvas_image_state)
            .unwrap_or(CanvasImageState::Loading);
        assets.states.insert(asset.clone(), status);
    }
    let mut changed_assets = HashSet::new();
    changed_assets.extend(
        previous
            .ids
            .keys()
            .chain(assets.ids.keys())
            .filter(|asset| {
                let asset = *asset;
                previous.ids.get(asset) != assets.ids.get(asset)
                    || previous.states.get(asset) != assets.states.get(asset)
            })
            .cloned(),
    );
    world.insert_resource(assets);
    if !changed_assets.is_empty() {
        let changed_items = world
            .get_resource::<CanvasObjectItems>()
            .into_iter()
            .flat_map(|items| items.0.values())
            .flat_map(HashMap::values)
            .filter_map(|entity| {
                let paint = world.get::<CanvasObjectPaint>(*entity)?;
                let CanvasShape::Image { asset, .. } = &paint.0.shape else {
                    return None;
                };
                changed_assets.contains(asset).then_some(*entity)
            })
            .collect::<Vec<_>>();
        world.init_resource::<UiDirty>();
        let mut dirty = world.resource_mut::<UiDirty>();
        for entity in changed_items {
            dirty.mark(UiDirtyDomain::Paint, entity);
        }
    }
}

pub(super) fn canvas_image_state(state: LoadState) -> CanvasImageState {
    match state {
        LoadState::Loaded => CanvasImageState::Ready,
        LoadState::Failed(error) => CanvasImageState::Failed(error.to_string()),
        _ => CanvasImageState::Loading,
    }
}

pub(super) fn visible_canvas_image_paths(world: &mut World) -> HashSet<String> {
    let mut query = world.query::<(Entity, &Canvas, &UiLayoutResult)>();
    let owners = query
        .iter(world)
        .map(|(owner, _, layout)| (owner, *layout))
        .collect::<Vec<_>>();
    let mut paths = HashSet::new();
    for (owner, layout) in owners {
        if !visible_in_hierarchy(world, owner) {
            continue;
        }
        ensure_canvas_spatial_index(world, owner);
        let Some(canvas) = world.get::<Canvas>(owner) else {
            continue;
        };
        let Some(viewport) = layout.clip.unwrap_or(layout.rect).intersection(layout.rect) else {
            continue;
        };
        let indices = query_canvas_object_indices(
            world,
            owner,
            canvas,
            layout.rect,
            ClipRect::from(viewport),
            0.0,
        )
        .unwrap_or_else(|| (0..canvas.objects.len()).collect());
        for index in indices {
            let Some(object) = canvas.objects.get(index) else {
                continue;
            };
            let CanvasShape::Image { size, asset } = &object.shape else {
                continue;
            };
            if !object.visible || !object.shape.valid() {
                continue;
            }
            let position = canvas.world_to_screen(object.origin);
            let screen_rect = UiRect::new(
                layout.rect.x + position[0],
                layout.rect.y + position[1],
                size[0] * canvas.zoom,
                size[1] * canvas.zoom,
            );
            if viewport.intersection(screen_rect).is_some() {
                paths.insert(asset.clone());
            }
        }
    }
    paths
}

pub(super) fn visible_in_hierarchy(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    for _ in 0..1024 {
        let Some(at) = current else {
            return true;
        };
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    false
}

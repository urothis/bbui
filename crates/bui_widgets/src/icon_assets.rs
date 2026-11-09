//! Visibility-aware Bevy image assets for icon buttons.

use std::collections::{HashMap, HashSet};

use bevy_app::{App, Update};
use bevy_asset::{AssetServer, Assets, Handle, LoadState};
use bevy_camera::Camera;
use bevy_ecs::change_detection::Tick;
use bevy_ecs::prelude::{Component, Entity, On, Remove, ResMut, Resource, World};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_image::Image;
use bui_core::{UiEnabled, UiNode, UiParent, UiSystems, UiVisible};
use bui_layout::{Rect, UiLayoutResult};
use bui_render::{ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive};
use bui_style::{Rgba, Theme, ThemeColorToken, UiComputedStyle, UiStyleProperty};

use crate::{IconButton, IconButtonAsset, IconButtonAssetState, IconButtonAssetStatus};

#[derive(Debug, Clone)]
struct VisibleIcon {
    entity: Entity,
    path: String,
    retry_generation: u64,
    rect: Rect,
    clip: Rect,
    z: i32,
    camera: Option<Entity>,
}

struct CachedIcon {
    path: String,
    retry_generation: u64,
    handle: Option<Handle<Image>>,
    asset_id: Option<u64>,
    status: IconButtonAssetStatus,
}

/// Strong handles and renderer IDs for icon buttons currently in the visible tree.
///
/// Entries are keyed by widget entity and discarded as soon as an icon leaves the clipped,
/// visible layout. The cache is therefore bounded by the number of visible image-backed buttons;
/// it does not retain a global path cache for hidden widgets.
#[derive(Resource, Default)]
pub struct IconButtonAssetCache {
    entries: HashMap<Entity, CachedIcon>,
    overlay_cameras: HashSet<Entity>,
}

#[derive(Resource, Default)]
struct IconSyncGate {
    cursor: Tick,
    initialized: bool,
    force: bool,
    configured: HashSet<Entity>,
    ancestors: HashSet<Entity>,
    camera_windows: HashMap<Entity, bool>,
    resource_presence: [bool; 4],
    sync_passes: usize,
}

impl IconButtonAssetCache {
    pub fn status(&self, entity: Entity) -> Option<&IconButtonAssetStatus> {
        self.entries.get(&entity).map(|entry| &entry.status)
    }

    pub fn asset_id(&self, entity: Entity) -> Option<u64> {
        self.entries.get(&entity).and_then(|entry| entry.asset_id)
    }

    pub fn visible_count(&self) -> usize {
        self.entries.len()
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<IconButtonAssetCache>()
        .init_resource::<IconSyncGate>()
        .add_observer(force_icon_sync_on_remove::<IconButtonAsset>)
        .add_observer(force_icon_sync_on_remove::<IconButton>)
        .add_observer(force_icon_sync_on_remove::<UiLayoutResult>)
        .add_observer(force_icon_sync_on_remove::<UiComputedStyle>)
        .add_observer(force_icon_sync_on_remove::<UiEnabled>)
        .add_observer(force_icon_sync_on_remove::<UiVisible>)
        .add_observer(force_icon_sync_on_remove::<UiParent>)
        .add_observer(force_icon_sync_on_remove::<UiNode>)
        .add_observer(force_icon_sync_on_remove::<Camera>)
        .add_systems(
            Update,
            sync_icon_button_assets
                .in_set(UiSystems::Widgets)
                .after(crate::actions::collect_widget_paint),
        );
}

fn force_icon_sync_on_remove<T: Component>(_on: On<Remove<T>>, mut gate: ResMut<IconSyncGate>) {
    gate.force = true;
}

/// Loads only mounted icons whose widget and clipped layout are visible, then paints the image
/// through the shared GPU-native `Primitive::Icon` path.
pub fn sync_icon_button_assets(world: &mut World) {
    world.init_resource::<IconButtonAssetCache>();
    world.init_resource::<IconSyncGate>();
    if !icon_inputs_changed(world) {
        return;
    }
    let visible = collect_visible_icons(world);
    let configured = collect_configured_icon_entities(world);
    let Some(asset_server) = world.get_resource::<AssetServer>().cloned() else {
        release_all(world);
        let visible_entities: HashSet<_> = visible.iter().map(|icon| icon.entity).collect();
        for entity in configured {
            set_status(
                world,
                entity,
                if visible_entities.contains(&entity) {
                    IconButtonAssetStatus::Failed("AssetServer is not installed".into())
                } else {
                    IconButtonAssetStatus::Hidden
                },
            );
        }
        settle_icon_gate(world);
        return;
    };
    if !world.contains_resource::<Assets<Image>>() || !world.contains_resource::<ImageRegistry>() {
        release_all(world);
        let visible_entities: HashSet<_> = visible.iter().map(|icon| icon.entity).collect();
        for entity in configured {
            set_status(
                world,
                entity,
                if visible_entities.contains(&entity) {
                    IconButtonAssetStatus::Failed(
                        "Bevy image assets or ImageRegistry are not installed".into(),
                    )
                } else {
                    IconButtonAssetStatus::Hidden
                },
            );
        }
        settle_icon_gate(world);
        return;
    }

    let visible_entities: HashSet<_> = visible.iter().map(|icon| icon.entity).collect();
    let mut cache = world
        .remove_resource::<IconButtonAssetCache>()
        .unwrap_or_default();

    let stale_entities: Vec<_> = cache
        .entries
        .keys()
        .copied()
        .filter(|entity| !visible_entities.contains(entity))
        .collect();
    for entity in stale_entities {
        release_entry(world, &mut cache, entity);
        if configured.contains(&entity) {
            set_status(world, entity, IconButtonAssetStatus::Hidden);
        }
    }

    for icon in &visible {
        let source_changed = cache.entries.get(&icon.entity).is_some_and(|entry| {
            entry.path != icon.path || entry.retry_generation != icon.retry_generation
        });
        if source_changed {
            release_entry(world, &mut cache, icon.entity);
        }
        if let std::collections::hash_map::Entry::Vacant(slot) = cache.entries.entry(icon.entity) {
            let entry = if icon.path.trim().is_empty() {
                CachedIcon {
                    path: icon.path.clone(),
                    retry_generation: icon.retry_generation,
                    handle: None,
                    asset_id: None,
                    status: IconButtonAssetStatus::Failed("icon asset path is empty".to_owned()),
                }
            } else {
                CachedIcon {
                    path: icon.path.clone(),
                    retry_generation: icon.retry_generation,
                    handle: Some(asset_server.load::<Image>(icon.path.clone())),
                    asset_id: None,
                    status: IconButtonAssetStatus::Loading,
                }
            };
            set_status(world, icon.entity, entry.status.clone());
            slot.insert(entry);
        }
        advance_entry(world, &asset_server, &mut cache, icon.entity);
    }

    paint_icon_overlays(world, &visible, &cache);
    cache.overlay_cameras = visible.iter().filter_map(|icon| icon.camera).collect();
    world.insert_resource(cache);
    settle_icon_gate(world);
}

fn icon_inputs_changed(world: &mut World) -> bool {
    let mut configured = world.query::<(Entity, &IconButtonAsset)>();
    let now = world.read_change_tick();
    let gate = world.resource::<IconSyncGate>();
    if !gate.initialized || gate.force {
        return true;
    }
    let presence = [
        world.contains_resource::<AssetServer>(),
        world.contains_resource::<Assets<Image>>(),
        world.contains_resource::<ImageRegistry>(),
        world.contains_resource::<Theme>(),
    ];
    if presence != gate.resource_presence
        || world
            .resource::<IconButtonAssetCache>()
            .entries
            .values()
            .any(|entry| entry.status == IconButtonAssetStatus::Loading)
        || world
            .get_resource_change_ticks::<Theme>()
            .is_some_and(|ticks| ticks.changed.is_newer_than(gate.cursor, now))
        || world
            .get_resource_change_ticks::<Assets<Image>>()
            .is_some_and(|ticks| ticks.changed.is_newer_than(gate.cursor, now))
        || world
            .get_resource_change_ticks::<ImageRegistry>()
            .is_some_and(|ticks| ticks.changed.is_newer_than(gate.cursor, now))
    {
        return true;
    }
    for ancestor in &gate.ancestors {
        let Ok(ancestor) = world.get_entity(*ancestor) else {
            return true;
        };
        if [
            ancestor.get_change_ticks::<UiVisible>(),
            ancestor.get_change_ticks::<UiParent>(),
            ancestor.get_change_ticks::<UiNode>(),
        ]
        .into_iter()
        .flatten()
        .any(|ticks| ticks.changed.is_newer_than(gate.cursor, now))
        {
            return true;
        }
    }
    if gate.camera_windows.iter().any(|(window, was_camera)| {
        world
            .get_entity(*window)
            .is_ok_and(|entity| entity.contains::<Camera>())
            != *was_camera
    }) {
        return true;
    }
    let mut count = 0;
    for (entity, _) in configured.iter(world) {
        count += 1;
        if !gate.configured.contains(&entity) {
            return true;
        }
        let Ok(entry) = world.get_entity(entity) else {
            return true;
        };
        if [
            entry.get_change_ticks::<IconButtonAsset>(),
            entry.get_change_ticks::<IconButton>(),
            entry.get_change_ticks::<UiLayoutResult>(),
            entry.get_change_ticks::<UiComputedStyle>(),
            entry.get_change_ticks::<UiEnabled>(),
        ]
        .into_iter()
        .flatten()
        .any(|ticks| ticks.changed.is_newer_than(gate.cursor, now))
        {
            return true;
        }
    }
    count != gate.configured.len()
}

fn settle_icon_gate(world: &mut World) {
    let cursor = world.read_change_tick();
    let configured = collect_configured_icon_entities(world);
    let mut ancestors = HashSet::new();
    let mut camera_windows = HashMap::new();
    for entity in &configured {
        let mut current = Some(*entity);
        let mut remaining = world.entities().len();
        while let Some(at) = current {
            if remaining == 0 || !ancestors.insert(at) {
                break;
            }
            remaining -= 1;
            current = world.get::<UiParent>(at).map(|parent| parent.0);
        }
        if let Some(layout) = world.get::<UiLayoutResult>(*entity) {
            camera_windows.insert(
                layout.window,
                world
                    .get_entity(layout.window)
                    .is_ok_and(|window| window.contains::<Camera>()),
            );
        }
    }
    let presence = [
        world.contains_resource::<AssetServer>(),
        world.contains_resource::<Assets<Image>>(),
        world.contains_resource::<ImageRegistry>(),
        world.contains_resource::<Theme>(),
    ];
    let mut gate = world.resource_mut::<IconSyncGate>();
    gate.cursor = cursor;
    gate.configured = configured;
    gate.ancestors = ancestors;
    gate.camera_windows = camera_windows;
    gate.resource_presence = presence;
    gate.initialized = true;
    gate.force = false;
    gate.sync_passes += 1;
}

fn collect_configured_icon_entities(world: &mut World) -> HashSet<Entity> {
    let mut query = world.query::<(Entity, &IconButtonAsset)>();
    query.iter(world).map(|(entity, _)| entity).collect()
}

fn collect_visible_icons(world: &mut World) -> Vec<VisibleIcon> {
    let mut query = world.query::<(Entity, &IconButtonAsset, &UiLayoutResult)>();
    query
        .iter(world)
        .filter_map(|(entity, asset, layout)| {
            if world.get::<IconButton>(entity).is_none() || !tree_visible(world, entity) {
                return None;
            }
            let clip = match layout.clip {
                Some(parent) => layout.rect.intersection(parent)?,
                None => layout.rect,
            };
            if clip.is_empty() {
                return None;
            }
            let camera = world
                .get_entity(layout.window)
                .is_ok_and(|window| window.contains::<Camera>())
                .then_some(layout.window);
            Some(VisibleIcon {
                entity,
                path: asset.path.clone(),
                retry_generation: asset.retry_generation,
                rect: layout.rect,
                clip,
                z: layout.z,
                camera,
            })
        })
        .collect()
}

fn tree_visible(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut remaining = world.entities().len();
    while let Some(at) = current {
        if remaining == 0 || world.get::<UiNode>(at).is_none() {
            return false;
        }
        remaining -= 1;
        if world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    true
}

fn advance_entry(
    world: &mut World,
    asset_server: &AssetServer,
    cache: &mut IconButtonAssetCache,
    entity: Entity,
) {
    let Some(entry) = cache.entries.get_mut(&entity) else {
        return;
    };
    if entry.handle.is_none() {
        set_status(world, entity, entry.status.clone());
        return;
    }
    let handle = entry.handle.as_ref().expect("handle was checked").clone();
    match asset_server.get_load_state(handle.id()) {
        Some(LoadState::Failed(error)) => {
            if let Some(asset_id) = entry.asset_id.take() {
                world.resource_mut::<ImageRegistry>().remove(asset_id);
            }
            entry.handle = None;
            entry.status = IconButtonAssetStatus::Failed(error.to_string());
        }
        Some(LoadState::Loaded) if world.resource::<Assets<Image>>().get(handle.id()).is_some() => {
            if let Some(asset_id) = entry.asset_id {
                if world.resource::<ImageRegistry>().get(asset_id).is_none() {
                    world
                        .resource_mut::<ImageRegistry>()
                        .insert(asset_id, handle.clone());
                }
            } else if let Some(asset_id) = world.resource_mut::<ImageRegistry>().allocate_id() {
                world
                    .resource_mut::<ImageRegistry>()
                    .insert(asset_id, handle);
                entry.asset_id = Some(asset_id);
            } else {
                entry.handle = None;
                entry.status = IconButtonAssetStatus::Failed(
                    "image registry ID space is exhausted".to_owned(),
                );
            }
            if entry.asset_id.is_some() {
                entry.status = IconButtonAssetStatus::Ready;
            }
        }
        Some(LoadState::Loaded) => {
            entry.status = IconButtonAssetStatus::Failed(
                "AssetServer reported a loaded icon without an image asset".to_owned(),
            );
            entry.handle = None;
        }
        _ => entry.status = IconButtonAssetStatus::Loading,
    }
    set_status(world, entity, entry.status.clone());
}

fn release_entry(world: &mut World, cache: &mut IconButtonAssetCache, entity: Entity) {
    if let Some(entry) = cache.entries.remove(&entity)
        && let Some(asset_id) = entry.asset_id
        && let Some(mut registry) = world.get_resource_mut::<ImageRegistry>()
    {
        registry.remove(asset_id);
    }
}

fn release_all(world: &mut World) {
    let mut cache = world
        .remove_resource::<IconButtonAssetCache>()
        .unwrap_or_default();
    let entities: Vec<_> = cache.entries.keys().copied().collect();
    for entity in entities {
        release_entry(world, &mut cache, entity);
    }
    if let Some(mut display_list) = world.get_resource_mut::<DisplayList>() {
        display_list.replace_overlay_widget_items(Vec::new());
        for camera in cache.overlay_cameras.drain() {
            display_list.replace_overlay_camera_items(camera, Vec::new());
        }
    }
    world.insert_resource(cache);
}

fn set_status(world: &mut World, entity: Entity, status: IconButtonAssetStatus) {
    if world
        .get::<IconButtonAssetState>(entity)
        .is_some_and(|current| current.status == status)
    {
        return;
    }
    if world.get_entity(entity).is_ok() {
        world
            .entity_mut(entity)
            .insert(IconButtonAssetState { status });
    }
}

fn paint_icon_overlays(world: &mut World, visible: &[VisibleIcon], cache: &IconButtonAssetCache) {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let old_cameras = cache.overlay_cameras.clone();
    let mut screen_items = Vec::new();
    let mut camera_items = HashMap::<Entity, Vec<DrawItem>>::new();
    for icon in visible {
        let Some(entry) = cache.entries.get(&icon.entity) else {
            continue;
        };
        let side = icon.rect.width.min(icon.rect.height).max(0.0) * 0.64;
        if side <= 0.0 {
            continue;
        }
        let rect = Rect::new(
            icon.rect.x + (icon.rect.width - side) * 0.5,
            icon.rect.y + (icon.rect.height - side) * 0.5,
            side,
            side,
        );
        let clip = ClipRect::from(icon.clip);
        let enabled = world
            .get::<UiEnabled>(icon.entity)
            .is_none_or(|enabled| enabled.0)
            && world
                .get::<IconButton>(icon.entity)
                .is_some_and(|button| button.enabled);
        let token = if enabled {
            ThemeColorToken::Icon
        } else {
            ThemeColorToken::DisabledIcon
        };
        let tint = world
            .get::<IconButtonAsset>(icon.entity)
            .and_then(|asset| asset.tint)
            .or_else(|| {
                world.get::<UiComputedStyle>(icon.entity).and_then(|style| {
                    style.color(if enabled {
                        UiStyleProperty::IconColor
                    } else {
                        UiStyleProperty::DisabledIconColor
                    })
                })
            })
            .unwrap_or_else(|| theme.color(token));
        let mut push = |z, primitive| {
            let draw = DrawItem { clip, z, primitive };
            if let Some(camera) = icon.camera {
                camera_items.entry(camera).or_default().push(draw);
            } else {
                screen_items.push(draw);
            }
        };
        match &entry.status {
            IconButtonAssetStatus::Ready => {
                if let Some(asset_id) = entry.asset_id {
                    push(
                        icon.z.saturating_add(1),
                        Primitive::Icon {
                            rect: ClipRect::from(rect),
                            asset_id,
                            tint,
                        },
                    );
                }
            }
            IconButtonAssetStatus::Loading => {
                let dot = side * 0.28;
                push(
                    icon.z.saturating_add(1),
                    Primitive::Ellipse {
                        rect: ClipRect {
                            x: rect.x + (side - dot) * 0.5,
                            y: rect.y + (side - dot) * 0.5,
                            width: dot,
                            height: dot,
                        },
                        color: Rgba(tint.0, tint.1, tint.2, tint.3 * 0.55),
                    },
                );
            }
            IconButtonAssetStatus::Failed(_) => {
                let inset = side * 0.24;
                let start = [rect.x + inset, rect.y + inset];
                let end = [rect.x + side - inset, rect.y + side - inset];
                push(
                    icon.z.saturating_add(1),
                    Primitive::Line {
                        from: start,
                        to: end,
                        width: 2.0,
                        color: theme.error,
                    },
                );
                push(
                    icon.z.saturating_add(1),
                    Primitive::Line {
                        from: [start[0], end[1]],
                        to: [end[0], start[1]],
                        width: 2.0,
                        color: theme.error,
                    },
                );
            }
            IconButtonAssetStatus::Hidden => {}
        }
    }

    if let Some(mut display_list) = world.get_resource_mut::<DisplayList>() {
        display_list.replace_overlay_widget_items(screen_items);
        let new_cameras: HashSet<_> = camera_items.keys().copied().collect();
        for camera in old_cameras.union(&new_cameras).copied() {
            display_list.replace_overlay_camera_items(
                camera,
                camera_items.remove(&camera).unwrap_or_default(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IconButton, IconButtonAsset, IconButtonAssetState, WidgetPlugin};
    use bevy_app::App;
    use bui_core::{UiEnabled, UiNode, UiVisible, set_parent};

    fn spawn_test_icon(world: &mut World) -> Entity {
        world
            .spawn((
                UiNode,
                UiVisible(true),
                UiEnabled(true),
                IconButton::new("", "Asset icon"),
                IconButtonAsset::new("missing/icon.png"),
                IconButtonAssetState::default(),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: Rect::new(10.0, 10.0, 32.0, 32.0),
                    clip: None,
                    z: 2,
                    content_size: [32.0, 32.0],
                    dpi_scale: 1.0,
                },
            ))
            .id()
    }

    #[test]
    fn missing_asset_server_reports_failure_and_hidden_icons_release_cache_entries() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let entity = spawn_test_icon(app.world_mut());

        app.update();
        assert!(matches!(
            &app.world().get::<IconButtonAssetState>(entity).unwrap().status,
            IconButtonAssetStatus::Failed(message) if message == "AssetServer is not installed"
        ));
        assert_eq!(
            app.world()
                .resource::<IconButtonAssetCache>()
                .visible_count(),
            0
        );

        app.world_mut().get_mut::<UiVisible>(entity).unwrap().0 = false;
        app.update();
        assert_eq!(
            app.world()
                .get::<IconButtonAssetState>(entity)
                .unwrap()
                .status,
            IconButtonAssetStatus::Hidden
        );
    }

    #[test]
    fn icon_asset_retry_generation_advances_without_changing_the_path() {
        let mut icon = IconButtonAsset::new("icons/missing.svg");
        icon.retry();
        assert_eq!(icon.path, "icons/missing.svg");
        assert_eq!(icon.retry_generation, 1);
        icon.retry();
        assert_eq!(icon.retry_generation, 2);
    }

    #[test]
    fn settled_icons_skip_idle_scans_and_follow_ancestor_visibility() {
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        let parent = app.world_mut().spawn((UiNode, UiVisible(true))).id();
        let icon = spawn_test_icon(app.world_mut());
        set_parent(app.world_mut(), icon, parent).unwrap();

        app.update();
        let passes = app.world().resource::<IconSyncGate>().sync_passes;
        app.update();
        assert_eq!(app.world().resource::<IconSyncGate>().sync_passes, passes);

        app.world_mut().get_mut::<UiVisible>(parent).unwrap().0 = false;
        app.update();
        assert_eq!(
            app.world()
                .get::<IconButtonAssetState>(icon)
                .unwrap()
                .status,
            IconButtonAssetStatus::Hidden
        );
        let passes = app.world().resource::<IconSyncGate>().sync_passes;
        app.update();
        assert_eq!(app.world().resource::<IconSyncGate>().sync_passes, passes);

        app.world_mut().get_mut::<UiVisible>(parent).unwrap().0 = true;
        app.update();
        assert!(matches!(
            &app.world().get::<IconButtonAssetState>(icon).unwrap().status,
            IconButtonAssetStatus::Failed(message) if message == "AssetServer is not installed"
        ));
        assert_eq!(
            app.world().resource::<IconSyncGate>().sync_passes,
            passes + 1
        );
    }
}

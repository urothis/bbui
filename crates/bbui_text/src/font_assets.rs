//! Visibility-aware AssetServer loading for UI font faces.

use bbui_core::{UiParent, UiVisible};
use bbui_layout::{UiLayoutResult, UiLayoutRoot, UiViewports};
use bevy_asset::{AssetServer, Assets, Handle, LoadState};
use bevy_ecs::prelude::{Component, Entity, World};
use bevy_text::{Font, FontCx, FontSource, TextFont};

/// Family name used for BBUI's bundled proportional default UI face.
pub const UI_DEFAULT_FONT_FAMILY: &str = "BBUI UI Sans";
/// Bundled fallback for disclosure arrows and other UI symbols absent from Inter.
pub const UI_SYMBOL_FONT_FAMILY: &str = "BBUI UI Symbols";

/// Embedded Inter face used by BBUI's default UI text.
pub const UI_DEFAULT_FONT_DATA: &[u8] =
    include_bytes!("../../../assets/fonts/Inter[opsz,wght].ttf");
/// Embedded Noto Sans Symbols 2 face used for Unicode symbols.
pub const UI_SYMBOL_FONT_DATA: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSansSymbols2-Regular.ttf");

/// Build a theme-sized `TextFont` with bundled Inter and symbol fallback faces.
///
/// This only chooses the font family and size. Callers can still provide a custom `TextFont`,
/// while [`resolve_text_font`] lets a ready [`UiFontAsset`] override the family for a subtree.
pub fn ui_default_text_font(font_size: f32) -> TextFont {
    TextFont {
        font: FontSource::list([
            FontSource::Family(UI_DEFAULT_FONT_FAMILY.into()),
            FontSource::Family(UI_SYMBOL_FONT_FAMILY.into()),
        ]),
        ..TextFont::from_font_size(font_size)
    }
}

/// Register BBUI's embedded text and symbol faces in Bevy's shared font database.
///
/// The operation is idempotent, so both `TextPlugin` and `BbuiRenderPlugin` can call it. It runs
/// synchronously during plugin setup, before intrinsic measurement or display-list rendering,
/// and does not depend on the host's asset root or filesystem.
pub fn register_default_ui_font(world: &mut World) {
    world.init_resource::<FontCx>();
    let mut font_cx = world.resource_mut::<FontCx>();
    for (family, data) in [
        (UI_DEFAULT_FONT_FAMILY, UI_DEFAULT_FONT_DATA),
        (UI_SYMBOL_FONT_FAMILY, UI_SYMBOL_FONT_DATA),
    ] {
        if font_cx.collection.family_id(family).is_none() {
            let font = Font::from_bytes(data.to_vec());
            font_cx.collection.register_fonts(
                font.data,
                Some(parley::fontique::FontInfoOverride {
                    family_name: Some(family),
                    ..Default::default()
                }),
            );
        }
    }
}

/// Loading status for a font requested by [`UiFontAsset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiFontAssetStatus {
    /// The widget is hidden, clipped outside its viewport, or has not requested the file.
    Hidden,
    Loading,
    Ready,
    Failed(String),
}

/// Read-only ECS status mirror for systems that observe a font without accessing its loader data.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiFontAssetState {
    pub status: UiFontAssetStatus,
}

/// AssetServer-backed font source for a BBUI text entity or a parent of text widgets.
///
/// Attach this to an entity with [`crate::UiTextLayout`] to use one font for both shaped editing
/// geometry and rendered text. A font on a parent is inherited by child text controls. The asset
/// is requested only while the component is visible; hiding or clipping its owner drops the BBUI
/// strong handle and allows Bevy to cancel an unfinished load or release the asset when no other
/// owners remain. Until the file is ready, text uses its normal Bevy/default font fallback.
#[derive(Component, Debug, Clone)]
pub struct UiFontAsset {
    /// Asset path relative to the configured Bevy asset root.
    pub path: String,
    /// Current load state, available to UI and accessibility presentation code.
    pub status: UiFontAssetStatus,
    /// Increment to retry a failed load without changing its path.
    pub retry_generation: u64,
    requested_path: Option<String>,
    requested_retry_generation: u64,
    handle: Option<Handle<Font>>,
}

impl UiFontAsset {
    /// Create a font source from a path relative to the Bevy asset root.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            status: UiFontAssetStatus::Hidden,
            retry_generation: 0,
            requested_path: None,
            requested_retry_generation: 0,
            handle: None,
        }
    }

    /// Request the current path again after a failure.
    pub fn retry(&mut self) {
        self.retry_generation = self.retry_generation.wrapping_add(1);
    }

    /// Return a strong handle only while the font is successfully loaded.
    pub fn ready_handle(&self) -> Option<Handle<Font>> {
        (self.status == UiFontAssetStatus::Ready)
            .then(|| self.handle.clone())
            .flatten()
    }
}

/// Resolve the nearest ready font asset on `entity` or one of its UI ancestors.
///
/// `fallback` keeps its size, weight, slant, width, and features. Only its font source changes,
/// which lets layout shaping and the renderer use identical metrics without losing theme sizing.
pub fn resolve_text_font(world: &World, entity: Entity, fallback: TextFont) -> TextFont {
    let mut current = Some(entity);
    let mut remaining = world.entities().len();
    while let Some(at) = current {
        if remaining == 0 {
            break;
        }
        remaining -= 1;
        if let Some(handle) = world
            .get::<UiFontAsset>(at)
            .and_then(UiFontAsset::ready_handle)
        {
            return TextFont {
                font: FontSource::list([
                    FontSource::Handle(handle),
                    FontSource::Family(UI_SYMBOL_FONT_FAMILY.into()),
                ]),
                ..fallback
            };
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    fallback
}

/// Update AssetServer font requests before BBUI text shaping and intrinsic measurement.
pub fn sync_ui_font_assets(world: &mut World) {
    let mut query = world.query_filtered::<Entity, bevy_ecs::query::With<UiFontAsset>>();
    let entities = query.iter(world).collect::<Vec<_>>();
    let mut asset_server = None;
    let mut asset_server_checked = false;
    let has_asset_server_resource = world.contains_resource::<AssetServer>();
    let has_font_assets = world.contains_resource::<Assets<Font>>();

    for entity in entities {
        if world.get::<UiFontAsset>(entity).is_none() {
            continue;
        }
        if world.get::<UiFontAssetState>(entity).is_none() {
            let Some(status) = world
                .get::<UiFontAsset>(entity)
                .map(|font_asset| font_asset.status.clone())
            else {
                continue;
            };
            world.entity_mut(entity).insert(UiFontAssetState { status });
        }
        if !font_owner_is_visible(world, entity) {
            let needs_reset = world.get::<UiFontAsset>(entity).is_some_and(|font_asset| {
                font_asset.handle.is_some()
                    || font_asset.requested_path.as_deref() != Some(font_asset.path.as_str())
                    || font_asset.requested_retry_generation != font_asset.retry_generation
            });
            if needs_reset && let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
                font_asset.handle = None;
                font_asset.requested_path = Some(font_asset.path.clone());
                font_asset.requested_retry_generation = font_asset.retry_generation;
            }
            set_status(world, entity, UiFontAssetStatus::Hidden);
            continue;
        }

        let ready_request_is_unchanged =
            world.get::<UiFontAsset>(entity).is_some_and(|font_asset| {
                font_asset.status == UiFontAssetStatus::Ready
                    && font_asset.requested_path.as_deref() == Some(font_asset.path.as_str())
                    && font_asset.requested_retry_generation == font_asset.retry_generation
                    && font_asset.handle.is_some()
            }) && has_asset_server_resource
                && has_font_assets;
        if ready_request_is_unchanged {
            // Ready assets are stable until their path, retry generation, or visibility changes.
            // Reconcile the status mirror without cloning the handle or polling AssetServer.
            set_status(world, entity, UiFontAssetStatus::Ready);
            continue;
        }

        let Some(snapshot) = world.get::<UiFontAsset>(entity).cloned() else {
            continue;
        };

        if !asset_server_checked {
            asset_server = world.get_resource::<AssetServer>().cloned();
            asset_server_checked = true;
        }
        let Some(asset_server) = asset_server.as_ref() else {
            clear_font_handle(world, entity);
            set_status(
                world,
                entity,
                UiFontAssetStatus::Failed("Bevy AssetServer is not installed".to_owned()),
            );
            continue;
        };
        if !has_font_assets {
            clear_font_handle(world, entity);
            set_status(
                world,
                entity,
                UiFontAssetStatus::Failed("Bevy font assets are not initialized".to_owned()),
            );
            continue;
        }

        if snapshot.path.trim().is_empty() {
            let needs_reset = world.get::<UiFontAsset>(entity).is_some_and(|font_asset| {
                font_asset.handle.is_some()
                    || font_asset.requested_path.as_deref() != Some(snapshot.path.as_str())
                    || font_asset.requested_retry_generation != snapshot.retry_generation
            });
            if needs_reset && let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
                font_asset.handle = None;
                font_asset.requested_path = Some(snapshot.path);
                font_asset.requested_retry_generation = snapshot.retry_generation;
            }
            set_status(
                world,
                entity,
                UiFontAssetStatus::Failed("font asset path is empty".to_owned()),
            );
            continue;
        }

        let path_changed = snapshot.requested_path.as_deref() != Some(snapshot.path.as_str());
        let retry_changed = snapshot.requested_retry_generation != snapshot.retry_generation;
        let needs_request =
            path_changed || retry_changed || snapshot.status == UiFontAssetStatus::Hidden;
        if needs_request {
            if retry_changed
                && !path_changed
                && matches!(snapshot.status, UiFontAssetStatus::Failed(_))
            {
                asset_server.reload(snapshot.path.clone());
            }
            let handle = asset_server.load::<Font>(snapshot.path.clone());
            if let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
                font_asset.requested_path = Some(snapshot.path.clone());
                font_asset.requested_retry_generation = snapshot.retry_generation;
                font_asset.handle = Some(handle);
            }
            set_status(world, entity, UiFontAssetStatus::Loading);
            continue;
        }

        let Some(handle) = snapshot.handle.as_ref() else {
            continue;
        };
        let status = match asset_server.get_load_state(handle.id()) {
            Some(LoadState::Failed(error)) => UiFontAssetStatus::Failed(error.to_string()),
            Some(LoadState::Loaded)
                if world.resource::<Assets<Font>>().get(handle.id()).is_some() =>
            {
                UiFontAssetStatus::Ready
            }
            Some(LoadState::Loaded) => UiFontAssetStatus::Failed(
                "AssetServer reported a loaded font without a font asset".to_owned(),
            ),
            _ => UiFontAssetStatus::Loading,
        };
        if matches!(status, UiFontAssetStatus::Failed(_)) {
            clear_font_handle(world, entity);
        }
        set_status(world, entity, status);
    }
}

fn clear_font_handle(world: &mut World, entity: Entity) {
    if world
        .get::<UiFontAsset>(entity)
        .is_some_and(|font_asset| font_asset.handle.is_some())
        && let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity)
    {
        font_asset.handle = None;
    }
}

fn set_status(world: &mut World, entity: Entity, status: UiFontAssetStatus) {
    let asset_status_changed = world
        .get::<UiFontAsset>(entity)
        .is_some_and(|font_asset| font_asset.status != status);
    if asset_status_changed && let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
        font_asset.status = status.clone();
    }
    let state_status_changed = world
        .get::<UiFontAssetState>(entity)
        .is_none_or(|state| state.status != status);
    if state_status_changed && world.get_entity(entity).is_ok() {
        world.entity_mut(entity).insert(UiFontAssetState { status });
    }
}

fn font_owner_is_visible(world: &World, entity: Entity) -> bool {
    let mut current = Some(entity);
    let mut remaining = world.entities().len();
    while let Some(at) = current {
        if remaining == 0 || world.get::<UiVisible>(at).is_some_and(|visible| !visible.0) {
            return false;
        }
        remaining -= 1;
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }

    let Some(layout) = world.get::<UiLayoutResult>(entity) else {
        return true;
    };
    let viewport = world
        .get_resource::<UiViewports>()
        .and_then(|viewports| {
            viewports
                .0
                .get(&layout.window)
                .map(|viewport| viewport.rect)
        })
        .or_else(|| {
            world
                .get::<UiLayoutRoot>(layout.window)
                .map(|root| root.viewport)
        });
    let clip = layout.clip.or(viewport);
    clip.is_none_or(|clip| layout.rect.intersection(clip).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::prelude::{App, MinimalPlugins};
    use bevy_text::TextPlugin as BevyTextPlugin;

    #[test]
    fn stable_ready_font_skips_load_state_poll_and_component_writes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), BevyTextPlugin));

        let mut font_asset = UiFontAsset::new("idle.ttf");
        font_asset.status = UiFontAssetStatus::Ready;
        font_asset.requested_path = Some("idle.ttf".to_owned());
        font_asset.handle = Some(Handle::default());
        let entity = app
            .world_mut()
            .spawn((
                font_asset,
                UiFontAssetState {
                    status: UiFontAssetStatus::Ready,
                },
            ))
            .id();
        let asset_changed = app
            .world()
            .get_entity(entity)
            .unwrap()
            .get_change_ticks::<UiFontAsset>()
            .unwrap()
            .changed;
        let state_changed = app
            .world()
            .get_entity(entity)
            .unwrap()
            .get_change_ticks::<UiFontAssetState>()
            .unwrap()
            .changed;

        for _ in 0..2 {
            app.world_mut().increment_change_tick();
            sync_ui_font_assets(app.world_mut());
            assert_eq!(
                app.world().get::<UiFontAsset>(entity).unwrap().status,
                UiFontAssetStatus::Ready,
                "polling this deliberately unloaded handle would demote it to Loading"
            );
            assert_eq!(
                app.world()
                    .get_entity(entity)
                    .unwrap()
                    .get_change_ticks::<UiFontAsset>()
                    .unwrap()
                    .changed,
                asset_changed,
                "idle ready fonts must not write their source component"
            );
            assert_eq!(
                app.world()
                    .get_entity(entity)
                    .unwrap()
                    .get_change_ticks::<UiFontAssetState>()
                    .unwrap()
                    .changed,
                state_changed,
                "idle ready fonts must not rewrite their status mirror"
            );
        }
    }
}

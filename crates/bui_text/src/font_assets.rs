//! Visibility-aware AssetServer loading for UI font faces.

use bevy_asset::{AssetServer, Assets, Handle, LoadState};
use bevy_ecs::prelude::{Component, Entity, World};
use bevy_text::{Font, FontCx, FontSource, TextFont};
use bui_core::{UiParent, UiVisible};
use bui_layout::{UiLayoutResult, UiLayoutRoot, UiViewports};

/// Family name used for BUI's bundled proportional default UI face.
pub const UI_DEFAULT_FONT_FAMILY: &str = "BUI UI Sans";

const UI_DEFAULT_FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/Inter[opsz,wght].ttf");

/// Build a theme-sized `TextFont` using BUI's bundled Inter face.
///
/// This only chooses the font family and size. Callers can still provide a custom `TextFont`,
/// while [`resolve_text_font`] lets a ready [`UiFontAsset`] override the family for a subtree.
pub fn ui_default_text_font(font_size: f32) -> TextFont {
    TextFont {
        font: FontSource::Family(UI_DEFAULT_FONT_FAMILY.into()),
        ..TextFont::from_font_size(font_size)
    }
}

/// Register BUI's embedded Inter face in Bevy's shared font database.
///
/// The operation is idempotent, so both `TextPlugin` and `BuiRenderPlugin` can call it. It runs
/// synchronously during plugin setup, before intrinsic measurement or display-list rendering,
/// and does not depend on the host's asset root or filesystem.
pub fn register_default_ui_font(world: &mut World) {
    world.init_resource::<FontCx>();
    let mut font_cx = world.resource_mut::<FontCx>();
    if font_cx
        .collection
        .family_id(UI_DEFAULT_FONT_FAMILY)
        .is_none()
    {
        let font = Font::from_bytes(UI_DEFAULT_FONT_DATA.to_vec());
        font_cx.collection.register_fonts(
            font.data,
            Some(parley::fontique::FontInfoOverride {
                family_name: Some(UI_DEFAULT_FONT_FAMILY),
                ..Default::default()
            }),
        );
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

/// AssetServer-backed font source for a BUI text entity or a parent of text widgets.
///
/// Attach this to an entity with [`crate::UiTextLayout`] to use one font for both shaped editing
/// geometry and rendered text. A font on a parent is inherited by child text controls. The asset
/// is requested only while the component is visible; hiding or clipping its owner drops the BUI
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
                font: FontSource::Handle(handle),
                ..fallback
            };
        }
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    fallback
}

/// Update AssetServer font requests before BUI text shaping and intrinsic measurement.
pub fn sync_ui_font_assets(world: &mut World) {
    let mut query = world.query_filtered::<Entity, bevy_ecs::query::With<UiFontAsset>>();
    let entities = query.iter(world).collect::<Vec<_>>();
    let asset_server = world.get_resource::<AssetServer>().cloned();
    let has_font_assets = world.contains_resource::<Assets<Font>>();

    for entity in entities {
        let Some(snapshot) = world.get::<UiFontAsset>(entity).cloned() else {
            continue;
        };
        if world.get::<UiFontAssetState>(entity).is_none() {
            world.entity_mut(entity).insert(UiFontAssetState {
                status: snapshot.status.clone(),
            });
        }
        if !font_owner_is_visible(world, entity) {
            if let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
                font_asset.handle = None;
                font_asset.requested_path = Some(font_asset.path.clone());
                font_asset.requested_retry_generation = font_asset.retry_generation;
            }
            set_status(world, entity, UiFontAssetStatus::Hidden);
            continue;
        }

        let Some(asset_server) = asset_server.as_ref() else {
            if let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
                font_asset.handle = None;
            }
            set_status(
                world,
                entity,
                UiFontAssetStatus::Failed("Bevy AssetServer is not installed".to_owned()),
            );
            continue;
        };
        if !has_font_assets {
            if let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
                font_asset.handle = None;
            }
            set_status(
                world,
                entity,
                UiFontAssetStatus::Failed("Bevy font assets are not initialized".to_owned()),
            );
            continue;
        }

        if snapshot.path.trim().is_empty() {
            if let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity) {
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
        if matches!(status, UiFontAssetStatus::Failed(_))
            && let Some(mut font_asset) = world.get_mut::<UiFontAsset>(entity)
        {
            font_asset.handle = None;
        }
        set_status(world, entity, status);
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

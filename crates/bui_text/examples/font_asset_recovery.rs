//! Headless demonstration of a missing UI font falling back and recovering on retry.
//!
//! Run with `cargo run -p bui_text --example font_asset_recovery`.

use std::{
    fs, io, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bevy::{
    app::App, asset::AssetPlugin, prelude::MinimalPlugins, text::TextPlugin as BevyTextPlugin,
};
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot};
use bui_text::{
    TextBuffer, TextPlugin as BuiTextPlugin, UiFontAsset, UiFontAssetStatus, UiTextLayout,
    resolve_text_font,
};

const FONT_BYTES: &[u8] = include_bytes!("../tests/assets/NotoSansSymbols2-U+1F7D5.ttf");

fn wait_for_status(
    app: &mut App,
    entity: bevy::ecs::entity::Entity,
    wanted: fn(&UiFontAssetStatus) -> bool,
) -> io::Result<UiFontAssetStatus> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if let Some(status) = app
            .world()
            .get::<UiFontAsset>(entity)
            .map(|asset| asset.status.clone())
            && wanted(&status)
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("timed out waiting for font state"));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn main() -> io::Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let asset_root = std::env::temp_dir().join(format!("bui-font-demo-{nonce}"));
    fs::create_dir_all(&asset_root)?;
    // Use the canonical spelling so AssetServer and file notifications agree on macOS.
    let asset_root = fs::canonicalize(asset_root)?;

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        BevyTextPlugin,
        BuiTextPlugin,
    ));

    let window = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(window).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 240.0, 80.0),
        1.0,
    ));
    let label = app
        .world_mut()
        .spawn((
            UiFontAsset::new("recovery.ttf"),
            UiTextLayout::default(),
            TextBuffer::new("A custom font with the default-font fallback"),
            UiLayoutNode::default(),
            UiLayoutResult {
                window,
                rect: Rect::new(0.0, 0.0, 240.0, 32.0),
                clip: Some(Rect::new(0.0, 0.0, 240.0, 80.0)),
                z: 0,
                content_size: [240.0, 32.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    let failed = wait_for_status(&mut app, label, |status| {
        matches!(status, UiFontAssetStatus::Failed(_))
    })?;
    let fallback_layout = app
        .world()
        .get::<UiTextLayout>(label)
        .ok_or_else(|| io::Error::other("label has no BUI text layout"))?;
    if fallback_layout.layout().is_none() {
        return Err(io::Error::other("fallback font did not shape the label"));
    }
    let fallback_font = bevy::text::TextFont::from_font_size(16.0);
    let fallback_source = fallback_font.font.clone();
    let fallback_font = resolve_text_font(app.world(), label, fallback_font);
    println!("missing file: {failed:?}; default font still shaped the label");

    fs::write(asset_root.join("recovery.ttf"), FONT_BYTES)?;
    app.world_mut()
        .get_mut::<UiFontAsset>(label)
        .ok_or_else(|| io::Error::other("font asset component is missing"))?
        .retry();
    let ready = wait_for_status(&mut app, label, |status| {
        *status == UiFontAssetStatus::Ready
    })?;
    let resolved_font = resolve_text_font(app.world(), label, fallback_font);
    if resolved_font.font == fallback_source
        || resolved_font.font_size != bevy::text::FontSize::Px(16.0)
    {
        return Err(io::Error::other("font recovery did not preserve text size"));
    }
    println!("retry: {ready:?}; resolved font changed while text size was preserved");

    fs::remove_dir_all(asset_root)?;
    Ok(())
}

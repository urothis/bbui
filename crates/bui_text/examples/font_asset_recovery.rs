//! Headless demonstration of a missing UI font falling back and recovering on retry.
//!
//! Run with `cargo run -p bui_text --example font_asset_recovery`.

use std::{
    fs, thread,
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
) -> UiFontAssetStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if let Some(status) = app
            .world()
            .get::<UiFontAsset>(entity)
            .map(|asset| asset.status.clone())
            && wanted(&status)
        {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for font state"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn main() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let asset_root = std::env::temp_dir().join(format!("bui-font-demo-{nonce}"));
    fs::create_dir_all(&asset_root).expect("create temporary asset directory");
    // Use the canonical spelling so AssetServer and file notifications agree on macOS.
    let asset_root = fs::canonicalize(asset_root).expect("canonicalize temporary asset directory");

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
    });
    let fallback_layout = app
        .world()
        .get::<UiTextLayout>(label)
        .expect("label has a BUI text layout");
    assert!(fallback_layout.layout().is_some());
    let fallback_font = bevy::text::TextFont::from_font_size(16.0);
    let fallback_source = fallback_font.font.clone();
    let fallback_font = resolve_text_font(app.world(), label, fallback_font);
    println!("missing file: {failed:?}; default font still shaped the label");

    fs::write(asset_root.join("recovery.ttf"), FONT_BYTES).expect("install font fixture");
    app.world_mut()
        .get_mut::<UiFontAsset>(label)
        .expect("font asset component exists")
        .retry();
    let ready = wait_for_status(&mut app, label, |status| {
        *status == UiFontAssetStatus::Ready
    });
    let resolved_font = resolve_text_font(app.world(), label, fallback_font);
    assert_ne!(resolved_font.font, fallback_source);
    assert_eq!(resolved_font.font_size, bevy::text::FontSize::Px(16.0));
    println!("retry: {ready:?}; resolved font changed while text size was preserved");

    fs::remove_dir_all(asset_root).expect("remove temporary asset directory");
}

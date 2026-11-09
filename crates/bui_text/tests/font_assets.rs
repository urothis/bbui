use std::{
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bevy::ecs::change_detection::{DetectChanges, Ref};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::Query;
use bevy::prelude::{ResMut, Resource};
use bevy::{
    app::Update,
    asset::{AssetPlugin, Assets},
    prelude::{App, MinimalPlugins},
    text::{Font, FontSource, TextFont, TextPlugin as BevyTextPlugin},
};
use bui_layout::{Rect, UiLayoutNode, UiLayoutResult, UiLayoutRoot};
use bui_text::{
    TextBuffer, TextPlugin as BuiTextPlugin, UiFontAsset, UiFontAssetState, UiFontAssetStatus,
    UiTextLayout, resolve_text_font, sync_ui_font_assets,
};

const SYMBOL_FONT: &[u8] = include_bytes!("assets/NotoSansSymbols2-U+1F7D5.ttf");

#[derive(Resource, Default)]
struct FontChangeObservation {
    font_changed: Option<bool>,
    state_changed: Option<bool>,
}

fn observe_font_changes(
    fonts: Query<Ref<UiFontAsset>>,
    states: Query<Ref<UiFontAssetState>>,
    mut observation: ResMut<FontChangeObservation>,
) {
    if let Some(font) = fonts.iter().next() {
        observation.font_changed = Some(font.is_changed());
    }
    if let Some(state) = states.iter().next() {
        observation.state_changed = Some(state.is_changed());
    }
}

fn temp_directory() -> std::io::Result<std::path::PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("bui-font-assets-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory)?;
    // Use the same spelling the filesystem watcher reports on macOS.
    fs::canonicalize(directory)
}

fn await_status(app: &mut App, entity: bevy::prelude::Entity, expected: UiFontAssetStatus) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if app
            .world()
            .get::<UiFontAsset>(entity)
            .is_some_and(|font| font.status == expected)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "font asset did not reach {expected:?}; status={:?}",
            app.world()
                .get::<UiFontAsset>(entity)
                .map(|font| &font.status)
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn await_failed(app: &mut App, entity: bevy::prelude::Entity) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if app
            .world()
            .get::<UiFontAsset>(entity)
            .is_some_and(|font| matches!(font.status, UiFontAssetStatus::Failed(_)))
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "font asset did not enter failure state"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn missing_font_falls_back_recovers_and_releases_when_offscreen() {
    let directory = temp_directory().unwrap();

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: directory.to_string_lossy().into_owned(),
            ..Default::default()
        },
        BevyTextPlugin,
        BuiTextPlugin,
    ));

    let window = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(window).insert(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 200.0, 100.0),
        1.0,
    ));
    let entity = app
        .world_mut()
        .spawn((
            UiFontAsset::new("recovery.ttf"),
            UiTextLayout::default(),
            TextBuffer::new("🟕"),
            UiLayoutNode::default(),
            UiLayoutResult {
                window,
                rect: Rect::new(0.0, 0.0, 180.0, 36.0),
                clip: Some(Rect::new(0.0, 0.0, 200.0, 100.0)),
                z: 0,
                content_size: [180.0, 36.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    let fallback = TextFont::from_font_size(18.0);
    await_failed(&mut app, entity);
    let status = app
        .world()
        .get::<UiFontAsset>(entity)
        .unwrap()
        .status
        .clone();
    assert!(matches!(status, UiFontAssetStatus::Failed(ref message) if !message.is_empty()));
    assert_eq!(
        app.world().get::<UiFontAssetState>(entity).unwrap().status,
        app.world().get::<UiFontAsset>(entity).unwrap().status
    );
    for _ in 0..5 {
        app.update();
        assert_eq!(
            app.world().get::<UiFontAsset>(entity).unwrap().status,
            status,
            "a failed load should remain stable until retry or path change"
        );
    }
    app.init_resource::<FontChangeObservation>();
    app.add_systems(Update, observe_font_changes.after(sync_ui_font_assets));
    // The first observer run establishes its change-detection baseline. On the next idle frame,
    // stable Failed status must not mark either component changed again.
    app.update();
    app.update();
    let changes = app.world().resource::<FontChangeObservation>();
    assert_eq!(changes.font_changed, Some(false));
    assert_eq!(changes.state_changed, Some(false));
    assert!(
        app.world()
            .get::<UiFontAsset>(entity)
            .unwrap()
            .ready_handle()
            .is_none()
    );
    assert_eq!(
        resolve_text_font(app.world(), entity, fallback.clone()),
        fallback,
        "failed font loading leaves Bevy's default font and size intact"
    );
    assert!(
        app.world()
            .get::<UiTextLayout>(entity)
            .unwrap()
            .layout()
            .is_some(),
        "default-font fallback must continue shaping text"
    );

    fs::write(directory.join("recovery.ttf"), SYMBOL_FONT).unwrap();
    app.world_mut()
        .get_mut::<UiFontAsset>(entity)
        .unwrap()
        .retry();
    await_status(&mut app, entity, UiFontAssetStatus::Ready);
    app.update();

    let retained = app
        .world()
        .get::<UiFontAsset>(entity)
        .unwrap()
        .ready_handle()
        .expect("ready font must expose its Bevy asset handle");
    let font_id = retained.id();
    let resolved = resolve_text_font(app.world(), entity, fallback.clone());
    assert!(matches!(resolved.font, FontSource::Handle(ref handle) if handle.id() == font_id));
    assert_eq!(resolved.font_size, fallback.font_size);
    let shaped = app.world().get::<UiTextLayout>(entity).unwrap();
    assert!(shaped.layout().is_some());
    assert!(shaped.last_error().is_none());

    // A clipped/offscreen entity drops its owning handle and reverts to fallback shaping.
    app.world_mut()
        .get_mut::<UiLayoutResult>(entity)
        .unwrap()
        .rect = Rect::new(220.0, 0.0, 180.0, 36.0);
    await_status(&mut app, entity, UiFontAssetStatus::Hidden);
    assert!(
        app.world()
            .get::<UiFontAsset>(entity)
            .unwrap()
            .ready_handle()
            .is_none()
    );
    assert_eq!(
        resolve_text_font(app.world(), entity, fallback.clone()),
        fallback
    );
    assert!(
        app.world()
            .resource::<Assets<Font>>()
            .get(font_id)
            .is_some(),
        "an external strong handle must keep the Bevy font asset alive"
    );
    drop(retained);

    app.world_mut()
        .get_mut::<UiLayoutResult>(entity)
        .unwrap()
        .rect = Rect::new(0.0, 0.0, 180.0, 36.0);
    await_status(&mut app, entity, UiFontAssetStatus::Ready);
    assert!(
        app.world()
            .resource::<Assets<Font>>()
            .get(font_id)
            .is_some()
    );

    fs::remove_dir_all(directory).unwrap();
}

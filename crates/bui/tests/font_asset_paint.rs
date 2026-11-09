//! Verifies ready font assets flow from a widget-tree ancestor into generic TextWrapped labels.

use std::{
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bevy::{
    asset::AssetPlugin,
    prelude::{App, MinimalPlugins},
    text::{Font, FontSource, TextFont, TextPlugin as BevyTextPlugin},
};
use bui::widgets::WidgetNode;
use bui_core::UiChildren;
use bui_layout::{Rect, UiLayoutResult, UiLayoutRoot};
use bui_render::{DisplayList, Primitive};
use bui_text::{
    TextPlugin, UI_DEFAULT_FONT_FAMILY, UiFontAsset, UiFontAssetStatus, sync_ui_font_assets,
};
use bui_widgets::{WidgetPlugin, WidgetTree, actions::collect_widget_paint};

const FONT_BYTES: &[u8] =
    include_bytes!("../../bui_text/tests/assets/NotoSansSymbols2-U+1F7D5.ttf");

#[cfg(test)]
fn temp_directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("bui-font-paint-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    // Keep AssetServer and watcher paths consistent with macOS's /var symlink target.
    fs::canonicalize(directory).unwrap()
}

fn await_font_status(app: &mut App, entity: bevy::prelude::Entity, ready: bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let is_ready = app
            .world()
            .get::<UiFontAsset>(entity)
            .is_some_and(|font| font.status == UiFontAssetStatus::Ready);
        if is_ready == ready {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "font failed to reach ready={ready}: {:?}",
            app.world()
                .get::<UiFontAsset>(entity)
                .map(|font| &font.status)
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
fn generic_label_font(app: &App, label: bevy::prelude::Entity) -> TextFont {
    let items = app.world().resource::<DisplayList>().widget_items();
    let layout = app.world().get::<bui_layout::UiLayoutResult>(label);
    items
        .iter()
        .find_map(|item| match &item.primitive {
            Primitive::TextWrapped {
                text,
                style: Some(style),
                ..
            } if text == "Asset-backed label" => style.font.clone(),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "generic label {label:?} had no TextWrapped font: {items:?}; layout={layout:?}; label={:?}",
                app.world().get::<bui_widgets::Label>(label).map(|label| &label.text)
            )
        })
}

#[test]
fn ready_inherited_font_reaches_generic_label_and_hidden_state_falls_back() {
    let directory = temp_directory();
    fs::write(directory.join("label.ttf"), FONT_BYTES).unwrap();

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: directory.to_string_lossy().into_owned(),
            ..Default::default()
        },
        BevyTextPlugin,
        TextPlugin,
    ));
    let window = app.world_mut().spawn_empty().id();
    let root = WidgetTree::new(
        WidgetNode::column([WidgetNode::label("Asset-backed label")])
            .with_width(bui_layout::UiLength::Px(200.0))
            .with_height(bui_layout::UiLength::Px(48.0)),
    )
    .spawn(app.world_mut(), None)
    .unwrap();
    app.world_mut().entity_mut(root).insert((
        UiFontAsset::new("label.ttf"),
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 200.0, 48.0), 1.0),
        UiLayoutResult {
            window,
            rect: Rect::new(0.0, 0.0, 200.0, 48.0),
            clip: Some(Rect::new(0.0, 0.0, 200.0, 48.0)),
            z: 0,
            content_size: [200.0, 48.0],
            dpi_scale: 1.0,
        },
    ));
    let label = *app
        .world()
        .get::<UiChildren>(root)
        .unwrap()
        .0
        .first()
        .expect("tree has one label child");
    app.world_mut().entity_mut(label).insert(UiLayoutResult {
        window,
        rect: Rect::new(0.0, 0.0, 200.0, 24.0),
        clip: Some(Rect::new(0.0, 0.0, 200.0, 48.0)),
        z: 1,
        content_size: [200.0, 24.0],
        dpi_scale: 1.0,
    });

    await_font_status(&mut app, root, true);
    let handle = app
        .world()
        .get::<UiFontAsset>(root)
        .unwrap()
        .ready_handle()
        .expect("ready parent font has a handle");
    app.add_plugins(WidgetPlugin);
    collect_widget_paint(app.world_mut());
    let painted_font = generic_label_font(&app, label);
    assert!(matches!(
        painted_font.font,
        FontSource::Handle(ref painted) if painted.id() == handle.id()
    ));
    assert_eq!(
        painted_font.font_size,
        TextFont::from_font_size(14.0).font_size
    );

    app.world_mut()
        .get_mut::<UiLayoutResult>(root)
        .unwrap()
        .rect = Rect::new(240.0, 0.0, 200.0, 48.0);
    sync_ui_font_assets(app.world_mut());
    assert_eq!(
        app.world().get::<UiFontAsset>(root).unwrap().status,
        UiFontAssetStatus::Hidden
    );
    collect_widget_paint(app.world_mut());
    let fallback_font = generic_label_font(&app, label);
    assert!(matches!(
        fallback_font.font,
        FontSource::Family(ref family) if family == UI_DEFAULT_FONT_FAMILY
    ));
    assert!(!matches!(
        fallback_font.font,
        FontSource::Handle(ref painted) if painted.id() == handle.id()
    ));
    assert!(
        app.world()
            .resource::<bevy::asset::Assets<Font>>()
            .get(handle.id())
            .is_some()
    );

    fs::remove_dir_all(directory).unwrap();
}

use std::{
    fs,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

use bevy_app::App;
use bevy_asset::Assets;
use bevy_ecs::prelude::World;
use bevy_tasks::{AsyncComputeTaskPool, TaskPoolBuilder};

use super::*;

fn preview_world() -> World {
    let mut world = World::new();
    world.insert_resource(FileBrowserPreviewCache::default());
    world.insert_resource(Assets::<Image>::default());
    world.insert_resource(ImageRegistry::default());
    world.insert_resource(DisplayList::default());
    world
}

#[test]
fn settled_preview_pass_skips_work_until_browser_input_changes() {
    let mut app = App::new();
    app.insert_resource(Assets::<Image>::default());
    app.insert_resource(ImageRegistry::default());
    app.insert_resource(DisplayList::default());
    install(&mut app);

    app.update();
    let first = app
        .world()
        .resource::<FileBrowserPreviewCache>()
        .sync_passes();
    assert_eq!(first, 1);
    app.update();
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .sync_passes(),
        first
    );

    app.world_mut()
        .resource_mut::<FileBrowserPreviewCache>()
        .entries
        .insert(
            PathBuf::from("cached-preview.png"),
            PreviewEntry {
                state: PreviewState::Failed("fixture".to_owned()),
                visible_owners: HashSet::new(),
                last_used: Duration::ZERO,
            },
        );
    app.world_mut().spawn((UiNode, UiVisible(true)));
    app.update();
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .sync_passes(),
        first,
        "an unrelated visibility change does not rescan browser previews"
    );

    app.world_mut().spawn(FileBrowserItem {
        owner: Entity::PLACEHOLDER,
        path: PathBuf::from("/unused-preview.png"),
        name: "unused-preview.png".to_owned(),
        index: 0,
        directory: false,
        size: 0,
        selected: false,
        preview: PreviewKind::Image,
    });
    app.update();
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .sync_passes(),
        first + 1
    );
}

#[test]
fn hidden_file_browser_items_do_not_start_preview_decoding() {
    let temp = std::env::temp_dir().join(format!(
        "bbui-hidden-preview-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp).unwrap();
    let path = temp.join("preview.png");
    fs::write(&path, b"fixture").unwrap();

    let mut world = preview_world();
    let mut browser = FileBrowser::new(&temp).unwrap();
    browser.view = FileView::Grid;
    let owner = world.spawn((UiNode, UiVisible(false), browser)).id();
    let item = world
        .spawn((
            UiNode,
            UiVisible(true),
            UiParent(owner),
            FileBrowserItem {
                owner,
                path: path.clone(),
                name: "preview.png".to_owned(),
                index: 0,
                directory: false,
                size: 7,
                selected: false,
                preview: PreviewKind::Image,
            },
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: Rect::new(0.0, 48.0, 120.0, 78.0),
                clip: Some(Rect::new(0.0, 0.0, 120.0, 126.0)),
                z: 1,
                content_size: [120.0, 78.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    sync_file_browser_previews(&mut world);
    assert_eq!(
        world.resource::<FileBrowserPreviewCache>().loading_count(),
        0
    );
    assert_eq!(world.resource::<FileBrowserPreviewCache>().ready_count(), 0);
    assert!(world.get_entity(item).is_ok());

    world.get_mut::<UiVisible>(owner).unwrap().0 = true;
    world.get_mut::<UiLayoutResult>(item).unwrap().rect.y = 500.0;
    sync_file_browser_previews(&mut world);
    assert_eq!(
        world.resource::<FileBrowserPreviewCache>().loading_count(),
        0,
        "a mounted item outside its inherited clip must not own a decode"
    );

    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn unowned_pending_preview_work_is_cancelled_and_released() {
    let _ = AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
    let cancelled = Arc::new(AtomicBool::new(false));
    let task = AsyncComputeTaskPool::get()
        .spawn(async { std::future::pending::<Result<RgbaFrame, String>>().await });
    let path = PathBuf::from("unmounted-preview.png");
    let mut world = preview_world();
    world
        .resource_mut::<FileBrowserPreviewCache>()
        .entries
        .insert(
            path,
            PreviewEntry {
                state: PreviewState::Loading {
                    task,
                    cancelled: Arc::clone(&cancelled),
                },
                visible_owners: HashSet::new(),
                last_used: Duration::ZERO,
            },
        );

    sync_file_browser_previews(&mut world);
    assert!(cancelled.load(Ordering::Acquire));
    assert_eq!(
        world.resource::<FileBrowserPreviewCache>().loading_count(),
        0
    );
}

#[test]
fn preview_expiry_starts_when_the_last_visible_owner_disappears() {
    let mut world = preview_world();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_secs(10));
    world.insert_resource(time);
    let path = PathBuf::from("cached-preview.png");
    world
        .resource_mut::<FileBrowserPreviewCache>()
        .entries
        .insert(
            path.clone(),
            PreviewEntry {
                state: PreviewState::Failed("fixture".to_owned()),
                visible_owners: HashSet::from([Entity::from_bits(1)]),
                last_used: Duration::ZERO,
            },
        );

    sync_file_browser_previews(&mut world);
    assert!(
        world
            .resource::<FileBrowserPreviewCache>()
            .entries
            .contains_key(&path)
    );
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(4));
    sync_file_browser_previews(&mut world);
    assert!(
        world
            .resource::<FileBrowserPreviewCache>()
            .entries
            .contains_key(&path)
    );
    world
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs(2));
    sync_file_browser_previews(&mut world);
    assert!(
        !world
            .resource::<FileBrowserPreviewCache>()
            .entries
            .contains_key(&path)
    );
}

#[test]
fn vp8_and_h264_video_posters_decode_and_unsupported_video_has_an_explicit_error() {
    let root = std::env::temp_dir().join(format!(
        "bbui-video-preview-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();

    for extension in ["webm", "mkv"] {
        let path = root.join(format!("sample.{extension}"));
        fs::write(
            &path,
            include_bytes!("../../../bbui_media/assets/sample.webm"),
        )
        .unwrap();
        let poster =
            decode_preview_file(path, PreviewKind::Video, Arc::new(AtomicBool::new(false)))
                .unwrap();
        assert_eq!((poster.width, poster.height), (16, 16));
        assert!(poster.pixels[0] > poster.pixels[1]);
    }

    for extension in ["mp4", "mov"] {
        let path = root.join(format!("sample.{extension}"));
        let mut fixture = include_bytes!("../../../bbui_media/assets/sample-h264.mp4").to_vec();
        if extension == "mov" {
            fixture[8..12].copy_from_slice(b"qt  ");
        }
        fs::write(&path, fixture).unwrap();
        let poster =
            decode_preview_file(path, PreviewKind::Video, Arc::new(AtomicBool::new(false)))
                .unwrap();
        assert_eq!((poster.width, poster.height), (32, 18));
        assert!(
            poster
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] != pixel[1])
        );
    }

    let unsupported = root.join("sample.avi");
    fs::write(&unsupported, b"unsupported video fixture").unwrap();
    let error = decode_preview_file(
        unsupported,
        PreviewKind::Video,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap_err();
    assert!(error.contains("not supported for .avi"), "{error}");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn loading_and_failed_preview_states_publish_visible_status_text() {
    let _ = AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(1).build());
    let root = std::env::temp_dir().join(format!(
        "bbui-preview-status-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();

    let mut world = preview_world();
    world.insert_resource(Theme::default());
    let mut browser = FileBrowser::new(&root).unwrap();
    browser.view = FileView::Grid;
    let owner = world.spawn((UiNode, browser)).id();
    let paths = [root.join("loading.webm"), root.join("broken.mp4")];
    let mut entities = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let entity = world
            .spawn((
                UiNode,
                UiParent(owner),
                FileBrowserItem {
                    owner,
                    path: path.clone(),
                    name: path.file_name().unwrap().to_string_lossy().into_owned(),
                    index,
                    directory: false,
                    size: 0,
                    selected: false,
                    preview: PreviewKind::Video,
                },
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: Rect::new(0.0, (index as f32) * 80.0, 120.0, 78.0),
                    clip: Some(Rect::new(0.0, 0.0, 120.0, 160.0)),
                    z: 1,
                    content_size: [120.0, 78.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        entities.push(entity);
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let task = AsyncComputeTaskPool::get()
        .spawn(async { std::future::pending::<Result<RgbaFrame, String>>().await });
    {
        let mut cache = world.resource_mut::<FileBrowserPreviewCache>();
        cache.entries.insert(
            paths[0].clone(),
            PreviewEntry {
                state: PreviewState::Loading {
                    task,
                    cancelled: Arc::clone(&cancelled),
                },
                visible_owners: HashSet::from([entities[0]]),
                last_used: Duration::ZERO,
            },
        );
        cache.entries.insert(
            paths[1].clone(),
            PreviewEntry {
                state: PreviewState::Failed("unsupported MP4 container".to_owned()),
                visible_owners: HashSet::from([entities[1]]),
                last_used: Duration::ZERO,
            },
        );
    }

    let visible = collect_visible_items(&mut world);
    publish_visible_overlays(&mut world, &visible);
    let status_texts: Vec<_> = world
        .resource::<DisplayList>()
        .overlay_camera_items(Entity::PLACEHOLDER)
        .unwrap()
        .iter()
        .filter_map(|item| match &item.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(status_texts.contains(&"Loading preview…"));
    assert!(status_texts.contains(&"Preview unavailable"));
    cancelled.store(true, Ordering::Release);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn thumbnail_bounds_preserve_source_aspect_ratio_without_upscaling() {
    assert_eq!(
        aspect_fit(Rect::new(0.0, 0.0, 104.0, 50.0), 1, 1),
        Some(Rect::new(27.0, 0.0, 50.0, 50.0))
    );
    assert_eq!(
        aspect_fit(Rect::new(0.0, 0.0, 104.0, 50.0), 2, 1),
        Some(Rect::new(2.0, 0.0, 100.0, 50.0))
    );
}

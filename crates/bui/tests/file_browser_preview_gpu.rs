//! Pixel coverage for raster, VP8, and H.264 previews decoded from visible file-browser items.
#![cfg(target_os = "linux")]

use std::{
    collections::HashMap,
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use bevy::{
    app::PluginGroup,
    asset::Assets,
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, ResMut, Resource, WindowPlugin},
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use bui::{BuiPlugin, media};
use bui_core::UiVisible;
use bui_layout::{Rect, UiLayoutNode, UiLayoutRoot, UiLength};
use bui_render::{ClipRect, DisplayList, ImageRegistry, Primitive};
use bui_widgets::{
    FileBrowser, FileView, WidgetNode, WidgetTree, file_previews::FileBrowserPreviewCache,
};

const TARGET_WIDTH: u32 = 256;
const TARGET_HEIGHT: u32 = 192;
const PNG_FIXTURE: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const WEBM_FIXTURE: &[u8] = include_bytes!("../../bui_media/assets/sample.webm");
const H264_MP4_FIXTURE: &[u8] = include_bytes!("../../bui_media/assets/sample-h264.mp4");

#[derive(Resource, Default)]
struct CapturedPixels(HashMap<usize, Vec<u8>>);

fn pixel_at(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let bytes_per_row = (TARGET_WIDTH as usize * 4).div_ceil(256) * 256;
    let offset = y * bytes_per_row + x * 4;
    &pixels[offset..offset + 4]
}

fn thumbnail_pixel_at(frame: &media::RgbaFrame, rect: ClipRect, x: usize, y: usize) -> &[u8] {
    let u = ((x as f32 + 0.5 - rect.x) / rect.width).clamp(0.0, 1.0);
    let v = ((y as f32 + 0.5 - rect.y) / rect.height).clamp(0.0, 1.0);
    let source_x = ((u * frame.width as f32).floor() as usize).min(frame.width as usize - 1);
    let source_y = ((v * frame.height as f32).floor() as usize).min(frame.height as usize - 1);
    let offset = (source_y * frame.width as usize + source_x) * 4;
    &frame.pixels[offset..offset + 4]
}

fn temp_directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("bui-file-preview-{}-{nonce}", std::process::id()))
}

#[test]
fn visible_file_browser_grid_decodes_and_gpu_draws_raster_vp8_and_h264_previews() {
    let directory = temp_directory();
    fs::create_dir_all(&directory).unwrap();
    let image_path = directory.join("preview.png");
    fs::write(&image_path, PNG_FIXTURE).unwrap();
    let video_path = directory.join("sample.webm");
    fs::write(&video_path, WEBM_FIXTURE).unwrap();
    let h264_path = directory.join("sample.mp4");
    fs::write(&h264_path, H264_MP4_FIXTURE).unwrap();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    force_fallback_adapter: true,
                    ..Default::default()
                })),
                ..Default::default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(BuiPlugin)
    .init_resource::<CapturedPixels>();
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.clone().into())))
        .id();
    let browser = WidgetTree::new(WidgetNode::file_browser(&directory))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut()
        .get_mut::<FileBrowser>(browser)
        .unwrap()
        .view = FileView::Grid;
    app.world_mut().entity_mut(browser).insert((
        UiVisible(false),
        UiLayoutRoot::new(camera, Rect::new(0.0, 0.0, 240.0, 160.0), 1.0),
    ));
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(browser).unwrap();
        layout.size.width = UiLength::Px(240.0);
        layout.size.height = UiLength::Px(160.0);
        layout.clip_contents = true;
    }

    for _ in 0..5 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .loading_count(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .ready_count(),
        0
    );
    assert!(
        app.world()
            .resource::<DisplayList>()
            .overlay_camera_items(camera)
            .is_none()
    );

    app.world_mut().entity_mut(browser).insert(UiVisible(true));
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline
        && app
            .world()
            .resource::<FileBrowserPreviewCache>()
            .ready_count()
            < 3
    {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    {
        let cache = app.world().resource::<FileBrowserPreviewCache>();
        assert_eq!(
            cache.ready_count(),
            3,
            "preview decode failed: image={:?}, VP8={:?}, H.264={:?}",
            cache.failure(&image_path),
            cache.failure(&video_path),
            cache.failure(&h264_path),
        );
        assert_eq!(cache.completed_decodes(), 3);
        assert_eq!(cache.cached_bytes(), 4 + 16 * 16 * 4 + 32 * 18 * 4);
    }

    let overlays = app
        .world()
        .resource::<DisplayList>()
        .overlay_camera_items(camera)
        .expect("visible image should be registered as a camera overlay");
    let image_items: Vec<_> = overlays
        .iter()
        .filter_map(|item| match &item.primitive {
            Primitive::Image { rect, asset_id } => Some((item.clip, *rect, *asset_id)),
            _ => None,
        })
        .collect();
    assert_eq!(image_items.len(), 3);
    let handle = app
        .world()
        .resource::<ImageRegistry>()
        .get(image_items[0].2)
        .expect("preview image ID should resolve to its Bevy handle");
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(handle.id())
            .is_some()
    );
    for (clip, rect, _) in &image_items {
        assert!(rect.x >= 8.0 && rect.y >= 68.0);
        assert!(
            clip.x >= 0.0
                && clip.y >= 48.0
                && clip.x + clip.width <= 240.0
                && clip.y + clip.height <= 160.0,
            "preview clip escaped the browser content viewport: clip={clip:?}"
        );
        assert!(clip.x <= rect.x && clip.y <= rect.y);
        assert!(clip.x + clip.width > rect.x && clip.y + clip.height > rect.y);
    }
    assert!(
        image_items
            .iter()
            .any(|(clip, rect, _)| clip.y + clip.height < rect.y + rect.height)
    );

    let expected_raster = media::decode_image_thumbnail(PNG_FIXTURE, 128, 128).unwrap();
    let expected_video = media::decode_webm_vp8_thumbnail(WEBM_FIXTURE, 128, 128).unwrap();
    let expected_h264 = media::decode_mp4_h264_thumbnail(H264_MP4_FIXTURE, 128, 128).unwrap();
    let readback = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(
            |captured: bevy::ecs::observer::On<ReadbackComplete>,
             mut pixels: ResMut<CapturedPixels>| {
                pixels.0.insert(0, captured.data.clone());
            },
        )
        .id();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !app.world().resource::<CapturedPixels>().0.contains_key(&0)
    {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let pixels = app
        .world()
        .resource::<CapturedPixels>()
        .0
        .get(&0)
        .cloned()
        .expect("file preview GPU readback did not complete");
    let mut raster_pixel_found = false;
    let mut video_pixel_found = false;
    for (clip, rect, _) in &image_items {
        let visible_left = rect.x.max(clip.x);
        let visible_top = rect.y.max(clip.y);
        let visible_right = (rect.x + rect.width).min(clip.x + clip.width);
        let visible_bottom = (rect.y + rect.height).min(clip.y + clip.height);
        let x = ((visible_left + visible_right) * 0.5).floor() as usize;
        let y = ((visible_top + visible_bottom) * 0.5).floor() as usize;
        let pixel = pixel_at(&pixels, x, y);
        let raster_expected = thumbnail_pixel_at(&expected_raster, *rect, x, y);
        let video_expected = thumbnail_pixel_at(&expected_video, *rect, x, y);
        raster_pixel_found |= pixel
            .iter()
            .zip(raster_expected)
            .all(|(actual, expected)| actual.abs_diff(*expected) <= 4);
        video_pixel_found |= pixel
            .iter()
            .zip(video_expected)
            .all(|(actual, expected)| actual.abs_diff(*expected) <= 8);
    }
    assert!(raster_pixel_found, "raster thumbnail pixel was not visible");
    assert!(video_pixel_found, "video poster pixel was not visible");
    let (h264_clip, h264_rect) = image_items
        .iter()
        .map(|(clip, rect, _)| (*clip, *rect))
        .find(|(_, rect)| rect.width > rect.height * 1.25)
        .expect("H.264 poster should retain its 16:9 aspect ratio");
    let mut h264_match_count = 0usize;
    for y in (h264_rect.y.ceil() as usize)..((h264_rect.y + h264_rect.height).floor() as usize) {
        for x in (h264_rect.x.ceil() as usize)..((h264_rect.x + h264_rect.width).floor() as usize) {
            if (x as f32) < h264_clip.x
                || (y as f32) < h264_clip.y
                || (x as f32) >= h264_clip.x + h264_clip.width
                || (y as f32) >= h264_clip.y + h264_clip.height
            {
                continue;
            }
            let actual = pixel_at(&pixels, x, y);
            let expected = thumbnail_pixel_at(&expected_h264, h264_rect, x, y);
            if actual[..3]
                .iter()
                .zip(&expected[..3])
                .all(|(a, b)| a.abs_diff(*b) <= 12)
            {
                h264_match_count += 1;
            }
        }
    }
    assert!(
        h264_match_count > 50,
        "H.264 MP4 poster pixel was not visible; only {h264_match_count} pixels matched"
    );
    app.world_mut().entity_mut(readback).despawn();

    app.world_mut().entity_mut(browser).insert(UiVisible(false));
    for _ in 0..3 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        app.world()
            .resource::<DisplayList>()
            .overlay_camera_items(camera)
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .ready_count(),
        3
    );
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .completed_decodes(),
        3
    );

    app.world_mut().entity_mut(browser).insert(UiVisible(true));
    app.update();
    assert!(
        app.world()
            .resource::<DisplayList>()
            .overlay_camera_items(camera)
            .is_some()
    );
    assert_eq!(
        app.world()
            .resource::<FileBrowserPreviewCache>()
            .completed_decodes(),
        3
    );

    fs::remove_dir_all(directory).unwrap();
    println!("file-browser raster, VP8 WebM, and H.264 MP4 thumbnail GPU readback succeeded");
}

use super::*;
use crate::decode_gif;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::{App, AssetPlugin, ImagePlugin, MinimalPlugins};
use image::ImageEncoder;
use std::{
    fs,
    path::PathBuf,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_asset_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    // `temp_dir()` may spell the same macOS directory as `/var` while the
    // filesystem watcher reports `/private/var`; use one canonical root.
    fs::canonicalize(root).unwrap()
}

fn bundled_asset_root() -> PathBuf {
    fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")).unwrap()
}

const VP8_KEYFRAME_16X16: &[u8] = &[
    48, 1, 0, 157, 1, 42, 16, 0, 16, 0, 1, 64, 38, 37, 160, 0, 3, 112, 0, 254, 240, 83, 31, 255,
    226, 223, 180, 127, 123, 101, 7, 255, 253, 251, 75, 255, 240, 39, 127, 255, 2, 119, 253, 12, 0,
    0,
];

fn long_ivf_vp8_fixture(frame_count: usize) -> Vec<u8> {
    let mut ivf = Vec::with_capacity(32 + frame_count * (12 + VP8_KEYFRAME_16X16.len()));
    ivf.extend_from_slice(b"DKIF");
    ivf.extend_from_slice(&0u16.to_le_bytes());
    ivf.extend_from_slice(&32u16.to_le_bytes());
    ivf.extend_from_slice(b"VP80");
    ivf.extend_from_slice(&16u16.to_le_bytes());
    ivf.extend_from_slice(&16u16.to_le_bytes());
    ivf.extend_from_slice(&30u32.to_le_bytes());
    ivf.extend_from_slice(&1u32.to_le_bytes());
    ivf.extend_from_slice(
        &u32::try_from(frame_count)
            .expect("fixture frame count fits in IVF header")
            .to_le_bytes(),
    );
    ivf.extend_from_slice(&0u32.to_le_bytes());
    for timestamp in 0..frame_count {
        ivf.extend_from_slice(&(VP8_KEYFRAME_16X16.len() as u32).to_le_bytes());
        ivf.extend_from_slice(
            &u64::try_from(timestamp)
                .expect("fixture timestamp fits in IVF packet")
                .to_le_bytes(),
        );
        ivf.extend_from_slice(VP8_KEYFRAME_16X16);
    }
    ivf
}

fn long_gif_fixture(frame_count: usize) -> Vec<u8> {
    let frames = (0..frame_count).map(|index| {
        let color = image::Rgba([
            (index % 255) as u8,
            ((index * 17) % 255) as u8,
            ((index * 43) % 255) as u8,
            255,
        ]);
        image::Frame::from_parts(
            image::RgbaImage::from_pixel(2, 2, color),
            0,
            0,
            image::Delay::from_numer_denom_ms(10, 1),
        )
    });
    let mut bytes = Vec::new();
    image::codecs::gif::GifEncoder::new(&mut bytes)
        .encode_frames(frames)
        .expect("long GIF fixture encodes");
    bytes
}

fn assert_source_stream_is_bounded(
    kind: MediaSequenceKind,
    source: Vec<u8>,
    frame_count: usize,
    frame_bytes: usize,
) {
    let max_cache_bytes = frame_bytes * MAX_STREAMED_VIDEO_CACHE_FRAMES;
    let mut app = App::new();
    app.insert_resource(BuiMediaLimits {
        max_sequence_bytes: max_cache_bytes,
        max_global_decoded_bytes: max_cache_bytes,
        ..Default::default()
    });
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        BuiMediaPlugin,
    ));
    let sequence = app
        .world_mut()
        .resource_mut::<Assets<BuiMediaSequence>>()
        .add(BuiMediaSequence::from_encoded_bytes(source, kind));
    let player = app
        .world_mut()
        .spawn((
            BuiMediaPlayer {
                playing: false,
                looping: false,
                ..BuiMediaPlayer::new(sequence)
            },
            ImageNode::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            bevy::camera::visibility::ViewVisibility::VISIBLE,
        ))
        .id();

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        if app
            .world()
            .resource::<BuiMediaFrameCache>()
            .video_streams
            .get(&player)
            .is_some_and(|stream| !stream.frames.is_empty())
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{kind:?} stream did not produce its first frame"
        );
        thread::sleep(Duration::from_millis(1));
    }
    app.world_mut()
        .get_mut::<BuiMediaPlayer>(player)
        .unwrap()
        .seek_to(Duration::from_secs(30));

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let stream = app
            .world()
            .resource::<BuiMediaFrameCache>()
            .video_streams
            .get(&player)
            .unwrap();
        if stream.eof && stream.next_index == frame_count {
            assert!(stream.frames.len() <= MAX_STREAMED_VIDEO_CACHE_FRAMES);
            assert!(stream.cached_bytes <= max_cache_bytes);
            assert_eq!(
                app.world()
                    .get::<BuiMediaPlayer>(player)
                    .unwrap()
                    .current_frame,
                frame_count - 1
            );
            break;
        }
        assert!(
            !matches!(
                app.world().get::<BuiMediaPlayer>(player).unwrap().state,
                BuiMediaPlayerState::Failed(_)
            ),
            "{kind:?} stream failed before frame {frame_count}: {:?}",
            app.world().get::<BuiMediaPlayer>(player).unwrap().state
        );
        assert!(
            std::time::Instant::now() < deadline,
            "{kind:?} stream did not finish: decoded={}, cached={}",
            stream.next_index,
            stream.frames.len()
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert!(app.world().resource::<BuiMediaDecodedBudget>().used_bytes() <= max_cache_bytes);
    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
    app.update();
    assert!(
        !app.world()
            .resource::<BuiMediaFrameCache>()
            .video_streams
            .contains_key(&player)
    );
    wait_for_decoded_budget(&mut app, 0);
}

fn wait_for_decoded_budget(app: &mut App, expected_bytes: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.world().resource::<BuiMediaDecodedBudget>().used_bytes() != expected_bytes {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "decoded budget did not settle at {expected_bytes} bytes; currently {}",
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes()
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[path = "tests/asset_loading_and_playback.rs"]
mod asset_loading_and_playback;
#[path = "tests/decoded_budget.rs"]
mod decoded_budget;
#[path = "tests/streaming_and_lifetime.rs"]
mod streaming_and_lifetime;

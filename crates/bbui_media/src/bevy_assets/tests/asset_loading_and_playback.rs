use super::*;

#[test]
pub(super) fn frame_conversion_creates_gpu_uploadable_bevy_image() {
    let image = image_from_frame(RgbaFrame::new(1, 1, vec![10, 20, 30, 255]).unwrap()).unwrap();
    assert_eq!(image.width(), 1);
    assert_eq!(image.height(), 1);
    assert_eq!(image.data.as_deref(), Some(&[10, 20, 30, 255][..]));
}

#[test]
pub(super) fn bundled_loop_gif_is_a_valid_two_frame_animation() {
    let frames = decode_gif(include_bytes!("../../../assets/loop.gif")).unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].frame.width, 2);
    assert_eq!(frames[0].frame.height, 1);
}

#[test]
pub(super) fn plugin_registers_custom_media_assets_and_loaders() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
    ));
    app.add_plugins(BbuiMediaPlugin);
    assert!(app.world().contains_resource::<Assets<BbuiMediaSequence>>());
}

#[test]
pub(super) fn svg_and_raster_assets_report_load_errors_and_enforce_configured_limits() {
    let root = temp_asset_root("bbui-still-image-loader");

    let svg = |width: u32, height: u32| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\"><rect width=\"100%\" height=\"100%\" fill=\"#336699\"/></svg>"
        )
    };
    fs::write(root.join("small.svg"), svg(1, 1)).unwrap();
    fs::write(root.join("oversized.svg"), svg(2, 2)).unwrap();
    fs::write(root.join("broken.svg"), "<svg><broken").unwrap();
    let mut encoded_over_limit = svg(1, 1).into_bytes();
    encoded_over_limit.resize(513, b' ');
    fs::write(root.join("encoded-over-limit.svg"), encoded_over_limit).unwrap();

    let encode_png = |width: u32, height: u32| {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &vec![90; (width * height * 4) as usize],
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    };
    fs::write(root.join("small.bbuiimg"), encode_png(1, 1)).unwrap();
    fs::write(root.join("oversized.bbuiimg"), encode_png(2, 2)).unwrap();

    let mut app = App::new();
    app.insert_resource(BbuiMediaLimits {
        max_encoded_bytes: 512,
        max_sequence_bytes: 4,
        ..Default::default()
    });
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        ImagePlugin::default(),
        BbuiMediaPlugin,
    ));
    app.finish();
    app.cleanup();

    let (small_svg, small_raster, oversized_svg, oversized_raster, broken_svg, encoded_limit) = {
        let server = app.world().resource::<AssetServer>();
        (
            server.load::<Image>("small.svg"),
            server.load::<Image>("small.bbuiimg"),
            server.load::<Image>("oversized.svg"),
            server.load::<Image>("oversized.bbuiimg"),
            server.load::<Image>("broken.svg"),
            server.load::<Image>("encoded-over-limit.svg"),
        )
    };
    let ids = [
        small_svg.id(),
        small_raster.id(),
        oversized_svg.id(),
        oversized_raster.id(),
        broken_svg.id(),
        encoded_limit.id(),
    ];
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let states = ids.map(|id| app.world().resource::<AssetServer>().get_load_state(id));
        if states.iter().all(|state| {
            state.as_ref().is_some_and(|state| {
                matches!(
                    state,
                    bevy::asset::LoadState::Loaded | bevy::asset::LoadState::Failed(_)
                )
            })
        }) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "bounded SVG/raster assets did not finish loading"
        );
        thread::sleep(Duration::from_millis(2));
    }

    for image in [&small_svg, &small_raster] {
        let image = app
            .world()
            .resource::<Assets<Image>>()
            .get(image)
            .expect("small image should load within the configured byte limit");
        assert_eq!((image.width(), image.height()), (1, 1));
    }
    let asset_server = app.world().resource::<AssetServer>();
    for image in [&oversized_svg, &oversized_raster] {
        let Some(bevy::asset::LoadState::Failed(error)) = asset_server.get_load_state(image.id())
        else {
            panic!("oversized decoded image should fail to load");
        };
        assert!(
            error
                .to_string()
                .contains("decoded media exceeds the configured byte limit"),
            "unexpected decoded-image error: {error}"
        );
    }
    for (image, expected) in [
        (&broken_svg, "SVG decode failed"),
        (
            &encoded_limit,
            "encoded asset exceeds the configured byte limit",
        ),
    ] {
        let Some(bevy::asset::LoadState::Failed(error)) = asset_server.get_load_state(image.id())
        else {
            panic!("invalid/oversized source should fail to load");
        };
        assert!(
            error.to_string().contains(expected),
            "unexpected load error: {error}"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn h264_mp4_mov_assets_play_frames_pause_offscreen_and_release_cache() {
    let root = temp_asset_root("bbui-h264-sequence");
    let mp4_path = root.join("sample.mp4");
    fs::write(&mp4_path, include_bytes!("../../../assets/sample-h264.mp4")).unwrap();
    let mut mov_bytes = include_bytes!("../../../assets/sample-h264.mp4").to_vec();
    mov_bytes[8..12].copy_from_slice(b"qt  ");
    fs::write(root.join("sample.mov"), mov_bytes).unwrap();

    let mut app = App::new();
    app.insert_resource(BbuiMediaLimits {
        ..Default::default()
    });
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        ImagePlugin::default(),
        BbuiMediaPlugin,
    ));
    app.finish();
    app.cleanup();

    let (mp4, mov) = {
        let server = app.world().resource::<AssetServer>();
        (
            server.load::<BbuiMediaSequence>("sample.mp4"),
            server.load::<BbuiMediaSequence>("sample.mov"),
        )
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !app
        .world()
        .resource::<AssetServer>()
        .is_loaded_with_dependencies(mp4.id())
        || !app
            .world()
            .resource::<AssetServer>()
            .is_loaded_with_dependencies(mov.id())
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "MP4/MOV sequence assets did not finish loading"
        );
        thread::sleep(Duration::from_millis(5));
    }
    for sequence in [&mp4, &mov] {
        let sequence_asset = app
            .world()
            .resource::<Assets<BbuiMediaSequence>>()
            .get(sequence)
            .unwrap();
        assert_eq!(sequence_asset.kind, MediaSequenceKind::Mp4H264);
        assert!(sequence_asset.frames.is_empty());
        assert!(sequence_asset.encoded_source.is_some());
    }

    let sequence_id = mp4.id();
    let baseline_image_count = app.world().resource::<Assets<Image>>().len();
    let player = app
        .world_mut()
        .spawn((
            BbuiMediaPlayer {
                playing: false,
                ..BbuiMediaPlayer::new(mp4.clone())
            },
            ImageNode::default(),
            Visibility::Hidden,
            InheritedVisibility::HIDDEN,
            bevy::camera::visibility::ViewVisibility::HIDDEN,
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<BbuiMediaPlayer>(player).unwrap().state,
        BbuiMediaPlayerState::Hidden
    );
    assert_eq!(
        app.world()
            .resource::<BbuiMediaFrameCache>()
            .pending_decode_count(),
        0
    );
    assert_eq!(
        app.world().resource::<Assets<Image>>().len(),
        baseline_image_count
    );

    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(player)
        .unwrap() = InheritedVisibility::VISIBLE;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let state = app
            .world()
            .get::<BbuiMediaPlayer>(player)
            .unwrap()
            .state
            .clone();
        if state == BbuiMediaPlayerState::Ready {
            break;
        }
        assert!(
            !matches!(state, BbuiMediaPlayerState::Failed(_)),
            "visible MP4 failed to decode: {state:?}"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "visible MP4 did not decode: {state:?}"
        );
        thread::sleep(Duration::from_millis(2));
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let stream = app
            .world()
            .resource::<BbuiMediaFrameCache>()
            .video_streams
            .get(&player);
        if stream.is_some_and(|stream| stream.eof && stream.frames.len() == 2) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "two-frame MP4 stream did not reach EOF"
        );
        thread::sleep(Duration::from_millis(2));
    }

    let (frame_ids, second_timestamp) = {
        let cache = app.world().resource::<BbuiMediaFrameCache>();
        let stream = &cache.video_streams[&player];
        assert_eq!(stream.sequence_id, sequence_id);
        assert_eq!(stream.frames.len(), 2);
        (
            stream
                .frames
                .iter()
                .map(|frame| frame.image.id())
                .collect::<Vec<_>>(),
            stream.frames[1].timestamp,
        )
    };
    assert_eq!(
        app.world()
            .resource::<BbuiMediaDecodedBudget>()
            .used_bytes(),
        2 * 32 * 18 * 4
    );
    assert_eq!(
        app.world().get::<ImageNode>(player).unwrap().image.id(),
        frame_ids[0]
    );
    {
        let mut playback = app.world_mut().get_mut::<BbuiMediaPlayer>(player).unwrap();
        playback.seek_to(second_timestamp + Duration::from_nanos(1));
    }
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    assert_eq!(
        app.world()
            .get::<BbuiMediaPlayer>(player)
            .unwrap()
            .current_frame,
        1
    );
    assert_eq!(
        app.world().get::<ImageNode>(player).unwrap().image.id(),
        frame_ids[1]
    );

    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(player)
        .unwrap() = InheritedVisibility::HIDDEN;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::HIDDEN;
    app.update();
    wait_for_decoded_budget(&mut app, 0);
    assert_eq!(
        app.world().get::<BbuiMediaPlayer>(player).unwrap().state,
        BbuiMediaPlayerState::Hidden
    );
    assert_eq!(
        app.world().get::<ImageNode>(player).unwrap().image,
        ImageNode::default().image
    );
    assert_eq!(
        app.world()
            .resource::<BbuiMediaDecodedBudget>()
            .used_bytes(),
        0
    );
    assert!(
        app.world()
            .resource::<Assets<BbuiMediaSequence>>()
            .get(sequence_id)
            .is_some(),
        "the encoded sequence must remain AssetServer-owned while handles are strong"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while frame_ids
        .iter()
        .any(|id| app.world().resource::<Assets<Image>>().get(*id).is_some())
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "hidden H.264 cache retained Bevy Image assets"
        );
        thread::sleep(Duration::from_millis(2));
    }

    app.world_mut()
        .get_mut::<BbuiMediaPlayer>(player)
        .unwrap()
        .sequence = mov.clone();
    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(player)
        .unwrap() = InheritedVisibility::VISIBLE;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let state = app
            .world()
            .get::<BbuiMediaPlayer>(player)
            .unwrap()
            .state
            .clone();
        if state == BbuiMediaPlayerState::Ready {
            break;
        }
        assert!(
            !matches!(state, BbuiMediaPlayerState::Failed(_)),
            "visible MOV failed to decode: {state:?}"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "visible MOV did not decode: {state:?}"
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        app.world().get::<BbuiMediaPlayer>(player).unwrap().state,
        BbuiMediaPlayerState::Ready
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn h264_asset_streams_long_video_with_bounded_cache_seek_loop_and_cancel() {
    let root = temp_asset_root("bbui-h264-long-stream");
    fs::write(
        root.join("long.mp4"),
        include_bytes!("../../../assets/sample-h264-long.mp4"),
    )
    .unwrap();

    let mut app = App::new();
    app.insert_resource(BbuiMediaLimits {
        ..Default::default()
    });
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(20),
    ));
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        ImagePlugin::default(),
        BbuiMediaPlugin,
    ));
    app.finish();
    app.cleanup();

    let sequence = app
        .world()
        .resource::<AssetServer>()
        .load::<BbuiMediaSequence>("long.mp4");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !app
        .world()
        .resource::<AssetServer>()
        .is_loaded_with_dependencies(sequence.id())
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "long H.264 sequence asset did not load"
        );
        thread::sleep(Duration::from_millis(2));
    }

    let player = app
        .world_mut()
        .spawn((
            BbuiMediaPlayer {
                playing: false,
                looping: false,
                ..BbuiMediaPlayer::new(sequence.clone())
            },
            ImageNode::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            bevy::camera::visibility::ViewVisibility::VISIBLE,
        ))
        .id();

    app.update();
    let ready_deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let state = &app.world().get::<BbuiMediaPlayer>(player).unwrap().state;
        if *state == BbuiMediaPlayerState::Ready {
            break;
        }
        assert!(
            !matches!(state, BbuiMediaPlayerState::Failed(_)),
            "long H.264 stream failed before its first frame: {state:?}"
        );
        assert!(
            std::time::Instant::now() < ready_deadline,
            "long H.264 stream did not produce its first frame"
        );
        thread::sleep(Duration::from_millis(1));
    }

    // Seeking past the end makes the player keep consuming the stream as frames arrive. The
    // decoder must reach all 601 frames without collecting them into one sequence allocation.
    app.world_mut()
        .get_mut::<BbuiMediaPlayer>(player)
        .unwrap()
        .seek_to(Duration::from_secs(30));
    let decode_deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let cache = app.world().resource::<BbuiMediaFrameCache>();
        let stream = cache.video_streams.get(&player).unwrap();
        if stream.eof && stream.next_index == 601 {
            assert_eq!(
                app.world()
                    .get::<BbuiMediaPlayer>(player)
                    .unwrap()
                    .current_frame,
                600
            );
            assert!(stream.frames.len() <= MAX_STREAMED_VIDEO_CACHE_FRAMES);
            assert!(stream.cached_bytes <= MAX_STREAMED_VIDEO_CACHE_FRAMES * 32 * 18 * 4);
            assert!(cache.cached_image_count() <= MAX_STREAMED_VIDEO_CACHE_FRAMES);
            assert!(
                app.world()
                    .resource::<BbuiMediaDecodedBudget>()
                    .used_bytes()
                    <= MAX_STREAMED_VIDEO_CACHE_FRAMES * 32 * 18 * 4
            );
            break;
        }
        assert!(
            !matches!(
                app.world().get::<BbuiMediaPlayer>(player).unwrap().state,
                BbuiMediaPlayerState::Failed(_)
            ),
            "long H.264 stream failed before EOF"
        );
        assert!(
            std::time::Instant::now() < decode_deadline,
            "long H.264 stream did not decode all frames; next={}, cached={}",
            stream.next_index,
            stream.frames.len()
        );
        thread::sleep(Duration::from_millis(1));
    }

    let duration = app.world().resource::<BbuiMediaFrameCache>().video_streams[&player]
        .duration
        .unwrap();
    {
        let mut playback = app.world_mut().get_mut::<BbuiMediaPlayer>(player).unwrap();
        playback.looping = true;
        playback.playing = true;
        playback.seek_to(duration.saturating_sub(Duration::from_millis(10)));
    }
    app.update();
    assert_eq!(
        app.world().get::<BbuiMediaPlayer>(player).unwrap().position,
        Duration::from_millis(10),
        "looping preserves the wrapped timeline position"
    );
    assert!(
        !app.world()
            .resource::<BbuiMediaFrameCache>()
            .video_streams
            .contains_key(&player),
        "looping must restart the source-backed stream"
    );

    // The loop restart begins at the source start. Advance into the restarted stream, then
    // seek backward and verify that the decoder is cancelled/recreated at frame zero.
    app.world_mut()
        .get_mut::<BbuiMediaPlayer>(player)
        .unwrap()
        .playing = false;
    let restart_deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let state = &app.world().get::<BbuiMediaPlayer>(player).unwrap().state;
        if *state == BbuiMediaPlayerState::Ready {
            break;
        }
        assert!(
            !matches!(state, BbuiMediaPlayerState::Failed(_)),
            "loop restart failed: {state:?}"
        );
        assert!(std::time::Instant::now() < restart_deadline);
        thread::sleep(Duration::from_millis(1));
    }
    app.world_mut()
        .get_mut::<BbuiMediaPlayer>(player)
        .unwrap()
        .seek_to(Duration::from_secs(1));
    let forward_deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        if app
            .world()
            .get::<BbuiMediaPlayer>(player)
            .unwrap()
            .current_frame
            > 0
        {
            break;
        }
        assert!(std::time::Instant::now() < forward_deadline);
        thread::sleep(Duration::from_millis(1));
    }
    app.world_mut()
        .get_mut::<BbuiMediaPlayer>(player)
        .unwrap()
        .seek_to(Duration::ZERO);
    app.update();
    assert!(
        !app.world()
            .resource::<BbuiMediaFrameCache>()
            .video_streams
            .contains_key(&player),
        "backward seek outside the rolling cache must restart decoding"
    );
    let backward_deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let playback = app.world().get::<BbuiMediaPlayer>(player).unwrap();
        if playback.state == BbuiMediaPlayerState::Ready && playback.current_frame == 0 {
            break;
        }
        assert!(
            !matches!(playback.state, BbuiMediaPlayerState::Failed(_)),
            "backward seek restart failed: {:?}",
            playback.state
        );
        assert!(std::time::Instant::now() < backward_deadline);
        thread::sleep(Duration::from_millis(1));
    }

    // Hiding cancels the active stream and drops its Bevy images while preserving the strong
    // sequence asset. Revealing starts a fresh stream at the paused timeline position.
    {
        let mut playback = app.world_mut().get_mut::<BbuiMediaPlayer>(player).unwrap();
        playback.playing = false;
        playback.seek_to(Duration::from_millis(500));
    }
    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(player)
        .unwrap() = InheritedVisibility::HIDDEN;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::HIDDEN;
    app.update();
    wait_for_decoded_budget(&mut app, 0);
    assert_eq!(
        app.world().get::<BbuiMediaPlayer>(player).unwrap().state,
        BbuiMediaPlayerState::Hidden
    );
    assert_eq!(
        app.world()
            .resource::<BbuiMediaFrameCache>()
            .cached_image_count(),
        0
    );
    assert_eq!(
        app.world()
            .resource::<BbuiMediaDecodedBudget>()
            .used_bytes(),
        0
    );
    assert!(
        app.world()
            .resource::<BbuiMediaFrameCache>()
            .cancelled_job_count()
            > 0
    );
    assert!(
        app.world()
            .resource::<Assets<BbuiMediaSequence>>()
            .get(&sequence)
            .is_some()
    );

    let hidden_position = app.world().get::<BbuiMediaPlayer>(player).unwrap().position;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world().get::<BbuiMediaPlayer>(player).unwrap().position,
        hidden_position,
        "hidden playback does not advance"
    );
    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Visible;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(player)
        .unwrap() = InheritedVisibility::VISIBLE;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
    let reveal_deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let playback = app.world().get::<BbuiMediaPlayer>(player).unwrap();
        if playback.state == BbuiMediaPlayerState::Ready {
            assert_eq!(playback.position, hidden_position);
            break;
        }
        assert!(
            !matches!(playback.state, BbuiMediaPlayerState::Failed(_)),
            "revealed H.264 stream failed: {:?}",
            playback.state
        );
        assert!(std::time::Instant::now() < reveal_deadline);
        thread::sleep(Duration::from_millis(1));
    }

    fs::remove_dir_all(root).unwrap();
}

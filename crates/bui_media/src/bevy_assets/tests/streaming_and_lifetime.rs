use super::*;

#[test]
pub(super) fn hidden_encoded_player_does_not_decode_and_releases_images_while_handle_stays_strong()
{
    let mut app = App::new();
    app.insert_resource(BuiMediaLimits {
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
        .add(BuiMediaSequence::from_encoded_bytes(
            include_bytes!("../../../assets/loop.gif").to_vec(),
            MediaSequenceKind::Gif,
        ));
    let sequence_id = sequence.id();
    let baseline_image_count = app.world().resource::<Assets<Image>>().len();
    let player = app
        .world_mut()
        .spawn((
            BuiMediaPlayer::new(sequence.clone()),
            ImageNode::default(),
            Visibility::Hidden,
            InheritedVisibility::HIDDEN,
            bevy::camera::visibility::ViewVisibility::HIDDEN,
        ))
        .id();

    app.update();
    assert_eq!(
        app.world().get::<BuiMediaPlayer>(player).unwrap().state,
        BuiMediaPlayerState::Hidden
    );
    assert_eq!(
        app.world()
            .resource::<BuiMediaFrameCache>()
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
    while !app
        .world()
        .resource::<BuiMediaFrameCache>()
        .video_streams
        .get(&player)
        .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "visible GIF did not decode"
        );
        thread::sleep(Duration::from_millis(2));
    }
    let frame_ids = app.world().resource::<BuiMediaFrameCache>().video_streams[&player]
        .frames
        .iter()
        .map(|frame| frame.image.id())
        .collect::<Vec<_>>();
    assert_eq!(frame_ids.len(), 2);
    let external_image_handle = app.world().get::<ImageNode>(player).unwrap().image.clone();
    let externally_held_bytes = app.world().resource::<BuiMediaFrameCache>().video_streams[&player]
        .frames
        .iter()
        .find(|frame| frame.image.id() == external_image_handle.id())
        .unwrap()
        .byte_len;
    assert!(app.world().resource::<BuiMediaDecodedBudget>().used_bytes() > 0);

    *app.world_mut().get_mut::<Visibility>(player).unwrap() = Visibility::Hidden;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(player)
        .unwrap() = InheritedVisibility::HIDDEN;
    app.update();
    assert_eq!(
        app.world().get::<BuiMediaPlayer>(player).unwrap().state,
        BuiMediaPlayerState::Hidden
    );
    assert_eq!(
        app.world().get::<ImageNode>(player).unwrap().image,
        ImageNode::default().image
    );
    wait_for_decoded_budget(&mut app, externally_held_bytes);
    assert!(
        app.world()
            .resource::<Assets<BuiMediaSequence>>()
            .get(sequence_id)
            .is_some()
    );
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(external_image_handle.id())
            .is_some()
    );
    drop(external_image_handle);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while frame_ids
        .iter()
        .any(|id| app.world().resource::<Assets<Image>>().get(*id).is_some())
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "Bevy did not release hidden cache-owned frame Images"
        );
        thread::sleep(Duration::from_millis(2));
    }
    wait_for_decoded_budget(&mut app, 0);
}

#[test]
pub(super) fn hidden_video_releases_cached_frames_before_the_next_visible_sequence_decodes() {
    let mut app = App::new();
    app.insert_resource(BuiMediaLimits {
        max_global_decoded_bytes: 16,
        ..Default::default()
    });
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        BuiMediaPlugin,
    ));
    let make_sequence = || {
        BuiMediaSequence::from_encoded_bytes(
            include_bytes!("../../../assets/loop.gif").to_vec(),
            MediaSequenceKind::Gif,
        )
    };
    let (first, second) = {
        let mut sequences = app.world_mut().resource_mut::<Assets<BuiMediaSequence>>();
        (
            sequences.add(make_sequence()),
            sequences.add(make_sequence()),
        )
    };
    let first_id = first.id();
    let first_player = app
        .world_mut()
        .spawn((
            BuiMediaPlayer {
                playing: false,
                ..BuiMediaPlayer::new(first.clone())
            },
            ImageNode::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            bevy::camera::visibility::ViewVisibility::VISIBLE,
        ))
        .id();
    let second_player = app
        .world_mut()
        .spawn((
            BuiMediaPlayer {
                playing: false,
                ..BuiMediaPlayer::new(second)
            },
            ImageNode::default(),
            Visibility::Hidden,
            InheritedVisibility::HIDDEN,
            bevy::camera::visibility::ViewVisibility::HIDDEN,
        ))
        .id();

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !app
        .world()
        .resource::<BuiMediaFrameCache>()
        .video_streams
        .get(&first_player)
        .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "first GIF did not decode: state={:?}, cache={}, budget={}",
            app.world()
                .get::<BuiMediaPlayer>(first_player)
                .unwrap()
                .state,
            app.world()
                .resource::<BuiMediaFrameCache>()
                .sequence_count(),
            app.world().resource::<BuiMediaDecodedBudget>().used_bytes()
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
        16
    );

    *app.world_mut().get_mut::<Visibility>(first_player).unwrap() = Visibility::Hidden;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(first_player)
        .unwrap() = InheritedVisibility::HIDDEN;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(first_player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::HIDDEN;
    wait_for_decoded_budget(&mut app, 0);
    *app.world_mut()
        .get_mut::<Visibility>(second_player)
        .unwrap() = Visibility::Visible;
    *app.world_mut()
        .get_mut::<InheritedVisibility>(second_player)
        .unwrap() = InheritedVisibility::VISIBLE;
    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(second_player)
        .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;

    while !app
        .world()
        .resource::<BuiMediaFrameCache>()
        .video_streams
        .get(&second_player)
        .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "second GIF did not decode"
        );
        thread::sleep(Duration::from_millis(2));
    }
    let cache = app.world().resource::<BuiMediaFrameCache>();
    assert_eq!(cache.cached_image_count(), 2);
    assert_eq!(cache.sequence_count(), 1);
    assert_eq!(
        app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
        16
    );
    assert!(
        app.world()
            .resource::<Assets<BuiMediaSequence>>()
            .get(first_id)
            .is_some()
    );
    assert_eq!(
        app.world()
            .get::<BuiMediaPlayer>(first_player)
            .unwrap()
            .state,
        BuiMediaPlayerState::Hidden
    );
}

#[test]
pub(super) fn gif_decode_honors_configured_frame_and_byte_limits() {
    let cancelled = AtomicBool::new(false);
    let source = include_bytes!("../../../assets/loop.gif");
    let mut discard = |_| true;
    let frame_limited =
        decode_gif_stream_cancellable(source, &cancelled, 1, usize::MAX, &mut discard).unwrap_err();
    assert!(matches!(frame_limited, MediaDecodeError::FrameLimit));

    let byte_limited =
        decode_gif_stream_cancellable(source, &cancelled, usize::MAX, 8, &mut discard).unwrap_err();
    assert!(matches!(byte_limited, MediaDecodeError::SequenceLimit));
}

#[test]
pub(super) fn hidden_player_keeps_its_clock_still_and_shows_current_frame() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
    ));
    app.add_plugins(BuiMediaPlugin);
    let (first, second) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    let sequence = app
        .world_mut()
        .resource_mut::<Assets<BuiMediaSequence>>()
        .add(BuiMediaSequence::new(
            vec![first.clone(), second],
            vec![Duration::from_millis(100); 2],
            MediaSequenceKind::Gif,
        ));
    let entity = app
        .world_mut()
        .spawn((
            BuiMediaPlayer::new(sequence),
            ImageNode::default(),
            Visibility::Hidden,
            InheritedVisibility::HIDDEN,
        ))
        .id();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(150));
    app.update();
    let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
    let image = app.world().get::<ImageNode>(entity).unwrap();
    assert_eq!(player.position, Duration::ZERO);
    assert_eq!(player.current_frame, 0);
    assert_eq!(image.image, first);
}

#[test]
pub(super) fn player_pauses_for_view_visibility_and_manual_pause_then_resumes() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
    ));
    app.add_plugins(BuiMediaPlugin);
    let (first, second) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    let sequence = app
        .world_mut()
        .resource_mut::<Assets<BuiMediaSequence>>()
        .add(BuiMediaSequence::new(
            vec![first.clone(), second.clone()],
            vec![Duration::from_millis(100); 2],
            MediaSequenceKind::Gif,
        ));
    let entity = app
        .world_mut()
        .spawn((
            BuiMediaPlayer::new(sequence),
            ImageNode::default(),
            Visibility::Inherited,
            InheritedVisibility::VISIBLE,
            bevy::camera::visibility::ViewVisibility::HIDDEN,
        ))
        .id();

    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(150));
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    {
        let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert_eq!(player.position, Duration::ZERO);
        assert_eq!(player.current_frame, 0);
        assert_eq!(image.image, first);
    }

    *app.world_mut()
        .get_mut::<bevy::camera::visibility::ViewVisibility>(entity)
        .unwrap() = bevy::camera::visibility::ViewVisibility::VISIBLE;
    app.world_mut()
        .get_mut::<BuiMediaPlayer>(entity)
        .unwrap()
        .playing = false;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(150));
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    assert_eq!(
        app.world().get::<BuiMediaPlayer>(entity).unwrap().position,
        Duration::ZERO
    );

    app.world_mut()
        .get_mut::<BuiMediaPlayer>(entity)
        .unwrap()
        .playing = true;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(150));
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    {
        let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
        let image = app.world().get::<ImageNode>(entity).unwrap();
        assert_eq!(player.position, Duration::from_millis(150));
        assert_eq!(player.current_frame, 1);
        assert_eq!(image.image, second);
    }

    *app.world_mut()
        .get_mut::<InheritedVisibility>(entity)
        .unwrap() = InheritedVisibility::HIDDEN;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(100));
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    assert_eq!(
        app.world().get::<BuiMediaPlayer>(entity).unwrap().position,
        Duration::from_millis(150)
    );

    *app.world_mut()
        .get_mut::<InheritedVisibility>(entity)
        .unwrap() = InheritedVisibility::VISIBLE;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(100));
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
    let image = app.world().get::<ImageNode>(entity).unwrap();
    assert_eq!(player.position, Duration::from_millis(50));
    assert_eq!(player.current_frame, 0);
    assert_eq!(image.image, first);
}

#[test]
pub(super) fn sequence_timeline_clamps_zero_delays_and_selects_frame() {
    let sequence = BuiMediaSequence::new(
        vec![Handle::default(), Handle::default()],
        vec![Duration::ZERO, Duration::from_millis(20)],
        MediaSequenceKind::Gif,
    );
    assert_eq!(sequence.duration(), Duration::from_millis(30));
    assert_eq!(sequence.frame_at(Duration::from_millis(5)), 0);
    assert_eq!(sequence.frame_at(Duration::from_millis(15)), 1);

    let missing_durations = BuiMediaSequence::new(
        vec![Handle::default(), Handle::default()],
        Vec::new(),
        MediaSequenceKind::Gif,
    );
    assert_eq!(
        missing_durations.duration(),
        DEFAULT_VIDEO_FRAME_DURATION.saturating_mul(2)
    );
    assert_eq!(missing_durations.frame_at(Duration::from_millis(20)), 0);
    assert_eq!(missing_durations.frame_at(Duration::from_millis(40)), 1);

    let partial_durations = BuiMediaSequence::new(
        vec![Handle::default(), Handle::default()],
        vec![Duration::ZERO],
        MediaSequenceKind::Gif,
    );
    assert_eq!(
        partial_durations.duration(),
        MIN_FRAME_DURATION.saturating_add(DEFAULT_VIDEO_FRAME_DURATION)
    );
    assert_eq!(partial_durations.frame_at(Duration::from_millis(15)), 1);
}

#[test]
pub(super) fn player_with_no_frame_durations_advances_without_zero_modulo() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
    ));
    app.add_plugins(BuiMediaPlugin);
    let (first, second) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    let sequence = app
        .world_mut()
        .resource_mut::<Assets<BuiMediaSequence>>()
        .add(BuiMediaSequence::new(
            vec![first, second.clone()],
            Vec::new(),
            MediaSequenceKind::Gif,
        ));
    let entity = app
        .world_mut()
        .spawn((
            BuiMediaPlayer::new(sequence),
            ImageNode::default(),
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            bevy::camera::visibility::ViewVisibility::VISIBLE,
        ))
        .id();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(40));
    app.world_mut()
        .run_system_once(update_media_players)
        .unwrap();
    let player = app.world().get::<BuiMediaPlayer>(entity).unwrap();
    let image = app.world().get::<ImageNode>(entity).unwrap();
    assert_eq!(player.position, Duration::from_millis(40));
    assert_eq!(player.current_frame, 1);
    assert_eq!(image.image, second);
}

#[test]
pub(super) fn asset_server_releases_sequence_frames_and_budget_after_last_reference() {
    let mut app = App::new();
    app.insert_resource(BuiMediaLimits {
        ..Default::default()
    });
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: bundled_asset_root().to_string_lossy().into_owned(),
            ..Default::default()
        },
        ImagePlugin::default(),
    ));
    app.add_plugins(BuiMediaPlugin);
    app.finish();
    app.cleanup();

    let sequence: Handle<BuiMediaSequence> = app.world().resource::<AssetServer>().load("loop.gif");
    let sequence_id = sequence.id();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let sequence_asset = app
            .world()
            .resource::<Assets<BuiMediaSequence>>()
            .get(&sequence);
        if let Some(sequence_asset) = sequence_asset
            && app
                .world()
                .resource::<AssetServer>()
                .is_loaded_with_dependencies(sequence_id)
        {
            assert!(sequence_asset.frames.is_empty());
            assert!(sequence_asset.encoded_source.is_some());
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "GIF sequence and frame dependencies did not load before deadline"
        );
        thread::sleep(Duration::from_millis(5));
    }
    wait_for_decoded_budget(&mut app, 0);

    let player = app
        .world_mut()
        .spawn((
            BuiMediaPlayer::new(sequence.clone()),
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
            .is_some_and(|stream| stream.eof && stream.frames.len() == 2)
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "visible GIF did not decode"
        );
        thread::sleep(Duration::from_millis(2));
    }
    let frame_ids = app.world().resource::<BuiMediaFrameCache>().video_streams[&player]
        .frames
        .iter()
        .map(|frame| frame.image.id())
        .collect::<Vec<_>>();
    assert_eq!(frame_ids.len(), 2);
    assert!(
        frame_ids
            .iter()
            .all(|id| app.world().resource::<Assets<Image>>().get(*id).is_some())
    );
    assert!(app.world().resource::<BuiMediaDecodedBudget>().used_bytes() > 0);

    app.world_mut().entity_mut(player).despawn();
    drop(sequence);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let sequence_gone = app
            .world()
            .resource::<Assets<BuiMediaSequence>>()
            .get(sequence_id)
            .is_none();
        let frames_gone = frame_ids
            .iter()
            .all(|id| app.world().resource::<Assets<Image>>().get(*id).is_none());
        let budget_released = app.world().resource::<BuiMediaDecodedBudget>().used_bytes() == 0;
        if sequence_gone && frames_gone && budget_released {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "dropped media sequence retained sequence={sequence_gone}, frames={frames_gone}, budget={budget_released}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
pub(super) fn gif_player_streams_beyond_eager_limit_with_bounded_cache() {
    const FRAME_COUNT: usize = 601;
    assert_source_stream_is_bounded(
        MediaSequenceKind::Gif,
        long_gif_fixture(FRAME_COUNT),
        FRAME_COUNT,
        2 * 2 * 4,
    );
}

#[test]
pub(super) fn ivf_vp8_player_streams_beyond_eager_limit_with_bounded_cache() {
    const FRAME_COUNT: usize = 601;
    assert_source_stream_is_bounded(
        MediaSequenceKind::IvfVp8,
        long_ivf_vp8_fixture(FRAME_COUNT),
        FRAME_COUNT,
        16 * 16 * 4,
    );
}

#[test]
pub(super) fn dropped_unneeded_sequence_load_does_not_retain_decoded_assets() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: bundled_asset_root().to_string_lossy().into_owned(),
            ..Default::default()
        },
        ImagePlugin::default(),
    ));
    app.add_plugins(BuiMediaPlugin);
    app.finish();
    app.cleanup();

    let initial_image_count = app.world().resource::<Assets<Image>>().len();
    let initial_sequence_count = app.world().resource::<Assets<BuiMediaSequence>>().len();
    let sequence: Handle<BuiMediaSequence> = app.world().resource::<AssetServer>().load("loop.gif");
    let sequence_id = sequence.id();
    drop(sequence);

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        let load_cancelled = app
            .world()
            .resource::<AssetServer>()
            .get_load_state(sequence_id)
            .is_none();
        let sequences_gone =
            app.world().resource::<Assets<BuiMediaSequence>>().len() == initial_sequence_count;
        let images_gone = app.world().resource::<Assets<Image>>().len() == initial_image_count;
        let budget_empty = app.world().resource::<BuiMediaDecodedBudget>().used_bytes() == 0;
        if load_cancelled && sequences_gone && images_gone && budget_empty {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "dropped sequence load retained load={load_cancelled}, sequences={sequences_gone}, images={images_gone}, budget={budget_empty}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

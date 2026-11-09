use super::*;

#[test]
pub(super) fn decoded_frame_budget_is_atomic_across_concurrent_sequence_loads() {
    const THREADS: usize = 8;
    const RESERVATION_BYTES: usize = 32;
    let budget = BuiMediaDecodedBudget::new(THREADS * RESERVATION_BYTES);
    let (status_send, status_receive) = std::sync::mpsc::channel();

    thread::scope(|scope| {
        let mut releases = Vec::with_capacity(THREADS);
        for _ in 0..THREADS {
            let budget = budget.clone();
            let status_send = status_send.clone();
            let (release_send, release_receive) = std::sync::mpsc::channel();
            releases.push(release_send);
            scope.spawn(move || {
                let reservation = budget.try_reserve(RESERVATION_BYTES);
                status_send.send(reservation.is_some()).unwrap();
                let _ = release_receive.recv_timeout(Duration::from_secs(2));
                drop(reservation);
            });
        }

        drop(status_send);
        let successful_reservations = (0..THREADS)
            .filter(|_| {
                status_receive
                    .recv_timeout(Duration::from_secs(3))
                    .expect("each worker reports one reservation within three seconds")
            })
            .count();
        if successful_reservations != THREADS {
            for release in &releases {
                let _ = release.send(());
            }
            assert_eq!(successful_reservations, THREADS);
        }
        assert_eq!(budget.used_bytes(), THREADS * RESERVATION_BYTES);
        assert!(budget.try_reserve(1).is_none());
        for release in releases {
            let _ = release.send(());
        }
    });

    assert_eq!(budget.used_bytes(), 0);
}

#[test]
pub(super) fn gif_loader_reports_global_budget_failure_and_releases_reservation() {
    let mut app = App::new();
    app.insert_resource(BuiMediaLimits {
        max_global_decoded_bytes: 1,
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
    let failure = loop {
        app.update();
        let state = &app.world().get::<BuiMediaPlayer>(player).unwrap().state;
        if let BuiMediaPlayerState::Failed(message) = state {
            break message.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "visible GIF did not report its decoded-frame budget failure"
        );
        thread::sleep(Duration::from_millis(2));
    };

    assert!(
        failure.contains("global decoded-frame memory budget"),
        "{failure}"
    );
    assert!(matches!(
        app.world()
            .resource::<AssetServer>()
            .get_load_state(sequence.id()),
        Some(bevy::asset::LoadState::Loaded)
    ));
    assert_eq!(
        app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
        0
    );
}

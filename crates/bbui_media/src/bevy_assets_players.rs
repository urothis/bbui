use super::*;

pub(super) fn normalize_duration(duration: Duration) -> Duration {
    duration.max(MIN_FRAME_DURATION)
}

pub(super) type MediaPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut BbuiMediaPlayer,
        &'static mut ImageNode,
        Option<&'static Visibility>,
        Option<&'static InheritedVisibility>,
        Option<&'static bevy::camera::visibility::ViewVisibility>,
    ),
>;

#[derive(SystemParam)]
pub(super) struct MediaPlayerUpdate<'w, 's> {
    time: Res<'w, Time>,
    sequences: Res<'w, Assets<BbuiMediaSequence>>,
    limits: Res<'w, BbuiMediaLimits>,
    budget: Res<'w, BbuiMediaDecodedBudget>,
    image_events: MessageReader<'w, 's, AssetEvent<Image>>,
    image_reservations: ResMut<'w, DecodedImageBudgetReservations>,
    cache: ResMut<'w, BbuiMediaFrameCache>,
    images: ResMut<'w, Assets<Image>>,
}

pub(super) fn update_media_players(update: MediaPlayerUpdate, mut players: MediaPlayerQuery) {
    let MediaPlayerUpdate {
        time,
        sequences,
        limits,
        budget,
        mut image_events,
        mut image_reservations,
        mut cache,
        mut images,
    } = update;
    for event in image_events.read() {
        match event {
            AssetEvent::Unused { id } | AssetEvent::Removed { id } => {
                image_reservations.by_image.remove(id);
            }
            AssetEvent::Added { .. }
            | AssetEvent::Modified { .. }
            | AssetEvent::LoadedWithDependencies { .. } => {}
        }
    }
    image_reservations
        .by_image
        .retain(|id, _| images.get(*id).is_some());
    let now = time.elapsed();
    let mut visible_video_players = HashSet::new();

    let mut restart_video_players = HashSet::new();
    for (entity, mut player, mut image_node, visibility, inherited, view_visibility) in &mut players
    {
        let sequence_id = player.sequence.id();
        if player.active_sequence != Some(sequence_id) {
            remove_video_stream(&mut cache, entity);
            player.active_sequence = Some(sequence_id);
            player.current_frame = 0;
            player.position = Duration::ZERO;
            player.state = BbuiMediaPlayerState::WaitingForAsset;
            image_node.image = ImageNode::default().image;
        }
        let Some(sequence) = sequences.get(&player.sequence) else {
            player.state = BbuiMediaPlayerState::WaitingForAsset;
            image_node.image = ImageNode::default().image;
            continue;
        };
        let is_visible = visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
            && inherited.is_none_or(|visibility| visibility.get())
            && view_visibility.is_none_or(|visibility| visibility.get());

        if let Some(source) = sequence.encoded_source.as_ref() {
            if !is_visible {
                player.state = BbuiMediaPlayerState::Hidden;
                image_node.image = ImageNode::default().image;
                continue;
            }
            visible_video_players.insert(entity);
            if let std::collections::hash_map::Entry::Vacant(entry) =
                cache.video_streams.entry(entity)
            {
                let stream = start_source_media_stream(
                    sequence_id,
                    Arc::clone(source),
                    sequence.kind,
                    *limits,
                    now,
                );
                entry.insert(stream);
            }
            if let Some(stream) = cache.video_streams.get_mut(&entity) {
                stream.last_used = now;
            }
            continue;
        }

        if sequence.frames.is_empty() {
            player.state = BbuiMediaPlayerState::Failed("sequence has no frames".into());
            continue;
        }
        player.state = BbuiMediaPlayerState::Ready;
        if player.playing && is_visible {
            advance_player(&mut player, sequence.duration(), time.delta());
        }
        let frame_index = sequence.frame_at(player.position);
        player.current_frame = frame_index;
        let frame = &sequence.frames[frame_index];
        if image_node.image != *frame {
            image_node.image = frame.clone();
        }
    }

    let hidden_video_players = cache
        .video_streams
        .keys()
        .filter(|entity| !visible_video_players.contains(entity))
        .copied()
        .collect::<Vec<_>>();
    for entity in hidden_video_players {
        remove_video_stream(&mut cache, entity);
    }

    poll_h264_video_streams(
        &mut cache,
        &mut images,
        &budget,
        &mut image_reservations,
        *limits,
        now,
    );

    for (entity, mut player, mut image_node, visibility, inherited, view_visibility) in &mut players
    {
        let Some(sequence) = sequences.get(&player.sequence) else {
            continue;
        };
        let Some(_) = sequence.encoded_source else {
            continue;
        };
        let is_visible = visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
            && inherited.is_none_or(|visibility| visibility.get())
            && view_visibility.is_none_or(|visibility| visibility.get());
        if !is_visible {
            continue;
        }
        let Some(stream) = cache.video_streams.get_mut(&entity) else {
            player.state = BbuiMediaPlayerState::Decoding;
            continue;
        };
        if let Some(error) = stream.failure.as_ref() {
            player.state = BbuiMediaPlayerState::Failed(error.clone());
            image_node.image = ImageNode::default().image;
            continue;
        }
        if stream.frames.is_empty() {
            player.state = BbuiMediaPlayerState::Decoding;
            continue;
        }
        if stream.eof
            && let Some(duration) = stream.duration
        {
            let previous_position = player.position;
            if player.playing {
                advance_player(&mut player, duration, time.delta());
            }
            if player.looping && player.playing && player.position < previous_position {
                player.state = BbuiMediaPlayerState::Decoding;
                image_node.image = ImageNode::default().image;
                restart_video_players.insert(entity);
                continue;
            }
        } else if player.playing {
            player.position = player.position.saturating_add(time.delta());
        }

        let desired_ns = player.position.as_nanos().min(u128::from(u64::MAX)) as u64;
        let oldest_timestamp = stream.frames.front().map(|frame| frame.timestamp);
        if player.position.is_zero() && stream.frames.front().is_some_and(|frame| frame.index > 0) {
            player.state = BbuiMediaPlayerState::Decoding;
            image_node.image = ImageNode::default().image;
            restart_video_players.insert(entity);
            continue;
        }
        if oldest_timestamp.is_some_and(|timestamp| player.position < timestamp)
            && stream.frames.front().is_some_and(|frame| frame.index > 0)
        {
            player.state = BbuiMediaPlayerState::Decoding;
            image_node.image = ImageNode::default().image;
            restart_video_players.insert(entity);
            continue;
        }
        let Some(selected) = stream
            .frames
            .iter()
            .rev()
            .find(|frame| frame.timestamp.as_nanos() <= u128::from(desired_ns))
            .or_else(|| stream.frames.front())
        else {
            continue;
        };
        player.current_frame = selected.index;
        stream.display_index = selected.index;
        if image_node.image != selected.image {
            image_node.image = selected.image.clone();
        }
        player.state = BbuiMediaPlayerState::Ready;
        stream.last_used = now;
        let current_index = selected.index;
        while stream
            .frames
            .front()
            .is_some_and(|frame| frame.index < current_index)
        {
            if let Some(frame) = stream.frames.pop_front() {
                stream.cached_bytes = stream.cached_bytes.saturating_sub(frame.byte_len);
                drop(frame.image);
            }
        }
    }
    for entity in restart_video_players {
        remove_video_stream(&mut cache, entity);
    }
}

pub(super) fn start_source_media_stream(
    sequence_id: AssetId<BbuiMediaSequence>,
    source: Arc<[u8]>,
    kind: MediaSequenceKind,
    limits: BbuiMediaLimits,
    now: Duration,
) -> StreamingVideo {
    let (sender, receiver) = sync_channel(STREAMED_VIDEO_CHANNEL_FRAMES);
    let failure_sender = sender.clone();
    let cancellation = Arc::new(AtomicBool::new(false));
    let task_cancellation = Arc::clone(&cancellation);
    let task = std::thread::Builder::new()
        .name("bbui-media-decode".to_owned())
        .spawn(move || {
            let mut emit = |frame| send_stream_frame(&sender, &task_cancellation, frame);
            let result = match kind {
                MediaSequenceKind::Gif => decode_gif_stream_cancellable(
                    &source,
                    &task_cancellation,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
                MediaSequenceKind::IvfVp8 => decode_ivf_vp8_stream_cancellable(
                    &source,
                    &task_cancellation,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
                MediaSequenceKind::WebmVp8 => decode_webm_vp8_stream_cancellable(
                    &source,
                    &task_cancellation,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
                MediaSequenceKind::Mp4H264 => decode_mp4_h264_stream_cancellable(
                    Arc::clone(&source),
                    &task_cancellation,
                    limits.max_encoded_bytes,
                    limits.max_sequence_frames,
                    limits.max_sequence_bytes,
                    &mut emit,
                ),
            };
            let completion = result
                .map(|_| ())
                .map_err(|error| configured_decode_error(error).to_string());
            let _ = sender.send(VideoStreamMessage::Finished(completion));
        });
    let task = match task {
        Ok(task) => Some(task),
        Err(error) => {
            let _ = failure_sender.send(VideoStreamMessage::Finished(Err(format!(
                "failed to spawn media decode worker: {error}"
            ))));
            None
        }
    };
    StreamingVideo {
        sequence_id,
        receiver: Mutex::new(receiver),
        task,
        cancellation,
        frames: VecDeque::new(),
        pending_frame: None,
        next_index: 0,
        frame_capacity: 0,
        cached_bytes: 0,
        display_index: 0,
        eof: false,
        duration: None,
        failure: None,
        last_used: now,
        pending_budget_waits: 0,
    }
}

pub(super) fn send_stream_frame(
    sender: &SyncSender<VideoStreamMessage>,
    cancellation: &AtomicBool,
    frame: crate::VideoFrame,
) -> bool {
    match sender.send(VideoStreamMessage::Frame(frame)) {
        Ok(()) => true,
        Err(_) => {
            cancellation.store(true, Ordering::Release);
            false
        }
    }
}

pub(super) fn remove_video_stream(cache: &mut BbuiMediaFrameCache, owner: Entity) {
    let Some(mut stream) = cache.video_streams.remove(&owner) else {
        return;
    };
    stream.cancellation.store(true, Ordering::Release);
    if stream.task.take().is_some() && !stream.eof {
        cache.cancelled_jobs = cache.cancelled_jobs.saturating_add(1);
    }
    stream.frames.clear();
}

pub(super) fn poll_h264_video_streams(
    cache: &mut BbuiMediaFrameCache,
    images: &mut Assets<Image>,
    budget: &BbuiMediaDecodedBudget,
    image_reservations: &mut DecodedImageBudgetReservations,
    limits: BbuiMediaLimits,
    now: Duration,
) {
    let mut messages = Vec::new();
    for (owner, stream) in &mut cache.video_streams {
        if stream.failure.is_some() || stream.eof {
            continue;
        }
        if let Some(frame) = stream.pending_frame.take() {
            let byte_len = frame.frame.pixels.len();
            if stream.frames.len() < stream.frame_capacity
                && stream.cached_bytes.saturating_add(byte_len) <= limits.max_sequence_bytes
            {
                messages.push((*owner, VideoStreamMessage::Frame(frame)));
            } else {
                stream.pending_frame = Some(frame);
            }
            continue;
        }
        let message = stream
            .receiver
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_recv();
        match message {
            Ok(message) => messages.push((*owner, message)),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
    }

    for (owner, message) in messages {
        match message {
            VideoStreamMessage::Frame(frame) => {
                let byte_len = frame.frame.pixels.len();
                if byte_len > limits.max_sequence_bytes {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.failure = Some(MediaAssetLoadError::DecodedLimit.to_string());
                        stream.cancellation.store(true, Ordering::Release);
                    }
                    continue;
                }
                let frame_capacity = (limits.max_sequence_bytes / byte_len.max(1))
                    .min(MAX_STREAMED_VIDEO_CACHE_FRAMES);
                if frame_capacity < 2 {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.failure = Some(
                            "configured decoded byte limit must hold at least two video frames"
                                .to_owned(),
                        );
                        stream.cancellation.store(true, Ordering::Release);
                    }
                    continue;
                }
                let at_capacity = cache.video_streams.get(&owner).is_some_and(|stream| {
                    stream.frames.len() >= frame_capacity
                        || stream.cached_bytes.saturating_add(byte_len) > limits.max_sequence_bytes
                });
                if at_capacity {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.frame_capacity = frame_capacity;
                        stream.pending_frame = Some(frame);
                    }
                    continue;
                }
                if byte_len > budget.max_bytes() {
                    if let Some(stream) = cache.video_streams.get_mut(&owner) {
                        stream.failure = Some(MediaAssetLoadError::GlobalDecodedLimit.to_string());
                        stream.cancellation.store(true, Ordering::Release);
                    }
                    continue;
                }
                if !make_room_for_video_frame(cache, budget, byte_len, owner) {
                    defer_video_frame_for_budget(cache, owner, frame);
                    continue;
                }
                let Some(reservation) = budget.try_reserve(byte_len) else {
                    defer_video_frame_for_budget(cache, owner, frame);
                    continue;
                };
                match image_from_frame(frame.frame) {
                    Ok(image) => {
                        let image = images.add(image);
                        image_reservations.by_image.insert(image.id(), reservation);
                        if let Some(stream) = cache.video_streams.get_mut(&owner) {
                            stream.frame_capacity = frame_capacity;
                            stream.cached_bytes = stream.cached_bytes.saturating_add(byte_len);
                            stream.frames.push_back(CachedVideoFrame {
                                index: stream.next_index,
                                timestamp: Duration::from_nanos(frame.timestamp_ns),
                                duration: Duration::from_nanos(
                                    frame
                                        .duration_ns
                                        .unwrap_or(DEFAULT_VIDEO_FRAME_DURATION.as_nanos() as u64),
                                ),
                                byte_len,
                                image,
                            });
                            stream.next_index = stream.next_index.saturating_add(1);
                            stream.pending_budget_waits = 0;
                            stream.last_used = now;
                        }
                    }
                    Err(error) => {
                        if let Some(stream) = cache.video_streams.get_mut(&owner) {
                            stream.failure = Some(error.to_string());
                            stream.cancellation.store(true, Ordering::Release);
                        }
                    }
                }
            }
            VideoStreamMessage::Finished(result) => {
                if let Some(stream) = cache.video_streams.get_mut(&owner) {
                    match result {
                        Ok(()) => {
                            stream.eof = true;
                            stream.duration = stream
                                .frames
                                .back()
                                .map(|frame| frame.timestamp.saturating_add(frame.duration));
                        }
                        Err(error) => {
                            stream.failure = Some(error);
                        }
                    }
                }
            }
        }
    }

    for stream in cache.video_streams.values_mut() {
        if stream
            .task
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(task) = stream.task.take()
        {
            let _ = task.join();
        }
    }
}

pub(super) fn defer_video_frame_for_budget(
    cache: &mut BbuiMediaFrameCache,
    owner: Entity,
    frame: crate::VideoFrame,
) {
    let Some(stream) = cache.video_streams.get_mut(&owner) else {
        return;
    };
    let waits = stream.pending_budget_waits.saturating_add(1);
    if waits > 8 {
        stream.failure = Some(MediaAssetLoadError::GlobalDecodedLimit.to_string());
        stream.cancellation.store(true, Ordering::Release);
    } else {
        stream.pending_budget_waits = waits;
        stream.pending_frame = Some(frame);
    }
}

pub(super) fn make_room_for_video_frame(
    cache: &mut BbuiMediaFrameCache,
    budget: &BbuiMediaDecodedBudget,
    bytes: usize,
    protected_owner: Entity,
) -> bool {
    while budget.available_bytes() < bytes {
        let inactive_video_frame = cache
            .video_streams
            .iter()
            .filter(|(owner, stream)| {
                **owner != protected_owner
                    && stream
                        .frames
                        .front()
                        .is_some_and(|frame| frame.index < stream.display_index)
            })
            .min_by_key(|(_, stream)| stream.last_used)
            .map(|(owner, _)| *owner);
        let Some(owner) = inactive_video_frame else {
            return false;
        };
        if let Some(stream) = cache.video_streams.get_mut(&owner)
            && let Some(frame) = stream.frames.pop_front()
        {
            stream.cached_bytes = stream.cached_bytes.saturating_sub(frame.byte_len);
            drop(frame.image);
        }
    }
    true
}

pub(super) fn advance_player(player: &mut BbuiMediaPlayer, duration: Duration, delta: Duration) {
    if duration.is_zero() {
        return;
    }
    player.position = player.position.saturating_add(delta);
    if player.position >= duration {
        if player.looping {
            let position_ns = player.position.as_nanos() % duration.as_nanos();
            player.position = Duration::new(
                (position_ns / 1_000_000_000) as u64,
                (position_ns % 1_000_000_000) as u32,
            );
        } else {
            player.position = duration.saturating_sub(MIN_FRAME_DURATION);
            player.playing = false;
        }
    }
}

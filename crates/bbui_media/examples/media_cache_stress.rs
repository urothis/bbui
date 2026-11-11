//! Rotate visibility across 16 strong sequence owners to exercise decode cancellation and limits.
//!
//! Run with `cargo run -p bbui_media --example media_cache_stress`. Each generated 8-frame GIF
//! decodes to 128 KiB. The global budget holds one rolling frame cache, while every player keeps
//! a strong sequence handle. Hiding a player cancels its worker and releases cached frame images.

use bbui_media::{
    BbuiMediaDecodedBudget, BbuiMediaFrameCache, BbuiMediaLimits, BbuiMediaPlayer,
    BbuiMediaPlayerState, BbuiMediaPlugin, BbuiMediaSequence, MediaSequenceKind,
};
use bevy::{
    asset::Assets,
    prelude::{
        App, Camera2d, DefaultPlugins, ImageNode, Local, Node, PositionType, Query, Res, ResMut,
        Resource, Startup, Time, Timer, TimerMode, Update, Val, Visibility, default,
    },
};
use image::{Delay, Frame, Rgba, RgbaImage, codecs::gif::GifEncoder};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const FRAMES: u32 = 8;
const SEQUENCES: usize = 16;
const FRAME_CACHE_BYTES: usize = WIDTH as usize * HEIGHT as usize * 4 * FRAMES as usize;

#[derive(Resource)]
struct StressPlayers {
    entities: Vec<bevy::prelude::Entity>,
    active: usize,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(BbuiMediaLimits {
            max_global_decoded_bytes: FRAME_CACHE_BYTES,
            ..Default::default()
        })
        .add_plugins(BbuiMediaPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (rotate_visible_owner, report_cache_state))
        .run();
}

fn setup(mut commands: bevy::prelude::Commands, mut sequences: ResMut<Assets<BbuiMediaSequence>>) {
    commands.spawn(Camera2d);
    let bytes = generated_animation();
    let mut entities = Vec::with_capacity(SEQUENCES);
    for index in 0..SEQUENCES {
        let sequence = sequences.add(BbuiMediaSequence::from_encoded_bytes(
            bytes.clone(),
            MediaSequenceKind::Gif,
        ));
        let column = index % 4;
        let row = index / 4;
        let visibility = if index == 0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let mut player = BbuiMediaPlayer::new(sequence);
        player.playing = false;
        let entity = commands
            .spawn((
                Node {
                    width: Val::Px(100.0),
                    height: Val::Px(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(20.0 + column as f32 * 112.0),
                    top: Val::Px(20.0 + row as f32 * 112.0),
                    ..default()
                },
                ImageNode::default(),
                player,
                visibility,
            ))
            .id();
        entities.push(entity);
    }
    commands.insert_resource(StressPlayers {
        entities,
        active: 0,
    });
}

fn generated_animation() -> Vec<u8> {
    let frames = (0..FRAMES).map(|frame_index| {
        let pixels = RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
            let phase = frame_index as u8 * 29;
            Rgba([
                (x as u8).wrapping_add(phase),
                (y as u8).wrapping_add(phase.wrapping_mul(3)),
                phase.wrapping_add(48),
                255,
            ])
        });
        Frame::from_parts(pixels, 0, 0, Delay::from_numer_denom_ms(100, 1))
    });
    let mut bytes = Vec::new();
    if let Err(error) = GifEncoder::new(&mut bytes).encode_frames(frames) {
        eprintln!("encode stress animation: {error}");
        return Vec::new();
    }
    bytes
}

fn rotate_visible_owner(
    time: Res<Time>,
    mut timer: Local<Timer>,
    mut stress: ResMut<StressPlayers>,
    mut visibility: Query<&mut Visibility>,
) {
    if timer.duration().is_zero() {
        *timer = Timer::from_seconds(1.5, TimerMode::Repeating);
    }
    timer.tick(time.delta());
    if !timer.just_finished() {
        return;
    }
    stress.active = (stress.active + 1) % stress.entities.len();
    for (index, entity) in stress.entities.iter().copied().enumerate() {
        if let Ok(mut visibility) = visibility.get_mut(entity) {
            *visibility = if index == stress.active {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
}

fn report_cache_state(
    stress: Res<StressPlayers>,
    budget: Res<BbuiMediaDecodedBudget>,
    cache: Res<BbuiMediaFrameCache>,
    players: Query<&BbuiMediaPlayer>,
    mut last_reported_state: Local<Option<(usize, u8)>>,
) {
    let (state_id, state) = match players.get(stress.entities[stress.active]) {
        Ok(player) => match &player.state {
            BbuiMediaPlayerState::WaitingForAsset => (0, "waiting for asset".to_owned()),
            BbuiMediaPlayerState::Hidden => (1, "hidden".to_owned()),
            BbuiMediaPlayerState::Decoding => (2, "decoding visible frames".to_owned()),
            BbuiMediaPlayerState::Ready => (3, "ready".to_owned()),
            BbuiMediaPlayerState::Failed(error) => (4, format!("failed: {error}")),
        },
        Err(_) => (5, "missing player".to_owned()),
    };
    if *last_reported_state == Some((stress.active, state_id)) {
        return;
    }
    *last_reported_state = Some((stress.active, state_id));
    let ready_strong_owners = players
        .iter()
        .filter(|player| player.state == BbuiMediaPlayerState::Ready)
        .count();
    println!(
        "active owner {}/{}: {state}; active streams={} frames={} cancelled={} decoded={} / {} bytes; ready players={ready_strong_owners}",
        stress.active + 1,
        stress.entities.len(),
        cache.sequence_count(),
        cache.cached_image_count(),
        cache.cancelled_job_count(),
        budget.used_bytes(),
        budget.max_bytes(),
    );
}

//! Run with `cargo run -p bbui_media --example media_playback`.
//! The crate-local assets directory contains one SVG, an animated GIF, and VP8 WebM and H.264 MP4
//! samples.
//! `loop.gif` is a generated two-pixel red/blue fixture with no third-party artwork.
//! Animated/video sources stay encoded until a player becomes visible. Hidden players pause their
//! clock, release their cache-owned frame images after the configured idle window, and keep their
//! sequence handle/source alive. Bevy releases the corresponding GPU images after render cleanup.

use std::collections::HashSet;

use bbui_media::{
    BbuiMediaDecodedBudget, BbuiMediaFrameCache, BbuiMediaPlayer, BbuiMediaPlayerState,
    BbuiMediaPlugin, BbuiMediaSequence,
};
use bevy::{
    app::PluginGroup,
    asset::{Asset, AssetPlugin, AssetServer, Handle, LoadState},
    prelude::{
        App, DefaultPlugins, Image, ImageNode, Node, PositionType, Query, Res, Resource, Startup,
        Update, Val, default,
    },
};

#[derive(Resource)]
struct SampleMedia {
    svg: Handle<Image>,
    gif: Handle<BbuiMediaSequence>,
    webm: Handle<BbuiMediaSequence>,
    h264: Handle<BbuiMediaSequence>,
    gif_player: bevy::prelude::Entity,
    webm_player: bevy::prelude::Entity,
    h264_player: bevy::prelude::Entity,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            file_path: "crates/bbui_media/assets".into(),
            ..default()
        }))
        .add_plugins(BbuiMediaPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, report_asset_states)
        .run();
}

fn setup(mut commands: bevy::prelude::Commands, asset_server: Res<AssetServer>) {
    commands.spawn(bevy::prelude::Camera2d);

    let svg = asset_server.load("sample.svg");
    let gif = asset_server.load("loop.gif");
    let webm = asset_server.load("sample.webm");
    let h264 = asset_server.load("sample-h264.mp4");

    spawn_image(&mut commands, 20.0, ImageNode::new(svg.clone()));
    let gif_player = spawn_player(&mut commands, 210.0, gif.clone());
    let webm_player = spawn_player(&mut commands, 400.0, webm.clone());
    let h264_player = spawn_player(&mut commands, 590.0, h264.clone());
    commands.insert_resource(SampleMedia {
        svg,
        gif,
        webm,
        h264,
        gif_player,
        webm_player,
        h264_player,
    });
}

fn spawn_image(commands: &mut bevy::prelude::Commands, left: f32, image: ImageNode) {
    commands.spawn((
        Node {
            width: Val::Px(160.0),
            height: Val::Px(100.0),
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(50.0),
            ..default()
        },
        image,
    ));
}

fn spawn_player(
    commands: &mut bevy::prelude::Commands,
    left: f32,
    sequence: Handle<BbuiMediaSequence>,
) -> bevy::prelude::Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(50.0),
                ..default()
            },
            ImageNode::default(),
            BbuiMediaPlayer::new(sequence),
        ))
        .id()
}

fn report_asset_states(
    asset_server: Res<AssetServer>,
    media: Option<Res<SampleMedia>>,
    budget: Res<BbuiMediaDecodedBudget>,
    cache: Res<BbuiMediaFrameCache>,
    players: Query<&BbuiMediaPlayer>,
    mut failures_reported: bevy::prelude::Local<HashSet<&'static str>>,
    mut states_reported: bevy::prelude::Local<HashSet<(&'static str, u8)>>,
) {
    let Some(media) = media else {
        return;
    };
    report_failure(
        &asset_server,
        &media.svg,
        "sample.svg",
        &mut failures_reported,
    );
    report_failure(
        &asset_server,
        &media.webm,
        "sample.webm",
        &mut failures_reported,
    );
    report_failure(
        &asset_server,
        &media.h264,
        "sample-h264.mp4",
        &mut failures_reported,
    );
    report_failure(
        &asset_server,
        &media.gif,
        "loop.gif",
        &mut failures_reported,
    );
    report_failure(
        &asset_server,
        &media.webm,
        "sample.webm",
        &mut failures_reported,
    );
    report_player_state(
        players.get(media.gif_player).ok(),
        "loop.gif",
        &budget,
        &cache,
        &mut states_reported,
    );
    report_player_state(
        players.get(media.webm_player).ok(),
        "sample.webm",
        &budget,
        &cache,
        &mut states_reported,
    );
    report_player_state(
        players.get(media.h264_player).ok(),
        "sample-h264.mp4",
        &budget,
        &cache,
        &mut states_reported,
    );
}

fn report_player_state(
    player: Option<&BbuiMediaPlayer>,
    path: &'static str,
    budget: &BbuiMediaDecodedBudget,
    cache: &BbuiMediaFrameCache,
    reported: &mut HashSet<(&'static str, u8)>,
) {
    let Some(player) = player else {
        return;
    };
    let (state_id, description) = match &player.state {
        BbuiMediaPlayerState::WaitingForAsset => (0, "waiting for sequence asset".to_owned()),
        BbuiMediaPlayerState::Hidden => (1, "hidden; decode paused".to_owned()),
        BbuiMediaPlayerState::Decoding => (2, "decoding visible frames".to_owned()),
        BbuiMediaPlayerState::Ready => (3, "playing from decoded frame cache".to_owned()),
        BbuiMediaPlayerState::Failed(error) => (4, format!("failed: {error}")),
    };
    if reported.insert((path, state_id)) {
        println!(
            "{path}: {description}; cached images {}, decoded bytes {} / {}, cancelled jobs {}",
            cache.cached_image_count(),
            budget.used_bytes(),
            budget.max_bytes(),
            cache.cancelled_job_count()
        );
    }
}

fn report_failure<A: Asset>(
    asset_server: &AssetServer,
    handle: &Handle<A>,
    path: &'static str,
    reported: &mut HashSet<&'static str>,
) {
    if let Some(LoadState::Failed(error)) = asset_server.get_load_state(handle.id())
        && reported.insert(path)
    {
        eprintln!("failed to load {path}: {error}");
    }
}

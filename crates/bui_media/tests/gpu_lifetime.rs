//! Verifies cache-owned media Images leave Bevy's render-world GPU asset map after eviction.
#![cfg(target_os = "linux")]

use std::{
    thread,
    time::{Duration, Instant},
};

use bevy::{
    app::PluginGroup,
    asset::{Assets, RenderAssetUsages},
    camera::RenderTarget,
    image::Image,
    prelude::{App, Camera2d, DefaultPlugins, ImageNode, Node, Val, WindowPlugin, default},
    render::{
        RenderApp, RenderPlugin,
        render_asset::RenderAssets,
        settings::{RenderCreation, WgpuSettings},
        texture::GpuImage,
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use bui_media::{
    BuiMediaDecodedBudget, BuiMediaLimits, BuiMediaPlayer, BuiMediaPlayerState, BuiMediaPlugin,
    BuiMediaSequence, MediaSequenceKind,
};

fn gpu_image_exists(app: &App, id: bevy::asset::AssetId<Image>) -> bool {
    app.get_sub_app(RenderApp)
        .and_then(|render| render.world().get_resource::<RenderAssets<GpuImage>>())
        .is_some_and(|assets| assets.get(id).is_some())
}

#[test]
fn evicted_media_image_releases_render_world_gpu_image_while_sequence_owner_remains() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    force_fallback_adapter: true,
                    ..default()
                })),
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::pbr::PbrPlugin>(),
    );
    app.insert_resource(BuiMediaLimits { ..default() });
    app.add_plugins(BuiMediaPlugin);
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(
        16,
        16,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Image(target.into())));
    let sequence = app
        .world_mut()
        .resource_mut::<Assets<BuiMediaSequence>>()
        .add(BuiMediaSequence::from_encoded_bytes(
            include_bytes!("../assets/loop.gif").to_vec(),
            MediaSequenceKind::Gif,
        ));
    let sequence_id = sequence.id();
    let player = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                ..default()
            },
            ImageNode::default(),
            BuiMediaPlayer::new(sequence.clone()),
            bevy::prelude::Visibility::Visible,
            bevy::prelude::InheritedVisibility::VISIBLE,
        ))
        .id();
    // A UI node without a window camera can be marked ViewVisibility::HIDDEN by Bevy's culling.
    // Omitting that optional signal keeps this test focused on ownership and render extraction;
    // real UI apps use the computed view visibility to pause offscreen players.
    app.world_mut()
        .entity_mut(player)
        .remove::<bevy::camera::visibility::ViewVisibility>();

    let deadline = Instant::now() + Duration::from_secs(10);
    while app.world().get::<BuiMediaPlayer>(player).unwrap().state != BuiMediaPlayerState::Ready {
        app.update();
        assert!(
            Instant::now() < deadline,
            "visible media player never became ready: state={:?}",
            app.world().get::<BuiMediaPlayer>(player).unwrap().state
        );
        thread::sleep(Duration::from_millis(5));
    }
    let frame_id = app.world().get::<ImageNode>(player).unwrap().image.id();
    assert!(
        app.world()
            .resource::<Assets<Image>>()
            .get(frame_id)
            .is_some()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !gpu_image_exists(&app, frame_id) {
        app.update();
        assert!(
            Instant::now() < deadline,
            "decoded media Image never reached RenderAssets<GpuImage>; source asset={:?}",
            app.world()
                .resource::<Assets<Image>>()
                .get(frame_id)
                .map(Image::width)
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(gpu_image_exists(&app, frame_id));

    *app.world_mut()
        .get_mut::<bevy::prelude::Visibility>(player)
        .unwrap() = bevy::prelude::Visibility::Hidden;
    *app.world_mut()
        .get_mut::<bevy::prelude::InheritedVisibility>(player)
        .unwrap() = bevy::prelude::InheritedVisibility::HIDDEN;
    app.update();
    assert!(
        app.world()
            .resource::<Assets<BuiMediaSequence>>()
            .get(sequence_id)
            .is_some()
    );
    assert!(app.world().get_entity(player).is_ok());

    let deadline = Instant::now() + Duration::from_secs(10);
    while app
        .world()
        .resource::<Assets<Image>>()
        .get(frame_id)
        .is_some()
        || gpu_image_exists(&app, frame_id)
        || app.world().resource::<BuiMediaDecodedBudget>().used_bytes() != 0
    {
        app.update();
        assert!(
            Instant::now() < deadline,
            "evicted frame was not released by Bevy asset/render cleanup"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.world().resource::<BuiMediaDecodedBudget>().used_bytes(),
        0
    );
}

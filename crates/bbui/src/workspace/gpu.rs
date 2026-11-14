//! Opt-in GPU capture of the real workspace chrome for visual review.

use super::*;
use bbui_style::{Theme, ThemeKind};
use bbui_widgets::{DockNode, SplitAxis, TabGroup};
use bevy::{
    app::PluginGroup,
    camera::RenderTarget,
    image::Image,
    render::{
        RenderPlugin,
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
        settings::{RenderCreation, WgpuSettings},
    },
    ui::IsDefaultUiCamera,
    window::{ExitCondition, WindowPlugin},
    winit::WinitPlugin,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

#[path = "../../examples/workspace/content.rs"]
mod content;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 480;

#[derive(Resource, Default)]
struct Frames(HashMap<usize, Vec<u8>>);

#[allow(clippy::unwrap_used, clippy::expect_used)]
fn capture(app: &mut App, target: &Handle<Image>, index: usize) -> Vec<u8> {
    let readback = app
        .world_mut()
        .spawn(Readback::texture(target.clone()))
        .observe(
            move |completed: bevy::ecs::observer::On<ReadbackComplete>,
                  mut frames: ResMut<Frames>| {
                frames.0.insert(index, completed.data.clone());
            },
        )
        .id();
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && !app.world().resource::<Frames>().0.contains_key(&index) {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    let bytes = app
        .world()
        .resource::<Frames>()
        .0
        .get(&index)
        .cloned()
        .expect("workspace GPU capture timed out");
    app.world_mut().despawn(readback);
    bytes
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
fn save_png(path: PathBuf, padded: &[u8]) {
    let row_stride = (WIDTH as usize * 4).div_ceil(256) * 256;
    let mut pixels = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
    for y in 0..HEIGHT as usize {
        pixels.extend_from_slice(&padded[y * row_stride..y * row_stride + WIDTH as usize * 4]);
    }
    image::save_buffer_with_format(
        path,
        &pixels,
        WIDTH,
        HEIGHT,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .unwrap();
}

#[test]
#[ignore = "run explicitly to write target/workspace-preview-{dark,light,drop}.png"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
fn capture_workspace_dark_and_light() {
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
    )
    .add_plugins(crate::BbuiPlugin)
    .init_resource::<Frames>();
    app.finish();
    app.cleanup();
    let mut target = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            IsDefaultUiCamera,
        ))
        .id();

    let layout = WorkspaceLayout::new(DockNode::Split {
        id: "main-split".into(),
        axis: SplitAxis::Horizontal,
        shares: vec![2.0, 1.0],
        children: vec![
            DockNode::Tabs(TabGroup::new(
                "documents",
                vec!["document".into(), "preview".into()],
            )),
            DockNode::Grid {
                id: "tools-grid".into(),
                columns: 1,
                children: vec![
                    DockNode::Tabs(TabGroup::new("inspector", vec!["inspector".into()])),
                    DockNode::Tabs(TabGroup::new("console", vec!["console".into()])),
                ],
            },
        ],
    });
    let mut workspace = Workspace::new(layout).unwrap();
    workspace.windows.insert("main".into(), camera);
    for (id, title) in [
        ("document", "Document"),
        ("preview", "Preview"),
        ("inspector", "Inspector"),
        ("console", "Console"),
    ] {
        workspace.register_pane(id, title, move |world, parent| {
            content::mount(world, parent, id)
        });
    }
    app.insert_resource(workspace);
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&output).unwrap();
    let mut dark_pixel = 0_u8;
    for (index, kind) in [ThemeKind::Dark, ThemeKind::Light].into_iter().enumerate() {
        let mut theme = Theme::new(kind);
        content::configure_theme(&mut theme);
        *app.world_mut().resource_mut::<Theme>() = theme;
        for _ in 0..4 {
            app.update();
        }
        let bytes = capture(&mut app, &target, index);
        let row_stride = (WIDTH as usize * 4).div_ceil(256) * 256;
        let pixel = bytes[300 * row_stride + 400 * 4];
        if index == 0 {
            dark_pixel = pixel;
        } else {
            assert!(
                pixel > dark_pixel.saturating_add(80),
                "light pane background did not update"
            );
        }
        let name = if kind == ThemeKind::Dark {
            "dark"
        } else {
            "light"
        };
        save_png(output.join(format!("workspace-preview-{name}.png")), &bytes);
    }
    let mut theme = Theme::new(ThemeKind::Dark);
    content::configure_theme(&mut theme);
    *app.world_mut().resource_mut::<Theme>() = theme;
    app.world_mut()
        .resource_mut::<input::WorkspaceGesture>()
        .hover = Some(input::DropLocation {
        group: "documents".into(),
        edge: input::DropEdge::Right,
    });
    app.world_mut().resource_mut::<Workspace>().dirty = true;
    for _ in 0..3 {
        app.update();
    }
    let bytes = capture(&mut app, &target, 2);
    save_png(output.join("workspace-preview-drop.png"), &bytes);
}

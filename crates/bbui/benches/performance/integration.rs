use bbui::BbuiPlugin;
use bbui_core::UiChildren;
use bbui_layout::{Rect, UiLayoutRoot};
use bbui_render::{ClipRect, DisplayList, Primitive, UiPaint};
use bbui_style::Theme;
use bbui_text::TextBuffer;
use bbui_widgets::{Checkbox, WidgetNode, WidgetTree};
use bevy::{
    asset::AssetPlugin,
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    prelude::{App, ImagePlugin, MinimalPlugins, Window},
    window::WindowPlugin,
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

struct Scene {
    app: App,
    changed_control: bevy::ecs::entity::Entity,
}

fn scene_with_tree(tree: WidgetTree) -> Result<Scene, String> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BbuiPlugin);
    let mut window = Window::default();
    window.resolution.set(1280.0, 720.0);
    let window = app.world_mut().spawn(window).id();
    let root = tree
        .spawn(app.world_mut(), None)
        .map_err(|error| format!("integrated benchmark widget tree: {error:?}"))?;
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
        UiPaint(Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            radius: 0.0,
            color: Theme::default().background,
        }),
    ));
    let changed_control = app
        .world()
        .get::<UiChildren>(root)
        .and_then(|children| children.0.first())
        .copied()
        .ok_or_else(|| "integrated benchmark has no controls".to_owned())?;
    for _ in 0..3 {
        app.update();
    }
    Ok(Scene {
        app,
        changed_control,
    })
}

fn scene(controls: usize) -> Result<Scene, String> {
    scene_with_tree(WidgetTree::new(WidgetNode::column((0..controls).map(
        |index| WidgetNode::checkbox(format!("Control {index:04}"), false),
    ))))
}

fn text_scene(controls: usize) -> Result<Scene, String> {
    scene_with_tree(WidgetTree::new(WidgetNode::column((0..controls).map(
        |index| WidgetNode::text_field(format!("Field {index:04}"), "alpha"),
    ))))
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("bbui_plugin/headless_cpu_update");
    for controls in [200, 1_000, 10_000] {
        group.throughput(Throughput::Elements(controls as u64));
        let Ok(mut idle) = scene(controls) else {
            eprintln!("failed to create integrated idle benchmark scene");
            continue;
        };
        group.bench_with_input(BenchmarkId::new("idle", controls), &controls, |b, _| {
            b.iter(|| {
                idle.app.update();
                black_box(idle.app.world().resource::<DisplayList>().item_count());
            });
        });
        let Ok(mut dirty) = scene(controls) else {
            eprintln!("failed to create integrated dirty benchmark scene");
            continue;
        };
        if dirty
            .app
            .world()
            .get::<Checkbox>(dirty.changed_control)
            .is_none()
        {
            eprintln!("integrated benchmark checkbox is missing");
            continue;
        }
        group.bench_with_input(
            BenchmarkId::new("toggle_checkbox", controls),
            &controls,
            |b, _| {
                b.iter(|| {
                    if let Some(mut checkbox) = dirty
                        .app
                        .world_mut()
                        .get_mut::<Checkbox>(dirty.changed_control)
                    {
                        checkbox.toggle();
                    }
                    dirty.app.update();
                    black_box(dirty.app.world().resource::<DisplayList>().item_count());
                });
            },
        );
    }
    group.finish();

    let mut text_group = c.benchmark_group("bbui_plugin/text_field_update");
    for controls in [200, 1_000] {
        text_group.throughput(Throughput::Elements(controls as u64));
        let Ok(mut idle) = text_scene(controls) else {
            eprintln!("failed to create text idle benchmark scene");
            continue;
        };
        text_group.bench_with_input(BenchmarkId::new("idle", controls), &controls, |b, _| {
            b.iter(|| {
                idle.app.update();
                black_box(idle.app.world().resource::<DisplayList>().item_count());
            });
        });
        let Ok(mut changed) = text_scene(controls) else {
            eprintln!("failed to create text edit benchmark scene");
            continue;
        };
        let mut phase = false;
        text_group.bench_with_input(BenchmarkId::new("edit_one", controls), &controls, |b, _| {
            b.iter(|| {
                phase = !phase;
                changed
                    .app
                    .world_mut()
                    .entity_mut(changed.changed_control)
                    .insert(TextBuffer::new(if phase { "bravo" } else { "alpha" }));
                changed.app.update();
                black_box(changed.app.world().resource::<DisplayList>().item_count());
            });
        });
    }
    text_group.finish();
}

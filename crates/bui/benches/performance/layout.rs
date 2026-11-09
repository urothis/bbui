use bevy::app::App;
use bevy::ecs::entity::Entity;
use bui_core::{CorePlugin, UiNode, set_parent};
use bui_layout::{
    LayoutPlugin, Rect, UiIntrinsicSize, UiLayoutNode, UiLayoutRoot, UiLayoutStats, UiLength,
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

struct Scene {
    app: App,
    first_leaf: Entity,
    next_width: f32,
}

fn scene(nodes: usize) -> Result<Scene, String> {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
            UiLayoutNode::column(2.0),
        ))
        .id();
    let mut first_leaf = None;
    for _ in 0..nodes {
        let leaf = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([20.0, 12.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
            ))
            .id();
        set_parent(app.world_mut(), leaf, root)
            .map_err(|error| format!("layout benchmark hierarchy: {error:?}"))?;
        first_leaf.get_or_insert(leaf);
    }
    for _ in 0..3 {
        app.update();
    }
    Ok(Scene {
        app,
        first_leaf: first_leaf.ok_or_else(|| "layout benchmark has no leaf".to_owned())?,
        next_width: 21.0,
    })
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout/update");
    for nodes in [1_000, 10_000] {
        group.throughput(Throughput::Elements(nodes as u64));
        let Ok(mut idle) = scene(nodes) else {
            eprintln!("failed to create idle layout benchmark scene");
            continue;
        };
        group.bench_with_input(BenchmarkId::new("idle", nodes), &nodes, |b, _| {
            b.iter(|| {
                idle.app.update();
                black_box(
                    idle.app
                        .world()
                        .resource::<UiLayoutStats>()
                        .total_nodes_processed,
                );
            });
        });

        let Ok(mut changed) = scene(nodes) else {
            eprintln!("failed to create dirty layout benchmark scene");
            continue;
        };
        group.bench_with_input(
            BenchmarkId::new("one_intrinsic_change", nodes),
            &nodes,
            |b, _| {
                b.iter(|| {
                    changed
                        .app
                        .world_mut()
                        .entity_mut(changed.first_leaf)
                        .insert(UiIntrinsicSize([changed.next_width, 12.0]));
                    changed.next_width = if changed.next_width == 21.0 {
                        20.0
                    } else {
                        21.0
                    };
                    changed.app.update();
                    black_box(
                        changed
                            .app
                            .world()
                            .resource::<UiLayoutStats>()
                            .total_nodes_processed,
                    );
                });
            },
        );
    }
    group.finish();
}

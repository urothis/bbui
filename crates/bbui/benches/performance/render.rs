use bbui_render::{
    BbuiRenderPlugin, ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive, RenderSyncStats,
    WidgetRenderFragment,
};
use bbui_style::Rgba;
use bevy::{app::App, asset::Handle, image::Image, prelude::MinimalPlugins};
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

fn items(count: usize, images: bool, changed: bool) -> Vec<DrawItem> {
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 1024.0,
        height: 768.0,
    };
    (0..count)
        .map(|index| {
            let rect = ClipRect {
                x: (index % 64) as f32 * 16.0,
                y: (index / 64) as f32 * 12.0,
                width: 14.0,
                height: 10.0,
            };
            let primitive = if images {
                Primitive::Image { rect, asset_id: 1 }
            } else {
                Primitive::RoundedRect {
                    rect,
                    radius: 2.0,
                    color: if changed && index == count / 2 {
                        Rgba(0.9, 0.2, 0.7, 1.0)
                    } else {
                        Rgba(0.18, 0.42, 0.68, 1.0)
                    },
                }
            };
            DrawItem {
                clip,
                z: if images && changed && index == count / 2 {
                    index as i32 + 1
                } else {
                    index as i32
                },
                primitive,
            }
        })
        .collect()
}

fn app(images: bool) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, BbuiRenderPlugin));
    if images {
        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(1, Handle::<Image>::default());
    }
    app
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("render/cpu_sync");
    for (name, count, images) in [
        ("rectangles_1000", 1_000, false),
        ("rectangles_10000", 10_000, false),
        ("images_4096", 4_096, true),
    ] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("first", name), &count, |b, _| {
            b.iter_batched(
                || {
                    let mut app = app(images);
                    app.world_mut()
                        .resource_mut::<DisplayList>()
                        .replace_widget_items(items(count, images, false));
                    app
                },
                |mut app| {
                    app.update();
                    black_box(app.world().entities().len());
                },
                BatchSize::LargeInput,
            );
        });

        let mut idle = app(images);
        idle.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(items(count, images, false));
        idle.update();
        group.bench_with_input(BenchmarkId::new("idle", name), &count, |b, _| {
            b.iter(|| {
                idle.update();
                black_box(idle.world().entities().len());
            });
        });

        let mut dirty = app(images);
        dirty
            .world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(items(count, images, false));
        dirty.update();
        let before = items(count, images, false);
        let after = items(count, images, true);
        let mut phase = false;
        group.bench_with_input(BenchmarkId::new("change_one", name), &count, |b, _| {
            b.iter_batched(
                || {
                    phase = !phase;
                    if phase { &after } else { &before }.clone()
                },
                |next| {
                    dirty
                        .world_mut()
                        .resource_mut::<DisplayList>()
                        .replace_widget_items(next);
                    dirty.update();
                    black_box(dirty.world().entities().len());
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();

    let mut keyed = c.benchmark_group("render/keyed_fragment_sync");
    const FRAGMENTS: usize = 10_000;
    keyed.throughput(Throughput::Elements(FRAGMENTS as u64));
    let mut app = app(false);
    let entities: Vec<_> = (0..FRAGMENTS)
        .map(|_| app.world_mut().spawn_empty().id())
        .collect();
    let base = items(1, false, false).remove(0);
    let changed = items(1, false, true).remove(0);
    {
        let mut list = app.world_mut().resource_mut::<DisplayList>();
        for (order, entity) in entities.iter().copied().enumerate() {
            list.upsert_widget_fragment(WidgetRenderFragment {
                entity,
                order: order as u64,
                target_camera: None,
                items: vec![base.clone()],
            });
        }
    }
    app.update();
    keyed.bench_function("idle/10000", |b| {
        b.iter(|| {
            app.update();
            black_box(app.world().resource::<RenderSyncStats>().scanned_draw_keys);
        });
    });
    let target = entities[FRAGMENTS / 2];
    let mut phase = false;
    keyed.bench_function("change_one/10000", |b| {
        b.iter(|| {
            phase = !phase;
            app.world_mut()
                .resource_mut::<DisplayList>()
                .upsert_widget_fragment(WidgetRenderFragment {
                    entity: target,
                    order: (FRAGMENTS / 2) as u64,
                    target_camera: None,
                    items: vec![if phase { changed.clone() } else { base.clone() }],
                });
            app.update();
            black_box(app.world().resource::<RenderSyncStats>().scanned_draw_keys);
        });
    });
    keyed.finish();
}

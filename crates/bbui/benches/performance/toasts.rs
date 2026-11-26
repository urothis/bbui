//! Bounded toast display with active queues.

use bbui::toasts::{ToastCenter, ToastLifetime, ToastSeverity};
use bevy::ecs::world::World;
use criterion::{BenchmarkId, Criterion};
use std::hint::black_box;

#[allow(clippy::expect_used)]
pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("toasts");
    let mut world = World::new();
    let window = world.spawn_empty().id();
    for count in [3, 32] {
        let mut center = ToastCenter::default();
        for index in 0..count {
            center.post(
                window,
                ToastSeverity::Info,
                None,
                format!("Toast {index}"),
                ToastLifetime::Persistent,
            );
        }
        assert_eq!(center.visible(window).len(), 3);
        group.bench_with_input(BenchmarkId::new("visible", count), &count, |b, _| {
            b.iter(|| black_box(center.visible(black_box(window))))
        });
    }
    group.finish();
}

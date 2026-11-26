//! Job status reconciliation under changing progress values.

use bbui::status::{StatusBar, StatusItem, StatusProgress, StatusTone};
use bbui_widgets::WidgetTree;
use bevy::ecs::world::World;
use criterion::Criterion;
use std::hint::black_box;

#[allow(clippy::expect_used)]
pub fn bench(c: &mut Criterion) {
    let mut world = World::new();
    let mut status = StatusBar {
        items: vec![StatusItem::new("Ready", StatusTone::Normal)],
        progress: Some(StatusProgress::new("Export", 0.0)),
    };
    let root = WidgetTree::new(status.view())
        .spawn(&mut world, None)
        .expect("status mount");
    let mut value = 0.0;
    c.bench_function("status/reconcile", |b| {
        b.iter(|| {
            value = (value + 0.01) % 1.0;
            if let Some(progress) = &mut status.progress {
                progress.fraction = value;
            }
            black_box(
                WidgetTree::new(status.view())
                    .reconcile(&mut world, root)
                    .expect("status update"),
            );
        })
    });
}

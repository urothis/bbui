//! Reconciliation of application-owned content in status bar sockets.

use bbui::status::{StatusBar, StatusBarAppearance, StatusBarSlots};
use bbui_widgets::{WidgetNode, WidgetTree};
use bevy::ecs::world::World;
use criterion::Criterion;
use std::hint::black_box;

fn view(value: f32) -> WidgetNode {
    StatusBar.view_with_slots(
        &StatusBarAppearance::default(),
        StatusBarSlots {
            leading: Some(WidgetNode::label("Ready")),
            center: Some(WidgetNode::label("Export")),
            trailing: Some(WidgetNode::progress(value)),
        },
    )
}

#[allow(clippy::expect_used)]
pub fn bench(c: &mut Criterion) {
    let mut world = World::new();
    let root = WidgetTree::new(view(0.0))
        .spawn(&mut world, None)
        .expect("status mount");
    let mut value = 0.0;
    c.bench_function("status/reconcile", |b| {
        b.iter(|| {
            value = (value + 0.01) % 1.0;
            black_box(
                WidgetTree::new(view(value))
                    .reconcile(&mut world, root)
                    .expect("status update"),
            );
        })
    });
}

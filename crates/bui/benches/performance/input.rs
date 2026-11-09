use bevy::prelude::App;
use bui_core::{CorePlugin, UiFocus, UiHitBox, UiInputEvent, UiInputQueue, UiNode, UiRect};
use criterion::{Criterion, Throughput};
use std::hint::black_box;

pub fn bench(c: &mut Criterion) {
    const CONTROLS: usize = 10_000;
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    let window = app.world_mut().spawn_empty().id();
    for index in 0..CONTROLS {
        let column = (index % 100) as f32;
        let row = (index / 100) as f32;
        app.world_mut().spawn((
            UiNode,
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(column * 24.0, row * 24.0, 20.0, 20.0),
                clip: None,
                z_index: 0,
            },
        ));
    }
    app.update();
    let mut group = c.benchmark_group("input/pointer_route");
    group.throughput(Throughput::Elements(CONTROLS as u64));
    let mut phase = false;
    group.bench_function("move/10000", |b| {
        b.iter(|| {
            phase = !phase;
            app.world_mut()
                .resource_mut::<UiInputQueue>()
                .push(UiInputEvent::PointerMove {
                    window,
                    position: if phase { [12.0, 12.0] } else { [36.0, 12.0] },
                    time_seconds: 0.0,
                });
            app.update();
            black_box(app.world().resource::<UiFocus>().hovered);
        });
    });
    group.finish();
}

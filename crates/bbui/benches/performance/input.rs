use bbui_core::{
    CorePlugin, UiFocus, UiGesturePhase, UiGestureSurface, UiHitBox, UiInputEvent, UiInputQueue,
    UiNode, UiRect, UiRoutedEvents, UiTrackpadGesture, UiTrackpadKind,
};
use bevy::prelude::App;
use criterion::{Criterion, Throughput};
use std::hint::black_box;

pub fn bench(c: &mut Criterion) {
    const CONTROLS: usize = 10_000;
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut first_control = None;
    for index in 0..CONTROLS {
        let column = (index % 100) as f32;
        let row = (index / 100) as f32;
        let entity = app
            .world_mut()
            .spawn((
                UiNode,
                UiHitBox {
                    window: Some(window),
                    rect: UiRect::new(column * 24.0, row * 24.0, 20.0, 20.0),
                    clip: None,
                    z_index: 0,
                },
            ))
            .id();
        if index == 0 {
            first_control = Some(entity);
        }
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

    if let Some(first_control) = first_control {
        app.world_mut()
            .entity_mut(first_control)
            .insert(UiGestureSurface);
    }
    app.update();
    let mut trackpad = c.benchmark_group("input/trackpad_route");
    for (name, position) in [("surface", [12.0, 12.0]), ("pixel_wheel", [36.0, 12.0])] {
        trackpad.bench_function(name, |b| {
            b.iter(|| {
                app.world_mut()
                    .resource_mut::<UiInputQueue>()
                    .push(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
                        window,
                        position,
                        delta: [0.0, 2.0],
                        scale: 1.0,
                        kind: UiTrackpadKind::Scroll,
                        phase: UiGesturePhase::Updated,
                        phased: false,
                    }));
                app.update();
                black_box(app.world().resource::<UiRoutedEvents>().events.len());
            });
        });
    }
    trackpad.finish();
}

//! Measure canvas paint and hit-test cost with many objects outside the clip.
//! Run with `cargo run --release -p bbui_widgets --example canvas_stress`.

use stats_alloc::StatsAlloc;
use std::alloc::System;
use std::hint::black_box;
use std::time::Instant;

use bbui_core::UiRect;
use bbui_render::{ClipRect, DrawItem};
use bbui_style::Theme;
use bbui_widgets::canvas::{CanvasSpatialIndex, paint_canvas_indexed};
use bbui_widgets::{Canvas, CanvasShape};

#[global_allocator]
static ALLOCATOR: StatsAlloc<System> = StatsAlloc::system();

fn main() -> Result<(), &'static str> {
    const OBJECTS: usize = 100_000;
    const FRAMES: usize = 100;
    let mut canvas = Canvas::default();
    for index in 0..OBJECTS {
        let (x, y) = if index < 100 {
            ((index % 10) as f32 * 10.0, (index / 10) as f32 * 10.0)
        } else {
            (1_000.0 + index as f32 * 12.0, 1_000.0)
        };
        canvas
            .add(
                [x, y],
                CanvasShape::Rectangle { size: [8.0, 8.0] },
                [1.0; 4],
            )
            .ok_or("benchmark geometry is invalid")?;
    }
    let viewport = UiRect::new(0.0, 0.0, 100.0, 100.0);
    let clip = ClipRect::from(viewport);
    let theme = Theme::default();
    let index = CanvasSpatialIndex::new(&canvas);
    let visible = index
        .visible_indices(&canvas, viewport, clip, 8.0)
        .ok_or("stress-scene index is invalid")?;
    let mut output = Vec::<DrawItem>::new();
    paint_canvas_indexed(
        &canvas,
        &visible,
        viewport,
        clip,
        0,
        &theme,
        None,
        &mut output,
    );

    let started = Instant::now();
    let mut draw_items = 0;
    for _ in 0..FRAMES {
        output.clear();
        paint_canvas_indexed(
            &canvas,
            &visible,
            viewport,
            clip,
            0,
            &theme,
            None,
            &mut output,
        );
        draw_items = black_box(output.len());
    }
    let paint = started.elapsed();

    let started = Instant::now();
    let mut hits = 0;
    for _ in 0..FRAMES {
        if black_box(canvas.hit_test_indexed(&index, [4.0, 4.0], viewport)).is_some() {
            hits += 1;
        }
    }
    let hit_test = started.elapsed();
    let before = ALLOCATOR.stats();
    for _ in 0..FRAMES {
        output.clear();
        paint_canvas_indexed(
            &canvas,
            &visible,
            viewport,
            clip,
            0,
            &theme,
            None,
            &mut output,
        );
        black_box(canvas.hit_test_indexed(&index, [4.0, 4.0], viewport));
    }
    let delta = ALLOCATOR.stats() - before;
    let allocations = delta.allocations + delta.reallocations;
    let bytes = delta.bytes_allocated;
    println!(
        "canvas objects={OBJECTS} indexed_visible={} frames={FRAMES} draw_items={draw_items} hits={hits} paint_ms/frame={:.3} hit_test_ms/frame={:.3} allocations={allocations} allocated_bytes={bytes} draw_buffer_bytes={}",
        visible.len(),
        paint.as_secs_f64() * 1_000.0 / FRAMES as f64,
        hit_test.as_secs_f64() * 1_000.0 / FRAMES as f64,
        output.capacity() * std::mem::size_of::<DrawItem>()
    );
    Ok(())
}

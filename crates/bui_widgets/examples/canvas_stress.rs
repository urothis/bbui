//! Measure canvas paint and hit-test cost with many objects outside the clip.
//! Run with `cargo run --release -p bui_widgets --example canvas_stress`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use bui_core::UiRect;
use bui_render::{ClipRect, DrawItem};
use bui_style::Theme;
use bui_widgets::canvas::{CanvasSpatialIndex, paint_canvas_indexed};
use bui_widgets::{Canvas, CanvasShape};

struct TrackingAllocator;

static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

// SAFETY: All allocations and deallocations delegate to System unchanged.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && TRACK.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, size) };
        if !new_pointer.is_null() && TRACK.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        new_pointer
    }
}

fn main() {
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
            .expect("benchmark geometry must be valid");
    }
    let viewport = UiRect::new(0.0, 0.0, 100.0, 100.0);
    let clip = ClipRect::from(viewport);
    let theme = Theme::default();
    let index = CanvasSpatialIndex::new(&canvas);
    let visible = index
        .visible_indices(&canvas, viewport, clip, 8.0)
        .expect("valid stress-scene index");
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
    ALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
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
    TRACK.store(false, Ordering::Relaxed);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);
    println!(
        "canvas objects={OBJECTS} indexed_visible={} frames={FRAMES} draw_items={draw_items} hits={hits} paint_ms/frame={:.3} hit_test_ms/frame={:.3} allocations={allocations} allocated_bytes={bytes} draw_buffer_bytes={}",
        visible.len(),
        paint.as_secs_f64() * 1_000.0 / FRAMES as f64,
        hit_test.as_secs_f64() * 1_000.0 / FRAMES as f64,
        output.capacity() * std::mem::size_of::<DrawItem>()
    );
}

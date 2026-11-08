//! CPU-side renderer synchronization measurements; this does not claim to measure GPU work.
//!
//! Run with `cargo run -p bui_render --example render_sync_bench --release`.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::HashSet,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use bevy::{
    asset::Handle,
    ecs::prelude::With,
    prelude::{App, Entity, GlobalZIndex, Image, MinimalPlugins, Node, Overflow},
};
use bui_render::{BuiRenderPlugin, ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive};
use bui_style::Rgba;

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static BASELINE_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

// This example is a standalone measurement binary, so counting all Rust allocations is safe.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwards the exact layout to the system allocator.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwards the exact layout to the system allocator.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwards the original allocation and requested size to the system allocator.
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            let old_size = layout.size();
            if new_size >= old_size {
                update_live(0, new_size - old_size);
            } else {
                update_live(old_size - new_size, 0);
            }
        }
        new_ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwards the allocation and original layout to the system allocator.
        unsafe { System.dealloc(ptr, layout) }
        update_live(layout.size(), 0);
    }
}

fn main() {
    for count in [1_000, 5_000, 10_000] {
        measure_scenario(&format!("rounded_rects_{count}"), count, false);
    }
    measure_scenario("image_grid_4096", 4_096, true);
}

fn measure_scenario(name: &str, count: usize, images: bool) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, BuiRenderPlugin));
    if images {
        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(1, Handle::<Image>::default());
    }
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(build_items(count, images));

    reset_counters();
    let first_started = Instant::now();
    app.update();
    let first_time = first_started.elapsed();
    let (first_allocations, first_bytes, first_peak) = read_counters();
    let entity_count_after_sync = app.world().entities().len();
    let (render_nodes_after_sync, clip_groups_after_sync) = renderer_snapshot(&mut app);

    reset_counters();
    let idle_started = Instant::now();
    for _ in 0..300 {
        app.update();
    }
    let idle_time = idle_started.elapsed();
    let (idle_allocations, idle_bytes, idle_peak) = read_counters();
    let entity_count_after_idle = app.world().entities().len();
    let (render_nodes_after_idle, clip_groups_after_idle) = renderer_snapshot(&mut app);

    let mut changed_items = build_items(count, images);
    if let Some(item) = changed_items.get_mut(count / 2) {
        if let Primitive::RoundedRect { color, .. } = &mut item.primitive {
            *color = Rgba(0.9, 0.2, 0.7, 1.0);
        } else {
            item.z += 1;
        }
    }
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(changed_items);
    reset_counters();
    let changed_started = Instant::now();
    app.update();
    let changed_time = changed_started.elapsed();
    let (changed_allocations, changed_bytes, changed_peak) = read_counters();
    let entity_count_after_change = app.world().entities().len();
    let (render_nodes_after_change, clip_groups_after_change) = renderer_snapshot(&mut app);
    let removed_renderer_nodes = render_nodes_after_sync
        .difference(&render_nodes_after_change)
        .count();
    let added_renderer_nodes = render_nodes_after_change
        .difference(&render_nodes_after_sync)
        .count();

    println!(
        "{name}: first_sync={:?} {first_allocations} alloc/{first_bytes} B allocated, {first_peak} B peak live; idle_300={:?} {:.2} us/frame {idle_allocations} alloc/{idle_bytes} B allocated, {idle_peak} B peak live; change_one={:?} {changed_allocations} alloc/{changed_bytes} B allocated, {changed_peak} B peak live; renderer_nodes={}->{}, clip_groups={}->{}, changed_node_ids=-{removed_renderer_nodes}/+{added_renderer_nodes}; total_entities={entity_count_after_sync}->{entity_count_after_idle}->{entity_count_after_change}",
        first_time,
        idle_time,
        idle_time.as_secs_f64() * 1_000_000.0 / 300.0,
        changed_time,
        render_nodes_after_sync.len(),
        render_nodes_after_change.len(),
        clip_groups_after_sync,
        clip_groups_after_change,
    );

    assert_eq!(entity_count_after_sync, entity_count_after_idle);
    assert_eq!(entity_count_after_sync, entity_count_after_change);
    assert_eq!(render_nodes_after_sync, render_nodes_after_idle);
    assert_eq!(clip_groups_after_sync, 1);
    assert_eq!(clip_groups_after_idle, clip_groups_after_sync);
    assert_eq!(clip_groups_after_change, clip_groups_after_sync);
    assert_eq!(render_nodes_after_sync.len(), count);
    assert_eq!(removed_renderer_nodes, 1);
    assert_eq!(added_renderer_nodes, 1);
}

fn renderer_snapshot(app: &mut App) -> (HashSet<Entity>, usize) {
    let mut rendered_query = app
        .world_mut()
        .query_filtered::<Entity, With<GlobalZIndex>>();
    let rendered_nodes = rendered_query.iter(app.world()).collect();
    let mut node_query = app.world_mut().query::<&Node>();
    let clip_groups = node_query
        .iter(app.world())
        .filter(|node| node.overflow == Overflow::clip())
        .count();
    (rendered_nodes, clip_groups)
}

fn build_items(count: usize, images: bool) -> Vec<DrawItem> {
    const COLUMNS: usize = 64;
    let mut items = Vec::with_capacity(count);
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 1024.0,
        height: 768.0,
    };
    for index in 0..count {
        let x = (index % COLUMNS) as f32 * 16.0;
        let y = (index / COLUMNS) as f32 * 12.0;
        let rect = ClipRect {
            x,
            y,
            width: 14.0,
            height: 10.0,
        };
        let primitive = if images {
            Primitive::Image { rect, asset_id: 1 }
        } else {
            Primitive::RoundedRect {
                rect,
                radius: 2.0,
                color: Rgba(0.18, 0.42, 0.68, 1.0),
            }
        };
        items.push(DrawItem {
            clip,
            z: index as i32,
            primitive,
        });
    }
    items
}

fn reset_counters() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    let live = LIVE_BYTES.load(Ordering::Relaxed);
    BASELINE_LIVE_BYTES.store(live, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(live, Ordering::Relaxed);
}

fn read_counters() -> (usize, usize, usize) {
    (
        ALLOCATIONS.load(Ordering::Relaxed),
        ALLOCATED_BYTES.load(Ordering::Relaxed),
        PEAK_LIVE_BYTES
            .load(Ordering::Relaxed)
            .saturating_sub(BASELINE_LIVE_BYTES.load(Ordering::Relaxed)),
    )
}

fn record_alloc(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(bytes, Ordering::Relaxed);
    update_live(0, bytes);
}

fn update_live(subtract: usize, add: usize) {
    let mut live = LIVE_BYTES.load(Ordering::Relaxed);
    loop {
        let next = live.saturating_sub(subtract).saturating_add(add);
        match LIVE_BYTES.compare_exchange_weak(live, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => {
                let mut peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
                while next > peak {
                    match PEAK_LIVE_BYTES.compare_exchange_weak(
                        peak,
                        next,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => peak = actual,
                    }
                }
                return;
            }
            Err(actual) => live = actual,
        }
    }
}

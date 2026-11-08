//! Compare idle and single-input layout work for large Flex, ResponsiveGrid,
//! and nested-scroll roots. Run with:
//!
//! ```text
//! cargo run --release -p bui_layout --example layout_stress_bench --offline
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bevy_app::App;
use bui_core::{CorePlugin, UiNode, set_parent};
use bui_layout::{
    LayoutPlugin, Rect, UiFlexDirection, UiFlexItem, UiIntrinsicSize, UiLayoutMode, UiLayoutNode,
    UiLayoutRoot, UiLayoutStats, UiLength, UiScroll, UiSize,
};

struct TrackingAllocator;

static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

// SAFETY: Every operation delegates to the system allocator with the original
// layout and pointer. Tracking only updates atomics around those operations.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() && TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(new_size);
        }
        new_pointer
    }
}

fn record_allocation(bytes: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

#[derive(Clone, Copy)]
enum DirtyInput {
    Intrinsic(bevy_ecs::entity::Entity),
    Scroll(bevy_ecs::entity::Entity),
}

struct Scene {
    app: App,
    dirty_input: DirtyInput,
    node_count: usize,
    name: &'static str,
}

fn main() {
    const FRAMES: usize = 20;
    for node_count in [1_000, 10_000] {
        for name in ["flex", "responsive-grid", "nested-scroll"] {
            let mut scene = make_scene(name, node_count);
            scene.app.update(); // Prime Bevy schedules and establish layout snapshots.
            scene.app.update(); // Keep warmup changes outside the measured intervals.

            let idle = measure_time(&mut scene, FRAMES, false);
            let dirty = measure_time(&mut scene, FRAMES, true);
            let idle_allocations = measure_allocations(&mut scene, FRAMES, false);
            let dirty_allocations = measure_allocations(&mut scene, FRAMES, true);

            println!("{} nodes={} frames={FRAMES}", scene.name, scene.node_count);
            print_phase("idle", idle, idle_allocations, FRAMES);
            print_phase("one input change/frame", dirty, dirty_allocations, FRAMES);
        }
    }
}

#[derive(Clone, Copy)]
struct PhaseStats {
    elapsed: Duration,
    roots_processed: u64,
    nodes_processed: u64,
}

#[derive(Clone, Copy)]
struct AllocationStats {
    allocations: u64,
    bytes: u64,
}

fn make_scene(name: &'static str, node_count: usize) -> Scene {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root_style = match name {
        "flex" => UiLayoutNode {
            mode: UiLayoutMode::Flex {
                direction: UiFlexDirection::Row,
                wrap: true,
            },
            gap: 3.0,
            ..UiLayoutNode::default()
        },
        "responsive-grid" => UiLayoutNode::responsive_grid(52, 16, 3.0),
        "nested-scroll" => UiLayoutNode {
            mode: UiLayoutMode::Column,
            clip_contents: true,
            ..UiLayoutNode::default()
        },
        _ => unreachable!("benchmark scenario is known"),
    };
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
            root_style,
        ))
        .id();
    if name == "nested-scroll" {
        app.world_mut().entity_mut(root).insert(UiScroll::new());
    }

    let dirty_input = match name {
        "flex" => spawn_flex_children(&mut app, root, node_count.saturating_sub(1)),
        "responsive-grid" => spawn_grid_children(&mut app, root, node_count.saturating_sub(1)),
        "nested-scroll" => spawn_nested_scroll(&mut app, root, node_count),
        _ => unreachable!("benchmark scenario is known"),
    };

    Scene {
        app,
        dirty_input,
        node_count,
        name,
    }
}

fn spawn_flex_children(app: &mut App, root: bevy_ecs::entity::Entity, count: usize) -> DirtyInput {
    let mut first = None;
    for _ in 0..count {
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([44.0, 22.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
                UiFlexItem {
                    basis: Some(UiLength::Px(44.0)),
                    grow: 0.25,
                    ..UiFlexItem::default()
                },
            ))
            .id();
        set_parent(app.world_mut(), child, root).expect("valid Flex benchmark hierarchy");
        first.get_or_insert(child);
    }
    DirtyInput::Intrinsic(first.expect("node_count is greater than one"))
}

fn spawn_grid_children(app: &mut App, root: bevy_ecs::entity::Entity, count: usize) -> DirtyInput {
    let mut first = None;
    for _ in 0..count {
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([48.0, 23.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
            ))
            .id();
        set_parent(app.world_mut(), child, root).expect("valid grid benchmark hierarchy");
        first.get_or_insert(child);
    }
    DirtyInput::Intrinsic(first.expect("node_count is greater than one"))
}

fn spawn_nested_scroll(
    app: &mut App,
    root: bevy_ecs::entity::Entity,
    node_count: usize,
) -> DirtyInput {
    const DEPTH: usize = 6;
    let mut parent = root;
    let mut deepest = root;
    for _ in 1..DEPTH {
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode {
                    size: UiSize {
                        width: UiLength::Percent(1.0),
                        height: UiLength::Px(600.0),
                    },
                    clip_contents: true,
                    ..UiLayoutNode::column(1.0)
                },
                UiScroll::new(),
            ))
            .id();
        set_parent(app.world_mut(), child, parent).expect("valid nested-scroll hierarchy");
        parent = child;
        deepest = child;
    }

    // Count the root and the intermediate scroll containers in the requested
    // total. Keep one leaf even if a future caller asks for a tiny scene.
    let leaf_count = node_count.saturating_sub(DEPTH).max(1);
    for _ in 0..leaf_count {
        let leaf = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([80.0, 18.0]),
                UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
            ))
            .id();
        set_parent(app.world_mut(), leaf, deepest).expect("valid nested-scroll leaf");
    }
    DirtyInput::Scroll(deepest)
}

fn measure_time(scene: &mut Scene, frames: usize, dirty: bool) -> PhaseStats {
    let before = *scene.app.world().resource::<UiLayoutStats>();
    let start = Instant::now();
    for frame in 0..frames {
        if dirty {
            update_dirty_input(&mut scene.app, scene.dirty_input, frame);
        }
        scene.app.update();
        black_box(
            scene
                .app
                .world()
                .resource::<UiLayoutStats>()
                .nodes_processed,
        );
    }
    let elapsed = start.elapsed();
    let after = *scene.app.world().resource::<UiLayoutStats>();
    PhaseStats {
        elapsed,
        roots_processed: after.total_roots_processed - before.total_roots_processed,
        nodes_processed: after.total_nodes_processed - before.total_nodes_processed,
    }
}

fn measure_allocations(scene: &mut Scene, frames: usize, dirty: bool) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
    for frame in 0..frames {
        if dirty {
            update_dirty_input(&mut scene.app, scene.dirty_input, frame + frames);
        }
        scene.app.update();
    }
    TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    AllocationStats {
        allocations: ALLOCATION_COUNT.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

fn update_dirty_input(app: &mut App, input: DirtyInput, frame: usize) {
    match input {
        DirtyInput::Intrinsic(entity) => {
            let width = if frame.is_multiple_of(2) { 45.0 } else { 44.0 };
            app.world_mut()
                .entity_mut(entity)
                .insert(UiIntrinsicSize([width, 22.0]));
        }
        DirtyInput::Scroll(entity) => {
            let offset = if frame.is_multiple_of(2) { 16.0 } else { 24.0 };
            app.world_mut()
                .get_mut::<UiScroll>(entity)
                .expect("scroll benchmark target")
                .offset[1] = offset;
        }
    }
}

fn print_phase(label: &str, time: PhaseStats, allocations: AllocationStats, frames: usize) {
    println!(
        "  {label}: {:.2} us/frame, {:.1} allocs/frame, {:.1} KB/frame, roots={}, nodes={}",
        time.elapsed.as_secs_f64() * 1_000_000.0 / frames as f64,
        allocations.allocations as f64 / frames as f64,
        allocations.bytes as f64 / frames as f64 / 1024.0,
        time.roots_processed,
        time.nodes_processed,
    );
}

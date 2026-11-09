//! Compare idle and single-input layout work for large Flex, ResponsiveGrid,
//! and nested-scroll roots. Run with:
//!
//! ```text
//! cargo run --release -p bui_layout --example layout_stress_bench --offline
//! ```

use stats_alloc::{Stats, StatsAlloc};
use std::alloc::System;
use std::hint::black_box;
use std::time::{Duration, Instant};

use bevy_app::App;
use bui_core::{CorePlugin, UiNode, set_parent};
use bui_layout::{
    LayoutPlugin, Rect, UiFlexDirection, UiFlexItem, UiIntrinsicSize, UiLayoutMode, UiLayoutNode,
    UiLayoutRoot, UiLayoutStats, UiLength, UiScroll, UiSize,
};

#[global_allocator]
static ALLOCATOR: StatsAlloc<System> = StatsAlloc::system();

fn allocation_delta(before: Stats) -> AllocationStats {
    let delta = ALLOCATOR.stats() - before;
    AllocationStats {
        allocations: (delta.allocations + delta.reallocations) as u64,
        bytes: delta.bytes_allocated as u64,
    }
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

fn main() -> Result<(), String> {
    const FRAMES: usize = 20;
    for node_count in [1_000, 10_000] {
        for name in ["flex", "responsive-grid", "nested-scroll"] {
            let mut scene = make_scene(name, node_count)?;
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
    Ok(())
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

fn make_scene(name: &'static str, node_count: usize) -> Result<Scene, String> {
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
        _ => return Err(format!("unknown layout benchmark scenario: {name}")),
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
        "flex" => spawn_flex_children(&mut app, root, node_count.saturating_sub(1))?,
        "responsive-grid" => spawn_grid_children(&mut app, root, node_count.saturating_sub(1))?,
        "nested-scroll" => spawn_nested_scroll(&mut app, root, node_count)?,
        _ => return Err(format!("unknown layout benchmark scenario: {name}")),
    };

    Ok(Scene {
        app,
        dirty_input,
        node_count,
        name,
    })
}

fn spawn_flex_children(
    app: &mut App,
    root: bevy_ecs::entity::Entity,
    count: usize,
) -> Result<DirtyInput, String> {
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
        set_parent(app.world_mut(), child, root)
            .map_err(|error| format!("flex hierarchy: {error:?}"))?;
        first.get_or_insert(child);
    }
    first
        .map(DirtyInput::Intrinsic)
        .ok_or_else(|| "flex benchmark has no children".to_owned())
}

fn spawn_grid_children(
    app: &mut App,
    root: bevy_ecs::entity::Entity,
    count: usize,
) -> Result<DirtyInput, String> {
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
        set_parent(app.world_mut(), child, root)
            .map_err(|error| format!("grid hierarchy: {error:?}"))?;
        first.get_or_insert(child);
    }
    first
        .map(DirtyInput::Intrinsic)
        .ok_or_else(|| "grid benchmark has no children".to_owned())
}

fn spawn_nested_scroll(
    app: &mut App,
    root: bevy_ecs::entity::Entity,
    node_count: usize,
) -> Result<DirtyInput, String> {
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
        set_parent(app.world_mut(), child, parent)
            .map_err(|error| format!("scroll hierarchy: {error:?}"))?;
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
        set_parent(app.world_mut(), leaf, deepest)
            .map_err(|error| format!("scroll leaf: {error:?}"))?;
    }
    Ok(DirtyInput::Scroll(deepest))
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
    let before = ALLOCATOR.stats();
    for frame in 0..frames {
        if dirty {
            update_dirty_input(&mut scene.app, scene.dirty_input, frame + frames);
        }
        scene.app.update();
    }
    allocation_delta(before)
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
            if let Some(mut scroll) = app.world_mut().get_mut::<UiScroll>(entity) {
                scroll.offset[1] = offset;
            }
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

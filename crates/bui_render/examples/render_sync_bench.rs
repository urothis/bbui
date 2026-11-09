//! CPU-side renderer synchronization measurements; this does not claim to measure GPU work.
//!
//! Run with `cargo run -p bui_render --example render_sync_bench --release`.

use stats_alloc::{Stats, StatsAlloc};
use std::{alloc::System, collections::HashSet, time::Instant};

use bevy::{
    asset::Handle,
    ecs::prelude::With,
    prelude::{App, Entity, GlobalZIndex, Image, MinimalPlugins, Node, Overflow},
};
use bui_render::{BuiRenderPlugin, ClipRect, DisplayList, DrawItem, ImageRegistry, Primitive};
use bui_style::Rgba;

#[global_allocator]
static GLOBAL: StatsAlloc<System> = StatsAlloc::system();

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

    let first_before = GLOBAL.stats();
    let first_started = Instant::now();
    app.update();
    let first_time = first_started.elapsed();
    let (first_allocations, first_bytes, first_live_delta) = read_counters(first_before);
    let entity_count_after_sync = app.world().entities().len();
    let (render_nodes_after_sync, clip_groups_after_sync) = renderer_snapshot(&mut app);

    let idle_before = GLOBAL.stats();
    let idle_started = Instant::now();
    for _ in 0..300 {
        app.update();
    }
    let idle_time = idle_started.elapsed();
    let (idle_allocations, idle_bytes, idle_live_delta) = read_counters(idle_before);
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
    let changed_before = GLOBAL.stats();
    let changed_started = Instant::now();
    app.update();
    let changed_time = changed_started.elapsed();
    let (changed_allocations, changed_bytes, changed_live_delta) = read_counters(changed_before);
    let entity_count_after_change = app.world().entities().len();
    let (render_nodes_after_change, clip_groups_after_change) = renderer_snapshot(&mut app);
    let removed_renderer_nodes = render_nodes_after_sync
        .difference(&render_nodes_after_change)
        .count();
    let added_renderer_nodes = render_nodes_after_change
        .difference(&render_nodes_after_sync)
        .count();

    println!(
        "{name}: first_sync={:?} {first_allocations} alloc/{first_bytes} B allocated, {first_live_delta} B live delta; idle_300={:?} {:.2} us/frame {idle_allocations} alloc/{idle_bytes} B allocated, {idle_live_delta} B live delta; change_one={:?} {changed_allocations} alloc/{changed_bytes} B allocated, {changed_live_delta} B live delta; renderer_nodes={}->{}, clip_groups={}->{}, changed_node_ids=-{removed_renderer_nodes}/+{added_renderer_nodes}; total_entities={entity_count_after_sync}->{entity_count_after_idle}->{entity_count_after_change}",
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

fn read_counters(before: Stats) -> (usize, usize, i128) {
    let after = GLOBAL.stats();
    let delta = after - before;
    let live = |stats: Stats| {
        stats.bytes_allocated as i128 - stats.bytes_deallocated as i128
            + stats.bytes_reallocated as i128
    };
    (
        delta.allocations + delta.reallocations,
        delta.bytes_allocated,
        live(after) - live(before),
    )
}

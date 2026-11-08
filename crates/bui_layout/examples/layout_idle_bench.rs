//! Run with `cargo run --release -p bui_layout --example layout_idle_bench`.

use std::time::Instant;

use bevy_app::App;
use bui_core::{CorePlugin, UiNode, set_parent};
use bui_layout::{LayoutPlugin, Rect, UiIntrinsicSize, UiLayoutNode, UiLayoutRoot, UiLayoutStats};

fn main() {
    const NODE_COUNT: usize = 2_000;
    const FRAMES: usize = 120;

    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
            UiLayoutNode::column(2.0),
        ))
        .id();
    let mut leaves = Vec::with_capacity(NODE_COUNT);
    for _ in 0..NODE_COUNT {
        let leaf = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([20.0, 12.0]),
                UiLayoutNode::leaf(bui_layout::UiLength::Auto, bui_layout::UiLength::Auto),
            ))
            .id();
        set_parent(app.world_mut(), leaf, root).expect("valid benchmark hierarchy");
        leaves.push(leaf);
    }

    app.update(); // Include archetype creation and first-layout costs in warmup.
    let warm_stats = *app.world().resource::<UiLayoutStats>();

    let idle_start = Instant::now();
    for _ in 0..FRAMES {
        app.update();
    }
    let idle_elapsed = idle_start.elapsed();
    let idle_stats = *app.world().resource::<UiLayoutStats>();
    let idle_nodes = idle_stats.total_nodes_processed - warm_stats.total_nodes_processed;

    let change_start = Instant::now();
    for frame in 0..FRAMES {
        let width = if frame % 2 == 0 { 21.0 } else { 20.0 };
        app.world_mut()
            .entity_mut(leaves[0])
            .insert(UiIntrinsicSize([width, 12.0]));
        app.update();
    }
    let change_elapsed = change_start.elapsed();
    let changed_stats = *app.world().resource::<UiLayoutStats>();
    let changed_nodes = changed_stats.total_nodes_processed - idle_stats.total_nodes_processed;

    println!("nodes={NODE_COUNT} frames={FRAMES}");
    println!(
        "idle: {:?} total, {:.2} us/frame, {idle_nodes} layout nodes visited",
        idle_elapsed,
        idle_elapsed.as_secs_f64() * 1_000_000.0 / FRAMES as f64,
    );
    println!(
        "one intrinsic change/frame: {:?} total, {:.2} us/frame, {changed_nodes} layout nodes visited",
        change_elapsed,
        change_elapsed.as_secs_f64() * 1_000_000.0 / FRAMES as f64,
    );
}

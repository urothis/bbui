//! Measure long virtualized collections, node graphs, and style animations.
//!
//! Run with `cargo run --release -p bui_widgets --example widget_model_stress_bench --locked`.
//! The app warms each model before recording idle and changed updates. Allocation traffic and
//! peak live Rust heap are measured separately from scene construction and reported per frame.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bevy_app::App;
use bevy_ecs::prelude::{Component, Entity, With};
use bevy_time::Time;
use bui_core::{CorePlugin, UiEnabled, UiNode, UiVisible, set_parent};
use bui_layout::{LayoutPlugin, Rect, UiLayoutMode, UiLayoutNode, UiLayoutRoot, UiLength, UiSize};
use bui_render::DisplayList;
use bui_style::{
    StylePlugin, UiStyleProperty, UiStyleState, UiTransitionSpec, UiTransitions, set_ui_style_state,
};
use bui_text::TextPlugin;
use bui_widgets::{
    Checkbox, DataTable, ListItem, ListModel, NodeGraph, NodeSocket, SocketType, TableColumn,
    TableRow, WidgetPlugin,
    declarative::{CollectionItemControl, DataTableCellControl, NodeGraphNodeControl},
};

const VIEWPORT_WIDTH: f32 = 1280.0;
const VIEWPORT_HEIGHT: f32 = 720.0;
const FRAMES: usize = 30;
const DATA_ROWS: usize = 10_000;

struct TrackingAllocator;

static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicU64 = AtomicU64::new(0);
static PEAK_LIVE_BYTES: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

// SAFETY: Every allocation operation delegates to the system allocator unchanged. The counters
// observe allocation sizes and current/peak live bytes without retaining pointers.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_alloc(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            record_alloc(layout.size());
        }
        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
                ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
                ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
            }
            if new_size >= layout.size() {
                update_live(new_size - layout.size(), 0);
            } else {
                update_live(0, layout.size() - new_size);
            }
        }
        new_pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        update_live(0, layout.size());
    }
}

fn record_alloc(bytes: usize) {
    if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
        ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    }
    update_live(bytes, 0);
}

fn update_live(add: usize, remove: usize) {
    let live = if add > 0 {
        LIVE_BYTES.fetch_add(add as u64, Ordering::Relaxed) + add as u64
    } else if remove > 0 {
        LIVE_BYTES.fetch_sub(remove as u64, Ordering::Relaxed) - remove as u64
    } else {
        LIVE_BYTES.load(Ordering::Relaxed)
    };
    let mut peak = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    while live > peak {
        match PEAK_LIVE_BYTES.compare_exchange_weak(
            peak,
            live,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(actual) => peak = actual,
        }
    }
}

struct Scene {
    name: &'static str,
    app: App,
    owner: Entity,
    workload: Workload,
}

enum Workload {
    List { rows: usize },
    Table { rows: usize, columns: usize },
    Graph { nodes: usize, links: usize },
    Animation { targets: Vec<Entity> },
}

#[derive(Clone, Copy)]
struct Measure {
    micros_per_frame: f64,
    allocs_per_frame: f64,
    kb_per_frame: f64,
    peak_live_kb: f64,
}

fn main() {
    bench_scene(make_list_scene(DATA_ROWS));
    bench_scene(make_table_scene(DATA_ROWS, 8));
    bench_scene(make_graph_scene(1_000, 4));
    bench_scene(make_animation_scene(1_000));
}

fn bench_scene(mut scene: Scene) {
    scene.app.update();
    scene.app.update();
    scene.app.update();

    let entity_count = scene.app.world().entities().len();
    let materialized = materialized_items(&mut scene);
    let draw_items = scene
        .app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .len();
    let idle = measure(&mut scene, FRAMES, false);

    if matches!(scene.workload, Workload::Animation { .. }) {
        begin_animation(&mut scene);
    }
    let changed = measure(&mut scene, FRAMES, true);
    let live_heap_kb = LIVE_BYTES.load(Ordering::Relaxed) as f64 / 1024.0;

    println!(
        "{} model_items={} entities={} materialized={} draw_items={} idle={:.2}us {:.1}allocs/{:.1}KB peak+{:.1}KB changed_or_animated={:.2}us {:.1}allocs/{:.1}KB peak+{:.1}KB live_heap={:.1}KB",
        scene.name,
        model_items(&scene),
        entity_count,
        materialized,
        draw_items,
        idle.micros_per_frame,
        idle.allocs_per_frame,
        idle.kb_per_frame,
        idle.peak_live_kb,
        changed.micros_per_frame,
        changed.allocs_per_frame,
        changed.kb_per_frame,
        changed.peak_live_kb,
        live_heap_kb,
    );
}

fn make_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        StylePlugin,
        LayoutPlugin,
        TextPlugin,
        WidgetPlugin,
    ));
    app.insert_resource(Time::<()>::default());

    let window = app.world_mut().spawn_empty().id();
    let owner = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiLayoutRoot::new(
                window,
                Rect::new(0.0, 0.0, VIEWPORT_WIDTH, VIEWPORT_HEIGHT),
                1.0,
            ),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                size: UiSize {
                    width: UiLength::Px(VIEWPORT_WIDTH),
                    height: UiLength::Px(VIEWPORT_HEIGHT),
                },
                clip_contents: true,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    (app, owner)
}

fn make_list_scene(rows: usize) -> Scene {
    let (mut app, owner) = make_app();
    let items = (0..rows)
        .map(|index| ListItem {
            id: index as u64,
            value: format!("List row {index:05}"),
            enabled: true,
        })
        .collect();
    app.world_mut().entity_mut(owner).insert(ListModel {
        items,
        ..ListModel::default()
    });
    Scene {
        name: "long-list",
        app,
        owner,
        workload: Workload::List { rows },
    }
}

fn make_table_scene(rows: usize, column_count: usize) -> Scene {
    let (mut app, owner) = make_app();
    let columns = (0..column_count)
        .map(|column| TableColumn::new(format!("Column {column}"), 140.0))
        .collect();
    let rows_data = (0..rows)
        .map(|row| TableRow {
            id: row as u64,
            cells: (0..column_count)
                .map(|column| format!("row-{row:05}-column-{column}"))
                .collect(),
        })
        .collect();
    app.world_mut().entity_mut(owner).insert(DataTable {
        columns,
        rows: rows_data,
        ..DataTable::default()
    });
    Scene {
        name: "long-table",
        app,
        owner,
        workload: Workload::Table {
            rows,
            columns: column_count,
        },
    }
}

fn make_graph_scene(nodes: usize, inputs_per_node: usize) -> Scene {
    let (mut app, owner) = make_app();
    let mut graph = NodeGraph::default();
    let sockets = std::iter::once(NodeSocket::output("out", SocketType::Float))
        .chain(
            (0..inputs_per_node)
                .map(|index| NodeSocket::input(format!("in-{index}"), SocketType::Float)),
        )
        .collect::<Vec<_>>();
    let mut ids = Vec::with_capacity(nodes);
    for index in 0..nodes {
        let column = index % 7;
        let row = index / 7;
        ids.push(graph.add_node(
            format!("Node {index:04}"),
            [column as f32 * 210.0, row as f32 * 150.0],
            sockets.clone(),
        ));
    }
    for target in inputs_per_node..nodes {
        for offset in 1..=inputs_per_node {
            graph
                .create_link(ids[target - offset], 0, ids[target], offset)
                .expect("typed graph links should validate");
        }
    }
    let links = graph.links.len();
    app.world_mut().entity_mut(owner).insert(graph);
    Scene {
        name: "node-graph",
        app,
        owner,
        workload: Workload::Graph { nodes, links },
    }
}

fn make_animation_scene(nodes: usize) -> Scene {
    let (mut app, owner) = make_app();
    let mut targets = Vec::with_capacity(nodes);
    let mut transitions = UiTransitions::default();
    transitions.set(
        UiStyleProperty::Background,
        UiTransitionSpec::new(Duration::from_millis(1_200)),
    );
    for index in 0..nodes {
        let target = app
            .world_mut()
            .spawn((
                UiNode,
                UiEnabled(true),
                UiVisible(true),
                UiLayoutNode::leaf(UiLength::Px(48.0), UiLength::Px(20.0)),
                transitions.clone(),
                UiStyleState::default(),
                Checkbox::new(format!("Animated control {index}"), false),
            ))
            .id();
        set_parent(app.world_mut(), target, owner).expect("valid animated child");
        targets.push(target);
    }
    Scene {
        name: "style-animation",
        app,
        owner,
        workload: Workload::Animation { targets },
    }
}

fn begin_animation(scene: &mut Scene) {
    let Workload::Animation { targets } = &scene.workload else {
        return;
    };
    for target in targets.iter().copied() {
        set_ui_style_state(scene.app.world_mut(), target, UiStyleState::SELECTED, true);
    }
    scene.app.update();
}

fn measure(scene: &mut Scene, frames: usize, changed: bool) -> Measure {
    let start = Instant::now();
    for frame in 0..frames {
        if changed {
            step_workload(scene, frame);
        }
        scene.app.update();
        black_box(
            scene
                .app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .len(),
        );
    }
    let micros_per_frame = start.elapsed().as_secs_f64() * 1_000_000.0 / frames as f64;

    let baseline_live = LIVE_BYTES.load(Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(baseline_live, Ordering::Relaxed);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
    for frame in 0..frames {
        if changed {
            step_workload(scene, frame);
        }
        scene.app.update();
        black_box(
            scene
                .app
                .world()
                .resource::<DisplayList>()
                .widget_items()
                .len(),
        );
    }
    TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    let count = ALLOCATION_COUNT.load(Ordering::Relaxed);
    let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let peak = PEAK_LIVE_BYTES
        .load(Ordering::Relaxed)
        .saturating_sub(baseline_live);
    Measure {
        micros_per_frame,
        allocs_per_frame: count as f64 / frames as f64,
        kb_per_frame: bytes as f64 / frames as f64 / 1024.0,
        peak_live_kb: peak as f64 / 1024.0,
    }
}

fn step_workload(scene: &mut Scene, frame: usize) {
    match &scene.workload {
        Workload::List { rows } => {
            if let Some(mut model) = scene
                .app
                .world_mut()
                .get_mut::<ListModel<String>>(scene.owner)
            {
                model.scroll_offset =
                    (frame as f32 * model.row_height) % (*rows as f32 * model.row_height);
            }
        }
        Workload::Table { rows, .. } => {
            if let Some(mut model) = scene.app.world_mut().get_mut::<DataTable>(scene.owner) {
                model.scroll_rows = (frame * 2) % rows.saturating_sub(32).max(1);
            }
        }
        Workload::Graph { .. } => {
            if let Some(mut graph) = scene.app.world_mut().get_mut::<NodeGraph>(scene.owner) {
                graph.pan_by([0.25, -0.15]);
            }
        }
        Workload::Animation { .. } => {
            scene
                .app
                .world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
        }
    }
}

fn materialized_items(scene: &mut Scene) -> usize {
    match &scene.workload {
        Workload::List { .. } => count_component::<CollectionItemControl>(&mut scene.app),
        Workload::Table { .. } => count_component::<DataTableCellControl>(&mut scene.app),
        Workload::Graph { .. } => count_component::<NodeGraphNodeControl>(&mut scene.app),
        Workload::Animation { .. } => count_component::<UiStyleState>(&mut scene.app),
    }
}

fn model_items(scene: &Scene) -> usize {
    match &scene.workload {
        Workload::List { rows } => *rows,
        Workload::Table { rows, columns } => rows.saturating_mul(*columns),
        Workload::Graph { nodes, links } => nodes.saturating_add(*links),
        Workload::Animation { targets } => targets.len(),
    }
}

fn count_component<T: Component>(app: &mut App) -> usize {
    let mut query = app.world_mut().query_filtered::<Entity, With<T>>();
    query.iter(app.world()).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bui_style::UiStyleTransitionState;
    use bui_widgets::NodeGraph;

    #[test]
    fn long_list_and_table_keep_model_data_and_materialize_only_viewport_rows() {
        let mut list = make_list_scene(10_000);
        list.app.update();
        list.app.update();
        assert_eq!(
            list.app
                .world()
                .get::<ListModel<String>>(list.owner)
                .unwrap()
                .items
                .len(),
            10_000
        );
        let list_rows = count_component::<CollectionItemControl>(&mut list.app);
        assert!(
            list_rows > 0 && list_rows < 50,
            "visible list rows={list_rows}"
        );

        let mut table = make_table_scene(10_000, 8);
        table.app.update();
        table.app.update();
        let model = table.app.world().get::<DataTable>(table.owner).unwrap();
        assert_eq!(model.rows.len(), 10_000);
        assert_eq!(model.columns.len(), 8);
        let cells = count_component::<DataTableCellControl>(&mut table.app);
        assert!(cells > 0 && cells < 400, "visible table cells={cells}");
    }

    #[test]
    fn graph_stress_keeps_thousands_of_nodes_and_links_and_virtualizes_controls() {
        let mut graph_scene = make_graph_scene(1_000, 4);
        graph_scene.app.update();
        graph_scene.app.update();
        let graph = graph_scene
            .app
            .world()
            .get::<NodeGraph>(graph_scene.owner)
            .unwrap();
        assert_eq!(graph.nodes.len(), 1_000);
        assert_eq!(graph.links.len(), 3_984);
        let visible_nodes = count_component::<NodeGraphNodeControl>(&mut graph_scene.app);
        assert!(visible_nodes > 0 && visible_nodes < 1_000);
    }

    #[test]
    fn active_style_animation_advances_then_returns_to_idle() {
        let mut scene = make_animation_scene(64);
        scene.app.update();
        begin_animation(&mut scene);
        assert!(count_component::<UiStyleTransitionState>(&mut scene.app) > 0);
        for _ in 0..100 {
            scene
                .app
                .world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
            scene.app.update();
        }
        assert_eq!(count_component::<UiStyleTransitionState>(&mut scene.app), 0);
    }
}

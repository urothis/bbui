//! Measure long virtualized collections, node graphs, and style animations.
//!
//! Run with `cargo run --release -p bbui_widgets --example widget_model_stress_bench --locked`.
//! The app warms each model before recording idle and changed updates. Allocation traffic and
//! sampled live Rust heap are measured separately from scene construction and reported per frame.

use stats_alloc::{Stats, StatsAlloc};
use std::alloc::System;
use std::hint::black_box;
use std::time::{Duration, Instant};

use bbui_core::{CorePlugin, UiEnabled, UiNode, UiVisible, set_parent};
use bbui_layout::{LayoutPlugin, Rect, UiLayoutMode, UiLayoutNode, UiLayoutRoot, UiLength, UiSize};
use bbui_render::DisplayList;
use bbui_style::{
    StylePlugin, UiAnimationSpec, UiAutoAnimations, UiStyleProperty, UiStyleState,
    set_ui_style_state,
};
use bbui_text::TextPlugin;
use bbui_widgets::{
    Checkbox, DataTable, ListItem, ListModel, NodeGraph, NodeSocket, SocketType, TableColumn,
    TableRow, WidgetPlugin,
    actions::{pan_node_graph, set_data_table_viewport},
    declarative::{CollectionItemControl, DataTableCellControl, NodeGraphNodeControl},
};
use bevy_app::App;
use bevy_ecs::prelude::{Component, Entity, With};
use bevy_time::Time;

const VIEWPORT_WIDTH: f32 = 1280.0;
const VIEWPORT_HEIGHT: f32 = 720.0;
const FRAMES: usize = 30;
const DATA_ROWS: usize = 10_000;

#[global_allocator]
static ALLOCATOR: StatsAlloc<System> = StatsAlloc::system();

fn live_bytes(stats: Stats) -> usize {
    let live = stats.bytes_allocated as i128 - stats.bytes_deallocated as i128
        + stats.bytes_reallocated as i128;
    usize::try_from(live.max(0)).unwrap_or(usize::MAX)
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

fn main() -> Result<(), String> {
    bench_scene(make_list_scene(DATA_ROWS));
    bench_scene(make_table_scene(DATA_ROWS, 8));
    bench_scene(make_graph_scene(1_000, 4)?);
    bench_scene(make_animation_scene(1_000)?);
    Ok(())
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
    let live_heap_kb = live_bytes(ALLOCATOR.stats()) as f64 / 1024.0;

    println!(
        "{} model_items={} entities={} materialized={} draw_items={} idle={:.2}us {:.1}allocs/{:.1}KB sampled_peak+{:.1}KB changed_or_animated={:.2}us {:.1}allocs/{:.1}KB sampled_peak+{:.1}KB live_heap={:.1}KB",
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

fn make_graph_scene(nodes: usize, inputs_per_node: usize) -> Result<Scene, String> {
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
                .map_err(|error| format!("graph benchmark link: {error:?}"))?;
        }
    }
    let links = graph.links.len();
    app.world_mut().entity_mut(owner).insert(graph);
    Ok(Scene {
        name: "node-graph",
        app,
        owner,
        workload: Workload::Graph { nodes, links },
    })
}

fn make_animation_scene(nodes: usize) -> Result<Scene, String> {
    let (mut app, owner) = make_app();
    let mut targets = Vec::with_capacity(nodes);
    let mut transitions = UiAutoAnimations::default();
    transitions.set(
        UiStyleProperty::Background,
        UiAnimationSpec::new(Duration::from_millis(1_200)),
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
        set_parent(app.world_mut(), target, owner)
            .map_err(|error| format!("animated benchmark child: {error:?}"))?;
        targets.push(target);
    }
    Ok(Scene {
        name: "style-animation",
        app,
        owner,
        workload: Workload::Animation { targets },
    })
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
        black_box(scene.app.world().resource::<DisplayList>().item_count());
    }
    let micros_per_frame = start.elapsed().as_secs_f64() * 1_000_000.0 / frames as f64;

    let before = ALLOCATOR.stats();
    let baseline_live = live_bytes(before);
    let mut peak_live = baseline_live;
    for frame in 0..frames {
        if changed {
            step_workload(scene, frame);
        }
        scene.app.update();
        peak_live = peak_live.max(live_bytes(ALLOCATOR.stats()));
        black_box(scene.app.world().resource::<DisplayList>().item_count());
    }
    let delta = ALLOCATOR.stats() - before;
    let count = delta.allocations + delta.reallocations;
    let bytes = delta.bytes_allocated;
    let peak = peak_live.saturating_sub(baseline_live);
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
            let scroll_x = scene
                .app
                .world()
                .get::<DataTable>(scene.owner)
                .map_or(0.0, |model| model.scroll_x);
            set_data_table_viewport(
                scene.app.world_mut(),
                scene.owner,
                (frame * 2) % rows.saturating_sub(32).max(1),
                scroll_x,
            );
        }
        Workload::Graph { .. } => {
            let pan = scene
                .app
                .world()
                .get::<NodeGraph>(scene.owner)
                .map(|graph| [graph.pan[0] + 0.25, graph.pan[1] - 0.15]);
            if let Some(pan) = pan {
                pan_node_graph(scene.app.world_mut(), scene.owner, pan);
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
    use bbui_motion::MotionPlayer;
    use bbui_widgets::NodeGraph;

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
        let mut graph_scene = make_graph_scene(1_000, 4).unwrap();
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
        let mut scene = make_animation_scene(64).unwrap();
        scene.app.update();
        begin_animation(&mut scene);
        assert!(scene.app.world().resource::<MotionPlayer>().active_count() > 0);
        for _ in 0..100 {
            scene
                .app
                .world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
            scene.app.update();
        }
        assert_eq!(
            scene.app.world().resource::<MotionPlayer>().active_count(),
            0
        );
    }
}

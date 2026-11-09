use std::time::Duration;

use bevy::{app::App, ecs::entity::Entity, time::Time};
use bui_core::{CorePlugin, UiEnabled, UiNode, UiVisible, set_parent};
use bui_layout::{LayoutPlugin, Rect, UiLayoutNode, UiLayoutRoot, UiLength, UiSize};
use bui_render::DisplayList;
use bui_style::{
    StylePlugin, UiStyleProperty, UiStyleState, UiTransitionSpec, UiTransitions, set_ui_style_state,
};
use bui_text::TextPlugin;
use bui_widgets::{
    Checkbox, DataTable, ListItem, ListModel, NodeGraph, NodeSocket, SocketType, TableColumn,
    TableRow, WidgetPlugin,
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

#[derive(Clone, Copy)]
enum Workload {
    Checkboxes,
    List,
    Table,
    Graph,
    Animation,
}

struct Scene {
    app: App,
    owner: Entity,
    changed_control: Option<Entity>,
    workload: Workload,
    step: usize,
}

fn app() -> (App, Entity) {
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
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 1280.0, 720.0), 1.0),
            UiLayoutNode {
                size: UiSize {
                    width: UiLength::Px(1280.0),
                    height: UiLength::Px(720.0),
                },
                clip_contents: true,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    (app, owner)
}

fn scene(workload: Workload, count: usize) -> Scene {
    let (mut app, owner) = app();
    let mut changed_control = None;
    match workload {
        Workload::Checkboxes => {
            app.world_mut().entity_mut(owner).insert(UiLayoutNode {
                clip_contents: true,
                ..UiLayoutNode::responsive_grid(148, 8, 4.0)
            });
            for index in 0..count {
                let entity = app
                    .world_mut()
                    .spawn((
                        UiNode,
                        UiEnabled(true),
                        UiVisible(true),
                        UiLayoutNode::leaf(UiLength::Fill(1.0), UiLength::Px(28.0)),
                        Checkbox::new(format!("Control {index:05}"), false),
                    ))
                    .id();
                set_parent(app.world_mut(), entity, owner).expect("checkbox benchmark hierarchy");
                changed_control.get_or_insert(entity);
            }
        }
        Workload::List => {
            let items = (0..count)
                .map(|index| ListItem {
                    id: index as u64,
                    value: format!("List row {index:05}"),
                    enabled: true,
                })
                .collect();
            app.world_mut().entity_mut(owner).insert(ListModel {
                items,
                ..ListModel::<String>::default()
            });
        }
        Workload::Table => {
            let columns = (0..8)
                .map(|index| TableColumn::new(format!("Column {index}"), 140.0))
                .collect();
            let rows = (0..count)
                .map(|row| TableRow {
                    id: row as u64,
                    cells: (0..8)
                        .map(|column| format!("row-{row:05}-column-{column}"))
                        .collect(),
                })
                .collect();
            app.world_mut().entity_mut(owner).insert(DataTable {
                columns,
                rows,
                ..DataTable::default()
            });
        }
        Workload::Graph => {
            let mut graph = NodeGraph::default();
            let sockets = std::iter::once(NodeSocket::output("out", SocketType::Float))
                .chain(
                    (0..4).map(|index| NodeSocket::input(format!("in-{index}"), SocketType::Float)),
                )
                .collect::<Vec<_>>();
            let mut ids = Vec::with_capacity(count);
            for index in 0..count {
                ids.push(graph.add_node(
                    format!("Node {index:04}"),
                    [(index % 7) as f32 * 210.0, (index / 7) as f32 * 150.0],
                    sockets.clone(),
                ));
            }
            for target in 4..count {
                for offset in 1..=4 {
                    graph
                        .create_link(ids[target - offset], 0, ids[target], offset)
                        .expect("valid typed benchmark link");
                }
            }
            app.world_mut().entity_mut(owner).insert(graph);
        }
        Workload::Animation => {
            let mut transitions = UiTransitions::default();
            transitions.set(
                UiStyleProperty::Background,
                // Keep the transition active for Criterion's complete sampling window.
                UiTransitionSpec::new(Duration::from_secs(3_600)),
            );
            for index in 0..count {
                let entity = app
                    .world_mut()
                    .spawn((
                        UiNode,
                        UiEnabled(true),
                        UiVisible(true),
                        UiLayoutNode::leaf(UiLength::Px(48.0), UiLength::Px(20.0)),
                        transitions.clone(),
                        UiStyleState::default(),
                        Checkbox::new(format!("Animated {index}"), false),
                    ))
                    .id();
                set_parent(app.world_mut(), entity, owner).expect("animation benchmark hierarchy");
                changed_control.get_or_insert(entity);
            }
        }
    }
    for _ in 0..3 {
        app.update();
    }
    if matches!(workload, Workload::Animation) {
        for entity in app
            .world()
            .get::<bui_core::UiChildren>(owner)
            .expect("animation controls have parent")
            .0
            .clone()
        {
            set_ui_style_state(app.world_mut(), entity, UiStyleState::SELECTED, true);
        }
        app.update();
    }
    Scene {
        app,
        owner,
        changed_control,
        workload,
        step: 0,
    }
}

fn change(scene: &mut Scene, count: usize) {
    scene.step += 1;
    match scene.workload {
        Workload::Checkboxes => {
            scene
                .app
                .world_mut()
                .get_mut::<Checkbox>(scene.changed_control.expect("checkbox control"))
                .expect("checkbox exists")
                .toggle();
        }
        Workload::List => {
            let mut model = scene
                .app
                .world_mut()
                .get_mut::<ListModel<String>>(scene.owner)
                .expect("list exists");
            model.scroll_offset =
                (scene.step as f32 * model.row_height) % (count as f32 * model.row_height);
        }
        Workload::Table => {
            scene
                .app
                .world_mut()
                .get_mut::<DataTable>(scene.owner)
                .expect("table exists")
                .scroll_rows = (scene.step * 2) % count.saturating_sub(32).max(1);
        }
        Workload::Graph => {
            scene
                .app
                .world_mut()
                .get_mut::<NodeGraph>(scene.owner)
                .expect("graph exists")
                .pan = if scene.step.is_multiple_of(2) {
                [0.0, 0.0]
            } else {
                [0.25, -0.15]
            };
        }
        Workload::Animation => {
            scene
                .app
                .world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(16));
        }
    }
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("widgets/full_update");
    for (name, workload, count) in [
        ("checkboxes", Workload::Checkboxes, 1_000),
        ("checkboxes", Workload::Checkboxes, 10_000),
        ("list", Workload::List, 10_000),
        ("table_8_columns", Workload::Table, 10_000),
        ("graph_4_links_per_node", Workload::Graph, 1_000),
        ("style_animation", Workload::Animation, 1_000),
    ] {
        group.throughput(Throughput::Elements(count as u64));
        let mut idle = scene(workload, count);
        group.bench_with_input(
            BenchmarkId::new("idle", format!("{name}_{count}")),
            &count,
            |b, _| {
                b.iter(|| {
                    idle.app.update();
                    black_box(idle.app.world().resource::<DisplayList>().item_count());
                });
            },
        );
        let mut changed = scene(workload, count);
        group.bench_with_input(
            BenchmarkId::new("changed", format!("{name}_{count}")),
            &count,
            |b, _| {
                b.iter(|| {
                    change(&mut changed, count);
                    changed.app.update();
                    black_box(changed.app.world().resource::<DisplayList>().item_count());
                });
            },
        );
    }
    group.finish();
}

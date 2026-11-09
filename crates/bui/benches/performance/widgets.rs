use std::time::Duration;

use bevy::{app::App, ecs::entity::Entity, time::Time};
use bui_core::{
    CorePlugin, UiEnabled, UiHitBox, UiNode, UiPointerButton, UiRoutedEvent, UiRoutedEvents,
    UiVisible, set_parent,
};
use bui_layout::{LayoutPlugin, Rect, UiLayoutNode, UiLayoutRoot, UiLength, UiSize};
use bui_render::DisplayList;
use bui_style::{
    StylePlugin, UiStyleProperty, UiStyleState, UiTransitionSpec, UiTransitions, set_ui_style_state,
};
use bui_text::TextPlugin;
use bui_widgets::actions::{move_node_graph_node, process_routed_widget_events};
use bui_widgets::canvas::edit_canvas_object;
use bui_widgets::declarative::NodeGraphGesture;
use bui_widgets::{
    Canvas, CanvasObject, CanvasShape, Checkbox, DataTable, ListItem, ListModel, NodeGraph,
    NodeGraphEditorState, NodeGraphNodeControl, NodeSocket, SocketType, TableColumn, TableRow,
    WidgetPlugin,
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

#[derive(Clone, Copy)]
enum Workload {
    Checkboxes,
    List,
    Table,
    Graph,
    GraphNodeGesture,
    GraphNodeTrackedEdit,
    GraphNodeDirectEdit,
    CanvasOneObjectEdit,
    CanvasOneObjectDirectEdit,
    Animation,
}

struct Scene {
    app: App,
    owner: Entity,
    changed_control: Option<Entity>,
    graph_drag_start: Option<[f32; 2]>,
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
        Workload::Graph
        | Workload::GraphNodeGesture
        | Workload::GraphNodeTrackedEdit
        | Workload::GraphNodeDirectEdit => {
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
                    [
                        40.0 + (index % 7) as f32 * 210.0,
                        40.0 + (index / 7) as f32 * 150.0,
                    ],
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
            app.world_mut()
                .entity_mut(owner)
                .insert((graph, NodeGraphEditorState::default()));
        }
        Workload::CanvasOneObjectEdit | Workload::CanvasOneObjectDirectEdit => {
            let mut canvas = Canvas::default();
            canvas.objects = (0..count)
                .map(|index| {
                    let visible = index < 100;
                    CanvasObject {
                        id: index as u64 + 1,
                        origin: if visible {
                            [(index % 10) as f32 * 32.0, (index / 10) as f32 * 32.0]
                        } else {
                            [2_000.0 + index as f32 * 10.0, 0.0]
                        },
                        shape: CanvasShape::Rectangle { size: [8.0, 8.0] },
                        color: [0.2, 0.7, 0.85, 1.0],
                        visible: true,
                        locked: false,
                    }
                })
                .collect();
            app.world_mut().entity_mut(owner).insert(canvas);
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
    let mut graph_drag_start = None;
    if matches!(
        workload,
        Workload::GraphNodeGesture | Workload::GraphNodeTrackedEdit | Workload::GraphNodeDirectEdit
    ) {
        let first_node_id = app.world().get::<NodeGraph>(owner).unwrap().nodes[0].id;
        let mut query = app.world_mut().query::<(Entity, &NodeGraphNodeControl)>();
        let node_control = query
            .iter(app.world())
            .find_map(|(entity, control)| {
                (control.owner == owner && control.node_id == first_node_id).then_some(entity)
            })
            .expect("the first graph node is visible in the benchmark viewport");
        changed_control = Some(node_control);
        if matches!(workload, Workload::GraphNodeGesture) {
            let layout = *app
                .world()
                .get::<bui_layout::UiLayoutResult>(owner)
                .expect("graph has a benchmark layout");
            let graph = app.world().get::<NodeGraph>(owner).unwrap();
            let node = &graph.nodes[0];
            let header_height = app
                .world()
                .resource::<bui_style::Theme>()
                .spacing
                .node_graph
                .header_height;
            let start = [
                layout.rect.x + graph.pan[0] + node.position[0] * graph.zoom + 48.0,
                layout.rect.y + header_height + graph.pan[1] + node.position[1] * graph.zoom + 10.0,
            ];
            let window = layout.window;
            app.world_mut().resource_mut::<UiRoutedEvents>().events =
                vec![UiRoutedEvent::PointerDown {
                    window,
                    target: owner,
                    button: UiPointerButton::Primary,
                    position: start,
                }];
            process_routed_widget_events(app.world_mut());
            app.world_mut()
                .resource_mut::<UiRoutedEvents>()
                .events
                .clear();
            app.update();
            assert!(
                matches!(
                    app.world()
                        .get::<NodeGraphEditorState>(owner)
                        .and_then(|state| state.gesture.as_ref()),
                    Some(NodeGraphGesture::MoveNodes { .. })
                ),
                "the benchmark pointer-down must begin a node move: start={start:?}, owner_layout={layout:?}, gesture={:?}",
                app.world()
                    .get::<NodeGraphEditorState>(owner)
                    .and_then(|state| state.gesture.as_ref())
            );
            graph_drag_start = Some(start);
        }
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
        graph_drag_start,
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
        Workload::GraphNodeGesture => {
            let target = scene.changed_control.expect("graph node control");
            let hitbox = *scene.app.world().get::<UiHitBox>(target).unwrap();
            let start = scene.graph_drag_start.expect("graph drag is prepared");
            let offset = if scene.step.is_multiple_of(2) {
                0.5
            } else {
                1.0
            };
            scene
                .app
                .world_mut()
                .resource_mut::<UiRoutedEvents>()
                .events = vec![UiRoutedEvent::PointerMoved {
                window: hitbox.window.expect("graph has a benchmark window"),
                target: scene.owner,
                position: [start[0] + offset, start[1]],
                delta: [offset, 0.0],
                dragging: true,
            }];
            process_routed_widget_events(scene.app.world_mut());
            scene
                .app
                .world_mut()
                .resource_mut::<UiRoutedEvents>()
                .events
                .clear();
        }
        Workload::GraphNodeDirectEdit => {
            let position = if !scene.step.is_multiple_of(2) {
                [40.5, 40.0]
            } else {
                [40.0, 40.0]
            };
            scene
                .app
                .world_mut()
                .get_mut::<NodeGraph>(scene.owner)
                .expect("graph exists")
                .nodes[0]
                .position = position;
        }
        Workload::GraphNodeTrackedEdit => {
            let position = if !scene.step.is_multiple_of(2) {
                [40.5, 40.0]
            } else {
                [40.0, 40.0]
            };
            let id = scene
                .app
                .world()
                .get::<NodeGraph>(scene.owner)
                .unwrap()
                .nodes[0]
                .id;
            assert!(move_node_graph_node(
                scene.app.world_mut(),
                scene.owner,
                id,
                position
            ));
        }
        Workload::CanvasOneObjectEdit => {
            let origin = if !scene.step.is_multiple_of(2) {
                [0.5, 0.0]
            } else {
                [0.0, 0.0]
            };
            assert!(edit_canvas_object(
                scene.app.world_mut(),
                scene.owner,
                1,
                |object| object.origin = origin,
            ));
        }
        Workload::CanvasOneObjectDirectEdit => {
            let origin = if !scene.step.is_multiple_of(2) {
                [0.5, 0.0]
            } else {
                [0.0, 0.0]
            };
            scene
                .app
                .world_mut()
                .get_mut::<Canvas>(scene.owner)
                .expect("canvas exists")
                .objects[0]
                .origin = origin;
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
        ("graph_node_drag_step", Workload::GraphNodeGesture, 1_000),
        (
            "graph_node_tracked_edit",
            Workload::GraphNodeTrackedEdit,
            1_000,
        ),
        (
            "graph_node_direct_ecs_edit",
            Workload::GraphNodeDirectEdit,
            1_000,
        ),
        (
            "canvas_100k_objects_100_visible_tracked_edit",
            Workload::CanvasOneObjectEdit,
            100_000,
        ),
        (
            "canvas_100k_objects_100_visible_direct_edit",
            Workload::CanvasOneObjectDirectEdit,
            100_000,
        ),
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
        if matches!(
            workload,
            Workload::GraphNodeGesture
                | Workload::GraphNodeTrackedEdit
                | Workload::GraphNodeDirectEdit
                | Workload::CanvasOneObjectEdit
                | Workload::CanvasOneObjectDirectEdit
        ) {
            let position = |scene: &Scene| match workload {
                Workload::CanvasOneObjectEdit | Workload::CanvasOneObjectDirectEdit => {
                    scene
                        .app
                        .world()
                        .get::<Canvas>(scene.owner)
                        .unwrap()
                        .objects[0]
                        .origin
                }
                _ => {
                    scene
                        .app
                        .world()
                        .get::<NodeGraph>(scene.owner)
                        .unwrap()
                        .nodes[0]
                        .position
                }
            };
            let before = position(&changed);
            change(&mut changed, count);
            changed.app.update();
            assert_ne!(
                position(&changed),
                before,
                "the changed benchmark must move its target"
            );
        }
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

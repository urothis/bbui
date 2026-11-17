//! End-to-end CPU cost of the docking workspace's common interactions.

use bbui::{BbuiPlugin, workspace::Workspace};
use bbui_core::{UiInputEvent, UiInputQueue, UiKey, UiPointerButton};
use bbui_layout::UiLayoutResult;
use bbui_render::DisplayList;
use bbui_widgets::{DockNode, PaneId, SplitAxis, TabGroup, WorkspaceLayout, WorkspaceOperation};
use bevy::{
    asset::AssetPlugin,
    clipboard::ClipboardPlugin,
    input::InputPlugin,
    prelude::{App, Entity, ImagePlugin, MinimalPlugins, Window, World},
    window::{PrimaryWindow, WindowPlugin},
};
use criterion::{BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

fn layout(groups: usize) -> WorkspaceLayout {
    let children = (0..groups)
        .map(|group| {
            DockNode::Tabs(TabGroup::new(
                format!("group-{group}"),
                (0..3)
                    .map(|pane| PaneId(format!("pane-{group}-{pane}")))
                    .collect(),
            ))
        })
        .collect();
    WorkspaceLayout::new(DockNode::Split {
        id: "root-split".to_owned(),
        axis: SplitAxis::Horizontal,
        children,
        shares: vec![1.0; groups],
    })
}

fn scene(groups: usize) -> Result<App, String> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BbuiPlugin);
    let mut window = Window::default();
    window.resolution.set(1280.0, 720.0);
    app.world_mut().spawn((window, PrimaryWindow));
    app.insert_resource(Workspace::new(layout(groups)).map_err(|error| error.to_string())?);
    for _ in 0..3 {
        app.update();
    }
    Ok(app)
}

fn update(app: &mut App) {
    app.update();
    black_box(app.world().resource::<DisplayList>().item_count());
}

fn splitter_drag_scene(groups: usize) -> Result<(App, Entity, [f32; 2]), String> {
    let mut app = scene(groups)?;
    let window = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, bevy::prelude::With<PrimaryWindow>>();
        query.iter(world).next()
    }
    .ok_or_else(|| "workspace benchmark has no primary window".to_owned())?;
    let splitter = {
        let world: &mut World = app.world_mut();
        let mut query = world.query::<(Entity, &UiKey)>();
        query
            .iter(world)
            .find(|(_, key)| key.0 == "ws:splitter:root-split:0")
            .map(|(entity, _)| entity)
    }
    .ok_or_else(|| "workspace benchmark has no splitter".to_owned())?;
    let rect = app
        .world()
        .get::<UiLayoutResult>(splitter)
        .ok_or_else(|| "workspace benchmark splitter has no layout".to_owned())?
        .rect;
    let start = [rect.x + rect.width * 0.5, rect.y + rect.height * 0.5];
    let first_group = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &UiKey)>();
        query
            .iter(world)
            .find(|(_, key)| key.0 == "ws:group:group-0")
            .map(|(entity, _)| entity)
    }
    .ok_or_else(|| "workspace benchmark has no first group".to_owned())?;
    let original_width = app
        .world()
        .get::<UiLayoutResult>(first_group)
        .ok_or_else(|| "workspace benchmark group has no layout".to_owned())?
        .rect
        .width;
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position: start,
            time_seconds: 0.0,
        });
    update(&mut app);
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: start,
            time_seconds: 0.01,
        });
    update(&mut app);
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position: [start[0] + 2.0, start[1]],
            time_seconds: 0.02,
        });
    update(&mut app);
    update(&mut app); // Resolve the preview through style and layout.
    let DockNode::Split { shares, .. } = &app.world().resource::<Workspace>().layout().root else {
        return Err("workspace benchmark root is not a split".to_owned());
    };
    if shares[0] == 1.0 {
        return Err("workspace benchmark splitter drag did not change the layout".to_owned());
    }
    let resized_width = app
        .world()
        .get::<UiLayoutResult>(first_group)
        .ok_or_else(|| "workspace benchmark group lost its layout".to_owned())?
        .rect
        .width;
    if (resized_width - original_width).abs() < 0.1 {
        return Err("workspace benchmark splitter drag did not resize the view".to_owned());
    }
    Ok((app, window, start))
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("workspace/headless_cpu_update");
    for groups in [2, 8, 32] {
        group.throughput(Throughput::Elements((groups * 3) as u64));

        let Ok(mut idle) = scene(groups) else {
            eprintln!("failed to create idle workspace benchmark scene");
            continue;
        };
        group.bench_with_input(BenchmarkId::new("idle", groups), &groups, |b, _| {
            b.iter(|| update(&mut idle));
        });

        let Ok(mut select) = scene(groups) else {
            eprintln!("failed to create tab selection benchmark scene");
            continue;
        };
        let mut selected = 0;
        group.bench_with_input(BenchmarkId::new("select_tab", groups), &groups, |b, _| {
            b.iter(|| {
                selected = 1 - selected;
                let result = select.world_mut().resource_mut::<Workspace>().apply(
                    WorkspaceOperation::Select {
                        group: "group-0".to_owned(),
                        pane: PaneId(format!("pane-0-{selected}")),
                    },
                );
                assert!(result.is_ok(), "tab selection benchmark operation failed");
                update(&mut select);
            });
        });

        let Ok(mut moved) = scene(groups) else {
            eprintln!("failed to create pane move benchmark scene");
            continue;
        };
        let mut in_second_group = false;
        group.bench_with_input(BenchmarkId::new("move_pane", groups), &groups, |b, _| {
            b.iter(|| {
                in_second_group = !in_second_group;
                let result =
                    moved
                        .world_mut()
                        .resource_mut::<Workspace>()
                        .apply(WorkspaceOperation::Move {
                            pane: PaneId("pane-0-0".to_owned()),
                            group: format!("group-{}", usize::from(in_second_group)),
                            index: 1,
                        });
                assert!(result.is_ok(), "pane move benchmark operation failed");
                update(&mut moved);
            });
        });

        let Ok(mut resized) = scene(groups) else {
            eprintln!("failed to create split resize benchmark scene");
            continue;
        };
        let mut expanded = false;
        group.bench_with_input(BenchmarkId::new("resize_split", groups), &groups, |b, _| {
            b.iter(|| {
                expanded = !expanded;
                let mut shares = vec![1.0; groups];
                shares[0] = if expanded { 1.1 } else { 0.9 };
                shares[1] = if expanded { 0.9 } else { 1.1 };
                let result = resized.world_mut().resource_mut::<Workspace>().apply(
                    WorkspaceOperation::Resize {
                        split: "root-split".to_owned(),
                        shares,
                    },
                );
                assert!(result.is_ok(), "split resize benchmark operation failed");
                update(&mut resized);
            });
        });

        let Ok((mut dragged, window, start)) = splitter_drag_scene(groups) else {
            eprintln!("failed to create splitter drag benchmark scene");
            continue;
        };
        let mut drag_right = true;
        let mut time_seconds = 0.03;
        group.bench_with_input(
            BenchmarkId::new("drag_splitter", groups),
            &groups,
            |b, _| {
                b.iter(|| {
                    drag_right = !drag_right;
                    time_seconds += 0.01;
                    dragged.world_mut().resource_mut::<UiInputQueue>().push(
                        UiInputEvent::PointerMove {
                            window,
                            position: [start[0] + if drag_right { 2.0 } else { -2.0 }, start[1]],
                            time_seconds,
                        },
                    );
                    update(&mut dragged);
                });
            },
        );

        let Ok(mut repainted) = scene(groups) else {
            eprintln!("failed to create chrome refresh benchmark scene");
            continue;
        };
        let mut alpha = false;
        group.bench_with_input(
            BenchmarkId::new("refresh_chrome", groups),
            &groups,
            |b, _| {
                b.iter(|| {
                    let mut appearance = repainted
                        .world()
                        .resource::<Workspace>()
                        .appearance()
                        .clone();
                    alpha = !alpha;
                    appearance.drop_preview_alpha = if alpha { 0.25 } else { 0.28 };
                    repainted
                        .world_mut()
                        .resource_mut::<Workspace>()
                        .set_appearance(appearance);
                    update(&mut repainted);
                });
            },
        );
    }
    group.finish();

    let mut model = c.benchmark_group("workspace/layout_operation");
    for groups in [2, 8, 32] {
        model.throughput(Throughput::Elements((groups * 3) as u64));
        let mut selected = layout(groups);
        let mut selected_index = 0;
        model.bench_with_input(BenchmarkId::new("select_tab", groups), &groups, |b, _| {
            b.iter(|| {
                selected_index = 1 - selected_index;
                let result = selected.apply(WorkspaceOperation::Select {
                    group: "group-0".to_owned(),
                    pane: PaneId(format!("pane-0-{selected_index}")),
                });
                assert!(result.is_ok(), "model tab selection failed");
                black_box(&selected);
            });
        });

        let mut moved = layout(groups);
        let mut in_second_group = false;
        model.bench_with_input(BenchmarkId::new("move_pane", groups), &groups, |b, _| {
            b.iter(|| {
                in_second_group = !in_second_group;
                let result = moved.apply(WorkspaceOperation::Move {
                    pane: PaneId("pane-0-0".to_owned()),
                    group: format!("group-{}", usize::from(in_second_group)),
                    index: 1,
                });
                assert!(result.is_ok(), "model pane move failed");
                black_box(&moved);
            });
        });

        let mut resized = layout(groups);
        let mut expanded = false;
        model.bench_with_input(BenchmarkId::new("resize_split", groups), &groups, |b, _| {
            b.iter(|| {
                expanded = !expanded;
                let mut shares = vec![1.0; groups];
                shares[0] = if expanded { 1.1 } else { 0.9 };
                shares[1] = if expanded { 0.9 } else { 1.1 };
                let result = resized.apply(WorkspaceOperation::Resize {
                    split: "root-split".to_owned(),
                    shares,
                });
                assert!(result.is_ok(), "model split resize failed");
                black_box(&resized);
            });
        });
    }
    model.finish();
}

use bbui_core::{CorePlugin, UiNode, set_parent};
use bbui_layout::{
    LayoutPlugin, Rect, UiAlign, UiEdges, UiFlexDirection, UiFlexItem, UiIntrinsicSize,
    UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot, UiLength, UiSize,
};
use bevy_app::App;

fn main() -> Result<(), bbui_core::UiHierarchyError> {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));

    let surface = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(surface, Rect::new(0.0, 0.0, 340.0, 360.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                gap: 12.0,
                padding: UiEdges::all(8.0),
                align_items: UiAlign::Stretch,
                ..UiLayoutNode::default()
            },
        ))
        .id();

    let flex = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Flex {
                    direction: UiFlexDirection::Row,
                    wrap: true,
                },
                size: UiSize {
                    width: UiLength::Percent(1.0),
                    height: UiLength::Auto,
                },
                gap: 8.0,
                align_items: UiAlign::Center,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    set_parent(app.world_mut(), flex, root)?;

    let mut flex_items = Vec::new();
    for height in [30.0, 42.0, 24.0, 36.0, 28.0] {
        let item = app
            .world_mut()
            .spawn((
                UiNode,
                UiIntrinsicSize([84.0, height]),
                UiFlexItem {
                    basis: Some(UiLength::Px(84.0)),
                    grow: 1.0,
                    ..UiFlexItem::default()
                },
            ))
            .id();
        set_parent(app.world_mut(), item, flex)?;
        flex_items.push(item);
    }

    let grid = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::ResponsiveGrid {
                    min_cell_width: 96,
                    max_columns: 3,
                },
                size: UiSize {
                    width: UiLength::Percent(1.0),
                    height: UiLength::Px(200.0),
                },
                gap: 8.0,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    set_parent(app.world_mut(), grid, root)?;

    let mut grid_items = Vec::new();
    for height in [22.0, 34.0, 28.0, 46.0, 30.0, 40.0] {
        let item = app
            .world_mut()
            .spawn((UiNode, UiIntrinsicSize([70.0, height])))
            .id();
        set_parent(app.world_mut(), item, grid)?;
        grid_items.push(item);
    }

    app.update();
    report(&app, flex, &flex_items, grid, &grid_items, "wide");

    let Some(mut root_config) = app.world().get::<UiLayoutRoot>(root).copied() else {
        return Ok(());
    };
    root_config.viewport.width = 240.0;
    app.world_mut().entity_mut(root).insert(root_config);
    app.update();
    report(&app, flex, &flex_items, grid, &grid_items, "narrow");
    Ok(())
}

fn report(
    app: &App,
    flex: bevy_ecs::entity::Entity,
    flex_items: &[bevy_ecs::entity::Entity],
    grid: bevy_ecs::entity::Entity,
    grid_items: &[bevy_ecs::entity::Entity],
    label: &str,
) {
    println!(
        "{label}: flex={:?}",
        app.world()
            .get::<UiLayoutResult>(flex)
            .map(|layout| layout.rect)
    );
    for (index, entity) in flex_items.iter().enumerate() {
        println!(
            "  flex[{index}]={:?}",
            app.world()
                .get::<UiLayoutResult>(*entity)
                .map(|layout| layout.rect)
        );
    }
    println!(
        "{label}: grid={:?}",
        app.world()
            .get::<UiLayoutResult>(grid)
            .map(|layout| layout.rect)
    );
    for (index, entity) in grid_items.iter().enumerate() {
        println!(
            "  grid[{index}]={:?}",
            app.world()
                .get::<UiLayoutResult>(*entity)
                .map(|layout| layout.rect)
        );
    }
}

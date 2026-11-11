use super::*;

#[test]
pub(super) fn widget_handled_wheel_does_not_scroll_its_ancestor() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let scroll = app
        .world_mut()
        .spawn((
            UiNode,
            UiScroll::new(),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(30.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            UiNode,
            UiWheelHandled,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(80.0)),
        ))
        .id();
    set_parent(app.world_mut(), scroll, root).unwrap();
    set_parent(app.world_mut(), child, scroll).unwrap();
    app.update();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window,
            position: [10.0, 10.0],
            delta: [0.0, -1.0],
            unit: UiWheelUnit::Lines,
            modifiers: Default::default(),
        });
    app.update();
    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 0.0]
    );
}

#[test]
pub(super) fn grid_and_inline_layout_produce_bounded_children() {
    let mut world = World::new();
    let (_, grid) = root(&mut world, Rect::new(0.0, 0.0, 120.0, 90.0));
    world.entity_mut(grid).insert(UiLayoutNode {
        mode: UiLayoutMode::Grid { columns: 2 },
        padding: UiEdges::ZERO,
        gap: 4.0,
        ..UiLayoutNode::default()
    });
    let cells: Vec<Entity> = (0..3)
        .map(|_| world.spawn((UiNode, UiIntrinsicSize([20.0, 15.0]))).id())
        .collect();
    for child in &cells {
        set_parent(&mut world, *child, grid).unwrap();
    }
    compute_layout(&mut world);
    let first = world.get::<UiLayoutResult>(cells[0]).unwrap().rect;
    let second = world.get::<UiLayoutResult>(cells[1]).unwrap().rect;
    let third = world.get::<UiLayoutResult>(cells[2]).unwrap().rect;
    assert!(first.x < second.x);
    assert!(third.y > first.y);
}

#[test]
pub(super) fn generated_flex_grid_and_scroll_bounds_match_paint_and_hit_clips() {
    fn check_bounds(world: &World, children: &[Entity], clip: Rect) -> (usize, usize) {
        let mut visible = 0;
        let mut clipped = 0;
        for &child in children {
            let layout = world.get::<UiLayoutResult>(child).unwrap();
            let hit_box = world.get::<UiHitBox>(child).unwrap();
            let rect = layout.rect;
            assert!(
                [rect.x, rect.y, rect.width, rect.height]
                    .into_iter()
                    .all(f32::is_finite)
            );
            assert!(rect.width >= 0.0 && rect.height >= 0.0);
            assert_eq!(layout.clip, Some(clip));
            assert_eq!(hit_box.rect, rect);
            assert_eq!(hit_box.clip, layout.clip);

            let left = rect.x.max(clip.x);
            let top = rect.y.max(clip.y);
            let right = (rect.x + rect.width).min(clip.x + clip.width);
            let bottom = (rect.y + rect.height).min(clip.y + clip.height);
            if right > left && bottom > top {
                visible += 1;
                assert!(hit_box.contains([(left + right) * 0.5, (top + bottom) * 0.5]));
            } else {
                clipped += 1;
                assert!(!hit_box.contains([rect.x + rect.width * 0.5, rect.y + rect.height * 0.5]));
            }
            assert!(!hit_box.contains([clip.x - 0.5, clip.y + clip.height * 0.5]));
            assert!(!hit_box.contains([clip.x + clip.width + 0.5, clip.y + clip.height * 0.5,]));
        }
        (visible, clipped)
    }

    let mut world = World::new();
    let window = world.spawn_empty().id();
    let flex_clip = Rect::new(10.0, 10.0, 220.0, 80.0);
    let flex = world
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, flex_clip, 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Flex {
                    direction: UiFlexDirection::Row,
                    wrap: true,
                },
                clip_contents: true,
                gap: 4.0,
                padding: UiEdges::all(2.0),
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let flex_children: Vec<_> = (0..30)
        .map(|index| {
            let width = 20 + (index * 17 % 50);
            let height = 8 + (index * 11 % 24);
            world
                .spawn((
                    UiNode,
                    UiLayoutNode::leaf(UiLength::Px(width as f32), UiLength::Px(height as f32)),
                ))
                .id()
        })
        .collect();
    for child in &flex_children {
        set_parent(&mut world, *child, flex).unwrap();
    }

    let grid_clip = Rect::new(250.0, 10.0, 140.0, 80.0);
    let grid = world
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, grid_clip, 2.0),
            UiLayoutNode {
                mode: UiLayoutMode::ResponsiveGrid {
                    min_cell_width: 40,
                    max_columns: 3,
                },
                clip_contents: true,
                gap: 4.0,
                padding: UiEdges::all(2.0),
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let grid_children: Vec<_> = (0..33)
        .map(|index| {
            let width = 12 + (index * 13 % 24);
            let height = 8 + (index * 7 % 20);
            world
                .spawn((
                    UiNode,
                    UiLayoutNode::leaf(UiLength::Px(width as f32), UiLength::Px(height as f32)),
                ))
                .id()
        })
        .collect();
    for child in &grid_children {
        set_parent(&mut world, *child, grid).unwrap();
    }

    let scroll_clip = Rect::new(10.0, 120.0, 220.0, 80.0);
    let scroll = world
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, scroll_clip, 1.5),
            UiScroll::at(0.0, 10_000.0),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                clip_contents: true,
                gap: 3.0,
                padding: UiEdges::all(2.0),
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let scroll_children: Vec<_> = (0..24)
        .map(|index| {
            let height = 10 + (index * 7 % 18);
            world
                .spawn((
                    UiNode,
                    UiLayoutNode::leaf(
                        UiLength::Px(40.0 + (index * 5 % 60) as f32),
                        UiLength::Px(height as f32),
                    ),
                ))
                .id()
        })
        .collect();
    for child in &scroll_children {
        set_parent(&mut world, *child, scroll).unwrap();
    }

    compute_layout(&mut world);

    let (flex_visible, flex_clipped) = check_bounds(&world, &flex_children, flex_clip);
    let (grid_visible, grid_clipped) = check_bounds(&world, &grid_children, grid_clip);
    let (scroll_visible, scroll_clipped) = check_bounds(&world, &scroll_children, scroll_clip);
    assert!(flex_visible > 0 && flex_clipped > 0);
    assert!(grid_visible > 0 && grid_clipped > 0);
    assert!(scroll_visible > 0 && scroll_clipped > 0);
    assert_eq!(world.get::<UiScroll>(scroll).unwrap().offset[0], 0.0);
    assert!(world.get::<UiScroll>(scroll).unwrap().offset[1] > 0.0);
    assert_eq!(
        world
            .get::<UiLayoutResult>(grid_children[0])
            .unwrap()
            .dpi_scale,
        2.0
    );
    assert_eq!(
        world
            .get::<UiLayoutResult>(scroll_children[0])
            .unwrap()
            .dpi_scale,
        1.5
    );
}

#[test]
pub(super) fn flex_wrap_uses_intrinsic_sizes_gaps_and_cross_axis_alignment() {
    let mut world = World::new();
    let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 150.0, 120.0));
    world.entity_mut(parent).insert(UiLayoutNode {
        mode: UiLayoutMode::Flex {
            direction: UiFlexDirection::Row,
            wrap: true,
        },
        gap: 10.0,
        align_items: UiAlign::Center,
        padding: UiEdges::ZERO,
        ..UiLayoutNode::default()
    });
    let children: Vec<Entity> = [[60.0, 20.0], [60.0, 40.0], [40.0, 10.0]]
        .into_iter()
        .map(|size| {
            world
                .spawn((UiNode, UiIntrinsicSize(size), UiLayoutNode::default()))
                .id()
        })
        .collect();
    for child in &children {
        set_parent(&mut world, *child, parent).unwrap();
    }

    compute_layout(&mut world);

    let first = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
    let second = world.get::<UiLayoutResult>(children[1]).unwrap().rect;
    let third = world.get::<UiLayoutResult>(children[2]).unwrap().rect;
    assert_eq!(first, Rect::new(0.0, 10.0, 60.0, 20.0));
    assert_eq!(second, Rect::new(70.0, 0.0, 60.0, 40.0));
    assert_eq!(third, Rect::new(0.0, 50.0, 40.0, 10.0));
    assert_eq!(
        world.get::<UiLayoutResult>(parent).unwrap().content_size,
        [130.0, 60.0]
    );
}

#[test]
pub(super) fn exact_child_lengths_measure_without_expanding_nested_content() {
    let mut world = World::new();
    let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 100.0));
    let fixed_container = world
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(40.0),
                    height: UiLength::Px(30.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let oversized_descendant = world
        .spawn((
            UiNode,
            UiIntrinsicSize([500.0, 400.0]),
            UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
        ))
        .id();
    set_parent(&mut world, fixed_container, parent).unwrap();
    set_parent(&mut world, oversized_descendant, fixed_container).unwrap();

    compute_layout(&mut world);

    assert_eq!(
        world.get::<UiLayoutResult>(parent).unwrap().content_size,
        [60.0, 50.0]
    );
    assert_eq!(
        world
            .get::<UiLayoutResult>(fixed_container)
            .unwrap()
            .content_size,
        [500.0, 400.0]
    );
    assert_eq!(
        world.get::<UiLayoutResult>(fixed_container).unwrap().rect,
        Rect::new(10.0, 10.0, 40.0, 30.0)
    );
}

#[test]
pub(super) fn flex_grow_respects_weights_and_maximum_constraints() {
    let mut world = World::new();
    let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 200.0, 50.0));
    world.entity_mut(parent).insert(UiLayoutNode {
        mode: UiLayoutMode::Flex {
            direction: UiFlexDirection::Row,
            wrap: false,
        },
        gap: 10.0,
        padding: UiEdges::ZERO,
        ..UiLayoutNode::default()
    });
    let first = world
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Auto, UiLength::Auto),
            UiFlexItem {
                basis: Some(UiLength::Px(40.0)),
                grow: 1.0,
                ..UiFlexItem::default()
            },
        ))
        .id();
    let second = world
        .spawn((
            UiNode,
            UiLayoutNode {
                mode: UiLayoutMode::Leaf,
                size: UiSize {
                    width: UiLength::Auto,
                    height: UiLength::Auto,
                },
                max_size: [Some(100.0), None],
                ..UiLayoutNode::default()
            },
            UiFlexItem {
                basis: Some(UiLength::Px(40.0)),
                grow: 3.0,
                ..UiFlexItem::default()
            },
        ))
        .id();
    set_parent(&mut world, first, parent).unwrap();
    set_parent(&mut world, second, parent).unwrap();

    compute_layout(&mut world);

    let first_rect = world.get::<UiLayoutResult>(first).unwrap().rect;
    let second_rect = world.get::<UiLayoutResult>(second).unwrap().rect;
    assert_eq!(first_rect.width, 90.0);
    assert_eq!(second_rect.width, 100.0);
    assert_eq!(second_rect.x, 100.0);

    let mut root_config = *world.get::<UiLayoutRoot>(parent).unwrap();
    root_config.viewport.width = 90.0;
    world.entity_mut(parent).insert(root_config);
    for child in [first, second] {
        let mut child_style = *world.get::<UiLayoutNode>(child).unwrap();
        child_style.min_size[0] = Some(50.0);
        world.entity_mut(child).insert(child_style);
    }
    compute_layout(&mut world);
    assert_eq!(world.get::<UiLayoutResult>(first).unwrap().rect.width, 50.0);
    assert_eq!(
        world.get::<UiLayoutResult>(second).unwrap().rect.width,
        50.0
    );
    assert_eq!(world.get::<UiLayoutResult>(second).unwrap().rect.x, 60.0);
}

#[test]
pub(super) fn flex_fill_basis_uses_same_wrapping_measurement_and_distribution() {
    let mut world = World::new();
    let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 50.0));
    world.entity_mut(parent).insert(UiLayoutNode {
        mode: UiLayoutMode::Flex {
            direction: UiFlexDirection::Row,
            wrap: true,
        },
        gap: 10.0,
        padding: UiEdges::ZERO,
        ..UiLayoutNode::default()
    });
    let fill = world
        .spawn((
            UiNode,
            UiIntrinsicSize([60.0, 20.0]),
            UiLayoutNode {
                mode: UiLayoutMode::Leaf,
                size: UiSize {
                    width: UiLength::Fill(1.0),
                    height: UiLength::Auto,
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let fixed = world
        .spawn((
            UiNode,
            UiIntrinsicSize([40.0, 20.0]),
            UiLayoutNode::leaf(UiLength::Px(40.0), UiLength::Auto),
        ))
        .id();
    set_parent(&mut world, fill, parent).unwrap();
    set_parent(&mut world, fixed, parent).unwrap();

    compute_layout(&mut world);

    let fill_rect = world.get::<UiLayoutResult>(fill).unwrap().rect;
    let fixed_rect = world.get::<UiLayoutResult>(fixed).unwrap().rect;
    assert_eq!(fill_rect.width, 50.0);
    assert_eq!(fixed_rect.x, 60.0);
    assert_eq!(fixed_rect.y, 0.0);
    assert_eq!(
        world.get::<UiLayoutResult>(parent).unwrap().content_size,
        [50.0, 20.0]
    );
}

#[test]
pub(super) fn column_flex_wraps_into_intrinsic_columns() {
    let mut world = World::new();
    let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 100.0));
    world.entity_mut(parent).insert(UiLayoutNode {
        mode: UiLayoutMode::Flex {
            direction: UiFlexDirection::Column,
            wrap: true,
        },
        gap: 10.0,
        padding: UiEdges::ZERO,
        ..UiLayoutNode::default()
    });
    let children: Vec<Entity> = [[20.0, 40.0], [40.0, 40.0], [30.0, 30.0]]
        .into_iter()
        .map(|size| {
            world
                .spawn((UiNode, UiIntrinsicSize(size), UiLayoutNode::default()))
                .id()
        })
        .collect();
    for child in &children {
        set_parent(&mut world, *child, parent).unwrap();
    }

    compute_layout(&mut world);

    let first = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
    let second = world.get::<UiLayoutResult>(children[1]).unwrap().rect;
    let third = world.get::<UiLayoutResult>(children[2]).unwrap().rect;
    assert_eq!(first, Rect::new(0.0, 0.0, 40.0, 40.0));
    assert_eq!(second, Rect::new(0.0, 50.0, 40.0, 40.0));
    assert_eq!(third, Rect::new(50.0, 0.0, 30.0, 30.0));
    assert_eq!(
        world.get::<UiLayoutResult>(parent).unwrap().content_size,
        [80.0, 90.0]
    );
}

#[test]
pub(super) fn flex_item_change_invalidates_and_reflows_its_layout_root() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin));
    let (_, parent) = root(app.world_mut(), Rect::new(0.0, 0.0, 200.0, 60.0));
    app.world_mut().entity_mut(parent).insert(UiLayoutNode {
        mode: UiLayoutMode::Flex {
            direction: UiFlexDirection::Row,
            wrap: false,
        },
        gap: 10.0,
        padding: UiEdges::ZERO,
        ..UiLayoutNode::default()
    });
    let first = app
        .world_mut()
        .spawn((
            UiNode,
            UiFlexItem {
                basis: Some(UiLength::Px(50.0)),
                ..UiFlexItem::default()
            },
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            UiNode,
            UiFlexItem {
                basis: Some(UiLength::Px(50.0)),
                ..UiFlexItem::default()
            },
        ))
        .id();
    set_parent(app.world_mut(), first, parent).unwrap();
    set_parent(app.world_mut(), second, parent).unwrap();
    app.update();
    assert_eq!(
        app.world().get::<UiLayoutResult>(second).unwrap().rect.x,
        60.0
    );

    app.world_mut().entity_mut(first).insert(UiFlexItem {
        basis: Some(UiLength::Px(80.0)),
        ..UiFlexItem::default()
    });
    app.update();
    assert_eq!(
        app.world().get::<UiLayoutResult>(second).unwrap().rect.x,
        90.0
    );
}

#[test]
pub(super) fn responsive_grid_reflows_at_width_threshold_and_uses_logical_pixels() {
    let mut world = World::new();
    let window = world.spawn_empty().id();
    let parent = world
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 320.0, 200.0), 2.0),
            UiLayoutNode {
                mode: UiLayoutMode::ResponsiveGrid {
                    min_cell_width: 100,
                    max_columns: 3,
                },
                gap: 10.0,
                align_items: UiAlign::Start,
                padding: UiEdges::ZERO,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let children: Vec<Entity> = [
        [20.0, 20.0],
        [30.0, 30.0],
        [40.0, 40.0],
        [50.0, 50.0],
        [60.0, 60.0],
    ]
    .into_iter()
    .map(|size| world.spawn((UiNode, UiIntrinsicSize(size))).id())
    .collect();
    for child in &children {
        set_parent(&mut world, *child, parent).unwrap();
    }

    compute_layout(&mut world);

    let first_row = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
    let third_cell = world.get::<UiLayoutResult>(children[2]).unwrap().rect;
    let second_row = world.get::<UiLayoutResult>(children[3]).unwrap().rect;
    assert_eq!(first_row.x, 0.0);
    assert_eq!(third_cell.x, 220.0);
    assert_eq!(second_row.y, 50.0);
    assert_eq!(world.get::<UiLayoutResult>(parent).unwrap().dpi_scale, 2.0);
    assert_eq!(
        world.get::<UiLayoutResult>(parent).unwrap().content_size,
        [320.0, 110.0]
    );

    let mut root_config = *world.get::<UiLayoutRoot>(parent).unwrap();
    root_config.viewport.width = 190.0;
    world.entity_mut(parent).insert(root_config);
    compute_layout(&mut world);

    let first = world.get::<UiLayoutResult>(children[0]).unwrap().rect;
    let second = world.get::<UiLayoutResult>(children[1]).unwrap().rect;
    assert_eq!(first.x, 0.0);
    assert_eq!(second.x, 0.0);
    assert_eq!(second.y, 30.0);
    assert_eq!(world.get::<UiLayoutResult>(parent).unwrap().dpi_scale, 2.0);
}

#[test]
pub(super) fn layout_skips_non_nodes_and_sanitizes_invalid_dimensions() {
    let mut world = World::new();
    let (_, parent) = root(&mut world, Rect::new(0.0, 0.0, 100.0, 50.0));
    let child = world
        .spawn((
            UiNode,
            UiVisible(true),
            UiLayoutNode::leaf(UiLength::Px(f32::NAN), UiLength::Px(-4.0)),
        ))
        .id();
    set_parent(&mut world, child, parent).unwrap();
    compute_layout(&mut world);
    assert_eq!(world.get::<UiLayoutResult>(child).unwrap().rect.width, 0.0);
    assert_eq!(world.get::<UiLayoutResult>(child).unwrap().rect.height, 0.0);
}

#[test]
pub(super) fn layout_phase_populates_hit_boxes_before_input_phase_in_the_same_frame() {
    let mut app = App::new();
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    let (window, parent) = root(app.world_mut(), Rect::new(0.0, 0.0, 100.0, 60.0));
    let button = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiLayoutNode::leaf(UiLength::Px(40.0), UiLength::Px(24.0)),
        ))
        .id();
    set_parent(app.world_mut(), button, parent).unwrap();
    app.world_mut().resource_mut::<UiInputQueue>().extend([
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: true,
            position: [10.0, 10.0],
            time_seconds: 0.0,
        },
        UiInputEvent::PointerButton {
            window,
            button: UiPointerButton::Primary,
            pressed: false,
            position: [10.0, 10.0],
            time_seconds: 0.1,
        },
    ]);
    app.update();
    let result = app.world().get::<UiLayoutResult>(button).unwrap();
    assert_eq!(result.window, window);
    assert!(
        app.world()
            .resource::<UiRoutedEvents>()
            .events
            .iter()
            .any(|event| matches!(event, UiRoutedEvent::Click { target, .. } if *target == button))
    );
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct LayoutChanges {
    pub(super) results: usize,
    pub(super) hit_boxes: usize,
    pub(super) issues: usize,
}

pub(super) fn count_layout_changes(
    results: Query<Ref<UiLayoutResult>>,
    hit_boxes: Query<Ref<UiHitBox>>,
    issues: Res<UiLayoutIssues>,
    mut changes: ResMut<LayoutChanges>,
) {
    changes.results += results.iter().filter(|result| result.is_changed()).count();
    changes.hit_boxes += hit_boxes
        .iter()
        .filter(|hit_box| hit_box.is_changed())
        .count();
    changes.issues += usize::from(issues.is_changed());
}

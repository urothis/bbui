use super::*;

#[test]
pub(super) fn camera_presence_changes_invalidate_only_layouts_for_that_window() {
    const ITEMS_PER_WINDOW: usize = 32;
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let first_window = app.world_mut().spawn_empty().id();
    let second_window = app.world_mut().spawn_empty().id();
    let make_painted = |world: &mut bevy::prelude::World, window| {
        world
            .spawn((
                UiPaint(Primitive::RoundedRect {
                    rect: ClipRect {
                        x: 0.0,
                        y: 0.0,
                        width: 20.0,
                        height: 20.0,
                    },
                    radius: 0.0,
                    color: Rgba(0.4, 0.5, 0.6, 1.0),
                }),
                UiLayoutResult {
                    window,
                    rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                    clip: None,
                    z: 0,
                    content_size: [20.0, 20.0],
                    dpi_scale: 1.0,
                },
            ))
            .id()
    };
    let first_items: Vec<_> = (0..ITEMS_PER_WINDOW)
        .map(|_| make_painted(app.world_mut(), first_window))
        .collect();
    let second_items: Vec<_> = (0..ITEMS_PER_WINDOW)
        .map(|_| make_painted(app.world_mut(), second_window))
        .collect();
    app.update();

    let stable_key = RenderKey::LayoutEntity(second_items[0]);
    let stable_node = cached_single_node(app.world().resource::<RenderCache>(), stable_key);
    app.world_mut()
        .entity_mut(first_window)
        .insert(Camera::default());
    app.update();
    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(stats.full_syncs, 1, "camera addition stays window-scoped");
    assert_eq!(stats.incremental_syncs, 1);
    assert_eq!(stats.scanned_draw_keys, ITEMS_PER_WINDOW);
    let cache = app.world().resource::<RenderCache>();
    assert_eq!(
        cache.items[&RenderKey::LayoutEntity(first_items[0])].target_camera,
        Some(first_window)
    );
    assert_eq!(cached_single_node(cache, stable_key), stable_node);

    app.world_mut().entity_mut(first_window).remove::<Camera>();
    app.update();
    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(stats.full_syncs, 1, "camera removal stays window-scoped");
    assert_eq!(stats.incremental_syncs, 2);
    assert_eq!(stats.scanned_draw_keys, ITEMS_PER_WINDOW);
    let cache = app.world().resource::<RenderCache>();
    assert_eq!(
        cache.items[&RenderKey::LayoutEntity(first_items[0])].target_camera,
        None
    );
    assert_eq!(cached_single_node(cache, stable_key), stable_node);
}

#[test]
pub(super) fn wrapped_text_uses_explicit_width_and_linebreak_rule() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 100.0,
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![DrawItem {
            clip,
            z: 0,
            primitive: Primitive::TextWrapped {
                origin: [5.0, 6.0],
                text: "wrapped text content".to_owned(),
                color: Rgba(1.0, 1.0, 1.0, 1.0),
                max_width: 83.0,
                wrap: true,
                style: None,
            },
        }]);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<(&Node, &TextBounds, &TextLayout), With<Text>>();
    let (node, bounds, layout) = query.single(app.world()).unwrap();
    assert_eq!(node.width, Val::Px(83.0));
    assert_eq!(bounds.width, Some(83.0));
    assert_eq!(bounds.height, None);
    assert_eq!(layout.linebreak, TextLayout::default().linebreak);

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![DrawItem {
            clip,
            z: 0,
            primitive: Primitive::TextWrapped {
                origin: [5.0, 6.0],
                text: "single line".to_owned(),
                color: Rgba(1.0, 1.0, 1.0, 1.0),
                max_width: 83.0,
                wrap: false,
                style: Some(TextRenderStyle {
                    font: Some(TextFont::from_font_size(22.0)),
                    line_height: Some(LineHeight::RelativeToFont(1.8)),
                    letter_spacing: LetterSpacing::Px(2.0),
                    justify: Justify::Right,
                    line_break: LineBreak::AnyCharacter,
                }),
            },
        }]);
    app.update();
    let (_, _, layout) = query.single(app.world()).unwrap();
    assert_eq!(layout.linebreak, LineBreak::AnyCharacter);
    assert_eq!(layout.justify, Justify::Right);
    let mut style_query = app
        .world_mut()
        .query_filtered::<(&TextFont, &LineHeight, &LetterSpacing), With<Text>>();
    let (font, line_height, spacing) = style_query.single(app.world()).unwrap();
    assert_eq!(font.font_size, bevy::text::FontSize::Px(22.0));
    assert_eq!(*line_height, LineHeight::RelativeToFont(1.8));
    assert_eq!(*spacing, LetterSpacing::Px(2.0));
}

#[test]
pub(super) fn shadow_icon_and_bezier_use_bevy_gpu_ui_components() {
    let mut app = App::new();
    app.insert_resource(Assets::<BezierMaterial>::default());
    app.add_plugins(BbuiRenderPlugin);
    app.world_mut()
        .resource_mut::<ImageRegistry>()
        .insert(5, Handle::default());
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 80.0,
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![
            DrawItem {
                clip,
                z: 0,
                primitive: Primitive::Shadow {
                    rect: ClipRect {
                        x: 8.0,
                        y: 8.0,
                        width: 30.0,
                        height: 20.0,
                    },
                    radius: 3.0,
                    color: Rgba(0.2, 0.25, 0.3, 1.0),
                    shadow: Rgba(0.0, 0.0, 0.0, 0.5),
                    offset: [1.0, 2.0],
                    blur: 4.0,
                    spread: 1.0,
                },
            },
            DrawItem {
                clip,
                z: 1,
                primitive: Primitive::Icon {
                    rect: ClipRect {
                        x: 45.0,
                        y: 8.0,
                        width: 16.0,
                        height: 16.0,
                    },
                    asset_id: 5,
                    tint: Rgba(0.8, 0.9, 1.0, 1.0),
                },
            },
            DrawItem {
                clip,
                z: 2,
                primitive: Primitive::Bezier {
                    points: [[10.0, 60.0], [30.0, 35.0], [60.0, 75.0], [90.0, 45.0]],
                    width: 2.0,
                    color: Rgba(0.3, 0.8, 0.7, 1.0),
                },
            },
        ]);
    app.update();

    let mut shadow_query = app.world_mut().query::<&BoxShadow>();
    assert_eq!(shadow_query.iter(app.world()).count(), 1);
    let mut icon_query = app.world_mut().query::<&ImageNode>();
    assert_eq!(icon_query.iter(app.world()).count(), 1);
    let mut curve_query = app
        .world_mut()
        .query_filtered::<(Entity, &MaterialNode<BezierMaterial>), With<RenderedItem>>();
    let (curve_entity, curve_material) = curve_query.single(app.world()).unwrap();
    let cached_curve =
        &app.world().resource::<RenderCache>().items[&RenderKey::WidgetDisplayListIndex(2)];
    assert_eq!(cached_curve.nodes.first(), Some(curve_entity));
    assert_eq!(
        app.world()
            .resource::<Assets<BezierMaterial>>()
            .get(&curve_material.0)
            .unwrap()
            .style
            .x,
        2.0
    );
    assert_eq!(app.world().resource::<Assets<BezierMaterial>>().len(), 1);

    let mut items = app
        .world_mut()
        .query_filtered::<Entity, With<RenderedItem>>();
    assert_eq!(items.iter(app.world()).count(), 3);
    let mut groups = app
        .world_mut()
        .query_filtered::<&Node, With<RenderedClipGroup>>();
    assert_eq!(groups.iter(app.world()).count(), 1);
    assert!(
        groups
            .iter(app.world())
            .all(|node| node.overflow == Overflow::clip())
    );
}

#[test]
pub(super) fn degenerate_bezier_controls_produce_finite_shader_uniforms() {
    let item = DrawItem {
        clip: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        z: 0,
        primitive: Primitive::Bezier {
            points: [[25.0, 25.0]; 4],
            width: 2.0,
            color: Rgba(0.9, 0.4, 0.2, 1.0),
        },
    };
    let values = bezier_material_values(&item).unwrap();
    assert!(
        [
            values.points_01.x,
            values.points_01.y,
            values.points_01.z,
            values.points_01.w,
            values.points_23.x,
            values.points_23.y,
            values.points_23.z,
            values.points_23.w,
            values.style.x,
        ]
        .iter()
        .all(|value| value.is_finite())
    );
    assert_eq!(
        (values.points_01.x, values.points_01.y),
        (values.points_01.z, values.points_01.w)
    );
    assert_eq!(
        (values.points_23.x, values.points_23.y),
        (values.points_23.z, values.points_23.w)
    );
}

#[test]
pub(super) fn changed_bezier_updates_shader_and_bounds_without_node_churn() {
    let mut app = App::new();
    app.insert_resource(Assets::<BezierMaterial>::default());
    app.add_plugins(BbuiRenderPlugin);
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 160.0,
        height: 120.0,
    };
    let item = |points, color| DrawItem {
        clip,
        z: 0,
        primitive: Primitive::Bezier {
            points,
            width: 2.0,
            color,
        },
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![item(
            [[10.0, 20.0], [20.0, 15.0], [35.0, 25.0], [45.0, 20.0]],
            Rgba(0.2, 0.6, 0.9, 1.0),
        )]);
    app.update();

    let (entity, handle) = {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &MaterialNode<BezierMaterial>), With<RenderedItem>>();
        let (entity, node) = query.single(app.world()).unwrap();
        (entity, node.0.clone())
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![item(
            [[30.0, 25.0], [42.0, 38.0], [55.0, 10.0], [70.0, 30.0]],
            Rgba(0.9, 0.3, 0.1, 1.0),
        )]);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &MaterialNode<BezierMaterial>), With<RenderedItem>>();
    let (updated_entity, updated_node) = query.single(app.world()).unwrap();
    assert_eq!(updated_entity, entity);
    assert_eq!(updated_node.0, handle);
    let material = app
        .world()
        .resource::<Assets<BezierMaterial>>()
        .get(&handle)
        .unwrap();
    assert_eq!(material.color, Vec4::new(0.9, 0.3, 0.1, 1.0));
    let new_bounds = app.world().resource::<RenderCache>().items
        [&RenderKey::WidgetDisplayListIndex(0)]
        .bezier_values
        .unwrap()
        .rect;
    assert_eq!(material.points_01.x, 30.0 - new_bounds.x);
    let node = app.world().get::<Node>(updated_entity).unwrap();
    assert_eq!(node.left, Val::Px(new_bounds.x - clip.x));
    assert_eq!(node.top, Val::Px(new_bounds.y - clip.y));
    assert_eq!(app.world().resource::<Assets<BezierMaterial>>().len(), 1);
}

#[test]
pub(super) fn direct_bezier_topology_edit_rebuilds_only_its_batch_boundary() {
    const CURVE_COUNT: usize = 16;
    const STATIC_COUNT: usize = 256;
    let mut app = App::new();
    app.insert_resource(Assets::<BezierMaterial>::default());
    app.insert_resource(Assets::<BezierBatchMaterial>::default());
    app.insert_resource(Assets::<ShaderBuffer>::default());
    app.add_plugins(BbuiRenderPlugin);
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 80.0,
    };
    let mut curves = Vec::new();
    for index in 0..CURVE_COUNT {
        let x = index as f32 * 8.0;
        curves.push(
            app.world_mut()
                .spawn((
                    UiPaint(Primitive::Bezier {
                        points: [[x, 10.0], [x + 2.0, 12.0], [x + 4.0, 8.0], [x + 6.0, 10.0]],
                        width: 1.5,
                        color: Rgba(0.2, 0.75, 0.9, 1.0),
                    }),
                    UiLayoutResult {
                        window: Entity::PLACEHOLDER,
                        rect: Rect::new(0.0, 0.0, 180.0, 40.0),
                        clip: Some(Rect::new(clip.x, clip.y, clip.width, clip.height)),
                        z: 0,
                        content_size: [180.0, 40.0],
                        dpi_scale: 1.0,
                    },
                ))
                .id(),
        );
    }
    let static_entities: Vec<_> = (0..STATIC_COUNT)
        .map(|index| {
            app.world_mut()
                .spawn((
                    UiPaint(Primitive::RoundedRect {
                        rect: clip,
                        radius: 0.0,
                        color: Rgba(0.4, 0.5, 0.6, 1.0),
                    }),
                    UiLayoutResult {
                        window: Entity::PLACEHOLDER,
                        rect: Rect::new(index as f32, 0.0, 1.0, 1.0),
                        clip: Some(Rect::new(clip.x, clip.y, clip.width, clip.height)),
                        z: 0,
                        content_size: [1.0, 1.0],
                        dpi_scale: 1.0,
                    },
                ))
                .id()
        })
        .collect();
    app.update();

    let changed = curves[CURVE_COUNT / 2];
    let stable_key = RenderKey::LayoutEntity(static_entities[0]);
    let (old_batch_entity, stable_node) = {
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
        (
            query.single(app.world()).unwrap(),
            cached_single_node(app.world().resource::<RenderCache>(), stable_key),
        )
    };
    app.world_mut()
        .entity_mut(changed)
        .insert(UiPaint(Primitive::RoundedRect {
            rect: clip,
            radius: 2.0,
            color: Rgba(0.9, 0.2, 0.1, 1.0),
        }));
    app.update();

    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(
        stats.full_syncs, 1,
        "a local batch boundary stays incremental"
    );
    assert_eq!(stats.incremental_syncs, 1);
    assert_eq!(stats.scanned_draw_keys, CURVE_COUNT + 1);
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
    let batch_entities: HashSet<_> = query.iter(app.world()).collect();
    assert_eq!(batch_entities.len(), 2);
    assert!(!batch_entities.contains(&old_batch_entity));
    assert_eq!(
        app.world().resource::<RenderCache>().bezier_batches.len(),
        2
    );
    assert_eq!(
        cached_single_node(app.world().resource::<RenderCache>(), stable_key),
        stable_node,
        "items outside the affected batch keep their Bevy nodes"
    );
    assert!(
        app.world()
            .resource::<RenderCache>()
            .items
            .contains_key(&RenderKey::LayoutEntity(changed))
    );
}

#[test]
pub(super) fn four_thousand_beziers_share_batched_render_and_release_gpu_assets() {
    const CURVE_COUNT: usize = 4_000;
    const MAX_BATCH_CURVES: usize = 128;
    let mut app = App::new();
    app.insert_resource(Assets::<BezierMaterial>::default());
    app.insert_resource(Assets::<BezierBatchMaterial>::default());
    app.insert_resource(Assets::<ShaderBuffer>::default());
    app.add_plugins(BbuiRenderPlugin);
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 1200.0,
        height: 800.0,
    };
    let owner = app.world_mut().spawn_empty().id();
    let fragments: Vec<_> = (0..CURVE_COUNT)
        .map(|index| {
            let entity = app.world_mut().spawn_empty().id();
            let x = (index % 100) as f32 * 10.0;
            let y = (index / 100) as f32 * 18.0;
            let fragment = WidgetRenderFragment {
                entity,
                order: index as u64,
                target_camera: None,
                items: vec![DrawItem {
                    clip,
                    z: 0,
                    primitive: Primitive::Bezier {
                        points: [
                            [x, y],
                            [x + 30.0, y + 18.0],
                            [x + 50.0, y - 12.0],
                            [x + 80.0, y + 2.0],
                        ],
                        width: 1.5,
                        color: Rgba(0.2, 0.75, 0.9, 1.0),
                    },
                }],
            };
            (entity, fragment)
        })
        .collect();
    {
        let mut display_list = app.world_mut().resource_mut::<DisplayList>();
        for (index, (_, fragment)) in fragments.iter().enumerate() {
            assert!(display_list.upsert_widget_fragment_with_batch_group(
                fragment.clone(),
                Some(WidgetFragmentBatchGroup {
                    owner,
                    chunk: (index / MAX_BATCH_CURVES) as u32,
                }),
            ));
        }
    }
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
    let original_batch_entities: HashSet<_> = query.iter(app.world()).collect();
    let expected_batches = CURVE_COUNT.div_ceil(MAX_BATCH_CURVES);
    assert_eq!(original_batch_entities.len(), expected_batches);
    assert_eq!(
        app.world().resource::<RenderCache>().items.len(),
        0,
        "batched curves are owned by the batch cache"
    );
    assert_eq!(
        app.world().resource::<RenderCache>().bezier_batches.len(),
        expected_batches
    );
    assert_eq!(
        app.world().resource::<Assets<BezierBatchMaterial>>().len(),
        expected_batches
    );
    assert_eq!(
        app.world().resource::<Assets<ShaderBuffer>>().len(),
        expected_batches * 3
    );

    let changed_index = CURVE_COUNT / 2;
    let (_, changed_fragment) = &fragments[changed_index];
    let mut changed_fragment = changed_fragment.clone();
    let Primitive::Bezier { points, width, .. } = &changed_fragment.items[0].primitive else {
        panic!();
    };
    changed_fragment.items[0].primitive = Primitive::Bezier {
        points: *points,
        width: *width,
        color: Rgba(0.9, 0.1, 0.2, 1.0),
    };
    assert!(
        app.world_mut()
            .resource_mut::<DisplayList>()
            .upsert_widget_fragment_with_batch_group(
                changed_fragment,
                Some(WidgetFragmentBatchGroup {
                    owner,
                    chunk: (changed_index / MAX_BATCH_CURVES) as u32,
                }),
            )
    );
    app.update();
    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(stats.full_syncs, 1);
    assert_eq!(stats.incremental_syncs, 1);
    assert_eq!(
        stats.scanned_draw_keys,
        MAX_BATCH_CURVES * 2,
        "one link update scans its old and rebuilt 128-link batch only"
    );
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
    let updated_batch_entities: HashSet<_> = query.iter(app.world()).collect();
    assert_eq!(updated_batch_entities, original_batch_entities);
    assert_eq!(
        app.world().resource::<Assets<BezierBatchMaterial>>().len(),
        expected_batches
    );
    assert_eq!(
        app.world().resource::<Assets<ShaderBuffer>>().len(),
        expected_batches * 3
    );

    {
        let mut display_list = app.world_mut().resource_mut::<DisplayList>();
        for (entity, _) in &fragments {
            assert!(display_list.remove_widget_fragment(*entity));
        }
    }
    app.update();
    assert!(
        app.world()
            .resource::<RenderCache>()
            .bezier_batches
            .is_empty()
    );
    assert_eq!(
        app.world().resource::<Assets<BezierBatchMaterial>>().len(),
        0
    );
    assert_eq!(app.world().resource::<Assets<ShaderBuffer>>().len(), 0);
}

#[test]
pub(super) fn widget_layer_updates_do_not_respawn_manual_gpu_nodes() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    app.world_mut()
        .resource_mut::<DisplayList>()
        .push(rect_item(0, 12.0));
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<RenderedItem>>();
    let manual = query.single(app.world()).unwrap();
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![rect_item(1, 44.0)]);
    app.update();
    assert!(app.world().entities().contains(manual));
    assert_eq!(query.iter(app.world()).count(), 2);
}

#[test]
pub(super) fn screen_and_world_paints_route_to_separate_ui_cameras() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let screen_window = app.world_mut().spawn_empty().id();
    let world_camera = app.world_mut().spawn(Camera::default()).id();
    let screen_entity = app
        .world_mut()
        .spawn((
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: Rgba(1.0, 0.0, 0.0, 1.0),
            }),
            UiLayoutResult {
                window: screen_window,
                rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                clip: None,
                z: 0,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let world_entity = app
        .world_mut()
        .spawn((
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: Rgba(0.0, 1.0, 0.0, 1.0),
            }),
            UiLayoutResult {
                window: world_camera,
                rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                clip: None,
                z: 0,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();

    let cache = app.world().resource::<RenderCache>();
    let screen_group_key = cache.items[&RenderKey::LayoutEntity(screen_entity)]
        .clip_group
        .unwrap();
    let world_group_key = cache.items[&RenderKey::LayoutEntity(world_entity)]
        .clip_group
        .unwrap();
    let screen_group = cache.clip_groups[&screen_group_key].entity;
    let world_group = cache.clip_groups[&world_group_key].entity;
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, Option<&UiTargetCamera>), With<RenderedClipGroup>>();
    let targets: HashMap<_, _> = query
        .iter(app.world())
        .map(|(entity, target)| (entity, target.map(|target| target.0)))
        .collect();
    assert_eq!(targets.get(&screen_group), Some(&None));
    assert_eq!(targets.get(&world_group), Some(&Some(world_camera)));
}

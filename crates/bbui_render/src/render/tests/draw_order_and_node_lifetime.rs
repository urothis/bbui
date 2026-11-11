use super::*;

#[test]
pub(super) fn display_list_is_stably_ordered_by_z() {
    let mut list = DisplayList::default();
    list.push(rect_item(4, 1.0));
    list.push(rect_item(2, 2.0));
    list.push(rect_item(4, 3.0));
    assert_eq!(
        list.items().iter().map(|item| item.z).collect::<Vec<_>>(),
        [2, 4, 4]
    );
    assert_eq!(list.items()[1].primitive, rect_item(4, 1.0).primitive);
    assert_eq!(list.items()[2].primitive, rect_item(4, 3.0).primitive);
}

#[test]
pub(super) fn keyed_fragments_keep_merged_views_and_camera_order_compatible() {
    let mut list = DisplayList::default();
    let first = Entity::from_bits(1);
    let second = Entity::from_bits(2);
    let camera = Entity::from_bits(3);
    assert!(list.upsert_widget_fragment(widget_fragment(
        first,
        20,
        None,
        vec![rect_item(4, 20.0)]
    )));
    assert!(list.upsert_widget_fragment(widget_fragment(
        second,
        10,
        None,
        vec![rect_item(4, 10.0)]
    )));
    assert!(list.upsert_widget_fragment(widget_fragment(
        camera,
        0,
        Some(camera),
        vec![rect_item(4, 30.0)]
    )));
    list.replace_widget_items(vec![rect_item(4, 15.0)]);
    list.replace_camera_items(Some(camera), vec![rect_item(4, 25.0)]);
    list.replace_overlay_camera_items(camera, vec![rect_item(4, 35.0)]);
    assert!(!list.upsert_widget_fragment(widget_fragment(
        first,
        20,
        None,
        vec![rect_item(4, 20.0)]
    )));
    assert_eq!(list.item_count(), 6);
    assert_eq!(list.widget_items().len(), 3);
    assert_eq!(list.camera_items(camera).unwrap().len(), 2);
    let positions: Vec<_> = list
        .items()
        .iter()
        .map(|item| match item.primitive {
            Primitive::RoundedRect { rect, .. } => rect.x,
            _ => panic!(),
        })
        .collect();
    assert_eq!(positions, [10.0, 20.0, 15.0, 25.0, 30.0, 35.0]);

    assert!(list.remove_widget_fragment(second));
    assert!(!list.remove_widget_fragment(second));
    assert_eq!(list.item_count(), 5);
    assert_eq!(list.items().len(), 5);
}

#[test]
pub(super) fn same_z_overlapping_fragment_items_keep_local_and_cross_fragment_order() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let first = app.world_mut().spawn_empty().id();
    let second = app.world_mut().spawn_empty().id();
    let mut back = rect_item(3, 0.0);
    if let Primitive::RoundedRect { color, .. } = &mut back.primitive {
        *color = Rgba(1.0, 0.0, 0.0, 1.0);
    }
    let mut front = rect_item(3, 0.0);
    if let Primitive::RoundedRect { color, .. } = &mut front.primitive {
        *color = Rgba(0.0, 1.0, 0.0, 1.0);
    }
    {
        let mut list = app.world_mut().resource_mut::<DisplayList>();
        list.upsert_widget_fragment(widget_fragment(
            first,
            10,
            None,
            vec![back.clone(), front.clone()],
        ));
        list.upsert_widget_fragment(widget_fragment(second, 20, None, vec![rect_item(3, 0.0)]));
    }
    app.update();
    let labels = |app: &App| {
        let cache = app.world().resource::<RenderCache>();
        [
            cache.items[&RenderKey::WidgetFragment(first, 0)].local_z,
            cache.items[&RenderKey::WidgetFragment(first, 1)].local_z,
            cache.items[&RenderKey::WidgetFragment(second, 0)].local_z,
        ]
    };
    let initial = labels(&app);
    assert!(initial[0] < initial[1] && initial[1] < initial[2]);

    if let Primitive::RoundedRect { color, .. } = &mut front.primitive {
        *color = Rgba(0.0, 0.0, 1.0, 1.0);
    }
    app.world_mut()
        .resource_mut::<DisplayList>()
        .upsert_widget_fragment(widget_fragment(first, 10, None, vec![back, front]));
    app.update();
    let updated = labels(&app);
    assert!(updated[0] < updated[1] && updated[1] < updated[2]);
    assert_eq!(updated[2], initial[2], "other fragment labels stay stable");
}

#[test]
pub(super) fn one_keyed_fragment_edit_scans_only_its_draw_keys_across_ten_thousand_fragments() {
    const FRAGMENT_COUNT: usize = 10_000;
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let entities: Vec<_> = (0..FRAGMENT_COUNT)
        .map(|_| app.world_mut().spawn_empty().id())
        .collect();
    {
        let mut display_list = app.world_mut().resource_mut::<DisplayList>();
        for (index, entity) in entities.iter().copied().enumerate() {
            display_list.upsert_widget_fragment(widget_fragment(
                entity,
                index as u64 + 1,
                None,
                vec![rect_item(5, index as f32)],
            ));
        }
    }
    app.update();
    assert_eq!(
        app.world().resource::<DisplayList>().item_count(),
        FRAGMENT_COUNT
    );
    assert_eq!(
        app.world().resource::<RenderCache>().items.len(),
        FRAGMENT_COUNT
    );
    assert_eq!(app.world().resource::<RenderSyncStats>().full_syncs, 1);

    let stable_key = RenderKey::WidgetFragment(entities[0], 0);
    let stable_node = cached_single_node(app.world().resource::<RenderCache>(), stable_key);
    let stable_local_z = app.world().resource::<RenderCache>().items[&stable_key].local_z;
    {
        let mut display_list = app.world_mut().resource_mut::<DisplayList>();
        display_list.upsert_widget_fragment(widget_fragment(
            entities[FRAGMENT_COUNT - 1],
            FRAGMENT_COUNT as u64,
            None,
            vec![rect_item(5, -20.0)],
        ));
    }
    app.update();
    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(
        stats.full_syncs, 1,
        "content-only fragment edits stay incremental"
    );
    assert_eq!(stats.incremental_syncs, 1);
    assert_eq!(
        stats.scanned_draw_keys, 2,
        "only the old and new fragment key are visited"
    );
    assert_eq!(stats.updated_render_entries, 2);
    assert_eq!(
        cached_single_node(app.world().resource::<RenderCache>(), stable_key),
        stable_node,
        "unchanged fragments retain their Bevy nodes"
    );
    assert_eq!(
        app.world().resource::<RenderCache>().items[&stable_key].local_z,
        stable_local_z,
        "sparse order labels keep existing siblings in place"
    );

    app.update();
    let idle_stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(idle_stats.scanned_draw_keys, 0);
    assert_eq!(idle_stats.updated_render_entries, 0);

    {
        let mut display_list = app.world_mut().resource_mut::<DisplayList>();
        display_list.upsert_widget_fragment(widget_fragment(
            entities[FRAGMENT_COUNT - 1],
            FRAGMENT_COUNT as u64,
            None,
            vec![rect_item(-5, -20.0), rect_item(5, 2.0), rect_item(8, 3.0)],
        ));
    }
    app.update();
    assert_eq!(app.world().resource::<RenderSyncStats>().full_syncs, 1);
    assert_eq!(
        app.world().resource::<RenderSyncStats>().scanned_draw_keys,
        4
    );
    assert_eq!(
        app.world().resource::<DisplayList>().item_count(),
        FRAGMENT_COUNT + 2
    );
    assert_eq!(
        app.world().resource::<RenderCache>().items
            [&RenderKey::WidgetFragment(entities[FRAGMENT_COUNT - 1], 0,)]
            .item
            .z,
        -5,
        "z changes update the affected fragment's GlobalZIndex"
    );

    let inserted = app.world_mut().spawn_empty().id();
    let old_label = stable_local_z;
    app.world_mut()
        .resource_mut::<DisplayList>()
        .upsert_widget_fragment(widget_fragment(
            inserted,
            0,
            None,
            vec![rect_item(5, 900.0), rect_item(5, 901.0)],
        ));
    app.update();
    assert_eq!(
        app.world().resource::<RenderSyncStats>().scanned_draw_keys,
        2
    );
    assert_eq!(
        app.world().resource::<RenderCache>().items[&stable_key].local_z,
        old_label,
        "inserting before many fragments leaves existing order labels unchanged"
    );
    assert!(
        app.world().resource::<RenderCache>().items[&RenderKey::WidgetFragment(inserted, 0)]
            .local_z
            < old_label
    );

    app.world_mut()
        .resource_mut::<DisplayList>()
        .remove_widget_fragment(entities[FRAGMENT_COUNT - 1]);
    app.update();
    assert_eq!(
        app.world().resource::<RenderSyncStats>().scanned_draw_keys,
        3
    );
    assert_eq!(
        app.world().resource::<DisplayList>().item_count(),
        FRAGMENT_COUNT + 1
    );
    assert!(
        !app.world()
            .resource::<RenderCache>()
            .items
            .contains_key(&RenderKey::WidgetFragment(entities[FRAGMENT_COUNT - 1], 0))
    );
}

#[test]
pub(super) fn keyed_fragment_batch_updates_keep_a_large_graph_in_bounded_gpu_batches() {
    const CURVE_COUNT: usize = 4_000;
    const MAX_BATCH_CURVES: usize = 128;
    let expected_batches = CURVE_COUNT.div_ceil(MAX_BATCH_CURVES);
    let mut app = App::new();
    app.insert_resource(Assets::<BezierMaterial>::default());
    app.insert_resource(Assets::<BezierBatchMaterial>::default());
    app.insert_resource(Assets::<ShaderBuffer>::default());
    app.add_plugins(BbuiRenderPlugin);
    let entity = app.world_mut().spawn_empty().id();
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 1200.0,
        height: 800.0,
    };
    let make_items = |color: Rgba| {
        (0..CURVE_COUNT)
            .map(|index| {
                let x = (index % 100) as f32 * 10.0;
                let y = (index / 100) as f32 * 18.0;
                DrawItem {
                    clip,
                    z: 7,
                    primitive: Primitive::Bezier {
                        points: [
                            [x, y],
                            [x + 30.0, y + 18.0],
                            [x + 50.0, y - 12.0],
                            [x + 80.0, y + 2.0],
                        ],
                        width: 1.5,
                        color,
                    },
                }
            })
            .collect::<Vec<_>>()
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .upsert_widget_fragment(widget_fragment(
            entity,
            0,
            None,
            make_items(Rgba(0.2, 0.75, 0.9, 1.0)),
        ));
    app.update();
    assert_eq!(
        app.world().resource::<RenderCache>().fragment_batches[&entity].len(),
        expected_batches
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
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
    let first_batch_entities: HashSet<_> = query.iter(app.world()).collect();
    assert_eq!(first_batch_entities.len(), expected_batches);

    let batch_keys = app.world().resource::<RenderCache>().fragment_batches[&entity].clone();
    let first_batch_assets: Vec<_> = {
        let cache = app.world().resource::<RenderCache>();
        batch_keys
            .iter()
            .map(|batch_key| {
                let batch = &cache.bezier_batches[batch_key];
                (
                    *batch_key,
                    batch.material.id(),
                    batch.segments.id(),
                    batch.curves.id(),
                    batch.tile_indices.id(),
                )
            })
            .collect()
    };

    let mut updated = make_items(Rgba(0.2, 0.75, 0.9, 1.0));
    updated[CURVE_COUNT / 2].primitive = match updated[CURVE_COUNT / 2].primitive.clone() {
        Primitive::Bezier { points, width, .. } => Primitive::Bezier {
            points,
            width,
            color: Rgba(0.9, 0.1, 0.2, 1.0),
        },
        _ => panic!(),
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .upsert_widget_fragment(widget_fragment(entity, 0, None, updated));
    app.update();
    assert_eq!(
        app.world().resource::<RenderCache>().fragment_batches[&entity].len(),
        expected_batches
    );
    assert_eq!(
        app.world().resource::<RenderCache>().bezier_batches.len(),
        expected_batches
    );
    assert_eq!(app.world().resource::<RenderSyncStats>().full_syncs, 1);
    assert_eq!(
        app.world().resource::<Assets<BezierBatchMaterial>>().len(),
        expected_batches
    );
    assert_eq!(
        app.world().resource::<Assets<ShaderBuffer>>().len(),
        expected_batches * 3
    );
    assert_eq!(
        app.world().resource::<RenderSyncStats>().scanned_draw_keys,
        CURVE_COUNT * 2,
        "replacing one flat fragment rediscovers all old and new draw keys"
    );
    assert_eq!(
        app.world()
            .resource::<RenderSyncStats>()
            .updated_render_entries,
        1,
        "only the bounded batch containing the changed curve needs new GPU assets"
    );
    let cache = app.world().resource::<RenderCache>();
    assert_eq!(cache.fragment_batches[&entity], batch_keys);
    for (batch_key, material, segments, curves, tile_indices) in first_batch_assets {
        let updated_batch = &cache.bezier_batches[&batch_key];
        assert_eq!(updated_batch.material.id(), material);
        assert_eq!(updated_batch.segments.id(), segments);
        assert_eq!(updated_batch.curves.id(), curves);
        assert_eq!(updated_batch.tile_indices.id(), tile_indices);
    }
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<MaterialNode<BezierBatchMaterial>>>();
    let updated_batch_entities: HashSet<_> = query.iter(app.world()).collect();
    assert_eq!(updated_batch_entities, first_batch_entities);
}

#[test]
pub(super) fn bezier_batch_stops_at_global_z_boundary() {
    let mut app = App::new();
    app.insert_resource(Assets::<BezierMaterial>::default());
    app.insert_resource(Assets::<BezierBatchMaterial>::default());
    app.insert_resource(Assets::<ShaderBuffer>::default());
    app.add_plugins(BbuiRenderPlugin);
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 80.0,
    };
    let bezier = |z, offset| DrawItem {
        clip,
        z,
        primitive: Primitive::Bezier {
            points: [
                [offset, 10.0],
                [offset + 20.0, 30.0],
                [offset + 40.0, 20.0],
                [offset + 60.0, 40.0],
            ],
            width: 2.0,
            color: Rgba(0.4, 0.6, 0.8, 1.0),
        },
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![bezier(1, 0.0), bezier(2, 10.0)]);
    app.update();
    let cache = app.world().resource::<RenderCache>();
    assert!(cache.bezier_batches.is_empty());
    assert_eq!(cache.items.len(), 2);
    assert_eq!(cache.items[&RenderKey::WidgetDisplayListIndex(0)].item.z, 1);
    assert_eq!(cache.items[&RenderKey::WidgetDisplayListIndex(1)].item.z, 2);
}

#[test]
pub(super) fn replacing_widget_layer_preserves_manual_items_and_merged_order() {
    let mut list = DisplayList::default();
    list.push(rect_item(5, 1.0));
    list.replace_widget_items(vec![rect_item(2, 2.0), rect_item(8, 3.0)]);
    assert_eq!(
        list.items().iter().map(|item| item.z).collect::<Vec<_>>(),
        [2, 5, 8]
    );
    assert_eq!(list.manual_items().len(), 1);
    assert_eq!(list.widget_items().len(), 2);

    list.replace_widget_items(vec![rect_item(3, 4.0)]);
    assert_eq!(
        list.items().iter().map(|item| item.z).collect::<Vec<_>>(),
        [3, 5]
    );
    assert_eq!(
        list.manual_items()[0].primitive,
        rect_item(5, 1.0).primitive
    );
}

#[test]
pub(super) fn camera_overlay_replacement_preserves_base_camera_items() {
    let camera = Entity::PLACEHOLDER;
    let mut list = DisplayList::default();
    list.replace_camera_items(Some(camera), vec![rect_item(2, 2.0)]);
    list.replace_overlay_camera_items(
        camera,
        vec![DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 80.0,
            },
            z: 2,
            primitive: Primitive::Ellipse {
                rect: ClipRect {
                    x: 8.0,
                    y: 8.0,
                    width: 20.0,
                    height: 20.0,
                },
                color: Rgba(0.9, 0.2, 0.1, 1.0),
            },
        }],
    );
    assert_eq!(list.camera_items(camera).unwrap().len(), 1);
    assert_eq!(list.items().len(), 2);
    assert!(matches!(
        list.items()[1].primitive,
        Primitive::Ellipse { .. }
    ));

    list.replace_overlay_camera_items(camera, Vec::new());
    assert_eq!(list.items().len(), 1);
    assert_eq!(list.camera_items(camera).unwrap().len(), 1);
}

#[test]
pub(super) fn screen_widget_overlay_preserves_base_layers_and_sorts_after_equal_z() {
    let mut list = DisplayList::default();
    list.push(rect_item(3, 1.0));
    list.replace_widget_items(vec![rect_item(3, 2.0)]);
    list.replace_overlay_widget_items(vec![rect_item(3, 3.0)]);

    assert_eq!(list.manual_items().len(), 1);
    assert_eq!(list.widget_items().len(), 1);
    assert_eq!(list.overlay_widget_items().len(), 1);
    assert_eq!(
        list.items()
            .iter()
            .map(|item| match &item.primitive {
                Primitive::RoundedRect { rect, .. } => rect.x,
                _ => panic!(),
            })
            .collect::<Vec<_>>(),
        [1.0, 2.0, 3.0]
    );

    list.replace_overlay_widget_items(vec![rect_item(4, 4.0)]);
    assert_eq!(
        list.manual_items()[0].primitive,
        rect_item(3, 1.0).primitive
    );
    assert_eq!(
        list.widget_items()[0].primitive,
        rect_item(3, 2.0).primitive
    );
    assert_eq!(
        list.overlay_widget_items()[0].primitive,
        rect_item(4, 4.0).primitive
    );
}

#[test]
pub(super) fn image_registry_allocates_unique_ids_and_skips_explicit_registrations() {
    let mut registry = ImageRegistry::default();
    assert_eq!(registry.allocate_id(), Some(1));
    registry.insert(2, Handle::default());
    assert_eq!(registry.allocate_id(), Some(3));
    assert_eq!(registry.allocate_id(), Some(4));
}

#[test]
pub(super) fn ui_paint_uses_layout_geometry_clipping_and_z() {
    let paint = UiPaint(Primitive::RoundedRect {
        rect: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        radius: 6.0,
        color: Rgba(0.3, 0.4, 0.5, 1.0),
    });
    let layout = UiLayoutResult {
        window: Entity::PLACEHOLDER,
        rect: Rect::new(10.0, 20.0, 50.0, 40.0),
        clip: Some(Rect::new(0.0, 30.0, 25.0, 60.0)),
        z: 12,
        content_size: [50.0, 40.0],
        dpi_scale: 2.0,
    };
    let item = paint.draw_item(&layout);
    assert_eq!(item.z, 12);
    assert_eq!(
        item.clip,
        ClipRect {
            x: 10.0,
            y: 30.0,
            width: 15.0,
            height: 30.0,
        }
    );
    let Primitive::RoundedRect { rect, .. } = item.primitive else {
        panic!("expected rounded rectangle");
    };
    assert_eq!(rect, ClipRect::from(layout.rect));

    let ellipse_color = Rgba(0.9, 0.2, 0.1, 1.0);
    let ellipse = UiPaint(Primitive::Ellipse {
        rect: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        color: ellipse_color,
    })
    .draw_item(&layout);
    assert_eq!(
        ellipse.primitive,
        Primitive::Ellipse {
            rect: ClipRect::from(layout.rect),
            color: ellipse_color,
        }
    );
}

#[test]
pub(super) fn gpu_ui_nodes_are_created_and_unchanged_entities_persist() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    app.world_mut()
        .resource_mut::<DisplayList>()
        .push(rect_item(3, 10.0));
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<RenderedItem>>();
    let first = query.single(app.world()).unwrap();
    let node = app.world().get::<Node>(first).unwrap();
    assert_ne!(node.overflow, Overflow::clip());
    assert_eq!(
        app.world().get::<GlobalZIndex>(first),
        Some(&GlobalZIndex(3))
    );
    let cache = app.world().resource::<RenderCache>();
    let group_key = cache.items[&RenderKey::ManualDisplayListIndex(0)]
        .clip_group
        .unwrap();
    let group = cache.clip_groups[&group_key].entity;
    assert_eq!(
        app.world().get::<Node>(group).unwrap().overflow,
        Overflow::clip()
    );

    app.update();
    assert_eq!(query.single(app.world()).unwrap(), first);

    app.world_mut()
        .resource_mut::<DisplayList>()
        .push(rect_item(4, 50.0));
    app.update();
    assert!(app.world().entities().contains(first));
    assert_eq!(query.iter(app.world()).count(), 2);
}

#[test]
pub(super) fn rounded_border_spawns_a_transparent_rounded_ui_node() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let mut border = rect_item(3, 10.0);
    border.primitive = Primitive::RoundedBorder {
        rect: ClipRect {
            x: 10.0,
            y: 4.0,
            width: 30.0,
            height: 18.0,
        },
        radius: 4.0,
        width: 1.0,
        color: Rgba(0.1, 0.2, 0.3, 1.0),
    };
    app.world_mut().resource_mut::<DisplayList>().push(border);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<RenderedItem>>();
    let entity = query.single(app.world()).unwrap();
    let node = app.world().get::<Node>(entity).unwrap();
    assert_eq!(node.border.left, Val::Px(1.0));
    assert_eq!(node.border_radius, BorderRadius::all(Val::Px(4.0)));
    assert!(app.world().get::<BorderColor>(entity).is_some());
    assert_eq!(
        app.world()
            .get::<BackgroundColor>(entity)
            .unwrap()
            .0
            .to_srgba()
            .alpha,
        0.0
    );
}

#[test]
pub(super) fn changed_items_reuse_clip_groups_and_retire_unused_groups() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let mut second_clip = rect_item(1, 40.0);
    second_clip.clip = ClipRect {
        x: 30.0,
        y: 0.0,
        width: 80.0,
        height: 80.0,
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![
            rect_item(0, 10.0),
            second_clip.clone(),
            rect_item(2, 70.0),
        ]);
    app.update();

    let first_group_key = ClipGroupKey::new(rect_item(0, 10.0).clip, None);
    let second_group_key = ClipGroupKey::new(second_clip.clip, None);
    let (first_group, second_group) = {
        let cache = app.world().resource::<RenderCache>();
        assert_eq!(cache.clip_groups.len(), 2);
        (
            cache.clip_groups[&first_group_key].entity,
            cache.clip_groups[&second_group_key].entity,
        )
    };

    let mut changed_second = second_clip;
    if let Primitive::RoundedRect { color, .. } = &mut changed_second.primitive {
        *color = Rgba(0.9, 0.1, 0.2, 1.0);
    }
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![rect_item(0, 10.0), changed_second]);
    app.update();
    {
        let cache = app.world().resource::<RenderCache>();
        assert_eq!(cache.clip_groups.len(), 2);
        assert_eq!(cache.clip_groups[&first_group_key].entity, first_group);
        assert_eq!(cache.clip_groups[&second_group_key].entity, second_group);
    }

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![rect_item(1, 20.0)]);
    app.update();
    let cache = app.world().resource::<RenderCache>();
    assert_eq!(cache.clip_groups.len(), 1);
    assert!(cache.clip_groups.contains_key(&first_group_key));
    assert!(app.world().entities().contains(first_group));
    assert!(!app.world().entities().contains(second_group));
}

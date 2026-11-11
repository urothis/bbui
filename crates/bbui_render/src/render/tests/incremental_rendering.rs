use super::*;

#[test]
pub(super) fn z_order_updates_reuse_unchanged_item_entities() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let first = rect_item(1, 10.0);
    let second = rect_item(2, 50.0);
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![first.clone(), second]);
    app.update();
    let (second_entity, group) = {
        let cache = app.world().resource::<RenderCache>();
        let key = RenderKey::WidgetDisplayListIndex(1);
        (
            cache.items[&key].nodes.first().unwrap(),
            cache.clip_groups[&cache.items[&key].clip_group.unwrap()].entity,
        )
    };
    let second_local_z = *app.world().get::<ZIndex>(second_entity).unwrap();
    assert_eq!(second_local_z, ZIndex(900_000_001));

    let mut moved_first = first;
    moved_first.z = 3;
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![moved_first, rect_item(2, 50.0)]);
    app.update();
    assert!(app.world().entities().contains(second_entity));
    assert_eq!(
        app.world().get::<ZIndex>(second_entity),
        Some(&second_local_z)
    );
    assert_eq!(
        app.world().get::<GlobalZIndex>(second_entity),
        Some(&GlobalZIndex(2))
    );
    assert!(app.world().entities().contains(group));
}

#[test]
pub(super) fn hsv_wheel_uses_one_material_node_and_updates_without_entity_churn() {
    let mut app = App::new();
    app.insert_resource(Assets::<HsvWheelMaterial>::default());
    app.add_plugins(BbuiRenderPlugin);
    let make_item = |hue_degrees, saturation, value| DrawItem {
        clip: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        z: 0,
        primitive: Primitive::HsvWheel {
            rect: ClipRect {
                x: 10.0,
                y: 10.0,
                width: 68.0,
                height: 68.0,
            },
            hue_degrees,
            saturation,
            value,
        },
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![make_item(20.0, 0.4, 0.8)]);
    app.update();

    let (first_entity, handle, first_parameters) = {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &MaterialNode<HsvWheelMaterial>), With<RenderedItem>>();
        let (entity, material_node) = query.single(app.world()).unwrap();
        let parameters = app
            .world()
            .resource::<Assets<HsvWheelMaterial>>()
            .get(&material_node.0)
            .unwrap()
            .parameters;
        (entity, material_node.0.clone(), parameters)
    };
    assert_eq!(first_parameters, Vec4::new(20.0, 0.4, 0.8, 0.0));

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![make_item(200.0, 0.75, 0.45)]);
    app.update();
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &MaterialNode<HsvWheelMaterial>), With<RenderedItem>>();
    let (second_entity, material_node) = query.single(app.world()).unwrap();
    assert_eq!(second_entity, first_entity);
    assert_eq!(material_node.0, handle);
    assert_eq!(
        app.world()
            .resource::<Assets<HsvWheelMaterial>>()
            .get(&handle)
            .unwrap()
            .parameters,
        Vec4::new(200.0, 0.75, 0.45, 0.0)
    );
}

#[test]
pub(super) fn hsv_wheel_material_assets_retire_when_items_leave_the_scene() {
    let mut app = App::new();
    app.insert_resource(Assets::<HsvWheelMaterial>::default());
    app.add_plugins(BbuiRenderPlugin);
    let wheel = DrawItem {
        clip: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        },
        z: 0,
        primitive: Primitive::HsvWheel {
            rect: ClipRect {
                x: 10.0,
                y: 10.0,
                width: 68.0,
                height: 68.0,
            },
            hue_degrees: 20.0,
            saturation: 0.4,
            value: 0.8,
        },
    };

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![wheel.clone()]);
    app.update();
    let initial_handle = app.world().resource::<RenderCache>().items
        [&RenderKey::WidgetDisplayListIndex(0)]
        .hsv_material
        .as_ref()
        .unwrap()
        .clone();
    assert!(
        app.world()
            .resource::<Assets<HsvWheelMaterial>>()
            .get(initial_handle.id())
            .is_some()
    );

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![rect_item(0, 12.0)]);
    app.update();
    assert!(
        app.world()
            .resource::<Assets<HsvWheelMaterial>>()
            .get(initial_handle.id())
            .is_none()
    );

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![wheel]);
    app.update();
    let restored_handle = app.world().resource::<RenderCache>().items
        [&RenderKey::WidgetDisplayListIndex(0)]
        .hsv_material
        .as_ref()
        .unwrap()
        .clone();
    assert!(
        app.world()
            .resource::<Assets<HsvWheelMaterial>>()
            .get(restored_handle.id())
            .is_some()
    );

    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(Vec::new());
    app.update();
    assert!(
        app.world()
            .resource::<Assets<HsvWheelMaterial>>()
            .get(restored_handle.id())
            .is_none()
    );
    assert!(app.world().resource::<RenderCache>().items.is_empty());
}

#[test]
pub(super) fn image_registry_replacement_updates_nodes_without_taking_asset_ownership() {
    let mut app = App::new();
    app.insert_resource(Assets::<Image>::default());
    app.add_plugins(BbuiRenderPlugin);
    let (first_image, second_image) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    app.world_mut()
        .resource_mut::<ImageRegistry>()
        .insert(7, first_image.clone());
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 80.0,
    };
    let image_item = DrawItem {
        clip,
        z: 0,
        primitive: Primitive::Image {
            rect: ClipRect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            },
            asset_id: 7,
        },
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![image_item]);
    app.update();

    let (first_entity, first_node_image) = {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &ImageNode), With<RenderedItem>>();
        let (entity, node) = query.single(app.world()).unwrap();
        (entity, node.image.id())
    };
    assert_eq!(first_node_image, first_image.id());

    app.world_mut()
        .resource_mut::<ImageRegistry>()
        .insert(7, second_image.clone());
    app.update();
    let (second_entity, second_node_image) = {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &ImageNode), With<RenderedItem>>();
        let (entity, node) = query.single(app.world()).unwrap();
        (entity, node.image.id())
    };
    assert_ne!(second_entity, first_entity);
    assert_eq!(second_node_image, second_image.id());
    assert_eq!(
        app.world().resource::<RenderCache>().items[&RenderKey::WidgetDisplayListIndex(0)]
            .image
            .as_ref()
            .unwrap()
            .id(),
        second_image.id()
    );

    app.world_mut().resource_mut::<ImageRegistry>().remove(7);
    app.update();
    let mut query = app
        .world_mut()
        .query_filtered::<Entity, With<RenderedItem>>();
    assert_eq!(query.iter(app.world()).count(), 0);
    assert!(
        app.world().resource::<RenderCache>().items[&RenderKey::WidgetDisplayListIndex(0)]
            .image
            .is_none()
    );
    let images = app.world().resource::<Assets<Image>>();
    assert!(images.get(first_image.id()).is_some());
    assert!(images.get(second_image.id()).is_some());
}

#[test]
pub(super) fn one_image_registry_change_updates_only_its_keyed_fragment() {
    const FRAGMENT_COUNT: usize = 10_000;
    let mut app = App::new();
    app.insert_resource(Assets::<Image>::default());
    app.add_plugins(BbuiRenderPlugin);
    let (first_image, second_image) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    let entities: Vec<_> = (0..FRAGMENT_COUNT)
        .map(|_| app.world_mut().spawn_empty().id())
        .collect();
    let clip = ClipRect {
        x: 0.0,
        y: 0.0,
        width: 120.0,
        height: 80.0,
    };
    for (index, entity) in entities.iter().copied().enumerate() {
        let asset_id = index as u64 + 1;
        app.world_mut()
            .resource_mut::<ImageRegistry>()
            .insert(asset_id, first_image.clone());
        app.world_mut()
            .resource_mut::<DisplayList>()
            .upsert_widget_fragment(widget_fragment(
                entity,
                index as u64,
                None,
                vec![DrawItem {
                    clip,
                    z: 2,
                    primitive: Primitive::Image {
                        rect: ClipRect {
                            x: index as f32,
                            y: 1.0,
                            width: 12.0,
                            height: 12.0,
                        },
                        asset_id,
                    },
                }],
            ));
    }
    app.update();
    let stable_key = RenderKey::WidgetFragment(entities[0], 0);
    let stable_node = cached_single_node(app.world().resource::<RenderCache>(), stable_key);
    let changed_entity = entities[FRAGMENT_COUNT - 1];
    let changed_key = RenderKey::WidgetFragment(changed_entity, 0);
    let initial_node = cached_single_node(app.world().resource::<RenderCache>(), changed_key);

    app.world_mut()
        .resource_mut::<ImageRegistry>()
        .insert(u64::MAX, second_image.clone());
    app.update();
    assert_eq!(
        app.world().resource::<RenderSyncStats>().scanned_draw_keys,
        0
    );

    app.world_mut()
        .resource_mut::<ImageRegistry>()
        .insert(FRAGMENT_COUNT as u64, second_image.clone());
    app.update();
    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(stats.full_syncs, 1);
    assert_eq!(stats.scanned_draw_keys, 2);
    assert_eq!(stats.updated_render_entries, 2);
    assert_eq!(
        cached_single_node(app.world().resource::<RenderCache>(), stable_key),
        stable_node
    );
    let new_node = cached_single_node(app.world().resource::<RenderCache>(), changed_key);
    assert_ne!(new_node, initial_node);
    assert_eq!(
        app.world().get::<ImageNode>(new_node).unwrap().image.id(),
        second_image.id()
    );
    assert_eq!(
        app.world().resource::<RenderCache>().image_users[&(FRAGMENT_COUNT as u64)].len(),
        1
    );
}

#[test]
pub(super) fn repeated_theme_and_clip_changes_keep_render_cache_bounded() {
    let mut app = App::new();
    app.insert_resource(Theme::default());
    app.add_plugins(BbuiRenderPlugin);
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 80.0,
            },
            z: 0,
            primitive: Primitive::Text {
                origin: [4.0, 4.0],
                text: "resize and theme".to_owned(),
                color: Rgba(1.0, 1.0, 1.0, 1.0),
            },
        }]);
    app.update();

    for step in 0..24 {
        let width = 100.0 + step as f32;
        app.world_mut()
            .resource_mut::<DisplayList>()
            .replace_widget_items(vec![DrawItem {
                clip: ClipRect {
                    x: step as f32,
                    y: 0.0,
                    width,
                    height: 70.0 + step as f32,
                },
                z: 0,
                primitive: Primitive::Text {
                    origin: [4.0, 4.0],
                    text: "resize and theme".to_owned(),
                    color: Rgba(1.0, 1.0, 1.0, 1.0),
                },
            }]);
        {
            let mut theme = app.world_mut().resource_mut::<Theme>();
            theme.font_size = 14.0 + (step % 3) as f32;
            theme.line_height = 1.25 + (step % 2) as f32 * 0.1;
        }
        app.update();

        let cache = app.world().resource::<RenderCache>();
        assert_eq!(cache.items.len(), 1);
        assert_eq!(cache.clip_groups.len(), 1);
        let rendered_items = {
            let mut query = app
                .world_mut()
                .query_filtered::<Entity, With<RenderedItem>>();
            query.iter(app.world()).count()
        };
        let rendered_clip_groups = {
            let mut query = app
                .world_mut()
                .query_filtered::<Entity, With<RenderedClipGroup>>();
            query.iter(app.world()).count()
        };
        assert_eq!(rendered_items, 1);
        assert_eq!(rendered_clip_groups, 1);
    }
}

#[test]
pub(super) fn theme_font_metrics_drive_text_nodes_and_invalidate_changed_text() {
    let mut app = App::new();
    app.insert_resource(Theme::default());
    app.add_plugins(BbuiRenderPlugin);
    app.world_mut()
        .resource_mut::<DisplayList>()
        .push(DrawItem {
            clip: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 120.0,
                height: 80.0,
            },
            z: 1,
            primitive: Primitive::Text {
                origin: [4.0, 4.0],
                text: "theme size".to_owned(),
                color: Rgba(1.0, 1.0, 1.0, 1.0),
            },
        });
    app.update();

    let (first_entity, first_size, first_line_height) = {
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &TextFont, &LineHeight), With<Text>>();
        let (entity, font, line_height) = query.single(app.world()).unwrap();
        (entity, font.font_size, *line_height)
    };
    assert_eq!(first_size, bevy::text::FontSize::Px(14.0));
    assert_eq!(first_line_height, LineHeight::RelativeToFont(1.25));
    let default_font = app.world().get::<TextFont>(first_entity).unwrap();
    assert!(matches!(
        default_font.font,
        bevy::text::FontSource::List(ref faces)
            if matches!(faces.as_slice(),
                [bevy::text::FontSource::Family(primary), bevy::text::FontSource::Family(_)]
                if primary == UI_DEFAULT_FONT_FAMILY)
    ));

    {
        let mut theme = app.world_mut().resource_mut::<Theme>();
        theme.font_size = 20.0;
        theme.line_height = 1.5;
    }
    app.update();
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &TextFont, &LineHeight), With<Text>>();
    let (second_entity, second_font, second_line_height) = query.single(app.world()).unwrap();
    assert_eq!(second_font.font_size, bevy::text::FontSize::Px(20.0));
    assert_eq!(*second_line_height, LineHeight::RelativeToFont(1.5));
    assert_ne!(first_entity, second_entity);
    app.world_mut().resource_mut::<Theme>().line_height = 1.7;
    app.update();
    let mut query = app
        .world_mut()
        .query_filtered::<(Entity, &LineHeight), With<Text>>();
    let (third_entity, third_line_height) = query.single(app.world()).unwrap();
    assert_eq!(*third_line_height, LineHeight::RelativeToFont(1.7));
    assert_ne!(second_entity, third_entity);
}

#[test]
pub(super) fn same_z_parent_background_is_ordered_under_nested_widget_text() {
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let rect = Rect::new(0.0, 0.0, 180.0, 32.0);
    let parent = app
        .world_mut()
        .spawn((
            bbui_core::UiNode,
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect::from(rect),
                radius: 0.0,
                color: Rgba(0.02, 0.03, 0.04, 1.0),
            }),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect,
                clip: None,
                z: 0,
                content_size: [rect.width, rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let child_rect = Rect::new(8.0, 4.0, 164.0, 24.0);
    let child = app
        .world_mut()
        .spawn((
            bbui_core::UiNode,
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect::from(child_rect),
                radius: 2.0,
                color: Rgba(0.08, 0.09, 0.10, 1.0),
            }),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: child_rect,
                clip: None,
                z: 0,
                content_size: [child_rect.width, child_rect.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    bbui_core::set_parent(app.world_mut(), child, parent)
        .expect("the label paint belongs to the background container");
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_widget_items(vec![DrawItem {
            clip: ClipRect::from(rect),
            z: 0,
            primitive: Primitive::TextWrapped {
                origin: [4.0, 4.0],
                text: "same layer label".to_owned(),
                color: Rgba(0.95, 0.95, 0.95, 1.0),
                max_width: rect.width - 8.0,
                wrap: false,
                style: None,
            },
        }]);
    app.update();

    let cache = app.world().resource::<RenderCache>();
    let rendered_entity = |key| {
        let cached = &cache.items[&key];
        match &cached.nodes {
            RenderedNodes::Single(entity) => *entity,
            RenderedNodes::Multiple(entities) => {
                *entities.first().expect("rendered item has a Bevy entity")
            }
            RenderedNodes::Empty => panic!("rendered item has a Bevy entity"),
        }
    };
    let parent_paint = rendered_entity(RenderKey::LayoutEntity(parent));
    let child_paint = rendered_entity(RenderKey::LayoutEntity(child));
    let widget_text = rendered_entity(RenderKey::WidgetDisplayListIndex(0));
    let parent_z = (
        *app.world().get::<GlobalZIndex>(parent_paint).unwrap(),
        *app.world().get::<ZIndex>(parent_paint).unwrap(),
    );
    let child_z = (
        *app.world().get::<GlobalZIndex>(child_paint).unwrap(),
        *app.world().get::<ZIndex>(child_paint).unwrap(),
    );
    let text_z = (
        *app.world().get::<GlobalZIndex>(widget_text).unwrap(),
        *app.world().get::<ZIndex>(widget_text).unwrap(),
    );
    assert_eq!(parent_z.0, child_z.0);
    assert_eq!(child_z.0, text_z.0);
    assert!(
        parent_z.1.0 < child_z.1.0,
        "parent paint must precede nested child paint, got parent={:?}, child={:?}",
        parent_z.1,
        child_z.1
    );
    assert!(
        child_z.1.0 < text_z.1.0,
        "nested paint must precede child widget text, got child={:?}, text={:?}",
        child_z.1,
        text_z.1
    );
}

#[test]
pub(super) fn reparenting_invalidates_only_the_rendered_subtree() {
    const STATIC_COUNT: usize = 256;
    let mut app = App::new();
    app.add_plugins(BbuiRenderPlugin);
    let source_parent = app.world_mut().spawn(bbui_core::UiNode).id();
    let destination_root = app.world_mut().spawn(bbui_core::UiNode).id();
    let destination_middle = app.world_mut().spawn(bbui_core::UiNode).id();
    let destination_parent = app.world_mut().spawn(bbui_core::UiNode).id();
    bbui_core::set_parent(app.world_mut(), destination_middle, destination_root).unwrap();
    bbui_core::set_parent(app.world_mut(), destination_parent, destination_middle).unwrap();

    let make_painted = |world: &mut bevy::prelude::World| {
        world
            .spawn((
                bbui_core::UiNode,
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
                    window: Entity::PLACEHOLDER,
                    rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                    clip: None,
                    z: 0,
                    content_size: [20.0, 20.0],
                    dpi_scale: 1.0,
                },
            ))
            .id()
    };
    let moving = make_painted(app.world_mut());
    let descendant = make_painted(app.world_mut());
    bbui_core::set_parent(app.world_mut(), descendant, moving).unwrap();
    bbui_core::set_parent(app.world_mut(), moving, source_parent).unwrap();
    let static_entities: Vec<_> = (0..STATIC_COUNT)
        .map(|_| make_painted(app.world_mut()))
        .collect();

    app.update();
    let moving_key = RenderKey::LayoutEntity(moving);
    let descendant_key = RenderKey::LayoutEntity(descendant);
    let stable_key = RenderKey::LayoutEntity(static_entities[0]);
    let (moving_node, descendant_node, stable_node, original_depth_order) = {
        let cache = app.world().resource::<RenderCache>();
        (
            cached_single_node(cache, moving_key),
            cached_single_node(cache, descendant_key),
            cached_single_node(cache, stable_key),
            (
                cache.layout_entity_order_keys[&moving].depth,
                cache.layout_entity_order_keys[&descendant].depth,
            ),
        )
    };
    assert_eq!(original_depth_order, (1, 2));

    bbui_core::set_parent(app.world_mut(), moving, destination_parent).unwrap();
    app.update();

    let stats = *app.world().resource::<RenderSyncStats>();
    assert_eq!(stats.full_syncs, 1, "a reparent stays scoped");
    assert_eq!(stats.incremental_syncs, 1);
    assert_eq!(
        stats.scanned_draw_keys, 2,
        "only the moved subtree is visited"
    );
    let cache = app.world().resource::<RenderCache>();
    assert_eq!(
        cache.parents_by_entity.get(&moving),
        Some(&destination_parent)
    );
    assert_eq!(cache.layout_entity_order_keys[&moving].depth, 3);
    assert_eq!(cache.layout_entity_order_keys[&descendant].depth, 4);
    assert_eq!(cached_single_node(cache, moving_key), moving_node);
    assert_eq!(cached_single_node(cache, descendant_key), descendant_node);
    assert_eq!(cached_single_node(cache, stable_key), stable_node);
}

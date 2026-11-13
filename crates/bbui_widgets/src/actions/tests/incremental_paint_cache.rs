use super::*;

#[test]
pub(super) fn background_transition_recolors_retained_checkbox_text() {
    use bbui_core::CorePlugin;
    use bbui_layout::LayoutPlugin;
    use bbui_style::{StylePlugin, UiAnimationSpec, UiAutoAnimations};
    use bbui_text::TextPlugin;
    use bevy_time::Time;
    use std::time::Duration;

    let mut app = App::new();
    app.add_plugins((
        CorePlugin,
        StylePlugin,
        LayoutPlugin,
        TextPlugin,
        WidgetPlugin,
    ));
    app.insert_resource(Time::<()>::default());
    let mut transitions = UiAutoAnimations::default();
    transitions.set(
        UiStyleProperty::Background,
        UiAnimationSpec::new(Duration::from_millis(1_200)),
    );
    let entity = app
        .world_mut()
        .spawn((
            UiNode,
            UiEnabled(true),
            UiVisible(true),
            UiStyleState::default(),
            Checkbox::new("static label", false),
            transitions,
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let assert_matches_full_paint = |app: &App| {
        let world = app.world();
        let layout = world.get::<UiLayoutResult>(entity).unwrap();
        let mut checkbox = world.get::<Checkbox>(entity).unwrap().clone();
        checkbox.enabled &= is_interactable(world, entity);
        let theme = styled_theme(world, entity, world.resource::<Theme>());
        let expected = checkbox.draw_items_clipped(layout.rect, layout.clip, &theme, layout.z);
        assert_eq!(
            world.resource::<DisplayList>().widget_fragments()[&entity].items,
            expected
        );
    };
    for _ in 0..3 {
        app.update();
    }
    set_ui_style_state(app.world_mut(), entity, UiStyleState::SELECTED, true);
    app.update();
    assert!(
        app.world()
            .resource::<bbui_motion::MotionPlayer>()
            .is_animating(
                entity,
                bbui_style::style_motion_binding(UiStyleProperty::Background).id(),
            )
    );
    let previous = app.world().resource::<DisplayList>().widget_fragments()[&entity]
        .items
        .clone();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(16));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        1
    );
    let current = &app.world().resource::<DisplayList>().widget_fragments()[&entity].items;
    assert_ne!(current[0], previous[0]);
    assert_eq!(current[1], previous[1]);
    assert_matches_full_paint(&app);
    let text_pointer = match &current[1].primitive {
        Primitive::Text { text, .. } => text.as_ptr(),
        _ => panic!("checkbox label must remain text"),
    };

    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(16));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        1
    );
    let current = &app.world().resource::<DisplayList>().widget_fragments()[&entity].items;
    let next_text_pointer = match &current[1].primitive {
        Primitive::Text { text, .. } => text.as_ptr(),
        _ => panic!("checkbox label must remain text"),
    };
    assert_eq!(
        next_text_pointer, text_pointer,
        "color ticks reuse the label allocation"
    );

    app.world_mut().get_mut::<Checkbox>(entity).unwrap().label = "new label".into();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(16));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        0
    );
    let current = &app.world().resource::<DisplayList>().widget_fragments()[&entity].items;
    assert!(matches!(&current[1].primitive, Primitive::Text { text, .. } if text == "new label"));

    app.world_mut()
        .get_mut::<UiLayoutResult>(entity)
        .unwrap()
        .rect
        .x = 15.0;
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(16));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        0
    );
    let current = &app.world().resource::<DisplayList>().widget_fragments()[&entity].items;
    assert!(matches!(current[0].primitive, Primitive::RoundedRect { rect, .. } if rect.x == 15.0));

    app.world_mut().resource_mut::<Theme>().text = Rgba(0.1, 0.8, 0.2, 1.0);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(16));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        0
    );
    let current = &app.world().resource::<DisplayList>().widget_fragments()[&entity].items;
    assert!(
        matches!(current[1].primitive, Primitive::Text { color, .. } if color == Rgba(0.1, 0.8, 0.2, 1.0))
    );

    for value in [CheckboxValue::On, CheckboxValue::Mixed] {
        app.world_mut().get_mut::<Checkbox>(entity).unwrap().value = value;
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.update();
        assert_eq!(
            app.world()
                .resource::<WidgetPaintCache>()
                .recolored_fragments,
            1
        );
        assert_matches_full_paint(&app);
    }

    app.world_mut()
        .resource_mut::<UiDirty>()
        .mark(UiDirtyDomain::Paint, entity);
    collect_widget_paint(app.world_mut());
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        0,
        "a later explicit paint request cannot reuse a previous transition tick"
    );
    assert_matches_full_paint(&app);

    app.world_mut()
        .entity_mut(entity)
        .insert(UiRole::new("filled-checkbox"));
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(16));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .recolored_fragments,
        0
    );
}

#[test]
pub(super) fn widget_paint_cache_skips_idle_and_invalidates_visual_and_layout_inputs() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let entity = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("cached", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(1.0, 2.0, 120.0, 24.0),
                clip: None,
                z: 3,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();

    app.update();
    let initial = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert!(!initial.is_empty());
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 1);

    app.update();
    assert_eq!(app.world().resource::<WidgetPaintCache>().skipped_frames, 1);
    assert_eq!(
        app.world().resource::<DisplayList>().widget_items(),
        initial
    );

    app.world_mut()
        .get_mut::<Checkbox>(entity)
        .unwrap()
        .toggle();
    app.update();
    let checked = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        checked, initial,
        "a value mutation repaints in the same frame"
    );
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 2);

    app.world_mut().resource_mut::<Theme>().surface = Rgba(0.12, 0.34, 0.56, 1.0);
    app.update();
    let themed = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        themed, checked,
        "theme changes invalidate widget paint immediately"
    );
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 3);

    app.world_mut()
        .get_mut::<UiLayoutResult>(entity)
        .unwrap()
        .rect
        .x += 16.0;
    app.update();
    let moved = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert_ne!(
        moved, themed,
        "layout changes repaint before the frame is presented"
    );
    assert_eq!(app.world().resource::<WidgetPaintCache>().rebuilt_frames, 4);
}

#[test]
pub(super) fn one_changed_checkbox_repaints_only_its_fragment() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let first = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("first", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("second", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(0.0, 30.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();
    let fragment_orders: HashMap<_, _> = app
        .world()
        .resource::<DisplayList>()
        .widget_fragments()
        .values()
        .map(|fragment| (fragment.entity, fragment.order))
        .collect();
    let overlay = item(
        ClipRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        },
        9,
        Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            radius: 2.0,
            color: Rgba(0.2, 0.3, 0.4, 1.0),
        },
    );
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_overlay_widget_items(vec![overlay.clone()]);
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .repainted_entities,
        2
    );
    app.world_mut().get_mut::<Checkbox>(first).unwrap().toggle();
    app.world_mut().resource_mut::<WidgetPaintCache>().prepared = true;
    app.world_mut()
        .resource_mut::<UiDirty>()
        .mark(UiDirtyDomain::Paint, first);
    collect_widget_paint(app.world_mut());
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(cache.repainted_entities, 1);
    assert_eq!(
        cache.published_fragments, 1,
        "one dirty control publishes only its own changed fragment"
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&first)
            .map(|fragment| fragment.order),
        fragment_orders.get(&first).copied()
    );
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&second)
            .map(|fragment| fragment.order),
        fragment_orders.get(&second).copied(),
        "updating one entity keeps its peers at their existing paint ranks"
    );
    assert_eq!(
        app.world().resource::<DisplayList>().overlay_widget_items(),
        &[overlay],
        "per-entity widget updates preserve the post-widget overlay layer"
    );
}

#[test]
pub(super) fn widget_fragment_camera_migration_and_removal_update_only_that_entity() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let first_camera = app.world_mut().spawn(bevy_camera::Camera::default()).id();
    let second_camera = app.world_mut().spawn_empty().id();
    let migrating = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("migrate", false),
            UiLayoutResult {
                window: first_camera,
                rect: UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z: 2,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut().spawn((
        UiNode,
        Checkbox::new("stays", false),
        UiLayoutResult {
            window: first_camera,
            rect: UiRect::new(0.0, 30.0, 120.0, 24.0),
            clip: None,
            z: 1,
            content_size: [120.0, 24.0],
            dpi_scale: 1.0,
        },
    ));

    app.update();
    app.world_mut()
        .get_mut::<UiLayoutResult>(migrating)
        .unwrap()
        .window = second_camera;
    app.update();

    let has_label = |items: &[DrawItem], label: &str| {
        items
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::Text { text, .. } if text == label))
    };
    let display_list = app.world().resource::<DisplayList>();
    assert!(
        display_list
            .camera_items(first_camera)
            .is_some_and(|items| has_label(items, "stays"))
    );
    assert!(has_label(display_list.widget_items(), "migrate"));
    assert!(
        display_list
            .camera_items(second_camera)
            .is_none_or(|items| items.is_empty())
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "moving to a window without a camera updates the same entity key"
    );

    app.world_mut()
        .entity_mut(second_camera)
        .insert(bevy_camera::Camera::default());
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(
        display_list
            .camera_items(second_camera)
            .is_some_and(|items| has_label(items, "migrate"))
    );
    assert!(!has_label(display_list.widget_items(), "migrate"));
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "adding a camera retargets associated widget fragments"
    );

    app.world_mut()
        .entity_mut(second_camera)
        .remove::<bevy_camera::Camera>();
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(has_label(display_list.widget_items(), "migrate"));
    assert!(
        display_list
            .camera_items(second_camera)
            .is_none_or(|items| items.is_empty())
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "removing a camera retargets associated widget fragments to the screen"
    );

    app.world_mut().entity_mut(migrating).despawn();
    app.update();
    let display_list = app.world().resource::<DisplayList>();
    assert!(
        display_list
            .camera_items(first_camera)
            .is_some_and(|items| has_label(items, "stays"))
    );
    assert!(
        !has_label(display_list.widget_items(), "migrate"),
        "despawning a screen-migrated control removes its cached fragment"
    );
    assert_eq!(
        app.world()
            .resource::<WidgetPaintCache>()
            .published_fragments,
        1,
        "despawning a control removes only its cached fragment"
    );
}

#[test]
pub(super) fn topology_edits_repaint_only_new_or_reordered_widget_fragments() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let first_group = app.world_mut().spawn((UiNode, UiChildren::default())).id();
    let second_group = app.world_mut().spawn((UiNode, UiChildren::default())).id();
    let spawn_child = |world: &mut World, parent: Entity, label: &str, y: f32| {
        world
            .spawn((
                UiNode,
                UiParent(parent),
                Checkbox::new(label, false),
                UiLayoutResult {
                    window: Entity::PLACEHOLDER,
                    rect: UiRect::new(0.0, y, 120.0, 24.0),
                    clip: None,
                    z: 1,
                    content_size: [120.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id()
    };
    let first = spawn_child(app.world_mut(), first_group, "first", 0.0);
    let second = spawn_child(app.world_mut(), first_group, "second", 30.0);
    let third = spawn_child(app.world_mut(), second_group, "third", 60.0);
    let fourth = spawn_child(app.world_mut(), first_group, "fourth", 45.0);
    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0 = vec![first, second, fourth];
    app.world_mut()
        .get_mut::<UiChildren>(second_group)
        .unwrap()
        .0 = vec![third];
    for index in 0..10_000 {
        app.world_mut().spawn((
            UiNode,
            Checkbox::new("peer", false),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(200.0, index as f32 * 24.0, 20.0, 20.0),
                clip: None,
                z: 1,
                content_size: [20.0, 20.0],
                dpi_scale: 1.0,
            },
        ));
    }
    app.update();

    let added = spawn_child(app.world_mut(), first_group, "added", 90.0);
    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0
        .push(added);
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 1,
        "mounting a widget paints only it"
    );
    assert_eq!(cache.published_fragments, 1);
    let order = |world: &World, entity| {
        world
            .resource::<DisplayList>()
            .widget_fragments()
            .get(&entity)
            .unwrap()
            .order
    };
    assert!(order(app.world(), first) < order(app.world(), second));
    assert!(order(app.world(), second) < order(app.world(), added));
    assert!(order(app.world(), fourth) < order(app.world(), added));
    assert!(
        order(app.world(), added) < order(app.world(), third),
        "added rank {} should precede second group rank {}",
        order(app.world(), added),
        order(app.world(), third)
    );

    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0 = vec![added, first, second];
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 1,
        "sibling reorder repaints the moved rank"
    );
    assert_eq!(cache.published_fragments, 1);
    assert!(order(app.world(), added) < order(app.world(), first));
    assert!(order(app.world(), first) < order(app.world(), second));

    app.world_mut()
        .get_mut::<UiChildren>(first_group)
        .unwrap()
        .0 = vec![second, fourth, added, first];
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 2,
        "a multi-sibling reorder republishes both moved fragments"
    );
    assert_eq!(cache.published_fragments, 2);
    assert!(order(app.world(), second) < order(app.world(), fourth));
    assert!(order(app.world(), fourth) < order(app.world(), added));
    assert!(order(app.world(), added) < order(app.world(), first));

    bbui_core::set_parent(app.world_mut(), second, second_group).unwrap();
    app.update();
    let cache = app.world().resource::<WidgetPaintCache>();
    assert_eq!(
        cache.repainted_entities, 1,
        "reparenting repaints the moved subtree only"
    );
    assert_eq!(cache.published_fragments, 1);
    assert!(order(app.world(), fourth) < order(app.world(), added));
    assert!(order(app.world(), added) < order(app.world(), first));
    assert!(order(app.world(), first) < order(app.world(), third));
    assert!(order(app.world(), third) < order(app.world(), second));
    assert_eq!(
        app.world()
            .resource::<DisplayList>()
            .widget_fragments()
            .len(),
        10_005,
        "unaffected peers retain their fragments"
    );
}

use super::*;

#[test]
pub(super) fn widget_paint_collector_uses_layout_bounds_and_current_theme() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let manual = DrawItem {
        clip: ClipRect {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        },
        z: -1,
        primitive: Primitive::Text {
            origin: [0.0, 0.0],
            text: "manual".into(),
            color: Rgba(1.0, 1.0, 1.0, 1.0),
        },
    };
    app.world_mut()
        .resource_mut::<DisplayList>()
        .push(manual.clone());
    let entity = app
        .world_mut()
        .spawn((
            UiNode,
            Checkbox::new("visible", true),
            UiLayoutResult {
                window: Entity::PLACEHOLDER,
                rect: UiRect::new(1.0, 2.0, 120.0, 24.0),
                clip: Some(UiRect::new(10.0, 0.0, 80.0, 30.0)),
                z: 7,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.update();
    let items = app.world().resource::<DisplayList>().items().to_vec();
    assert!(!items.is_empty());
    assert!(items.contains(&manual));
    assert!(items.len() > 1);
    let widget_items = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .to_vec();
    assert!(widget_items.iter().all(|item| item.z >= 7));
    assert!(
        widget_items
            .iter()
            .all(|item| item.clip.x == 10.0 && item.clip.width == 80.0)
    );
    app.world_mut().entity_mut(entity).despawn();
}

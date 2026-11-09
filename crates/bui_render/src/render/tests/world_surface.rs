use super::*;

#[test]
pub(super) fn explicit_camera_display_items_attach_the_requested_camera() {
    let mut app = App::new();
    app.add_plugins(BuiRenderPlugin);
    let camera = app.world_mut().spawn(Camera::default()).id();
    app.world_mut()
        .resource_mut::<DisplayList>()
        .replace_camera_items(Some(camera), vec![rect_item(7, 20.0)]);
    app.update();

    let cache = app.world().resource::<RenderCache>();
    let group_key = cache.items[&RenderKey::CameraDisplayListIndex(camera, 0)]
        .clip_group
        .unwrap();
    let group = cache.clip_groups[&group_key].entity;
    assert_eq!(app.world().get::<UiTargetCamera>(group).unwrap().0, camera);
}

#[test]
pub(super) fn world_surface_maps_center_and_rejects_half_open_edges() {
    let surface = UiWorldSurface {
        target_camera: Entity::PLACEHOLDER,
        texture_asset_id: 9,
        world_size: Vec2::new(2.0, 1.0),
        resolution: UVec2::new(400, 100),
    };
    assert_eq!(surface.local_to_ui(Vec2::ZERO), Some([200.0, 50.0]));
    assert_eq!(surface.local_to_ui(Vec2::new(-1.0, 0.5)), Some([0.0, 0.0]));
    assert_eq!(surface.local_to_ui(Vec2::new(1.0, 0.0)), None);
    assert_eq!(surface.local_to_ui(Vec2::new(0.0, -0.5)), None);
}

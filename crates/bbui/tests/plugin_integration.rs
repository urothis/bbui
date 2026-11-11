use bbui::{
    BbuiPlugin, UiCrosshairPointer, WorldPointerState,
    widgets::{WidgetNode, WidgetTree},
};
use bbui_core::{
    UiChildren, UiFocus, UiHitBox, UiInputEvent, UiInputQueue, UiKeyCode, UiModifiers,
    UiPointerButton, UiRoutedEvent, UiRoutedEvents,
};
use bbui_layout::{Rect, UiAlign, UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot};
use bbui_render::{DisplayList, Primitive, UiPaint, UiWorldSurface};
use bbui_text::{TextBuffer, UiTextLayout};
use bbui_widgets::{
    Button, ClipboardRequestQueue, CursorCaptureRequest, CursorCaptureRequestQueue, TextField,
    WidgetEvent, WidgetEventQueue,
};
use bevy::{
    a11y::AccessibilityNode,
    asset::{AssetPlugin, Assets},
    camera::{
        Camera, CameraProjection, OrthographicProjection, RenderTarget, RenderTargetInfo,
        primitives::Aabb, visibility::ViewVisibility,
    },
    clipboard::ClipboardPlugin,
    input::{
        ButtonState, InputPlugin,
        keyboard::{KeyCode, KeyboardInput},
    },
    math::UVec2,
    mesh::Mesh3d,
    prelude::{
        App, BackgroundColor, Cuboid, GlobalTransform, ImagePlugin, InheritedVisibility, Mesh,
        MinimalPlugins, Node, Text, Transform, Update, Vec2, Vec3, Window,
    },
    text::{Font, TextBounds, TextFont, load_font_assets_into_font_collection},
    window::{CursorMoved, CursorOptions, Ime, WindowPlugin, WindowRef},
};

struct Scene {
    window: bevy::prelude::Entity,
    root: bevy::prelude::Entity,
    button: bevy::prelude::Entity,
    text_field: bevy::prelude::Entity,
}

struct WorldScene {
    physical_window: bevy::prelude::Entity,
    logical_window: bevy::prelude::Entity,
    view_transform: GlobalTransform,
    text_field: bevy::prelude::Entity,
}

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(WindowPlugin {
            primary_window: None,
            ..Default::default()
        })
        .add_plugins((ImagePlugin::default(), InputPlugin, ClipboardPlugin))
        .add_plugins(BbuiPlugin);
    app
}

#[cfg(test)]
fn spawn_scene(app: &mut App) -> Scene {
    let mut window = Window::default();
    window.resolution.set(400.0, 200.0);
    let window = app.world_mut().spawn(window).id();
    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::button("Run").with_key("run"),
        WidgetNode::text_field("Name", "draft").with_key("name"),
    ]));
    let root = tree.spawn(app.world_mut(), None).unwrap();
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(
            window,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 200.0,
            },
            1.0,
        ),
        UiPaint(Primitive::RoundedRect {
            rect: bbui_render::ClipRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            radius: 0.0,
            color: bbui_style::Theme::default().background,
        }),
    ));
    let children = app.world().get::<UiChildren>(root).unwrap().0.clone();
    Scene {
        window,
        root,
        button: children[0],
        text_field: children[1],
    }
}

#[test]
fn accessibility_tracks_direct_control_edits_and_removal_in_the_same_update() {
    let mut app = test_app();
    let scene = spawn_scene(&mut app);
    app.update();
    assert!(app.world().get::<AccessibilityNode>(scene.button).is_some());

    app.world_mut()
        .get_mut::<Button>(scene.button)
        .unwrap()
        .label = "Run again".into();
    app.update();
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(scene.button)
            .unwrap()
            .0
            .label(),
        Some("Run again")
    );

    app.world_mut().entity_mut(scene.button).remove::<Button>();
    app.update();
    assert!(app.world().get::<AccessibilityNode>(scene.button).is_none());
}

#[cfg(test)]
fn center(app: &App, entity: bevy::prelude::Entity) -> Vec2 {
    let rect = app.world().get::<UiLayoutResult>(entity).unwrap().rect;
    Vec2::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

#[cfg(test)]
fn spawn_world_scene(app: &mut App) -> WorldScene {
    let mut window = Window::default();
    window.resolution.set(400.0, 200.0);
    let physical_window = app
        .world_mut()
        .spawn((window, CursorOptions::default()))
        .id();
    let logical_window = app.world_mut().spawn_empty().id();

    let mut projection = OrthographicProjection::default_3d();
    projection.update(400.0, 200.0);
    let view_transform =
        GlobalTransform::from(Transform::from_xyz(0.0, 0.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y));
    let mut camera = Camera::default();
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: UVec2::new(400, 200),
        scale_factor: 1.0,
    });
    camera.computed.clip_from_view = projection.get_clip_from_view();
    app.world_mut().spawn((
        camera,
        RenderTarget::Window(WindowRef::Entity(physical_window)),
        view_transform,
    ));

    let root = WidgetTree::new(WidgetNode::text_field("Name", "draft"))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(root).insert(UiLayoutRoot::new(
        logical_window,
        Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 200.0,
        },
        1.0,
    ));
    app.world_mut().spawn((
        UiWorldSurface {
            target_camera: logical_window,
            texture_asset_id: 1,
            world_size: Vec2::new(400.0, 200.0),
            resolution: UVec2::new(400, 200),
        },
        GlobalTransform::IDENTITY,
    ));

    WorldScene {
        physical_window,
        logical_window,
        view_transform,
        text_field: app
            .world()
            .get::<UiChildren>(root)
            .map_or(root, |children| children.0[0]),
    }
}

#[cfg(test)]
fn screen_position_for_surface_ui(scene: &WorldScene, ui_position: Vec2) -> Vec2 {
    let local_world = Vec3::new(ui_position.x - 200.0, 100.0 - ui_position.y, 0.0);
    // The synthetic camera has a 1:1 orthographic mapping to this world surface.
    // Calling the camera conversion keeps this test correct if its projection setup changes.
    let mut projection = OrthographicProjection::default_3d();
    projection.update(400.0, 200.0);
    let mut camera = Camera::default();
    camera.computed.target_info = Some(RenderTargetInfo {
        physical_size: UVec2::new(400, 200),
        scale_factor: 1.0,
    });
    camera.computed.clip_from_view = projection.get_clip_from_view();
    camera
        .world_to_viewport(&scene.view_transform, local_world)
        .unwrap()
}

#[test]
fn world_surface_accesskit_bounds_follow_the_mesh_and_view_camera() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();
    let layout = app
        .world()
        .get::<UiLayoutResult>(scene.text_field)
        .unwrap()
        .rect;
    let expected = screen_position_for_surface_ui(&scene, Vec2::new(layout.x, layout.y));
    let initial = app
        .world()
        .get::<AccessibilityNode>(scene.text_field)
        .unwrap()
        .0
        .bounds()
        .unwrap();
    assert!((initial.x0 - f64::from(expected.x)).abs() < 1.0);
    assert!((initial.y0 - f64::from(expected.y)).abs() < 1.0);

    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(surface)
        .insert(GlobalTransform::from_translation(Vec3::new(30.0, 0.0, 0.0)));
    app.update();
    let moved = app
        .world()
        .get::<AccessibilityNode>(scene.text_field)
        .unwrap()
        .0
        .bounds()
        .unwrap();
    assert!((moved.x0 - initial.x0 - 30.0).abs() < 1.0);

    let camera = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Camera>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(5.0, 0.0, 10.0)));
    app.update();
    let viewed = app
        .world()
        .get::<AccessibilityNode>(scene.text_field)
        .unwrap()
        .0
        .bounds()
        .unwrap();
    assert!((viewed.x0 - moved.x0 + 5.0).abs() < 1.0);

    app.world_mut()
        .entity_mut(surface)
        .remove::<UiWorldSurface>();
    app.update();
    assert!(
        app.world()
            .get::<AccessibilityNode>(scene.text_field)
            .unwrap()
            .0
            .is_hidden()
    );
}

#[test]
fn plugin_lays_out_widgets_collects_paint_and_spawns_bevy_ui_nodes() {
    let mut app = test_app();
    let scene = spawn_scene(&mut app);
    app.update();

    let root_layout = app.world().get::<UiLayoutResult>(scene.root).unwrap();
    assert_eq!(root_layout.window, scene.window);
    assert_eq!(root_layout.rect.width, 400.0);
    assert_eq!(root_layout.rect.height, 200.0);
    let button_hit_box = app
        .world()
        .get::<bbui_core::UiHitBox>(scene.button)
        .unwrap();
    assert_eq!(
        button_hit_box.rect,
        app.world()
            .get::<UiLayoutResult>(scene.button)
            .unwrap()
            .rect
    );
    assert!(button_hit_box.contains(center(&app, scene.button).to_array()));

    let display_list = app.world().resource::<DisplayList>();
    assert!(display_list.widget_items().iter().any(|item| {
        matches!(
            &item.primitive,
            Primitive::TextWrapped {
                text,
                max_width,
                wrap: false,
                style: Some(_),
                ..
            } if text == "Run" && *max_width > 0.0
        )
    }));
    assert!(display_list.items().iter().any(|item| {
        matches!(
            &item.primitive,
            Primitive::TextWrapped {
                text,
                max_width,
                wrap: false,
                ..
            } if text == "Name: draft" && *max_width > 0.0
        )
    }));

    let mut query = app
        .world_mut()
        .query::<(&Node, Option<&BackgroundColor>, Option<&Text>)>();
    let rendered_nodes = query.iter(app.world()).collect::<Vec<_>>();
    assert!(
        rendered_nodes
            .iter()
            .any(|(_, background, _)| background.is_some())
    );
    assert!(
        rendered_nodes
            .iter()
            .any(|(_, _, text)| { text.is_some_and(|text| text.0 == "Run") })
    );
    assert!(app.world().get::<Button>(scene.button).is_some());
}

#[test]
fn bevy_pointer_messages_drive_a_widget_activation() {
    let mut app = test_app();
    let scene = spawn_scene(&mut app);
    app.update();
    let position = center(&app, scene.button);
    app.world_mut()
        .get_mut::<Window>(scene.window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.world_mut().write_message(CursorMoved {
        window: scene.window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Pressed,
            window: scene.window,
        });
    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Released,
            window: scene.window,
        });
    app.update();

    assert_eq!(
        app.world().get::<Button>(scene.button).unwrap().activations,
        1
    );
    assert!(app.world().resource::<UiInputQueue>().is_empty());
    assert!(app.world().resource::<UiRoutedEvents>().events.iter().any(|event| {
        matches!(event, UiRoutedEvent::Click { target, button: UiPointerButton::Primary, .. } if *target == scene.button)
    }));
    assert!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| {
                matches!(event, WidgetEvent::Activated { target } if *target == scene.button)
            })
    );
}

#[test]
fn ime_and_external_clipboard_requests_reach_the_focused_text_field() {
    let mut app = test_app();
    let scene = spawn_scene(&mut app);
    app.update();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(scene.window)
        .focused = Some(scene.text_field);

    app.world_mut().write_message(Ime::Preedit {
        window: scene.window,
        value: "漢字".into(),
        cursor: Some((0, 6)),
    });
    app.update();
    let text = app.world().get::<TextBuffer>(scene.text_field).unwrap();
    let composition = text.composition().unwrap();
    assert_eq!(composition.text, "漢字");
    assert_eq!(
        composition.cursor_range,
        Some(bbui_text::TextRange::new(0, 6))
    );

    app.world_mut().write_message(Ime::Commit {
        window: scene.window,
        value: "漢字".into(),
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .as_str(),
        "draft漢字"
    );
    assert_eq!(
        app.world().get::<TextField>(scene.text_field).unwrap().text,
        "draft漢字"
    );
    app.world_mut()
        .get_mut::<TextBuffer>(scene.text_field)
        .unwrap()
        .select_all();

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Key {
            window: scene.window,
            key: UiKeyCode::C,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                ..Default::default()
            },
        });
    app.update();
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(|event| {
        matches!(event, WidgetEvent::ClipboardWrite { target, text } if *target == scene.text_field && text == "draft漢字")
    }));
    assert_eq!(
        app.world_mut()
            .resource_mut::<ClipboardRequestQueue>()
            .drain()
            .count(),
        0
    );
}

#[test]
fn bevy_keyboard_and_ime_messages_preserve_composition_text() {
    let mut app = test_app();
    let scene = spawn_scene(&mut app);
    app.update();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(scene.window)
        .focused = Some(scene.text_field);

    app.world_mut().write_message(Ime::Preedit {
        window: scene.window,
        value: "に".into(),
        cursor: Some((0, 1)),
    });
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyN,
        logical_key: bevy::input::keyboard::Key::Character("n".into()),
        state: ButtonState::Pressed,
        text: Some("n".into()),
        repeat: false,
        window: scene.window,
    });
    app.update();

    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .as_str(),
        "draft"
    );
    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .composition()
            .unwrap()
            .text,
        "に"
    );
    assert!(app.world().resource::<UiInputQueue>().is_empty());
}

#[test]
fn crosshair_surface_respects_occlusion_and_keeps_a_drag_outside_the_mesh() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.world_mut().init_resource::<Assets<Mesh>>();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(400.0, 200.0, 0.05));
    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    let camera = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Camera>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(surface).insert((
        Mesh3d(mesh.clone()),
        Aabb::from_min_max(
            Vec3::new(-200.0, -100.0, -0.025),
            Vec3::new(200.0, 100.0, 0.025),
        ),
        InheritedVisibility::VISIBLE,
        ViewVisibility::VISIBLE,
    ));
    app.world_mut()
        .entity_mut(camera)
        .insert(UiCrosshairPointer { max_distance: 20.0 });
    app.update();
    assert_eq!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .map(|(target, _)| target),
        Some(scene.logical_window)
    );

    let blocker = app
        .world_mut()
        .spawn((
            Mesh3d(mesh),
            Aabb::from_min_max(
                Vec3::new(-200.0, -100.0, -0.025),
                Vec3::new(200.0, 100.0, 0.025),
            ),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
            InheritedVisibility::VISIBLE,
            ViewVisibility::VISIBLE,
        ))
        .id();
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
    app.world_mut().despawn(blocker);
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_some()
    );

    let field_center = center(&app, scene.text_field);
    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(
            field_center.x - 200.0,
            100.0 - field_center.y,
            10.0,
        )));
    app.update();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut()
            .write_message(bevy::input::mouse::MouseButtonInput {
                button: bevy::input::mouse::MouseButton::Left,
                state,
                window: scene.physical_window,
            });
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(scene.logical_window)
            .and_then(|focus| focus.focused),
        Some(scene.text_field)
    );

    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(0.0, 0.0, 10.0)));
    app.update();

    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Pressed,
            window: scene.physical_window,
        });
    app.update();
    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from_translation(Vec3::new(
            300.0, 0.0, 10.0,
        )));
    app.update();
    let (_, position) = app
        .world()
        .resource::<WorldPointerState>()
        .hover_target(scene.physical_window)
        .unwrap();
    assert!(
        position[0] > 400.0,
        "captured world drag should keep unbounded UI coordinates"
    );

    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Released,
            window: scene.physical_window,
        });
    app.update();
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
}

#[test]
fn crosshair_camera_deactivation_clears_world_hover() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(400.0, 200.0, 0.05));
    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    let camera = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<Camera>>()
        .single(app.world())
        .unwrap();
    app.world_mut().entity_mut(surface).insert((
        Mesh3d(mesh),
        Aabb::from_min_max(
            Vec3::new(-200.0, -100.0, -0.025),
            Vec3::new(200.0, 100.0, 0.025),
        ),
        InheritedVisibility::VISIBLE,
        ViewVisibility::VISIBLE,
    ));
    app.world_mut()
        .entity_mut(camera)
        .insert(UiCrosshairPointer { max_distance: 20.0 });
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_some()
    );
    app.world_mut().get_mut::<Camera>(camera).unwrap().is_active = false;
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
}

#[test]
fn removed_world_surface_clears_stationary_pointer_hover() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();
    let position = screen_position_for_surface_ui(&scene, center(&app, scene.text_field));
    app.world_mut().write_message(CursorMoved {
        window: scene.physical_window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_some()
    );
    let surface = app
        .world_mut()
        .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<UiWorldSurface>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .entity_mut(surface)
        .remove::<UiWorldSurface>();
    app.update();
    assert!(
        app.world()
            .resource::<WorldPointerState>()
            .hover_target(scene.physical_window)
            .is_none()
    );
}

#[test]
fn world_surface_routes_pointer_wheel_keyboard_text_and_ime_to_its_camera() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();

    let ui_position = center(&app, scene.text_field);
    let position = screen_position_for_surface_ui(&scene, ui_position);
    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.world_mut().write_message(CursorMoved {
        window: scene.physical_window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.update();

    app.world_mut()
        .write_message(bevy::input::mouse::MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: scene.physical_window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    app.update();
    assert!(
        app.world()
            .resource::<UiRoutedEvents>()
            .events
            .iter()
            .any(|event| {
                matches!(event, UiRoutedEvent::Wheel { window, target, .. }
            if *window == scene.logical_window && *target == scene.text_field)
            })
    );

    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Pressed,
            window: scene.physical_window,
        });
    app.world_mut()
        .write_message(bevy::input::mouse::MouseButtonInput {
            button: bevy::input::mouse::MouseButton::Left,
            state: ButtonState::Released,
            window: scene.physical_window,
        });
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(scene.logical_window)
            .unwrap()
            .focused,
        Some(scene.text_field)
    );

    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyX,
        logical_key: bevy::input::keyboard::Key::Character("x".into()),
        state: ButtonState::Pressed,
        text: Some("x".into()),
        repeat: false,
        window: scene.physical_window,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .as_str(),
        "draftx"
    );

    app.world_mut().write_message(Ime::Preedit {
        window: scene.physical_window,
        value: "かな".into(),
        cursor: Some((0, 2)),
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<TextBuffer>(scene.text_field)
            .unwrap()
            .composition()
            .unwrap()
            .text,
        "かな"
    );
    app.world_mut().write_message(Ime::Commit {
        window: scene.physical_window,
        value: "かな".into(),
    });
    app.update();
    assert_eq!(
        app.world().get::<TextField>(scene.text_field).unwrap().text,
        "draftxかな"
    );
}

#[test]
fn world_surface_cursor_capture_uses_the_physical_window_and_routes_motion() {
    let mut app = test_app();
    let scene = spawn_world_scene(&mut app);
    app.update();

    let ui_position = center(&app, scene.text_field);
    let position = screen_position_for_surface_ui(&scene, ui_position);
    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .set_cursor_position(Some(position));
    app.world_mut().write_message(CursorMoved {
        window: scene.physical_window,
        position,
        delta: Some(Vec2::ZERO),
    });
    app.update();
    let initial_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();

    app.world_mut()
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::Begin {
            window: scene.logical_window,
            target: scene.text_field,
            hide_cursor: true,
            continuous: true,
        });
    app.update();
    let cursor = app
        .world()
        .get::<CursorOptions>(scene.physical_window)
        .unwrap();
    assert!(!cursor.visible);
    assert_eq!(cursor.grab_mode, bevy::window::CursorGrabMode::Locked);

    app.world_mut()
        .write_message(bevy::input::mouse::MouseMotion {
            delta: Vec2::new(7.0, 0.0),
        });
    app.update();
    let routed_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();
    assert_eq!(
        routed_position,
        [initial_position[0] + 7.0, initial_position[1]]
    );

    // Raw mouse motion is physical pixels. A captured world-surface pointer
    // must use the current physical window's DPI even while the OS cursor is
    // hidden and the virtual pointer is routed to another UI window.
    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(2.0));
    app.world_mut()
        .write_message(bevy::input::mouse::MouseMotion {
            delta: Vec2::new(12.0, 0.0),
        });
    app.update();
    let high_dpi_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();
    assert_eq!(
        high_dpi_position,
        [routed_position[0] + 6.0, routed_position[1]]
    );

    app.world_mut()
        .get_mut::<Window>(scene.physical_window)
        .unwrap()
        .resolution
        .set_scale_factor_override(Some(1.0));
    app.world_mut()
        .write_message(bevy::input::mouse::MouseMotion {
            delta: Vec2::new(12.0, 0.0),
        });
    app.update();
    let low_dpi_position = app
        .world()
        .resource::<UiFocus>()
        .state(scene.logical_window)
        .unwrap()
        .pointer_position
        .unwrap();
    assert_eq!(
        low_dpi_position,
        [high_dpi_position[0] + 12.0, high_dpi_position[1]]
    );

    app.world_mut()
        .resource_mut::<CursorCaptureRequestQueue>()
        .0
        .push_back(CursorCaptureRequest::End {
            window: scene.logical_window,
            target: scene.text_field,
            restore_position: true,
        });
    app.update();
    let cursor = app
        .world()
        .get::<CursorOptions>(scene.physical_window)
        .unwrap();
    assert!(cursor.visible);
    assert_eq!(cursor.grab_mode, bevy::window::CursorGrabMode::None);
}

#[test]
fn window_resize_and_dpi_refresh_layout_text_clip_hitbox_and_render_in_one_update() {
    let mut app = test_app();
    let font = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(bbui_text::UI_DEFAULT_FONT_DATA.to_vec()));
    app.add_systems(Update, load_font_assets_into_font_collection);
    app.update();

    let mut window = Window::default();
    window.resolution.set(400.0, 200.0);
    let window = app.world_mut().spawn(window).id();
    let root = WidgetTree::new(WidgetNode::column([WidgetNode::text_area(
        "Memo",
        "Responsive text wraps consistently as the window changes size and scale.",
    )]))
    .spawn(app.world_mut(), None)
    .unwrap();
    let mut root_node = *app.world().get::<UiLayoutNode>(root).unwrap();
    root_node.clip_contents = true;
    root_node.align_items = UiAlign::Start;
    root_node.mode = UiLayoutMode::Overlay;
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 400.0, 200.0), 1.0),
        root_node,
    ));
    let area = app.world().get::<UiChildren>(root).unwrap().0[0];
    app.world_mut().get_mut::<UiTextLayout>(area).unwrap().font =
        Some(TextFont::from_font_size(14.0).with_font(font));

    app.update();

    let initial_root = *app.world().get::<UiLayoutResult>(root).unwrap();
    let initial_text = app
        .world()
        .get::<UiTextLayout>(area)
        .unwrap()
        .layout()
        .unwrap()
        .bounds();
    assert_eq!(initial_root.rect, Rect::new(0.0, 0.0, 400.0, 200.0));
    assert_eq!(initial_root.dpi_scale, 1.0);
    assert!(initial_text.width > 0.0);

    let mut window_mut = app.world_mut().get_mut::<Window>(window).unwrap();
    window_mut.resolution.set_physical_resolution(320, 160);
    window_mut.resolution.set_scale_factor_override(Some(2.0));

    app.update();

    let synced_root = *app.world().get::<UiLayoutRoot>(root).unwrap();
    let root_layout = *app.world().get::<UiLayoutResult>(root).unwrap();
    let area_layout = *app.world().get::<UiLayoutResult>(area).unwrap();
    let hit_box = *app.world().get::<UiHitBox>(area).unwrap();
    assert_eq!(synced_root.viewport, Rect::new(0.0, 0.0, 160.0, 80.0));
    assert_eq!(synced_root.dpi_scale, 2.0);
    assert_eq!(root_layout.rect, synced_root.viewport);
    assert_eq!(root_layout.dpi_scale, 2.0);
    assert_eq!(area_layout.dpi_scale, 2.0);
    assert_eq!(area_layout.rect, Rect::new(0.0, 0.0, 220.0, 96.0));
    assert_eq!(area_layout.clip, Some(synced_root.viewport));
    assert_eq!(hit_box.rect, area_layout.rect);
    assert_eq!(hit_box.clip, area_layout.clip);
    assert!(hit_box.rect.contains(170.0, 10.0));
    assert!(!hit_box.contains([170.0, 10.0]));

    let text_layout = app.world().get::<UiTextLayout>(area).unwrap();
    let resized_text = text_layout.layout().unwrap().bounds();
    assert!((resized_text.width / initial_text.width - 2.0).abs() < 0.05);
    assert!((resized_text.height / initial_text.height - 2.0).abs() < 0.05);

    let (paint_clip, paint_width, paint_text) = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .iter()
        .find_map(|item| match &item.primitive {
            Primitive::TextWrapped {
                text, max_width, ..
            } if text.starts_with("Memo:") => Some((item.clip, *max_width, text.clone())),
            _ => None,
        })
        .expect("text area should be present in the same-frame display list");
    assert_eq!(
        paint_clip,
        bbui_render::ClipRect::from(synced_root.viewport)
    );

    let mut rendered_text = app.world_mut().query::<(&Text, &Node, &TextBounds)>();
    let rendered = rendered_text
        .iter(app.world())
        .find(|(text, _, bounds)| text.0 == paint_text && bounds.width == Some(paint_width))
        .expect("renderer should publish the resized text node in the same update");
    assert_eq!(rendered.1.width, bevy::ui::Val::Px(paint_width));
}

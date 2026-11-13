use super::*;
use bevy_ecs::component::Component;

#[derive(Component)]
struct Values {
    x: f32,
    y: f32,
}

fn x_binding() -> MotionBinding<f32> {
    MotionBinding::register(
        |world, entity| Some(world.get::<Values>(entity)?.x),
        |world, entity, value| {
            if let Some(mut values) = world.get_mut::<Values>(entity) {
                values.x = value;
            }
        },
    )
    .dirty_domains([UiDirtyDomain::Paint])
}

fn y_binding() -> MotionBinding<f32> {
    MotionBinding::register(
        |world, entity| Some(world.get::<Values>(entity)?.y),
        |world, entity, value| {
            if let Some(mut values) = world.get_mut::<Values>(entity) {
                values.y = value;
            }
        },
    )
}

fn track(from: f32, to: f32, duration: Duration, curve: MotionCurve) -> KeyframeTrack<f32> {
    KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, from).curve(curve),
        Keyframe::at(duration, to),
    ])
    .unwrap()
}

#[test]
fn curves_and_keyframes_validate_and_sample_multiple_segments() {
    assert_eq!(
        MotionCurve::cubic_bezier(-0.1, 0.0, 1.0, 1.0),
        Err(MotionError::InvalidCurve)
    );
    let bezier = MotionCurve::cubic_bezier(0.25, 0.1, 0.25, 1.0).unwrap();
    assert_eq!(bezier.sample(0.0), 0.0);
    assert!((bezier.sample(1.0) - 1.0).abs() < 0.0001);
    assert!(bezier.sample(0.5) > 0.5);
    assert!(
        MotionCurve::cubic_bezier(0.1, f32::MAX, 0.9, f32::MAX)
            .unwrap()
            .sample(0.5)
            .is_finite()
    );
    assert_eq!(MotionCurve::EaseOut.sample(0.5), 0.75);
    assert_eq!(MotionCurve::Step.sample(0.99), 0.0);
    assert_eq!(MotionCurve::Step.sample(1.0), 1.0);
    assert_eq!(
        KeyframeTrack::new([Keyframe::at(Duration::ZERO, f32::NAN)]),
        Err(MotionError::InvalidValue)
    );
    assert_eq!(KeyframeTrack::<f32>::new([]), Err(MotionError::EmptyTrack));
    let frames = KeyframeTrack::new([
        Keyframe::at(Duration::ZERO, 0.0),
        Keyframe::at(Duration::from_millis(500), 10.0).curve(MotionCurve::Step),
        Keyframe::at(Duration::from_secs(1), 20.0),
    ])
    .unwrap();
    assert_eq!(frames.sample(Duration::from_millis(250)), Some(5.0));
    assert_eq!(frames.sample(Duration::from_millis(750)), Some(10.0));
    assert_eq!(frames.sample(Duration::from_secs(1)), Some(20.0));
}

#[test]
fn custom_bindings_play_pause_seek_reverse_and_publish_dirty_domains() {
    let mut world = World::new();
    world.init_resource::<MotionPlayer>();
    world.init_resource::<UiDirty>();
    let entity = world.spawn(Values { x: 0.0, y: 0.0 }).id();
    let clip = MotionClip::new()
        .track(
            x_binding(),
            track(0.0, 10.0, Duration::from_secs(1), MotionCurve::Linear),
        )
        .track(
            y_binding(),
            track(0.0, 20.0, Duration::from_secs(1), MotionCurve::EaseOut),
        );
    let handle = world
        .resource_mut::<MotionPlayer>()
        .play(entity, clip, MotionPriority::Explicit)
        .unwrap();
    advance_motion_by(&mut world, Duration::from_millis(250));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 2.5);
    assert_eq!(world.get::<Values>(entity).unwrap().y, 8.75);
    assert!(
        world
            .resource::<UiDirty>()
            .is_marked(UiDirtyDomain::Paint, entity)
    );
    world.resource_mut::<MotionPlayer>().pause(handle);
    advance_motion_by(&mut world, Duration::from_millis(250));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 2.5);
    assert!(seek_motion(&mut world, handle, Duration::from_millis(750)));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 7.5);
    world.resource_mut::<MotionPlayer>().resume(handle);
    world.resource_mut::<MotionPlayer>().reverse(handle);
    advance_motion_by(&mut world, Duration::from_millis(250));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 5.0);
    advance_motion_by(&mut world, Duration::from_millis(500));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 0.0);
    assert_eq!(world.resource::<MotionPlayer>().active_count(), 0);
    assert_eq!(
        world
            .resource::<MotionEvents>()
            .0
            .last()
            .map(|event| event.kind),
        Some(MotionEventKind::Completed)
    );
}

#[derive(Resource, Default)]
struct Releases(usize);

#[test]
fn explicit_ownership_and_repeat_completion_are_deterministic() {
    let mut world = World::new();
    world.init_resource::<MotionPlayer>();
    world.init_resource::<Releases>();
    let entity = world.spawn(Values { x: 0.0, y: 0.0 }).id();
    let binding = x_binding().on_release(|world, _| world.resource_mut::<Releases>().0 += 1);
    let explicit = MotionClip::new().track(
        binding.clone(),
        track(0.0, 1.0, Duration::from_secs(1), MotionCurve::Linear),
    );
    let handle = world
        .resource_mut::<MotionPlayer>()
        .play(entity, explicit, MotionPriority::Explicit)
        .unwrap();
    let automatic = MotionClip::new().track(
        binding.clone(),
        track(0.0, 2.0, Duration::from_secs(1), MotionCurve::Linear),
    );
    assert!(
        world
            .resource_mut::<MotionPlayer>()
            .play(entity, automatic, MotionPriority::Automatic)
            .is_none()
    );
    world
        .resource_mut::<MotionPlayer>()
        .set_repeats(handle, Some(3));
    advance_motion_by(&mut world, Duration::from_millis(2_500));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 0.5);
    assert_eq!(world.resource::<MotionPlayer>().active_count(), 1);
    advance_motion_by(&mut world, Duration::from_millis(500));
    assert_eq!(world.get::<Values>(entity).unwrap().x, 1.0);
    assert_eq!(world.resource::<Releases>().0, 1);
    assert_eq!(world.resource::<MotionPlayer>().active_count(), 0);
}

#[test]
fn reduced_motion_snaps_to_final_keyframes_and_emits_completion() {
    let mut world = World::new();
    world.init_resource::<MotionPlayer>();
    let entity = world.spawn(Values { x: 0.0, y: 0.0 }).id();
    let clip = MotionClip::new().track(
        x_binding(),
        track(0.0, 10.0, Duration::from_secs(1), MotionCurve::Linear),
    );
    let handle = world
        .resource_mut::<MotionPlayer>()
        .play(entity, clip, MotionPriority::Explicit)
        .unwrap();
    world.resource_mut::<MotionPlayer>().pause(handle);
    world
        .resource_mut::<MotionPlayer>()
        .set_reduced_motion(true);
    advance_motion_by(&mut world, Duration::ZERO);
    assert_eq!(world.get::<Values>(entity).unwrap().x, 10.0);
    assert_eq!(
        world
            .resource::<MotionEvents>()
            .0
            .last()
            .map(|event| (event.handle, event.kind)),
        Some((handle, MotionEventKind::Completed))
    );
}

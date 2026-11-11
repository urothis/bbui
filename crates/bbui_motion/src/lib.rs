//! Typed keyframe timelines and active-only Bevy UI playback.

use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use bbui_core::{UiDirty, UiDirtyDomain, UiSystems};
use bevy_app::{App, Plugin, Update};
use bevy_curve::{Curve, easing::EaseFunction};
use bevy_ecs::{
    entity::Entity,
    prelude::{Resource, SystemSet},
    schedule::IntoScheduleConfigs,
    world::World,
};
use bevy_time::Time;
use indexmap::IndexMap;
use smallvec::SmallVec;

static NEXT_BINDING: AtomicU64 = AtomicU64::new(1 << 32);

/// A value that a motion track can interpolate. Return `None` for incompatible values.
pub trait MotionValue: Clone + PartialEq + Send + Sync + 'static {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Option<Self>;
    fn is_valid(&self) -> bool {
        true
    }
}

impl MotionValue for f32 {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Option<Self> {
        Some(*from + (*to - *from) * progress)
    }
    fn is_valid(&self) -> bool {
        self.is_finite()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezierCurve {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

/// Normalized timing function for one keyframe interval.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum MotionCurve {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Ease(EaseFunction),
    CubicBezier(CubicBezierCurve),
    Step,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionError {
    EmptyTrack,
    UnsortedKeyframes,
    InvalidCurve,
    InvalidValue,
    IncompatibleValues,
    InvalidSpeed,
    UnknownHandle,
}

impl MotionCurve {
    pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Result<Self, MotionError> {
        if ![x1, y1, x2, y2].into_iter().all(f32::is_finite)
            || !(0.0..=1.0).contains(&x1)
            || !(0.0..=1.0).contains(&x2)
        {
            return Err(MotionError::InvalidCurve);
        }
        Ok(Self::CubicBezier(CubicBezierCurve { x1, y1, x2, y2 }))
    }

    pub fn sample(self, progress: f32) -> f32 {
        if !progress.is_finite() {
            return 0.0;
        }
        let progress = progress.clamp(0.0, 1.0);
        if progress == 0.0 || progress == 1.0 {
            return progress;
        }
        match self {
            Self::Linear => progress,
            Self::EaseIn => EaseFunction::QuadraticIn.sample_clamped(progress),
            Self::EaseOut => EaseFunction::QuadraticOut.sample_clamped(progress),
            Self::EaseInOut => EaseFunction::QuadraticInOut.sample_clamped(progress),
            Self::Ease(ease) => ease.sample_clamped(progress),
            Self::Step => {
                if progress < 1.0 {
                    0.0
                } else {
                    1.0
                }
            }
            Self::CubicBezier(CubicBezierCurve { x1, y1, x2, y2 }) => {
                let cubic = |a: f64, b: f64, t: f64| {
                    let inverse = 1.0 - t;
                    3.0 * inverse * inverse * t * a + 3.0 * inverse * t * t * b + t * t * t
                };
                let (mut low, mut high) = (0.0, 1.0);
                for _ in 0..18 {
                    let middle = (low + high) * 0.5;
                    if cubic(f64::from(x1), f64::from(x2), middle) < f64::from(progress) {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                cubic(f64::from(y1), f64::from(y2), (low + high) * 0.5)
                    .clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyframe<T> {
    pub at: Duration,
    pub value: T,
    /// Curve from this frame to the next frame.
    pub curve: MotionCurve,
}

impl<T> Keyframe<T> {
    pub fn at(at: Duration, value: T) -> Self {
        Self {
            at,
            value,
            curve: MotionCurve::Linear,
        }
    }
    pub fn curve(mut self, curve: MotionCurve) -> Self {
        self.curve = curve;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyframeTrack<T> {
    frames: Vec<Keyframe<T>>,
}

impl<T: MotionValue> KeyframeTrack<T> {
    pub fn new(frames: impl IntoIterator<Item = Keyframe<T>>) -> Result<Self, MotionError> {
        let frames: Vec<_> = frames.into_iter().collect();
        if frames.is_empty() {
            return Err(MotionError::EmptyTrack);
        }
        if frames.iter().any(|frame| !frame.value.is_valid()) {
            return Err(MotionError::InvalidValue);
        }
        if frames.windows(2).any(|pair| pair[0].at >= pair[1].at) {
            return Err(MotionError::UnsortedKeyframes);
        }
        if frames
            .windows(2)
            .any(|pair| T::interpolate(&pair[0].value, &pair[1].value, 0.5).is_none())
        {
            return Err(MotionError::IncompatibleValues);
        }
        Ok(Self { frames })
    }

    pub fn duration(&self) -> Duration {
        self.frames.last().map_or(Duration::ZERO, |frame| frame.at)
    }
    pub fn last(&self) -> &T {
        &self.frames[self.frames.len() - 1].value
    }

    pub fn sample(&self, time: Duration) -> Option<T> {
        let first = self.frames.first()?;
        if time <= first.at {
            return Some(first.value.clone());
        }
        let last = self.frames.last()?;
        if time >= last.at {
            return Some(last.value.clone());
        }
        let index = self.frames.partition_point(|frame| frame.at < time);
        let from = &self.frames[index - 1];
        let to = &self.frames[index];
        let elapsed = time.saturating_sub(from.at).as_secs_f64();
        let span = to.at.saturating_sub(from.at).as_secs_f64();
        let value = MotionValue::interpolate(
            &from.value,
            &to.value,
            from.curve.sample((elapsed / span) as f32),
        )?;
        value.is_valid().then_some(value)
    }
}

type Reader<T> = Arc<dyn Fn(&World, Entity) -> Option<T> + Send + Sync>;
type Writer<T> = Arc<dyn Fn(&mut World, Entity, T) + Send + Sync>;
type Release = Arc<dyn Fn(&mut World, Entity) + Send + Sync>;

/// A typed ECS destination. Register custom fields once and reuse the returned binding.
pub struct MotionBinding<T> {
    id: u64,
    read: Reader<T>,
    write: Writer<T>,
    release: Option<Release>,
    dirty_domains: Arc<[UiDirtyDomain]>,
}

impl<T> Clone for MotionBinding<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            read: self.read.clone(),
            write: self.write.clone(),
            release: self.release.clone(),
            dirty_domains: self.dirty_domains.clone(),
        }
    }
}

impl<T> fmt::Debug for MotionBinding<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MotionBinding")
            .field("id", &self.id)
            .finish()
    }
}

impl<T: MotionValue> MotionBinding<T> {
    pub fn register(
        read: impl Fn(&World, Entity) -> Option<T> + Send + Sync + 'static,
        write: impl Fn(&mut World, Entity, T) + Send + Sync + 'static,
    ) -> Self {
        Self::with_id(NEXT_BINDING.fetch_add(1, Ordering::Relaxed), read, write)
    }

    /// Stable IDs below `2^32` are reserved for BBUI's built-in bindings.
    pub fn with_id(
        id: u64,
        read: impl Fn(&World, Entity) -> Option<T> + Send + Sync + 'static,
        write: impl Fn(&mut World, Entity, T) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            read: Arc::new(read),
            write: Arc::new(write),
            release: None,
            dirty_domains: Arc::from([]),
        }
    }

    /// Dirty domains published when this binding writes a changed value.
    pub fn dirty_domains(mut self, domains: impl IntoIterator<Item = UiDirtyDomain>) -> Self {
        self.dirty_domains = domains.into_iter().collect::<Vec<_>>().into();
        self
    }

    pub fn on_release(
        mut self,
        release: impl Fn(&mut World, Entity) + Send + Sync + 'static,
    ) -> Self {
        self.release = Some(Arc::new(release));
        self
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

trait ErasedTrack: Send + Sync {
    fn binding_id(&self) -> u64;
    fn duration(&self) -> Duration;
    fn apply(&self, world: &mut World, entity: Entity, time: Duration);
    fn has_target(&self, value: &dyn std::any::Any) -> bool;
    fn release(&self, world: &mut World, entity: Entity);
}

struct BoundTrack<T: MotionValue> {
    binding: MotionBinding<T>,
    frames: KeyframeTrack<T>,
}

impl<T: MotionValue> ErasedTrack for BoundTrack<T> {
    fn binding_id(&self) -> u64 {
        self.binding.id
    }
    fn duration(&self) -> Duration {
        self.frames.duration()
    }
    fn apply(&self, world: &mut World, entity: Entity, time: Duration) {
        if let Some(value) = self.frames.sample(time)
            && (self.binding.read)(world, entity).as_ref() != Some(&value)
        {
            (self.binding.write)(world, entity, value);
            if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
                for &domain in self.binding.dirty_domains.iter() {
                    dirty.mark(domain, entity);
                }
            }
        }
    }
    fn has_target(&self, value: &dyn std::any::Any) -> bool {
        value.downcast_ref::<T>() == Some(self.frames.last())
    }
    fn release(&self, world: &mut World, entity: Entity) {
        if let Some(callback) = &self.binding.release {
            callback(world, entity);
        }
    }
}

/// A heterogeneous group of typed tracks on one UI entity.
#[derive(Default)]
pub struct MotionClip {
    tracks: SmallVec<[Box<dyn ErasedTrack>; 1]>,
    duration: Duration,
}

impl MotionClip {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn track<T: MotionValue>(
        mut self,
        binding: MotionBinding<T>,
        frames: KeyframeTrack<T>,
    ) -> Self {
        let track = Box::new(BoundTrack { binding, frames });
        if let Some(index) = self
            .tracks
            .iter()
            .position(|existing| existing.binding_id() == track.binding_id())
        {
            self.tracks[index] = track;
        } else {
            self.tracks.push(track);
        }
        self.duration = self
            .tracks
            .iter()
            .map(|track| track.duration())
            .max()
            .unwrap_or_default();
        self
    }
    pub fn duration(&self) -> Duration {
        self.duration
    }
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MotionHandle(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPriority {
    Automatic,
    Explicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionEventKind {
    Completed,
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionEvent {
    pub handle: MotionHandle,
    pub entity: Entity,
    pub kind: MotionEventKind,
}

#[derive(Resource, Default)]
pub struct MotionEvents(pub Vec<MotionEvent>);

impl MotionEvents {
    pub fn take(&mut self) -> Vec<MotionEvent> {
        std::mem::take(&mut self.0)
    }
}

struct ActiveClip {
    entity: Entity,
    clip: MotionClip,
    time: Duration,
    delay: Duration,
    speed: f32,
    repeats: Option<u32>,
    completed_cycles: u32,
    paused: bool,
    reverse: bool,
    priority: MotionPriority,
    defer_first_advance: bool,
}

/// Owns only active clips; an idle frame does not visit UI entities.
#[derive(Resource)]
pub struct MotionPlayer {
    next: u64,
    clips: IndexMap<MotionHandle, ActiveClip>,
    pending: Vec<MotionEvent>,
    pending_releases: Vec<ActiveClip>,
    reduced_motion: bool,
    time_scale: f32,
    advanced_this_frame: bool,
}

impl Default for MotionPlayer {
    fn default() -> Self {
        Self {
            next: 0,
            clips: IndexMap::new(),
            pending: Vec::new(),
            pending_releases: Vec::new(),
            reduced_motion: false,
            time_scale: 1.0,
            advanced_this_frame: false,
        }
    }
}

impl MotionPlayer {
    pub fn play(
        &mut self,
        entity: Entity,
        clip: MotionClip,
        priority: MotionPriority,
    ) -> Option<MotionHandle> {
        self.play_inner(entity, clip, priority, false)
    }

    /// Start at the first keyframe this update; consume time on the next update.
    pub fn play_deferred(
        &mut self,
        entity: Entity,
        clip: MotionClip,
        priority: MotionPriority,
    ) -> Option<MotionHandle> {
        self.play_inner(entity, clip, priority, true)
    }

    fn play_inner(
        &mut self,
        entity: Entity,
        clip: MotionClip,
        priority: MotionPriority,
        defer_first_advance: bool,
    ) -> Option<MotionHandle> {
        if clip.is_empty() {
            return None;
        }
        let requested: Vec<_> = clip.tracks.iter().map(|track| track.binding_id()).collect();
        if priority == MotionPriority::Automatic
            && self.clips.values().any(|active| {
                active.entity == entity
                    && active.priority == MotionPriority::Explicit
                    && active
                        .clip
                        .tracks
                        .iter()
                        .any(|track| requested.contains(&track.binding_id()))
            })
        {
            return None;
        }
        let canceled: Vec<_> = self
            .clips
            .iter()
            .filter_map(|(handle, active)| {
                let conflict = active.entity == entity
                    && active
                        .clip
                        .tracks
                        .iter()
                        .any(|track| requested.contains(&track.binding_id()));
                if conflict
                    && (priority == MotionPriority::Explicit
                        || active.priority == MotionPriority::Automatic)
                {
                    Some(*handle)
                } else {
                    None
                }
            })
            .collect();
        for handle in canceled {
            self.cancel(handle);
        }
        self.next = self.next.wrapping_add(1);
        let handle = MotionHandle(self.next);
        self.clips.insert(
            handle,
            ActiveClip {
                entity,
                clip,
                time: Duration::ZERO,
                delay: Duration::ZERO,
                speed: 1.0,
                repeats: Some(1),
                completed_cycles: 0,
                paused: false,
                reverse: false,
                priority,
                defer_first_advance,
            },
        );
        Some(handle)
    }

    pub fn is_animating(&self, entity: Entity, binding_id: u64) -> bool {
        self.clips.values().any(|active| {
            active.entity == entity
                && active
                    .clip
                    .tracks
                    .iter()
                    .any(|track| track.binding_id() == binding_id)
        })
    }

    pub fn has_explicit(&self, entity: Entity, binding_id: u64) -> bool {
        self.clips.values().any(|active| {
            active.entity == entity
                && active.priority == MotionPriority::Explicit
                && active
                    .clip
                    .tracks
                    .iter()
                    .any(|track| track.binding_id() == binding_id)
        })
    }

    pub fn targets<T: MotionValue>(
        &self,
        entity: Entity,
        binding: &MotionBinding<T>,
        target: &T,
    ) -> bool {
        self.clips.values().any(|active| {
            active.entity == entity
                && active
                    .clip
                    .tracks
                    .iter()
                    .any(|track| track.binding_id() == binding.id && track.has_target(target))
        })
    }

    pub fn pause(&mut self, handle: MotionHandle) -> bool {
        self.set_paused(handle, true)
    }
    pub fn resume(&mut self, handle: MotionHandle) -> bool {
        self.set_paused(handle, false)
    }
    fn set_paused(&mut self, handle: MotionHandle, paused: bool) -> bool {
        let Some(clip) = self.clips.get_mut(&handle) else {
            return false;
        };
        clip.paused = paused;
        true
    }
    pub fn reverse(&mut self, handle: MotionHandle) -> bool {
        let Some(clip) = self.clips.get_mut(&handle) else {
            return false;
        };
        if clip.time.is_zero() && !clip.reverse {
            clip.time = clip.clip.duration;
        }
        clip.reverse = !clip.reverse;
        true
    }
    pub fn seek(&mut self, handle: MotionHandle, time: Duration) -> bool {
        let Some(clip) = self.clips.get_mut(&handle) else {
            return false;
        };
        clip.time = time.min(clip.clip.duration);
        true
    }
    pub fn set_speed(&mut self, handle: MotionHandle, speed: f32) -> Result<(), MotionError> {
        if !speed.is_finite() || speed <= 0.0 {
            return Err(MotionError::InvalidSpeed);
        }
        let Some(clip) = self.clips.get_mut(&handle) else {
            return Err(MotionError::UnknownHandle);
        };
        clip.speed = speed;
        Ok(())
    }
    pub fn set_delay(&mut self, handle: MotionHandle, delay: Duration) -> bool {
        let Some(clip) = self.clips.get_mut(&handle) else {
            return false;
        };
        clip.delay = delay;
        true
    }
    /// `None` repeats indefinitely; `Some(1)` plays once.
    pub fn set_repeats(&mut self, handle: MotionHandle, repeats: Option<u32>) -> bool {
        let Some(clip) = self.clips.get_mut(&handle) else {
            return false;
        };
        clip.repeats = repeats.map(|count| count.max(1));
        true
    }
    pub fn cancel(&mut self, handle: MotionHandle) -> bool {
        let Some(clip) = self.clips.shift_remove(&handle) else {
            return false;
        };
        self.pending.push(MotionEvent {
            handle,
            entity: clip.entity,
            kind: MotionEventKind::Canceled,
        });
        if clip.priority == MotionPriority::Explicit {
            self.pending_releases.push(clip);
        }
        true
    }
    pub fn cancel_binding(&mut self, entity: Entity, binding_id: u64) {
        let handles: Vec<_> = self
            .clips
            .iter()
            .filter_map(|(handle, clip)| {
                (clip.entity == entity
                    && clip
                        .clip
                        .tracks
                        .iter()
                        .any(|track| track.binding_id() == binding_id))
                .then_some(*handle)
            })
            .collect();
        for handle in handles {
            self.cancel(handle);
        }
    }
    pub fn cancel_automatic_binding(&mut self, entity: Entity, binding_id: u64) {
        let handles: Vec<_> = self
            .clips
            .iter()
            .filter_map(|(handle, clip)| {
                (clip.entity == entity
                    && clip.priority == MotionPriority::Automatic
                    && clip
                        .clip
                        .tracks
                        .iter()
                        .any(|track| track.binding_id() == binding_id))
                .then_some(*handle)
            })
            .collect();
        for handle in handles {
            self.cancel(handle);
        }
    }
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }
    pub fn set_time_scale(&mut self, scale: f32) {
        self.time_scale = if scale.is_finite() {
            scale.max(0.0)
        } else {
            0.0
        };
    }
    pub fn active_count(&self) -> usize {
        self.clips.len()
    }
    pub fn begin_frame(&mut self) {
        self.advanced_this_frame = false;
    }
    pub fn advanced_this_frame(&self) -> bool {
        self.advanced_this_frame
    }
}

pub struct MotionPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionSystems {
    Prepare,
    Advance,
}

impl Plugin for MotionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MotionPlayer>()
            .init_resource::<MotionEvents>()
            .configure_sets(
                Update,
                (MotionSystems::Prepare, MotionSystems::Advance)
                    .chain()
                    .in_set(UiSystems::Motion),
            )
            .add_systems(Update, clear_motion_events.in_set(MotionSystems::Prepare))
            .add_systems(Update, advance_motion.in_set(MotionSystems::Advance));
    }
}

fn clear_motion_events(mut events: bevy_ecs::prelude::ResMut<MotionEvents>) {
    events.0.clear();
}

pub fn advance_motion(world: &mut World) {
    let delta = world
        .get_resource::<Time>()
        .map(Time::delta)
        .unwrap_or(Duration::from_secs_f32(1.0 / 60.0));
    advance_motion_by(world, delta);
}

pub fn advance_motion_by(world: &mut World, delta: Duration) {
    advance_motion_selected(world, delta, None);
}

/// Advance only newly started handles in the current widget phase.
pub fn advance_motion_handles_by(world: &mut World, handles: &[MotionHandle], delta: Duration) {
    advance_motion_selected(world, delta, Some(handles));
}

/// Seek and immediately apply sampled values, including for a paused clip.
pub fn seek_motion(world: &mut World, handle: MotionHandle, time: Duration) -> bool {
    let Some(mut player) = world.get_resource_mut::<MotionPlayer>() else {
        return false;
    };
    let mut player = std::mem::take(&mut *player);
    if !player.seek(handle, time) {
        *world.resource_mut::<MotionPlayer>() = player;
        return false;
    }
    if let Some(active) = player.clips.get(&handle) {
        for track in &active.clip.tracks {
            track.apply(world, active.entity, active.time.min(track.duration()));
        }
    }
    *world.resource_mut::<MotionPlayer>() = player;
    true
}

fn advance_motion_selected(world: &mut World, delta: Duration, selected: Option<&[MotionHandle]>) {
    world.init_resource::<MotionPlayer>();
    world.init_resource::<MotionEvents>();
    if selected.is_none()
        && world.resource::<MotionPlayer>().clips.is_empty()
        && world.resource::<MotionPlayer>().pending.is_empty()
        && world.resource::<MotionPlayer>().pending_releases.is_empty()
    {
        world.resource_mut::<MotionPlayer>().advanced_this_frame = true;
        return;
    }
    let mut player = std::mem::take(&mut *world.resource_mut::<MotionPlayer>());
    if selected.is_none() {
        player.advanced_this_frame = true;
    }
    for released in player.pending_releases.drain(..) {
        for track in &released.clip.tracks {
            track.release(world, released.entity);
        }
    }
    let mut completed = Vec::new();
    for (handle, active) in &mut player.clips {
        if selected.is_some_and(|handles| !handles.contains(handle)) {
            continue;
        }
        if !world.entities().contains(active.entity) {
            completed.push((*handle, MotionEventKind::Canceled));
            continue;
        }
        if active.paused && !player.reduced_motion && player.time_scale > 0.0 {
            continue;
        }
        let duration = active.clip.duration;
        let snap = player.reduced_motion || player.time_scale == 0.0 || duration.is_zero();
        let elapsed = if active.defer_first_advance && !snap {
            active.defer_first_advance = false;
            Duration::ZERO
        } else if snap {
            duration
        } else {
            let seconds = delta.as_secs_f64() * f64::from(active.speed * player.time_scale);
            let scaled = if seconds >= Duration::MAX.as_secs_f64() {
                Duration::MAX
            } else {
                Duration::from_secs_f64(seconds.max(0.0))
            };
            if active.delay > scaled {
                active.delay -= scaled;
                continue;
            }
            let available = scaled.saturating_sub(active.delay);
            active.delay = Duration::ZERO;
            available
        };
        let mut finished = snap;
        if snap {
            active.time = if active.reverse {
                Duration::ZERO
            } else {
                duration
            };
        } else if !duration.is_zero() {
            let span = duration.as_nanos();
            let elapsed = elapsed.as_nanos();
            let current = active.time.as_nanos();
            let (cycles, position) = if active.reverse {
                if elapsed < current {
                    (0, current - elapsed)
                } else {
                    let excess = elapsed - current;
                    (
                        1 + excess / span,
                        if excess % span == 0 {
                            span
                        } else {
                            span - excess % span
                        },
                    )
                }
            } else {
                let total = current + elapsed;
                (total / span, total % span)
            };
            if cycles > 0 {
                active.completed_cycles = active
                    .completed_cycles
                    .saturating_add(cycles.min(u128::from(u32::MAX)) as u32);
                finished = active
                    .repeats
                    .is_some_and(|count| active.completed_cycles >= count);
            }
            let position = if finished {
                if active.reverse { 0 } else { span }
            } else {
                position
            };
            active.time = duration_from_nanos(position);
        }
        for track in &active.clip.tracks {
            track.apply(world, active.entity, active.time.min(track.duration()));
        }
        if finished {
            completed.push((*handle, MotionEventKind::Completed));
        }
    }
    for (handle, kind) in completed {
        if let Some(active) = player.clips.shift_remove(&handle) {
            if active.priority == MotionPriority::Explicit {
                for track in &active.clip.tracks {
                    track.release(world, active.entity);
                }
            }
            player.pending.push(MotionEvent {
                handle,
                entity: active.entity,
                kind,
            });
        }
    }
    world
        .resource_mut::<MotionEvents>()
        .0
        .append(&mut player.pending);
    *world.resource_mut::<MotionPlayer>() = player;
}

fn duration_from_nanos(nanos: u128) -> Duration {
    let seconds = (nanos / 1_000_000_000).min(u128::from(u64::MAX)) as u64;
    let subsec = (nanos % 1_000_000_000) as u32;
    Duration::new(seconds, subsec)
}

#[cfg(test)]
mod tests {
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
}

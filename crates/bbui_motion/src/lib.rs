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

mod runtime;
pub use runtime::{
    MotionPlugin, MotionSystems, advance_motion, advance_motion_by, advance_motion_handles_by,
    seek_motion,
};

#[cfg(test)]
mod motion_tests;

//! Convert AppKit's positioned, phased samples into BBUI's gesture queue.

use bbui_core::{UiGesturePhase, UiInputEvent, UiInputQueue, UiTrackpadGesture, UiTrackpadKind};
use bbui_macos_trackpad::{TrackpadMonitor, TrackpadSample};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};

#[derive(Resource, Default)]
pub(crate) struct MacTrackpadFrame {
    pub(crate) available: bool,
}

type NativePrimaryWindow<'w, 's> =
    Query<'w, 's, (Entity, &'static Window), (With<PrimaryWindow>, With<RawHandleWrapper>)>;

fn logical_position(position: [f64; 2], window_height: f32) -> Option<[f32; 2]> {
    let [x, y] = position;
    (x.is_finite() && y.is_finite() && window_height.is_finite())
        .then_some([x as f32, window_height - y as f32])
}

fn convert(sample: TrackpadSample, window: Entity, height: f32) -> Option<UiInputEvent> {
    match sample {
        TrackpadSample::Scroll {
            position,
            delta,
            began,
            momentum_ended,
            phased,
            command,
            ..
        } => {
            let position = logical_position(position, height)?;
            if !delta.iter().all(|value| value.is_finite()) {
                return None;
            }
            let phase = if momentum_ended {
                UiGesturePhase::Ended
            } else if began {
                UiGesturePhase::Started
            } else {
                UiGesturePhase::Updated
            };
            // Command/Control scroll zooms around the event's native position.
            let scale = if command {
                ((delta[0] + delta[1]) * 0.01).exp() as f32
            } else {
                1.0
            };
            Some(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
                window,
                position,
                delta: if command {
                    [0.0, 0.0]
                } else {
                    [delta[0] as f32, delta[1] as f32]
                },
                scale,
                kind: UiTrackpadKind::Scroll,
                phase,
                phased,
            }))
        }
        TrackpadSample::Magnify {
            position,
            delta,
            began,
            ended,
            cancelled,
            phased,
        } => {
            let position = logical_position(position, height)?;
            if !delta.is_finite() || delta <= -1.0 {
                return None;
            }
            let phase = if cancelled {
                UiGesturePhase::Cancelled
            } else if ended {
                UiGesturePhase::Ended
            } else if began {
                UiGesturePhase::Started
            } else {
                UiGesturePhase::Updated
            };
            Some(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
                window,
                position,
                delta: [0.0, 0.0],
                scale: (1.0 + delta) as f32,
                kind: UiTrackpadKind::Magnify,
                phase,
                phased,
            }))
        }
    }
}

pub(crate) fn bridge_native_trackpad(
    primary: NativePrimaryWindow,
    mut monitor: NonSendMut<TrackpadMonitor>,
    mut frame: ResMut<MacTrackpadFrame>,
    mut queue: ResMut<UiInputQueue>,
) {
    let Ok((window, data)) = primary.single() else {
        frame.available = false;
        return;
    };
    let samples = monitor.drain();
    frame.available = monitor.available();
    if !data.focused {
        return;
    }
    let height = data.resolution.height();
    queue.extend(
        samples
            .into_iter()
            .filter_map(|sample| convert(sample, window, height)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_magnify_keeps_position_and_incremental_scale() {
        let window = Entity::PLACEHOLDER;
        let event = convert(
            TrackpadSample::Magnify {
                position: [88.0, 26.0],
                delta: 0.125,
                began: true,
                ended: false,
                cancelled: false,
                phased: true,
            },
            window,
            200.0,
        );
        assert!(
            matches!(event, Some(UiInputEvent::TrackpadGesture(UiTrackpadGesture {
            position: [88.0, 174.0], scale, phase: UiGesturePhase::Started,
            kind: UiTrackpadKind::Magnify, ..
        })) if (scale - 1.125).abs() < 0.0001)
        );
    }
}

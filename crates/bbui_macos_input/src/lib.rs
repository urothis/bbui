//! AppKit pointer and trackpad events behind a safe sampling API.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MacInputSample {
    Trackpad(TrackpadSample),
    MouseUp(u8),
    MouseDrag { button: u8, position: [f64; 2] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackpadSample {
    Scroll {
        /// AppKit window-local position in points, origin at the bottom left.
        position: [f64; 2],
        delta: [f64; 2],
        began: bool,
        ended: bool,
        momentum_ended: bool,
        phased: bool,
        command: bool,
    },
    Magnify {
        position: [f64; 2],
        delta: f64,
        began: bool,
        ended: bool,
        cancelled: bool,
        phased: bool,
    },
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{MacInputSample, TrackpadSample};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventPhase, NSEventType};
    use std::sync::mpsc::{Receiver, channel};

    #[derive(Default)]
    pub struct TrackpadMonitor {
        receiver: Option<Receiver<MacInputSample>>,
        token: Option<Retained<AnyObject>>,
        attempted: bool,
    }

    impl TrackpadMonitor {
        pub fn available(&self) -> bool {
            self.receiver.is_some()
        }

        /// Install on the main thread and drain samples in AppKit event order.
        pub fn drain(&mut self) -> Vec<MacInputSample> {
            if !self.attempted {
                self.install();
            }
            self.receiver
                .as_ref()
                .map_or_else(Vec::new, |receiver| receiver.try_iter().collect())
        }

        /// Button snapshot used only when AppKit's local monitor could not be installed.
        pub fn pressed_mouse_buttons() -> usize {
            NSEvent::pressedMouseButtons()
        }

        fn install(&mut self) {
            self.attempted = true;
            let (sender, receiver) = channel();
            let block = block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
                // SAFETY: AppKit invokes the monitor with a live NSEvent for the callback's duration.
                let event = unsafe { event.as_ref() };
                let location = event.locationInWindow();
                let position = [location.x, location.y];
                if event.r#type() == NSEventType::ScrollWheel && event.hasPreciseScrollingDeltas() {
                    let phase = event.phase();
                    let momentum = event.momentumPhase();
                    let ended = phase.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled);
                    let momentum_ended =
                        momentum.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled);
                    let _ = sender.send(MacInputSample::Trackpad(TrackpadSample::Scroll {
                        position,
                        delta: [event.scrollingDeltaX(), event.scrollingDeltaY()],
                        began: phase.intersects(NSEventPhase::Began | NSEventPhase::MayBegin),
                        ended,
                        momentum_ended,
                        phased: phase != NSEventPhase::None || momentum != NSEventPhase::None,
                        command: event.modifierFlags().intersects(
                            NSEventModifierFlags::Command | NSEventModifierFlags::Control,
                        ),
                    }));
                } else if event.r#type() == NSEventType::Magnify {
                    let phase = event.phase();
                    let _ = sender.send(MacInputSample::Trackpad(TrackpadSample::Magnify {
                        position,
                        delta: event.magnification(),
                        began: phase.intersects(NSEventPhase::Began),
                        ended: phase.intersects(NSEventPhase::Ended),
                        cancelled: phase.intersects(NSEventPhase::Cancelled),
                        phased: phase != NSEventPhase::None,
                    }));
                } else if matches!(
                    event.r#type(),
                    NSEventType::LeftMouseDragged
                        | NSEventType::RightMouseDragged
                        | NSEventType::OtherMouseDragged
                ) {
                    let button = event.buttonNumber();
                    if (0..=2).contains(&button) {
                        let _ = sender.send(MacInputSample::MouseDrag {
                            button: button as u8,
                            position,
                        });
                    }
                } else if matches!(
                    event.r#type(),
                    NSEventType::LeftMouseUp
                        | NSEventType::RightMouseUp
                        | NSEventType::OtherMouseUp
                ) {
                    let button = event.buttonNumber();
                    if (0..=2).contains(&button) {
                        let _ = sender.send(MacInputSample::MouseUp(button as u8));
                    }
                }
                event as *const NSEvent as *mut NSEvent
            });
            let mask = NSEventMask::ScrollWheel
                | NSEventMask::Magnify
                | NSEventMask::LeftMouseUp
                | NSEventMask::RightMouseUp
                | NSEventMask::OtherMouseUp
                | NSEventMask::LeftMouseDragged
                | NSEventMask::RightMouseDragged
                | NSEventMask::OtherMouseDragged;
            // SAFETY: This local monitor is installed and later removed on AppKit's main thread.
            if let Some(token) =
                unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) }
            {
                self.receiver = Some(receiver);
                self.token = Some(token);
            }
        }
    }

    impl Drop for TrackpadMonitor {
        fn drop(&mut self) {
            if let Some(token) = self.token.take() {
                // SAFETY: The token belongs to this local monitor and is removed on its owner thread.
                unsafe { NSEvent::removeMonitor(&token) };
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::TrackpadMonitor;

//! Fulfill one-shot widget clipboard requests through Bevy's desktop clipboard.

use bbui_core::{UiFocus, UiInputEvent, UiInputQueue};
use bbui_widgets::actions::{ClipboardRequest, ClipboardRequestQueue};
use bevy::clipboard::Clipboard;
use bevy::prelude::*;

pub fn process_clipboard_requests(
    mut requests: ResMut<ClipboardRequestQueue>,
    mut clipboard: ResMut<Clipboard>,
    focus: Res<UiFocus>,
    mut input: ResMut<UiInputQueue>,
) {
    for request in requests.drain() {
        match request {
            ClipboardRequest::Write(text) => {
                if let Err(error) = clipboard.set_text(text) {
                    warn!("BBUI clipboard write failed: {error}");
                }
            }
            ClipboardRequest::Read { target } => {
                let Some((&window, _)) = focus
                    .windows
                    .iter()
                    .find(|(_, state)| state.focused == Some(target))
                else {
                    continue;
                };
                match clipboard.fetch_text().poll_result() {
                    Some(Ok(text)) if !text.is_empty() => {
                        input.push(UiInputEvent::Text { window, text })
                    }
                    Some(Err(error)) => warn!("BBUI clipboard read failed: {error}"),
                    _ => {}
                }
            }
        }
    }
}

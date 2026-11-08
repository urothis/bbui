//! Text editing, numeric expressions, display formatting, and shared metrics.

use bevy_app::{App, Plugin, Update};
use bevy_asset::Assets;
use bevy_ecs::prelude::*;
use bevy_text::{Font, FontCx, LayoutCx, TextPipeline};
use bevy_time::Time;
use bui_core::{UiKeyCode, UiModifiers, UiRoutedEvent, UiRoutedEvents, UiSystems};
use std::collections::HashSet;

pub mod expression;
pub mod font_assets;
pub mod formatting;
pub mod layout;
pub mod text_buffer;
pub mod ui_layout;

pub use font_assets::{
    UiFontAsset, UiFontAssetState, UiFontAssetStatus, resolve_text_font, sync_ui_font_assets,
};
pub use formatting::format_float;
pub use layout::{
    BevyTextLayoutConfig, GlyphPlacement, MonospaceMeasurer, TextBounds, TextLayout, TextMeasurer,
    TextRect,
};
pub use text_buffer::{
    CaretAffinity, ImeComposition, TextBuffer, TextCaretPosition, TextRange, UiExternalClipboard,
    UiMultiline, UiReadOnly, UiTextBuffer,
};
pub use ui_layout::{UiTextLayout, measure_text_intrinsics, update_text_layouts};

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiClipboard(pub String);

#[derive(Resource, Debug, Default)]
pub struct UiTextUndoFallback {
    targets: HashSet<Entity>,
}

impl UiTextUndoFallback {
    pub fn needs_global_undo(&self, target: Entity) -> bool {
        self.targets.contains(&target)
    }
}

pub struct TextPlugin;

impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiClipboard>()
            .init_resource::<UiTextUndoFallback>()
            .init_resource::<UiRoutedEvents>()
            .init_resource::<Assets<Font>>()
            .init_resource::<TextPipeline>()
            .init_resource::<FontCx>()
            .init_resource::<LayoutCx>()
            .init_resource::<bui_style::Theme>()
            .add_systems(
                Update,
                sync_ui_font_assets
                    .in_set(UiSystems::TextMeasure)
                    .before(measure_text_intrinsics),
            )
            .add_systems(
                Update,
                measure_text_intrinsics.in_set(UiSystems::TextMeasure),
            )
            .add_systems(
                Update,
                (process_text_events, update_text_layouts)
                    .chain()
                    .in_set(UiSystems::Text),
            );
    }
}

/// Apply routed text, IME, editing, and clipboard commands to focused text
/// buffers. Widget systems run afterward and observe the updated buffer.
pub fn process_text_events(world: &mut World) {
    let edit_time = world
        .get_resource::<Time>()
        .map(|time| time.elapsed_secs_f64());
    if let Some(mut fallback) = world.get_resource_mut::<UiTextUndoFallback>() {
        fallback.targets.clear();
    }
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    for event in events {
        match event {
            UiRoutedEvent::Text { target, text, .. } if !text.is_empty() => {
                if !is_read_only(world, target) {
                    with_text_buffer_mut(world, target, |buffer| {
                        if let Some(now) = edit_time {
                            buffer.begin_timed_edit(now);
                        }
                        buffer.insert_text(&text);
                    });
                }
            }
            UiRoutedEvent::ImePreedit {
                target,
                text,
                cursor_range,
                ..
            } => {
                with_text_buffer_mut(world, target, |buffer| {
                    buffer.break_undo_group();
                    buffer.set_composition(
                        text,
                        cursor_range.map(|range| TextRange::new(range[0], range[1])),
                    );
                });
            }
            UiRoutedEvent::ImeCommit { target, text, .. } => {
                if !is_read_only(world, target) {
                    with_text_buffer_mut(world, target, |buffer| {
                        if let Some(now) = edit_time {
                            buffer.begin_timed_edit(now);
                        }
                        buffer.commit_composition(Some(&text));
                    });
                }
            }
            UiRoutedEvent::ImeDisabled { target, .. } => {
                with_text_buffer_mut(world, target, |buffer| {
                    buffer.cancel_composition();
                    buffer.break_undo_group();
                });
            }
            UiRoutedEvent::Key {
                target,
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                process_key(world, target, key, modifiers, edit_time);
            }
            _ => {}
        }
    }
}

fn process_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    edit_time: Option<f64>,
) {
    let primary = modifiers.control || modifiers.super_key;
    let read_only = is_read_only(world, target);
    let external_clipboard = world.get::<UiExternalClipboard>(target).is_some();
    let mut text_layout = world.get::<UiTextLayout>(target).cloned();
    let multiline = world.get::<UiMultiline>(target).is_some()
        || text_layout.as_ref().is_some_and(|layout| layout.multiline);
    let retain_visual_x = multiline && matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown);
    if !retain_visual_x && let Some(layout) = text_layout.as_mut() {
        layout.reset_visual_navigation();
    }
    let clipboard = world
        .get_resource::<UiClipboard>()
        .map(|clipboard| clipboard.0.clone())
        .unwrap_or_default();
    let mut clipboard_update = None;
    let mut local_history_result = None;
    let has_buffer = with_text_buffer_mut(world, target, |buffer| {
        if matches!(
            key,
            UiKeyCode::Backspace | UiKeyCode::Delete | UiKeyCode::Enter
        ) && let Some(now) = edit_time
        {
            buffer.begin_timed_edit(now);
        }
        match (key, primary, modifiers.shift) {
            (UiKeyCode::A, true, _) => buffer.select_all(),
            (UiKeyCode::C, true, _) if !external_clipboard => clipboard_update = buffer.copy(),
            (UiKeyCode::X, true, _) if !read_only && !external_clipboard => {
                clipboard_update = buffer.cut()
            }
            (UiKeyCode::V, true, _) if !read_only && !external_clipboard => {
                buffer.paste(&clipboard);
            }
            (UiKeyCode::Z, true, true) | (UiKeyCode::Y, true, _) => {
                local_history_result = Some(if read_only { false } else { buffer.redo() });
            }
            (UiKeyCode::Z, true, false) => {
                local_history_result = Some(if read_only { false } else { buffer.undo() });
            }
            (UiKeyCode::ArrowLeft, true, extend) => buffer.move_word_left(extend),
            (UiKeyCode::ArrowRight, true, extend) => buffer.move_word_right(extend),
            (UiKeyCode::ArrowLeft, false, extend) if multiline => {
                let moved = text_layout
                    .as_mut()
                    .and_then(|layout| layout.move_caret_visual_horizontal(buffer, false, extend));
                if moved.is_none() && buffer.composition().is_none() {
                    buffer.move_left(extend);
                }
            }
            (UiKeyCode::ArrowRight, false, extend) if multiline => {
                let moved = text_layout
                    .as_mut()
                    .and_then(|layout| layout.move_caret_visual_horizontal(buffer, true, extend));
                if moved.is_none() && buffer.composition().is_none() {
                    buffer.move_right(extend);
                }
            }
            (UiKeyCode::ArrowLeft, _, extend) => buffer.move_left(extend),
            (UiKeyCode::ArrowRight, _, extend) => buffer.move_right(extend),
            (UiKeyCode::ArrowUp, _, extend) if multiline => {
                let moved = text_layout
                    .as_mut()
                    .and_then(|layout| layout.move_caret_visual_line(buffer, false, extend));
                if moved.is_none() && buffer.composition().is_none() {
                    buffer.move_vertical(false, extend);
                }
            }
            (UiKeyCode::ArrowDown, _, extend) if multiline => {
                let moved = text_layout
                    .as_mut()
                    .and_then(|layout| layout.move_caret_visual_line(buffer, true, extend));
                if moved.is_none() && buffer.composition().is_none() {
                    buffer.move_vertical(true, extend);
                }
            }
            (UiKeyCode::ArrowUp, _, extend) => buffer.move_vertical(false, extend),
            (UiKeyCode::ArrowDown, _, extend) => buffer.move_vertical(true, extend),
            (UiKeyCode::Home, true, extend) => buffer.move_document_start(extend),
            (UiKeyCode::End, true, extend) => buffer.move_document_end(extend),
            (UiKeyCode::Home, false, extend) if multiline => {
                let moved = text_layout
                    .as_mut()
                    .and_then(|layout| layout.move_caret_visual_line_edge(buffer, false, extend));
                if moved.is_none() {
                    buffer.move_home(extend);
                }
            }
            (UiKeyCode::End, false, extend) if multiline => {
                let moved = text_layout
                    .as_mut()
                    .and_then(|layout| layout.move_caret_visual_line_edge(buffer, true, extend));
                if moved.is_none() {
                    buffer.move_end(extend);
                }
            }
            (UiKeyCode::Home, _, extend) => buffer.move_home(extend),
            (UiKeyCode::End, _, extend) => buffer.move_end(extend),
            (UiKeyCode::Backspace, true, _) if !read_only => {
                buffer.delete_word_backward();
            }
            (UiKeyCode::Delete, true, _) if !read_only => {
                buffer.delete_word_forward();
            }
            (UiKeyCode::Backspace, false, _) if !read_only => {
                buffer.delete_backward();
            }
            (UiKeyCode::Delete, false, _) if !read_only => {
                buffer.delete_forward();
            }
            (UiKeyCode::Enter, false, _) if !read_only && multiline => {
                buffer.insert_text("\n");
            }
            _ => {}
        }
    });
    if world.get::<UiTextLayout>(target).is_some()
        && let Some(layout) = text_layout
    {
        world.entity_mut(target).insert(layout);
    }
    if has_buffer
        && primary
        && matches!(key, UiKeyCode::Z | UiKeyCode::Y)
        && local_history_result == Some(false)
        && let Some(mut fallback) = world.get_resource_mut::<UiTextUndoFallback>()
    {
        fallback.targets.insert(target);
    }
    if let Some(value) = clipboard_update
        && let Some(mut clipboard) = world.get_resource_mut::<UiClipboard>()
    {
        clipboard.0 = value;
    }
}

fn is_read_only(world: &World, entity: Entity) -> bool {
    world.get::<UiReadOnly>(entity).is_some()
}

fn with_text_buffer_mut(
    world: &mut World,
    entity: Entity,
    update: impl FnOnce(&mut TextBuffer),
) -> bool {
    if let Some(mut buffer) = world.get_mut::<TextBuffer>(entity) {
        update(&mut buffer);
        true
    } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(entity) {
        update(&mut buffer.0);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn key(target: Entity, key: UiKeyCode, modifiers: UiModifiers) -> UiRoutedEvent {
        UiRoutedEvent::Key {
            window: Entity::PLACEHOLDER,
            target,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn routed_typing_uses_elapsed_time_for_local_undo_boundaries() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<Time>();
        let target = world.spawn(UiTextBuffer(TextBuffer::default())).id();
        for (delay, text) in [(0, "a"), (200, "b"), (900, "c")] {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(delay));
            world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
                window: Entity::PLACEHOLDER,
                target,
                text: text.to_owned(),
            }];
            process_text_events(&mut world);
        }
        let mut buffer = world.get_mut::<UiTextBuffer>(target).unwrap();
        assert_eq!(buffer.0.as_str(), "abc");
        assert_eq!(buffer.0.undo_len(), 2);
        assert!(buffer.0.undo());
        assert_eq!(buffer.0.as_str(), "ab");
    }

    #[test]
    fn routed_text_and_clipboard_commands_update_component_before_widgets() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<UiClipboard>();
        let target = world.spawn(UiTextBuffer(TextBuffer::new("hello"))).id();
        world
            .get_mut::<UiTextBuffer>(target)
            .unwrap()
            .0
            .set_selection(0, 5);
        world.resource_mut::<UiRoutedEvents>().events = vec![key(
            target,
            UiKeyCode::C,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        process_text_events(&mut world);
        assert_eq!(world.resource::<UiClipboard>().0, "hello");

        world
            .get_mut::<UiTextBuffer>(target)
            .unwrap()
            .0
            .set_selection(0, 5);
        world.resource_mut::<UiRoutedEvents>().events[0] = key(
            target,
            UiKeyCode::X,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        process_text_events(&mut world);
        assert_eq!(world.get::<UiTextBuffer>(target).unwrap().0.as_str(), "");

        world.resource_mut::<UiRoutedEvents>().events[0] = key(
            target,
            UiKeyCode::V,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        );
        process_text_events(&mut world);
        assert_eq!(
            world.get::<UiTextBuffer>(target).unwrap().0.as_str(),
            "hello"
        );
    }

    #[test]
    fn multiline_clipboard_and_local_undo_preserve_newlines_and_selection_direction() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<UiClipboard>();
        let original = "first line\nsecond line";
        let target = world.spawn((TextBuffer::new(original), UiMultiline)).id();
        world
            .get_mut::<TextBuffer>(target)
            .unwrap()
            .set_selection_with_affinity(
                original.len(),
                0,
                CaretAffinity::Upstream,
                CaretAffinity::Downstream,
            );
        let control = UiModifiers {
            control: true,
            ..UiModifiers::default()
        };

        world.resource_mut::<UiRoutedEvents>().events = vec![key(target, UiKeyCode::C, control)];
        process_text_events(&mut world);
        assert_eq!(world.resource::<UiClipboard>().0, original);

        world.resource_mut::<UiRoutedEvents>().events = vec![key(target, UiKeyCode::X, control)];
        process_text_events(&mut world);
        assert_eq!(world.get::<TextBuffer>(target).unwrap().as_str(), "");
        world.resource_mut::<UiRoutedEvents>().events = vec![key(target, UiKeyCode::Z, control)];
        process_text_events(&mut world);
        let buffer = world.get::<TextBuffer>(target).unwrap();
        assert_eq!(buffer.as_str(), original);
        assert_eq!(buffer.anchor(), original.len());
        assert_eq!(buffer.caret(), 0, "undo restores the backward selection");
        assert_eq!(buffer.anchor_affinity(), CaretAffinity::Upstream);
        assert_eq!(buffer.caret_affinity(), CaretAffinity::Downstream);

        world.resource_mut::<UiRoutedEvents>().events = vec![key(
            target,
            UiKeyCode::Z,
            UiModifiers {
                shift: true,
                ..control
            },
        )];
        process_text_events(&mut world);
        assert_eq!(world.get::<TextBuffer>(target).unwrap().as_str(), "");

        world.resource_mut::<UiClipboard>().0 = "pasted\ntext\nwith lines".into();
        world.resource_mut::<UiRoutedEvents>().events = vec![key(target, UiKeyCode::V, control)];
        process_text_events(&mut world);
        assert_eq!(
            world.get::<TextBuffer>(target).unwrap().as_str(),
            "pasted\ntext\nwith lines"
        );
        world.resource_mut::<UiRoutedEvents>().events = vec![key(target, UiKeyCode::Z, control)];
        process_text_events(&mut world);
        assert_eq!(world.get::<TextBuffer>(target).unwrap().as_str(), "");
    }

    #[test]
    fn routed_ime_commit_and_read_only_behavior_are_respected() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<UiClipboard>();
        let target = world
            .spawn((UiTextBuffer(TextBuffer::default()), UiReadOnly))
            .id();
        world
            .resource_mut::<UiRoutedEvents>()
            .events
            .push(UiRoutedEvent::ImePreedit {
                window: Entity::PLACEHOLDER,
                target,
                text: "仮".into(),
                cursor_range: Some([0, 3]),
            });
        world
            .resource_mut::<UiRoutedEvents>()
            .events
            .push(UiRoutedEvent::ImeCommit {
                window: Entity::PLACEHOLDER,
                target,
                text: "仮".into(),
            });
        process_text_events(&mut world);
        assert_eq!(world.get::<UiTextBuffer>(target).unwrap().0.as_str(), "");
        assert!(
            world
                .get::<UiTextBuffer>(target)
                .unwrap()
                .0
                .composition()
                .is_some()
        );
    }

    #[test]
    fn direct_text_buffer_component_uses_external_clipboard_bridge() {
        let mut world = World::new();
        world.init_resource::<UiRoutedEvents>();
        world.init_resource::<UiClipboard>();
        let target = world
            .spawn((TextBuffer::new("abc"), UiExternalClipboard))
            .id();
        world.resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
            window: Entity::PLACEHOLDER,
            target,
            text: "!".into(),
        }];
        process_text_events(&mut world);
        assert_eq!(world.get::<TextBuffer>(target).unwrap().as_str(), "abc!");
        world
            .get_mut::<TextBuffer>(target)
            .unwrap()
            .set_selection(0, 3);
        world.resource_mut::<UiRoutedEvents>().events = vec![key(
            target,
            UiKeyCode::X,
            UiModifiers {
                control: true,
                ..UiModifiers::default()
            },
        )];
        process_text_events(&mut world);
        assert_eq!(world.get::<TextBuffer>(target).unwrap().as_str(), "abc!");
        assert!(world.resource::<UiClipboard>().0.is_empty());
    }
}

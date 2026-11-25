//! Application commands shared by menus and searchable palettes.

use bbui_core::search::MenuSearch;
use bbui_core::{UiKey, UiKeyCode, UiModifiers};
use bevy_ecs::world::World;

use crate::{MenuEntry, WidgetEvent, WidgetNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandShortcut {
    pub key: UiKeyCode,
    pub modifiers: UiModifiers,
}

impl CommandShortcut {
    pub const fn new(key: UiKeyCode, modifiers: UiModifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn matches(self, key: UiKeyCode, modifiers: UiModifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub shortcut: Option<CommandShortcut>,
    pub shortcut_hint: Option<String>,
}

impl Command {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            category: String::new(),
            enabled: true,
            shortcut: None,
            shortcut_hint: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: CommandShortcut, hint: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut);
        self.shortcut_hint = Some(hint.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    EmptyId,
    DuplicateId,
    ShortcutConflict,
}

#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: Vec<Command>,
    search: MenuSearch,
}

impl CommandRegistry {
    pub fn register(&mut self, command: Command) -> Result<(), CommandError> {
        if command.id.trim().is_empty() {
            return Err(CommandError::EmptyId);
        }
        if self.commands.iter().any(|entry| entry.id == command.id) {
            return Err(CommandError::DuplicateId);
        }
        if let Some(shortcut) = command.shortcut
            && self
                .commands
                .iter()
                .any(|entry| entry.shortcut == Some(shortcut))
        {
            return Err(CommandError::ShortcutConflict);
        }
        self.commands.push(command);
        Ok(())
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        let Some(command) = self.commands.iter_mut().find(|command| command.id == id) else {
            return false;
        };
        command.enabled = enabled;
        true
    }

    pub fn resolve_shortcut(&self, key: UiKeyCode, modifiers: UiModifiers) -> Option<&str> {
        self.commands
            .iter()
            .find(|command| {
                command.enabled
                    && command
                        .shortcut
                        .is_some_and(|chord| chord.matches(key, modifiers))
            })
            .map(|command| command.id.as_str())
    }

    pub fn ranked(&self, query: &str) -> Vec<&Command> {
        let labels: Vec<_> = self
            .commands
            .iter()
            .map(|command| {
                let text = format!(
                    "{} {} {} {}",
                    command.label,
                    command.category,
                    command.description,
                    command.shortcut_hint.as_deref().unwrap_or_default()
                );
                (command.id.as_str(), text)
            })
            .collect();
        let ranked = self
            .search
            .rank(query, labels.iter().map(|(id, text)| (*id, text.as_str())));
        ranked
            .into_iter()
            .filter_map(|id| {
                self.commands
                    .iter()
                    .find(|command| command.id == id && command.enabled)
            })
            .collect()
    }

    pub fn menu_entries(&self) -> Vec<MenuEntry> {
        self.commands
            .iter()
            .map(|command| {
                let mut entry = MenuEntry::command(&command.id, &command.label);
                entry.enabled = command.enabled;
                entry.shortcut = command.shortcut_hint.clone();
                entry
            })
            .collect()
    }

    pub fn record_activation(&mut self, id: &str) -> bool {
        if !self
            .commands
            .iter()
            .any(|command| command.id == id && command.enabled)
        {
            return false;
        }
        self.search.pick(id);
        true
    }
}

/// Host-mounted palette surface. Keep it in an overlay or other app-owned layout region.
#[derive(Debug, Default)]
pub struct CommandPalette {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

impl CommandPalette {
    pub fn show(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn view(&self, registry: &CommandRegistry) -> WidgetNode {
        if !self.open {
            return WidgetNode::column([]).with_key("command-palette:closed");
        }
        let results = registry.ranked(&self.query);
        WidgetNode::column([
            WidgetNode::label("Commands").with_key("command-palette:title"),
            WidgetNode::text_field("Search commands", &self.query)
                .with_key("command-palette:query"),
            WidgetNode::column(results.into_iter().enumerate().map(|(index, command)| {
                let label = match &command.shortcut_hint {
                    Some(hint) => format!("{}  {}", command.label, hint),
                    None => command.label.clone(),
                };
                let label = if index == self.selected {
                    format!("› {label}")
                } else {
                    label
                };
                WidgetNode::button(label).with_key(format!("command-palette:action:{}", command.id))
            }))
            .with_key("command-palette:results"),
        ])
        .with_key("command-palette:root")
        .with_role("command-palette")
    }

    /// Route widget events from a mounted palette. Returns an activated command ID.
    pub fn handle_widget_event(
        &mut self,
        world: &World,
        event: &WidgetEvent,
        registry: &mut CommandRegistry,
    ) -> Option<String> {
        let (entity, text) = match event {
            WidgetEvent::Activated { target } => (*target, None),
            WidgetEvent::TextChanged { target, text } => (*target, Some(text.as_str())),
            _ => return None,
        };
        let key = world.get::<UiKey>(entity)?.0.as_str();
        if key == "command-palette:query" {
            if let Some(text) = text {
                self.query = text.to_owned();
                self.selected = 0;
            }
            return None;
        }
        let id = key.strip_prefix("command-palette:action:")?;
        if registry.record_activation(id) {
            self.open = false;
            return Some(id.to_owned());
        }
        None
    }

    /// Handle palette navigation when the host routes an unconsumed key event.
    pub fn handle_key(&mut self, key: UiKeyCode, registry: &mut CommandRegistry) -> Option<String> {
        if !self.open {
            return None;
        }
        let count = registry.ranked(&self.query).len();
        match key {
            UiKeyCode::Escape => self.open = false,
            UiKeyCode::ArrowDown if count > 0 => self.selected = (self.selected + 1) % count,
            UiKeyCode::ArrowUp if count > 0 => self.selected = (self.selected + count - 1) % count,
            UiKeyCode::Enter if count > 0 => {
                let id = registry.ranked(&self.query)[self.selected.min(count - 1)]
                    .id
                    .clone();
                if registry.record_activation(&id) {
                    self.open = false;
                    return Some(id);
                }
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_rejects_conflicts_and_respects_availability() {
        let mut registry = CommandRegistry::default();
        let chord = CommandShortcut::new(
            UiKeyCode::N,
            UiModifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(
            registry.register(Command::new("new", "New").with_shortcut(chord, "Ctrl+N")),
            Ok(())
        );
        assert_eq!(
            registry.register(Command::new("other", "Other").with_shortcut(chord, "Ctrl+N")),
            Err(CommandError::ShortcutConflict)
        );
        assert_eq!(
            registry.resolve_shortcut(UiKeyCode::N, chord.modifiers),
            Some("new")
        );
        assert!(registry.set_enabled("new", false));
        assert_eq!(
            registry.resolve_shortcut(UiKeyCode::N, chord.modifiers),
            None
        );
        assert!(registry.ranked("new").is_empty());
    }

    #[test]
    fn palette_search_and_keyboard_activation() {
        let mut registry = CommandRegistry::default();
        assert_eq!(
            registry.register(Command::new("export", "Export image")),
            Ok(())
        );
        assert_eq!(registry.register(Command::new("open", "Open file")), Ok(()));
        let mut palette = CommandPalette::default();
        palette.show();
        palette.query = "expo".into();
        assert_eq!(
            palette.handle_key(UiKeyCode::Enter, &mut registry),
            Some("export".into())
        );
        assert!(!palette.open);
    }
}

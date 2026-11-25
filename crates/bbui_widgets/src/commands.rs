//! Application commands shared by menus and searchable palettes.

use bbui_core::search::MenuSearch;
use bbui_core::{UiKey, UiKeyCode, UiModifiers};
use bbui_layout::{UiAlign, UiEdges, UiLayoutNode, UiLength};
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bevy_ecs::world::World;

use crate::{MenuEntry, UiBorderEffect, UiEdgeGlow, UiVisualEffects, WidgetEvent, WidgetNode};

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

/// Paint and copy for the palette surface. All colors resolve from the active theme
/// unless callers replace the corresponding inline style or visual effect.
#[derive(Debug, Clone)]
pub struct CommandPaletteAppearance {
    pub title: String,
    pub subtitle: Option<String>,
    pub search_placeholder: String,
    pub empty_message: String,
    pub footer: Option<String>,
    pub width: UiLength,
    pub padding: UiEdges,
    pub gap: f32,
    pub row_height: f32,
    pub shortcut_column_width: f32,
    pub surface_style: UiInlineStyle,
    pub title_style: UiInlineStyle,
    pub subtitle_style: UiInlineStyle,
    pub section_style: UiInlineStyle,
    pub search_style: UiInlineStyle,
    pub result_style: UiInlineStyle,
    pub selected_style: UiInlineStyle,
    pub selected_effects: Option<UiVisualEffects>,
    pub detail_style: UiInlineStyle,
    pub shortcut_style: UiInlineStyle,
    pub shortcut_surface_style: UiInlineStyle,
    pub footer_style: UiInlineStyle,
    pub surface_effects: Option<UiVisualEffects>,
}

fn text_style(size: f32, color: ThemeColorToken) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(size));
    style.set_color_token(UiStyleProperty::Foreground, color);
    style
}

impl Default for CommandPaletteAppearance {
    fn default() -> Self {
        let mut surface_style = UiInlineStyle::default();
        surface_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Surface);
        surface_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(12.0));
        surface_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(0.0));
        let mut search_style = text_style(14.0, ThemeColorToken::Text);
        search_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Background);
        search_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        search_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(6.0));
        let mut result_style = text_style(14.0, ThemeColorToken::Text);
        result_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Background);
        result_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(6.0));
        let mut selected_style = result_style.clone();
        selected_style.set_color_token(
            UiStyleProperty::Background,
            ThemeColorToken::SelectedSurface,
        );
        let mut shortcut_surface_style = UiInlineStyle::default();
        shortcut_surface_style
            .set_color_token(UiStyleProperty::Background, ThemeColorToken::Background);
        shortcut_surface_style
            .set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        shortcut_surface_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        shortcut_surface_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(4.0));
        Self {
            title: "Quick actions".into(),
            subtitle: Some("Find a command and keep moving.".into()),
            search_placeholder: "Search commands".into(),
            empty_message: "No matching commands".into(),
            footer: Some("↑ ↓  Navigate     Enter  Run     Esc  Close".into()),
            width: UiLength::Px(620.0),
            padding: UiEdges::all(22.0),
            gap: 10.0,
            row_height: 38.0,
            shortcut_column_width: 80.0,
            surface_style,
            title_style: text_style(20.0, ThemeColorToken::Text),
            subtitle_style: text_style(12.0, ThemeColorToken::MutedText),
            section_style: text_style(10.5, ThemeColorToken::MutedText),
            search_style,
            result_style,
            selected_style,
            selected_effects: Some(
                UiVisualEffects::default().with_border(UiBorderEffect::solid(
                    ThemeColorToken::Accent,
                    1.5,
                    6.0,
                )),
            ),
            detail_style: text_style(11.5, ThemeColorToken::MutedText),
            shortcut_style: text_style(11.5, ThemeColorToken::MutedText),
            shortcut_surface_style,
            footer_style: text_style(11.0, ThemeColorToken::MutedText),
            surface_effects: Some(
                UiVisualEffects::default()
                    .with_border(
                        UiBorderEffect::gradient(
                            [
                                ThemeColorToken::Border,
                                ThemeColorToken::Accent,
                                ThemeColorToken::Border,
                            ],
                            1.5,
                            12.0,
                        )
                        .with_strength(0.75),
                    )
                    .with_edge_glow(
                        UiEdgeGlow::solid(ThemeColorToken::Accent, 14.0, 0.0).with_strength(0.16),
                    ),
            ),
        }
    }
}

/// Host-mounted palette surface. Keep it in an overlay or other app-owned layout region.
#[derive(Debug)]
pub struct CommandPalette {
    pub open: bool,
    pub query: String,
    pub selected: usize,
    /// Maximum number of ranked results rendered and visited by keyboard navigation.
    pub visible_limit: usize,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
            visible_limit: 8,
        }
    }
}

impl CommandPalette {
    /// Stable key for a host-rendered search field.
    pub const QUERY_KEY: &'static str = "command-palette:query";

    /// Stable key for a host-rendered command control.
    pub fn action_key(id: &str) -> String {
        format!("command-palette:action:{id}")
    }

    pub fn show(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
    }

    pub fn view(&self, registry: &CommandRegistry) -> WidgetNode {
        self.view_with(registry, &CommandPaletteAppearance::default())
    }

    /// Build a themed surface with caller-selected copy, dimensions, styles, and effects.
    pub fn view_with(
        &self,
        registry: &CommandRegistry,
        appearance: &CommandPaletteAppearance,
    ) -> WidgetNode {
        if !self.open {
            return WidgetNode::column([]).with_key("command-palette:closed");
        }
        let results = registry.ranked(&self.query);
        let visible = results.len().min(self.visible_limit.max(1));
        let mut children = vec![
            WidgetNode::label(&appearance.title)
                .with_key("command-palette:title")
                .with_height(UiLength::Px(29.0))
                .with_style(appearance.title_style.clone()),
        ];
        if let Some(subtitle) = &appearance.subtitle {
            children.push(
                WidgetNode::label(subtitle)
                    .with_key("command-palette:subtitle")
                    .with_height(UiLength::Px(21.0))
                    .with_style(appearance.subtitle_style.clone()),
            );
        }
        children.push(
            WidgetNode::text_field(
                if self.query.is_empty() {
                    &appearance.search_placeholder
                } else {
                    ""
                },
                &self.query,
            )
            .with_key(Self::QUERY_KEY)
            .with_accessible_name(&appearance.search_placeholder)
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(40.0))
            .with_style(appearance.search_style.clone()),
        );
        children.push(
            WidgetNode::label(format!("{} RESULTS", visible))
                .with_key("command-palette:count")
                .with_height(UiLength::Px(21.0))
                .with_style(appearance.section_style.clone()),
        );
        let rows = results
            .into_iter()
            .take(visible)
            .enumerate()
            .map(|(index, command)| {
                let selected = index == self.selected.min(visible.saturating_sub(1));
                let mut button = WidgetNode::button(if selected {
                    format!("›  {}", command.label)
                } else {
                    format!("   {}", command.label)
                })
                .with_key(Self::action_key(&command.id))
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(appearance.row_height))
                .with_accessible_name(if command.description.is_empty() {
                    command.label.clone()
                } else {
                    format!("{}; {}", command.label, command.description)
                })
                .with_style(if selected {
                    appearance.selected_style.clone()
                } else {
                    appearance.result_style.clone()
                });
                if selected && let Some(effects) = &appearance.selected_effects {
                    button = button.with_visual_effects(effects.clone());
                }
                let shortcut = if let Some(hint) = command
                    .shortcut_hint
                    .as_deref()
                    .filter(|hint| !hint.is_empty())
                {
                    let column_width = appearance.shortcut_column_width.max(34.0);
                    let key_width =
                        (hint.chars().count() as f32 * 7.5 + 18.0).clamp(34.0, column_width);
                    let keycap = WidgetNode::boxed([WidgetNode::label(hint)
                        .with_height(UiLength::Px(16.0))
                        .with_style(appearance.shortcut_style.clone())])
                    .with_width(UiLength::Px(key_width))
                    .with_height(UiLength::Px(27.0))
                    .with_padding(UiEdges::all(5.0))
                    .with_style(appearance.shortcut_surface_style.clone());
                    let mut layout = UiLayoutNode::row(0.0);
                    layout.align_items = UiAlign::Center;
                    WidgetNode::row([
                        WidgetNode::column([]).with_width(UiLength::Fill(1.0)),
                        keycap,
                    ])
                    .with_layout(layout)
                    .with_width(UiLength::Px(column_width))
                    .with_height(UiLength::Px(appearance.row_height))
                } else {
                    WidgetNode::column([])
                        .with_width(UiLength::Px(appearance.shortcut_column_width.max(34.0)))
                };
                let detail = if command.description.is_empty() {
                    command.category.clone()
                } else if command.category.is_empty() {
                    command.description.clone()
                } else {
                    format!("{}  ·  {}", command.category, command.description)
                };
                let mut content = vec![
                    WidgetNode::row([button, shortcut])
                        .with_width(UiLength::Fill(1.0))
                        .with_gap(8.0),
                ];
                if !detail.is_empty() {
                    content.push(
                        WidgetNode::label(detail)
                            .with_height(UiLength::Px(18.0))
                            .with_style(appearance.detail_style.clone()),
                    );
                }
                WidgetNode::column(content)
                    .with_key(format!("command-palette:result:{}", command.id))
                    .with_width(UiLength::Fill(1.0))
                    .with_gap(2.0)
            });
        if visible == 0 {
            children.push(
                WidgetNode::label(&appearance.empty_message)
                    .with_key("command-palette:empty")
                    .with_height(UiLength::Px(48.0))
                    .with_style(appearance.detail_style.clone()),
            );
        } else {
            children.push(
                WidgetNode::column(rows)
                    .with_key("command-palette:results")
                    .with_width(UiLength::Fill(1.0))
                    .with_gap(appearance.gap),
            );
        }
        if let Some(footer) = &appearance.footer {
            children.push(
                WidgetNode::label(footer)
                    .with_key("command-palette:footer")
                    .with_height(UiLength::Px(22.0))
                    .with_style(appearance.footer_style.clone()),
            );
        }
        let mut surface = WidgetNode::boxed(children)
            .with_key("command-palette:root")
            .with_width(appearance.width)
            .with_padding(appearance.padding)
            .with_gap(appearance.gap)
            .with_style(appearance.surface_style.clone());
        if let Some(effects) = &appearance.surface_effects {
            surface = surface.with_visual_effects(effects.clone());
        }
        surface
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
        if key == Self::QUERY_KEY {
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
        let count = registry
            .ranked(&self.query)
            .len()
            .min(self.visible_limit.max(1));
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

    fn by_key<'a>(node: &'a WidgetNode, key: &str) -> Option<&'a WidgetNode> {
        if node.key.as_deref() == Some(key) {
            return Some(node);
        }
        node.children.iter().find_map(|child| by_key(child, key))
    }

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

    #[test]
    fn palette_appearance_and_visible_limit_keep_rendering_and_navigation_in_sync() {
        let mut registry = CommandRegistry::default();
        for id in ["alpha", "beta", "gamma"] {
            assert_eq!(registry.register(Command::new(id, id)), Ok(()));
        }
        let mut palette = CommandPalette::default();
        palette.show();
        palette.visible_limit = 2;
        let appearance = CommandPaletteAppearance {
            title: "Actions".into(),
            width: UiLength::Fill(1.0),
            surface_effects: None,
            ..Default::default()
        };
        let view = palette.view_with(&registry, &appearance);
        assert_eq!(view.visual_effects, None);
        assert_eq!(
            view.layout.map(|layout| layout.size.width),
            Some(UiLength::Fill(1.0))
        );
        assert!(matches!(
            &by_key(&view, "command-palette:title").map(|node| &node.spec),
            Some(crate::WidgetSpec::Label { text }) if text == "Actions"
        ));
        assert_eq!(
            by_key(&view, "command-palette:results").map(|node| node.children.len()),
            Some(2)
        );
        palette.handle_key(UiKeyCode::ArrowDown, &mut registry);
        palette.handle_key(UiKeyCode::ArrowDown, &mut registry);
        assert_eq!(palette.selected, 0);
        palette.query = "no such command".into();
        let empty = palette.view_with(&registry, &appearance);
        assert!(by_key(&empty, "command-palette:empty").is_some());
    }
}

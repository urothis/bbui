//! Application commands shared by menus and searchable palettes.

use bbui_core::search::MenuSearch;
use bbui_core::{
    UiChildren, UiDirty, UiDirtyDomain, UiFocus, UiKey, UiKeyCode, UiModalScopes, UiModifiers,
};
use bbui_layout::{UiAlign, UiEdges, UiLayoutNode, UiLength, UiScroll};
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
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
    /// Optional short glyph shown before the command name.
    pub icon: Option<String>,
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
            icon: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: CommandShortcut, hint: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut);
        self.shortcut_hint = Some(hint.into());
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Extra text presentation for a palette action. The button remains the hit target.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct PaletteActionText {
    pub query: String,
    pub icon: Option<String>,
    pub highlight_color: ThemeColorToken,
    pub icon_color: ThemeColorToken,
    pub icon_width: f32,
    pub reserved_trailing_width: f32,
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

    pub fn unregister(&mut self, id: &str) -> bool {
        let Some(index) = self.commands.iter().position(|command| command.id == id) else {
            return false;
        };
        self.commands.remove(index);
        true
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
    pub max_width: f32,
    pub max_height: Option<f32>,
    pub max_results_height: f32,
    pub min_results_height: f32,
    pub padding: UiEdges,
    pub gap: f32,
    pub row_height: f32,
    pub result_padding_x: f32,
    pub shortcut_column_width: f32,
    pub icon_width: f32,
    pub show_icons: bool,
    pub show_shortcuts: bool,
    pub show_descriptions: bool,
    pub highlight_color: ThemeColorToken,
    pub icon_color: ThemeColorToken,
    pub surface_style: UiInlineStyle,
    pub title_style: UiInlineStyle,
    pub subtitle_style: UiInlineStyle,
    pub section_style: UiInlineStyle,
    pub search_style: UiInlineStyle,
    pub result_style: UiInlineStyle,
    pub selected_style: UiInlineStyle,
    pub selected_effects: Option<UiVisualEffects>,
    pub detail_style: UiInlineStyle,
    pub category_style: UiInlineStyle,
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
            max_width: 620.0,
            max_height: Some(600.0),
            max_results_height: 330.0,
            min_results_height: 0.0,
            padding: UiEdges::all(22.0),
            gap: 10.0,
            row_height: 38.0,
            result_padding_x: 12.0,
            shortcut_column_width: 116.0,
            icon_width: 24.0,
            show_icons: true,
            show_shortcuts: true,
            show_descriptions: true,
            highlight_color: ThemeColorToken::Accent,
            icon_color: ThemeColorToken::Accent,
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
            category_style: text_style(11.5, ThemeColorToken::Accent),
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

/// Optional sections for apps that need different palette structure. A custom
/// result must use `CommandPalette::action_key` on its activating control.
pub type CommandResultBuilder<'a> = dyn Fn(&Command, bool) -> WidgetNode + 'a;

#[derive(Default)]
pub struct CommandPaletteSlots<'a> {
    pub header: Option<WidgetNode>,
    pub empty: Option<WidgetNode>,
    pub footer: Option<WidgetNode>,
    pub result: Option<&'a CommandResultBuilder<'a>>,
}

/// Focus and modal input scope for a mounted palette. The host owns its overlay.
#[derive(Debug)]
pub struct CommandPaletteSession {
    window: Entity,
    previous_focus: Option<Entity>,
    previous_scope: Option<Entity>,
}

impl CommandPaletteSession {
    pub fn begin(world: &mut World, window: Entity, root: Entity) -> Option<Self> {
        fn find(world: &World, entity: Entity) -> Option<Entity> {
            if world
                .get::<UiKey>(entity)
                .is_some_and(|key| key.0 == CommandPalette::QUERY_KEY)
            {
                return Some(entity);
            }
            world
                .get::<UiChildren>(entity)?
                .0
                .iter()
                .find_map(|child| find(world, *child))
        }
        let query = find(world, root)?;
        let previous_focus = world
            .get_resource::<UiFocus>()?
            .state(window)
            .and_then(|state| state.focused);
        world.init_resource::<UiModalScopes>();
        let previous_scope = world.resource_mut::<UiModalScopes>().0.insert(window, root);
        if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
            dirty.mark_all(UiDirtyDomain::Accessibility);
        }
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(window);
        state.focused = Some(query);
        state.selection = None;
        state.captured = None;
        state.captured_button = None;
        focus.set_active(window);
        Some(Self {
            window,
            previous_focus,
            previous_scope,
        })
    }

    pub fn end(self, world: &mut World) {
        let mut scopes = world.resource_mut::<UiModalScopes>();
        if let Some(previous) = self.previous_scope {
            scopes.0.insert(self.window, previous);
        } else {
            scopes.0.remove(&self.window);
        }
        if let Some(mut dirty) = world.get_resource_mut::<UiDirty>() {
            dirty.mark_all(UiDirtyDomain::Accessibility);
        }
        let restored = self
            .previous_focus
            .filter(|entity| bbui_core::is_interactable_in_window(world, *entity, self.window));
        let mut focus = world.resource_mut::<UiFocus>();
        let state = focus.state_mut(self.window);
        state.focused = restored;
        state.selection = None;
        focus.set_active(self.window);
    }
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
    fn results_viewport_limit(appearance: &CommandPaletteAppearance) -> f32 {
        appearance
            .max_height
            .map_or(appearance.max_results_height, |height| {
                appearance
                    .max_results_height
                    .min((height - 220.0).max(40.0))
            })
    }
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

    /// Keep keyboard selection inside the bounded results viewport.
    pub fn scroll_selected_into_view(
        &self,
        world: &mut World,
        root: Entity,
        appearance: &CommandPaletteAppearance,
    ) {
        if !self.open {
            return;
        }
        let stride = appearance.row_height + 20.0 + appearance.gap;
        let bottom = (self.selected + 1) as f32 * stride;
        let top = self.selected as f32 * stride;
        fn find(world: &World, entity: Entity) -> Option<Entity> {
            if world
                .get::<UiKey>(entity)
                .is_some_and(|key| key.0 == "command-palette:results-scroll")
            {
                return Some(entity);
            }
            world
                .get::<UiChildren>(entity)?
                .0
                .iter()
                .find_map(|child| find(world, *child))
        }
        if let Some(entity) = find(world, root)
            && let Some(mut scroll) = world.get_mut::<UiScroll>(entity)
        {
            if top < scroll.offset[1] {
                scroll.offset[1] = top;
            } else if bottom > scroll.offset[1] + Self::results_viewport_limit(appearance) {
                scroll.offset[1] = bottom - Self::results_viewport_limit(appearance);
            }
        }
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
        self.view_with_slots(registry, appearance, CommandPaletteSlots::default())
    }

    /// Build a palette with optional caller-owned sections and result controls.
    pub fn view_with_slots(
        &self,
        registry: &CommandRegistry,
        appearance: &CommandPaletteAppearance,
        slots: CommandPaletteSlots<'_>,
    ) -> WidgetNode {
        if !self.open {
            return WidgetNode::column([]).with_key("command-palette:closed");
        }
        let results = registry.ranked(&self.query);
        let visible = results.len().min(self.visible_limit.max(1));
        let mut children = Vec::new();
        if let Some(header) = slots.header {
            children.push(header);
        } else {
            children.push(
                WidgetNode::label(&appearance.title)
                    .with_key("command-palette:title")
                    .with_height(UiLength::Px(29.0))
                    .with_style(appearance.title_style.clone()),
            );
            if let Some(subtitle) = &appearance.subtitle {
                children.push(
                    WidgetNode::label(subtitle)
                        .with_key("command-palette:subtitle")
                        .with_height(UiLength::Px(21.0))
                        .with_style(appearance.subtitle_style.clone()),
                );
            }
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
            .with_padding(UiEdges::all(8.0))
            .with_style(appearance.search_style.clone()),
        );
        children.push(
            WidgetNode::label(format!("{} RESULTS", visible))
                .with_key("command-palette:count")
                .with_height(UiLength::Px(21.0))
                .with_style(appearance.section_style.clone()),
        );
        if visible == 0 {
            children.push(slots.empty.unwrap_or_else(|| {
                WidgetNode::label(&appearance.empty_message)
                    .with_key("command-palette:empty")
                    .with_height(UiLength::Px(48.0))
                    .with_style(appearance.detail_style.clone())
            }));
        } else {
            let rows = results
                .into_iter()
                .take(visible)
                .enumerate()
                .map(|(index, command)| {
                    let selected = index == self.selected.min(visible - 1);
                    if let Some(render) = slots.result {
                        return render(command, selected);
                    }
                    self.default_result(command, selected, appearance)
                });
            let mut scroll_layout = UiLayoutNode::column(0.0);
            let limit = Self::results_viewport_limit(appearance);
            scroll_layout.size.height = UiLength::Px(
                (visible as f32 * (appearance.row_height + 20.0 + appearance.gap))
                    .clamp(appearance.min_results_height.min(limit), limit),
            );
            children.push(
                WidgetNode::scroll([WidgetNode::column(rows)
                    .with_key("command-palette:results")
                    .with_width(UiLength::Fill(1.0))
                    .with_padding(UiEdges {
                        left: 0.0,
                        top: 0.0,
                        right: 10.0,
                        bottom: 0.0,
                    })
                    .with_gap(appearance.gap)])
                .with_key("command-palette:results-scroll")
                .with_layout(scroll_layout),
            );
        }
        if let Some(footer) = slots.footer {
            children.push(footer);
        } else if let Some(footer) = &appearance.footer {
            children.push(
                WidgetNode::label(footer)
                    .with_key("command-palette:footer")
                    .with_height(UiLength::Px(22.0))
                    .with_style(appearance.footer_style.clone()),
            );
        }
        let mut layout = UiLayoutNode::column(appearance.gap);
        layout.size.width = appearance.width;
        layout.max_size[0] = Some(appearance.max_width);
        layout.max_size[1] = appearance.max_height;
        layout.padding = appearance.padding;
        let mut surface_style = appearance.surface_style.clone();
        surface_style
            .0
            .entry(UiStyleProperty::Padding)
            .or_insert(UiStyleValue::Edges(appearance.padding));
        surface_style
            .0
            .entry(UiStyleProperty::Gap)
            .or_insert(UiStyleValue::Scalar(appearance.gap));
        let mut surface = WidgetNode::boxed(children)
            .with_key("command-palette:root")
            .with_layout(layout)
            .with_style(surface_style);
        if let Some(effects) = &appearance.surface_effects {
            surface = surface.with_visual_effects(effects.clone());
        }
        surface
    }

    fn default_result(
        &self,
        command: &Command,
        selected: bool,
        appearance: &CommandPaletteAppearance,
    ) -> WidgetNode {
        let mut button = WidgetNode::button(&command.label)
            .with_key(Self::action_key(&command.id))
            .with_role("command-palette-action")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(appearance.row_height))
            .with_padding(UiEdges::horizontal_vertical(
                appearance.result_padding_x,
                0.0,
            ))
            .with_accessible_name(if command.description.is_empty() {
                command.label.clone()
            } else {
                format!("{}; {}", command.label, command.description)
            })
            .with_palette_action_text(PaletteActionText {
                query: self.query.clone(),
                icon: appearance
                    .show_icons
                    .then(|| command.icon.clone())
                    .flatten(),
                highlight_color: appearance.highlight_color,
                icon_color: appearance.icon_color,
                icon_width: appearance.icon_width,
                reserved_trailing_width: 8.0,
            })
            .with_style(if selected {
                appearance.selected_style.clone()
            } else {
                appearance.result_style.clone()
            });
        if selected && let Some(effects) = &appearance.selected_effects {
            button = button.with_visual_effects(effects.clone());
        }
        let shortcut = if appearance.show_shortcuts {
            command
                .shortcut_hint
                .as_deref()
                .filter(|hint| !hint.is_empty())
        } else {
            None
        };
        let shortcut = if let Some(hint) = shortcut {
            let column_width = appearance.shortcut_column_width.max(34.0);
            let key_width = (hint.chars().count() as f32 * 7.5 + 18.0).clamp(34.0, column_width);
            let badge_padding = UiEdges::horizontal_vertical(8.0, 5.0);
            let mut badge_style = appearance.shortcut_surface_style.clone();
            badge_style
                .0
                .entry(UiStyleProperty::Padding)
                .or_insert(UiStyleValue::Edges(badge_padding));
            badge_style
                .0
                .entry(UiStyleProperty::Gap)
                .or_insert(UiStyleValue::Scalar(0.0));
            let mut keycap_layout = UiLayoutNode::column(0.0);
            keycap_layout.align_items = UiAlign::Center;
            keycap_layout.justify_content = bbui_layout::UiJustify::Center;
            let keycap = WidgetNode::boxed([WidgetNode::label(hint)
                .with_role("centered-label")
                .with_height(UiLength::Px(16.0))
                .with_style(appearance.shortcut_style.clone())])
            .with_layout(keycap_layout)
            .with_width(UiLength::Px(key_width))
            .with_height(UiLength::Px(27.0))
            .with_padding(badge_padding)
            .with_style(badge_style);
            let mut layout = UiLayoutNode::row(0.0);
            layout.align_items = UiAlign::Center;
            WidgetNode::row([
                WidgetNode::column([]).with_width(UiLength::Fill(1.0)),
                keycap,
            ])
            .with_layout(layout)
            .with_width(UiLength::Px(column_width))
            .with_height(UiLength::Px(appearance.row_height))
        } else if appearance.show_shortcuts {
            WidgetNode::column([])
                .with_width(UiLength::Px(appearance.shortcut_column_width.max(34.0)))
        } else {
            WidgetNode::column([]).with_width(UiLength::Px(0.0))
        };
        let mut content = vec![if appearance.show_shortcuts {
            WidgetNode::row([button, shortcut])
                .with_width(UiLength::Fill(1.0))
                .with_gap(8.0)
        } else {
            WidgetNode::row([button]).with_width(UiLength::Fill(1.0))
        }];
        if !command.category.is_empty() || !command.description.is_empty() {
            let mut metadata = Vec::new();
            if !command.category.is_empty() {
                metadata.push(
                    WidgetNode::label(&command.category)
                        .with_style(appearance.category_style.clone()),
                );
            }
            if appearance.show_descriptions && !command.description.is_empty() {
                metadata.push(
                    WidgetNode::label(&command.description)
                        .with_width(UiLength::Fill(1.0))
                        .with_style(appearance.detail_style.clone()),
                );
            }
            content.push(
                WidgetNode::row([
                    WidgetNode::column([]).with_width(UiLength::Px(appearance.result_padding_x)),
                    WidgetNode::row(metadata)
                        .with_width(UiLength::Fill(1.0))
                        .with_height(UiLength::Px(18.0))
                        .with_gap(8.0),
                ])
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Px(18.0)),
            );
        }
        WidgetNode::column(content)
            .with_key(format!("command-palette:result:{}", command.id))
            .with_width(UiLength::Fill(1.0))
            .with_gap(2.0)
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

    #[test]
    fn palette_slots_and_bounded_results_keep_action_keys() {
        let mut registry = CommandRegistry::default();
        registry.register(Command::new("one", "One")).unwrap();
        let mut palette = CommandPalette::default();
        palette.show();
        let custom = |command: &Command, _: bool| {
            WidgetNode::button(&command.label).with_key(CommandPalette::action_key(&command.id))
        };
        let view = palette.view_with_slots(
            &registry,
            &CommandPaletteAppearance::default(),
            CommandPaletteSlots {
                header: Some(WidgetNode::label("Custom header").with_key("custom-header")),
                footer: Some(WidgetNode::label("Custom footer").with_key("custom-footer")),
                result: Some(&custom),
                ..Default::default()
            },
        );
        assert!(by_key(&view, "custom-header").is_some());
        assert!(by_key(&view, "custom-footer").is_some());
        assert!(by_key(&view, &CommandPalette::action_key("one")).is_some());
        let scroll = by_key(&view, "command-palette:results-scroll").unwrap();
        assert!(scroll.scrollable);
        assert!(matches!(
            scroll.layout.unwrap().size.height,
            UiLength::Px(_)
        ));
    }

    #[test]
    fn palette_session_scopes_focus_and_restores_it() {
        let mut world = World::new();
        world.init_resource::<UiFocus>();
        let window = world.spawn_empty().id();
        let previous = world
            .spawn((bbui_core::UiNode, bbui_core::UiFocusable))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(previous);
        let query = world
            .spawn((bbui_core::UiNode, UiKey(CommandPalette::QUERY_KEY.into())))
            .id();
        let root = world
            .spawn((bbui_core::UiNode, UiChildren(vec![query])))
            .id();
        let session = CommandPaletteSession::begin(&mut world, window, root).unwrap();
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().focused,
            Some(query)
        );
        assert_eq!(
            world.resource::<UiModalScopes>().0.get(&window),
            Some(&root)
        );
        session.end(&mut world);
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().focused,
            Some(previous)
        );
        assert!(!world.resource::<UiModalScopes>().0.contains_key(&window));
    }
}

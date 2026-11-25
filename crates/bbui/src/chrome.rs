//! Composable desktop and compact-window chrome for application shells.

use bbui_core::{UiKey, UiPointerButton, UiRoutedEvent};
use bbui_layout::{UiEdges, UiLength};
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bbui_widgets::{CommandRegistry, WidgetEvent, WidgetNode};
use bevy::prelude::*;

const MOBILE_BREAKPOINT: f32 = 600.0;

fn sheet_foreground(mut node: WidgetNode) -> WidgetNode {
    let style = node.style.get_or_insert_with(UiInlineStyle::default);
    style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(2));
    style.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::Text);
    node.children = node.children.into_iter().map(sheet_foreground).collect();
    node
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChromeAction {
    CloseRequested,
    Minimize,
    ToggleMaximize,
    Command(String),
    Navigate(String),
    DismissSheet,
}

/// The native titlebar button inset on macOS, in logical pixels.
pub const MACOS_BUTTON_INSET: f32 = 90.0;

#[derive(Debug, Clone)]
pub struct DesktopChrome {
    pub title: String,
}

impl DesktopChrome {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
        }
    }

    pub fn view(&self, commands: &CommandRegistry) -> WidgetNode {
        let mut row = Vec::new();
        if cfg!(target_os = "macos") {
            row.push(WidgetNode::column([]).with_width(UiLength::Px(MACOS_BUTTON_INSET)));
        }
        row.push(
            WidgetNode::button(&self.title)
                .with_key("chrome:drag")
                .with_role("titlebar-drag")
                .with_width(UiLength::Fill(1.0)),
        );
        row.push(WidgetNode::menu("Commands", commands.menu_entries()).with_key("chrome:commands"));
        if !cfg!(target_os = "macos") {
            for (key, label, name) in [
                ("minimize", "−", "Minimize"),
                ("maximize", "□", "Maximize"),
                ("close", "×", "Close"),
            ] {
                row.push(
                    WidgetNode::button(label)
                        .with_key(format!("chrome:{key}"))
                        .with_accessible_name(name)
                        .with_width(UiLength::Px(36.0)),
                );
            }
        }
        WidgetNode::row(row)
            .with_key("chrome:desktop")
            .with_role("window-chrome")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(40.0))
            .with_padding(UiEdges::all(6.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileNavItem {
    pub id: String,
    pub label: String,
}

impl MobileNavItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MobileChrome {
    pub title: String,
    pub navigation: Vec<MobileNavItem>,
    pub selected: Option<String>,
    pub sheet_title: Option<String>,
}

impl MobileChrome {
    pub fn new(title: impl Into<String>, navigation: Vec<MobileNavItem>) -> Self {
        Self {
            title: title.into(),
            navigation,
            selected: None,
            sheet_title: None,
        }
    }

    /// Build a toolbar, content region, optional sheet, and bottom navigation.
    /// The application supplies both the main page and sheet contents.
    pub fn view(
        &self,
        commands: &CommandRegistry,
        content: WidgetNode,
        sheet: Option<WidgetNode>,
    ) -> WidgetNode {
        let toolbar = WidgetNode::row([
            WidgetNode::label(&self.title).with_width(UiLength::Fill(1.0)),
            WidgetNode::menu("Actions", commands.menu_entries()).with_key("chrome:mobile:commands"),
        ])
        .with_key("chrome:mobile:toolbar")
        .with_role("mobile-toolbar")
        .with_height(UiLength::Px(52.0))
        .with_padding(UiEdges::all(8.0));
        let mut body = vec![
            toolbar,
            content
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0)),
        ];
        if let (Some(title), Some(sheet)) = (&self.sheet_title, sheet) {
            let mut sheet_style = UiInlineStyle::default();
            sheet_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Surface);
            sheet_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
            sheet_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
            sheet_style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(1));
            body.push(
                WidgetNode::column([
                    sheet_foreground(WidgetNode::row([
                        WidgetNode::label(title).with_width(UiLength::Fill(1.0)),
                        WidgetNode::button("Close sheet").with_key("chrome:mobile:sheet-close"),
                    ])),
                    sheet_foreground(sheet),
                ])
                .with_key("chrome:mobile:sheet")
                .with_role("box")
                .with_style(sheet_style)
                .with_padding(UiEdges::all(12.0)),
            );
        }
        body.push(
            WidgetNode::row(self.navigation.iter().map(|item| {
                let label = if self.selected.as_deref() == Some(&item.id) {
                    format!("● {}", item.label)
                } else {
                    item.label.clone()
                };
                WidgetNode::button(label)
                    .with_key(format!("chrome:mobile:nav:{}", item.id))
                    .with_width(UiLength::Fill(1.0))
            }))
            .with_key("chrome:mobile:bottom-nav")
            .with_role("mobile-bottom-nav")
            .with_height(UiLength::Px(56.0))
            .with_padding(UiEdges::all(6.0)),
        );
        WidgetNode::column(body)
            .with_key("chrome:mobile")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
    }

    pub fn navigate(&mut self, id: &str, title: impl Into<String>) -> bool {
        if !self.navigation.iter().any(|item| item.id == id) {
            return false;
        }
        self.selected = Some(id.to_owned());
        self.sheet_title = Some(title.into());
        true
    }
}

/// Choose a desktop titlebar or compact mobile chrome from logical window width.
pub fn compact_chrome(width: f32) -> bool {
    width < MOBILE_BREAKPOINT
}

/// Call on a routed primary press; a native move must start on the press edge.
pub fn handle_chrome_press(world: &mut World, event: &UiRoutedEvent) -> bool {
    let UiRoutedEvent::PointerDown {
        window,
        target,
        button: UiPointerButton::Primary,
        ..
    } = event
    else {
        return false;
    };
    if world
        .get::<UiKey>(*target)
        .is_none_or(|key| key.0 != "chrome:drag")
    {
        return false;
    }
    let Some(mut state) = world.get_mut::<Window>(*window) else {
        return false;
    };
    state.start_drag_move();
    true
}

/// Translate a chrome button activation into an application or window action.
pub fn chrome_action(world: &World, event: &WidgetEvent) -> Option<ChromeAction> {
    if let WidgetEvent::MenuCommand { target, command_id } = event {
        let key = world.get::<UiKey>(*target)?.0.as_str();
        if key == "chrome:commands" || key == "chrome:mobile:commands" {
            return Some(ChromeAction::Command(command_id.clone()));
        }
    }
    let WidgetEvent::Activated { target } = event else {
        return None;
    };
    let key = world.get::<UiKey>(*target)?.0.as_str();
    match key {
        "chrome:close" => Some(ChromeAction::CloseRequested),
        "chrome:minimize" => Some(ChromeAction::Minimize),
        "chrome:maximize" => Some(ChromeAction::ToggleMaximize),
        "chrome:mobile:sheet-close" => Some(ChromeAction::DismissSheet),
        _ => key
            .strip_prefix("chrome:mobile:nav:")
            .map(|id| ChromeAction::Navigate(id.to_owned())),
    }
}

/// Apply only native window actions. The host handles close confirmation and navigation.
pub fn apply_native_chrome_action(window: &mut Window, maximized: bool, action: &ChromeAction) {
    match action {
        ChromeAction::Minimize => window.set_minimized(true),
        ChromeAction::ToggleMaximize => window.set_maximized(!maximized),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_breakpoint_and_navigation_state() {
        assert!(!compact_chrome(600.0));
        assert!(compact_chrome(599.0));
        let mut chrome = MobileChrome::new("Editor", vec![MobileNavItem::new("files", "Files")]);
        assert!(!chrome.navigate("missing", "Missing"));
        assert!(chrome.navigate("files", "Files"));
        assert_eq!(chrome.selected.as_deref(), Some("files"));
        assert_eq!(chrome.sheet_title.as_deref(), Some("Files"));
    }

    #[test]
    fn controls_route_actions_and_drag_only_from_title_press() {
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let drag = world.spawn(UiKey("chrome:drag".into())).id();
        let close = world.spawn(UiKey("chrome:close".into())).id();
        let commands = world.spawn(UiKey("chrome:commands".into())).id();
        assert_eq!(
            chrome_action(
                &world,
                &WidgetEvent::MenuCommand {
                    target: commands,
                    command_id: "save".into(),
                }
            ),
            Some(ChromeAction::Command("save".into()))
        );
        assert_eq!(
            chrome_action(&world, &WidgetEvent::Activated { target: close }),
            Some(ChromeAction::CloseRequested)
        );
        assert!(handle_chrome_press(
            &mut world,
            &UiRoutedEvent::PointerDown {
                window,
                target: drag,
                button: UiPointerButton::Primary,
                position: [20.0, 10.0],
            }
        ));
        assert!(!handle_chrome_press(
            &mut world,
            &UiRoutedEvent::PointerDown {
                window,
                target: close,
                button: UiPointerButton::Primary,
                position: [20.0, 10.0],
            }
        ));
    }
}

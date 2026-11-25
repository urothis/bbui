//! Composable desktop and compact-window chrome for application shells.

use bbui_core::{UiKey, UiPointerButton, UiRoutedEvent};
use bbui_layout::{UiEdges, UiLength};
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bbui_widgets::{CommandRegistry, WidgetEvent, WidgetNode};
use bevy::prelude::*;

const MOBILE_BREAKPOINT: f32 = 600.0;

fn chrome_style(
    size: f32,
    foreground: ThemeColorToken,
    background: Option<ThemeColorToken>,
) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(size));
    style.set_color_token(UiStyleProperty::Foreground, foreground);
    if let Some(background) = background {
        style.set_color_token(UiStyleProperty::Background, background);
    }
    style
}

/// Theme-aware dimensions, labels, and paint for a desktop titlebar.
#[derive(Debug, Clone)]
pub struct DesktopChromeAppearance {
    pub height: f32,
    pub padding: UiEdges,
    pub gap: f32,
    pub control_width: f32,
    pub native_button_inset: f32,
    pub menu_label: String,
    pub minimize_label: String,
    pub maximize_label: String,
    pub close_label: String,
    pub show_window_controls: bool,
    pub surface_style: UiInlineStyle,
    pub title_style: UiInlineStyle,
    pub menu_style: UiInlineStyle,
    pub control_style: UiInlineStyle,
    pub close_style: UiInlineStyle,
}

impl Default for DesktopChromeAppearance {
    fn default() -> Self {
        let mut surface_style =
            chrome_style(13.0, ThemeColorToken::Text, Some(ThemeColorToken::Surface));
        surface_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        surface_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        let mut control_style = chrome_style(
            16.0,
            ThemeColorToken::MutedText,
            Some(ThemeColorToken::Background),
        );
        control_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(5.0));
        Self {
            height: 44.0,
            padding: UiEdges::horizontal_vertical(10.0, 6.0),
            gap: 6.0,
            control_width: 36.0,
            native_button_inset: MACOS_BUTTON_INSET,
            menu_label: "Commands".into(),
            minimize_label: "−".into(),
            maximize_label: "□".into(),
            close_label: "×".into(),
            show_window_controls: true,
            surface_style,
            title_style: chrome_style(14.0, ThemeColorToken::Text, Some(ThemeColorToken::Surface)),
            menu_style: chrome_style(
                12.0,
                ThemeColorToken::MutedText,
                Some(ThemeColorToken::Background),
            ),
            close_style: control_style.clone(),
            control_style,
        }
    }
}

/// Optional host controls placed between the drag region and command menu.
#[derive(Debug, Default)]
pub struct DesktopChromeSlots {
    pub leading: Option<WidgetNode>,
    pub trailing: Option<WidgetNode>,
}

/// Theme-aware layout and paint for the compact shell.
#[derive(Debug, Clone)]
pub struct MobileChromeAppearance {
    pub toolbar_height: f32,
    pub navigation_height: f32,
    pub sheet_padding: UiEdges,
    pub sheet_gap: f32,
    pub menu_label: String,
    pub close_sheet_label: String,
    pub selected_indicator: String,
    pub toolbar_style: UiInlineStyle,
    pub title_style: UiInlineStyle,
    pub menu_style: UiInlineStyle,
    pub navigation_style: UiInlineStyle,
    pub navigation_item_style: UiInlineStyle,
    pub selected_item_style: UiInlineStyle,
    pub sheet_style: UiInlineStyle,
    pub sheet_title_style: UiInlineStyle,
    pub sheet_close_style: UiInlineStyle,
}

impl Default for MobileChromeAppearance {
    fn default() -> Self {
        let mut selected_item_style = chrome_style(
            12.0,
            ThemeColorToken::Text,
            Some(ThemeColorToken::SelectedSurface),
        );
        selected_item_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(6.0));
        let mut sheet_style =
            chrome_style(12.0, ThemeColorToken::Text, Some(ThemeColorToken::Surface));
        sheet_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        sheet_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        sheet_style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(1));
        Self {
            toolbar_height: 52.0,
            navigation_height: 56.0,
            sheet_padding: UiEdges::all(12.0),
            sheet_gap: 10.0,
            menu_label: "Actions".into(),
            close_sheet_label: "Done".into(),
            selected_indicator: "●".into(),
            toolbar_style: chrome_style(
                12.0,
                ThemeColorToken::Text,
                Some(ThemeColorToken::Surface),
            ),
            title_style: chrome_style(15.0, ThemeColorToken::Text, None),
            menu_style: chrome_style(
                12.0,
                ThemeColorToken::MutedText,
                Some(ThemeColorToken::Background),
            ),
            navigation_style: chrome_style(
                12.0,
                ThemeColorToken::Text,
                Some(ThemeColorToken::Surface),
            ),
            navigation_item_style: chrome_style(
                12.0,
                ThemeColorToken::MutedText,
                Some(ThemeColorToken::Surface),
            ),
            selected_item_style,
            sheet_style,
            sheet_title_style: chrome_style(14.0, ThemeColorToken::Text, None),
            sheet_close_style: chrome_style(
                12.0,
                ThemeColorToken::Accent,
                Some(ThemeColorToken::Background),
            ),
        }
    }
}

fn sheet_foreground(mut node: WidgetNode) -> WidgetNode {
    let style = node.style.get_or_insert_with(UiInlineStyle::default);
    style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(2));
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
        self.view_with_slots(
            commands,
            &DesktopChromeAppearance::default(),
            DesktopChromeSlots::default(),
        )
    }

    pub fn view_with(
        &self,
        commands: &CommandRegistry,
        appearance: &DesktopChromeAppearance,
    ) -> WidgetNode {
        self.view_with_slots(commands, appearance, DesktopChromeSlots::default())
    }

    pub fn view_with_slots(
        &self,
        commands: &CommandRegistry,
        appearance: &DesktopChromeAppearance,
        slots: DesktopChromeSlots,
    ) -> WidgetNode {
        let mut row = Vec::new();
        if cfg!(target_os = "macos") {
            row.push(
                WidgetNode::column([]).with_width(UiLength::Px(appearance.native_button_inset)),
            );
        }
        if let Some(leading) = slots.leading {
            row.push(leading);
        }
        row.push(
            WidgetNode::button(&self.title)
                .with_key("chrome:drag")
                .with_role("titlebar-drag")
                .with_accessible_name(format!("Move {} window", self.title))
                .with_width(UiLength::Fill(1.0))
                .with_style(appearance.title_style.clone()),
        );
        if let Some(trailing) = slots.trailing {
            row.push(trailing);
        }
        row.push(
            WidgetNode::menu(&appearance.menu_label, commands.menu_entries())
                .with_key("chrome:commands")
                .with_style(appearance.menu_style.clone()),
        );
        if !cfg!(target_os = "macos") && appearance.show_window_controls {
            for (key, label, name, style) in [
                (
                    "minimize",
                    &appearance.minimize_label,
                    "Minimize",
                    &appearance.control_style,
                ),
                (
                    "maximize",
                    &appearance.maximize_label,
                    "Maximize",
                    &appearance.control_style,
                ),
                (
                    "close",
                    &appearance.close_label,
                    "Close",
                    &appearance.close_style,
                ),
            ] {
                row.push(
                    WidgetNode::button(label)
                        .with_key(format!("chrome:{key}"))
                        .with_accessible_name(name)
                        .with_width(UiLength::Px(appearance.control_width))
                        .with_style(style.clone()),
                );
            }
        }
        WidgetNode::boxed([WidgetNode::row(row)
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
            .with_gap(appearance.gap)])
        .with_key("chrome:desktop")
        .with_role("box")
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(appearance.height))
        .with_padding(appearance.padding)
        .with_gap(appearance.gap)
        .with_style(appearance.surface_style.clone())
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
        self.view_with(commands, content, sheet, &MobileChromeAppearance::default())
    }

    pub fn view_with(
        &self,
        commands: &CommandRegistry,
        content: WidgetNode,
        sheet: Option<WidgetNode>,
        appearance: &MobileChromeAppearance,
    ) -> WidgetNode {
        let toolbar = WidgetNode::boxed([WidgetNode::row([
            WidgetNode::label(&self.title)
                .with_width(UiLength::Fill(1.0))
                .with_style(appearance.title_style.clone()),
            WidgetNode::menu(&appearance.menu_label, commands.menu_entries())
                .with_key("chrome:mobile:commands")
                .with_style(appearance.menu_style.clone()),
        ])
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0))])
        .with_key("chrome:mobile:toolbar")
        .with_role("box")
        .with_height(UiLength::Px(appearance.toolbar_height))
        .with_padding(UiEdges::all(8.0))
        .with_style(appearance.toolbar_style.clone());
        let mut body = vec![
            toolbar,
            content
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0)),
        ];
        if let (Some(title), Some(sheet)) = (&self.sheet_title, sheet) {
            body.push(
                WidgetNode::column([
                    sheet_foreground(WidgetNode::row([
                        WidgetNode::label(title)
                            .with_width(UiLength::Fill(1.0))
                            .with_style(appearance.sheet_title_style.clone()),
                        WidgetNode::button(&appearance.close_sheet_label)
                            .with_key("chrome:mobile:sheet-close")
                            .with_accessible_name("Close sheet")
                            .with_style(appearance.sheet_close_style.clone()),
                    ])),
                    sheet_foreground(sheet),
                ])
                .with_key("chrome:mobile:sheet")
                .with_role("box")
                .with_style(appearance.sheet_style.clone())
                .with_padding(appearance.sheet_padding)
                .with_gap(appearance.sheet_gap),
            );
        }
        body.push(
            WidgetNode::boxed([WidgetNode::row(self.navigation.iter().map(|item| {
                let label = if self.selected.as_deref() == Some(&item.id) {
                    format!("{} {}", appearance.selected_indicator, item.label)
                } else {
                    item.label.clone()
                };
                WidgetNode::button(label)
                    .with_key(format!("chrome:mobile:nav:{}", item.id))
                    .with_width(UiLength::Fill(1.0))
                    .with_accessible_name(&item.label)
                    .with_tab_selected(self.selected.as_deref() == Some(&item.id))
                    .with_style(if self.selected.as_deref() == Some(&item.id) {
                        appearance.selected_item_style.clone()
                    } else {
                        appearance.navigation_item_style.clone()
                    })
            }))
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
            .with_gap(6.0)])
            .with_key("chrome:mobile:bottom-nav")
            .with_role("box")
            .with_tab_list()
            .with_height(UiLength::Px(appearance.navigation_height))
            .with_padding(UiEdges::all(6.0))
            .with_gap(6.0)
            .with_style(appearance.navigation_style.clone()),
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
    compact_chrome_at(width, MOBILE_BREAKPOINT)
}

/// Choose compact chrome using a host-selected logical-pixel breakpoint.
pub fn compact_chrome_at(width: f32, breakpoint: f32) -> bool {
    width.is_finite() && breakpoint.is_finite() && width < breakpoint
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
        assert!(compact_chrome_at(700.0, 720.0));
        assert!(!compact_chrome_at(f32::NAN, 720.0));
        let mut chrome = MobileChrome::new("Editor", vec![MobileNavItem::new("files", "Files")]);
        assert!(!chrome.navigate("missing", "Missing"));
        assert!(chrome.navigate("files", "Files"));
        assert_eq!(chrome.selected.as_deref(), Some("files"));
        assert_eq!(chrome.sheet_title.as_deref(), Some("Files"));
    }

    #[test]
    fn appearance_and_slots_preserve_action_keys() {
        fn find<'a>(node: &'a WidgetNode, key: &str) -> Option<&'a WidgetNode> {
            if node.key.as_deref() == Some(key) {
                return Some(node);
            }
            node.children.iter().find_map(|child| find(child, key))
        }
        let commands = CommandRegistry::default();
        let appearance = DesktopChromeAppearance {
            height: 56.0,
            menu_label: "Tools".into(),
            show_window_controls: false,
            ..Default::default()
        };
        let desktop = DesktopChrome::new("Editor").view_with_slots(
            &commands,
            &appearance,
            DesktopChromeSlots {
                leading: Some(WidgetNode::label("Brand").with_key("host:brand")),
                trailing: None,
            },
        );
        assert!(find(&desktop, "host:brand").is_some());
        assert!(find(&desktop, "chrome:drag").is_some());
        assert!(find(&desktop, "chrome:commands").is_some());
        assert!(find(&desktop, "chrome:close").is_none());
        assert_eq!(desktop.layout.unwrap().size.height, UiLength::Px(56.0));
        let mut mobile = MobileChrome::new("Editor", vec![MobileNavItem::new("files", "Files")]);
        mobile.selected = Some("files".into());
        mobile.sheet_title = Some("Details".into());
        let compact = mobile.view_with(
            &commands,
            WidgetNode::column([]),
            Some(WidgetNode::label("Body")),
            &MobileChromeAppearance {
                close_sheet_label: "Done".into(),
                ..Default::default()
            },
        );
        assert!(find(&compact, "chrome:mobile:nav:files").is_some());
        assert!(find(&compact, "chrome:mobile:sheet-close").is_some());
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

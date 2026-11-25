//! Composable desktop and compact-window chrome for application shells.

use bbui_core::{UiFocusable, UiKey, UiParent, UiPointerButton, UiRoutedEvent};
use bbui_layout::{UiEdges, UiLength};
use bbui_style::{
    Rgba, ThemeColorToken, UiBoxShadow, UiInlineStyle, UiStyleProperty, UiStyleValue,
};
use bbui_widgets::{CommandRegistry, WidgetEvent, WidgetNode};
use bevy::prelude::*;

const MOBILE_BREAKPOINT: f32 = 600.0;

fn safe_inset(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

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
    pub center_width: f32,
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
            center_width: 220.0,
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

/// Optional host content in the desktop titlebar.
#[derive(Debug, Default)]
pub struct DesktopChromeSlots {
    pub leading: Option<WidgetNode>,
    /// Content centered across the full titlebar. Noninteractive descendants remain draggable.
    pub center: Option<WidgetNode>,
    pub trailing: Option<WidgetNode>,
}

/// Theme-aware layout and paint for the compact shell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MobileSheetPresentation {
    Inline,
    #[default]
    Overlay,
}

#[derive(Debug, Clone)]
pub struct MobileChromeAppearance {
    pub toolbar_height: f32,
    pub navigation_height: f32,
    /// Logical pixels reserved for platform cutouts and gesture areas.
    pub safe_area: UiEdges,
    pub sheet_padding: UiEdges,
    pub sheet_gap: f32,
    pub sheet_presentation: MobileSheetPresentation,
    pub toolbar_style: UiInlineStyle,
    pub navigation_style: UiInlineStyle,
    pub sheet_style: UiInlineStyle,
    pub scrim_style: UiInlineStyle,
}

impl Default for MobileChromeAppearance {
    fn default() -> Self {
        let mut sheet_style =
            chrome_style(12.0, ThemeColorToken::Text, Some(ThemeColorToken::Surface));
        sheet_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        sheet_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        sheet_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(12.0));
        sheet_style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(3));
        sheet_style.set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(UiBoxShadow::themed(
                ThemeColorToken::PopupShadow,
                0.0,
                -4.0,
                0.0,
                16.0,
            )),
        );
        let mut scrim_style = UiInlineStyle::default();
        scrim_style.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(Rgba::new(0.0, 0.0, 0.0, 0.36)),
        );
        scrim_style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(2));
        let mut toolbar_style =
            chrome_style(12.0, ThemeColorToken::Text, Some(ThemeColorToken::Surface));
        toolbar_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        toolbar_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        let mut navigation_style =
            chrome_style(12.0, ThemeColorToken::Text, Some(ThemeColorToken::Surface));
        navigation_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        navigation_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        Self {
            toolbar_height: 60.0,
            navigation_height: 68.0,
            safe_area: UiEdges::ZERO,
            sheet_padding: UiEdges::horizontal_vertical(16.0, 12.0),
            sheet_gap: 12.0,
            sheet_presentation: MobileSheetPresentation::Overlay,
            toolbar_style,
            navigation_style,
            sheet_style,
            scrim_style,
        }
    }
}

fn sheet_foreground(mut node: WidgetNode) -> WidgetNode {
    let style = node.style.get_or_insert_with(UiInlineStyle::default);
    style
        .0
        .entry(UiStyleProperty::ZIndex)
        .or_insert(UiStyleValue::Integer(4));
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
        let DesktopChromeSlots {
            leading,
            center,
            trailing,
        } = slots;
        let mut row = Vec::new();
        if cfg!(target_os = "macos") {
            row.push(
                WidgetNode::column([]).with_width(UiLength::Px(appearance.native_button_inset)),
            );
        }
        if let Some(leading) = leading {
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
        if let Some(trailing) = trailing {
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
                        .with_role(format!("chrome-window-{key}"))
                        .with_accessible_name(name)
                        .with_width(UiLength::Px(appearance.control_width))
                        .with_style(style.clone()),
                );
            }
        }
        let mut layers = vec![
            WidgetNode::row(row)
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))
                .with_gap(appearance.gap),
        ];
        if let Some(content) = center
            && appearance.center_width > 0.0
        {
            layers.push(centered_chrome_layer(
                WidgetNode::overlay([chrome_center_foreground(content)])
                    .with_key("chrome:center-drag")
                    .with_width(UiLength::Px(appearance.center_width))
                    .with_height(UiLength::Fill(1.0))
                    .with_style(chrome_center_style()),
            ));
        }
        WidgetNode::boxed([WidgetNode::overlay(layers)
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))])
        .with_key("chrome:desktop")
        .with_role("box")
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(appearance.height))
        .with_padding(appearance.padding)
        .with_gap(appearance.gap)
        .with_style(appearance.surface_style.clone())
    }
}

fn chrome_center_style() -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::ZIndex, UiStyleValue::Integer(2));
    style
}

fn chrome_center_foreground(mut node: WidgetNode) -> WidgetNode {
    let style = node.style.get_or_insert_with(UiInlineStyle::default);
    style
        .0
        .entry(UiStyleProperty::ZIndex)
        .or_insert(UiStyleValue::Integer(3));
    node.children = node
        .children
        .into_iter()
        .map(chrome_center_foreground)
        .collect();
    node
}

fn centered_chrome_layer(content: WidgetNode) -> WidgetNode {
    WidgetNode::row([
        WidgetNode::column([]).with_width(UiLength::Fill(1.0)),
        content,
        WidgetNode::column([]).with_width(UiLength::Fill(1.0)),
    ])
    .with_width(UiLength::Fill(1.0))
    .with_height(UiLength::Fill(1.0))
}

/// Host-owned content for each region of the compact shell.
#[derive(Debug)]
pub struct MobileChromeSlots {
    pub toolbar: WidgetNode,
    pub content: WidgetNode,
    pub sheet: Option<WidgetNode>,
    pub navigation: WidgetNode,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MobileChrome;

impl MobileChrome {
    /// Place host widgets in the toolbar, page, optional sheet, and navigation regions.
    pub fn view(&self, slots: MobileChromeSlots) -> WidgetNode {
        self.view_with(slots, &MobileChromeAppearance::default())
    }

    pub fn view_with(
        &self,
        slots: MobileChromeSlots,
        appearance: &MobileChromeAppearance,
    ) -> WidgetNode {
        let MobileChromeSlots {
            toolbar,
            content,
            sheet,
            navigation,
        } = slots;
        let safe = UiEdges {
            left: safe_inset(appearance.safe_area.left),
            top: safe_inset(appearance.safe_area.top),
            right: safe_inset(appearance.safe_area.right),
            bottom: safe_inset(appearance.safe_area.bottom),
        };
        let toolbar = WidgetNode::boxed([toolbar
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))])
        .with_key("chrome:mobile:toolbar")
        .with_role("box")
        .with_style(appearance.toolbar_style.clone())
        .with_height(UiLength::Px(appearance.toolbar_height + safe.top))
        .with_padding(UiEdges {
            left: 14.0 + safe.left,
            top: 8.0 + safe.top,
            right: 14.0 + safe.right,
            bottom: 8.0,
        });
        let mut body = vec![
            toolbar,
            WidgetNode::column([content
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))])
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
            .with_padding(UiEdges {
                left: safe.left,
                top: 0.0,
                right: safe.right,
                bottom: 0.0,
            }),
        ];
        let sheet = sheet.map(|content| {
            let pad = appearance.sheet_padding;
            WidgetNode::column([sheet_foreground(content)])
                .with_key("chrome:mobile:sheet")
                .with_role("box")
                .with_style(appearance.sheet_style.clone())
                .with_padding(UiEdges {
                    left: safe_inset(pad.left) + safe.left,
                    top: safe_inset(pad.top),
                    right: safe_inset(pad.right) + safe.right,
                    bottom: safe_inset(pad.bottom)
                        + if appearance.sheet_presentation == MobileSheetPresentation::Overlay {
                            safe.bottom
                        } else {
                            0.0
                        },
                })
                .with_gap(appearance.sheet_gap)
                .with_width(UiLength::Fill(1.0))
        });
        if appearance.sheet_presentation == MobileSheetPresentation::Inline
            && let Some(sheet) = sheet.clone()
        {
            body.push(sheet);
        }
        body.push(
            WidgetNode::boxed([navigation
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))])
            .with_key("chrome:mobile:bottom-nav")
            .with_role("box")
            .with_tab_list()
            .with_style(appearance.navigation_style.clone())
            .with_height(UiLength::Px(appearance.navigation_height + safe.bottom))
            .with_padding(UiEdges {
                left: 8.0 + safe.left,
                top: 7.0,
                right: 8.0 + safe.right,
                bottom: 7.0 + safe.bottom,
            }),
        );
        let base = WidgetNode::column(body)
            .with_key("chrome:mobile")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0));
        if appearance.sheet_presentation == MobileSheetPresentation::Overlay
            && let Some(sheet) = sheet
        {
            WidgetNode::overlay([
                base,
                WidgetNode::button("")
                    .with_key("chrome:mobile:sheet-backdrop")
                    .with_role("chrome-mobile-sheet-backdrop")
                    .with_accessible_name("Dismiss sheet")
                    .with_style(appearance.scrim_style.clone())
                    .with_width(UiLength::Fill(1.0))
                    .with_height(UiLength::Fill(1.0)),
                WidgetNode::column([
                    WidgetNode::column([]).with_height(UiLength::Fill(1.0)),
                    sheet,
                ])
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0)),
            ])
            .with_key("chrome:mobile:overlay")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
        } else {
            base
        }
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
    let mut current = Some(*target);
    let mut drag_target = false;
    while let Some(entity) = current {
        if world
            .get::<UiKey>(entity)
            .is_some_and(|key| matches!(key.0.as_str(), "chrome:drag" | "chrome:center-drag"))
        {
            drag_target = true;
            break;
        }
        if world.get::<UiFocusable>(entity).is_some() {
            return false;
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    if !drag_target {
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
        "chrome:mobile:sheet-close" | "chrome:mobile:sheet-backdrop" => {
            Some(ChromeAction::DismissSheet)
        }
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
    fn compact_breakpoint() {
        assert!(!compact_chrome(600.0));
        assert!(compact_chrome(599.0));
        assert!(compact_chrome_at(700.0, 720.0));
        assert!(!compact_chrome_at(f32::NAN, 720.0));
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
                center: Some(
                    WidgetNode::row([
                        WidgetNode::label("Exporting"),
                        WidgetNode::progress(0.6).with_key("host:progress"),
                    ])
                    .with_key("host:center"),
                ),
                trailing: None,
            },
        );
        assert!(find(&desktop, "host:brand").is_some());
        assert!(find(&desktop, "chrome:drag").is_some());
        assert!(find(&desktop, "chrome:center-drag").is_some());
        assert!(find(&desktop, "host:progress").is_some());
        assert!(find(&desktop, "chrome:commands").is_some());
        assert!(find(&desktop, "chrome:close").is_none());
        assert_eq!(desktop.layout.unwrap().size.height, UiLength::Px(56.0));
        let compact = MobileChrome.view_with(
            MobileChromeSlots {
                toolbar: WidgetNode::row([
                    WidgetNode::label("Editor"),
                    WidgetNode::menu("Actions", commands.menu_entries())
                        .with_key("chrome:mobile:commands"),
                ])
                .with_key("host:toolbar"),
                content: WidgetNode::label("Body"),
                sheet: Some(
                    WidgetNode::column([
                        WidgetNode::label("Details"),
                        WidgetNode::button("Done").with_key("chrome:mobile:sheet-close"),
                    ])
                    .with_key("host:sheet"),
                ),
                navigation: WidgetNode::row([
                    WidgetNode::button("Files").with_key("chrome:mobile:nav:files")
                ])
                .with_key("host:navigation"),
            },
            &MobileChromeAppearance {
                navigation_height: 72.0,
                safe_area: UiEdges {
                    left: 12.0,
                    top: 28.0,
                    right: 12.0,
                    bottom: 24.0,
                },
                ..Default::default()
            },
        );
        assert!(find(&compact, "host:toolbar").is_some());
        assert!(find(&compact, "host:sheet").is_some());
        assert!(find(&compact, "host:navigation").is_some());
        assert!(find(&compact, "chrome:mobile:nav:files").is_some());
        assert!(find(&compact, "chrome:mobile:sheet-close").is_some());
        assert!(find(&compact, "chrome:mobile:sheet-backdrop").is_some());
        assert_eq!(
            find(&compact, "chrome:mobile:toolbar")
                .and_then(|node| node.layout)
                .map(|layout| layout.size.height),
            Some(UiLength::Px(88.0))
        );
        assert_eq!(
            find(&compact, "chrome:mobile:bottom-nav")
                .and_then(|node| node.layout)
                .map(|layout| layout.size.height),
            Some(UiLength::Px(96.0))
        );
        assert_eq!(
            find(&compact, "chrome:mobile:sheet")
                .and_then(|node| node.layout)
                .map(|layout| layout.padding),
            Some(UiEdges {
                left: 28.0,
                top: 12.0,
                right: 28.0,
                bottom: 36.0,
            })
        );
        let inline = MobileChrome.view_with(
            MobileChromeSlots {
                toolbar: WidgetNode::label("Editor"),
                content: WidgetNode::label("Body"),
                sheet: Some(WidgetNode::label("Sheet")),
                navigation: WidgetNode::label("Navigation"),
            },
            &MobileChromeAppearance {
                sheet_presentation: MobileSheetPresentation::Inline,
                ..Default::default()
            },
        );
        assert!(find(&inline, "chrome:mobile:sheet").is_some());
        assert!(find(&inline, "chrome:mobile:sheet-backdrop").is_none());
    }

    #[test]
    fn controls_route_actions_and_drag_only_from_title_press() {
        let mut world = World::new();
        let window = world.spawn(Window::default()).id();
        let drag = world.spawn(UiKey("chrome:drag".into())).id();
        let center_drag = world.spawn(UiKey("chrome:center-drag".into())).id();
        let progress = world.spawn(UiParent(center_drag)).id();
        let center_button = world.spawn((UiParent(center_drag), UiFocusable)).id();
        let close = world.spawn(UiKey("chrome:close".into())).id();
        let commands = world.spawn(UiKey("chrome:commands".into())).id();
        let mobile_commands = world.spawn(UiKey("chrome:mobile:commands".into())).id();
        let mobile_nav = world.spawn(UiKey("chrome:mobile:nav:files".into())).id();
        let sheet_close = world.spawn(UiKey("chrome:mobile:sheet-close".into())).id();
        let sheet_backdrop = world
            .spawn(UiKey("chrome:mobile:sheet-backdrop".into()))
            .id();
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
        assert_eq!(
            chrome_action(
                &world,
                &WidgetEvent::MenuCommand {
                    target: mobile_commands,
                    command_id: "save".into(),
                }
            ),
            Some(ChromeAction::Command("save".into()))
        );
        assert_eq!(
            chrome_action(&world, &WidgetEvent::Activated { target: mobile_nav }),
            Some(ChromeAction::Navigate("files".into()))
        );
        assert_eq!(
            chrome_action(
                &world,
                &WidgetEvent::Activated {
                    target: sheet_close
                }
            ),
            Some(ChromeAction::DismissSheet)
        );
        assert_eq!(
            chrome_action(
                &world,
                &WidgetEvent::Activated {
                    target: sheet_backdrop
                }
            ),
            Some(ChromeAction::DismissSheet)
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
        assert!(handle_chrome_press(
            &mut world,
            &UiRoutedEvent::PointerDown {
                window,
                target: progress,
                button: UiPointerButton::Primary,
                position: [400.0, 10.0],
            }
        ));
        assert!(!handle_chrome_press(
            &mut world,
            &UiRoutedEvent::PointerDown {
                window,
                target: center_button,
                button: UiPointerButton::Primary,
                position: [400.0, 10.0],
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

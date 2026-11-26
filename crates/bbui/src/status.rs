//! A compact, application-composed status bar.

use bbui_layout::{UiAlign, UiEdges, UiJustify, UiLayoutNode, UiLength};
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bbui_widgets::WidgetNode;

/// Theme-aware surface and layout for a status bar.
#[derive(Debug, Clone)]
pub struct StatusBarAppearance {
    pub height: f32,
    pub padding: UiEdges,
    pub gap: f32,
    pub surface_style: UiInlineStyle,
}

impl Default for StatusBarAppearance {
    fn default() -> Self {
        let mut surface_style = UiInlineStyle::default();
        surface_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Surface);
        surface_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        surface_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        Self {
            height: 36.0,
            padding: UiEdges::horizontal_vertical(10.0, 5.0),
            gap: 8.0,
            surface_style,
        }
    }
}

/// Application-owned widgets in three equal-width regions. Each region may contain
/// any BBUI widget, including text, a progress bar, or a control.
#[derive(Debug, Default)]
pub struct StatusBarSlots {
    pub leading: Option<WidgetNode>,
    pub center: Option<WidgetNode>,
    pub trailing: Option<WidgetNode>,
}

/// A status bar surface with no built-in job state or actions.
#[derive(Debug, Clone, Copy, Default)]
pub struct StatusBar;

impl StatusBar {
    pub fn view(&self) -> WidgetNode {
        self.view_with_slots(&StatusBarAppearance::default(), StatusBarSlots::default())
    }

    pub fn view_with(&self, appearance: &StatusBarAppearance) -> WidgetNode {
        self.view_with_slots(appearance, StatusBarSlots::default())
    }

    pub fn view_with_slots(
        &self,
        appearance: &StatusBarAppearance,
        slots: StatusBarSlots,
    ) -> WidgetNode {
        let region = |key: &str, node: Option<WidgetNode>, justify_content| {
            WidgetNode::row(node)
                .with_key(key)
                .with_layout(UiLayoutNode {
                    align_items: UiAlign::Center,
                    justify_content,
                    ..UiLayoutNode::row(0.0)
                })
                .with_width(UiLength::Fill(1.0))
                .with_height(UiLength::Fill(1.0))
        };
        WidgetNode::boxed([WidgetNode::row([
            region("status:leading", slots.leading, UiJustify::Start),
            region("status:center", slots.center, UiJustify::Center),
            region("status:trailing", slots.trailing, UiJustify::End),
        ])
        .with_key("status:content")
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0))
        .with_gap(appearance.gap)])
        .with_key("status:bar")
        .with_role("box")
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Px(appearance.height))
        .with_padding(appearance.padding)
        .with_style(appearance.surface_style.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(node: &'a WidgetNode, key: &str) -> Option<&'a WidgetNode> {
        if node.key.as_deref() == Some(key) {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, key))
    }

    #[test]
    fn all_regions_accept_arbitrary_widgets() {
        let bar = StatusBar.view_with_slots(
            &StatusBarAppearance::default(),
            StatusBarSlots {
                leading: Some(WidgetNode::label("Ready").with_key("host:label")),
                center: Some(WidgetNode::progress(0.5).with_key("host:progress")),
                trailing: Some(WidgetNode::button("Cancel").with_key("host:action")),
            },
        );
        for key in ["host:label", "host:progress", "host:action"] {
            assert!(find(&bar, key).is_some());
        }
        assert!(find(&bar, "status:center").is_some());
    }
}

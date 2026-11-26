//! Application-owned job status presentation.

use bbui_core::{UiKey, UiLiveRegion};
use bbui_layout::{UiEdges, UiLength};
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bbui_widgets::{WidgetEvent, WidgetNode};
use bevy::prelude::*;

fn status_text(size: f32, color: ThemeColorToken) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(size));
    style.set_color_token(UiStyleProperty::Foreground, color);
    style
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusTone {
    Normal,
    Muted,
    Accent,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusItem {
    pub text: String,
    pub tone: StatusTone,
}

impl StatusItem {
    pub fn new(text: impl Into<String>, tone: StatusTone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatusProgress {
    pub label: String,
    pub fraction: f32,
    pub cancel_label: Option<String>,
}

/// Copy-independent, theme-aware status bar layout and paint.
#[derive(Debug, Clone)]
pub struct StatusBarAppearance {
    pub height: f32,
    pub padding: UiEdges,
    pub gap: f32,
    pub progress_width: f32,
    pub show_secondary_items: bool,
    pub show_progress_label: bool,
    pub surface_style: UiInlineStyle,
    pub normal_style: UiInlineStyle,
    pub muted_style: UiInlineStyle,
    pub accent_style: UiInlineStyle,
    pub success_style: UiInlineStyle,
    pub error_style: UiInlineStyle,
    pub progress_label_style: UiInlineStyle,
    pub cancel_style: UiInlineStyle,
}

impl Default for StatusBarAppearance {
    fn default() -> Self {
        let mut surface_style = UiInlineStyle::default();
        surface_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Surface);
        surface_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        surface_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        let mut cancel_style = status_text(11.5, ThemeColorToken::Text);
        cancel_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Background);
        cancel_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(4.0));
        Self {
            height: 36.0,
            padding: UiEdges::horizontal_vertical(10.0, 5.0),
            gap: 14.0,
            progress_width: 150.0,
            show_secondary_items: true,
            show_progress_label: true,
            surface_style,
            normal_style: status_text(11.5, ThemeColorToken::Text),
            muted_style: status_text(11.5, ThemeColorToken::MutedText),
            accent_style: status_text(11.5, ThemeColorToken::Accent),
            success_style: status_text(11.5, ThemeColorToken::Success),
            error_style: status_text(11.5, ThemeColorToken::Error),
            progress_label_style: status_text(11.5, ThemeColorToken::Text),
            cancel_style,
        }
    }
}

#[derive(Debug, Default)]
pub struct StatusBarSlots {
    pub leading: Option<WidgetNode>,
    pub trailing: Option<WidgetNode>,
}

impl StatusProgress {
    pub fn new(label: impl Into<String>, fraction: f32) -> Self {
        Self {
            label: label.into(),
            fraction,
            cancel_label: None,
        }
    }
    pub fn normalized_fraction(&self) -> f32 {
        if self.fraction.is_finite() {
            self.fraction.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusBar {
    pub items: Vec<StatusItem>,
    pub progress: Option<StatusProgress>,
}

impl StatusBar {
    pub fn view(&self) -> WidgetNode {
        self.view_with(&StatusBarAppearance::default())
    }

    pub fn view_with(&self, appearance: &StatusBarAppearance) -> WidgetNode {
        self.view_with_slots(appearance, StatusBarSlots::default())
    }

    pub fn view_with_slots(
        &self,
        appearance: &StatusBarAppearance,
        slots: StatusBarSlots,
    ) -> WidgetNode {
        let mut items: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(index, _)| appearance.show_secondary_items || *index == 0)
            .map(|(index, item)| {
                WidgetNode::label(&item.text)
                    .with_key(format!("status:item:{index}"))
                    .with_role(match item.tone {
                        StatusTone::Normal => "status-normal",
                        StatusTone::Muted => "status-muted",
                        StatusTone::Accent => "status-accent",
                        StatusTone::Success => "status-success",
                        StatusTone::Error => "status-error",
                    })
                    .with_live_region(if item.tone == StatusTone::Error {
                        UiLiveRegion::assertive()
                    } else {
                        UiLiveRegion::polite()
                    })
                    .with_style(match item.tone {
                        StatusTone::Normal => appearance.normal_style.clone(),
                        StatusTone::Muted => appearance.muted_style.clone(),
                        StatusTone::Accent => appearance.accent_style.clone(),
                        StatusTone::Success => appearance.success_style.clone(),
                        StatusTone::Error => appearance.error_style.clone(),
                    })
            })
            .collect();
        if let Some(leading) = slots.leading {
            items.insert(0, leading);
        }
        let mut children = vec![
            WidgetNode::row(items)
                .with_key("status:items")
                .with_width(UiLength::Fill(1.0))
                .with_gap(appearance.gap),
        ];
        if let Some(progress) = &self.progress {
            if appearance.show_progress_label {
                children.push(
                    WidgetNode::label(&progress.label)
                        .with_key("status:progress-label")
                        .with_live_region(UiLiveRegion::polite())
                        .with_style(appearance.progress_label_style.clone()),
                );
            }
            children.push(
                WidgetNode::progress(progress.normalized_fraction())
                    .with_key("status:progress")
                    .with_width(UiLength::Px(appearance.progress_width)),
            );
            if let Some(label) = &progress.cancel_label
                && !label.is_empty()
            {
                children.push(
                    WidgetNode::button(label)
                        .with_key("status:cancel")
                        .with_accessible_name(format!("Cancel {}", progress.label))
                        .with_style(appearance.cancel_style.clone()),
                );
            }
        }
        if let Some(trailing) = slots.trailing {
            children.push(trailing);
        }
        WidgetNode::boxed([WidgetNode::row(children)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusAction {
    CancelRequested,
}

pub fn status_action(world: &World, event: &WidgetEvent) -> Option<StatusAction> {
    let WidgetEvent::Activated { target } = event else {
        return None;
    };
    (world.get::<UiKey>(*target)?.0 == "status:cancel").then_some(StatusAction::CancelRequested)
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
    fn compact_bar_keeps_progress_cancel_and_host_slots() {
        let bar = StatusBar {
            items: vec![
                StatusItem::new("Ready", StatusTone::Normal),
                StatusItem::new("Details", StatusTone::Muted),
            ],
            progress: Some(StatusProgress {
                label: "Export".into(),
                fraction: 0.5,
                cancel_label: Some("Cancel".into()),
            }),
        };
        let appearance = StatusBarAppearance {
            show_secondary_items: false,
            show_progress_label: false,
            progress_width: 90.0,
            ..Default::default()
        };
        let view = bar.view_with_slots(
            &appearance,
            StatusBarSlots {
                leading: Some(WidgetNode::label("Sync").with_key("host:sync")),
                trailing: None,
            },
        );
        assert!(find(&view, "host:sync").is_some());
        assert!(find(&view, "status:item:0").is_some());
        assert!(find(&view, "status:item:1").is_none());
        assert!(find(&view, "status:progress-label").is_none());
        assert!(find(&view, "status:progress").is_some());
        assert!(find(&view, "status:cancel").is_some());
    }

    #[test]
    fn progress_sanitizes_values_and_cancel_routes() {
        assert_eq!(
            StatusProgress::new("Job", f32::NAN).normalized_fraction(),
            0.0
        );
        assert_eq!(StatusProgress::new("Job", 1.5).normalized_fraction(), 1.0);
        let mut world = World::new();
        let button = world.spawn(UiKey("status:cancel".into())).id();
        assert_eq!(
            status_action(&world, &WidgetEvent::Activated { target: button }),
            Some(StatusAction::CancelRequested)
        );
    }
}

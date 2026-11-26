//! Per-window floating toasts with timed or persistent lifetimes.

use std::{collections::VecDeque, time::Duration};

use bbui_core::{UiKey, UiLiveRegion};
use bbui_layout::{UiEdges, UiLength};
use bbui_motion::MotionCurve;
use bbui_style::{
    Theme, ThemeColorToken, UiAnimationSpec, UiAutoAnimations, UiBoxShadow, UiInlineStyle,
    UiReducedMotion, UiStyleProperty, UiStyleValue,
};
use bbui_widgets::{UiVisualEffects, WidgetEvent, WidgetNode};
use bevy::prelude::*;
use bevy::window::WindowClosed;

const DEFAULT_VISIBLE: usize = 3;
const MAX_STORED_PER_WINDOW: usize = 32;

/// Corner used by a floating toast stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

/// Logical-pixel placement of toasts over application content.
#[derive(Debug, Clone)]
pub struct ToastOverlayAppearance {
    pub corner: ToastCorner,
    pub width: f32,
    pub margin: UiEdges,
}

/// Timing, travel, and easing for floating toast entry and exit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToastMotion {
    pub enter: Duration,
    pub exit: Duration,
    pub travel: f32,
    /// Easing for the card and content fading into view.
    pub enter_opacity_curve: MotionCurve,
    /// Easing for the card moving into place.
    pub enter_travel_curve: MotionCurve,
    /// Easing for the card and content fading out.
    pub exit_opacity_curve: MotionCurve,
    /// Easing for the card moving away.
    pub exit_travel_curve: MotionCurve,
}

impl Default for ToastMotion {
    fn default() -> Self {
        Self {
            enter: Duration::from_millis(280),
            exit: Duration::from_millis(220),
            travel: 18.0,
            enter_opacity_curve: MotionCurve::EaseOut,
            enter_travel_curve: MotionCurve::EaseOut,
            exit_opacity_curve: MotionCurve::EaseIn,
            exit_travel_curve: MotionCurve::EaseIn,
        }
    }
}

impl Default for ToastOverlayAppearance {
    fn default() -> Self {
        Self {
            corner: ToastCorner::BottomRight,
            width: 280.0,
            margin: UiEdges::all(18.0),
        }
    }
}

fn toast_text(size: f32, color: ThemeColorToken) -> UiInlineStyle {
    let mut style = UiInlineStyle::default();
    style.set(UiStyleProperty::FontSize, UiStyleValue::Scalar(size));
    style.set_color_token(UiStyleProperty::Foreground, color);
    style
}

fn floating_foreground(mut node: WidgetNode) -> WidgetNode {
    node.style
        .get_or_insert_with(UiInlineStyle::default)
        .0
        .entry(UiStyleProperty::ZIndex)
        .or_insert(UiStyleValue::Integer(100));
    node.children = node.children.into_iter().map(floating_foreground).collect();
    node
}

fn transition_toast(node: &mut WidgetNode, opacity: f32, spec: UiAnimationSpec) {
    let style = node.style.get_or_insert_with(UiInlineStyle::default);
    let base = match style.0.get(&UiStyleProperty::Opacity) {
        Some(UiStyleValue::Scalar(value)) if value.is_finite() => *value,
        _ => 1.0,
    };
    style.set(
        UiStyleProperty::Opacity,
        UiStyleValue::Scalar(base * opacity),
    );
    node.auto_animations
        .get_or_insert_with(UiAutoAnimations::default)
        .set(UiStyleProperty::Opacity, spec);
    for child in &mut node.children {
        transition_toast_content(child, opacity, spec);
    }
}

fn transition_toast_content(node: &mut WidgetNode, opacity: f32, spec: UiAnimationSpec) {
    if node.children.is_empty() {
        transition_toast(node, opacity, spec);
    } else {
        for child in &mut node.children {
            transition_toast_content(child, opacity, spec);
        }
    }
}

fn wrap_toast_message(message: &str, columns: usize, max_lines: usize) -> Vec<String> {
    let columns = columns.max(1);
    let max_lines = max_lines.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in message.split_whitespace() {
        let word_len = word.chars().count();
        if !line.is_empty() && line.chars().count() + 1 + word_len > columns {
            lines.push(std::mem::take(&mut line));
        }
        if word_len > columns {
            for character in word.chars() {
                if line.chars().count() >= columns {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(character);
            }
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            while last.chars().count() >= columns {
                last.pop();
            }
            last.push('…');
        }
    }
    lines
}

/// Theme-aware layout, copy, and paint for the toast stack.
#[derive(Debug, Clone)]
pub struct ToastAppearance {
    pub gap: f32,
    pub card_gap: f32,
    pub card_padding: UiEdges,
    pub dismiss_label: String,
    /// Logical-pixel width of the dismiss control; increase this for text labels.
    pub dismiss_width: f32,
    /// Logical-pixel height of the dismiss control.
    pub dismiss_height: f32,
    pub empty_message: String,
    pub show_empty_message: bool,
    pub severity_borders: bool,
    pub message_wrap_columns: usize,
    pub message_max_lines: usize,
    pub center_style: UiInlineStyle,
    pub card_style: UiInlineStyle,
    pub title_style: UiInlineStyle,
    pub message_style: UiInlineStyle,
    pub dismiss_style: UiInlineStyle,
    pub empty_style: UiInlineStyle,
    pub info_style: UiInlineStyle,
    pub success_style: UiInlineStyle,
    pub warning_style: UiInlineStyle,
    pub error_style: UiInlineStyle,
    pub info_effects: Option<UiVisualEffects>,
    pub success_effects: Option<UiVisualEffects>,
    pub warning_effects: Option<UiVisualEffects>,
    pub error_effects: Option<UiVisualEffects>,
}

impl Default for ToastAppearance {
    fn default() -> Self {
        let mut card_style = UiInlineStyle::default();
        card_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Background);
        card_style.set_color_token(UiStyleProperty::BorderColor, ThemeColorToken::Border);
        card_style.set(UiStyleProperty::BorderWidth, UiStyleValue::Scalar(1.0));
        card_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(7.0));
        card_style.set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(UiBoxShadow::themed(
                ThemeColorToken::PopupShadow,
                0.0,
                5.0,
                0.0,
                18.0,
            )),
        );
        let mut dismiss_style = toast_text(17.0, ThemeColorToken::MutedText);
        dismiss_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::Surface);
        dismiss_style.set(UiStyleProperty::CornerRadius, UiStyleValue::Scalar(4.0));
        Self {
            gap: 8.0,
            card_gap: 4.0,
            card_padding: UiEdges::all(10.0),
            dismiss_label: "×".into(),
            dismiss_width: 24.0,
            dismiss_height: 22.0,
            empty_message: "All caught up".into(),
            show_empty_message: true,
            severity_borders: false,
            message_wrap_columns: 32,
            message_max_lines: 3,
            center_style: UiInlineStyle::default(),
            card_style,
            title_style: toast_text(12.5, ThemeColorToken::Text),
            message_style: toast_text(11.5, ThemeColorToken::MutedText),
            dismiss_style,
            empty_style: toast_text(12.0, ThemeColorToken::MutedText),
            info_style: toast_text(10.5, ThemeColorToken::Accent),
            success_style: toast_text(10.5, ThemeColorToken::Success),
            warning_style: toast_text(10.5, ThemeColorToken::Warning),
            error_style: toast_text(10.5, ThemeColorToken::Error),
            info_effects: None,
            success_effects: None,
            warning_effects: None,
            error_effects: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastSeverity {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
    fn live_region(self) -> UiLiveRegion {
        match self {
            Self::Warning | Self::Error => UiLiveRegion::assertive().atomic(),
            Self::Info | Self::Success => UiLiveRegion::polite().atomic(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLifetime {
    For(Duration),
    Persistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastEntryPhase {
    Unmounted,
    Mounted,
    Active,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub window: Entity,
    pub title: Option<String>,
    pub message: String,
    pub severity: ToastSeverity,
    expires_at: Option<f64>,
    entry_phase: ToastEntryPhase,
    leaving_at: Option<f64>,
}

#[derive(Resource, Debug)]
pub struct ToastCenter {
    entries: VecDeque<Toast>,
    next_id: u64,
    clock_seconds: f64,
    visible_limit: usize,
    generation: u64,
    motion: ToastMotion,
    reduced_motion: bool,
    motion_scale: f32,
}

/// Optional host content and toast rendering while retaining queue behavior.
#[derive(Default)]
pub struct ToastSlots<'a> {
    pub header: Option<WidgetNode>,
    pub empty: Option<WidgetNode>,
    pub footer: Option<WidgetNode>,
    pub toast: Option<&'a dyn Fn(&Toast) -> WidgetNode>,
}

impl Default for ToastCenter {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            next_id: 1,
            clock_seconds: 0.0,
            visible_limit: DEFAULT_VISIBLE,
            generation: 0,
            motion: ToastMotion::default(),
            reduced_motion: false,
            motion_scale: 1.0,
        }
    }
}

impl ToastCenter {
    pub fn motion(&self) -> ToastMotion {
        self.motion
    }

    pub fn set_motion(&mut self, motion: ToastMotion) {
        if self.motion != motion {
            self.motion = motion;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn set_reduced_motion(&mut self, reduced: bool) {
        if self.reduced_motion != reduced {
            self.reduced_motion = reduced;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn set_motion_scale(&mut self, scale: f32) {
        let scale = if scale.is_finite() {
            scale.max(0.0)
        } else {
            0.0
        };
        if self.motion_scale != scale {
            self.motion_scale = scale;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn set_visible_limit(&mut self, limit: usize) {
        let limit = limit.max(1);
        if self.visible_limit != limit {
            self.visible_limit = limit;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn post(
        &mut self,
        window: Entity,
        severity: ToastSeverity,
        title: Option<String>,
        message: impl Into<String>,
        lifetime: ToastLifetime,
    ) -> u64 {
        if self
            .entries
            .iter()
            .filter(|entry| entry.window == window)
            .count()
            >= MAX_STORED_PER_WINDOW
            && let Some(index) = self.entries.iter().position(|entry| entry.window == window)
        {
            self.entries.remove(index);
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let expires_at = match lifetime {
            ToastLifetime::For(duration) => Some(self.clock_seconds + duration.as_secs_f64()),
            ToastLifetime::Persistent => None,
        };
        self.entries.push_back(Toast {
            id,
            window,
            title,
            message: message.into(),
            severity,
            expires_at,
            entry_phase: if self.reduced_motion || self.motion.enter.is_zero() {
                ToastEntryPhase::Active
            } else {
                ToastEntryPhase::Unmounted
            },
            leaving_at: None,
        });
        self.generation = self.generation.wrapping_add(1);
        id
    }

    pub fn advance(&mut self, elapsed: Duration) {
        self.clock_seconds += elapsed.as_secs_f64();
        let mut changed = false;
        for entry in &mut self.entries {
            if entry.entry_phase == ToastEntryPhase::Mounted {
                entry.entry_phase = ToastEntryPhase::Active;
                changed = true;
            }
            if entry.leaving_at.is_none()
                && let Some(deadline) = entry.expires_at
                && self.clock_seconds >= deadline
            {
                entry.leaving_at = Some(deadline);
                changed = true;
            }
        }
        let before = self.entries.len();
        self.entries.retain(|entry| {
            entry.leaving_at.is_none_or(|at| {
                !self.reduced_motion
                    && self.clock_seconds + 1e-9
                        < at + self.motion.exit.as_secs_f64() * f64::from(self.motion_scale)
            })
        });
        if changed || self.entries.len() != before {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn acknowledge(&mut self, id: u64) -> Option<Toast> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == id && entry.leaving_at.is_none())?;
        let toast = self.entries.get(index)?.clone();
        if let Some(entry) = self.entries.get_mut(index) {
            entry.leaving_at = Some(self.clock_seconds);
        }
        if self.reduced_motion || self.motion.exit.is_zero() || self.motion_scale == 0.0 {
            self.entries.remove(index);
        }
        self.generation = self.generation.wrapping_add(1);
        Some(toast)
    }

    pub fn pending(&self, window: Entity) -> impl DoubleEndedIterator<Item = &Toast> {
        self.entries
            .iter()
            .filter(move |entry| entry.window == window && entry.leaving_at.is_none())
    }

    /// Remove every toast for a window, including persistent and exiting entries.
    /// The plugin calls this when Bevy reports that a window closed.
    pub fn clear_window(&mut self, window: Entity) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.window != window);
        let removed = before - self.entries.len();
        if removed > 0 {
            self.generation = self.generation.wrapping_add(1);
        }
        removed
    }

    pub fn visible(&self, window: Entity) -> Vec<&Toast> {
        self.entries
            .iter()
            .filter(|entry| entry.window == window)
            .rev()
            .take(self.visible_limit)
            .collect()
    }

    pub fn view(&self, window: Entity) -> WidgetNode {
        self.view_with(window, &ToastAppearance::default())
    }

    pub fn view_with(&self, window: Entity, appearance: &ToastAppearance) -> WidgetNode {
        self.view_with_slots(window, appearance, ToastSlots::default())
    }

    /// Layer dismissible toasts over a full-size content node without reserving workspace space.
    /// The host should mount the result in the same bounds as `content`.
    pub fn floating_view(
        &mut self,
        window: Entity,
        content: WidgetNode,
        appearance: &ToastAppearance,
        overlay: &ToastOverlayAppearance,
    ) -> WidgetNode {
        let inset = |value: f32| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        let margin = UiEdges {
            left: inset(overlay.margin.left),
            top: inset(overlay.margin.top),
            right: inset(overlay.margin.right),
            bottom: inset(overlay.margin.bottom),
        };
        let stack = self.floating_stack(window, appearance, overlay);
        let left = matches!(
            overlay.corner,
            ToastCorner::TopLeft | ToastCorner::BottomLeft
        );
        let bottom = matches!(
            overlay.corner,
            ToastCorner::BottomLeft | ToastCorner::BottomRight
        );
        let row = if left {
            WidgetNode::row([
                stack,
                WidgetNode::column([]).with_width(UiLength::Fill(1.0)),
            ])
        } else {
            WidgetNode::row([
                WidgetNode::column([]).with_width(UiLength::Fill(1.0)),
                stack,
            ])
        }
        .with_width(UiLength::Fill(1.0));
        let layer = if bottom {
            WidgetNode::column([WidgetNode::column([]).with_height(UiLength::Fill(1.0)), row])
        } else {
            WidgetNode::column([row])
        }
        .with_key("toast:floating-layer")
        .with_width(UiLength::Fill(1.0))
        .with_height(UiLength::Fill(1.0))
        .with_padding(margin);
        WidgetNode::overlay([content, layer])
            .with_key("toast:overlay")
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Fill(1.0))
    }

    /// Reconcile this node by itself when toast data changes and content has external children.
    pub fn floating_stack(
        &mut self,
        window: Entity,
        appearance: &ToastAppearance,
        overlay: &ToastOverlayAppearance,
    ) -> WidgetNode {
        if self.visible(window).is_empty() {
            return WidgetNode::column([])
                .with_key("toast:center")
                .with_width(UiLength::Px(0.0))
                .with_height(UiLength::Px(0.0));
        }
        let width = if overlay.width.is_finite() {
            overlay.width.max(0.0)
        } else {
            280.0
        };
        let mut appearance = appearance.clone();
        appearance.show_empty_message = false;
        let mut stack = self
            .view_with(window, &appearance)
            .with_width(UiLength::Px(width));
        stack.role = Some("toast-stack".into());
        let travel = if self.motion.travel.is_finite() {
            self.motion.travel.max(0.0)
        } else {
            0.0
        };
        let bottom = matches!(
            overlay.corner,
            ToastCorner::BottomLeft | ToastCorner::BottomRight
        );
        for (card, toast) in stack.children.iter_mut().zip(
            self.entries
                .iter_mut()
                .filter(|entry| entry.window == window)
                .rev()
                .take(self.visible_limit),
        ) {
            let exiting = toast.leaving_at.is_some();
            let hidden = toast.entry_phase != ToastEntryPhase::Active || exiting;
            let (opacity_spec, travel_spec) = if exiting {
                (
                    UiAnimationSpec::new(self.motion.exit).curve(self.motion.exit_opacity_curve),
                    UiAnimationSpec::new(self.motion.exit).curve(self.motion.exit_travel_curve),
                )
            } else {
                (
                    UiAnimationSpec::new(self.motion.enter).curve(self.motion.enter_opacity_curve),
                    UiAnimationSpec::new(self.motion.enter).curve(self.motion.enter_travel_curve),
                )
            };
            transition_toast(card, if hidden { 0.0 } else { 1.0 }, opacity_spec);
            let translation = if hidden {
                if bottom { travel } else { -travel }
            } else {
                0.0
            };
            card.style.get_or_insert_with(UiInlineStyle::default).set(
                UiStyleProperty::TranslateY,
                UiStyleValue::Scalar(translation),
            );
            card.auto_animations
                .get_or_insert_with(UiAutoAnimations::default)
                .set(UiStyleProperty::TranslateY, travel_spec);
            if toast.entry_phase == ToastEntryPhase::Unmounted {
                toast.entry_phase = ToastEntryPhase::Mounted;
            }
        }
        floating_foreground(stack)
    }

    pub fn view_with_slots(
        &self,
        window: Entity,
        appearance: &ToastAppearance,
        slots: ToastSlots<'_>,
    ) -> WidgetNode {
        let mut children = Vec::new();
        if let Some(header) = slots.header {
            children.push(header);
        }
        let visible = self.visible(window);
        if visible.is_empty() {
            if let Some(empty) = slots.empty {
                children.push(empty);
            } else if appearance.show_empty_message {
                children.push(
                    WidgetNode::label(&appearance.empty_message)
                        .with_key("toast:empty")
                        .with_style(appearance.empty_style.clone()),
                );
            }
        } else {
            children.extend(visible.into_iter().map(|toast| {
                if let Some(render) = slots.toast {
                    render(toast)
                } else {
                    Self::default_toast_view(toast, appearance)
                }
            }));
        }
        if let Some(footer) = slots.footer {
            children.push(footer);
        }
        WidgetNode::boxed(children)
            .with_key("toast:center")
            .with_role("box")
            .with_width(UiLength::Fill(1.0))
            .with_gap(appearance.gap)
            .with_style(appearance.center_style.clone())
    }

    fn default_toast_view(toast: &Toast, appearance: &ToastAppearance) -> WidgetNode {
        let (severity_style, severity_token) = match toast.severity {
            ToastSeverity::Info => (&appearance.info_style, ThemeColorToken::Accent),
            ToastSeverity::Success => (&appearance.success_style, ThemeColorToken::Success),
            ToastSeverity::Warning => (&appearance.warning_style, ThemeColorToken::Warning),
            ToastSeverity::Error => (&appearance.error_style, ThemeColorToken::Error),
        };
        let mut card_style = appearance.card_style.clone();
        if appearance.severity_borders {
            card_style.set_color_token(UiStyleProperty::BorderColor, severity_token);
        }
        let title = toast
            .title
            .as_deref()
            .unwrap_or_else(|| toast.severity.label());
        let message_lines = wrap_toast_message(
            &toast.message,
            appearance.message_wrap_columns,
            appearance.message_max_lines,
        );
        let message_height = message_lines.len() as f32 * 17.0 + 6.0;
        let dismiss_width = if appearance.dismiss_width.is_finite() {
            appearance.dismiss_width.max(0.0)
        } else {
            24.0
        };
        let dismiss_height = if appearance.dismiss_height.is_finite() {
            appearance.dismiss_height.max(0.0)
        } else {
            22.0
        };
        let card = WidgetNode::boxed([
            WidgetNode::row([
                WidgetNode::label(toast.severity.label().to_uppercase())
                    .with_width(UiLength::Fill(1.0))
                    .with_style(severity_style.clone()),
                WidgetNode::button(&appearance.dismiss_label)
                    .with_key(format!("toast:dismiss:{}", toast.id))
                    .with_width(UiLength::Px(dismiss_width))
                    .with_height(UiLength::Px(dismiss_height))
                    .with_accessible_name(format!("Dismiss {} toast", title))
                    .with_style(appearance.dismiss_style.clone()),
            ])
            .with_height(UiLength::Px(dismiss_height.max(23.0))),
            WidgetNode::label(title).with_style(appearance.title_style.clone()),
            WidgetNode::column(message_lines.into_iter().enumerate().map(|(index, line)| {
                let mut label = WidgetNode::label(line)
                    .with_height(UiLength::Px(17.0))
                    .with_style(appearance.message_style.clone());
                if index == 0 {
                    label = label
                        .with_key(format!("toast:message:{}", toast.id))
                        .with_accessible_name(&toast.message)
                        .with_live_region(toast.severity.live_region());
                }
                label
            }))
            .with_width(UiLength::Fill(1.0))
            .with_height(UiLength::Px(message_height)),
        ])
        .with_key(format!("toast:{}", toast.id))
        .with_role("box")
        .with_width(UiLength::Fill(1.0))
        .with_padding(appearance.card_padding)
        .with_gap(appearance.card_gap)
        .with_style(card_style);
        let effects = match toast.severity {
            ToastSeverity::Info => &appearance.info_effects,
            ToastSeverity::Success => &appearance.success_effects,
            ToastSeverity::Warning => &appearance.warning_effects,
            ToastSeverity::Error => &appearance.error_effects,
        };
        if let Some(effects) = effects {
            card.with_visual_effects(effects.clone())
        } else {
            card
        }
    }

    /// Acknowledge a dismiss button activation without consuming other widget events.
    pub fn handle_widget_event(&mut self, world: &World, event: &WidgetEvent) -> Option<Toast> {
        let WidgetEvent::Activated { target } = event else {
            return None;
        };
        let id = world
            .get::<UiKey>(*target)?
            .0
            .strip_prefix("toast:dismiss:")?
            .parse()
            .ok()?;
        self.acknowledge(id)
    }
}

pub struct ToastsPlugin;

impl Plugin for ToastsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ToastCenter>()
            .add_systems(Update, advance_toasts);
    }
}

fn advance_toasts(
    time: Option<Res<Time>>,
    reduced: Option<Res<UiReducedMotion>>,
    theme: Option<Res<Theme>>,
    mut closed: MessageReader<WindowClosed>,
    mut center: ResMut<ToastCenter>,
) {
    for event in closed.read() {
        center.clear_window(event.window);
    }
    let scale = theme.map_or(1.0, |theme| theme.motion_scale);
    center.set_motion_scale(scale);
    center.set_reduced_motion(reduced.is_some_and(|value| value.0) || scale <= 0.0);
    if let Some(time) = time {
        center.advance(time.delta());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translation_y(node: &WidgetNode) -> Option<f32> {
        match node.style.as_ref()?.0.get(&UiStyleProperty::TranslateY)? {
            UiStyleValue::Scalar(value) => Some(*value),
            _ => None,
        }
    }

    fn find<'a>(node: &'a WidgetNode, key: &str) -> Option<&'a WidgetNode> {
        if node.key.as_deref() == Some(key) {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, key))
    }

    #[test]
    fn toast_appearance_wraps_long_text_and_exposes_slots() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        let id = center.post(
            window,
            ToastSeverity::Error,
            Some("Export failed".into()),
            "The output directory is unavailable. Choose another location and try exporting again.",
            ToastLifetime::Persistent,
        );
        let appearance = ToastAppearance {
            message_wrap_columns: 28,
            message_max_lines: 3,
            dismiss_label: "Close".into(),
            dismiss_width: 58.0,
            dismiss_height: 26.0,
            ..Default::default()
        };
        let view = center.view_with(window, &appearance);
        let card = find(&view, &format!("toast:{id}")).unwrap();
        assert_eq!(card.role.as_deref(), Some("box"));
        let dismiss = find(card, &format!("toast:dismiss:{id}")).unwrap();
        assert_eq!(dismiss.layout.unwrap().size.width, UiLength::Px(58.0));
        assert_eq!(dismiss.layout.unwrap().size.height, UiLength::Px(26.0));
        let message = find(card, &format!("toast:message:{id}")).unwrap();
        assert!(
            message
                .accessible_name
                .as_deref()
                .is_some_and(|name| name.contains("try exporting again"))
        );
        assert_eq!(
            wrap_toast_message("alpha beta gamma delta", 10, 2),
            vec!["alpha beta", "gamma…"]
        );
        let custom = center.view_with_slots(
            window,
            &appearance,
            ToastSlots {
                header: Some(WidgetNode::label("Activity").with_key("host:header")),
                toast: Some(&|toast| WidgetNode::label(&toast.message).with_key("host:toast")),
                ..Default::default()
            },
        );
        assert!(find(&custom, "host:header").is_some());
        assert!(find(&custom, "host:toast").is_some());
    }

    #[test]
    fn toast_appearance_applies_effects_by_severity() {
        use bbui_style::Rgba;

        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        center.set_visible_limit(4);
        let colors = [
            Rgba(0.1, 0.3, 0.9, 1.0),
            Rgba(0.1, 0.8, 0.4, 1.0),
            Rgba(0.9, 0.6, 0.1, 1.0),
            Rgba(0.9, 0.2, 0.3, 1.0),
        ];
        let effects = colors.map(|color| {
            UiVisualEffects::default().with_border(bbui_widgets::UiBorderEffect::gradient(
                [color],
                2.0,
                8.0,
            ))
        });
        let appearance = ToastAppearance {
            info_effects: Some(effects[0].clone()),
            success_effects: Some(effects[1].clone()),
            warning_effects: Some(effects[2].clone()),
            error_effects: Some(effects[3].clone()),
            ..Default::default()
        };
        let ids = [
            ToastSeverity::Info,
            ToastSeverity::Success,
            ToastSeverity::Warning,
            ToastSeverity::Error,
        ]
        .map(|severity| center.post(window, severity, None, "Message", ToastLifetime::Persistent));
        let view = center.view_with(window, &appearance);
        for (index, id) in ids.into_iter().enumerate() {
            let card = find(&view, &format!("toast:{id}")).expect("toast card");
            assert_eq!(card.visual_effects.as_ref(), Some(&effects[index]));
            assert!(find(card, &format!("toast:dismiss:{id}")).is_some());
        }
    }

    #[test]
    fn floating_toasts_anchor_in_each_corner_and_hide_when_empty() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        let appearance = ToastAppearance::default();
        let overlay = ToastOverlayAppearance::default();
        let empty = center.floating_stack(window, &appearance, &overlay);
        assert!(empty.children.is_empty());
        assert_eq!(empty.key.as_deref(), Some("toast:center"));
        center.post(
            window,
            ToastSeverity::Info,
            None,
            "Ready",
            ToastLifetime::Persistent,
        );
        for corner in [
            ToastCorner::TopLeft,
            ToastCorner::TopRight,
            ToastCorner::BottomLeft,
            ToastCorner::BottomRight,
        ] {
            let view = center.floating_view(
                window,
                WidgetNode::label("Editor").with_key("host:editor"),
                &appearance,
                &ToastOverlayAppearance {
                    corner,
                    ..overlay.clone()
                },
            );
            assert_eq!(view.key.as_deref(), Some("toast:overlay"));
            assert!(find(&view, "host:editor").is_some());
            assert_eq!(
                find(&view, "toast:center").unwrap().role.as_deref(),
                Some("toast-stack")
            );
            let layer = find(&view, "toast:floating-layer").unwrap();
            let row = if matches!(corner, ToastCorner::BottomLeft | ToastCorner::BottomRight) {
                &layer.children[1]
            } else {
                &layer.children[0]
            };
            let stack_index = if matches!(corner, ToastCorner::TopLeft | ToastCorner::BottomLeft) {
                0
            } else {
                1
            };
            assert_eq!(
                row.children[stack_index].key.as_deref(),
                Some("toast:center")
            );
            let card = find(&view, "toast:1").unwrap();
            assert_eq!(
                translation_y(card),
                Some(
                    if matches!(corner, ToastCorner::BottomLeft | ToastCorner::BottomRight) {
                        center.motion.travel
                    } else {
                        -center.motion.travel
                    }
                )
            );
        }
    }

    #[test]
    fn floating_toasts_enter_and_finish_exit_before_removal() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        let id = center.post(
            window,
            ToastSeverity::Success,
            None,
            "Saved",
            ToastLifetime::Persistent,
        );
        let appearance = ToastAppearance::default();
        let overlay = ToastOverlayAppearance::default();
        center.advance(Duration::from_millis(1));
        assert_eq!(
            center.visible(window)[0].entry_phase,
            ToastEntryPhase::Unmounted
        );
        let card = center.floating_stack(window, &appearance, &overlay);
        assert_eq!(
            translation_y(find(&card, &format!("toast:{id}")).unwrap()),
            Some(center.motion.travel)
        );
        let generation = center.generation();
        center.advance(Duration::from_millis(1));
        assert!(center.generation() > generation);
        let card = center.floating_stack(window, &appearance, &overlay);
        let card = find(&card, &format!("toast:{id}")).unwrap();
        assert_eq!(translation_y(card), Some(0.0));
        assert_eq!(
            card.auto_animations.as_ref().unwrap().0[&UiStyleProperty::TranslateY].duration,
            center.motion.enter
        );
        assert!(center.acknowledge(id).is_some());
        assert_eq!(center.pending(window).count(), 0);
        assert_eq!(center.visible(window).len(), 1);
        let card = center.floating_stack(window, &appearance, &overlay);
        let card = find(&card, &format!("toast:{id}")).unwrap();
        assert_eq!(translation_y(card), Some(center.motion.travel));
        assert_eq!(
            card.auto_animations.as_ref().unwrap().0[&UiStyleProperty::TranslateY].duration,
            center.motion.exit
        );
        center.advance(Duration::from_millis(110));
        assert_eq!(center.visible(window).len(), 1);
        center.advance(Duration::from_millis(110));
        assert!(center.visible(window).is_empty());
    }

    #[test]
    fn floating_toast_uses_independent_fade_and_travel_curves() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        let travel_curve = MotionCurve::cubic_bezier(0.2, 0.8, 0.3, 1.0).unwrap();
        let motion = ToastMotion {
            enter_opacity_curve: MotionCurve::EaseInOut,
            enter_travel_curve: travel_curve,
            exit_opacity_curve: MotionCurve::Linear,
            exit_travel_curve: MotionCurve::Step,
            ..ToastMotion::default()
        };
        let generation = center.generation();
        center.set_motion(motion);
        assert!(center.generation() > generation);
        assert_eq!(center.motion(), motion);

        let id = center.post(
            window,
            ToastSeverity::Info,
            Some("Ready".into()),
            "Workspace loaded",
            ToastLifetime::Persistent,
        );
        let appearance = ToastAppearance::default();
        let overlay = ToastOverlayAppearance::default();
        center.floating_stack(window, &appearance, &overlay);
        center.advance(Duration::from_millis(1));
        let stack = center.floating_stack(window, &appearance, &overlay);
        let card = find(&stack, &format!("toast:{id}")).unwrap();
        let animations = &card.auto_animations.as_ref().unwrap().0;
        assert_eq!(
            animations[&UiStyleProperty::Opacity].curve,
            motion.enter_opacity_curve
        );
        assert_eq!(
            animations[&UiStyleProperty::TranslateY].curve,
            motion.enter_travel_curve
        );
        assert_eq!(animations[&UiStyleProperty::Opacity].duration, motion.enter);
        assert_eq!(
            animations[&UiStyleProperty::TranslateY].duration,
            motion.enter
        );
        let message = find(card, &format!("toast:message:{id}")).unwrap();
        assert_eq!(
            message.auto_animations.as_ref().unwrap().0[&UiStyleProperty::Opacity].curve,
            motion.enter_opacity_curve
        );

        center.acknowledge(id);
        let stack = center.floating_stack(window, &appearance, &overlay);
        let card = find(&stack, &format!("toast:{id}")).unwrap();
        let animations = &card.auto_animations.as_ref().unwrap().0;
        assert_eq!(
            animations[&UiStyleProperty::Opacity].curve,
            motion.exit_opacity_curve
        );
        assert_eq!(
            animations[&UiStyleProperty::TranslateY].curve,
            motion.exit_travel_curve
        );
        assert_eq!(animations[&UiStyleProperty::Opacity].duration, motion.exit);
        assert_eq!(
            animations[&UiStyleProperty::TranslateY].duration,
            motion.exit
        );
    }

    #[test]
    fn reduced_motion_snaps_toast_entry_and_exit() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        center.set_reduced_motion(true);
        let id = center.post(
            window,
            ToastSeverity::Info,
            None,
            "Ready",
            ToastLifetime::Persistent,
        );
        let card = center.floating_stack(
            window,
            &ToastAppearance::default(),
            &ToastOverlayAppearance::default(),
        );
        assert_eq!(
            translation_y(find(&card, &format!("toast:{id}")).unwrap()),
            Some(0.0)
        );
        center.acknowledge(id);
        assert!(center.visible(window).is_empty());
    }

    #[test]
    fn timed_toast_expires_then_completes_exit() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        center.post(
            window,
            ToastSeverity::Info,
            None,
            "Saved",
            ToastLifetime::For(Duration::from_millis(200)),
        );
        center.advance(Duration::from_millis(200));
        assert_eq!(center.pending(window).count(), 0);
        assert_eq!(center.visible(window).len(), 1);
        center.advance(Duration::from_millis(219));
        assert_eq!(center.visible(window).len(), 1);
        center.advance(Duration::from_millis(1));
        assert!(center.visible(window).is_empty());
    }

    #[test]
    fn theme_motion_scale_extends_exit_without_extending_toast_lifetime() {
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        center.set_motion_scale(2.0);
        center.post(
            window,
            ToastSeverity::Info,
            None,
            "Saved",
            ToastLifetime::For(Duration::from_millis(200)),
        );
        center.advance(Duration::from_millis(200));
        assert!(center.pending(window).next().is_none());
        assert_eq!(center.visible(window).len(), 1);
        center.advance(Duration::from_millis(439));
        assert_eq!(center.visible(window).len(), 1);
        center.advance(Duration::from_millis(1));
        assert!(center.visible(window).is_empty());
    }

    #[test]
    fn queue_limits_windows_expires_and_acknowledges() {
        let mut world = World::new();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        let first = center.post(
            a,
            ToastSeverity::Info,
            None,
            "Timed",
            ToastLifetime::For(Duration::from_secs(2)),
        );
        center.post(
            b,
            ToastSeverity::Error,
            None,
            "Persistent",
            ToastLifetime::Persistent,
        );
        assert_eq!(center.visible(a).len(), 1);
        assert_eq!(center.visible(b).len(), 1);
        center.advance(Duration::from_secs(2));
        assert!(center.pending(a).next().is_none());
        assert_eq!(center.pending(b).count(), 1);
        assert!(center.acknowledge(first).is_none());
        let id = center.post(
            a,
            ToastSeverity::Warning,
            None,
            "Review",
            ToastLifetime::Persistent,
        );
        let button = world.spawn(UiKey(format!("toast:dismiss:{id}"))).id();
        assert_eq!(
            center
                .handle_widget_event(&world, &WidgetEvent::Activated { target: button })
                .map(|toast| toast.id),
            Some(id)
        );
    }

    #[test]
    fn closing_a_window_removes_only_its_toasts() {
        let mut app = App::new();
        app.add_message::<WindowClosed>()
            .init_resource::<ToastCenter>()
            .add_systems(Update, advance_toasts);
        let first = app.world_mut().spawn_empty().id();
        let second = app.world_mut().spawn_empty().id();
        let center = &mut app.world_mut().resource_mut::<ToastCenter>();
        center.post(
            first,
            ToastSeverity::Info,
            None,
            "Persistent",
            ToastLifetime::Persistent,
        );
        center.post(
            first,
            ToastSeverity::Warning,
            None,
            "Also persistent",
            ToastLifetime::Persistent,
        );
        center.post(
            second,
            ToastSeverity::Success,
            None,
            "Keep",
            ToastLifetime::Persistent,
        );
        let generation = center.generation();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<WindowClosed>>()
            .write(WindowClosed { window: first });
        app.update();
        let center = app.world().resource::<ToastCenter>();
        assert!(center.pending(first).next().is_none());
        assert_eq!(center.pending(second).count(), 1);
        assert!(center.generation() > generation);
    }

    #[test]
    fn visible_stack_is_bounded_and_error_is_assertive() {
        use bbui_widgets::WidgetTree;
        let mut world = World::new();
        let window = world.spawn_empty().id();
        let mut center = ToastCenter::default();
        for index in 0..35 {
            center.post(
                window,
                ToastSeverity::Info,
                None,
                format!("Toast {index}"),
                ToastLifetime::Persistent,
            );
        }
        assert_eq!(center.pending(window).count(), MAX_STORED_PER_WINDOW);
        assert_eq!(center.visible(window).len(), DEFAULT_VISIBLE);
        center.set_visible_limit(2);
        assert_eq!(center.visible(window).len(), 2);
        center.post(
            window,
            ToastSeverity::Error,
            None,
            "Export failed",
            ToastLifetime::Persistent,
        );
        let root = WidgetTree::new(center.view(window))
            .spawn(&mut world, None)
            .unwrap();
        assert!(
            world
                .get::<UiKey>(root)
                .is_some_and(|key| key.0 == "toast:center")
        );
        let mut labels = world.query::<(&UiKey, &UiLiveRegion)>();
        assert!(labels.iter(&world).any(|(key, region)| {
            key.0.starts_with("toast:message:") && *region == UiLiveRegion::assertive().atomic()
        }));
    }
}

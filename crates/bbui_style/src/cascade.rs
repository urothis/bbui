use super::*;

/// Named values accepted by inline styles and style rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UiStyleProperty {
    Background,
    Foreground,
    MutedForeground,
    BorderColor,
    AccentColor,
    SelectionColor,
    FocusColor,
    DisabledColor,
    ErrorColor,
    IconColor,
    MutedIconColor,
    DisabledIconColor,
    BorderWidth,
    CornerRadius,
    Opacity,
    BoxShadow,
    FontSize,
    LineHeight,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    Padding,
    Margin,
    Gap,
    LayoutMode,
    AlignItems,
    JustifyContent,
    ClipContents,
    ZIndex,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiStyleValue {
    Color(Rgba),
    /// Late-bound color token. Resolution stores a concrete `Color` in the
    /// computed style so theme switches participate in transitions and change
    /// detection.
    ColorToken(ThemeColorToken),
    BoxShadow(UiBoxShadow),
    Scalar(f32),
    Length(UiLength),
    Edges(UiEdges),
    LayoutMode(UiLayoutMode),
    Align(UiAlign),
    Justify(UiJustify),
    Bool(bool),
    Integer(i32),
}

#[derive(Component, Debug, Clone, Default, PartialEq)]
#[component(summary_tick)]
pub struct UiInlineStyle(pub BTreeMap<UiStyleProperty, UiStyleValue>);

impl UiInlineStyle {
    pub fn set(&mut self, property: UiStyleProperty, value: UiStyleValue) {
        self.0.insert(property, value);
    }

    pub fn set_color_token(&mut self, property: UiStyleProperty, token: ThemeColorToken) {
        self.set(property, UiStyleValue::ColorToken(token));
    }

    pub fn remove(&mut self, property: UiStyleProperty) -> Option<UiStyleValue> {
        self.0.remove(&property)
    }
}

/// Timing for an automatic two-keyframe style clip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiAnimationSpec {
    pub duration: Duration,
    pub curve: bbui_motion::MotionCurve,
}

impl UiAnimationSpec {
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            curve: bbui_motion::MotionCurve::Linear,
        }
    }

    pub const fn curve(mut self, curve: bbui_motion::MotionCurve) -> Self {
        self.curve = curve;
        self
    }
}

impl ThemeMotion {
    pub const fn duration(self, token: ThemeMotionToken) -> Duration {
        match token {
            ThemeMotionToken::Fast => self.fast,
            ThemeMotionToken::Standard => self.standard,
            ThemeMotionToken::Slow => self.slow,
            ThemeMotionToken::PanelExpand => self.panel_expand,
            ThemeMotionToken::MenuOpen => self.menu_open,
        }
    }

    pub const fn animation(self, token: ThemeMotionToken) -> UiAnimationSpec {
        let curve = match token {
            ThemeMotionToken::Fast | ThemeMotionToken::MenuOpen => MotionCurve::EaseOut,
            ThemeMotionToken::Standard | ThemeMotionToken::Slow | ThemeMotionToken::PanelExpand => {
                MotionCurve::EaseInOut
            }
        };
        UiAnimationSpec::new(self.duration(token)).curve(curve)
    }
}

/// Per-property automatic keyframe settings. Unsupported value pairs snap to target.
#[derive(Component, Debug, Clone, Default, PartialEq)]
#[component(summary_tick)]
pub struct UiAutoAnimations(pub BTreeMap<UiStyleProperty, UiAnimationSpec>);

impl UiAutoAnimations {
    pub fn set(&mut self, property: UiStyleProperty, spec: UiAnimationSpec) {
        self.0.insert(property, spec);
    }

    pub fn set_motion(
        &mut self,
        property: UiStyleProperty,
        theme: &Theme,
        token: ThemeMotionToken,
    ) {
        self.set(property, theme.motion.animation(token));
    }

    pub fn remove(&mut self, property: UiStyleProperty) -> Option<UiAnimationSpec> {
        self.0.remove(&property)
    }
}

/// Accessibility override that disables all style animation immediately.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiReducedMotion(pub bool);

#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiClasses(pub Vec<String>);

impl UiClasses {
    pub fn contains(&self, class: &str) -> bool {
        self.0.iter().any(|item| item == class)
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiRole(pub String);

impl UiRole {
    pub fn new(role: impl Into<String>) -> Self {
        Self(role.into())
    }
}

/// Bitset for state selectors. Multiple states can be active together.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[component(summary_tick)]
pub struct UiStyleState(pub u16);

impl UiStyleState {
    pub const HOVERED: Self = Self(1 << 0);
    pub const PRESSED: Self = Self(1 << 1);
    pub const FOCUSED: Self = Self(1 << 2);
    pub const DISABLED: Self = Self(1 << 3);
    pub const SELECTED: Self = Self(1 << 4);
    pub const CHECKED: Self = Self(1 << 5);
    pub const ERROR: Self = Self(1 << 6);
    /// A control-specific active item, such as the current table cell.
    pub const ACTIVE: Self = Self(1 << 7);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn set(&mut self, state: Self, enabled: bool) -> bool {
        let next = if enabled {
            self.union(state)
        } else {
            self.without(state)
        };
        let changed = self.0 != next.0;
        self.0 = next.0;
        changed
    }
}

/// Set or clear dynamic state flags without replacing other flags on an
/// entity. Widgets typically call this after selection or validation changes;
/// the next style resolution computes the new visual state.
pub fn set_ui_style_state(
    world: &mut World,
    entity: Entity,
    state: UiStyleState,
    enabled: bool,
) -> bool {
    let mut current = world
        .get::<UiStyleState>(entity)
        .copied()
        .unwrap_or_default();
    if !current.set(state, enabled) {
        return false;
    }
    if current.0 == 0 {
        world.entity_mut(entity).remove::<UiStyleState>();
    } else {
        world.entity_mut(entity).insert(current);
    }
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiStyleSelector {
    pub role: Option<String>,
    pub class: Option<String>,
    pub required_state: UiStyleState,
    pub forbidden_state: UiStyleState,
}

impl UiStyleSelector {
    pub(super) fn matches(
        &self,
        role: Option<&UiRole>,
        classes: Option<&UiClasses>,
        state: UiStyleState,
    ) -> bool {
        self.role
            .as_ref()
            .is_none_or(|wanted| role.is_some_and(|actual| &actual.0 == wanted))
            && self
                .class
                .as_ref()
                .is_none_or(|wanted| classes.is_some_and(|actual| actual.contains(wanted)))
            && state.contains(self.required_state)
            && !state.intersects(self.forbidden_state)
    }

    pub(super) fn specificity(&self) -> u16 {
        u16::from(self.role.is_some()) * 100
            + u16::from(self.class.is_some()) * 10
            + self.required_state.0.count_ones() as u16 * 10
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiStyleDeclaration(pub BTreeMap<UiStyleProperty, UiStyleValue>);

impl UiStyleDeclaration {
    pub fn set(&mut self, property: UiStyleProperty, value: UiStyleValue) {
        self.0.insert(property, value);
    }

    pub fn set_color_token(&mut self, property: UiStyleProperty, token: ThemeColorToken) {
        self.set(property, UiStyleValue::ColorToken(token));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiStyleRule {
    pub selector: UiStyleSelector,
    pub declaration: UiStyleDeclaration,
    /// Rules with higher priority win after selector specificity.
    pub priority: i16,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct UiStyleSheet {
    pub rules: Vec<UiStyleRule>,
}

impl UiStyleSheet {
    pub fn push(&mut self, rule: UiStyleRule) {
        self.rules.push(rule);
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }
}

#[derive(Component, Debug, Clone, Default, PartialEq)]
#[component(summary_tick)]
pub struct UiComputedStyle {
    pub properties: HashMap<UiStyleProperty, UiStyleValue>,
}

impl UiComputedStyle {
    pub fn get(&self, property: UiStyleProperty) -> Option<UiStyleValue> {
        self.properties.get(&property).cloned()
    }

    pub fn color(&self, property: UiStyleProperty) -> Option<Rgba> {
        match self.get(property) {
            Some(UiStyleValue::Color(value)) => Some(value),
            _ => None,
        }
    }

    pub fn scalar(&self, property: UiStyleProperty) -> Option<f32> {
        match self.get(property) {
            Some(UiStyleValue::Scalar(value)) => Some(value),
            _ => None,
        }
    }

    pub fn box_shadow(&self) -> Option<UiBoxShadow> {
        match self.get(UiStyleProperty::BoxShadow) {
            Some(UiStyleValue::BoxShadow(value)) => Some(value),
            _ => None,
        }
    }
}

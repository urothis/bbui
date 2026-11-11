use super::*;

/// sRGB color channels and linear alpha in the 0..=1 range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba(pub f32, pub f32, pub f32, pub f32);

impl Rgba {
    pub const TRANSPARENT: Self = Self(0.0, 0.0, 0.0, 0.0);

    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self(clamp(red), clamp(green), clamp(blue), clamp(alpha))
    }

    pub fn with_alpha(self, alpha: f32) -> Self {
        Self(self.0, self.1, self.2, clamp(alpha))
    }

    pub fn mix(self, other: Self, amount: f32) -> Self {
        let amount = clamp(amount);
        Self(
            mix_channel(self.0, other.0, amount),
            mix_channel(self.1, other.1, amount),
            mix_channel(self.2, other.2, amount),
            mix_channel(self.3, other.3, amount),
        )
    }

    pub fn is_finite(self) -> bool {
        [self.0, self.1, self.2, self.3]
            .into_iter()
            .all(f32::is_finite)
    }
}

/// Semantic colors available to widgets, editors, and style selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeColorToken {
    Background,
    Surface,
    HoverSurface,
    PressedSurface,
    SelectedSurface,
    DisabledSurface,
    Text,
    MutedText,
    DisabledText,
    Accent,
    Border,
    DisabledBorder,
    Focus,
    Error,
    Icon,
    MutedIcon,
    DisabledIcon,
    ButtonDisabledSurface,
    ButtonHoverSurface,
    MenuOpenSurface,
    MenuBorder,
    MenuDisabledSurface,
    MenuSelectedSurface,
    MenuDisabledText,
    FileBrowserSelectedSurface,
    TextSelectionOverlay,
    DataTableTitleSurface,
    DataTableHeaderSurface,
    DataTableSelectedHeaderSurface,
    DataTableSelectedCellSurface,
    DataTableActiveCellSurface,
    CollectionDisabledSurface,
    CollectionSelectedSurface,
    CollectionSelectionOverlay,
    EditorActiveFieldSurface,
    EditorCaret,
    EditorImeUnderline,
    EditorChromeSurface,
    EditorSubpanelSurface,
    EditorProgressTrack,
    EditorObjectAccent,
    CurvePlotSurface,
    CurveGridLine,
    CurveAxisLine,
    NodeGraphSurface,
    NodeGraphSelectedNodeSurface,
    NodeGraphHeaderSurface,
    NodeGraphGridLine,
    NodeGraphSelectionOverlay,
    NodeGraphBooleanSocket,
    NodeGraphFloatSocket,
    NodeGraphVectorSocket,
    NodeGraphColorSocket,
    NodeGraphTextSocket,
    PreviewPlaceholder,
    PopupShadow,
    CanvasSelectionOverlay,
    ControlDisabledSurface,
    ControlDisabledMark,
    ControlDisabledText,
}

impl ThemeColorToken {
    /// Every built-in semantic color token. Keep this list in sync with
    /// [`Theme::color`] so theme completeness can be checked exhaustively.
    pub const ALL: [Self; 60] = [
        Self::Background,
        Self::Surface,
        Self::HoverSurface,
        Self::PressedSurface,
        Self::SelectedSurface,
        Self::DisabledSurface,
        Self::Text,
        Self::MutedText,
        Self::DisabledText,
        Self::Accent,
        Self::Border,
        Self::DisabledBorder,
        Self::Focus,
        Self::Error,
        Self::Icon,
        Self::MutedIcon,
        Self::DisabledIcon,
        Self::ButtonDisabledSurface,
        Self::ButtonHoverSurface,
        Self::MenuOpenSurface,
        Self::MenuBorder,
        Self::MenuDisabledSurface,
        Self::MenuSelectedSurface,
        Self::MenuDisabledText,
        Self::FileBrowserSelectedSurface,
        Self::TextSelectionOverlay,
        Self::DataTableTitleSurface,
        Self::DataTableHeaderSurface,
        Self::DataTableSelectedHeaderSurface,
        Self::DataTableSelectedCellSurface,
        Self::DataTableActiveCellSurface,
        Self::CollectionDisabledSurface,
        Self::CollectionSelectedSurface,
        Self::CollectionSelectionOverlay,
        Self::EditorActiveFieldSurface,
        Self::EditorCaret,
        Self::EditorImeUnderline,
        Self::EditorChromeSurface,
        Self::EditorSubpanelSurface,
        Self::EditorProgressTrack,
        Self::EditorObjectAccent,
        Self::CurvePlotSurface,
        Self::CurveGridLine,
        Self::CurveAxisLine,
        Self::NodeGraphSurface,
        Self::NodeGraphSelectedNodeSurface,
        Self::NodeGraphHeaderSurface,
        Self::NodeGraphGridLine,
        Self::NodeGraphSelectionOverlay,
        Self::NodeGraphBooleanSocket,
        Self::NodeGraphFloatSocket,
        Self::NodeGraphVectorSocket,
        Self::NodeGraphColorSocket,
        Self::NodeGraphTextSocket,
        Self::PreviewPlaceholder,
        Self::PopupShadow,
        Self::CanvasSelectionOverlay,
        Self::ControlDisabledSurface,
        Self::ControlDisabledMark,
        Self::ControlDisabledText,
    ];
}

/// A box-shadow color that may be a fixed sRGBA value or late-bound theme token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiShadowColor {
    Color(Rgba),
    Token(ThemeColorToken),
}

/// One logical-pixel shadow layer in a [`UiBoxShadow`] effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiShadow {
    pub color: UiShadowColor,
    pub x_offset: f32,
    pub y_offset: f32,
    pub spread_radius: f32,
    pub blur_radius: f32,
}

impl UiShadow {
    pub const fn new(
        color: Rgba,
        x_offset: f32,
        y_offset: f32,
        spread_radius: f32,
        blur_radius: f32,
    ) -> Self {
        Self {
            color: UiShadowColor::Color(color),
            x_offset,
            y_offset,
            spread_radius,
            blur_radius,
        }
    }

    pub const fn themed(
        color: ThemeColorToken,
        x_offset: f32,
        y_offset: f32,
        spread_radius: f32,
        blur_radius: f32,
    ) -> Self {
        Self {
            color: UiShadowColor::Token(color),
            x_offset,
            y_offset,
            spread_radius,
            blur_radius,
        }
    }

    pub(super) fn interpolate(self, target: Self, progress: f32) -> Option<Self> {
        let color = match (self.color, target.color) {
            (UiShadowColor::Color(from), UiShadowColor::Color(to)) => {
                UiShadowColor::Color(from.mix(to, progress))
            }
            (UiShadowColor::Token(from), UiShadowColor::Token(to)) if from == to => {
                UiShadowColor::Token(from)
            }
            _ => return None,
        };
        Some(Self {
            color,
            x_offset: mix_channel(self.x_offset, target.x_offset, progress),
            y_offset: mix_channel(self.y_offset, target.y_offset, progress),
            spread_radius: mix_channel(self.spread_radius, target.spread_radius, progress),
            blur_radius: mix_channel(self.blur_radius, target.blur_radius, progress),
        })
    }

    pub(super) fn resolve_theme_color(self, theme: &Theme) -> Self {
        Self {
            color: match self.color {
                UiShadowColor::Token(token) => UiShadowColor::Color(theme.color(token)),
                color => color,
            },
            ..self
        }
    }

    pub(super) fn to_native(self, theme: &Theme) -> bevy_ui::ShadowStyle {
        let color = match self.color {
            UiShadowColor::Color(color) => color,
            UiShadowColor::Token(token) => theme.color(token),
        };
        let color = Rgba::new(color.0, color.1, color.2, color.3);
        let px = |value: f32| Val::Px(if value.is_finite() { value } else { 0.0 });
        bevy_ui::ShadowStyle {
            color: Color::srgba(color.0, color.1, color.2, color.3),
            x_offset: px(self.x_offset),
            y_offset: px(self.y_offset),
            spread_radius: px(self.spread_radius),
            blur_radius: px(if self.blur_radius.is_finite() {
                self.blur_radius.max(0.0)
            } else {
                0.0
            }),
        }
    }
}

/// An ordered, possibly empty list of native Bevy UI box-shadow layers.
///
/// Values are cloned through style declarations and computed styles. Layer
/// order matches Bevy's back-to-front rendering order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiBoxShadow {
    pub shadows: Vec<UiShadow>,
}

impl UiBoxShadow {
    pub fn new(
        color: Rgba,
        x_offset: f32,
        y_offset: f32,
        spread_radius: f32,
        blur_radius: f32,
    ) -> Self {
        Self {
            shadows: vec![UiShadow::new(
                color,
                x_offset,
                y_offset,
                spread_radius,
                blur_radius,
            )],
        }
    }

    pub fn themed(
        color: ThemeColorToken,
        x_offset: f32,
        y_offset: f32,
        spread_radius: f32,
        blur_radius: f32,
    ) -> Self {
        Self {
            shadows: vec![UiShadow::themed(
                color,
                x_offset,
                y_offset,
                spread_radius,
                blur_radius,
            )],
        }
    }

    pub fn from_layers(shadows: impl IntoIterator<Item = UiShadow>) -> Self {
        Self {
            shadows: shadows.into_iter().collect(),
        }
    }

    pub fn push(&mut self, shadow: UiShadow) {
        self.shadows.push(shadow);
    }

    pub fn is_empty(&self) -> bool {
        self.shadows.is_empty()
    }

    pub(super) fn interpolate(&self, target: &Self, progress: f32) -> Option<Self> {
        if self.shadows.len() != target.shadows.len() {
            return None;
        }
        Some(Self::from_layers(
            self.shadows
                .iter()
                .copied()
                .zip(target.shadows.iter().copied())
                .map(|(from, to)| from.interpolate(to, progress))
                .collect::<Option<Vec<_>>>()?,
        ))
    }

    pub(super) fn resolve_theme_color(&self, theme: &Theme) -> Self {
        Self::from_layers(
            self.shadows
                .iter()
                .map(|shadow| shadow.resolve_theme_color(theme)),
        )
    }

    pub(super) fn to_native(&self, theme: &Theme) -> BoxShadow {
        BoxShadow(
            self.shadows
                .iter()
                .copied()
                .map(|shadow| shadow.to_native(theme))
                .collect(),
        )
    }
}

/// Theme-level effects used by semantic roles such as popup surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeEffects {
    pub popup_shadow: UiBoxShadow,
}

impl Default for ThemeEffects {
    fn default() -> Self {
        Self {
            popup_shadow: UiBoxShadow::themed(ThemeColorToken::PopupShadow, 0.0, 4.0, 0.0, 12.0),
        }
    }
}

mod spacing;
pub use spacing::*;

/// Named durations for common interface transitions. [`Theme::motion_scale`]
/// scales these durations together and [`UiReducedMotion`] can snap them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeMotion {
    pub fast: Duration,
    pub standard: Duration,
    pub slow: Duration,
    pub panel_expand: Duration,
    pub menu_open: Duration,
}

impl Default for ThemeMotion {
    fn default() -> Self {
        Self {
            fast: Duration::from_millis(80),
            standard: Duration::from_millis(160),
            slow: Duration::from_millis(280),
            panel_expand: Duration::from_millis(160),
            menu_open: Duration::from_millis(120),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMotionToken {
    Fast,
    Standard,
    Slow,
    PanelExpand,
    MenuOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeKind {
    #[default]
    Dark,
    Light,
}

#[derive(Resource, Debug, Clone)]
pub struct Theme {
    pub kind: ThemeKind,
    pub background: Rgba,
    pub surface: Rgba,
    pub surface_hover: Rgba,
    pub surface_pressed: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub border: Rgba,
    pub selection: Rgba,
    pub disabled: Rgba,
    pub error: Rgba,
    pub focus: Rgba,
    pub disabled_surface: Rgba,
    pub disabled_border: Rgba,
    pub disabled_opacity: f32,
    pub spacing_unit: f32,
    /// Semantic spacing values for widgets which expose standard roles.
    pub spacing: ThemeSpacing,
    /// Typed visual effects used by elevated semantic roles.
    pub effects: ThemeEffects,
    pub corner_radius: f32,
    pub control_height: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub motion: ThemeMotion,
    pub motion_scale: f32,
}

impl Theme {
    pub fn new(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Dark => Self {
                kind,
                background: Rgba(0.10, 0.10, 0.10, 1.0),
                surface: Rgba(0.20, 0.20, 0.20, 1.0),
                surface_hover: Rgba(0.27, 0.27, 0.27, 1.0),
                surface_pressed: Rgba(0.14, 0.14, 0.14, 1.0),
                text: Rgba(0.88, 0.88, 0.88, 1.0),
                text_muted: Rgba(0.60, 0.60, 0.60, 1.0),
                accent: Rgba(0.32, 0.55, 0.78, 1.0),
                border: Rgba(0.06, 0.06, 0.06, 1.0),
                selection: Rgba(0.18, 0.35, 0.52, 1.0),
                disabled: Rgba(0.46, 0.46, 0.46, 1.0),
                error: Rgba(0.80, 0.22, 0.18, 1.0),
                focus: Rgba(0.52, 0.70, 0.96, 1.0),
                disabled_surface: Rgba(0.16, 0.16, 0.16, 1.0),
                disabled_border: Rgba(0.24, 0.24, 0.24, 1.0),
                disabled_opacity: 0.72,
                spacing_unit: 4.0,
                spacing: ThemeSpacing::default(),
                effects: ThemeEffects::default(),
                corner_radius: 3.0,
                control_height: 24.0,
                font_size: 14.0,
                line_height: 1.25,
                motion: ThemeMotion::default(),
                motion_scale: 1.0,
            },
            ThemeKind::Light => Self {
                kind,
                background: Rgba(0.86, 0.86, 0.86, 1.0),
                surface: Rgba(0.94, 0.94, 0.94, 1.0),
                surface_hover: Rgba(0.98, 0.98, 0.98, 1.0),
                surface_pressed: Rgba(0.82, 0.82, 0.82, 1.0),
                text: Rgba(0.08, 0.08, 0.08, 1.0),
                text_muted: Rgba(0.35, 0.35, 0.35, 1.0),
                accent: Rgba(0.18, 0.38, 0.68, 1.0),
                border: Rgba(0.67, 0.67, 0.67, 1.0),
                selection: Rgba(0.67, 0.79, 0.93, 1.0),
                disabled: Rgba(0.56, 0.56, 0.56, 1.0),
                error: Rgba(0.70, 0.14, 0.12, 1.0),
                focus: Rgba(0.12, 0.30, 0.62, 1.0),
                disabled_surface: Rgba(0.89, 0.89, 0.89, 1.0),
                disabled_border: Rgba(0.78, 0.78, 0.78, 1.0),
                disabled_opacity: 0.72,
                spacing_unit: 4.0,
                spacing: ThemeSpacing::default(),
                effects: ThemeEffects::default(),
                corner_radius: 3.0,
                control_height: 24.0,
                font_size: 14.0,
                line_height: 1.25,
                motion: ThemeMotion::default(),
                motion_scale: 1.0,
            },
        }
    }

    pub fn switch(&mut self, kind: ThemeKind) {
        *self = Self::new(kind);
    }

    pub fn color(&self, token: ThemeColorToken) -> Rgba {
        match token {
            ThemeColorToken::Background => self.background,
            ThemeColorToken::Surface => self.surface,
            ThemeColorToken::HoverSurface => self.surface_hover,
            ThemeColorToken::PressedSurface => self.surface_pressed,
            ThemeColorToken::SelectedSurface => self.selection,
            ThemeColorToken::DisabledSurface => self.disabled_surface,
            ThemeColorToken::Text => self.text,
            ThemeColorToken::MutedText => self.text_muted,
            ThemeColorToken::DisabledText => self.disabled,
            ThemeColorToken::Accent => self.accent,
            ThemeColorToken::Border => self.border,
            ThemeColorToken::DisabledBorder => self.disabled_border,
            ThemeColorToken::Focus => self.focus,
            ThemeColorToken::Error => self.error,
            ThemeColorToken::Icon => self.text,
            ThemeColorToken::MutedIcon => self.text_muted,
            ThemeColorToken::DisabledIcon => self.disabled,
            ThemeColorToken::ButtonDisabledSurface => self.surface.mix(self.background, 0.45),
            ThemeColorToken::ButtonHoverSurface => self.surface.mix(self.accent, 0.18),
            ThemeColorToken::MenuOpenSurface => self.surface.mix(self.accent, 0.24),
            ThemeColorToken::MenuBorder => self.border,
            ThemeColorToken::MenuDisabledSurface => self.surface.mix(self.background, 0.48),
            ThemeColorToken::MenuSelectedSurface => self.surface.mix(self.accent, 0.34),
            ThemeColorToken::MenuDisabledText => self.text.mix(self.background, 0.5),
            ThemeColorToken::FileBrowserSelectedSurface => self.surface.mix(self.accent, 0.34),
            ThemeColorToken::TextSelectionOverlay => self.accent.with_alpha(self.accent.3 * 0.32),
            ThemeColorToken::DataTableTitleSurface => self.surface.mix(self.background, 0.18),
            ThemeColorToken::DataTableHeaderSurface => self.surface.mix(self.background, 0.12),
            ThemeColorToken::DataTableSelectedHeaderSurface => {
                self.surface.mix(self.selection, 0.34)
            }
            ThemeColorToken::DataTableSelectedCellSurface => self.surface.mix(self.selection, 0.48),
            ThemeColorToken::DataTableActiveCellSurface => self.surface.mix(self.selection, 0.68),
            ThemeColorToken::CollectionDisabledSurface => self.surface.mix(self.background, 0.38),
            ThemeColorToken::CollectionSelectedSurface => self.surface.mix(self.accent, 0.28),
            ThemeColorToken::CollectionSelectionOverlay => {
                self.accent.with_alpha(self.accent.3 * 0.16)
            }
            ThemeColorToken::EditorActiveFieldSurface => self.background.mix(self.accent, 0.16),
            ThemeColorToken::EditorCaret => self.text,
            ThemeColorToken::EditorImeUnderline => self.accent,
            ThemeColorToken::EditorChromeSurface => match self.kind {
                ThemeKind::Dark => self.background.mix(Rgba(0.0, 0.0, 0.0, 1.0), 0.5),
                ThemeKind::Light => Rgba(179.0 / 255.0, 179.0 / 255.0, 179.0 / 255.0, 1.0),
            },
            ThemeColorToken::EditorSubpanelSurface => self.surface.mix(self.background, 0.615),
            ThemeColorToken::EditorProgressTrack => {
                self.background.mix(Rgba(0.0, 0.0, 0.0, 1.0), 0.4)
            }
            ThemeColorToken::EditorObjectAccent => match self.kind {
                ThemeKind::Dark => Rgba(0.91, 0.52, 0.23, 1.0),
                ThemeKind::Light => Rgba(0.67, 0.31, 0.06, 1.0),
            },
            ThemeColorToken::CurvePlotSurface => self.background.mix(self.surface, 0.25),
            ThemeColorToken::CurveGridLine => self.border.mix(self.background, 0.35),
            ThemeColorToken::CurveAxisLine => self.border.mix(self.text, 0.25),
            ThemeColorToken::NodeGraphSurface => self.background.mix(self.surface, 0.22),
            ThemeColorToken::NodeGraphSelectedNodeSurface => self.surface.mix(self.selection, 0.55),
            ThemeColorToken::NodeGraphHeaderSurface => self.surface.mix(self.background, 0.22),
            ThemeColorToken::NodeGraphGridLine => self.border.mix(self.background, 0.35),
            ThemeColorToken::NodeGraphSelectionOverlay => {
                self.accent.with_alpha(self.accent.3 * 0.16)
            }
            ThemeColorToken::NodeGraphBooleanSocket => match self.kind {
                ThemeKind::Dark => Rgba(0.85, 0.24, 0.28, 1.0),
                ThemeKind::Light => Rgba(0.72, 0.12, 0.16, 1.0),
            },
            ThemeColorToken::NodeGraphFloatSocket => match self.kind {
                ThemeKind::Dark => Rgba(0.28, 0.55, 0.94, 1.0),
                ThemeKind::Light => Rgba(0.12, 0.35, 0.72, 1.0),
            },
            ThemeColorToken::NodeGraphVectorSocket => match self.kind {
                ThemeKind::Dark => Rgba(0.55, 0.38, 0.88, 1.0),
                ThemeKind::Light => Rgba(0.40, 0.22, 0.70, 1.0),
            },
            ThemeColorToken::NodeGraphColorSocket => match self.kind {
                ThemeKind::Dark => Rgba(0.96, 0.62, 0.18, 1.0),
                ThemeKind::Light => Rgba(0.78, 0.40, 0.06, 1.0),
            },
            ThemeColorToken::NodeGraphTextSocket => match self.kind {
                ThemeKind::Dark => Rgba(0.30, 0.76, 0.43, 1.0),
                ThemeKind::Light => Rgba(0.12, 0.52, 0.26, 1.0),
            },
            ThemeColorToken::PreviewPlaceholder => {
                let (red, green, blue) = match self.kind {
                    ThemeKind::Dark => (0.34, 0.38, 0.46),
                    ThemeKind::Light => (0.73, 0.76, 0.82),
                };
                Rgba(red, green, blue, self.surface.3)
            }
            ThemeColorToken::PopupShadow => match self.kind {
                ThemeKind::Dark => Rgba(0.0, 0.0, 0.0, 0.52),
                ThemeKind::Light => Rgba(0.0, 0.0, 0.0, 0.24),
            },
            ThemeColorToken::CanvasSelectionOverlay => self.accent.with_alpha(self.accent.3 * 0.16),
            ThemeColorToken::ControlDisabledSurface => self.surface.mix(self.background, 0.5),
            ThemeColorToken::ControlDisabledMark => self.text.mix(self.background, 0.55),
            ThemeColorToken::ControlDisabledText => self.text.mix(self.background, 0.45),
        }
    }

    pub(super) fn value(&self, property: UiStyleProperty) -> Option<UiStyleValue> {
        use UiStyleProperty::*;
        Some(match property {
            Background => UiStyleValue::Color(self.surface),
            Foreground => UiStyleValue::Color(self.text),
            MutedForeground => UiStyleValue::Color(self.text_muted),
            BorderColor => UiStyleValue::Color(self.border),
            AccentColor => UiStyleValue::Color(self.accent),
            SelectionColor => UiStyleValue::Color(self.selection),
            FocusColor => UiStyleValue::Color(self.focus),
            DisabledColor => UiStyleValue::Color(self.disabled),
            ErrorColor => UiStyleValue::Color(self.error),
            IconColor => UiStyleValue::Color(self.text),
            MutedIconColor => UiStyleValue::Color(self.text_muted),
            DisabledIconColor => UiStyleValue::Color(self.disabled),
            FontSize => UiStyleValue::Scalar(self.font_size),
            LineHeight => UiStyleValue::Scalar(self.line_height),
            CornerRadius => UiStyleValue::Scalar(self.corner_radius),
            Opacity => UiStyleValue::Scalar(1.0),
            BorderWidth => UiStyleValue::Scalar(1.0),
            _ => return None,
        })
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(ThemeKind::Dark)
    }
}

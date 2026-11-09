//! Typed theme tokens and a deterministic state-aware style cascade.

use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use bevy_app::{App, Plugin, Update};
use bevy_color::Color;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_time::Time;
use bevy_ui::{BoxShadow, Val};
use bui_core::{
    UiEnabled, UiFocus, UiNode, UiParent, UiRoutedEvent, UiRoutedEvents, UiSystems,
    refresh_ui_hover,
};
use bui_layout::{
    UiAlign, UiEdges, UiJustify, UiLayoutMode, UiLayoutNode, UiLength, compute_layout,
};

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
    pub const ALL: [Self; 56] = [
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

    fn interpolate(self, target: Self, progress: f32) -> Option<Self> {
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

    fn resolve_theme_color(self, theme: &Theme) -> Self {
        Self {
            color: match self.color {
                UiShadowColor::Token(token) => UiShadowColor::Color(theme.color(token)),
                color => color,
            },
            ..self
        }
    }

    fn to_native(self, theme: &Theme) -> bevy_ui::ShadowStyle {
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

    fn interpolate(&self, target: &Self, progress: f32) -> Option<Self> {
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

    fn resolve_theme_color(&self, theme: &Theme) -> Self {
        Self::from_layers(
            self.shadows
                .iter()
                .map(|shadow| shadow.resolve_theme_color(theme)),
        )
    }

    fn to_native(&self, theme: &Theme) -> BoxShadow {
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

/// Spacing for common control and editor families. These values are logical
/// pixels and are intentionally separate from colors so a theme can keep
/// menu, list, table, and editor geometry coherent across a live switch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeControlSpacing {
    pub padding_x: f32,
    pub padding_y: f32,
    pub gap: f32,
    pub checkbox_label_gap: f32,
    pub error_outline_width: f32,
    pub slider_height: f32,
    /// Logical height of the painted slider track, independent of its hit area.
    pub slider_track_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePanelSpacing {
    pub header_height: f32,
    pub content_padding: f32,
    pub section_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeMenuSpacing {
    pub item_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub section_gap: f32,
    pub popup_padding: f32,
    pub pie_inner_inset: f32,
    pub pie_inner_radius: f32,
    pub pie_outer_radius: f32,
    pub pie_popup_diameter: f32,
    pub pie_outer_item_radius: f32,
    pub pie_nested_item_radius: f32,
    pub pie_item_max_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeListSpacing {
    pub row_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub indent: f32,
    pub section_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTableSpacing {
    pub header_height: f32,
    pub row_height: f32,
    pub cell_padding_x: f32,
    pub cell_padding_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeEditorSpacing {
    pub padding_x: f32,
    pub padding_y: f32,
    pub gutter_width: f32,
    pub toolbar_gap: f32,
    pub handle_radius: f32,
    pub grid_spacing: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeCurveSpacing {
    pub selected_status_width: f32,
    pub interpolation_status_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeHsvSpacing {
    pub wheel_center_x: f32,
    pub wheel_center_y: f32,
    pub wheel_outer_radius: f32,
    pub wheel_inner_radius: f32,
    pub preview_right_inset: f32,
    pub preview_top_inset: f32,
    pub preview_width: f32,
    pub preview_height: f32,
    pub controls_x: f32,
    pub value_label_y: f32,
    pub value_track_y: f32,
    pub value_track_width_inset: f32,
    pub value_track_height: f32,
    pub alpha_label_y: f32,
    pub alpha_track_y: f32,
    pub alpha_track_width_inset: f32,
    pub alpha_field_y: f32,
    pub alpha_field_right_inset: f32,
    pub alpha_field_width: f32,
    pub field_cell_width: f32,
    pub field_gap: f32,
    pub field_first_row_y: f32,
    pub field_second_row_y: f32,
    pub field_hex_y: f32,
    pub field_hex_width_inset: f32,
    pub field_height: f32,
    pub field_text_inset_x: f32,
    pub field_text_inset_y: f32,
    pub help_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColorRampSpacing {
    pub title_x: f32,
    pub title_y: f32,
    pub button_width: f32,
    pub button_step: f32,
    pub button_right_inset: f32,
    pub button_top_inset: f32,
    pub button_height: f32,
    pub header_hit_height: f32,
    pub button_label_x: f32,
    pub button_label_y: f32,
    pub track_x: f32,
    pub track_y: f32,
    pub track_width_inset: f32,
    pub track_height: f32,
    pub marker_line_extension: f32,
    pub marker_hit_extension: f32,
    pub track_hit_top_inset: f32,
    pub marker_handle_radius: f32,
    pub marker_handle_top_inset: f32,
    pub details_x: f32,
    pub details_y: f32,
    pub channel_x: f32,
    pub channel_y: f32,
    pub channel_step: f32,
    pub channel_width_inset: f32,
    pub channel_height: f32,
    pub channel_selection_hit_slop: f32,
    pub channel_click_hit_slop: f32,
    pub channel_label_hit_slop: f32,
    pub channel_label_x: f32,
    pub channel_label_y: f32,
    pub channel_value_right_inset: f32,
    pub editing_right_inset: f32,
    pub editing_bottom_inset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTooltipSpacing {
    pub padding_x: f32,
    pub padding_y: f32,
    pub maximum_width: f32,
    pub viewport_margin: f32,
    pub anchor_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBoxSpacing {
    pub padding: f32,
    pub gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePropertyRowSpacing {
    pub label_width: f32,
    pub gap: f32,
    pub row_height: f32,
}

/// Logical geometry used by node graph editors before applying the graph zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeNodeGraphSpacing {
    pub header_height: f32,
    pub header_hint_width: f32,
    pub body_padding: f32,
    pub socket_spacing: f32,
    pub socket_radius: f32,
    pub socket_hit_slop: f32,
    pub connection_tangent_min: f32,
    pub grid_spacing: f32,
    pub grid_spacing_min: f32,
    pub grid_spacing_max: f32,
    pub node_radius: f32,
    pub title_padding_x: f32,
    pub title_padding_y: f32,
    pub collapse_toggle_inset_x: f32,
    pub socket_label_inset_x: f32,
    pub socket_label_inset_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeFileBrowserSpacing {
    pub header_height: f32,
    pub list_row_height: f32,
    pub grid_cell_width: f32,
    pub grid_cell_height: f32,
    pub grid_gap: f32,
    pub padding_x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeCollectionSpacing {
    pub header_height: f32,
    pub item_gap: f32,
    pub padding: f32,
    pub preview_cell_width: f32,
    pub preview_cell_height: f32,
    pub preview_padding: f32,
    pub preview_caption_height: f32,
    pub minimum_preview_cell_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeSpacing {
    pub control: ThemeControlSpacing,
    pub box_container: ThemeBoxSpacing,
    pub property_row: ThemePropertyRowSpacing,
    pub panel: ThemePanelSpacing,
    pub menu: ThemeMenuSpacing,
    pub list: ThemeListSpacing,
    pub table: ThemeTableSpacing,
    pub editor: ThemeEditorSpacing,
    pub curve: ThemeCurveSpacing,
    pub hsv: ThemeHsvSpacing,
    pub color_ramp: ThemeColorRampSpacing,
    pub tooltip: ThemeTooltipSpacing,
    pub node_graph: ThemeNodeGraphSpacing,
    pub file_browser: ThemeFileBrowserSpacing,
    pub collection: ThemeCollectionSpacing,
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            control: ThemeControlSpacing {
                padding_x: 8.0,
                padding_y: 4.0,
                gap: 8.0,
                checkbox_label_gap: 6.0,
                error_outline_width: 2.0,
                slider_height: 32.0,
                slider_track_height: 4.0,
            },
            box_container: ThemeBoxSpacing {
                padding: 12.0,
                gap: 8.0,
            },
            property_row: ThemePropertyRowSpacing {
                label_width: 112.0,
                gap: 8.0,
                row_height: 24.0,
            },
            panel: ThemePanelSpacing {
                header_height: 24.0,
                content_padding: 8.0,
                section_gap: 8.0,
            },
            menu: ThemeMenuSpacing {
                item_height: 24.0,
                padding_x: 12.0,
                padding_y: 4.0,
                section_gap: 4.0,
                popup_padding: 4.0,
                pie_inner_inset: 19.0,
                pie_inner_radius: 12.0,
                pie_outer_radius: 88.0,
                pie_popup_diameter: 200.0,
                pie_outer_item_radius: 76.0,
                pie_nested_item_radius: 48.0,
                pie_item_max_width: 130.0,
            },
            list: ThemeListSpacing {
                row_height: 22.0,
                padding_x: 8.0,
                padding_y: 2.0,
                indent: 16.0,
                section_gap: 4.0,
            },
            table: ThemeTableSpacing {
                header_height: 26.0,
                row_height: 24.0,
                cell_padding_x: 8.0,
                cell_padding_y: 4.0,
            },
            editor: ThemeEditorSpacing {
                padding_x: 6.0,
                padding_y: 4.0,
                gutter_width: 24.0,
                toolbar_gap: 4.0,
                handle_radius: 4.0,
                grid_spacing: 16.0,
            },
            curve: ThemeCurveSpacing {
                selected_status_width: 176.0,
                interpolation_status_width: 142.0,
            },
            hsv: ThemeHsvSpacing {
                wheel_center_x: 84.0,
                wheel_center_y: 122.0,
                wheel_outer_radius: 68.0,
                wheel_inner_radius: 54.0,
                preview_right_inset: 48.0,
                preview_top_inset: 4.0,
                preview_width: 38.0,
                preview_height: 20.0,
                controls_x: 170.0,
                value_label_y: 27.0,
                value_track_y: 40.0,
                value_track_width_inset: 188.0,
                value_track_height: 14.0,
                alpha_label_y: 62.0,
                alpha_track_y: 75.0,
                alpha_track_width_inset: 238.0,
                alpha_field_y: 71.0,
                alpha_field_right_inset: 60.0,
                alpha_field_width: 50.0,
                field_cell_width: 78.0,
                field_gap: 8.0,
                field_first_row_y: 111.0,
                field_second_row_y: 143.0,
                field_hex_y: 177.0,
                field_hex_width_inset: 188.0,
                field_height: 22.0,
                field_text_inset_x: 5.0,
                field_text_inset_y: 3.0,
                help_y: 207.0,
            },
            color_ramp: ThemeColorRampSpacing {
                title_x: 8.0,
                title_y: 4.0,
                button_width: 48.0,
                button_step: 52.0,
                button_right_inset: 156.0,
                button_top_inset: 3.0,
                button_height: 20.0,
                header_hit_height: 26.0,
                button_label_x: 4.0,
                button_label_y: 3.0,
                track_x: 12.0,
                track_y: 38.0,
                track_width_inset: 24.0,
                track_height: 25.0,
                marker_line_extension: 8.0,
                marker_hit_extension: 14.0,
                track_hit_top_inset: 2.0,
                marker_handle_radius: 5.0,
                marker_handle_top_inset: 5.0,
                details_x: 8.0,
                details_y: 68.0,
                channel_x: 48.0,
                channel_y: 82.0,
                channel_step: 22.0,
                channel_width_inset: 58.0,
                channel_height: 14.0,
                channel_selection_hit_slop: 3.0,
                channel_click_hit_slop: 4.0,
                channel_label_hit_slop: 6.0,
                channel_label_x: 8.0,
                channel_label_y: 1.0,
                channel_value_right_inset: 36.0,
                editing_right_inset: 104.0,
                editing_bottom_inset: 17.0,
            },
            tooltip: ThemeTooltipSpacing {
                padding_x: 12.0,
                padding_y: 4.0,
                maximum_width: 280.0,
                viewport_margin: 4.0,
                anchor_gap: 5.0,
            },
            node_graph: ThemeNodeGraphSpacing {
                header_height: 28.0,
                header_hint_width: 210.0,
                body_padding: 8.0,
                socket_spacing: 22.0,
                socket_radius: 5.0,
                socket_hit_slop: 4.0,
                connection_tangent_min: 36.0,
                grid_spacing: 40.0,
                grid_spacing_min: 12.0,
                grid_spacing_max: 160.0,
                node_radius: 5.0,
                title_padding_x: 24.0,
                title_padding_y: 6.0,
                collapse_toggle_inset_x: 7.0,
                socket_label_inset_x: 11.0,
                socket_label_inset_y: 7.0,
            },
            file_browser: ThemeFileBrowserSpacing {
                header_height: 48.0,
                list_row_height: 22.0,
                // Matches FileBrowser's default tile width and current hitbox
                // geometry. The grid cells are 120 wide and 78 tall.
                grid_cell_width: 120.0,
                grid_cell_height: 78.0,
                grid_gap: 0.0,
                padding_x: 6.0,
            },
            collection: ThemeCollectionSpacing {
                header_height: 28.0,
                item_gap: 8.0,
                padding: 8.0,
                preview_cell_width: 112.0,
                preview_cell_height: 104.0,
                preview_padding: 5.0,
                preview_caption_height: 26.0,
                minimum_preview_cell_size: 40.0,
            },
        }
    }
}

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

    fn value(&self, property: UiStyleProperty) -> Option<UiStyleValue> {
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

/// Interpolation function applied by a [`UiTransitionSpec`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl UiEasing {
    fn sample(self, progress: f32) -> f32 {
        let progress = clamp(progress);
        match self {
            Self::Linear => progress,
            Self::EaseIn => progress * progress,
            Self::EaseOut => 1.0 - (1.0 - progress).powi(2),
            Self::EaseInOut if progress < 0.5 => 2.0 * progress * progress,
            Self::EaseInOut => 1.0 - (-2.0 * progress + 2.0).powi(2) * 0.5,
        }
    }
}

/// Duration and easing for one animatable style property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiTransitionSpec {
    pub duration: Duration,
    pub easing: UiEasing,
}

impl UiTransitionSpec {
    pub const fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: UiEasing::Linear,
        }
    }

    pub const fn easing(mut self, easing: UiEasing) -> Self {
        self.easing = easing;
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

    pub const fn transition(self, token: ThemeMotionToken) -> UiTransitionSpec {
        let easing = match token {
            ThemeMotionToken::Fast | ThemeMotionToken::MenuOpen => UiEasing::EaseOut,
            ThemeMotionToken::Standard | ThemeMotionToken::Slow | ThemeMotionToken::PanelExpand => {
                UiEasing::EaseInOut
            }
        };
        UiTransitionSpec::new(self.duration(token)).easing(easing)
    }
}

/// Per-property transition settings. Unsupported value pairs snap to target.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct UiTransitions(pub BTreeMap<UiStyleProperty, UiTransitionSpec>);

impl UiTransitions {
    pub fn set(&mut self, property: UiStyleProperty, spec: UiTransitionSpec) {
        self.0.insert(property, spec);
    }

    pub fn set_motion(
        &mut self,
        property: UiStyleProperty,
        theme: &Theme,
        token: ThemeMotionToken,
    ) {
        self.set(property, theme.motion.transition(token));
    }

    pub fn remove(&mut self, property: UiStyleProperty) -> Option<UiTransitionSpec> {
        self.0.remove(&property)
    }
}

/// Accessibility override that disables all style animation immediately.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiReducedMotion(pub bool);

#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiClasses(pub Vec<String>);

impl UiClasses {
    pub fn contains(&self, class: &str) -> bool {
        self.0.iter().any(|item| item == class)
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct UiRole(pub String);

impl UiRole {
    pub fn new(role: impl Into<String>) -> Self {
        Self(role.into())
    }
}

/// Bitset for state selectors. Multiple states can be active together.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    fn matches(
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

    fn specificity(&self) -> u16 {
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

#[derive(Debug, Clone, PartialEq)]
struct StyleTrack {
    from: UiStyleValue,
    to: UiStyleValue,
    elapsed: Duration,
    duration: Duration,
    easing: UiEasing,
}

/// Active transitions for one entity. Tracks are kept private; consumers read
/// the interpolated values from [`UiComputedStyle`].
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct UiStyleTransitionState {
    tracks: BTreeMap<UiStyleProperty, StyleTrack>,
}

impl UiStyleTransitionState {
    pub fn is_transitioning(&self, property: UiStyleProperty) -> bool {
        self.tracks.contains_key(&property)
    }

    pub fn active_properties(&self) -> impl Iterator<Item = UiStyleProperty> + '_ {
        self.tracks.keys().copied()
    }
}

/// Captured pre-cascade layout values. This lets state/theme changes remove a
/// previous declaration without permanently overwriting caller components.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiBaseLayoutNode(pub UiLayoutNode);

#[derive(Component, Debug, Clone, PartialEq)]
struct UiBaseBoxShadow(Option<BoxShadow>);

pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Theme>()
            .init_resource::<UiReducedMotion>()
            .init_resource::<UiStyleSheet>()
            .init_resource::<UiStyleResolutionCache>()
            .init_resource::<UiRoutedEvents>()
            .add_systems(
                Update,
                (advance_ui_style_transitions, resolve_ui_styles)
                    .chain()
                    .in_set(UiSystems::Style),
            )
            .add_systems(Update, resolve_ui_styles.in_set(UiSystems::StyleAfterInput))
            .add_systems(
                Update,
                converge_hover_layout
                    .run_if(has_pointer_hover_transition)
                    .in_set(UiSystems::LayoutAfterStyle),
            );
    }
}

#[derive(Resource, Debug, Default)]
struct UiStyleResolutionCache {
    input_stamp: Option<u64>,
    resolved_passes: u64,
    skipped_passes: u64,
}

fn has_pointer_hover_transition(events: Res<UiRoutedEvents>) -> bool {
    events.events.iter().any(|event| {
        matches!(
            event,
            UiRoutedEvent::PointerEnter { .. } | UiRoutedEvent::PointerLeave { .. }
        )
    })
}

fn converge_hover_layout(world: &mut World) {
    const MAX_HOVER_LAYOUT_PASSES: usize = 3;
    for _ in 0..MAX_HOVER_LAYOUT_PASSES {
        compute_layout(world);
        if !refresh_ui_hover(world) {
            return;
        }
        resolve_ui_styles(world);
    }

    // A state rule can move the hit target repeatedly. Bound that work, then
    // leave hover aligned with the final hitboxes; the next frame can continue
    // if the rule set is oscillating.
    compute_layout(world);
    refresh_ui_hover(world);
}

/// Resolve theme defaults, matching rules, inherited text values, and inline
/// values. Layout declarations are applied before the following layout phase.
pub fn resolve_ui_styles(world: &mut World) {
    world.init_resource::<UiStyleResolutionCache>();
    let input_stamp = ui_style_input_stamp(world);
    if world.resource::<UiStyleResolutionCache>().input_stamp == Some(input_stamp) {
        world
            .resource_mut::<UiStyleResolutionCache>()
            .skipped_passes += 1;
        return;
    }

    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let sheet = world
        .get_resource::<UiStyleSheet>()
        .cloned()
        .unwrap_or_default();
    let mut query = world.query_filtered::<Entity, With<UiNode>>();
    let mut nodes: Vec<Entity> = query.iter(world).collect();
    nodes.sort_by_key(|entity| {
        (
            hierarchy_depth(world, *entity),
            hierarchy_order(world, *entity),
        )
    });

    for entity in nodes {
        let role = world.get::<UiRole>(entity).cloned();
        let classes = world.get::<UiClasses>(entity).cloned();
        let mut state = world
            .get::<UiStyleState>(entity)
            .copied()
            .unwrap_or_default();
        if world
            .get::<UiEnabled>(entity)
            .is_some_and(|enabled| !enabled.0)
        {
            state = state.union(UiStyleState::DISABLED);
        }
        if let Some(focus) = world.get_resource::<UiFocus>() {
            for window_state in focus.windows.values() {
                if window_state.hovered == Some(entity) {
                    state = state.union(UiStyleState::HOVERED);
                }
                if window_state.captured == Some(entity) {
                    state = state.union(UiStyleState::PRESSED);
                }
                if window_state.focused == Some(entity) {
                    state = state.union(UiStyleState::FOCUSED);
                }
            }
        }
        let inline = world
            .get::<UiInlineStyle>(entity)
            .cloned()
            .unwrap_or_default();
        let parent_style = world
            .get::<UiParent>(entity)
            .and_then(|parent| world.get::<UiComputedStyle>(parent.0))
            .cloned();
        let mut target = resolve_style(
            &theme,
            &sheet,
            role.as_ref(),
            classes.as_ref(),
            state,
            parent_style.as_ref(),
            &inline,
        );
        let base_layout = capture_base_layout_node(world, entity);
        add_base_layout_values(&mut target, base_layout);

        let current = world
            .get::<UiComputedStyle>(entity)
            .cloned()
            .unwrap_or_default();
        let previous_transitions = world
            .get::<UiStyleTransitionState>(entity)
            .cloned()
            .unwrap_or_default();
        let transitions = world
            .get::<UiTransitions>(entity)
            .cloned()
            .unwrap_or_default();
        let reduced_motion = world
            .get_resource::<UiReducedMotion>()
            .is_some_and(|setting| setting.0)
            || !world.contains_resource::<Time>()
            || nonnegative(theme.motion_scale) == 0.0;
        let motion_scale = nonnegative(theme.motion_scale);
        let (resolved, next_transitions) = transition_style(
            &current,
            &target,
            &transitions,
            &previous_transitions,
            reduced_motion,
            motion_scale,
        );
        if current != resolved {
            apply_box_shadow_style(world, entity, &resolved, &theme);
            world.entity_mut(entity).insert(resolved.clone());
        }
        if previous_transitions != next_transitions {
            if next_transitions.tracks.is_empty() {
                world.entity_mut(entity).remove::<UiStyleTransitionState>();
            } else {
                world.entity_mut(entity).insert(next_transitions);
            }
        }
        apply_layout_style(world, entity, &resolved);
    }

    let settled_stamp = ui_style_input_stamp(world);
    let mut cache = world.resource_mut::<UiStyleResolutionCache>();
    cache.input_stamp = Some(settled_stamp);
    cache.resolved_passes = cache.resolved_passes.saturating_add(1);
}

fn ui_style_input_stamp(world: &World) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_style_resource_ticks::<Theme>(world, &mut hasher);
    hash_style_resource_ticks::<UiStyleSheet>(world, &mut hasher);
    hash_style_resource_ticks::<UiReducedMotion>(world, &mut hasher);
    world.contains_resource::<Time>().hash(&mut hasher);

    if let Some(focus) = world.get_resource::<UiFocus>() {
        for (window, state) in &focus.windows {
            window.hash(&mut hasher);
            state.focused.hash(&mut hasher);
            state.hovered.hash(&mut hasher);
            state.captured.hash(&mut hasher);
        }
    }

    macro_rules! hash_style_component {
        ($entity:ident, $component:ty) => {
            if let Some(ticks) = $entity.get_change_ticks::<$component>() {
                true.hash(&mut hasher);
                ticks.added.get().hash(&mut hasher);
                ticks.changed.get().hash(&mut hasher);
            } else {
                false.hash(&mut hasher);
            }
        };
    }

    for entity in world.iter_entities() {
        if !entity.contains::<UiNode>()
            && !entity.contains::<UiParent>()
            && !entity.contains::<UiComputedStyle>()
            && !entity.contains::<UiBaseLayoutNode>()
            && !entity.contains::<UiLayoutNode>()
        {
            continue;
        }
        entity.id().hash(&mut hasher);
        hash_style_component!(entity, UiNode);
        hash_style_component!(entity, UiParent);
        hash_style_component!(entity, UiRole);
        hash_style_component!(entity, UiClasses);
        hash_style_component!(entity, UiStyleState);
        hash_style_component!(entity, UiEnabled);
        hash_style_component!(entity, UiInlineStyle);
        hash_style_component!(entity, UiComputedStyle);
        hash_style_component!(entity, UiTransitions);
        hash_style_component!(entity, UiStyleTransitionState);
        hash_style_component!(entity, UiBaseLayoutNode);
        hash_style_component!(entity, UiLayoutNode);
    }
    hasher.finish()
}

fn hash_style_resource_ticks<R: Resource>(world: &World, hasher: &mut impl Hasher) {
    if let Some(ticks) = world.get_resource_change_ticks::<R>() {
        true.hash(hasher);
        ticks.added.get().hash(hasher);
        ticks.changed.get().hash(hasher);
    } else {
        false.hash(hasher);
    }
}

/// Advance active transitions once per frame. Resolution runs in
/// `StyleAfterInput` too, but only this function consumes Bevy's frame delta.
pub fn advance_ui_style_transitions(world: &mut World) {
    let delta = world
        .get_resource::<Time>()
        .map(Time::delta)
        .unwrap_or_default();
    if delta.is_zero() {
        return;
    }
    let mut query = world.query_filtered::<Entity, With<UiStyleTransitionState>>();
    let entities: Vec<_> = query.iter(world).collect();
    for entity in entities {
        let Some(mut state) = world.get::<UiStyleTransitionState>(entity).cloned() else {
            continue;
        };
        let mut changed = false;
        for track in state.tracks.values_mut() {
            if track.elapsed < track.duration {
                let elapsed = track.elapsed.saturating_add(delta).min(track.duration);
                changed |= elapsed != track.elapsed;
                track.elapsed = elapsed;
            }
        }
        if changed {
            world.entity_mut(entity).insert(state);
        }
    }
}

fn transition_style(
    current: &UiComputedStyle,
    target: &UiComputedStyle,
    transitions: &UiTransitions,
    previous: &UiStyleTransitionState,
    reduced_motion: bool,
    motion_scale: f32,
) -> (UiComputedStyle, UiStyleTransitionState) {
    let mut result = UiComputedStyle::default();
    let mut state = previous.clone();
    state
        .tracks
        .retain(|property, _| target.properties.contains_key(property));

    for (&property, target_value) in &target.properties {
        let target_value = target_value.clone();
        let current_value = current.get(property);
        let spec = transitions.0.get(&property).copied();
        let duration = spec.map_or(Duration::ZERO, |spec| {
            scaled_duration(spec.duration, motion_scale)
        });
        let eased = if reduced_motion { None } else { spec };
        let mut displayed = target_value.clone();

        if let Some(spec) = eased.filter(|_| !duration.is_zero()) {
            if let Some(track) = state.tracks.get(&property).cloned()
                && track.to == target_value
                && track.duration == duration
                && track.easing == spec.easing
            {
                let progress = (track.elapsed.as_secs_f64() / track.duration.as_secs_f64()) as f32;
                if progress >= 1.0 {
                    state.tracks.remove(&property);
                } else if let Some(interpolated) =
                    interpolate_style_value(track.from, track.to, track.easing.sample(progress))
                {
                    displayed = interpolated;
                } else {
                    state.tracks.remove(&property);
                }
            } else {
                state.tracks.remove(&property);
                if let Some(from) = current_value
                    && from != target_value
                    && interpolate_style_value(from.clone(), target_value.clone(), 0.0).is_some()
                {
                    state.tracks.insert(
                        property,
                        StyleTrack {
                            from: from.clone(),
                            to: target_value.clone(),
                            elapsed: Duration::ZERO,
                            duration,
                            easing: spec.easing,
                        },
                    );
                    displayed = from;
                }
            }
        } else {
            state.tracks.remove(&property);
        }

        result.properties.insert(property, displayed);
    }

    (result, state)
}

fn interpolate_style_value(
    from: UiStyleValue,
    to: UiStyleValue,
    progress: f32,
) -> Option<UiStyleValue> {
    let progress = clamp(progress);
    Some(match (from, to) {
        (UiStyleValue::Color(from), UiStyleValue::Color(to)) => {
            UiStyleValue::Color(from.mix(to, progress))
        }
        (UiStyleValue::Scalar(from), UiStyleValue::Scalar(to)) => {
            UiStyleValue::Scalar(mix_channel(from, to, progress))
        }
        (UiStyleValue::BoxShadow(from), UiStyleValue::BoxShadow(to)) => {
            UiStyleValue::BoxShadow(from.interpolate(&to, progress)?)
        }
        (UiStyleValue::Length(UiLength::Px(from)), UiStyleValue::Length(UiLength::Px(to))) => {
            UiStyleValue::Length(UiLength::Px(mix_channel(from, to, progress)))
        }
        (
            UiStyleValue::Length(UiLength::Percent(from)),
            UiStyleValue::Length(UiLength::Percent(to)),
        ) => UiStyleValue::Length(UiLength::Percent(mix_channel(from, to, progress))),
        (UiStyleValue::Length(UiLength::Fill(from)), UiStyleValue::Length(UiLength::Fill(to))) => {
            UiStyleValue::Length(UiLength::Fill(mix_channel(from, to, progress)))
        }
        (UiStyleValue::Length(UiLength::Auto), UiStyleValue::Length(UiLength::Auto)) => {
            UiStyleValue::Length(UiLength::Auto)
        }
        (UiStyleValue::Edges(from), UiStyleValue::Edges(to)) => UiStyleValue::Edges(UiEdges {
            left: mix_channel(from.left, to.left, progress),
            top: mix_channel(from.top, to.top, progress),
            right: mix_channel(from.right, to.right, progress),
            bottom: mix_channel(from.bottom, to.bottom, progress),
        }),
        _ => return None,
    })
}

fn scaled_duration(duration: Duration, scale: f32) -> Duration {
    if !scale.is_finite() || scale <= 0.0 || duration.is_zero() {
        return Duration::ZERO;
    }
    let seconds = (duration.as_secs_f64() * f64::from(scale)).min(Duration::MAX.as_secs_f64());
    Duration::from_secs_f64(seconds)
}

pub fn resolve_style(
    theme: &Theme,
    sheet: &UiStyleSheet,
    role: Option<&UiRole>,
    classes: Option<&UiClasses>,
    state: UiStyleState,
    parent: Option<&UiComputedStyle>,
    inline: &UiInlineStyle,
) -> UiComputedStyle {
    let mut result = UiComputedStyle::default();
    for property in [
        UiStyleProperty::Foreground,
        UiStyleProperty::MutedForeground,
        UiStyleProperty::SelectionColor,
        UiStyleProperty::IconColor,
        UiStyleProperty::MutedIconColor,
        UiStyleProperty::FontSize,
        UiStyleProperty::LineHeight,
    ] {
        if let Some(value) = parent.and_then(|style| style.get(property)) {
            result.properties.insert(property, value);
        }
    }
    for property in [
        UiStyleProperty::Background,
        UiStyleProperty::Foreground,
        UiStyleProperty::MutedForeground,
        UiStyleProperty::BorderColor,
        UiStyleProperty::AccentColor,
        UiStyleProperty::SelectionColor,
        UiStyleProperty::FocusColor,
        UiStyleProperty::DisabledColor,
        UiStyleProperty::ErrorColor,
        UiStyleProperty::IconColor,
        UiStyleProperty::MutedIconColor,
        UiStyleProperty::DisabledIconColor,
        UiStyleProperty::BorderWidth,
        UiStyleProperty::CornerRadius,
        UiStyleProperty::Opacity,
        UiStyleProperty::FontSize,
        UiStyleProperty::LineHeight,
    ] {
        if let Some(value) = theme.value(property)
            && (!matches!(
                property,
                UiStyleProperty::Foreground
                    | UiStyleProperty::MutedForeground
                    | UiStyleProperty::SelectionColor
                    | UiStyleProperty::IconColor
                    | UiStyleProperty::MutedIconColor
                    | UiStyleProperty::FontSize
                    | UiStyleProperty::LineHeight
            ) || !result.properties.contains_key(&property))
        {
            result.properties.insert(property, value);
        }
    }
    if let Some(role) = role {
        result.properties.extend(
            role_style_values(theme, &role.0)
                .into_iter()
                .map(|(key, value)| (key, resolve_theme_color_token(theme, value))),
        );
    }
    if state.contains(UiStyleState::DISABLED) {
        result.properties.insert(
            UiStyleProperty::Background,
            UiStyleValue::Color(theme.disabled_surface),
        );
        result.properties.insert(
            UiStyleProperty::Foreground,
            UiStyleValue::Color(theme.disabled),
        );
        result.properties.insert(
            UiStyleProperty::BorderColor,
            UiStyleValue::Color(theme.disabled_border),
        );
        result.properties.insert(
            UiStyleProperty::Opacity,
            UiStyleValue::Scalar(nonnegative(theme.disabled_opacity).min(1.0)),
        );
        result.properties.insert(
            UiStyleProperty::DisabledIconColor,
            UiStyleValue::Color(theme.disabled),
        );
    } else {
        let selected = state.contains(UiStyleState::SELECTED);
        let background = if state.contains(UiStyleState::PRESSED) {
            Some(if selected {
                theme.selection.mix(theme.surface_pressed, 0.3)
            } else {
                theme.surface_pressed
            })
        } else if state.contains(UiStyleState::HOVERED) {
            Some(if selected {
                theme.selection.mix(theme.surface_hover, 0.28)
            } else {
                theme.surface_hover
            })
        } else if selected {
            Some(theme.selection)
        } else {
            None
        };
        if let Some(background) = background {
            result
                .properties
                .insert(UiStyleProperty::Background, UiStyleValue::Color(background));
        }
        if state.contains(UiStyleState::FOCUSED) || state.contains(UiStyleState::ACTIVE) {
            result.properties.insert(
                UiStyleProperty::BorderColor,
                UiStyleValue::Color(theme.focus),
            );
        }
        if state.contains(UiStyleState::CHECKED) {
            result.properties.insert(
                UiStyleProperty::AccentColor,
                UiStyleValue::Color(theme.accent),
            );
        }
        if state.contains(UiStyleState::ERROR) {
            result.properties.insert(
                UiStyleProperty::BorderColor,
                UiStyleValue::Color(theme.error),
            );
        }
    }
    let mut matches: Vec<(u16, i16, usize, &UiStyleRule)> = sheet
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.selector.matches(role, classes, state))
        .map(|(index, rule)| (rule.selector.specificity(), rule.priority, index, rule))
        .collect();
    matches.sort_by_key(|(specificity, priority, index, _)| (*specificity, *priority, *index));
    for (_, _, _, rule) in matches {
        result.properties.extend(
            rule.declaration
                .0
                .iter()
                .map(|(key, value)| (*key, resolve_theme_color_token(theme, value.clone()))),
        );
    }
    result.properties.extend(
        inline
            .0
            .iter()
            .map(|(key, value)| (*key, resolve_theme_color_token(theme, value.clone()))),
    );
    result
}

fn resolve_theme_color_token(theme: &Theme, value: UiStyleValue) -> UiStyleValue {
    match value {
        UiStyleValue::ColorToken(token) => UiStyleValue::Color(theme.color(token)),
        UiStyleValue::BoxShadow(shadow) => {
            UiStyleValue::BoxShadow(shadow.resolve_theme_color(theme))
        }
        value => value,
    }
}

fn role_style_values(theme: &Theme, role: &str) -> BTreeMap<UiStyleProperty, UiStyleValue> {
    use UiStyleProperty::*;
    let mut values = BTreeMap::new();
    let uniform = |x, y| UiEdges {
        left: x,
        top: y,
        right: x,
        bottom: y,
    };
    match role {
        "menu-popup" | "dropdown-popup" | "context-menu-popup" | "tooltip-popup"
        | "pie-menu-popup" => {
            values.insert(
                BoxShadow,
                UiStyleValue::BoxShadow(theme.effects.popup_shadow.clone()),
            );
        }
        "layout-row" | "layout-column" | "layout-grid" | "layout-flex" | "layout-inline"
        | "scroll" => {
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.control.gap));
        }
        "menu-bar" => {
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.menu.section_gap));
        }
        "box" => {
            values.insert(Background, UiStyleValue::Color(theme.surface));
            values.insert(BorderColor, UiStyleValue::Color(theme.border));
            values.insert(BorderWidth, UiStyleValue::Scalar(1.0));
            values.insert(CornerRadius, UiStyleValue::Scalar(theme.corner_radius));
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.box_container.padding)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.box_container.gap));
        }
        "property-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.property_row.row_height)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.property_row.gap));
        }
        "property-label" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.property_row.label_width)),
            );
        }
        "button" | "checkbox" | "radio-group" | "dropdown" | "text-field" | "numeric-field" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.control_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.control.gap));
        }
        "slider" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.control.slider_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.control.gap));
        }
        "text-area" => {
            values.insert(MinHeight, UiStyleValue::Scalar(theme.control_height));
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
        }
        "panel" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.panel.content_padding)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.panel.section_gap));
        }
        "panel-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.panel.header_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.control.padding_x,
                    theme.spacing.control.padding_y,
                )),
            );
        }
        "menu" | "popup" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.menu.popup_padding)),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.menu.section_gap));
        }
        "menu-trigger" | "popover-trigger" | "context-menu-trigger" | "pie-menu-trigger" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.control_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.menu.padding_x,
                    theme.spacing.menu.padding_y,
                )),
            );
        }
        "menu-item" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.menu.item_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.menu.padding_x,
                    theme.spacing.menu.padding_y,
                )),
            );
        }
        "list" => {
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.list.section_gap));
        }
        "list-row" | "tree-row" | "collection-list-row" | "collection-tree-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.list.row_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.list.padding_x,
                    theme.spacing.list.padding_y,
                )),
            );
        }
        "preview-grid-item" | "collection-preview-item" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.collection.preview_cell_width)),
            );
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.collection.preview_cell_height)),
            );
        }
        "file-browser" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(theme.spacing.file_browser.padding_x, 0.0)),
            );
        }
        "file-browser-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.header_height)),
            );
        }
        "file-browser-list-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.list_row_height)),
            );
        }
        "file-browser-grid-item" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.grid_cell_width)),
            );
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.file_browser.grid_cell_height)),
            );
        }
        "collection" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(UiEdges::all(theme.spacing.collection.padding)),
            );
        }
        "collection-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.collection.header_height)),
            );
        }
        "table-header" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.table.header_height)),
            );
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.table.cell_padding_x,
                    theme.spacing.table.cell_padding_y,
                )),
            );
        }
        "table-row" => {
            values.insert(
                Height,
                UiStyleValue::Length(UiLength::Px(theme.spacing.table.row_height)),
            );
        }
        "table-cell" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.table.cell_padding_x,
                    theme.spacing.table.cell_padding_y,
                )),
            );
        }
        "editor" | "color-editor" | "curve-editor" | "ramp-editor" | "node-graph" | "canvas" => {
            values.insert(
                Padding,
                UiStyleValue::Edges(uniform(
                    theme.spacing.editor.padding_x,
                    theme.spacing.editor.padding_y,
                )),
            );
            values.insert(Gap, UiStyleValue::Scalar(theme.spacing.editor.toolbar_gap));
        }
        "editor-gutter" => {
            values.insert(
                Width,
                UiStyleValue::Length(UiLength::Px(theme.spacing.editor.gutter_width)),
            );
        }
        _ => {}
    }
    values
}

fn capture_base_layout_node(world: &mut World, entity: Entity) -> UiLayoutNode {
    if let Some(base) = world.get::<UiBaseLayoutNode>(entity) {
        return base.0;
    }
    let base = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default();
    world.entity_mut(entity).insert(UiBaseLayoutNode(base));
    base
}

fn add_base_layout_values(style: &mut UiComputedStyle, layout: UiLayoutNode) {
    use UiStyleProperty::*;
    let properties = &mut style.properties;
    properties
        .entry(Width)
        .or_insert(UiStyleValue::Length(layout.size.width));
    properties
        .entry(Height)
        .or_insert(UiStyleValue::Length(layout.size.height));
    for (property, value) in [
        (MinWidth, layout.min_size[0]),
        (MinHeight, layout.min_size[1]),
        (MaxWidth, layout.max_size[0]),
        (MaxHeight, layout.max_size[1]),
    ] {
        if let Some(value) = value {
            properties
                .entry(property)
                .or_insert(UiStyleValue::Scalar(value));
        }
    }
    properties
        .entry(Padding)
        .or_insert(UiStyleValue::Edges(layout.padding));
    properties
        .entry(Margin)
        .or_insert(UiStyleValue::Edges(layout.margin));
    properties
        .entry(Gap)
        .or_insert(UiStyleValue::Scalar(layout.gap));
    properties
        .entry(LayoutMode)
        .or_insert(UiStyleValue::LayoutMode(layout.mode));
    properties
        .entry(AlignItems)
        .or_insert(UiStyleValue::Align(layout.align_items));
    properties
        .entry(JustifyContent)
        .or_insert(UiStyleValue::Justify(layout.justify_content));
    properties
        .entry(ClipContents)
        .or_insert(UiStyleValue::Bool(layout.clip_contents));
    properties
        .entry(ZIndex)
        .or_insert(UiStyleValue::Integer(layout.z_index));
}

fn apply_layout_style(world: &mut World, entity: Entity, computed: &UiComputedStyle) {
    let base = capture_base_layout_node(world, entity);
    let mut layout = base;
    apply_layout_property(&mut layout, computed, UiStyleProperty::Width);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Height);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MinWidth);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MinHeight);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MaxWidth);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MaxHeight);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Padding);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Margin);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Gap);
    apply_layout_property(&mut layout, computed, UiStyleProperty::LayoutMode);
    apply_layout_property(&mut layout, computed, UiStyleProperty::AlignItems);
    apply_layout_property(&mut layout, computed, UiStyleProperty::JustifyContent);
    apply_layout_property(&mut layout, computed, UiStyleProperty::ClipContents);
    apply_layout_property(&mut layout, computed, UiStyleProperty::ZIndex);
    if world.get::<UiLayoutNode>(entity).copied() != Some(layout) {
        world.entity_mut(entity).insert(layout);
    }
}

fn apply_box_shadow_style(
    world: &mut World,
    entity: Entity,
    computed: &UiComputedStyle,
    theme: &Theme,
) {
    if let Some(shadow) = computed.box_shadow() {
        if world.get::<UiBaseBoxShadow>(entity).is_none() {
            let base = world.get::<BoxShadow>(entity).cloned();
            world.entity_mut(entity).insert(UiBaseBoxShadow(base));
        }
        let native = shadow.to_native(theme);
        if world.get::<BoxShadow>(entity) != Some(&native) {
            world.entity_mut(entity).insert(native);
        }
    } else if let Some(base) = world.get::<UiBaseBoxShadow>(entity).cloned() {
        if let Some(base) = base.0 {
            world.entity_mut(entity).insert(base);
        } else {
            world.entity_mut(entity).remove::<BoxShadow>();
        }
        world.entity_mut(entity).remove::<UiBaseBoxShadow>();
    }
}

fn apply_layout_property(
    layout: &mut UiLayoutNode,
    style: &UiComputedStyle,
    property: UiStyleProperty,
) {
    use UiStyleProperty::*;
    match (property, style.get(property)) {
        (Width, Some(UiStyleValue::Length(value))) => layout.size.width = value,
        (Height, Some(UiStyleValue::Length(value))) => layout.size.height = value,
        (MinWidth, Some(UiStyleValue::Scalar(value))) => {
            layout.min_size[0] = Some(nonnegative(value))
        }
        (MinHeight, Some(UiStyleValue::Scalar(value))) => {
            layout.min_size[1] = Some(nonnegative(value))
        }
        (MaxWidth, Some(UiStyleValue::Scalar(value))) => {
            layout.max_size[0] = Some(nonnegative(value))
        }
        (MaxHeight, Some(UiStyleValue::Scalar(value))) => {
            layout.max_size[1] = Some(nonnegative(value))
        }
        (Padding, Some(UiStyleValue::Edges(value))) => layout.padding = value,
        (Margin, Some(UiStyleValue::Edges(value))) => layout.margin = value,
        (Gap, Some(UiStyleValue::Scalar(value))) => layout.gap = nonnegative(value),
        (LayoutMode, Some(UiStyleValue::LayoutMode(value))) => layout.mode = value,
        (AlignItems, Some(UiStyleValue::Align(value))) => layout.align_items = value,
        (JustifyContent, Some(UiStyleValue::Justify(value))) => layout.justify_content = value,
        (ClipContents, Some(UiStyleValue::Bool(value))) => layout.clip_contents = value,
        (ZIndex, Some(UiStyleValue::Integer(value))) => layout.z_index = value,
        _ => {}
    }
}

fn hierarchy_depth(world: &World, entity: Entity) -> usize {
    let mut depth = 0;
    let mut current = Some(entity);
    let mut seen = Vec::new();
    while let Some(at) = current {
        if seen.contains(&at) {
            break;
        }
        seen.push(at);
        let Some(parent) = world.get::<UiParent>(at) else {
            break;
        };
        current = Some(parent.0);
        depth += 1;
    }
    depth
}

fn hierarchy_order(world: &World, entity: Entity) -> u64 {
    let mut path = Vec::new();
    let mut current = Some(entity);
    while let Some(at) = current {
        path.push(at);
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    path.reverse();
    path.into_iter().fold(0u64, |order, item| {
        order.wrapping_mul(65_537).wrapping_add(item.to_bits())
    })
}

fn nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn clamp(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn mix_channel(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::IntoScheduleConfigs;
    use bui_core::{
        CorePlugin, UiEnabled, UiFocus, UiFocusable, UiInputEvent, UiInputQueue, UiNode, UiSystems,
        UiWheelUnit, set_parent,
    };
    use bui_layout::{
        LayoutPlugin, Rect, UiAlign, UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot,
        UiLength, UiScroll, UiSize,
    };

    fn style_app() -> App {
        let mut app = App::new();
        app.add_plugins((CorePlugin, StylePlugin))
            .init_resource::<Time>();
        app
    }

    fn set_transition(transitions: &mut UiTransitions, property: UiStyleProperty, millis: u64) {
        transitions.set(
            property,
            UiTransitionSpec::new(Duration::from_millis(millis)),
        );
    }

    #[derive(Resource, Default)]
    struct ChangedStyleEntities(Vec<Entity>);

    #[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct StyleChangeCapture;

    fn collect_changed_styles(
        query: Query<Entity, Changed<UiComputedStyle>>,
        mut changes: ResMut<ChangedStyleEntities>,
    ) {
        changes.0.clear();
        changes.0.extend(query.iter());
    }

    fn advance(app: &mut App, millis: u64) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(millis));
        app.update();
    }

    fn all_spacing_values(spacing: ThemeSpacing) -> Vec<f32> {
        let control = spacing.control;
        let panel = spacing.panel;
        let menu = spacing.menu;
        let list = spacing.list;
        let table = spacing.table;
        let editor = spacing.editor;
        let curve = spacing.curve;
        let hsv = spacing.hsv;
        let color_ramp = spacing.color_ramp;
        let tooltip = spacing.tooltip;
        let box_container = spacing.box_container;
        let property_row = spacing.property_row;
        let node_graph = spacing.node_graph;
        let file_browser = spacing.file_browser;
        let collection = spacing.collection;
        vec![
            control.padding_x,
            control.padding_y,
            control.gap,
            control.checkbox_label_gap,
            control.error_outline_width,
            control.slider_height,
            control.slider_track_height,
            panel.header_height,
            panel.content_padding,
            panel.section_gap,
            menu.item_height,
            menu.padding_x,
            menu.padding_y,
            menu.section_gap,
            menu.popup_padding,
            menu.pie_inner_inset,
            menu.pie_inner_radius,
            menu.pie_outer_radius,
            menu.pie_popup_diameter,
            menu.pie_outer_item_radius,
            menu.pie_nested_item_radius,
            menu.pie_item_max_width,
            list.row_height,
            list.padding_x,
            list.padding_y,
            list.indent,
            list.section_gap,
            table.header_height,
            table.row_height,
            table.cell_padding_x,
            table.cell_padding_y,
            editor.padding_x,
            editor.padding_y,
            editor.gutter_width,
            editor.toolbar_gap,
            editor.handle_radius,
            editor.grid_spacing,
            curve.selected_status_width,
            curve.interpolation_status_width,
            hsv.wheel_center_x,
            hsv.wheel_center_y,
            hsv.wheel_outer_radius,
            hsv.wheel_inner_radius,
            hsv.preview_right_inset,
            hsv.preview_top_inset,
            hsv.preview_width,
            hsv.preview_height,
            hsv.controls_x,
            hsv.value_label_y,
            hsv.value_track_y,
            hsv.value_track_width_inset,
            hsv.value_track_height,
            hsv.alpha_label_y,
            hsv.alpha_track_y,
            hsv.alpha_track_width_inset,
            hsv.alpha_field_y,
            hsv.alpha_field_right_inset,
            hsv.alpha_field_width,
            hsv.field_cell_width,
            hsv.field_gap,
            hsv.field_first_row_y,
            hsv.field_second_row_y,
            hsv.field_hex_y,
            hsv.field_hex_width_inset,
            hsv.field_height,
            hsv.field_text_inset_x,
            hsv.field_text_inset_y,
            hsv.help_y,
            color_ramp.title_x,
            color_ramp.title_y,
            color_ramp.button_width,
            color_ramp.button_step,
            color_ramp.button_right_inset,
            color_ramp.button_top_inset,
            color_ramp.button_height,
            color_ramp.header_hit_height,
            color_ramp.button_label_x,
            color_ramp.button_label_y,
            color_ramp.track_x,
            color_ramp.track_y,
            color_ramp.track_width_inset,
            color_ramp.track_height,
            color_ramp.marker_line_extension,
            color_ramp.marker_hit_extension,
            color_ramp.track_hit_top_inset,
            color_ramp.marker_handle_radius,
            color_ramp.marker_handle_top_inset,
            color_ramp.details_x,
            color_ramp.details_y,
            color_ramp.channel_x,
            color_ramp.channel_y,
            color_ramp.channel_step,
            color_ramp.channel_width_inset,
            color_ramp.channel_height,
            color_ramp.channel_selection_hit_slop,
            color_ramp.channel_click_hit_slop,
            color_ramp.channel_label_hit_slop,
            color_ramp.channel_label_x,
            color_ramp.channel_label_y,
            color_ramp.channel_value_right_inset,
            color_ramp.editing_right_inset,
            color_ramp.editing_bottom_inset,
            tooltip.padding_x,
            tooltip.padding_y,
            tooltip.maximum_width,
            tooltip.viewport_margin,
            tooltip.anchor_gap,
            box_container.padding,
            box_container.gap,
            property_row.label_width,
            property_row.gap,
            property_row.row_height,
            node_graph.header_height,
            node_graph.header_hint_width,
            node_graph.body_padding,
            node_graph.socket_spacing,
            node_graph.socket_radius,
            node_graph.socket_hit_slop,
            node_graph.connection_tangent_min,
            node_graph.grid_spacing,
            node_graph.grid_spacing_min,
            node_graph.grid_spacing_max,
            node_graph.node_radius,
            node_graph.title_padding_x,
            node_graph.title_padding_y,
            node_graph.collapse_toggle_inset_x,
            node_graph.socket_label_inset_x,
            node_graph.socket_label_inset_y,
            file_browser.header_height,
            file_browser.list_row_height,
            file_browser.grid_cell_width,
            file_browser.grid_cell_height,
            file_browser.grid_gap,
            file_browser.padding_x,
            collection.header_height,
            collection.item_gap,
            collection.padding,
            collection.preview_cell_width,
            collection.preview_cell_height,
            collection.preview_padding,
            collection.preview_caption_height,
            collection.minimum_preview_cell_size,
        ]
    }

    #[test]
    fn theme_switch_updates_complete_palette() {
        let mut theme = Theme::default();
        theme.switch(ThemeKind::Light);
        assert_eq!(theme.kind, ThemeKind::Light);
        assert_ne!(theme.background, Theme::new(ThemeKind::Dark).background);
        assert_eq!(theme.spacing_unit, 4.0);
        assert_eq!(theme.control_height, 24.0);
    }

    #[test]
    fn typed_motion_preset_drives_editor_transition_and_reduced_motion() {
        let dark_theme = Theme::new(ThemeKind::Dark);
        assert_eq!(
            dark_theme.motion.duration(ThemeMotionToken::Fast),
            Duration::from_millis(80)
        );
        assert_eq!(
            dark_theme.motion.duration(ThemeMotionToken::MenuOpen),
            Duration::from_millis(120)
        );
        assert_eq!(
            dark_theme
                .motion
                .transition(ThemeMotionToken::PanelExpand)
                .easing,
            UiEasing::EaseInOut
        );

        let mut app = style_app();
        let mut transitions = UiTransitions::default();
        transitions.set_motion(
            UiStyleProperty::Background,
            &dark_theme,
            ThemeMotionToken::PanelExpand,
        );
        let editor = app
            .world_mut()
            .spawn((UiNode, UiRole::new("curve-editor"), transitions))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        advance(&mut app, 80);
        let halfway = app
            .world()
            .get::<UiComputedStyle>(editor)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        let light_surface = Theme::new(ThemeKind::Light).surface;
        assert_eq!(
            halfway,
            dark_theme.surface.mix(light_surface, 0.5),
            "editor style follows the typed panel-duration preset"
        );
        assert!(
            app.world()
                .get::<UiStyleTransitionState>(editor)
                .is_some_and(|state| state.is_transitioning(UiStyleProperty::Background))
        );

        app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Dark);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(editor)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(dark_theme.surface)
        );
        assert!(app.world().get::<UiStyleTransitionState>(editor).is_none());
    }

    #[test]
    fn both_themes_provide_finite_semantic_tokens_and_editor_spacing() {
        for kind in [ThemeKind::Dark, ThemeKind::Light] {
            let theme = Theme::new(kind);
            for color in ThemeColorToken::ALL {
                assert!(theme.color(color).is_finite(), "{kind:?} {color:?}");
            }
            assert_eq!(theme.spacing.control.checkbox_label_gap, 6.0);
            assert_eq!(theme.spacing.control.slider_height, 32.0);
            assert_eq!(theme.spacing.control.slider_track_height, 4.0);
            assert_eq!(theme.spacing.box_container.padding, 12.0);
            assert_eq!(theme.spacing.box_container.gap, 8.0);
            assert_eq!(theme.spacing.property_row.label_width, 112.0);
            assert_eq!(theme.spacing.property_row.gap, 8.0);
            assert_eq!(theme.spacing.property_row.row_height, 24.0);
            assert_eq!(theme.spacing.menu.item_height, 24.0);
            assert_eq!(theme.spacing.list.row_height, 22.0);
            assert_eq!(theme.spacing.table.header_height, 26.0);
            assert_eq!(theme.spacing.table.row_height, 24.0);
            assert!(theme.spacing.editor.gutter_width > 0.0);
            assert!(theme.spacing.editor.grid_spacing > 0.0);
            assert_eq!(theme.spacing.node_graph.header_height, 28.0);
            assert_eq!(theme.spacing.node_graph.header_hint_width, 210.0);
            assert_eq!(theme.spacing.node_graph.body_padding, 8.0);
            assert_eq!(theme.spacing.node_graph.socket_spacing, 22.0);
            assert_eq!(theme.spacing.node_graph.socket_radius, 5.0);
            assert_eq!(theme.spacing.node_graph.socket_hit_slop, 4.0);
            assert_eq!(theme.spacing.node_graph.connection_tangent_min, 36.0);
            assert_eq!(theme.spacing.node_graph.grid_spacing, 40.0);
            assert_eq!(theme.spacing.node_graph.grid_spacing_min, 12.0);
            assert_eq!(theme.spacing.node_graph.grid_spacing_max, 160.0);
            let graph_surface = theme.color(ThemeColorToken::NodeGraphSurface);
            assert_eq!(graph_surface, theme.background.mix(theme.surface, 0.22));
            assert!(graph_surface.0 < theme.surface.0);
            assert_eq!(theme.spacing.file_browser.header_height, 48.0);
            assert_eq!(theme.spacing.file_browser.list_row_height, 22.0);
            assert_eq!(theme.spacing.file_browser.grid_cell_width, 120.0);
            assert_eq!(theme.spacing.file_browser.grid_cell_height, 78.0);
            assert_eq!(theme.spacing.file_browser.grid_gap, 0.0);
            assert_eq!(theme.spacing.collection.header_height, 28.0);
            assert_eq!(theme.spacing.collection.preview_cell_width, 112.0);
            assert!(theme.disabled_opacity > 0.0 && theme.disabled_opacity < 1.0);
        }
        assert_ne!(
            Theme::new(ThemeKind::Dark).color(ThemeColorToken::PreviewPlaceholder),
            Theme::new(ThemeKind::Light).color(ThemeColorToken::PreviewPlaceholder),
        );
        let dark = Theme::new(ThemeKind::Dark);
        let light = Theme::new(ThemeKind::Light);
        for color in ThemeColorToken::ALL {
            assert_ne!(dark.color(color), light.color(color), "{color:?}");
        }
    }

    #[test]
    fn every_spacing_token_is_finite_nonnegative_and_theme_rhythm_is_shared() {
        let dark = all_spacing_values(Theme::new(ThemeKind::Dark).spacing);
        let light = all_spacing_values(Theme::new(ThemeKind::Light).spacing);
        assert_eq!(dark, light, "Dark and Light share logical layout rhythm");
        assert_eq!(
            dark.len(),
            142,
            "update this check when adding a spacing token"
        );
        for (index, value) in dark.into_iter().enumerate() {
            assert!(
                value.is_finite() && value >= 0.0,
                "spacing[{index}]={value}"
            );
        }
        let motion = Theme::new(ThemeKind::Dark).motion;
        for (name, duration) in [
            ("fast", motion.fast),
            ("standard", motion.standard),
            ("slow", motion.slow),
            ("panel_expand", motion.panel_expand),
            ("menu_open", motion.menu_open),
        ] {
            assert!(!duration.is_zero(), "{name} motion duration must be usable");
        }
    }

    #[test]
    fn theme_defaults_resolve_selected_checked_active_focused_disabled_and_error_states() {
        let theme = Theme::default();
        let resolve = |state| {
            resolve_style(
                &theme,
                &UiStyleSheet::default(),
                None,
                None,
                state,
                None,
                &UiInlineStyle::default(),
            )
        };

        let selected = resolve(UiStyleState::SELECTED);
        assert_eq!(
            selected.color(UiStyleProperty::Background),
            Some(theme.selection)
        );
        assert_eq!(
            selected.color(UiStyleProperty::SelectionColor),
            Some(theme.selection)
        );
        let hovered_selection = resolve(UiStyleState::SELECTED.union(UiStyleState::HOVERED));
        assert_eq!(
            hovered_selection.color(UiStyleProperty::Background),
            Some(theme.selection.mix(theme.surface_hover, 0.28))
        );
        let checked = resolve(UiStyleState::CHECKED);
        assert_eq!(
            checked.color(UiStyleProperty::AccentColor),
            Some(theme.accent)
        );
        for state in [UiStyleState::FOCUSED, UiStyleState::ACTIVE] {
            assert_eq!(
                resolve(state).color(UiStyleProperty::BorderColor),
                Some(theme.focus)
            );
        }
        let disabled = resolve(UiStyleState::DISABLED);
        assert_eq!(
            disabled.color(UiStyleProperty::Background),
            Some(theme.disabled_surface)
        );
        assert_eq!(
            disabled.color(UiStyleProperty::Foreground),
            Some(theme.disabled)
        );
        assert_eq!(
            disabled.color(UiStyleProperty::BorderColor),
            Some(theme.disabled_border)
        );
        assert_eq!(
            disabled.scalar(UiStyleProperty::Opacity),
            Some(theme.disabled_opacity)
        );
        assert_eq!(
            resolve(UiStyleState::ERROR).color(UiStyleProperty::BorderColor),
            Some(theme.error)
        );
    }

    #[test]
    fn dynamic_state_updates_preserve_other_flags_and_remove_empty_component() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        assert!(set_ui_style_state(
            &mut world,
            entity,
            UiStyleState::SELECTED,
            true
        ));
        assert!(set_ui_style_state(
            &mut world,
            entity,
            UiStyleState::ACTIVE,
            true
        ));
        assert_eq!(
            world.get::<UiStyleState>(entity).copied(),
            Some(UiStyleState::SELECTED.union(UiStyleState::ACTIVE))
        );
        assert!(set_ui_style_state(
            &mut world,
            entity,
            UiStyleState::SELECTED,
            false
        ));
        assert_eq!(
            world.get::<UiStyleState>(entity).copied(),
            Some(UiStyleState::ACTIVE)
        );
        assert!(!set_ui_style_state(
            &mut world,
            entity,
            UiStyleState::SELECTED,
            false
        ));
        assert!(set_ui_style_state(
            &mut world,
            entity,
            UiStyleState::ACTIVE,
            false
        ));
        assert!(world.get::<UiStyleState>(entity).is_none());
    }

    #[test]
    fn semantic_roles_apply_and_refresh_container_and_widget_spacing() {
        let mut world = World::new();
        world.init_resource::<Theme>();
        world.init_resource::<UiStyleSheet>();
        let box_container = world
            .spawn((UiNode, UiRole::new("box"), UiLayoutNode::default()))
            .id();
        let property_row = world
            .spawn((UiNode, UiRole::new("property-row"), UiLayoutNode::default()))
            .id();
        let property_label = world
            .spawn((
                UiNode,
                UiRole::new("property-label"),
                UiLayoutNode::default(),
            ))
            .id();
        let menu_item = world
            .spawn((UiNode, UiRole::new("menu-item"), UiLayoutNode::default()))
            .id();
        let slider = world
            .spawn((UiNode, UiRole::new("slider"), UiLayoutNode::default()))
            .id();
        let menu_triggers = [
            "menu-trigger",
            "popover-trigger",
            "context-menu-trigger",
            "pie-menu-trigger",
        ]
        .map(|role| {
            world
                .spawn((UiNode, UiRole::new(role), UiLayoutNode::default()))
                .id()
        });
        let editor_roots = ["node-graph", "canvas"].map(|role| {
            world
                .spawn((UiNode, UiRole::new(role), UiLayoutNode::default()))
                .id()
        });
        let list_row = world
            .spawn((UiNode, UiRole::new("list-row"), UiLayoutNode::default()))
            .id();
        let table_header = world
            .spawn((UiNode, UiRole::new("table-header"), UiLayoutNode::default()))
            .id();
        let table_cell = world
            .spawn((UiNode, UiRole::new("table-cell"), UiLayoutNode::default()))
            .id();
        let gutter = world
            .spawn((
                UiNode,
                UiRole::new("editor-gutter"),
                UiLayoutNode::default(),
            ))
            .id();
        let browser_header = world
            .spawn((
                UiNode,
                UiRole::new("file-browser-header"),
                UiLayoutNode::default(),
            ))
            .id();
        let browser_grid_item = world
            .spawn((
                UiNode,
                UiRole::new("file-browser-grid-item"),
                UiLayoutNode::default(),
            ))
            .id();
        let collection_header = world
            .spawn((
                UiNode,
                UiRole::new("collection-header"),
                UiLayoutNode::default(),
            ))
            .id();
        let preview_item = world
            .spawn((
                UiNode,
                UiRole::new("preview-grid-item"),
                UiLayoutNode::default(),
            ))
            .id();
        let control_gap_roles = [
            "layout-row",
            "layout-column",
            "layout-grid",
            "layout-flex",
            "layout-inline",
            "scroll",
        ]
        .map(|role| {
            world
                .spawn((UiNode, UiRole::new(role), UiLayoutNode::default()))
                .id()
        });
        let menu_bar = world
            .spawn((UiNode, UiRole::new("menu-bar"), UiLayoutNode::default()))
            .id();

        resolve_ui_styles(&mut world);
        assert_eq!(Theme::default().spacing.control.slider_track_height, 4.0);
        for entity in control_gap_roles {
            assert_eq!(world.get::<UiLayoutNode>(entity).unwrap().gap, 8.0);
        }
        assert_eq!(world.get::<UiLayoutNode>(menu_bar).unwrap().gap, 4.0);
        assert_eq!(
            world.get::<UiLayoutNode>(slider).unwrap().size.height,
            UiLength::Px(32.0)
        );
        for entity in menu_triggers {
            let node = world.get::<UiLayoutNode>(entity).unwrap();
            assert_eq!(node.size.height, UiLength::Px(24.0));
            assert_eq!(node.padding, UiEdges::horizontal_vertical(12.0, 4.0));
        }
        for entity in editor_roots {
            let node = world.get::<UiLayoutNode>(entity).unwrap();
            assert_eq!(node.padding, UiEdges::horizontal_vertical(6.0, 4.0));
            assert_eq!(node.gap, 4.0);
        }
        assert_eq!(
            world.get::<UiLayoutNode>(box_container).unwrap().padding,
            UiEdges::all(12.0)
        );
        assert_eq!(world.get::<UiLayoutNode>(box_container).unwrap().gap, 8.0);
        assert_eq!(
            world.get::<UiLayoutNode>(property_row).unwrap().size.height,
            UiLength::Px(24.0)
        );
        assert_eq!(world.get::<UiLayoutNode>(property_row).unwrap().gap, 8.0);
        assert_eq!(
            world
                .get::<UiLayoutNode>(property_label)
                .unwrap()
                .size
                .width,
            UiLength::Px(112.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(menu_item).unwrap().size.height,
            UiLength::Px(24.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(list_row).unwrap().size.height,
            UiLength::Px(22.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(table_header).unwrap().size.height,
            UiLength::Px(26.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(table_cell).unwrap().padding,
            UiEdges::horizontal_vertical(8.0, 4.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(gutter).unwrap().size.width,
            UiLength::Px(24.0)
        );
        assert_eq!(
            world
                .get::<UiLayoutNode>(browser_header)
                .unwrap()
                .size
                .height,
            UiLength::Px(48.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(browser_grid_item).unwrap().size,
            UiSize {
                width: UiLength::Px(120.0),
                height: UiLength::Px(78.0),
            }
        );
        assert_eq!(
            world
                .get::<UiLayoutNode>(collection_header)
                .unwrap()
                .size
                .height,
            UiLength::Px(28.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(preview_item).unwrap().size,
            UiSize {
                width: UiLength::Px(112.0),
                height: UiLength::Px(104.0),
            }
        );

        world.increment_change_tick();
        {
            let mut theme = world.resource_mut::<Theme>();
            theme.spacing.menu.item_height = 32.0;
            theme.spacing.list.row_height = 28.0;
            theme.spacing.table.header_height = 30.0;
            theme.spacing.editor.gutter_width = 36.0;
            theme.spacing.file_browser.header_height = 52.0;
            theme.spacing.file_browser.grid_cell_width = 90.0;
            theme.spacing.collection.header_height = 34.0;
            theme.spacing.collection.preview_cell_height = 120.0;
            theme.spacing.box_container.padding = 16.0;
            theme.spacing.box_container.gap = 10.0;
            theme.spacing.property_row.label_width = 140.0;
            theme.spacing.property_row.gap = 12.0;
            theme.spacing.property_row.row_height = 28.0;
            theme.spacing.control.gap = 10.0;
            theme.spacing.control.slider_height = 36.0;
            theme.spacing.control.slider_track_height = 6.0;
            theme.spacing.menu.section_gap = 6.0;
            theme.spacing.menu.padding_x = 14.0;
            theme.spacing.editor.padding_x = 10.0;
        }
        resolve_ui_styles(&mut world);
        for entity in control_gap_roles {
            assert_eq!(world.get::<UiLayoutNode>(entity).unwrap().gap, 10.0);
        }
        assert_eq!(world.get::<UiLayoutNode>(menu_bar).unwrap().gap, 6.0);
        assert_eq!(
            world.get::<UiLayoutNode>(slider).unwrap().size.height,
            UiLength::Px(36.0)
        );
        for entity in menu_triggers {
            assert_eq!(
                world.get::<UiLayoutNode>(entity).unwrap().padding,
                UiEdges::horizontal_vertical(14.0, 4.0)
            );
        }
        for entity in editor_roots {
            assert_eq!(
                world.get::<UiLayoutNode>(entity).unwrap().padding,
                UiEdges::horizontal_vertical(10.0, 4.0)
            );
        }
        assert_eq!(
            world
                .resource::<Theme>()
                .spacing
                .control
                .slider_track_height,
            6.0
        );
        assert_eq!(
            world.get::<UiLayoutNode>(box_container).unwrap().padding,
            UiEdges::all(16.0)
        );
        assert_eq!(world.get::<UiLayoutNode>(box_container).unwrap().gap, 10.0);
        assert_eq!(
            world.get::<UiLayoutNode>(property_row).unwrap().size.height,
            UiLength::Px(28.0)
        );
        assert_eq!(world.get::<UiLayoutNode>(property_row).unwrap().gap, 12.0);
        assert_eq!(
            world
                .get::<UiLayoutNode>(property_label)
                .unwrap()
                .size
                .width,
            UiLength::Px(140.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(menu_item).unwrap().size.height,
            UiLength::Px(32.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(list_row).unwrap().size.height,
            UiLength::Px(28.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(table_header).unwrap().size.height,
            UiLength::Px(30.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(gutter).unwrap().size.width,
            UiLength::Px(36.0)
        );
        assert_eq!(
            world
                .get::<UiLayoutNode>(browser_header)
                .unwrap()
                .size
                .height,
            UiLength::Px(52.0)
        );
        assert_eq!(
            world
                .get::<UiLayoutNode>(browser_grid_item)
                .unwrap()
                .size
                .width,
            UiLength::Px(90.0)
        );
        assert_eq!(
            world
                .get::<UiLayoutNode>(collection_header)
                .unwrap()
                .size
                .height,
            UiLength::Px(34.0)
        );
        assert_eq!(
            world.get::<UiLayoutNode>(preview_item).unwrap().size.height,
            UiLength::Px(120.0)
        );
    }

    #[test]
    fn box_role_has_theme_surface_border_and_corner_defaults() {
        let mut world = World::new();
        world.init_resource::<Theme>();
        world.init_resource::<UiStyleSheet>();
        let box_node = world
            .spawn((UiNode, UiRole::new("box"), UiLayoutNode::default()))
            .id();

        resolve_ui_styles(&mut world);
        let dark = world.get::<UiComputedStyle>(box_node).unwrap();
        assert_eq!(
            dark.color(UiStyleProperty::Background),
            Some(Theme::default().surface)
        );
        assert_eq!(
            dark.color(UiStyleProperty::BorderColor),
            Some(Theme::default().border)
        );
        assert_eq!(dark.scalar(UiStyleProperty::BorderWidth), Some(1.0));
        assert_eq!(dark.scalar(UiStyleProperty::CornerRadius), Some(3.0));

        world.increment_change_tick();
        world.resource_mut::<Theme>().switch(ThemeKind::Light);
        resolve_ui_styles(&mut world);
        let light = world.get::<UiComputedStyle>(box_node).unwrap();
        let theme = world.resource::<Theme>();
        assert_eq!(
            light.color(UiStyleProperty::Background),
            Some(theme.surface)
        );
        assert_eq!(
            light.color(UiStyleProperty::BorderColor),
            Some(theme.border)
        );
        assert_eq!(light.scalar(UiStyleProperty::BorderWidth), Some(1.0));
        assert_eq!(
            light.scalar(UiStyleProperty::CornerRadius),
            Some(theme.corner_radius)
        );
    }

    #[test]
    fn native_box_shadow_role_tracks_theme_tokens_and_restores_caller_value() {
        let mut app = style_app();
        let base_shadow = BoxShadow::new(
            Color::srgba(0.12, 0.24, 0.36, 0.48),
            Val::Px(1.0),
            Val::Px(2.0),
            Val::Px(3.0),
            Val::Px(4.0),
        );
        let node = app
            .world_mut()
            .spawn((
                UiNode,
                bevy_ui::Node::default(),
                UiRole::new("menu-popup"),
                UiLayoutNode::default(),
                base_shadow.clone(),
            ))
            .id();

        app.update();
        let dark = Theme::default();
        let dark_shadow = dark.effects.popup_shadow.resolve_theme_color(&dark);
        let computed = app.world().get::<UiComputedStyle>(node).unwrap();
        assert_eq!(computed.box_shadow(), Some(dark_shadow.clone()));
        assert_eq!(
            app.world().get::<BoxShadow>(node),
            Some(&dark_shadow.to_native(&dark))
        );

        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        let light = Theme::new(ThemeKind::Light);
        let light_shadow = light.effects.popup_shadow.resolve_theme_color(&light);
        let computed = app.world().get::<UiComputedStyle>(node).unwrap();
        assert_eq!(computed.box_shadow(), Some(light_shadow.clone()));
        assert_ne!(dark_shadow, light_shadow);
        assert_eq!(
            app.world().get::<BoxShadow>(node),
            Some(&light_shadow.to_native(&light))
        );

        let rule_shadow = UiBoxShadow::themed(ThemeColorToken::Error, 1.0, 3.0, 0.0, 6.0);
        let mut declaration = UiStyleDeclaration::default();
        declaration.set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(rule_shadow.clone()),
        );
        app.world_mut()
            .resource_mut::<UiStyleSheet>()
            .push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("raised".into()),
                    required_state: UiStyleState::ACTIVE,
                    ..UiStyleSelector::default()
                },
                declaration,
                priority: 0,
            });
        app.world_mut()
            .entity_mut(node)
            .insert((UiClasses(vec!["raised".into()]), UiStyleState::ACTIVE));
        app.update();
        let resolved_rule = rule_shadow.resolve_theme_color(&light);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            Some(resolved_rule.clone())
        );
        assert_eq!(
            app.world().get::<BoxShadow>(node),
            Some(&resolved_rule.to_native(&light))
        );

        let mut inline_shadow = UiBoxShadow::themed(ThemeColorToken::Accent, 2.0, 7.0, 1.0, 9.0);
        inline_shadow.push(UiShadow::new(Rgba(0.1, 0.2, 0.3, 0.4), -2.0, 1.0, 0.0, 5.0));
        let mut inline = UiInlineStyle::default();
        inline.set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(inline_shadow.clone()),
        );
        app.world_mut().entity_mut(node).insert(inline);
        app.update();
        let resolved_inline = inline_shadow.resolve_theme_color(&light);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            Some(resolved_inline.clone())
        );
        assert_eq!(
            app.world().get::<BoxShadow>(node),
            Some(&resolved_inline.to_native(&light))
        );
        assert_eq!(app.world().get::<BoxShadow>(node).unwrap().0.len(), 2);

        app.world_mut().entity_mut(node).remove::<UiInlineStyle>();
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            Some(resolved_rule.clone()),
            "stylesheet value overrides role defaults and survives inline removal"
        );
        app.world_mut()
            .entity_mut(node)
            .remove::<(UiClasses, UiStyleState)>();
        app.world_mut().entity_mut(node).remove::<UiRole>();
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            None
        );
        assert_eq!(app.world().get::<BoxShadow>(node), Some(&base_shadow));
    }

    #[test]
    fn native_box_shadow_interpolates_and_reduced_motion_snaps() {
        let mut app = style_app();
        let mut start = UiBoxShadow::new(Rgba(0.0, 0.0, 0.0, 0.1), 0.0, 2.0, 0.0, 4.0);
        start.push(UiShadow::new(Rgba(0.1, 0.2, 0.3, 0.2), -2.0, 0.0, 0.0, 2.0));
        let mut finish = UiBoxShadow::new(Rgba(1.0, 0.5, 0.25, 0.5), 4.0, 8.0, 2.0, 12.0);
        finish.push(UiShadow::new(Rgba(0.5, 0.8, 0.9, 0.6), 2.0, 4.0, 2.0, 6.0));
        let mut inline = UiInlineStyle::default();
        inline.set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(start.clone()),
        );
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::BoxShadow, 100);
        let node = app
            .world_mut()
            .spawn((
                UiNode,
                bevy_ui::Node::default(),
                UiLayoutNode::default(),
                inline,
                transitions,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            Some(start.clone())
        );

        app.world_mut().get_mut::<UiInlineStyle>(node).unwrap().set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(finish.clone()),
        );
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            Some(start.clone()),
            "a shadow transition begins at the displayed value"
        );

        advance(&mut app, 50);
        let halfway = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow()
            .unwrap();
        assert_eq!(halfway, start.interpolate(&finish, 0.5).unwrap());
        assert_eq!(halfway.shadows.len(), 2);
        assert_eq!(halfway.shadows[0].x_offset, 2.0);
        assert_eq!(halfway.shadows[0].blur_radius, 8.0);
        assert_eq!(halfway.shadows[1].y_offset, 2.0);
        let UiShadowColor::Color(second_color) = halfway.shadows[1].color else {
            panic!("resolved shadow color should be concrete");
        };
        assert!((second_color.3 - 0.4).abs() < 0.0001);
        assert_eq!(
            app.world().get::<BoxShadow>(node),
            Some(&halfway.to_native(&Theme::default())),
            "the Bevy component tracks the interpolated computed effect"
        );

        app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
        let target = UiBoxShadow::new(Rgba(0.2, 0.4, 0.6, 0.8), 3.0, 6.0, 1.0, 10.0);
        app.world_mut().get_mut::<UiInlineStyle>(node).unwrap().set(
            UiStyleProperty::BoxShadow,
            UiStyleValue::BoxShadow(target.clone()),
        );
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .box_shadow(),
            Some(target.clone())
        );
        assert_eq!(
            app.world().get::<BoxShadow>(node),
            Some(&target.to_native(&Theme::default()))
        );
    }

    #[test]
    fn cascade_uses_selector_specificity_state_then_inline_values() {
        let theme = Theme::default();
        let mut sheet = UiStyleSheet::default();
        let mut broad = UiStyleDeclaration::default();
        broad.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(Rgba(0.1, 0.2, 0.3, 1.0)),
        );
        sheet.push(UiStyleRule {
            selector: UiStyleSelector::default(),
            declaration: broad,
            priority: 0,
        });
        let mut state = UiStyleDeclaration::default();
        state.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::Accent);
        sheet.push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("button".into()),
                required_state: UiStyleState::HOVERED,
                ..UiStyleSelector::default()
            },
            declaration: state,
            priority: 0,
        });
        let mut inline = UiInlineStyle::default();
        inline.set_color_token(
            UiStyleProperty::Background,
            ThemeColorToken::CurvePlotSurface,
        );
        let resolved = resolve_style(
            &theme,
            &sheet,
            None,
            Some(&UiClasses(vec!["button".into()])),
            UiStyleState::HOVERED,
            None,
            &inline,
        );
        assert_eq!(
            resolved.color(UiStyleProperty::Background),
            Some(theme.color(ThemeColorToken::CurvePlotSurface))
        );
        assert_eq!(
            resolved.color(UiStyleProperty::Foreground),
            Some(theme.color(ThemeColorToken::Accent))
        );
        let light = Theme::new(ThemeKind::Light);
        let light_resolved = resolve_style(
            &light,
            &sheet,
            None,
            Some(&UiClasses(vec!["button".into()])),
            UiStyleState::HOVERED,
            None,
            &inline,
        );
        assert_eq!(
            light_resolved.color(UiStyleProperty::Background),
            Some(light.color(ThemeColorToken::CurvePlotSurface))
        );
        assert_eq!(
            light_resolved.color(UiStyleProperty::Foreground),
            Some(light.color(ThemeColorToken::Accent))
        );
        assert_ne!(
            resolved.color(UiStyleProperty::Background),
            light_resolved.color(UiStyleProperty::Background)
        );
    }

    #[test]
    fn inline_literal_override_is_stable_across_theme_and_state_changes() {
        let mut sheet = UiStyleSheet::default();
        for (required_state, token) in [
            (UiStyleState::HOVERED, ThemeColorToken::ButtonHoverSurface),
            (UiStyleState::PRESSED, ThemeColorToken::PressedSurface),
            (UiStyleState::SELECTED, ThemeColorToken::SelectedSurface),
            (UiStyleState::DISABLED, ThemeColorToken::DisabledSurface),
        ] {
            let mut declaration = UiStyleDeclaration::default();
            declaration.set_color_token(UiStyleProperty::Background, token);
            sheet.push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("control".into()),
                    required_state,
                    ..UiStyleSelector::default()
                },
                declaration,
                priority: 0,
            });
        }
        let override_color = Rgba(0.31, 0.27, 0.83, 1.0);
        let mut inline = UiInlineStyle::default();
        inline.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(override_color),
        );
        let classes = UiClasses(vec!["control".into()]);
        let states = [
            UiStyleState::default(),
            UiStyleState::HOVERED,
            UiStyleState::PRESSED,
            UiStyleState::SELECTED,
            UiStyleState::DISABLED,
            UiStyleState(UiStyleState::HOVERED.0 | UiStyleState::SELECTED.0),
        ];

        for kind in [ThemeKind::Dark, ThemeKind::Light] {
            let theme = Theme::new(kind);
            for state in states {
                let resolved =
                    resolve_style(&theme, &sheet, None, Some(&classes), state, None, &inline);
                assert_eq!(
                    resolved.color(UiStyleProperty::Background),
                    Some(override_color),
                    "inline literal must dominate {kind:?} state {state:?}"
                );
                assert_eq!(
                    resolved.color(UiStyleProperty::Foreground),
                    Some(if state.contains(UiStyleState::DISABLED) {
                        theme.disabled
                    } else {
                        theme.text
                    }),
                    "non-overridden foreground remains theme/state-derived"
                );
            }
        }
    }

    #[test]
    fn text_values_inherit_but_layout_declarations_do_not() {
        let theme = Theme::default();
        let parent = UiComputedStyle {
            properties: HashMap::from([
                (
                    UiStyleProperty::Foreground,
                    UiStyleValue::Color(Rgba(0.2, 0.3, 0.4, 1.0)),
                ),
                (UiStyleProperty::Gap, UiStyleValue::Scalar(19.0)),
            ]),
        };
        let child = resolve_style(
            &theme,
            &UiStyleSheet::default(),
            None,
            None,
            UiStyleState::default(),
            Some(&parent),
            &UiInlineStyle::default(),
        );
        assert_eq!(
            child.color(UiStyleProperty::Foreground),
            Some(Rgba(0.2, 0.3, 0.4, 1.0))
        );
        assert_eq!(child.scalar(UiStyleProperty::Gap), None);
    }

    #[test]
    fn state_changes_apply_layout_before_layout_and_revert_to_base() {
        let mut world = World::new();
        world.init_resource::<UiStyleSheet>();
        let node = world
            .spawn((
                UiNode,
                UiLayoutNode {
                    gap: 2.0,
                    ..UiLayoutNode::default()
                },
                UiClasses(vec!["compact".into()]),
            ))
            .id();
        let mut declaration = UiStyleDeclaration::default();
        declaration.set(UiStyleProperty::Gap, UiStyleValue::Scalar(8.0));
        world
            .resource_mut::<UiStyleSheet>()
            .rules
            .push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("compact".into()),
                    ..UiStyleSelector::default()
                },
                declaration,
                priority: 0,
            });
        resolve_ui_styles(&mut world);
        assert_eq!(world.get::<UiLayoutNode>(node).unwrap().gap, 8.0);
        world.increment_change_tick();
        world.resource_mut::<UiStyleSheet>().clear();
        resolve_ui_styles(&mut world);
        assert_eq!(world.get::<UiLayoutNode>(node).unwrap().gap, 2.0);
    }

    #[test]
    fn plugin_resolves_before_layout_phase() {
        let mut app = App::new();
        app.add_plugins(StylePlugin);
        let node = app
            .world_mut()
            .spawn((UiNode, UiInlineStyle::default()))
            .id();
        app.update();
        assert!(app.world().get::<UiComputedStyle>(node).is_some());
    }

    #[test]
    fn theme_switch_transitions_from_current_to_new_theme_with_time() {
        let mut app = style_app();
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Background, 1000);
        let node = app.world_mut().spawn((UiNode, transitions)).id();
        app.update();
        let start = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();

        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(start),
            "a transition begins at the currently displayed theme value"
        );
        assert!(
            app.world()
                .get::<UiStyleTransitionState>(node)
                .unwrap()
                .is_transitioning(UiStyleProperty::Background)
        );

        advance(&mut app, 500);
        let current = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        let target = Theme::new(ThemeKind::Light).surface;
        assert!((current.0 - start.mix(target, 0.5).0).abs() < 0.001);
        assert!((current.1 - start.mix(target, 0.5).1).abs() < 0.001);

        advance(&mut app, 500);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(target)
        );
        assert!(
            app.world()
                .get::<UiStyleTransitionState>(node)
                .is_none_or(|state| !state.is_transitioning(UiStyleProperty::Background))
        );
    }

    #[test]
    fn live_theme_switch_invalidates_only_styles_with_changed_resolved_values() {
        let mut app = style_app();
        app.init_resource::<ChangedStyleEntities>()
            .configure_sets(Update, StyleChangeCapture.after(UiSystems::Style))
            .add_systems(Update, collect_changed_styles.in_set(StyleChangeCapture));
        let normal = app.world_mut().spawn(UiNode).id();
        let dark = Theme::default();
        let frozen_style = resolve_style(
            &dark,
            &UiStyleSheet::default(),
            None,
            None,
            UiStyleState::default(),
            None,
            &UiInlineStyle::default(),
        );
        let frozen = app
            .world_mut()
            .spawn((
                UiNode,
                UiInlineStyle(frozen_style.properties.into_iter().collect()),
            ))
            .id();
        let mut token_style = UiInlineStyle::default();
        token_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::CurveGridLine);
        let tokenized = app.world_mut().spawn((UiNode, token_style)).id();

        app.update();
        assert!(
            app.world()
                .resource::<ChangedStyleEntities>()
                .0
                .contains(&normal)
        );
        assert!(
            app.world()
                .resource::<ChangedStyleEntities>()
                .0
                .contains(&frozen)
        );
        assert!(
            app.world()
                .resource::<ChangedStyleEntities>()
                .0
                .contains(&tokenized)
        );
        app.update();
        assert!(
            app.world().resource::<ChangedStyleEntities>().0.is_empty(),
            "unchanged resolved styles are not rewritten on idle frames"
        );

        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        let changed = &app.world().resource::<ChangedStyleEntities>().0;
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&normal));
        assert!(changed.contains(&tokenized));
        assert!(!changed.contains(&frozen));
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(normal)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(Theme::new(ThemeKind::Light).surface)
        );
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(frozen)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(dark.surface)
        );
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(tokenized)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(Theme::new(ThemeKind::Light).color(ThemeColorToken::CurveGridLine))
        );
    }

    #[test]
    fn theme_color_token_transitions_and_reduced_motion_use_current_theme_values() {
        let mut app = style_app();
        let mut inline = UiInlineStyle::default();
        inline.set_color_token(
            UiStyleProperty::Background,
            ThemeColorToken::NodeGraphSelectedNodeSurface,
        );
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Background, 1000);
        let node = app.world_mut().spawn((UiNode, inline, transitions)).id();
        app.update();
        let dark = Theme::new(ThemeKind::Dark).color(ThemeColorToken::NodeGraphSelectedNodeSurface);
        let light =
            Theme::new(ThemeKind::Light).color(ThemeColorToken::NodeGraphSelectedNodeSurface);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(dark)
        );

        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        advance(&mut app, 500);
        let middle = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        assert!((middle.0 - dark.mix(light, 0.5).0).abs() < 0.001);
        assert!((middle.1 - dark.mix(light, 0.5).1).abs() < 0.001);

        app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Dark);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(dark)
        );
        assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
    }

    #[test]
    fn reduced_motion_snaps_active_style_transitions_to_their_targets() {
        let mut app = style_app();
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Background, 1000);
        let node = app.world_mut().spawn((UiNode, transitions)).id();
        app.update();
        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        advance(&mut app, 250);
        let partial = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        let target = Theme::new(ThemeKind::Light).surface;
        assert_ne!(partial, target);
        assert!(
            app.world()
                .get::<UiStyleTransitionState>(node)
                .is_some_and(|state| state.is_transitioning(UiStyleProperty::Background))
        );

        app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(target)
        );
        assert!(app.world().get::<UiStyleTransitionState>(node).is_none());

        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Dark);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(Theme::default().surface)
        );
        assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
    }

    #[test]
    fn hover_and_disabled_state_transitions_use_per_property_specs() {
        let mut app = style_app();
        let window = app.world_mut().spawn_empty().id();
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Background, 400);
        set_transition(&mut transitions, UiStyleProperty::Foreground, 400);
        let node = app.world_mut().spawn((UiNode, transitions)).id();
        app.update();
        let base_background = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        let base_foreground = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Foreground)
            .unwrap();

        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .hovered = Some(node);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(base_background)
        );
        advance(&mut app, 400);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(Theme::default().surface_hover)
        );

        app.world_mut().entity_mut(node).insert(UiEnabled(false));
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Foreground),
            Some(base_foreground)
        );
        advance(&mut app, 200);
        let disabled_foreground = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Foreground)
            .unwrap();
        assert!(
            (disabled_foreground.0 - base_foreground.mix(Theme::default().disabled, 0.5).0).abs()
                < 0.001
        );
        advance(&mut app, 200);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Foreground),
            Some(Theme::default().disabled)
        );
    }

    #[test]
    fn interpolated_layout_properties_apply_before_layout() {
        let mut app = style_app();
        let window = app.world_mut().spawn_empty().id();
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Gap, 400);
        let mut declaration = UiStyleDeclaration::default();
        declaration.set(UiStyleProperty::Gap, UiStyleValue::Scalar(10.0));
        app.world_mut()
            .resource_mut::<UiStyleSheet>()
            .push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("animated".into()),
                    required_state: UiStyleState::HOVERED,
                    ..UiStyleSelector::default()
                },
                declaration,
                priority: 0,
            });
        let node = app
            .world_mut()
            .spawn((
                UiNode,
                UiClasses(vec!["animated".into()]),
                UiLayoutNode {
                    gap: 2.0,
                    ..UiLayoutNode::default()
                },
                transitions,
            ))
            .id();
        app.update();
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .hovered = Some(node);
        app.update();
        assert_eq!(app.world().get::<UiLayoutNode>(node).unwrap().gap, 2.0);
        advance(&mut app, 200);
        assert!((app.world().get::<UiLayoutNode>(node).unwrap().gap - 6.0).abs() < 0.001);
        assert!(
            (app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .scalar(UiStyleProperty::Gap)
                .unwrap()
                - 6.0)
                .abs()
                < 0.001
        );
        advance(&mut app, 200);
        assert_eq!(app.world().get::<UiLayoutNode>(node).unwrap().gap, 10.0);
    }

    #[test]
    fn scroll_hover_style_reflows_before_text_and_keeps_stationary_hover_on_hitbox() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, LayoutPlugin, StylePlugin));
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Overlay,
                    align_items: UiAlign::Start,
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let scroll = app
            .world_mut()
            .spawn((
                UiNode,
                UiScroll::new(),
                UiLayoutNode {
                    mode: UiLayoutMode::Column,
                    size: UiSize {
                        width: UiLength::Px(80.0),
                        height: UiLength::Px(30.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let spacer = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            ))
            .id();
        let first = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            ))
            .id();
        let hovered = app
            .world_mut()
            .spawn((
                UiNode,
                UiClasses(vec!["hover-reflow".into()]),
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            ))
            .id();
        set_parent(app.world_mut(), scroll, root).unwrap();
        set_parent(app.world_mut(), spacer, scroll).unwrap();
        set_parent(app.world_mut(), first, scroll).unwrap();
        set_parent(app.world_mut(), hovered, scroll).unwrap();

        let mut declaration = UiStyleDeclaration::default();
        declaration.set(
            UiStyleProperty::Margin,
            UiStyleValue::Edges(UiEdges {
                top: 5.0,
                ..UiEdges::ZERO
            }),
        );
        app.world_mut()
            .resource_mut::<UiStyleSheet>()
            .push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("hover-reflow".into()),
                    required_state: UiStyleState::HOVERED,
                    ..UiStyleSelector::default()
                },
                declaration,
                priority: 0,
            });

        app.update();
        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::PointerMove {
                window,
                position: [10.0, 25.0],
                time_seconds: 0.0,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .hovered,
            Some(first)
        );

        app.world_mut()
            .resource_mut::<UiInputQueue>()
            .push(UiInputEvent::Wheel {
                window,
                position: [10.0, 25.0],
                delta: [0.0, -1.0],
                unit: UiWheelUnit::Lines,
                modifiers: Default::default(),
            });
        app.update();

        assert_eq!(
            app.world().get::<UiScroll>(scroll).unwrap().offset,
            [0.0, 30.0]
        );
        assert_eq!(
            app.world()
                .resource::<UiFocus>()
                .state(window)
                .unwrap()
                .hovered,
            Some(hovered)
        );
        assert_eq!(
            app.world().get::<UiLayoutNode>(hovered).unwrap().margin.top,
            5.0
        );
        assert_eq!(
            app.world().get::<UiLayoutResult>(hovered).unwrap().rect.y,
            15.0
        );
        assert!(
            app.world()
                .get::<bui_core::UiHitBox>(hovered)
                .unwrap()
                .contains([10.0, 25.0])
        );
    }

    #[test]
    fn animated_width_updates_shared_layout_results_before_paint() {
        let mut app = App::new();
        app.add_plugins((CorePlugin, StylePlugin, bui_layout::LayoutPlugin))
            .init_resource::<Time>();
        let window = app.world_mut().spawn_empty().id();
        let root = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutRoot::new(window, bui_layout::Rect::new(0.0, 0.0, 200.0, 80.0), 1.0),
                UiLayoutNode {
                    mode: UiLayoutMode::Row,
                    size: bui_layout::UiSize {
                        width: UiLength::Px(200.0),
                        height: UiLength::Px(80.0),
                    },
                    ..UiLayoutNode::default()
                },
            ))
            .id();
        let mut inline = UiInlineStyle::default();
        inline.set(
            UiStyleProperty::Width,
            UiStyleValue::Length(UiLength::Px(40.0)),
        );
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Width, 1000);
        let child = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(40.0), UiLength::Px(20.0)),
                inline,
                transitions,
            ))
            .id();
        bui_core::set_parent(app.world_mut(), child, root)
            .expect("the test child belongs to the layout root");
        app.update();
        assert_eq!(
            app.world().get::<UiLayoutResult>(child).unwrap().rect.width,
            40.0
        );

        {
            let mut inline = app.world_mut().get_mut::<UiInlineStyle>(child).unwrap();
            inline.set(
                UiStyleProperty::Width,
                UiStyleValue::Length(UiLength::Px(120.0)),
            );
        }
        app.update();
        assert_eq!(
            app.world().get::<UiLayoutResult>(child).unwrap().rect.width,
            40.0
        );
        advance(&mut app, 500);
        assert!(
            (app.world().get::<UiLayoutResult>(child).unwrap().rect.width - 80.0).abs() < 0.001
        );
    }

    #[test]
    fn transition_reversal_starts_at_the_displayed_value_and_reduced_motion_snaps() {
        let mut app = style_app();
        let window = app.world_mut().spawn_empty().id();
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Background, 1000);
        let node = app.world_mut().spawn((UiNode, transitions)).id();
        app.update();
        let base = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        let hover = Theme::default().surface_hover;
        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .hovered = Some(node);
        app.update();
        advance(&mut app, 250);
        let before_reverse = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();

        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .hovered = None;
        app.update();
        let after_reverse = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        assert_eq!(after_reverse, before_reverse);
        advance(&mut app, 500);
        let halfway_back = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        assert!((halfway_back.0 - before_reverse.mix(base, 0.5).0).abs() < 0.001);

        app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
        app.world_mut()
            .resource_mut::<Theme>()
            .switch(ThemeKind::Light);
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(Theme::new(ThemeKind::Light).surface)
        );
        assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
        assert_ne!(base, hover);

        app.world_mut().resource_mut::<UiReducedMotion>().0 = false;
        {
            let mut theme = app.world_mut().resource_mut::<Theme>();
            theme.switch(ThemeKind::Dark);
            theme.motion_scale = 0.0;
        }
        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(Theme::new(ThemeKind::Dark).surface)
        );
        assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
    }

    #[test]
    fn style_cache_tracks_focus_changes_and_active_transition_ticks_same_frame() {
        let mut app = style_app();
        let window = app.world_mut().spawn_empty().id();
        let base_color = Rgba(0.1, 0.2, 0.8, 1.0);
        let focus_color = Rgba(0.9, 0.2, 0.1, 1.0);
        let mut base_declaration = UiStyleDeclaration::default();
        base_declaration.set(UiStyleProperty::Background, UiStyleValue::Color(base_color));
        let mut focus_declaration = UiStyleDeclaration::default();
        focus_declaration.set(
            UiStyleProperty::Background,
            UiStyleValue::Color(focus_color),
        );
        {
            let mut sheet = app.world_mut().resource_mut::<UiStyleSheet>();
            sheet.push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("focus-cache".into()),
                    ..UiStyleSelector::default()
                },
                declaration: base_declaration,
                priority: 0,
            });
            sheet.push(UiStyleRule {
                selector: UiStyleSelector {
                    class: Some("focus-cache".into()),
                    required_state: UiStyleState::FOCUSED,
                    ..UiStyleSelector::default()
                },
                declaration: focus_declaration,
                priority: 0,
            });
        }
        let mut transitions = UiTransitions::default();
        set_transition(&mut transitions, UiStyleProperty::Background, 100);
        let node = app
            .world_mut()
            .spawn((
                UiNode,
                UiFocusable,
                UiClasses(vec!["focus-cache".into()]),
                UiLayoutNode::default(),
                transitions,
            ))
            .id();

        app.update();
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(base_color)
        );
        let initial_passes = app
            .world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes;
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiStyleResolutionCache>()
                .resolved_passes,
            initial_passes,
            "an idle frame skips both cascade passes"
        );

        app.world_mut()
            .resource_mut::<UiFocus>()
            .state_mut(window)
            .focused = Some(node);
        app.update();
        let started_color = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        assert_eq!(started_color, base_color);
        assert!(app.world().get::<UiStyleTransitionState>(node).is_some());
        assert_eq!(
            app.world()
                .resource::<UiStyleResolutionCache>()
                .resolved_passes,
            initial_passes + 1,
            "focus changes are resolved before widgets in the same update"
        );

        advance(&mut app, 50);
        let halfway = app
            .world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background)
            .unwrap();
        assert_ne!(halfway, base_color);
        assert_ne!(halfway, focus_color);
        assert_eq!(
            app.world()
                .resource::<UiStyleResolutionCache>()
                .resolved_passes,
            initial_passes + 2,
            "the advanced transition tick invalidates the next style phase"
        );

        advance(&mut app, 50);
        assert_eq!(
            app.world()
                .get::<UiComputedStyle>(node)
                .unwrap()
                .color(UiStyleProperty::Background),
            Some(focus_color)
        );
        assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
        let settled_passes = app
            .world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes;
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiStyleResolutionCache>()
                .resolved_passes,
            settled_passes,
            "completed transitions return to the idle fast path"
        );
    }

    #[derive(Resource, Debug, Default, Clone, Copy)]
    struct ChangeCounts {
        styles: usize,
        layouts: usize,
    }

    fn count_changed_components(
        styles: Query<Ref<UiComputedStyle>>,
        layouts: Query<Ref<UiLayoutNode>>,
        mut counts: ResMut<ChangeCounts>,
    ) {
        counts.styles += styles.iter().filter(|style| style.is_changed()).count();
        counts.layouts += layouts.iter().filter(|layout| layout.is_changed()).count();
    }

    #[test]
    fn unchanged_style_and_layout_values_do_not_retrigger_change_detection() {
        let mut app = style_app();
        app.init_resource::<ChangeCounts>().add_systems(
            Update,
            count_changed_components.after(UiSystems::StyleAfterInput),
        );
        let node = app
            .world_mut()
            .spawn((
                UiNode,
                UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
                UiInlineStyle::default(),
            ))
            .id();
        app.update();
        let first = *app.world().resource::<ChangeCounts>();
        let first_style_passes = app
            .world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes;
        assert_eq!(first.styles, 1);
        assert_eq!(first_style_passes, 1);
        app.update();
        let second = *app.world().resource::<ChangeCounts>();
        assert_eq!(second.styles, first.styles);
        assert_eq!(second.layouts, first.layouts);
        assert_eq!(
            app.world()
                .resource::<UiStyleResolutionCache>()
                .resolved_passes,
            first_style_passes,
            "both style phases skip a frame with unchanged inputs"
        );

        {
            let mut inline = app.world_mut().get_mut::<UiInlineStyle>(node).unwrap();
            inline.set(UiStyleProperty::Gap, UiStyleValue::Scalar(6.0));
        }
        app.update();
        let changed = *app.world().resource::<ChangeCounts>();
        assert!(changed.styles > second.styles);
        assert!(changed.layouts > second.layouts);
        assert_eq!(
            app.world()
                .resource::<UiStyleResolutionCache>()
                .resolved_passes,
            first_style_passes + 1,
            "an inline declaration change resolves in the same update"
        );
    }
}

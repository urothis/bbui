//! BUI's Bevy plugin and public facade.

use bevy::prelude::*;

mod accessibility;
mod clipboard_bridge;
mod cursor_bridge;
mod input_bridge;
mod window_layout;
mod world_input;

pub use bui_macros::bui;
pub use bui_media as media;
pub use bui_render as render;
pub use bui_widgets as widgets;

pub struct BuiPlugin;

impl Plugin for BuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bui_core::CorePlugin);
        app.add_plugins(bui_style::StylePlugin);
        app.add_plugins(bui_layout::LayoutPlugin);
        app.add_plugins(bui_text::TextPlugin);
        app.add_plugins(bui_media::BuiMediaPlugin);
        app.add_plugins(bui_render::BuiRenderPlugin);
        app.add_plugins(bui_widgets::WidgetPlugin);
        app.init_resource::<accessibility::BuiFocusMirror>();
        app.init_resource::<accessibility::AccessibilityLinkSelection>();
        app.add_message::<bevy::a11y::ActionRequest>();
        app.add_systems(
            Update,
            accessibility::route_accessibility_actions
                .in_set(bui_core::UiSystems::Widgets)
                .before(bui_widgets::actions::process_routed_widget_events),
        );
        app.add_systems(
            Update,
            accessibility::sync_accessible_focus
                .in_set(bui_core::UiSystems::Widgets)
                .after(bui_widgets::actions::sync_text_controls)
                .before(accessibility::sync_accessibility),
        );
        app.add_systems(
            Update,
            accessibility::sync_accessibility
                .in_set(bui_core::UiSystems::Widgets)
                .after(bui_widgets::actions::sync_text_controls)
                .after(bui_widgets::canvas::sync_canvas_object_views),
        );
        app.add_systems(
            Update,
            window_layout::sync_window_layout_roots
                .in_set(bui_core::UiSystems::Style)
                .before(bui_text::measure_text_intrinsics),
        );
        app.add_systems(
            Update,
            clipboard_bridge::process_clipboard_requests
                .in_set(bui_core::UiSystems::Widgets)
                .after(bui_widgets::actions::process_routed_widget_events),
        );
        app.init_resource::<input_bridge::ImeBridgeState>();
        app.init_resource::<cursor_bridge::CursorCaptureState>();
        app.init_resource::<world_input::WorldPointerState>();
        app.add_systems(
            Update,
            input_bridge::bridge_bevy_input
                .in_set(bui_core::UiSystems::Input)
                .before(bui_core::process_ui_input),
        );
        app.add_systems(
            Update,
            world_input::bridge_world_pointer
                .in_set(bui_core::UiSystems::Input)
                .before(input_bridge::bridge_bevy_input)
                .before(bui_core::process_ui_input),
        );
        app.add_systems(
            Update,
            cursor_bridge::process_cursor_capture_requests
                .in_set(bui_core::UiSystems::Widgets)
                .after(bui_widgets::actions::process_routed_widget_events),
        );
    }
}

pub mod prelude {
    pub use crate::BuiPlugin;
    pub use crate::bui;
    pub use bui_core::{
        UiEnabled, UiFocus, UiFocusable, UiKey, UiLivePoliteness, UiLiveRegion, UiNode, UiVisible,
    };
    pub use bui_layout::{
        UiAlign, UiEdges, UiFlexDirection, UiFlexItem, UiIntrinsicSize, UiJustify, UiLayoutMode,
        UiLayoutNode, UiLayoutRoot, UiLength, UiScroll, UiSize,
    };
    pub use bui_style::{
        Theme, ThemeBoxSpacing, ThemeCollectionSpacing, ThemeColorRampSpacing, ThemeColorToken,
        ThemeControlSpacing, ThemeCurveSpacing, ThemeEditorSpacing, ThemeEffects,
        ThemeFileBrowserSpacing, ThemeHsvSpacing, ThemeKind, ThemeListSpacing, ThemeMenuSpacing,
        ThemeMotion, ThemeMotionToken, ThemeNodeGraphSpacing, ThemePanelSpacing,
        ThemePropertyRowSpacing, ThemeSpacing, ThemeTableSpacing, ThemeTooltipSpacing, UiBoxShadow,
        UiClasses, UiComputedStyle, UiEasing, UiInlineStyle, UiReducedMotion, UiRole, UiShadow,
        UiShadowColor, UiStyleProperty, UiStyleSheet, UiStyleState, UiStyleValue, UiTransitionSpec,
        UiTransitions, set_ui_style_state,
    };
    pub use bui_text::{
        TextBuffer, UI_DEFAULT_FONT_FAMILY, UiFontAsset, UiFontAssetState, UiFontAssetStatus,
        UiTextLayout, resolve_text_font, ui_default_text_font,
    };
    pub use bui_widgets::icon_assets::IconButtonAssetCache;
    pub use bui_widgets::{
        Button, Canvas, Checkbox, ColorRamp, CurveEditor, DataTable, Dropdown, FileBrowser,
        HsvColorEditor, HsvColorField, IconButton, IconButtonAsset, IconButtonAssetState,
        IconButtonAssetStatus, Label, ListItem, ListModel, NodeCategory, NodeGraph, NumberField,
        NumericExpressionVariables, NumericField, Panel, PanelExpansionStore, PanelGroup,
        PanelOrderStore, PreviewGrid, PreviewGridItem, Progress, RadioGroup, Slider, TableColumn,
        TableRow, TextArea, TextField, Tooltip, TreeModel, WidgetContainer, WidgetNode, WidgetTree,
    };
}

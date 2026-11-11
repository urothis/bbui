//! BBUI's Bevy plugin and public facade.

use bevy::prelude::*;

mod accessibility;
mod clipboard_bridge;
mod cursor_bridge;
mod input_bridge;
mod window_layout;
mod world_input;
pub use world_input::{UiCrosshairPointer, WorldPointerState, bridge_world_pointer};

pub use bbui_macros::bbui;
pub use bbui_media as media;
pub use bbui_motion as motion;
pub use bbui_render as render;
pub use bbui_widgets as widgets;

pub struct BbuiPlugin;

impl Plugin for BbuiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bbui_core::CorePlugin);
        app.add_plugins(bbui_style::StylePlugin);
        app.add_plugins(bbui_layout::LayoutPlugin);
        app.add_plugins(bbui_text::TextPlugin);
        app.add_plugins(bbui_media::BbuiMediaPlugin);
        app.add_plugins(bbui_render::BbuiRenderPlugin);
        app.add_plugins(bbui_widgets::WidgetPlugin);
        app.init_resource::<accessibility::BbuiFocusMirror>();
        app.init_resource::<accessibility::AccessibilityLinkSelection>();
        app.init_resource::<accessibility::AccessibilitySyncCache>();
        app.add_message::<bevy::a11y::ActionRequest>();
        app.add_systems(
            Update,
            accessibility::route_accessibility_actions
                .in_set(bbui_core::UiSystems::Widgets)
                .before(bbui_widgets::actions::process_routed_widget_events),
        );
        app.add_systems(
            Update,
            accessibility::sync_accessible_focus
                .in_set(bbui_core::UiSystems::Widgets)
                .after(bbui_widgets::actions::sync_text_controls)
                .before(accessibility::sync_accessibility),
        );
        app.add_systems(
            Update,
            accessibility::collect_accessibility_resource_changes
                .in_set(bbui_core::UiSystems::Widgets)
                .after(bbui_widgets::actions::collect_widget_paint)
                .before(accessibility::sync_accessibility),
        );
        app.add_systems(
            Update,
            accessibility::collect_world_accessibility_changes
                .in_set(bbui_core::UiSystems::Widgets)
                .before(accessibility::sync_accessibility),
        );
        app.add_systems(
            Update,
            accessibility::sync_accessibility
                .in_set(bbui_core::UiSystems::Widgets)
                .after(bbui_widgets::actions::sync_text_controls)
                .after(bbui_widgets::canvas::sync_canvas_object_views),
        );
        app.add_systems(
            Update,
            window_layout::sync_window_layout_roots
                .in_set(bbui_core::UiSystems::Style)
                .before(bbui_text::measure_text_intrinsics),
        );
        app.add_systems(
            Update,
            clipboard_bridge::process_clipboard_requests
                .in_set(bbui_core::UiSystems::Widgets)
                .after(bbui_widgets::actions::process_routed_widget_events),
        );
        app.init_resource::<input_bridge::ImeBridgeState>();
        app.init_resource::<cursor_bridge::CursorCaptureState>();
        app.init_resource::<world_input::WorldPointerState>();
        app.init_resource::<Assets<Mesh>>();
        app.add_systems(
            Update,
            input_bridge::bridge_bevy_input
                .in_set(bbui_core::UiSystems::Input)
                .before(bbui_core::process_ui_input),
        );
        app.add_systems(
            Update,
            world_input::bridge_world_pointer
                .in_set(bbui_core::UiSystems::Input)
                .before(input_bridge::bridge_bevy_input)
                .before(bbui_core::process_ui_input),
        );
        app.add_systems(
            Update,
            cursor_bridge::process_cursor_capture_requests
                .in_set(bbui_core::UiSystems::Widgets)
                .after(bbui_widgets::actions::process_routed_widget_events),
        );
    }
}

pub mod prelude {
    pub use crate::BbuiPlugin;
    pub use crate::bbui;
    pub use bbui_core::{
        UiDirty, UiDirtyBatch, UiDirtyDomain, UiEnabled, UiFocus, UiFocusable, UiKey,
        UiLivePoliteness, UiLiveRegion, UiNode, UiSystems, UiVisible,
    };
    pub use bbui_layout::{
        Rect, UiAlign, UiEdges, UiFlexDirection, UiFlexItem, UiIntrinsicSize, UiJustify,
        UiLayoutMode, UiLayoutNode, UiLayoutRoot, UiLength, UiScroll, UiSize,
    };
    pub use bbui_motion::{
        Keyframe, KeyframeTrack, MotionBinding, MotionClip, MotionCurve, MotionError, MotionEvent,
        MotionEventKind, MotionEvents, MotionHandle, MotionPlayer, MotionPriority, MotionValue,
        seek_motion,
    };
    pub use bbui_style::{
        Theme, ThemeBoxSpacing, ThemeCollectionSpacing, ThemeColorRampSpacing, ThemeColorToken,
        ThemeControlSpacing, ThemeCurveSpacing, ThemeEditorSpacing, ThemeEffects,
        ThemeFileBrowserSpacing, ThemeHsvSpacing, ThemeKind, ThemeListSpacing, ThemeMenuSpacing,
        ThemeMotion, ThemeMotionToken, ThemeNodeGraphSpacing, ThemePanelSpacing,
        ThemePropertyRowSpacing, ThemeSpacing, ThemeTableSpacing, ThemeTooltipSpacing,
        UiAnimationSpec, UiAutoAnimations, UiBoxShadow, UiClasses, UiComputedStyle, UiInlineStyle,
        UiReducedMotion, UiRole, UiShadow, UiShadowColor, UiStyleProperty, UiStyleSheet,
        UiStyleState, UiStyleValue, set_ui_style_state, style_motion_binding,
    };
    pub use bbui_text::{
        TextBuffer, UI_DEFAULT_FONT_FAMILY, UiFontAsset, UiFontAssetState, UiFontAssetStatus,
        UiTextLayout, resolve_text_font, ui_default_text_font,
    };
    pub use bbui_widgets::actions::{menu_progress_binding, panel_progress_binding};
    pub use bbui_widgets::icon_assets::IconButtonAssetCache;
    pub use bbui_widgets::{
        Button, Canvas, Checkbox, ColorRamp, CurveEditor, DataTable, Dropdown, FileBrowser,
        HsvColorEditor, HsvColorField, IconButton, IconButtonAsset, IconButtonAssetState,
        IconButtonAssetStatus, Label, ListItem, ListModel, NodeCategory, NodeGraph, NumberField,
        NumericExpressionVariables, NumericField, Panel, PanelExpansionStore, PanelGroup,
        PanelOrderStore, PreviewGrid, PreviewGridItem, Progress, RadioGroup, Slider, TableColumn,
        TableRow, TextArea, TextField, Tooltip, TreeModel, WidgetContainer, WidgetNode, WidgetTree,
    };
}

//! Reusable, stateful controls and editor data models for Bevy applications.

use bbui_text::expression::{self, Error};
use bevy_app::Update;
use bevy_app::{App, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use std::collections::HashMap;

pub mod actions;
pub mod canvas;
pub mod collections;
pub mod controls;
pub mod declarative;
pub mod editor_chrome;
pub mod editors;
pub mod file_browser;
pub mod file_previews;
pub mod gestures;
pub mod history;
pub mod icon_assets;
pub mod menus;
pub mod numeric;
pub mod panels;
pub mod sliders;

pub use actions::{
    ClipboardRequest, ClipboardRequestQueue, CursorCaptureRequest, CursorCaptureRequestQueue,
    WidgetAction, WidgetActionKind, WidgetActionQueue, WidgetEvent, WidgetEventQueue,
    WidgetInteractionState, WidgetValue,
};
pub use canvas::CanvasObjectControl;
pub use canvas::{Canvas, CanvasObject, CanvasShape};
pub use collections::{
    DataTable, ListItem, ListModel, ListSelection, PreviewGrid, PreviewGridItem, TableColumn,
    TableRow, TreeModel, TreeNode,
};
pub use controls::{
    Button, Checkbox, CheckboxValue, Dropdown, IconButton, IconButtonAsset, IconButtonAssetState,
    IconButtonAssetStatus, Label, Progress, RadioGroup, TextArea, TextField, Tooltip, WidgetStatus,
};
pub use declarative::{
    CollectionEditorState, CollectionItemControl, CollectionItemKind, ColorRampEditorState,
    DataTableCellControl, DataTableHeaderControl, DropdownOptionControl, DropdownPopupState,
    FileBrowserItem, NodeGraphEditorState, NodeGraphLinkControl, NodeGraphNodeControl,
    NodeGraphSocketControl, RadioOptionControl, WidgetContainer, WidgetElement, WidgetNode,
    WidgetSpec, WidgetTree, WidgetTreeError,
};
pub use editor_chrome::EditorChrome;
pub use editors::{
    ColorChannel, ColorRamp, ColorStop, CurveEditor, CurvePoint, HsvColorEditor,
    HsvColorEditorState, HsvColorField, Interpolation, LinkEndpoint, NodeCategory, NodeGraph,
    NodeGraphError, NodeGraphTypeRegistry, NodeId, NodeLink, NodeModel, NodeSocket,
    NodeTypeDefinition, SocketDirection, SocketEditorDefinition, SocketId, SocketPlacement,
    SocketType, SocketTypeDefinition, hsv_to_rgb, parse_hex_color, rgb_to_hsv,
};
pub use file_browser::{
    Breadcrumb, FileBrowser, FileBrowserError, FileEntry, FileSort, FileView, PreviewKind,
};
pub use menus::{
    MenuControl, MenuDisplay, MenuEntry, MenuGesture, MenuItemControl, MenuSession, MenuTree,
    PieMenu, PopupState,
};
pub use numeric::{
    NumericExpressionVariables, NumericField, NumericFieldKind, NumericValue, format_float,
};
pub use panels::{Panel, PanelExpansionStore, PanelGroup, PanelOrderStore, PanelState};
pub use sliders::Slider;

pub mod prelude {
    pub use crate::{
        Button, Canvas, Checkbox, ColorRamp, CurveEditor, DataTable, Dropdown,
        DropdownOptionControl, DropdownPopupState, HsvColorEditor, HsvColorField, IconButton,
        IconButtonAsset, IconButtonAssetState, IconButtonAssetStatus, LinkEndpoint, ListModel,
        MenuSession, NodeCategory, NodeGraph, NodeGraphNodeControl, NodeGraphSocketControl,
        NodeGraphTypeRegistry, NodeId, NodeLink, NodeModel, NodeSocket, NodeTypeDefinition,
        NumberField, NumericExpressionVariables, NumericField, Panel, PanelGroup, PanelOrderStore,
        PreviewGrid, RadioGroup, RadioOptionControl, Slider, SocketDirection,
        SocketEditorDefinition, SocketId, SocketPlacement, SocketType, SocketTypeDefinition,
        TextArea, TextField, TreeModel, WidgetActionQueue, WidgetEventQueue, WidgetPlugin,
        WidgetTree,
    };
}

/// Installs the widget action processor and its event queues.
pub struct WidgetPlugin;

impl Plugin for WidgetPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bbui_motion::MotionPlugin>() {
            app.add_plugins(bbui_motion::MotionPlugin);
        }
        actions::install_widget_systems(app);
        app.init_resource::<NodeGraphTypeRegistry>();
        file_previews::install(app);
        icon_assets::install(app);
        panels::install_panel_persistence(app);
        app.add_systems(
            Update,
            declarative::sync_intrinsic_label_text
                .in_set(bbui_core::UiSystems::TextMeasure)
                .before(bbui_text::measure_text_intrinsics),
        );
        app.init_resource::<canvas::CanvasInputState>()
            .init_resource::<canvas::CanvasImageAssets>()
            .init_resource::<canvas::CanvasObjectItems>()
            .add_systems(
                Update,
                canvas::process_canvas_input
                    .in_set(bbui_core::UiSystems::Widgets)
                    .before(actions::process_routed_widget_events),
            )
            .add_systems(
                Update,
                canvas::sync_canvas_object_views
                    .in_set(bbui_core::UiSystems::Widgets)
                    .after(actions::apply_widget_actions)
                    .before(actions::collect_widget_paint),
            )
            .add_systems(
                Update,
                canvas::sync_canvas_images
                    .in_set(bbui_core::UiSystems::Widgets)
                    .after(actions::apply_widget_actions)
                    .before(actions::collect_widget_paint),
            );
        app.init_resource::<bbui_core::UiHistory>()
            .init_resource::<history::WidgetHistoryCursor>()
            .add_systems(
                Update,
                history::process_widget_undo_shortcuts
                    .before(actions::process_routed_widget_events)
                    .in_set(bbui_core::UiSystems::Widgets),
            )
            .add_systems(
                Update,
                history::record_widget_edits
                    .after(actions::collect_widget_paint)
                    .in_set(bbui_core::UiSystems::Widgets),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberField {
    pub value: f64,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl NumberField {
    pub fn new(value: f64, min: f64, max: f64, step: f64) -> Result<Self, &'static str> {
        if !value.is_finite()
            || !min.is_finite()
            || !max.is_finite()
            || !step.is_finite()
            || min > max
            || step <= 0.0
            || value < min
            || value > max
        {
            return Err("invalid number-field bounds or value");
        }
        Ok(Self {
            value,
            default: value,
            min,
            max,
            step,
        })
    }

    pub fn reset(&mut self) {
        self.value = self.default;
    }

    pub fn evaluate_input(
        &mut self,
        input: &str,
        vars: &HashMap<String, f64>,
    ) -> Result<f64, Error> {
        let mut context = vars.clone();
        context.insert("x".into(), self.value);
        let value = expression::evaluate(input, &context)?;
        self.value = value.clamp(self.min, self.max);
        Ok(self.value)
    }

    pub fn scrub(&mut self, start: f64, delta_pixels: f64, precise: bool, snap: bool) -> f64 {
        let speed = if precise { 0.1 } else { 1.0 };
        let raw = start + delta_pixels * self.step * speed;
        let value = if snap {
            (raw / self.step).round() * self.step
        } else {
            raw
        };
        self.value = value.clamp(self.min, self.max);
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_failure_preserves_value_and_scrub_uses_modifiers() {
        let mut field = NumberField::new(2.0, 0.0, 10.0, 1.0).unwrap();
        assert!(field.evaluate_input("1/0", &HashMap::new()).is_err());
        assert_eq!(field.value, 2.0);
        assert_eq!(field.scrub(2.0, 5.0, true, false), 2.5);
        assert_eq!(field.scrub(2.0, 5.0, false, true), 7.0);
        field.reset();
        assert_eq!(field.value, 2.0);
    }
}

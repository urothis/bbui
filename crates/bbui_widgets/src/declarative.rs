//! Declarative widget trees that instantiate real Bevy entities and components.

use bbui_core::{
    UiAccessibleName, UiChildren, UiDirty, UiDirtyDomain, UiEnabled, UiFocusable, UiHierarchyError,
    UiKey, UiNode, UiTab, UiTabIndex, UiTabList, UiVisible, cleanup_removed_node, set_parent,
};
use bbui_layout::{
    Rect, UiContentReveal, UiEdges, UiFlexDirection, UiLayoutMode, UiLayoutNode, UiLength,
    UiScroll, UiWheelHandled,
};
use bbui_style::{Rgba, UiInlineStyle, UiRole, UiStyleProperty, UiStyleValue};
use bbui_text::{TextBuffer, UiExternalClipboard, UiMultiline, UiTextLayout};
use bevy_ecs::prelude::{Changed, Component, Entity, Or, Query, ResMut, With, World};
use std::path::PathBuf;

use crate::{
    Button, Canvas, Checkbox, ColorRamp, CurveEditor, DataTable, Dropdown, FileBrowser,
    HsvColorEditor, IconButton, IconButtonAsset, IconButtonAssetState, Label, ListModel,
    MenuControl, MenuDisplay, MenuItemControl, MenuTree, NodeGraph, NumericField, NumericFieldKind,
    Panel, PreviewGrid, Progress, RadioGroup, Slider, TextArea, TextField,
    Tooltip as TooltipControl, TreeModel,
    editors::{ColorChannel, HsvColorEditorState, LinkEndpoint, NodeId, SocketDirection},
    menus::{MenuEntry, PopupState},
};

mod model;
pub(crate) use model::IntrinsicLabelText;
pub use model::*;
mod tree;
pub use tree::*;

mod mount;
pub(crate) use mount::sync_intrinsic_label_text;
use mount::*;

mod reconcile;
use reconcile::*;

pub(crate) fn remove_widget_subtree(world: &mut World, entity: Entity) {
    remove_subtree(world, entity);
}

pub(crate) fn default_widget_layout(node: &WidgetNode) -> UiLayoutNode {
    layout_for(&node.spec)
}

mod view;
pub use view::*;

#[cfg(test)]
#[path = "declarative/tests.rs"]
mod tests;

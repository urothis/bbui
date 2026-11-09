//! Declarative widget trees that instantiate real Bevy entities and components.

use bevy_ecs::prelude::{Changed, Component, Entity, Or, Query, ResMut, With, World};
use bui_core::{
    UiDirty, UiDirtyDomain, UiEnabled, UiFocusable, UiHierarchyError, UiKey, UiNode, UiTabIndex,
    UiVisible, set_parent,
};
use bui_layout::{
    Rect, UiEdges, UiFlexDirection, UiLayoutMode, UiLayoutNode, UiLength, UiScroll, UiWheelHandled,
};
use bui_style::{Rgba, UiInlineStyle, UiRole, UiStyleProperty, UiStyleValue};
use bui_text::{TextBuffer, UiExternalClipboard, UiMultiline, UiTextLayout};
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

mod mount;
pub(crate) use mount::sync_intrinsic_label_text;
use mount::*;

mod view;
pub use view::*;

#[cfg(test)]
#[path = "declarative/tests.rs"]
mod tests;

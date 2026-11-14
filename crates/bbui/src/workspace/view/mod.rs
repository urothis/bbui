use super::input::DropEdge;
use super::input::WorkspaceTarget;
use super::*;
use bbui_core::{
    UiChildren, UiDirty, UiDirtyDomain, UiEnabled, UiFocus, UiHistoryDelegate, UiKey, UiNode,
    UiParent, UiVisible, cleanup_removed_node, detach_from_parent, set_parent,
};
use bbui_layout::{Rect, UiLayoutNode, UiLayoutRoot, UiLength};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bbui_style::Theme;
use bbui_style::{ThemeColorToken, UiInlineStyle, UiStyleProperty, UiStyleValue};
use bbui_widgets::{DockNode, SplitAxis, WidgetNode, WidgetTree};
use bevy::{
    camera::RenderTarget,
    window::{PrimaryWindow, WindowRef, WindowResolution},
};
use std::collections::{HashMap, HashSet};

mod chrome;
mod helpers;
mod sync;
use chrome::chrome;
use helpers::*;
pub(super) use sync::sync_workspace;

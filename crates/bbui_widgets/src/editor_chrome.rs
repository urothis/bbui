//! Reusable chrome for inspector and properties-style editors.

use bbui_style::EditorRole;

use crate::{MenuEntry, WidgetNode};

/// Styled composites shared by editors built with [`WidgetNode`].
pub struct EditorChrome;

impl EditorChrome {
    pub fn flat_menu(
        label: impl Into<String>,
        entries: impl IntoIterator<Item = MenuEntry>,
    ) -> WidgetNode {
        WidgetNode::menu(label, entries).with_role(EditorRole::FLAT_MENU)
    }

    pub fn menu_bar(menus: impl IntoIterator<Item = WidgetNode>) -> WidgetNode {
        WidgetNode::menu_bar(menus).with_role(EditorRole::MENU_BAR)
    }

    pub fn tab_rail(tabs: impl IntoIterator<Item = WidgetNode>) -> WidgetNode {
        WidgetNode::column(tabs)
            .with_role(EditorRole::TAB_RAIL)
            .with_tab_list()
    }

    pub fn primary_panel(
        title: impl Into<String>,
        expanded: bool,
        children: impl IntoIterator<Item = WidgetNode>,
    ) -> WidgetNode {
        WidgetNode::panel(title, expanded, children).with_role(EditorRole::PRIMARY_PANEL)
    }

    pub fn sub_panel(
        title: impl Into<String>,
        expanded: bool,
        children: impl IntoIterator<Item = WidgetNode>,
    ) -> WidgetNode {
        WidgetNode::panel(title, expanded, children).with_role(EditorRole::SUB_PANEL)
    }

    pub fn progress(value: f32) -> WidgetNode {
        WidgetNode::progress(value).with_role(EditorRole::PROGRESS)
    }
}

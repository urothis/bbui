//! Reusable editor chrome roles shared by style resolution and widget painting.

pub struct EditorRole;

impl EditorRole {
    pub const FLAT_MENU: &'static str = "editor-flat-menu";
    pub const MENU_BAR: &'static str = "editor-menu-bar";
    pub const TAB_RAIL: &'static str = "editor-tab-rail";
    pub const PRIMARY_PANEL: &'static str = "editor-primary-panel";
    pub const SUB_PANEL: &'static str = "editor-sub-panel";
    pub const PROGRESS: &'static str = "editor-progress";
}

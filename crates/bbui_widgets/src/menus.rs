//! Menu trees, nested keyboard navigation, popup placement, and pie-menu selection.

use bbui_core::search::MenuSearch;
use bbui_layout::Rect;
use bevy_ecs::prelude::{Component, Entity};
use std::f32::consts::TAU;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub shortcut: Option<String>,
    pub children: Vec<MenuEntry>,
}

impl MenuEntry {
    pub fn command(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            separator: false,
            shortcut: None,
            children: Vec::new(),
        }
    }

    pub fn submenu(
        id: impl Into<String>,
        label: impl Into<String>,
        children: Vec<MenuEntry>,
    ) -> Self {
        Self {
            children,
            ..Self::command(id, label)
        }
    }

    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            enabled: false,
            separator: true,
            shortcut: None,
            children: Vec::new(),
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    fn selectable(&self) -> bool {
        self.enabled && !self.separator
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuTree {
    pub entries: Vec<MenuEntry>,
}

impl MenuTree {
    pub fn new(entries: Vec<MenuEntry>) -> Self {
        Self { entries }
    }

    /// Validate command identities so search ranking and routed activation stay unambiguous.
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut ids = std::collections::HashSet::new();
        for (_, _, entry) in self.flattened() {
            if entry.separator {
                continue;
            }
            if entry.id.trim().is_empty() {
                return Err("menu commands and submenus need a non-empty id");
            }
            if !ids.insert(entry.id.as_str()) {
                return Err("menu command ids must be unique within a menu tree");
            }
        }
        Ok(())
    }

    pub fn search<'a>(&'a self, search: &MenuSearch, query: &str) -> Vec<&'a MenuEntry> {
        let mut all = Vec::new();
        flatten_paths(&self.entries, "", &mut all);
        let mut entries_by_id = std::collections::HashMap::with_capacity(all.len());
        for (entry, _) in &all {
            entries_by_id.entry(entry.id.as_str()).or_insert(*entry);
        }
        let ranked = search.rank(
            query,
            all.iter().flat_map(|(entry, path)| {
                [
                    (entry.id.as_str(), path.as_str()),
                    (entry.id.as_str(), entry.label.as_str()),
                ]
            }),
        );
        let mut seen = std::collections::HashSet::new();
        ranked
            .into_iter()
            .filter(|id| seen.insert(*id))
            .filter_map(|id| entries_by_id.get(id).copied())
            .collect()
    }

    pub fn entries_at(&self, path: &[usize]) -> Option<&[MenuEntry]> {
        let mut entries = self.entries.as_slice();
        for index in path {
            entries = entries.get(*index)?.children.as_slice();
        }
        Some(entries)
    }

    pub fn flattened(&self) -> Vec<(Vec<usize>, usize, &MenuEntry)> {
        let mut entries = Vec::new();
        flatten_indexed(&self.entries, &mut Vec::new(), &mut entries);
        entries
    }
}

fn flatten_indexed<'a>(
    entries: &'a [MenuEntry],
    parent_path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, usize, &'a MenuEntry)>,
) {
    for (index, entry) in entries.iter().enumerate() {
        out.push((parent_path.clone(), index, entry));
        parent_path.push(index);
        flatten_indexed(&entry.children, parent_path, out);
        parent_path.pop();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuDisplay {
    #[default]
    Dropdown,
    Popover,
    Context,
    Pie,
}

#[derive(Component, Debug)]
#[component(summary_tick)]
pub struct MenuControl {
    pub label: String,
    pub tree: MenuTree,
    pub session: MenuSession,
    pub popup: PopupState,
    pub display: MenuDisplay,
    pub search: MenuSearch,
    pub query: String,
    pub items: Vec<Entity>,
    pub gesture: MenuGesture,
    pub pie: Option<PieMenu>,
    pub width: f32,
    pub row_height: f32,
    /// Set while a press on a closed trigger opens the popup for drag-to-pick.
    pub opened_by_pointer: bool,
    /// Pointer-down position on the trigger, used to distinguish a click from a drag-open.
    pub press_position: Option<[f32; 2]>,
    /// Context menus anchor to the pointer rather than the trigger rectangle.
    pub context_position: Option<[f32; 2]>,
}

impl MenuControl {
    pub fn new(label: impl Into<String>, tree: MenuTree, display: MenuDisplay) -> Self {
        Self {
            label: label.into(),
            tree,
            session: MenuSession::default(),
            popup: PopupState::new(Rect::default()),
            display,
            search: MenuSearch::default(),
            query: String::new(),
            items: Vec::new(),
            gesture: MenuGesture::default(),
            pie: None,
            width: 220.0,
            row_height: 24.0,
            opened_by_pointer: false,
            press_position: None,
            context_position: None,
        }
    }

    pub fn open(&mut self, focus_restore_key: Option<String>) {
        self.session.open(focus_restore_key);
        self.query.clear();
        self.context_position = None;
        self.opened_by_pointer = false;
        if self.display == MenuDisplay::Pie {
            self.pie = Some(PieMenu::new([0.0, 0.0], 10.0, 96.0));
        }
    }

    pub fn open_at(&mut self, position: [f32; 2], focus_restore_key: Option<String>) {
        self.open(focus_restore_key);
        if position.into_iter().all(f32::is_finite) {
            self.context_position = Some(position);
        }
    }

    pub fn clear_search_history(&mut self) {
        self.search.clear_history();
    }

    pub fn close(&mut self) -> Option<&str> {
        self.gesture.cancel();
        self.pie = None;
        self.opened_by_pointer = false;
        self.press_position = None;
        self.context_position = None;
        self.session.close()
    }

    pub fn toggle(&mut self, focus_restore_key: Option<String>) {
        if self.session.open {
            self.close();
        } else {
            self.open(focus_restore_key);
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(summary_tick)]
pub struct MenuItemControl {
    pub owner: Entity,
    pub id: String,
    pub label: String,
    pub parent_path: Vec<usize>,
    pub index: usize,
    pub enabled: bool,
    pub separator: bool,
    pub has_submenu: bool,
    pub shortcut: Option<String>,
}

fn flatten_paths<'a>(
    entries: &'a [MenuEntry],
    parent: &str,
    out: &mut Vec<(&'a MenuEntry, String)>,
) {
    for entry in entries {
        let path = if parent.is_empty() {
            entry.label.clone()
        } else {
            format!("{parent} {}", entry.label)
        };
        if !entry.separator && !entry.id.is_empty() {
            out.push((entry, path.clone()));
        }
        flatten_paths(&entry.children, &path, out);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuSession {
    pub open: bool,
    /// Each element is the selected child index at one nesting depth.
    pub path: Vec<usize>,
    pub focus_restore_key: Option<String>,
}

impl MenuSession {
    pub fn open(&mut self, focus_restore_key: Option<String>) {
        self.open = true;
        self.path.clear();
        self.focus_restore_key = focus_restore_key;
    }

    pub fn close(&mut self) -> Option<&str> {
        self.open = false;
        self.path.clear();
        self.focus_restore_key.as_deref()
    }

    pub fn escape(&mut self) -> Option<&str> {
        if !self.open {
            return None;
        }
        if self.path.len() > 1 {
            self.path.pop();
            None
        } else {
            self.close()
        }
    }

    pub fn outside_click(&mut self) -> Option<&str> {
        self.close()
    }

    pub fn selected<'a>(&self, tree: &'a MenuTree) -> Option<&'a MenuEntry> {
        if !self.open || self.path.is_empty() {
            return None;
        }
        entry_at_path(&tree.entries, &self.path)
    }

    pub fn move_selection(&mut self, tree: &MenuTree, delta: i32) -> Option<usize> {
        if !self.open || delta == 0 {
            return self.path.last().copied();
        }
        let depth = self.path.len().saturating_sub(1);
        let entries = children_at_depth(&tree.entries, &self.path, depth)?;
        let current = self.path.last().copied();
        let start = current.map_or(if delta > 0 { -1 } else { entries.len() as i64 }, |index| {
            index as i64
        });
        let direction = delta.signum() as i64;
        let mut at = start;
        for _ in 0..entries.len() {
            at = (at + direction).rem_euclid(entries.len() as i64);
            if entries[at as usize].selectable() {
                if self.path.is_empty() {
                    self.path.push(at as usize);
                } else if let Some(last) = self.path.last_mut() {
                    *last = at as usize;
                }
                return Some(at as usize);
            }
        }
        None
    }

    pub fn hover(&mut self, tree: &MenuTree, index: usize) -> bool {
        if !self.open {
            return false;
        }
        let depth = self.path.len().saturating_sub(1);
        let Some(entries) = children_at_depth(&tree.entries, &self.path, depth) else {
            return false;
        };
        if !entries.get(index).is_some_and(|entry| entry.selectable()) {
            return false;
        }
        if self.path.is_empty() {
            self.path.push(index);
        } else {
            self.path.truncate(depth + 1);
            self.path[depth] = index;
        }
        true
    }

    pub fn open_submenu(&mut self, tree: &MenuTree) -> bool {
        let Some(selected) = self.selected(tree) else {
            return false;
        };
        if selected.children.is_empty() {
            return false;
        }
        let Some(index) = selected.children.iter().position(MenuEntry::selectable) else {
            return false;
        };
        self.path.push(index);
        true
    }

    pub fn close_submenu(&mut self) -> bool {
        if self.path.len() <= 1 {
            return false;
        }
        self.path.pop();
        true
    }

    /// Activates the highlighted command. Submenus open instead of returning an action.
    pub fn activate(&mut self, tree: &MenuTree) -> Option<String> {
        let entry = self.selected(tree)?;
        if !entry.children.is_empty() {
            self.open_submenu(tree);
            return None;
        }
        if !entry.selectable() {
            return None;
        }
        let id = entry.id.clone();
        self.close();
        Some(id)
    }
}

fn children_at_depth<'a>(
    root: &'a [MenuEntry],
    path: &[usize],
    depth: usize,
) -> Option<&'a [MenuEntry]> {
    if depth == 0 {
        return Some(root);
    }
    let mut entries = root;
    for index in path.iter().take(depth) {
        entries = entries.get(*index)?.children.as_slice();
    }
    Some(entries)
}

fn entry_at_path<'a>(root: &'a [MenuEntry], path: &[usize]) -> Option<&'a MenuEntry> {
    let mut entries = root;
    let mut entry = None;
    for index in path {
        entry = entries.get(*index);
        entries = &entry?.children;
    }
    entry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupSide {
    #[default]
    Below,
    Above,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopupState {
    pub open: bool,
    pub anchor: Rect,
    pub bounds: Rect,
    pub side: PopupSide,
}

impl PopupState {
    pub fn new(anchor: Rect) -> Self {
        Self {
            open: false,
            anchor,
            bounds: Rect::default(),
            side: PopupSide::Below,
        }
    }

    /// Place a popup in logical UI coordinates, keeping its full bounds inside the viewport.
    pub fn place(
        &mut self,
        viewport: Rect,
        size: [f32; 2],
        preferred: PopupSide,
        margin: f32,
    ) -> Rect {
        let margin = margin.max(0.0);
        let width = size[0]
            .max(0.0)
            .min((viewport.width - margin * 2.0).max(0.0));
        let height = size[1]
            .max(0.0)
            .min((viewport.height - margin * 2.0).max(0.0));
        let below = viewport.y + viewport.height - (self.anchor.y + self.anchor.height);
        let above = self.anchor.y - viewport.y;
        self.side = match preferred {
            PopupSide::Below if below >= height || below >= above => PopupSide::Below,
            PopupSide::Above if above >= height || above >= below => PopupSide::Above,
            PopupSide::Left | PopupSide::Right => preferred,
            _ if below >= above => PopupSide::Below,
            _ => PopupSide::Above,
        };
        let (x, y) = match self.side {
            PopupSide::Below => (self.anchor.x, self.anchor.y + self.anchor.height),
            PopupSide::Above => (self.anchor.x, self.anchor.y - height),
            PopupSide::Left => (self.anchor.x - width, self.anchor.y),
            PopupSide::Right => (self.anchor.x + self.anchor.width, self.anchor.y),
        };
        self.bounds = Rect {
            x: x.clamp(
                viewport.x + margin,
                (viewport.x + viewport.width - margin - width).max(viewport.x + margin),
            ),
            y: y.clamp(
                viewport.y + margin,
                (viewport.y + viewport.height - margin - height).max(viewport.y + margin),
            ),
            width,
            height,
        };
        self.bounds
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuGesture {
    pub pressed: bool,
    pub hovered: Option<usize>,
    pub cancelled: bool,
}

impl MenuGesture {
    pub fn press(&mut self, item: Option<usize>) {
        self.pressed = true;
        self.hovered = item;
        self.cancelled = false;
    }
    pub fn drag_over(&mut self, item: Option<usize>) {
        if self.pressed {
            self.hovered = item;
        }
    }
    pub fn cancel(&mut self) {
        self.pressed = false;
        self.hovered = None;
        self.cancelled = true;
    }

    pub fn release(
        &mut self,
        item_under_pointer: Option<usize>,
        entries: &[MenuEntry],
    ) -> Option<String> {
        if !self.pressed || self.cancelled {
            return None;
        }
        self.pressed = false;
        let selected = item_under_pointer.or(self.hovered)?;
        let entry = entries.get(selected)?;
        entry.selectable().then(|| entry.id.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PieMenu {
    pub center: [f32; 2],
    pub dead_zone: f32,
    pub radius: f32,
    pub selected: Option<usize>,
}

impl PieMenu {
    pub fn new(center: [f32; 2], dead_zone: f32, radius: f32) -> Self {
        Self {
            center,
            dead_zone: dead_zone.max(0.0),
            radius: radius.max(0.0),
            selected: None,
        }
    }

    /// The first wedge starts at twelve o'clock and proceeds clockwise.
    pub fn update_pointer(&mut self, position: [f32; 2], item_count: usize) -> Option<usize> {
        if item_count == 0 {
            self.selected = None;
            return None;
        }
        let dx = position[0] - self.center[0];
        let dy = position[1] - self.center[1];
        let distance = dx.hypot(dy);
        if distance < self.dead_zone || (self.radius > 0.0 && distance > self.radius) {
            self.selected = None;
            return None;
        }
        let angle = (dx.atan2(-dy)).rem_euclid(TAU);
        let wedge = (angle / (TAU / item_count as f32)).floor() as usize;
        self.selected = Some(wedge.min(item_count - 1));
        self.selected
    }

    pub fn select_keyboard(&mut self, index: usize, item_count: usize) -> Option<usize> {
        self.selected = (index < item_count).then_some(index);
        self.selected
    }

    pub fn in_outer_radius(&self, position: [f32; 2]) -> bool {
        self.radius <= 0.0
            || (position[0] - self.center[0]).hypot(position[1] - self.center[1]) <= self.radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu() -> MenuTree {
        MenuTree::new(vec![
            MenuEntry::command("new", "New"),
            MenuEntry::separator(),
            MenuEntry::submenu(
                "add",
                "Add",
                vec![
                    MenuEntry::command("cube", "Cube"),
                    MenuEntry::command("curve", "Curve"),
                ],
            ),
            MenuEntry {
                enabled: false,
                ..MenuEntry::command("hidden", "Hidden")
            },
        ])
    }

    #[test]
    fn keyboard_navigation_skips_separators_and_opens_submenus() {
        let tree = menu();
        let mut session = MenuSession::default();
        session.open(Some("toolbar".into()));
        assert_eq!(session.move_selection(&tree, 1), Some(0));
        assert_eq!(session.move_selection(&tree, 1), Some(2));
        assert!(session.open_submenu(&tree));
        assert_eq!(session.selected(&tree).unwrap().id, "cube");
        assert_eq!(session.activate(&tree).as_deref(), Some("cube"));
        assert!(!session.open);
    }

    #[test]
    fn press_drag_release_and_outside_click_close_with_focus_restore() {
        let tree = menu();
        let mut gesture = MenuGesture::default();
        gesture.press(Some(0));
        gesture.drag_over(Some(2));
        assert_eq!(
            gesture.release(Some(0), &tree.entries).as_deref(),
            Some("new")
        );
        let mut session = MenuSession::default();
        session.open(Some("button".into()));
        assert_eq!(session.outside_click(), Some("button"));
    }

    #[test]
    fn popup_flips_above_and_clamps_to_viewport() {
        let mut popup = PopupState::new(Rect {
            x: 80.0,
            y: 85.0,
            width: 10.0,
            height: 5.0,
        });
        let bounds = popup.place(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            [60.0, 40.0],
            PopupSide::Below,
            2.0,
        );
        assert_eq!(popup.side, PopupSide::Above);
        assert_eq!(
            bounds,
            Rect {
                x: 38.0,
                y: 45.0,
                width: 60.0,
                height: 40.0
            }
        );
    }

    #[test]
    fn pie_menu_uses_clockwise_slices_and_dead_zone() {
        let mut pie = PieMenu::new([0.0, 0.0], 10.0, 80.0);
        assert_eq!(pie.update_pointer([0.0, -5.0], 4), None);
        assert_eq!(pie.update_pointer([0.0, -20.0], 4), Some(0));
        assert_eq!(pie.update_pointer([20.0, 0.0], 4), Some(1));
        assert!(!pie.in_outer_radius([100.0, 0.0]));
    }

    #[test]
    fn nested_fuzzy_search_finds_initials_and_typos() {
        let tree = menu();
        let mut search = MenuSearch::default();
        search.pick("cube");
        assert_eq!(tree.search(&search, "add cu")[0].id, "cube");
        assert_eq!(tree.search(&search, "cubx")[0].id, "cube");
        assert_eq!(tree.search(&search, "ad cubr")[0].id, "cube");

        let tied = MenuTree::new(vec![
            MenuEntry::command("z-item", "Select"),
            MenuEntry::command("a-item", "Select"),
        ]);
        let tie_search = MenuSearch::default();
        let first: Vec<_> = tied
            .search(&tie_search, "select")
            .into_iter()
            .map(|entry| entry.id.as_str())
            .collect();
        let second: Vec<_> = tied
            .search(&tie_search, "select")
            .into_iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(first, ["a-item", "z-item"]);
        assert_eq!(second, first);
    }

    #[test]
    fn search_resolves_ranked_ids_from_large_menu_trees() {
        let mut entries = (0..10_000)
            .map(|index| MenuEntry::command(format!("item-{index}"), format!("Item {index}")))
            .collect::<Vec<_>>();
        entries.push(MenuEntry::command("unique-target", "Unique Target"));
        let tree = MenuTree::new(entries);
        let results = tree.search(&MenuSearch::default(), "unique target");
        assert_eq!(
            results.first().map(|entry| entry.id.as_str()),
            Some("unique-target")
        );
    }
}

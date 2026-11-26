//! Host-defined menus for the macOS application menu bar.
//!
//! Hosts own commands and call [`sync`] from the main thread after the native
//! application menu exists. AppKit queues selected command IDs for
//! [`take_commands`]; it never executes application commands directly.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeMenuBar {
    /// Items placed in the application menu before the system-owned entries.
    pub application: Vec<NativeMenuEntry>,
    /// Top-level menus after the application menu.
    pub menus: Vec<NativeMenuGroup>,
    /// If present, intercept the system Quit item and queue this command ID.
    pub quit_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMenuGroup {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub visible: bool,
    pub items: Vec<NativeMenuEntry>,
}

impl NativeMenuGroup {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        items: Vec<NativeMenuEntry>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            enabled: true,
            visible: true,
            items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeMenuEntry {
    Command(NativeMenuCommand),
    Submenu(NativeMenuGroup),
    Separator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMenuCommand {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub visible: bool,
    pub state: NativeMenuState,
    pub shortcut: Option<NativeMenuShortcut>,
}

impl NativeMenuCommand {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            enabled: true,
            visible: true,
            state: NativeMenuState::Off,
            shortcut: None,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn enabled_if(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_state(mut self, state: NativeMenuState) -> Self {
        self.state = state;
        self
    }

    pub fn with_shortcut(mut self, shortcut: NativeMenuShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NativeMenuState {
    #[default]
    Off,
    On,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeMenuShortcut {
    /// AppKit key equivalent; use one character, including AppKit's private
    /// function-key characters when necessary.
    pub key: char,
    pub modifiers: NativeMenuModifiers,
}

impl NativeMenuShortcut {
    pub const fn new(key: char, modifiers: NativeMenuModifiers) -> Self {
        Self { key, modifiers }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeMenuModifiers {
    pub command: bool,
    pub control: bool,
    pub option: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeMenuError {
    EmptyId,
    DuplicateGroupId,
    WrongThread,
}

impl NativeMenuBar {
    pub fn validate(&self) -> Result<(), NativeMenuError> {
        let mut groups = std::collections::HashSet::new();
        if self
            .quit_command
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(NativeMenuError::EmptyId);
        }
        validate_entries(&self.application, &mut groups)?;
        for group in &self.menus {
            validate_group(group, &mut groups)?;
        }
        Ok(())
    }

    /// Whether two bars can update their existing native items in place.
    pub fn same_structure(&self, other: &Self) -> bool {
        entries_same_structure(&self.application, &other.application)
            && self.menus.len() == other.menus.len()
            && self
                .menus
                .iter()
                .zip(&other.menus)
                .all(|(left, right)| groups_same_structure(left, right))
    }
}

fn groups_same_structure(left: &NativeMenuGroup, right: &NativeMenuGroup) -> bool {
    left.id == right.id && entries_same_structure(&left.items, &right.items)
}

fn entries_same_structure(left: &[NativeMenuEntry], right: &[NativeMenuEntry]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (NativeMenuEntry::Command(left), NativeMenuEntry::Command(right)) => {
                    left.id == right.id
                }
                (NativeMenuEntry::Submenu(left), NativeMenuEntry::Submenu(right)) => {
                    groups_same_structure(left, right)
                }
                (NativeMenuEntry::Separator, NativeMenuEntry::Separator) => true,
                _ => false,
            })
}

fn validate_group<'a>(
    group: &'a NativeMenuGroup,
    groups: &mut std::collections::HashSet<&'a str>,
) -> Result<(), NativeMenuError> {
    if group.id.trim().is_empty() {
        return Err(NativeMenuError::EmptyId);
    }
    if !groups.insert(group.id.as_str()) {
        return Err(NativeMenuError::DuplicateGroupId);
    }
    validate_entries(&group.items, groups)
}

fn validate_entries<'a>(
    entries: &'a [NativeMenuEntry],
    groups: &mut std::collections::HashSet<&'a str>,
) -> Result<(), NativeMenuError> {
    for entry in entries {
        match entry {
            NativeMenuEntry::Command(command) if command.id.trim().is_empty() => {
                return Err(NativeMenuError::EmptyId);
            }
            NativeMenuEntry::Command(_) | NativeMenuEntry::Separator => {}
            NativeMenuEntry::Submenu(group) => validate_group(group, groups)?,
        }
    }
    Ok(())
}

/// Returns `Ok(false)` until AppKit has created the application's main menu.
#[cfg(target_os = "macos")]
pub fn sync(model: &NativeMenuBar) -> Result<bool, NativeMenuError> {
    model.validate()?;
    macos::sync(model)
}

/// Non-macOS hosts can share menu configuration without installing AppKit.
#[cfg(not(target_os = "macos"))]
pub fn sync(model: &NativeMenuBar) -> Result<bool, NativeMenuError> {
    model.validate()?;
    Ok(false)
}

#[cfg(target_os = "macos")]
pub fn take_commands() -> Vec<String> {
    macos::take_commands()
}

#[cfg(not(target_os = "macos"))]
pub fn take_commands() -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_groups_validate_and_reject_duplicate_ids() {
        let mut bar = NativeMenuBar {
            menus: vec![NativeMenuGroup::new(
                "file",
                "File",
                vec![NativeMenuEntry::Submenu(NativeMenuGroup::new(
                    "recent",
                    "Recent",
                    vec![NativeMenuEntry::Command(NativeMenuCommand::new(
                        "open", "Open",
                    ))],
                ))],
            )],
            ..Default::default()
        };
        assert_eq!(bar.validate(), Ok(()));
        bar.menus
            .push(NativeMenuGroup::new("recent", "Other", vec![]));
        assert_eq!(bar.validate(), Err(NativeMenuError::DuplicateGroupId));
    }

    #[test]
    fn presentation_changes_keep_structure_but_new_items_change_it() {
        let original = NativeMenuBar {
            menus: vec![NativeMenuGroup::new(
                "file",
                "File",
                vec![NativeMenuEntry::Command(NativeMenuCommand::new(
                    "save", "Save",
                ))],
            )],
            ..Default::default()
        };
        let mut changed = original.clone();
        changed.menus[0].title = "Fichier".into();
        if let NativeMenuEntry::Command(command) = &mut changed.menus[0].items[0] {
            command.title = "Enregistrer".into();
            command.enabled = false;
            command.state = NativeMenuState::On;
            command.shortcut = Some(NativeMenuShortcut::new(
                's',
                NativeMenuModifiers {
                    command: true,
                    ..Default::default()
                },
            ));
        }
        assert!(original.same_structure(&changed));
        changed.menus[0].items.push(NativeMenuEntry::Separator);
        assert!(!original.same_structure(&changed));
    }
}

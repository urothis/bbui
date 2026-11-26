//! Resolve a native menu's commands from BBUI's shared command registry.
//!
//! The menu template owns placement, separators, visibility, and check state.
//! The registry owns command labels, enabled state, and shortcuts.

use bbui_core::{UiKeyCode, UiModifiers};
use bbui_native_menu::{
    self, NativeMenuBar, NativeMenuEntry, NativeMenuError, NativeMenuGroup, NativeMenuModifiers,
    NativeMenuShortcut,
};
use bbui_widgets::{CommandRegistry, CommandShortcut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCommandMenuError {
    MissingCommand(String),
    UnsupportedShortcut(String),
    NativeMenu(NativeMenuError),
}

impl std::fmt::Display for NativeCommandMenuError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCommand(id) => write!(formatter, "menu command is not registered: {id}"),
            Self::UnsupportedShortcut(id) => {
                write!(formatter, "menu command has no AppKit key equivalent: {id}")
            }
            Self::NativeMenu(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeCommandMenuError {}

impl From<NativeMenuError> for NativeCommandMenuError {
    fn from(error: NativeMenuError) -> Self {
        Self::NativeMenu(error)
    }
}

/// Build a native menu from a placement template and current registry state.
///
/// Every command ID in the template, including `quit_command`, must be
/// registered. The template's command title, enabled flag, and shortcut are
/// replaced with registry values; visibility and check state remain local to
/// the menu. The system Quit item remains selectable so the host can present
/// an unsaved-work dialog before deciding whether to exit.
pub fn resolve_native_menu(
    template: &NativeMenuBar,
    registry: &CommandRegistry,
) -> Result<NativeMenuBar, NativeCommandMenuError> {
    template.validate()?;
    let mut menu = template.clone();
    if let Some(id) = &menu.quit_command
        && registry.get(id).is_none()
    {
        return Err(NativeCommandMenuError::MissingCommand(id.clone()));
    }
    resolve_entries(&mut menu.application, registry)?;
    for group in &mut menu.menus {
        resolve_group(group, registry)?;
    }
    Ok(menu)
}

/// Synchronize the menu with the same command state used by BBUI palettes.
/// Call on the macOS main thread after the application menu exists.
pub fn sync_native_menu_from_registry(
    template: &NativeMenuBar,
    registry: &CommandRegistry,
) -> Result<bool, NativeCommandMenuError> {
    let menu = resolve_native_menu(template, registry)?;
    Ok(bbui_native_menu::sync(&menu)?)
}

fn resolve_group(
    group: &mut NativeMenuGroup,
    registry: &CommandRegistry,
) -> Result<(), NativeCommandMenuError> {
    resolve_entries(&mut group.items, registry)
}

fn resolve_entries(
    entries: &mut [NativeMenuEntry],
    registry: &CommandRegistry,
) -> Result<(), NativeCommandMenuError> {
    for entry in entries {
        match entry {
            NativeMenuEntry::Command(item) => {
                let command = registry
                    .get(&item.id)
                    .ok_or_else(|| NativeCommandMenuError::MissingCommand(item.id.clone()))?;
                item.title.clone_from(&command.label);
                item.enabled = command.enabled;
                item.shortcut = command
                    .shortcut
                    .map(|shortcut| native_shortcut(shortcut, &item.id))
                    .transpose()?;
            }
            NativeMenuEntry::Submenu(group) => resolve_group(group, registry)?,
            NativeMenuEntry::Separator => {}
        }
    }
    Ok(())
}

fn native_shortcut(
    shortcut: CommandShortcut,
    command_id: &str,
) -> Result<NativeMenuShortcut, NativeCommandMenuError> {
    let key = appkit_key(shortcut.key)
        .ok_or_else(|| NativeCommandMenuError::UnsupportedShortcut(command_id.to_owned()))?;
    Ok(NativeMenuShortcut::new(
        key,
        native_modifiers(shortcut.modifiers),
    ))
}

fn native_modifiers(modifiers: UiModifiers) -> NativeMenuModifiers {
    NativeMenuModifiers {
        command: modifiers.super_key,
        control: modifiers.control,
        option: modifiers.alt,
        shift: modifiers.shift,
    }
}

fn appkit_key(key: UiKeyCode) -> Option<char> {
    Some(match key {
        UiKeyCode::A => 'a',
        UiKeyCode::B => 'b',
        UiKeyCode::C => 'c',
        UiKeyCode::D => 'd',
        UiKeyCode::E => 'e',
        UiKeyCode::F => 'f',
        UiKeyCode::G => 'g',
        UiKeyCode::H => 'h',
        UiKeyCode::I => 'i',
        UiKeyCode::J => 'j',
        UiKeyCode::K => 'k',
        UiKeyCode::L => 'l',
        UiKeyCode::M => 'm',
        UiKeyCode::N => 'n',
        UiKeyCode::O => 'o',
        UiKeyCode::P => 'p',
        UiKeyCode::Q => 'q',
        UiKeyCode::R => 'r',
        UiKeyCode::S => 's',
        UiKeyCode::T => 't',
        UiKeyCode::U => 'u',
        UiKeyCode::V => 'v',
        UiKeyCode::W => 'w',
        UiKeyCode::X => 'x',
        UiKeyCode::Y => 'y',
        UiKeyCode::Z => 'z',
        UiKeyCode::Digit0 => '0',
        UiKeyCode::Digit1 => '1',
        UiKeyCode::Digit2 => '2',
        UiKeyCode::Digit3 => '3',
        UiKeyCode::Digit4 => '4',
        UiKeyCode::Digit5 => '5',
        UiKeyCode::Digit6 => '6',
        UiKeyCode::Digit7 => '7',
        UiKeyCode::Digit8 => '8',
        UiKeyCode::Digit9 => '9',
        UiKeyCode::Comma => ',',
        UiKeyCode::Backquote => '`',
        UiKeyCode::Backslash => '\\',
        UiKeyCode::BracketLeft => '[',
        UiKeyCode::BracketRight => ']',
        UiKeyCode::Equal => '=',
        UiKeyCode::Minus => '-',
        UiKeyCode::Period => '.',
        UiKeyCode::Quote => '\'',
        UiKeyCode::Semicolon => ';',
        UiKeyCode::Slash => '/',
        UiKeyCode::Tab => '\t',
        UiKeyCode::Escape => '\u{1b}',
        UiKeyCode::Enter => '\r',
        UiKeyCode::Space => ' ',
        UiKeyCode::Backspace => '\u{8}',
        UiKeyCode::Delete => '\u{7f}',
        // AppKit's NS*FunctionKey values (Unicode private-use range).
        UiKeyCode::ArrowUp => '\u{f700}',
        UiKeyCode::ArrowDown => '\u{f701}',
        UiKeyCode::ArrowLeft => '\u{f702}',
        UiKeyCode::ArrowRight => '\u{f703}',
        UiKeyCode::F1 => '\u{f704}',
        UiKeyCode::F2 => '\u{f705}',
        UiKeyCode::F3 => '\u{f706}',
        UiKeyCode::F4 => '\u{f707}',
        UiKeyCode::F5 => '\u{f708}',
        UiKeyCode::F6 => '\u{f709}',
        UiKeyCode::F7 => '\u{f70a}',
        UiKeyCode::F8 => '\u{f70b}',
        UiKeyCode::F9 => '\u{f70c}',
        UiKeyCode::F10 => '\u{f70d}',
        UiKeyCode::F11 => '\u{f70e}',
        UiKeyCode::F12 => '\u{f70f}',
        UiKeyCode::F13 => '\u{f710}',
        UiKeyCode::F14 => '\u{f711}',
        UiKeyCode::F15 => '\u{f712}',
        UiKeyCode::F16 => '\u{f713}',
        UiKeyCode::F17 => '\u{f714}',
        UiKeyCode::F18 => '\u{f715}',
        UiKeyCode::F19 => '\u{f716}',
        UiKeyCode::F20 => '\u{f717}',
        UiKeyCode::F21 => '\u{f718}',
        UiKeyCode::F22 => '\u{f719}',
        UiKeyCode::F23 => '\u{f71a}',
        UiKeyCode::F24 => '\u{f71b}',
        UiKeyCode::F25 => '\u{f71c}',
        UiKeyCode::F26 => '\u{f71d}',
        UiKeyCode::F27 => '\u{f71e}',
        UiKeyCode::F28 => '\u{f71f}',
        UiKeyCode::F29 => '\u{f720}',
        UiKeyCode::F30 => '\u{f721}',
        UiKeyCode::F31 => '\u{f722}',
        UiKeyCode::F32 => '\u{f723}',
        UiKeyCode::F33 => '\u{f724}',
        UiKeyCode::F34 => '\u{f725}',
        UiKeyCode::F35 => '\u{f726}',
        UiKeyCode::Home => '\u{f729}',
        UiKeyCode::End => '\u{f72b}',
        UiKeyCode::PageUp => '\u{f72c}',
        UiKeyCode::PageDown => '\u{f72d}',
        UiKeyCode::Other(_) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bbui_native_menu::{NativeMenuCommand, NativeMenuState};
    use bbui_widgets::Command;

    #[test]
    fn punctuation_and_function_keys_have_native_equivalents() {
        for (key, native) in [
            (UiKeyCode::Backquote, '`'),
            (UiKeyCode::Backslash, '\\'),
            (UiKeyCode::BracketLeft, '['),
            (UiKeyCode::BracketRight, ']'),
            (UiKeyCode::Equal, '='),
            (UiKeyCode::Minus, '-'),
            (UiKeyCode::Period, '.'),
            (UiKeyCode::Quote, '\''),
            (UiKeyCode::Semicolon, ';'),
            (UiKeyCode::Slash, '/'),
            (UiKeyCode::F1, '\u{f704}'),
            (UiKeyCode::F12, '\u{f70f}'),
            (UiKeyCode::F35, '\u{f726}'),
        ] {
            assert_eq!(appkit_key(key), Some(native));
        }
    }

    #[test]
    fn registry_controls_menu_labels_availability_and_shortcuts() {
        let mut registry = CommandRegistry::default();
        let mut save = Command::new("save", "Save Document");
        save.enabled = false;
        save.shortcut = Some(CommandShortcut::new(
            UiKeyCode::S,
            UiModifiers {
                super_key: true,
                ..Default::default()
            },
        ));
        assert!(registry.register(save).is_ok());
        let template = NativeMenuBar {
            menus: vec![NativeMenuGroup::new(
                "file",
                "File",
                vec![NativeMenuEntry::Command(
                    NativeMenuCommand::new("save", "stale title").with_state(NativeMenuState::On),
                )],
            )],
            ..Default::default()
        };
        let resolved = resolve_native_menu(&template, &registry).unwrap();
        let NativeMenuEntry::Command(item) = &resolved.menus[0].items[0] else {
            panic!("expected command");
        };
        assert_eq!(item.title, "Save Document");
        assert!(!item.enabled);
        assert_eq!(item.state, NativeMenuState::On);
        assert_eq!(item.shortcut.as_ref().map(|s| s.key), Some('s'));
        assert!(item.shortcut.as_ref().is_some_and(|s| s.modifiers.command));
        assert!(registry.set_label("save", "Save Changes"));
        assert!(registry.set_enabled("save", true));
        let updated = resolve_native_menu(&template, &registry).unwrap();
        let NativeMenuEntry::Command(updated_item) = &updated.menus[0].items[0] else {
            panic!("expected command");
        };
        assert_eq!(updated_item.title, "Save Changes");
        assert!(updated_item.enabled);
        assert_eq!(
            template.menus[0].items[0],
            NativeMenuEntry::Command(
                NativeMenuCommand::new("save", "stale title").with_state(NativeMenuState::On)
            )
        );
    }

    #[test]
    fn missing_commands_and_opaque_shortcuts_fail_explicitly() {
        let template = NativeMenuBar {
            application: vec![NativeMenuEntry::Command(NativeMenuCommand::new("x", ""))],
            ..Default::default()
        };
        let mut registry = CommandRegistry::default();
        assert_eq!(
            resolve_native_menu(&template, &registry),
            Err(NativeCommandMenuError::MissingCommand("x".into()))
        );
        let mut command = Command::new("x", "X");
        command.shortcut = Some(CommandShortcut::new(
            UiKeyCode::Other(42),
            UiModifiers::default(),
        ));
        assert!(registry.register(command).is_ok());
        assert_eq!(
            resolve_native_menu(&template, &registry),
            Err(NativeCommandMenuError::UnsupportedShortcut("x".into()))
        );
    }
}

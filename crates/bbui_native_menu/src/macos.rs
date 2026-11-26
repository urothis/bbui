use super::{
    NativeMenuBar, NativeMenuCommand, NativeMenuEntry, NativeMenuError, NativeMenuGroup,
    NativeMenuModifiers, NativeMenuState,
};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueMixed, NSControlStateValueOff, NSControlStateValueOn,
    NSEventModifierFlags, NSMenu, NSMenuItem, NSMenuItemValidation,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr;

thread_local! {
    static INSTALLED: RefCell<Option<InstalledMenu>> = const { RefCell::new(None) };
    static COMMAND_IDS: RefCell<HashMap<isize, CommandBinding>> = RefCell::new(HashMap::new());
    static SELECTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

struct CommandBinding {
    id: String,
    enabled: bool,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct MenuTarget;

    unsafe impl NSObjectProtocol for MenuTarget {}
    unsafe impl NSMenuItemValidation for MenuTarget {
        // The Objective-C protocol requires this selector's spelling.
        #[allow(non_snake_case)]
        #[unsafe(method(validateMenuItem:))]
        fn validateMenuItem(&self, sender: &NSMenuItem) -> bool {
            COMMAND_IDS.with_borrow(|ids| ids.get(&sender.tag()).is_some_and(|command| command.enabled))
        }
    }

    impl MenuTarget {
        #[unsafe(method(performBbuiNativeCommand:))]
        fn perform_command(&self, sender: &NSMenuItem) {
            COMMAND_IDS.with_borrow(|ids| {
                if let Some(command) = ids.get(&sender.tag()).filter(|command| command.enabled) {
                    SELECTED.with_borrow_mut(|selected| selected.push(command.id.clone()));
                }
            });
        }
    }
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's designated initializer returns the retained object.
        unsafe { msg_send![super(this), init] }
    }
}

enum InstalledEntry {
    Command {
        item: Retained<NSMenuItem>,
        tag: isize,
    },
    Submenu {
        item: Retained<NSMenuItem>,
        menu: Retained<NSMenu>,
        children: Vec<InstalledEntry>,
    },
    Separator {
        item: Retained<NSMenuItem>,
    },
}

impl InstalledEntry {
    fn item(&self) -> &NSMenuItem {
        match self {
            Self::Command { item, .. } | Self::Submenu { item, .. } | Self::Separator { item } => {
                item
            }
        }
    }
}

struct OriginalQuit {
    item: Retained<NSMenuItem>,
    target: Option<Retained<AnyObject>>,
    action: Option<Sel>,
    tag: isize,
}

struct InstalledMenu {
    model: NativeMenuBar,
    root: Retained<NSMenu>,
    app_menu: Retained<NSMenu>,
    application: Vec<InstalledEntry>,
    menus: Vec<InstalledEntry>,
    original_quit: Option<OriginalQuit>,
    _target: Retained<MenuTarget>,
}

fn modifiers(value: NativeMenuModifiers) -> NSEventModifierFlags {
    (if value.command {
        NSEventModifierFlags::Command
    } else {
        NSEventModifierFlags::empty()
    }) | (if value.control {
        NSEventModifierFlags::Control
    } else {
        NSEventModifierFlags::empty()
    }) | (if value.option {
        NSEventModifierFlags::Option
    } else {
        NSEventModifierFlags::empty()
    }) | (if value.shift {
        NSEventModifierFlags::Shift
    } else {
        NSEventModifierFlags::empty()
    })
}

fn update_command(item: &NSMenuItem, command: &NativeMenuCommand) {
    item.setTitle(&NSString::from_str(&command.title));
    item.setEnabled(command.enabled);
    item.setHidden(!command.visible);
    item.setState(match command.state {
        NativeMenuState::Off => NSControlStateValueOff,
        NativeMenuState::On => NSControlStateValueOn,
        NativeMenuState::Mixed => NSControlStateValueMixed,
    });
    let key = command
        .shortcut
        .as_ref()
        .map_or_else(String::new, |shortcut| shortcut.key.to_string());
    item.setKeyEquivalent(&NSString::from_str(&key));
    item.setKeyEquivalentModifierMask(
        command
            .shortcut
            .as_ref()
            .map_or_else(NSEventModifierFlags::empty, |shortcut| {
                modifiers(shortcut.modifiers)
            }),
    );
}

fn update_group(item: &NSMenuItem, menu: &NSMenu, group: &NativeMenuGroup) {
    let title = NSString::from_str(&group.title);
    item.setTitle(&title);
    item.setEnabled(group.enabled);
    item.setHidden(!group.visible);
    menu.setTitle(&title);
}

fn build_entry(
    mtm: MainThreadMarker,
    entry: &NativeMenuEntry,
    target: &MenuTarget,
    next_tag: &mut isize,
) -> InstalledEntry {
    match entry {
        NativeMenuEntry::Command(command) => {
            let item = NSMenuItem::new(mtm);
            item.setTag(*next_tag);
            *next_tag += 1;
            // SAFETY: The target is retained by InstalledMenu while this item exists.
            unsafe {
                item.setTarget(Some(target));
                item.setAction(Some(sel!(performBbuiNativeCommand:)));
            }
            update_command(&item, command);
            InstalledEntry::Command {
                tag: item.tag(),
                item,
            }
        }
        NativeMenuEntry::Submenu(group) => {
            let item = NSMenuItem::new(mtm);
            let menu = NSMenu::new(mtm);
            menu.setAutoenablesItems(false);
            update_group(&item, &menu, group);
            item.setSubmenu(Some(&menu));
            let children = group
                .items
                .iter()
                .map(|child| {
                    let installed = build_entry(mtm, child, target, next_tag);
                    menu.addItem(installed.item());
                    installed
                })
                .collect();
            InstalledEntry::Submenu {
                item,
                menu,
                children,
            }
        }
        NativeMenuEntry::Separator => InstalledEntry::Separator {
            item: NSMenuItem::separatorItem(mtm),
        },
    }
}

fn update_entry(entry: &NativeMenuEntry, installed: &InstalledEntry) {
    match (entry, installed) {
        (NativeMenuEntry::Command(command), InstalledEntry::Command { item, .. }) => {
            update_command(item, command)
        }
        (
            NativeMenuEntry::Submenu(group),
            InstalledEntry::Submenu {
                item,
                menu,
                children,
            },
        ) => {
            update_group(item, menu, group);
            for (entry, child) in group.items.iter().zip(children) {
                update_entry(entry, child);
            }
        }
        _ => {}
    }
}

fn collect_ids(
    entry: &NativeMenuEntry,
    installed: &InstalledEntry,
    ids: &mut HashMap<isize, CommandBinding>,
) {
    match (entry, installed) {
        (NativeMenuEntry::Command(command), InstalledEntry::Command { tag, .. }) => {
            ids.insert(
                *tag,
                CommandBinding {
                    id: command.id.clone(),
                    enabled: command.enabled,
                },
            );
        }
        (NativeMenuEntry::Submenu(group), InstalledEntry::Submenu { children, .. }) => {
            for (entry, child) in group.items.iter().zip(children) {
                collect_ids(entry, child, ids);
            }
        }
        _ => {}
    }
}

impl InstalledMenu {
    fn new(mtm: MainThreadMarker, root: Retained<NSMenu>, model: &NativeMenuBar) -> Option<Self> {
        let app_menu = root.itemAtIndex(0)?.submenu()?;
        let target = MenuTarget::new(mtm);
        let mut next_tag = 1;
        let mut application = Vec::new();
        for (index, entry) in model.application.iter().enumerate() {
            let installed = build_entry(mtm, entry, &target, &mut next_tag);
            app_menu.insertItem_atIndex(installed.item(), index as isize + 1);
            application.push(installed);
        }
        let next_is_separator = app_menu
            .itemAtIndex(application.len() as isize + 1)
            .is_some_and(|item| item.isSeparatorItem());
        if !application.is_empty()
            && !next_is_separator
            && !matches!(model.application.last(), Some(NativeMenuEntry::Separator))
        {
            let separator = InstalledEntry::Separator {
                item: NSMenuItem::separatorItem(mtm),
            };
            app_menu.insertItem_atIndex(separator.item(), application.len() as isize + 1);
            application.push(separator);
        }
        let mut menus = Vec::new();
        for group in &model.menus {
            let entry = NativeMenuEntry::Submenu(group.clone());
            let installed = build_entry(mtm, &entry, &target, &mut next_tag);
            root.addItem(installed.item());
            menus.push(installed);
        }
        let original_quit = (0..app_menu.numberOfItems()).find_map(|index| {
            let item = app_menu.itemAtIndex(index)?;
            (item.action() == Some(sel!(terminate:))).then(|| OriginalQuit {
                target: item.target(),
                action: item.action(),
                tag: item.tag(),
                item,
            })
        });
        let mut installed = Self {
            model: model.clone(),
            root,
            app_menu,
            application,
            menus,
            original_quit,
            _target: target,
        };
        installed.update_quit();
        installed.refresh_ids();
        Some(installed)
    }

    fn same_shape(&self, model: &NativeMenuBar) -> bool {
        self.model.same_structure(model)
    }

    fn update(&mut self, model: &NativeMenuBar) {
        for (entry, installed) in model.application.iter().zip(&self.application) {
            update_entry(entry, installed);
        }
        for (group, installed) in model.menus.iter().zip(&self.menus) {
            update_entry(&NativeMenuEntry::Submenu(group.clone()), installed);
        }
        self.model = model.clone();
        self.update_quit();
        self.refresh_ids();
    }

    fn update_quit(&mut self) {
        let Some(original) = &self.original_quit else {
            return;
        };
        if self.model.quit_command.is_some() {
            original.item.setTag(-1);
            // SAFETY: The target remains retained until this item is restored.
            unsafe {
                original.item.setTarget(Some(&*self._target));
                original
                    .item
                    .setAction(Some(sel!(performBbuiNativeCommand:)));
            }
        } else {
            self.restore_quit();
        }
    }

    fn restore_quit(&self) {
        let Some(original) = &self.original_quit else {
            return;
        };
        original.item.setTag(original.tag);
        // SAFETY: The original AppKit target/action were retained before interception.
        unsafe {
            original.item.setTarget(original.target.as_deref());
            original.item.setAction(original.action);
        }
    }

    fn refresh_ids(&self) {
        let mut ids = HashMap::new();
        for (entry, installed) in self.model.application.iter().zip(&self.application) {
            collect_ids(entry, installed, &mut ids);
        }
        for (group, installed) in self.model.menus.iter().zip(&self.menus) {
            collect_ids(
                &NativeMenuEntry::Submenu(group.clone()),
                installed,
                &mut ids,
            );
        }
        if let Some(quit) = &self.model.quit_command {
            ids.insert(
                -1,
                CommandBinding {
                    id: quit.clone(),
                    enabled: true,
                },
            );
        }
        COMMAND_IDS.with_borrow_mut(|shared| *shared = ids);
    }

    fn remove(&self) {
        self.restore_quit();
        for entry in &self.application {
            self.app_menu.removeItem(entry.item());
        }
        for entry in &self.menus {
            self.root.removeItem(entry.item());
        }
    }
}

pub(super) fn sync(model: &NativeMenuBar) -> Result<bool, NativeMenuError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(NativeMenuError::WrongThread);
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(root) = app.mainMenu() else {
        return Ok(false);
    };
    INSTALLED.with_borrow_mut(|state| {
        if state.as_ref().is_some_and(|installed| {
            ptr::eq(&*installed.root, &*root) && installed.same_shape(model)
        }) {
            if let Some(installed) = state.as_mut()
                && installed.model != *model
            {
                installed.update(model);
            }
            return Ok(true);
        }
        if let Some(installed) = state.take() {
            installed.remove();
        }
        let installed = InstalledMenu::new(mtm, root, model);
        let available = installed.is_some();
        *state = installed;
        Ok(available)
    })
}

pub(super) fn take_commands() -> Vec<String> {
    SELECTED.with_borrow_mut(std::mem::take)
}

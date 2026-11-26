//! macOS application menus backed by the same commands as a BBUI command palette.

#[cfg(target_os = "macos")]
mod macos {
    use bbui::native_menu::{
        self, NativeMenuBar, NativeMenuCommand, NativeMenuEntry as Entry, NativeMenuGroup,
        NativeMenuState,
    };
    use bbui::native_menu_commands::sync_native_menu_from_registry;
    use bbui::widgets::{Command, CommandRegistry, CommandShortcut};
    use bbui_core::{UiKeyCode, UiModifiers};
    use bevy::prelude::*;

    #[derive(Resource)]
    struct Demo {
        edited: bool,
        show_grid: bool,
        commands: CommandRegistry,
        menu_dirty: bool,
        menu_installed: bool,
    }

    impl Default for Demo {
        fn default() -> Self {
            let mut commands = CommandRegistry::default();
            for (id, label, key) in [
                ("settings", "Settings…", Some(UiKeyCode::Comma)),
                ("new", "New", Some(UiKeyCode::N)),
                ("save", "Save", Some(UiKeyCode::S)),
                ("recent.example", "Example Document", None),
                ("grid", "Show Grid", None),
                ("quit", "Quit", Some(UiKeyCode::Q)),
            ] {
                let mut command = Command::new(id, label);
                if let Some(key) = key {
                    command = command.with_shortcut(
                        CommandShortcut::new(
                            key,
                            UiModifiers {
                                super_key: true,
                                ..Default::default()
                            },
                        ),
                        format!("⌘{key:?}"),
                    );
                }
                assert!(commands.register(command).is_ok(), "unique example command");
            }
            commands.set_enabled("save", false);
            Self {
                edited: false,
                show_grid: false,
                commands,
                menu_dirty: true,
                menu_installed: false,
            }
        }
    }

    struct MainThreadToken;

    fn command(id: &str) -> Entry {
        Entry::Command(NativeMenuCommand::new(id, ""))
    }

    fn model(demo: &Demo) -> NativeMenuBar {
        NativeMenuBar {
            application: vec![command("settings")],
            menus: vec![
                NativeMenuGroup::new(
                    "file",
                    "File",
                    vec![
                        command("new"),
                        Entry::Separator,
                        command("save"),
                        Entry::Submenu(NativeMenuGroup::new(
                            "recent",
                            "Open Recent",
                            vec![command("recent.example")],
                        )),
                    ],
                ),
                NativeMenuGroup::new(
                    "view",
                    "View",
                    vec![Entry::Command(
                        NativeMenuCommand::new("grid", "").with_state(if demo.show_grid {
                            NativeMenuState::On
                        } else {
                            NativeMenuState::Off
                        }),
                    )],
                ),
            ],
            quit_command: Some("quit".into()),
        }
    }

    fn update(
        _main_thread: NonSend<MainThreadToken>,
        mut demo: ResMut<Demo>,
        mut exits: MessageWriter<AppExit>,
    ) {
        for id in native_menu::take_commands() {
            if !demo.commands.record_activation(&id) {
                continue;
            }
            match id.as_str() {
                "new" => {
                    demo.edited = true;
                    demo.menu_dirty = true;
                }
                "save" => {
                    demo.edited = false;
                    demo.menu_dirty = true;
                }
                "grid" => {
                    demo.show_grid = !demo.show_grid;
                    demo.menu_dirty = true;
                }
                "quit" if !demo.edited => {
                    exits.write(AppExit::Success);
                }
                "quit" => info!("Save before quitting"),
                _ => info!("Selected {id}"),
            }
        }
        // AppKit may create the application menu after the first update. Once
        // installed, rebuild only when command or checkmark state changes.
        if demo.menu_dirty || !demo.menu_installed {
            let edited = demo.edited;
            demo.commands.set_enabled("save", edited);
            match sync_native_menu_from_registry(&model(&demo), &demo.commands) {
                Ok(true) => {
                    demo.menu_dirty = false;
                    demo.menu_installed = true;
                }
                Ok(false) if demo.menu_installed => demo.menu_dirty = false,
                Ok(false) => {}
                Err(error) => warn!("Native menu update failed: {error:?}"),
            }
        }
    }

    pub fn run() {
        App::new()
            .add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "BBUI native menu example".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }))
            .insert_non_send(MainThreadToken)
            .init_resource::<Demo>()
            .add_systems(Update, update)
            .run();
    }
}

#[cfg(target_os = "macos")]
fn main() {
    macos::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("Run this example on macOS to inspect the native application menu.");
}

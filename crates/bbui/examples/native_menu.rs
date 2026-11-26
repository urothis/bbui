//! macOS application menus driven by host state. Run on macOS and inspect the menu bar.

#[cfg(target_os = "macos")]
mod macos {
    use bbui::native_menu::{
        self, NativeMenuBar, NativeMenuCommand, NativeMenuEntry as Entry, NativeMenuGroup,
        NativeMenuModifiers, NativeMenuShortcut, NativeMenuState,
    };
    use bevy::prelude::*;

    #[derive(Resource, Default)]
    struct Demo {
        edited: bool,
        show_grid: bool,
    }

    fn command(id: &str, title: &str, key: Option<char>) -> Entry {
        let mut command = NativeMenuCommand::new(id, title);
        if let Some(key) = key {
            command = command.with_shortcut(NativeMenuShortcut::new(
                key,
                NativeMenuModifiers {
                    command: true,
                    ..Default::default()
                },
            ));
        }
        Entry::Command(command)
    }

    fn model(demo: &Demo) -> NativeMenuBar {
        NativeMenuBar {
            application: vec![command("settings", "Settings…", Some(','))],
            menus: vec![
                NativeMenuGroup::new(
                    "file",
                    "File",
                    vec![
                        command("new", "New", Some('n')),
                        Entry::Separator,
                        Entry::Command(
                            NativeMenuCommand::new("save", "Save")
                                .with_shortcut(NativeMenuShortcut::new(
                                    's',
                                    NativeMenuModifiers {
                                        command: true,
                                        ..Default::default()
                                    },
                                ))
                                .enabled_if(demo.edited),
                        ),
                        Entry::Submenu(NativeMenuGroup::new(
                            "recent",
                            "Open Recent",
                            vec![command("recent.example", "Example Document", None)],
                        )),
                    ],
                ),
                NativeMenuGroup::new(
                    "view",
                    "View",
                    vec![Entry::Command(
                        NativeMenuCommand::new("grid", "Show Grid").with_state(if demo.show_grid {
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

    fn update(mut demo: ResMut<Demo>, mut exits: MessageWriter<AppExit>) {
        for id in native_menu::take_commands() {
            match id.as_str() {
                "new" => demo.edited = true,
                "save" => demo.edited = false,
                "grid" => demo.show_grid = !demo.show_grid,
                "quit" if !demo.edited => {
                    exits.write(AppExit::Success);
                }
                "quit" => info!("Save before quitting"),
                _ => info!("Selected {id}"),
            }
        }
        if let Err(error) = native_menu::sync(&model(&demo)) {
            warn!("Native menu update failed: {error:?}");
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

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
    use bbui_text::ui_default_text_font;
    use bevy::prelude::*;

    #[derive(Resource)]
    struct Demo {
        edited: bool,
        show_grid: bool,
        alternate_save: bool,
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
                ("switch-save", "Use Alternate Save", None),
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
                alternate_save: false,
                commands,
                menu_dirty: true,
                menu_installed: false,
            }
        }
    }

    #[derive(Component)]
    struct StatusText;

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
                        Entry::Command(NativeMenuCommand::new("switch-save", "").with_state(
                            if demo.alternate_save {
                                NativeMenuState::On
                            } else {
                                NativeMenuState::Off
                            },
                        )),
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
        mut status: Query<&mut Text, With<StatusText>>,
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
                "switch-save" => {
                    demo.alternate_save = !demo.alternate_save;
                    let alternate = demo.alternate_save;
                    demo.commands
                        .set_label("save", if alternate { "Save Changes" } else { "Save" });
                    let shortcut = CommandShortcut::new(
                        UiKeyCode::S,
                        UiModifiers {
                            super_key: true,
                            shift: alternate,
                            ..Default::default()
                        },
                    );
                    if let Err(error) = demo.commands.set_shortcut(
                        "save",
                        Some(shortcut),
                        Some(if alternate { "⌘⇧S" } else { "⌘S" }.into()),
                    ) {
                        warn!("Save shortcut update failed: {error:?}");
                    }
                    demo.menu_dirty = true;
                }
                "quit" if !demo.edited => {
                    exits.write(AppExit::Success);
                }
                "quit" => info!("Save before quitting"),
                _ => info!("Selected {id}"),
            }
        }
        if let Ok(mut text) = status.single_mut() {
            let next = format!(
                "DOCUMENT  {}\nGRID  {}\nSAVE  {}",
                if demo.edited {
                    "Unsaved changes"
                } else {
                    "Saved"
                },
                if demo.show_grid { "On" } else { "Off" },
                if demo.alternate_save {
                    "Save Changes  ⌘⇧S"
                } else {
                    "Save  ⌘S"
                },
            );
            if text.0 != next {
                text.0 = next;
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

    fn setup(mut commands: Commands) {
        commands.spawn(Camera2d);
        commands
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.075, 0.11)),
            ))
            .with_children(|root| {
                root.spawn((
                    Node {
                        width: px(620),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(18),
                        padding: UiRect::all(px(32)),
                        border_radius: BorderRadius::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.105, 0.14, 0.2)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("BBUI  /  macOS"),
                        ui_default_text_font(16.0),
                        TextColor(Color::srgb(0.4, 0.85, 0.85)),
                    ));
                    card.spawn((
                        Text::new("Native menus, shared commands"),
                        ui_default_text_font(34.0),
                        TextColor(Color::WHITE),
                    ));
                    card.spawn((
                        Text::new(
                            "One CommandRegistry supplies menu labels, availability, and shortcuts. File > New enables Save; Use Alternate Save changes its label and shortcut at runtime. View toggles a checkmark; Quit goes through the host.",
                        ),
                        ui_default_text_font(18.0),
                        TextColor(Color::srgb(0.72, 0.78, 0.86)),
                    ));
                    card.spawn((
                        Text::new("DOCUMENT  Saved\nGRID  Off\nSAVE  Save  ⌘S"),
                        ui_default_text_font(20.0),
                        TextColor(Color::srgb(0.4, 0.85, 0.85)),
                        StatusText,
                    ));
                });
            });
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
            .add_plugins(bbui::BbuiPlugin)
            .insert_non_send(MainThreadToken)
            .init_resource::<Demo>()
            .add_systems(Startup, setup)
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

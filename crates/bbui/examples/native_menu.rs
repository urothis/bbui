//! macOS application menus driven by host state. Run on macOS and inspect the menu bar.

#[cfg(target_os = "macos")]
mod macos {
    use bbui::native_menu::{
        self, NativeMenuBar, NativeMenuCommand, NativeMenuEntry as Entry, NativeMenuGroup,
        NativeMenuModifiers, NativeMenuShortcut, NativeMenuState,
    };
    use bbui_text::ui_default_text_font;
    use bevy::prelude::*;

    #[derive(Resource, Default)]
    struct Demo {
        edited: bool,
        show_grid: bool,
    }

    #[derive(Component)]
    struct StatusText;

    struct MainThreadToken;

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

    fn update(
        _main_thread: NonSend<MainThreadToken>,
        mut demo: ResMut<Demo>,
        mut exits: MessageWriter<AppExit>,
        mut status: Query<&mut Text, With<StatusText>>,
    ) {
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
        if let Ok(mut text) = status.single_mut() {
            let next = format!(
                "DOCUMENT  {}\nGRID  {}",
                if demo.edited {
                    "Unsaved changes"
                } else {
                    "Saved"
                },
                if demo.show_grid { "On" } else { "Off" },
            );
            if text.0 != next {
                text.0 = next;
            }
        }
        if let Err(error) = native_menu::sync(&model(&demo)) {
            warn!("Native menu update failed: {error:?}");
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
                        Text::new("Native application menus"),
                        ui_default_text_font(34.0),
                        TextColor(Color::WHITE),
                    ));
                    card.spawn((
                        Text::new(
                            "Choose File > New to enable Save. Open Recent is nested; View > Show Grid toggles its checkmark. Quit asks the host before exiting.",
                        ),
                        ui_default_text_font(18.0),
                        TextColor(Color::srgb(0.72, 0.78, 0.86)),
                    ));
                    card.spawn((
                        Text::new("DOCUMENT  Saved\nGRID  Off"),
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

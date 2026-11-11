//! Run with `cargo run -p bbui --example file_browser`.
//!
//! Double-click a row to open it. Use Up/Down and Enter for keyboard navigation, PageUp/PageDown
//! for larger moves, Shift+Up/Down for range selection, Ctrl+A to select visible entries, and
//! Ctrl-click for discontiguous selection. Backspace goes up when the filter is empty, and Escape
//! clears the filter. The toolbar exposes sorting, list/grid view, bookmark add/remove, folder
//! creation, rename, copy, move, delete, and explicit operation errors.

use bbui::prelude::*;
use bbui::widgets::actions::WidgetValue;
use bbui::widgets::{
    Button as WidgetButton, Dropdown, FileSort, FileView, Label as BbuiLabel, WidgetAction,
    WidgetActionKind, WidgetActionQueue, WidgetEvent, WidgetEventQueue, WidgetNode,
};
use bbui_layout::{Rect, UiEdges};
use bbui_render::{ClipRect, Primitive, UiPaint};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Resource)]
struct BrowserUi {
    browser: Entity,
    controls: HashMap<String, Entity>,
}

#[derive(Resource, Default)]
struct PendingDelete(Option<Vec<PathBuf>>);

#[derive(Resource)]
struct DemoDirectory(PathBuf);

impl Drop for DemoDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn demo_directory() -> std::io::Result<PathBuf> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("bbui-demo-{nonce:x}"));
    std::fs::create_dir(&root)?;
    std::fs::create_dir(root.join("Projects"))?;
    let sample = root.join("Welcome.txt");
    if !sample.exists() {
        std::fs::write(
            &sample,
            b"This is a disposable BBUI file-browser example.\n",
        )?;
    }
    root.canonicalize()
}

#[derive(Resource, Default)]
struct LocationChoices {
    paths: Vec<PathBuf>,
    displayed_directory: Option<PathBuf>,
}

#[derive(SystemParam)]
struct BrowserControlQueries<'w, 's> {
    fields: Query<'w, 's, &'static TextField>,
    browsers: Query<'w, 's, &'static FileBrowser>,
    labels: Query<'w, 's, &'static mut BbuiLabel>,
}

fn button(label: &str, key: &str) -> WidgetNode {
    WidgetNode::button(label).with_key(key)
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);
    let Ok(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
    else {
        eprintln!("file browser example requires a window");
        return;
    };
    let background = world.resource::<Theme>().background;
    let root = world
        .spawn((
            UiNode,
            UiLayoutNode {
                padding: UiEdges::all(12.0),
                gap: 4.0,
                ..default()
            },
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 800.0, 640.0), 1.0),
            UiPaint(Primitive::RoundedRect {
                rect: ClipRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                radius: 0.0,
                color: background,
            }),
        ))
        .id();
    let Ok(directory) = demo_directory() else {
        eprintln!("file browser example cannot create its temporary demo directory");
        return;
    };
    world.insert_resource(DemoDirectory(directory.clone()));
    let tree = WidgetNode::column([
        WidgetNode::row([
            WidgetNode::dropdown(
                "Ancestors and bookmarks",
                [directory.display().to_string()],
                Some(0),
            )
            .with_key("locations")
            .with_width(UiLength::Px(180.0)),
            WidgetNode::text_field("Path", directory.display().to_string())
                .with_key("path")
                .with_width(UiLength::Px(430.0)),
            button("Go", "go").with_width(UiLength::Px(60.0)),
        ]),
        WidgetNode::row([
            button("Up", "up"),
            button("Refresh", "refresh"),
            button("Bookmark", "bookmark"),
            button("Remove bookmark", "remove_bookmark"),
            button("List / Grid", "view"),
        ]),
        WidgetNode::row([
            button("Sort", "sort"),
            button("New folder", "new"),
            button("Rename selected", "rename"),
            button("Delete selected", "delete"),
        ]),
        WidgetNode::row([
            button("Copy selected", "copy"),
            button("Move selected", "move"),
            WidgetNode::text_field("Name", "New Folder").with_key("name"),
            WidgetNode::text_field("Destination", "").with_key("destination"),
        ]),
        WidgetNode::text_field("Filter", "").with_key("filter"),
        WidgetNode::label(
            "Demo files live in a temporary directory. Delete requires a second click.",
        )
        .with_key("status"),
        WidgetNode::file_browser(directory.clone()).with_key("browser"),
    ]);
    let Ok(container) = WidgetTree::new(tree).spawn(world, Some(root)) else {
        eprintln!("file browser cannot read the current directory");
        return;
    };
    if let Some(mut layout) = world.get_mut::<UiLayoutNode>(container) {
        layout.size.width = UiLength::Fill(1.0);
        layout.size.height = UiLength::Fill(1.0);
    }
    let controls: HashMap<_, _> = world
        .query::<(Entity, &UiKey)>()
        .iter(world)
        .map(|(entity, key)| (key.0.clone(), entity))
        .collect();
    let browser = controls["browser"];
    if let Some(mut browser_state) = world.get_mut::<FileBrowser>(browser)
        && let Err(error) = browser_state.confine_to(&directory)
    {
        eprintln!("file browser demo confinement failed: {error}");
        return;
    }
    world
        .entity_mut(controls["status"])
        .insert(UiLiveRegion::assertive());
    if let Some(mut layout) = world.get_mut::<UiLayoutNode>(browser) {
        layout.size.width = UiLength::Fill(1.0);
        layout.size.height = UiLength::Fill(1.0);
    }
    world.insert_resource(BrowserUi { browser, controls });
    world.insert_resource(LocationChoices::default());
    world.insert_resource(PendingDelete::default());
}

fn dispatch_controls(
    ui: Res<BrowserUi>,
    locations: Res<LocationChoices>,
    mut events: ResMut<WidgetEventQueue>,
    mut actions: ResMut<WidgetActionQueue>,
    mut controls: BrowserControlQueries,
    mut pending_delete: ResMut<PendingDelete>,
) {
    let field = |key: &str| {
        ui.controls
            .get(key)
            .and_then(|entity| controls.fields.get(*entity).ok())
            .map(|field| field.text.trim().to_owned())
            .unwrap_or_default()
    };
    let mut status = None;
    for event in events.take() {
        match event {
            WidgetEvent::Activated { target } => {
                let key = ui
                    .controls
                    .iter()
                    .find_map(|(key, entity)| (*entity == target).then_some(key.as_str()));
                let command = match key {
                    Some("go") => {
                        let path = field("path");
                        if path.is_empty() {
                            status = Some("Enter a directory path first".to_owned());
                            None
                        } else {
                            Some(WidgetActionKind::FileNavigateTo(PathBuf::from(path)))
                        }
                    }
                    Some("up") => Some(WidgetActionKind::FileNavigateUp),
                    Some("refresh") => Some(WidgetActionKind::FileRefresh),
                    Some("bookmark") => Some(WidgetActionKind::FileBookmarkCurrent),
                    Some("remove_bookmark") => Some(WidgetActionKind::FileRemoveBookmarkCurrent),
                    Some("view") => controls.browsers.get(ui.browser).ok().map(|browser| {
                        WidgetActionKind::FileSetView(if browser.view == FileView::List {
                            FileView::Grid
                        } else {
                            FileView::List
                        })
                    }),
                    Some("sort") => controls.browsers.get(ui.browser).ok().map(|browser| {
                        WidgetActionKind::FileSetSort(
                            match browser.sort() {
                                FileSort::Name => FileSort::Kind,
                                FileSort::Kind => FileSort::Size,
                                FileSort::Size => FileSort::Modified,
                                FileSort::Modified => FileSort::Name,
                            },
                            true,
                        )
                    }),
                    Some("new") => Some(WidgetActionKind::FileCreateDirectory(field("name"))),
                    Some("rename") => Some(WidgetActionKind::FileRenameSelected(field("name"))),
                    Some("delete") => {
                        let selected = controls
                            .browsers
                            .get(ui.browser)
                            .ok()
                            .map(|browser| browser.selected.iter().cloned().collect::<Vec<_>>())
                            .unwrap_or_default();
                        if selected.is_empty() {
                            pending_delete.0 = None;
                            status = Some("Select files to delete first".to_owned());
                            None
                        } else if pending_delete.0.as_ref() == Some(&selected) {
                            pending_delete.0 = None;
                            Some(WidgetActionKind::FileDeleteSelected)
                        } else {
                            pending_delete.0 = Some(selected.clone());
                            status = Some(format!(
                                "Delete {} selected item(s)? Click Delete selected again to confirm",
                                selected.len()
                            ));
                            None
                        }
                    }
                    Some("copy") | Some("move") => {
                        let destination = field("destination");
                        if destination.is_empty() {
                            status = Some("Enter a destination directory first".to_owned());
                            None
                        } else if key == Some("copy") {
                            Some(WidgetActionKind::FileCopySelectedTo(PathBuf::from(
                                destination,
                            )))
                        } else {
                            Some(WidgetActionKind::FileMoveSelectedTo(PathBuf::from(
                                destination,
                            )))
                        }
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    if key != Some("delete") {
                        pending_delete.0 = None;
                    }
                    actions.push(WidgetAction::new(ui.browser, command));
                }
            }
            WidgetEvent::TextChanged { target, text }
                if ui.controls.get("filter") == Some(&target) =>
            {
                actions.push(WidgetAction::new(
                    ui.browser,
                    WidgetActionKind::FileSetFilter(text),
                ));
            }
            WidgetEvent::ValueChanged {
                target,
                after: WidgetValue::Option(Some(index)),
                ..
            } if ui.controls.get("locations") == Some(&target) => {
                if let Some(path) = locations.paths.get(index) {
                    actions.push(WidgetAction::new(
                        ui.browser,
                        WidgetActionKind::FileNavigateTo(path.clone()),
                    ));
                }
            }
            WidgetEvent::FileOperationFailed {
                operation,
                path,
                message,
                partial_effects,
                ..
            } => {
                status = Some(if partial_effects.is_empty() {
                    format!("{operation} {}: {message}", path.display())
                } else {
                    format!(
                        "{operation} {}: {message}; {} path(s) changed before failure",
                        path.display(),
                        partial_effects.len()
                    )
                });
            }
            WidgetEvent::FileOperationCommitted {
                operation, paths, ..
            } => {
                status = Some(format!(
                    "{operation} completed for {} path(s); filesystem changes cannot be undone with Ctrl+Z",
                    paths.len()
                ));
            }
            WidgetEvent::FileBrowserRestoreFailed { path, message, .. } => {
                status = Some(format!("Cannot restore {}: {message}", path.display()));
            }
            WidgetEvent::FileDirectoryChanged { path, .. } => {
                status = Some(format!("Browsing {}", path.display()));
            }
            WidgetEvent::FileSelected { path, .. } => {
                pending_delete.0 = None;
                status = Some(format!("Selected {}", path.display()));
            }
            WidgetEvent::FileBookmarksChanged { bookmarks, .. } => {
                status = Some(format!("{} bookmark(s) saved", bookmarks.len()));
            }
            other => info!("file browser: {other:?}"),
        }
    }
    if let Some(status) = status
        && let Some(entity) = ui.controls.get("status")
        && let Ok(mut label) = controls.labels.get_mut(*entity)
    {
        label.text = status;
    }
}

fn sync_locations(
    ui: Res<BrowserUi>,
    browsers: Query<&FileBrowser>,
    mut locations: ResMut<LocationChoices>,
    mut dropdowns: Query<&mut Dropdown>,
    mut fields: Query<&mut TextField>,
    mut buttons: Query<&mut WidgetButton>,
) {
    let Ok(browser) = browsers.get(ui.browser) else {
        return;
    };
    if let Some(remove_bookmark) = ui.controls.get("remove_bookmark")
        && let Ok(mut button) = buttons.get_mut(*remove_bookmark)
    {
        button.enabled = browser.bookmarks.contains(&browser.directory);
    }
    let Some(dropdown_entity) = ui.controls.get("locations") else {
        return;
    };
    let Ok(mut dropdown) = dropdowns.get_mut(*dropdown_entity) else {
        return;
    };
    let mut paths: Vec<_> = browser
        .breadcrumbs()
        .into_iter()
        .map(|crumb| crumb.path)
        .collect();
    for bookmark in &browser.bookmarks {
        if !paths.contains(bookmark) {
            paths.push(bookmark.clone());
        }
    }
    let options: Vec<_> = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let selected = paths.iter().position(|path| path == &browser.directory);
    if dropdown.options != options {
        dropdown.options = options;
    }
    dropdown.selected = selected;
    dropdown.highlighted = selected;
    locations.paths = paths;
    if locations.displayed_directory.as_ref() != Some(&browser.directory) {
        if let Some(path_entity) = ui.controls.get("path")
            && let Ok(mut field) = fields.get_mut(*path_entity)
        {
            field.text = browser.directory.display().to_string();
            field.caret = field.text.chars().count();
            field.selection = None;
        }
        locations.displayed_directory = Some(browser.directory.clone());
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "BBUI File Browser".into(),
                resolution: WindowResolution::new(800, 640),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(BbuiPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (dispatch_controls, sync_locations)
                .chain()
                .after(bbui_core::UiSystems::Widgets),
        )
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn delete_button_requires_matching_second_activation() {
        let directory = std::env::temp_dir().join(format!(
            "bbui-browser-delete-confirm-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("keep.txt");
        std::fs::write(&file, b"keep").unwrap();
        let mut app = App::new();
        app.init_resource::<WidgetEventQueue>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<LocationChoices>()
            .init_resource::<PendingDelete>()
            .add_systems(
                Update,
                (
                    dispatch_controls,
                    bbui::widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let mut browser_state = FileBrowser::new(&directory).unwrap();
        browser_state.confine_to(&directory).unwrap();
        browser_state.select(0, false, false);
        let browser = app.world_mut().spawn(browser_state).id();
        let button = app.world_mut().spawn_empty().id();
        let status = app.world_mut().spawn(BbuiLabel::new("Ready")).id();
        app.world_mut().insert_resource(BrowserUi {
            browser,
            controls: HashMap::from([("delete".to_owned(), button), ("status".to_owned(), status)]),
        });
        for should_exist in [true, false] {
            app.world_mut()
                .resource_mut::<WidgetEventQueue>()
                .0
                .push(WidgetEvent::Activated { target: button });
            app.update();
            assert_eq!(file.exists(), should_exist);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn toolbar_routes_file_operation_and_filter_to_the_browser() {
        let directory = std::env::temp_dir().join(format!(
            "bbui-browser-toolbar-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut app = App::new();
        app.init_resource::<WidgetEventQueue>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<LocationChoices>()
            .init_resource::<PendingDelete>()
            .add_systems(
                Update,
                (
                    dispatch_controls,
                    bbui::widgets::actions::apply_widget_actions,
                )
                    .chain(),
            );
        let browser = app
            .world_mut()
            .spawn(FileBrowser::new(&directory).unwrap())
            .id();
        let new_button = app.world_mut().spawn_empty().id();
        let name_field = app.world_mut().spawn(TextField::new("Created")).id();
        let filter_field = app.world_mut().spawn(TextField::new("")).id();
        let status = app.world_mut().spawn(BbuiLabel::new("Ready")).id();
        app.world_mut().insert_resource(BrowserUi {
            browser,
            controls: HashMap::from([
                ("new".to_owned(), new_button),
                ("name".to_owned(), name_field),
                ("filter".to_owned(), filter_field),
                ("status".to_owned(), status),
            ]),
        });
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::Activated { target: new_button });
        app.update();
        assert!(directory.join("Created").is_dir());
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::TextChanged {
                target: filter_field,
                text: "Cre".to_owned(),
            });
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().filter(),
            "Cre"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn path_and_location_controls_navigate_to_real_directories() {
        let directory = std::env::temp_dir().join(format!(
            "bbui-browser-locations-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let child = directory.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let directory = std::fs::canonicalize(directory).unwrap();
        let child = directory.join("child");
        let mut app = App::new();
        app.init_resource::<WidgetEventQueue>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<LocationChoices>()
            .init_resource::<PendingDelete>()
            .add_systems(
                Update,
                (
                    dispatch_controls,
                    bbui::widgets::actions::apply_widget_actions,
                    sync_locations,
                )
                    .chain(),
            );
        let browser = app
            .world_mut()
            .spawn(FileBrowser::new(&directory).unwrap())
            .id();
        let path_field = app
            .world_mut()
            .spawn(TextField::new(child.display().to_string()))
            .id();
        let go_button = app.world_mut().spawn_empty().id();
        let locations = app
            .world_mut()
            .spawn(Dropdown::new([directory.display().to_string()], Some(0)))
            .id();
        app.world_mut().insert_resource(BrowserUi {
            browser,
            controls: HashMap::from([
                ("path".to_owned(), path_field),
                ("go".to_owned(), go_button),
                ("locations".to_owned(), locations),
            ]),
        });
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::Activated { target: go_button });
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().directory,
            child
        );
        let parent_index = app
            .world()
            .resource::<LocationChoices>()
            .paths
            .iter()
            .position(|path| path == &directory)
            .unwrap();
        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::ValueChanged {
                target: locations,
                before: WidgetValue::Option(None),
                after: WidgetValue::Option(Some(parent_index)),
            });
        app.update();
        assert_eq!(
            app.world().get::<FileBrowser>(browser).unwrap().directory,
            directory
        );
        assert_eq!(
            app.world().get::<TextField>(path_field).unwrap().text,
            directory.display().to_string()
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn remove_bookmark_toolbar_routes_and_reports_the_change() {
        let directory = std::env::temp_dir().join(format!(
            "bbui-browser-bookmark-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let directory = std::fs::canonicalize(directory).unwrap();
        let mut browser_state = FileBrowser::new(&directory).unwrap();
        browser_state.bookmark(directory.clone()).unwrap();

        let mut app = App::new();
        app.init_resource::<WidgetEventQueue>()
            .init_resource::<WidgetActionQueue>()
            .init_resource::<LocationChoices>()
            .init_resource::<PendingDelete>()
            .add_systems(
                Update,
                (
                    dispatch_controls,
                    bbui::widgets::actions::apply_widget_actions,
                    sync_locations,
                )
                    .chain(),
            );
        let browser = app.world_mut().spawn(browser_state).id();
        let remove_button = app
            .world_mut()
            .spawn(WidgetButton::new("Remove bookmark"))
            .id();
        let locations = app
            .world_mut()
            .spawn(Dropdown::new([directory.display().to_string()], Some(0)))
            .id();
        let path_field = app
            .world_mut()
            .spawn(TextField::new(directory.display().to_string()))
            .id();
        let status = app.world_mut().spawn(BbuiLabel::new("Ready")).id();
        app.world_mut().insert_resource(BrowserUi {
            browser,
            controls: HashMap::from([
                ("remove_bookmark".to_owned(), remove_button),
                ("locations".to_owned(), locations),
                ("path".to_owned(), path_field),
                ("status".to_owned(), status),
            ]),
        });

        app.world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::Activated {
                target: remove_button,
            });
        app.update();

        assert!(
            app.world()
                .get::<FileBrowser>(browser)
                .unwrap()
                .bookmarks
                .is_empty()
        );
        assert!(
            !app.world()
                .get::<WidgetButton>(remove_button)
                .unwrap()
                .enabled
        );
        assert!(app
            .world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { bookmarks, .. } if bookmarks.is_empty())));

        app.update();
        assert_eq!(
            app.world().get::<BbuiLabel>(status).unwrap().text,
            "0 bookmark(s) saved"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}

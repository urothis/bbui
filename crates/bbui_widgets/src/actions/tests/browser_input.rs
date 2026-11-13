use super::*;

#[test]
pub(super) fn file_navigate_to_updates_directory_reports_errors_and_resets_scroll() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "bbui-widgets-navigate-to-{}-{unique}",
        std::process::id()
    ));
    let nested = root.join("Nested");
    std::fs::create_dir_all(&nested).unwrap();
    let browser = FileBrowser::new(&root).unwrap();
    let owner = app.world_mut().spawn(browser).id();
    app.world_mut()
        .entity_mut(owner)
        .insert(FileBrowserViewState {
            scroll_rows: 17,
            ..FileBrowserViewState::default()
        });

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileNavigateTo(nested.clone()),
        ));
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().directory,
        std::fs::canonicalize(&nested).unwrap()
    );
    assert_eq!(
        app.world()
            .get::<FileBrowserViewState>(owner)
            .unwrap()
            .scroll_rows,
        0
    );
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
            |event| matches!(event, WidgetEvent::FileDirectoryChanged { target, .. } if *target == owner)
        ));

    app.world_mut()
        .get_mut::<FileBrowserViewState>(owner)
        .unwrap()
        .scroll_rows = 11;
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileNavigateTo(root.join("missing")),
        ));
    app.update();
    assert_eq!(
        app.world()
            .get::<FileBrowserViewState>(owner)
            .unwrap()
            .scroll_rows,
        0
    );
    assert!(app.world().resource::<WidgetEventQueue>().0.iter().any(
        |event| matches!(event, WidgetEvent::FileOperationFailed { target, .. } if *target == owner)
    ));

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn file_browser_grid_keyboard_selection_and_bookmarks_route_end_to_end() {
    let root = std::env::temp_dir().join(format!(
        "bbui-widgets-browser-routes-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    for index in 0..10 {
        std::fs::write(root.join(format!("entry-{index:02}.txt")), "item").unwrap();
    }

    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let mut browser = FileBrowser::new(&root).unwrap();
    browser.view = FileView::Grid;
    let owner = app.world_mut().spawn(browser).id();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 250.0, 130.0),
        clip: None,
        z: 2,
        content_size: [250.0, 130.0],
        dpi_scale: 1.0,
    });
    app.world_mut()
        .get_mut::<FileBrowser>(owner)
        .unwrap()
        .select(0, false, false);

    let send_key = |app: &mut App, key, modifiers| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: owner,
            key,
            pressed: true,
            repeat: false,
            modifiers,
        }];
        app.update();
    };
    let selected_index = |app: &App| {
        let browser = app.world().get::<FileBrowser>(owner).unwrap();
        browser
            .visible_range(0, browser.visible_count())
            .iter()
            .position(|entry| Some(&entry.path) == browser.cursor.as_ref())
            .unwrap()
    };

    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(selected_index(&app), 2, "grid down advances one column");
    send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
    assert_eq!(selected_index(&app), 3, "grid right advances one cell");
    send_key(&mut app, UiKeyCode::ArrowLeft, UiModifiers::default());
    assert_eq!(selected_index(&app), 2, "grid left reverses one cell");
    send_key(&mut app, UiKeyCode::ArrowRight, UiModifiers::default());
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        5,
        "page down advances a visible grid row"
    );
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        3,
        "page up reverses the visible-row stride"
    );

    app.world_mut().get_mut::<FileBrowser>(owner).unwrap().view = FileView::List;
    app.world_mut()
        .get_mut::<FileBrowser>(owner)
        .unwrap()
        .select(0, false, false);
    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(selected_index(&app), 1, "list down advances one row");
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        4,
        "list page down advances visible rows"
    );
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(
        selected_index(&app),
        1,
        "list page up reverses the row stride"
    );

    send_key(
        &mut app,
        UiKeyCode::A,
        UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    );
    assert_eq!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .selected
            .len(),
        10
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileSelectionChanged { target, selected } if *target == owner && selected.len() == 10)));

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileBookmarkCurrent,
        ));
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().bookmarks,
        vec![std::fs::canonicalize(&root).unwrap()]
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { target, bookmarks } if *target == owner && bookmarks.len() == 1)));

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            owner,
            WidgetActionKind::FileRemoveBookmarkCurrent,
        ));
    app.update();
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .bookmarks
            .is_empty()
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileBookmarksChanged { target, bookmarks } if *target == owner && bookmarks.is_empty())));

    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .cursor
            .as_ref()
            .and_then(|path| path.file_name())
            .unwrap(),
        "entry-00.txt"
    );
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .cursor
            .as_ref()
            .and_then(|path| path.file_name())
            .unwrap(),
        "entry-09.txt"
    );
    send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileActivated { target, path } if *target == owner && path.ends_with("entry-09.txt"))));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "entry".to_owned(),
    }];
    app.update();
    send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().filter(),
        "entr"
    );
    send_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .filter()
            .is_empty()
    );
    assert!(
        !app.world()
            .resource::<WidgetFileBrowserFilterEdits>()
            .active
            .contains_key(&owner),
        "Escape must close the filter draft before the next key is routed"
    );
    send_key(&mut app, UiKeyCode::Backspace, UiModifiers::default());
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().directory,
        std::fs::canonicalize(root.parent().unwrap()).unwrap(),
        "Backspace leaves an empty filter by navigating to the parent directory"
    );

    std::fs::remove_dir_all(root).unwrap();
}

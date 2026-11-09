use super::*;

#[test]
pub(super) fn direct_file_browser_filter_undo_consumes_draft_before_application_history() {
    let root = std::env::temp_dir().join(format!(
        "bui-widgets-browser-filter-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("cube.txt"), "cube").unwrap();
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut().init_resource::<UiFocus>();
    let window = app.world_mut().spawn_empty().id();
    let browser = app
        .world_mut()
        .spawn((
            FileBrowser::new(&root).unwrap(),
            UiFocusable,
            UiKey("browser".into()),
        ))
        .id();
    let checkbox = app
        .world_mut()
        .spawn((Checkbox::new("enabled", false), UiKey("enabled".into())))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(browser);
    app.world_mut().resource_mut::<UiFocus>().set_active(window);
    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(checkbox, WidgetActionKind::SetBool(true)));
    app.update();
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: browser,
        text: "cube".into(),
    }];
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().filter(),
        "cube"
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);

    let route_history_key = |app: &mut App, shift: bool| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: browser,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                shift,
                ..UiModifiers::default()
            },
        }];
        app.update();
    };
    route_history_key(&mut app, false);

    assert!(
        app.world()
            .get::<FileBrowser>(browser)
            .unwrap()
            .filter()
            .is_empty()
    );
    assert_eq!(
        app.world().get::<Checkbox>(checkbox).unwrap().value,
        CheckboxValue::On,
        "the in-progress filter undo must not consume the previous application edit"
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
    route_history_key(&mut app, true);
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().filter(),
        "cube",
        "redo should restore a locally cancelled filter draft"
    );
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
    route_history_key(&mut app, false);
    assert!(
        app.world()
            .get::<FileBrowser>(browser)
            .unwrap()
            .filter()
            .is_empty()
    );
    route_history_key(&mut app, false);
    assert_eq!(
        app.world().get::<Checkbox>(checkbox).unwrap().value,
        CheckboxValue::Off,
        "once the local draft redo is consumed, Ctrl+Z should reach app history"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn file_browser_ui_actions_round_trip_through_widget_history() {
    let root = std::env::temp_dir().join(format!(
        "bui-widgets-browser-history-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let child = root.join("child");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::write(root.join("entry.txt"), "entry").unwrap();
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut().init_resource::<UiFocus>();
    let window = app.world_mut().spawn_empty().id();
    let browser = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiKey("browser-history".into()),
            FileBrowser::new(&root).unwrap(),
            FileBrowserViewState::default(),
            UiLayoutResult {
                window,
                rect: UiRect::new(0.0, 0.0, 280.0, 180.0),
                clip: None,
                z: 1,
                content_size: [280.0, 180.0],
                dpi_scale: 1.0,
            },
            UiHitBox {
                window: Some(window),
                rect: UiRect::new(0.0, 0.0, 280.0, 180.0),
                clip: None,
                z_index: 1,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(browser);
    app.world_mut().resource_mut::<UiFocus>().set_active(window);

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            browser,
            WidgetActionKind::FileBookmarkCurrent,
        ));
    app.update();
    assert_eq!(app.world().resource::<bui_core::UiHistory>().undo_len(), 1);
    let route_history = |app: &mut App, shift: bool| {
        app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
            window,
            target: browser,
            key: UiKeyCode::Z,
            pressed: true,
            repeat: false,
            modifiers: UiModifiers {
                control: true,
                shift,
                ..UiModifiers::default()
            },
        }];
        app.update();
    };
    route_history(&mut app, false);
    assert!(
        app.world()
            .get::<FileBrowser>(browser)
            .unwrap()
            .bookmarks
            .is_empty()
    );
    route_history(&mut app, true);
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().bookmarks,
        vec![std::fs::canonicalize(&root).unwrap()]
    );

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            browser,
            WidgetActionKind::FileSetView(FileView::Grid),
        ));
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().view,
        FileView::Grid
    );
    route_history(&mut app, false);
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().view,
        FileView::List
    );

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Key {
        window,
        target: browser,
        key: UiKeyCode::A,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            ..UiModifiers::default()
        },
    }];
    app.update();
    assert_eq!(
        app.world()
            .get::<FileBrowser>(browser)
            .unwrap()
            .selected
            .len(),
        2
    );
    route_history(&mut app, false);
    assert!(
        app.world()
            .get::<FileBrowser>(browser)
            .unwrap()
            .selected
            .is_empty(),
        "selection state is part of the browser snapshot"
    );

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            browser,
            WidgetActionKind::FileNavigateTo(child.clone()),
        ));
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().directory,
        std::fs::canonicalize(&child).unwrap()
    );
    route_history(&mut app, false);
    assert_eq!(
        app.world().get::<FileBrowser>(browser).unwrap().directory,
        std::fs::canonicalize(&root).unwrap()
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .focused,
        Some(browser),
        "undo restores focus only while the browser is still interactable in its window"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn successful_file_operations_are_reported_as_irreversible_side_effects() {
    let root = std::env::temp_dir().join(format!(
        "bui-widgets-browser-side-effect-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let path = root.join("delete-me.txt");
    std::fs::write(&path, "external file").unwrap();
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let mut browser_model = FileBrowser::new(&root).unwrap();
    browser_model.selected.insert(path.clone());
    let browser = app.world_mut().spawn(browser_model).id();

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            browser,
            WidgetActionKind::FileDeleteSelected,
        ));
    app.update();

    assert!(!path.exists());
    assert!(
        app.world()
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| {
                matches!(event, WidgetEvent::FileOperationCommitted {
                target,
                operation: "delete",
                paths,
                irreversible: true,
            } if *target == browser && paths.as_slice() == std::slice::from_ref(&path))
            })
    );
    assert_eq!(
        app.world().resource::<bui_core::UiHistory>().undo_len(),
        0,
        "filesystem mutations are external side effects, not UI history values"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn failed_partial_delete_reports_only_paths_changed_by_the_operation() {
    let root = std::env::temp_dir().join(format!(
        "bui-widgets-browser-partial-delete-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let first = root.join("a.txt");
    let missing = root.join("b.txt");
    std::fs::write(&first, "first").unwrap();
    std::fs::write(&missing, "missing before action").unwrap();
    let mut browser_model = FileBrowser::new(&root).unwrap();
    browser_model
        .selected
        .extend([first.clone(), missing.clone()]);
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let browser = app.world_mut().spawn(browser_model).id();
    std::fs::remove_file(&missing).unwrap();

    app.world_mut()
        .resource_mut::<WidgetActionQueue>()
        .push(WidgetAction::new(
            browser,
            WidgetActionKind::FileDeleteSelected,
        ));
    app.update();

    let failure = app
        .world()
        .resource::<WidgetEventQueue>()
        .0
        .iter()
        .find_map(|event| match event {
            WidgetEvent::FileOperationFailed {
                target,
                partial_effects,
                message,
                ..
            } if *target == browser => Some((partial_effects, message)),
            _ => None,
        })
        .expect("the stale second entry should report a failed operation");
    assert!(!first.exists());
    assert_eq!(failure.0.as_slice(), std::slice::from_ref(&first));
    assert!(failure.1.contains("1 earlier item(s) were deleted"));
    std::fs::remove_dir_all(root).unwrap();
}

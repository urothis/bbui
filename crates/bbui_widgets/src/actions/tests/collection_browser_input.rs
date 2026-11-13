use super::*;

#[test]
pub(super) fn widget_tree_file_browser_virtualizes_rows_selects_filters_and_opens_directories() {
    let directory = std::env::temp_dir().join(format!(
        "bbui-widget-browser-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(directory.join("subfolder")).unwrap();
    std::fs::write(directory.join("alpha.txt"), "a").unwrap();
    std::fs::write(directory.join("beta.bin"), "b").unwrap();
    std::fs::write(directory.join("subfolder/nested.txt"), "n").unwrap();
    // FileBrowser entries use canonical paths; macOS may canonicalize
    // temp_dir's `/var` prefix to `/private/var`.
    let directory = std::fs::canonicalize(directory).unwrap();

    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let owner = WidgetTree::new(WidgetNode::file_browser(directory.clone()))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 360.0, 170.0),
        clip: None,
        z: 5,
        content_size: [360.0, 170.0],
        dpi_scale: 1.0,
    });
    app.update();

    let visible_items = browser_items(app.world_mut(), owner);
    assert_eq!(visible_items.len(), 3);
    assert!(
        visible_items
            .iter()
            .all(|(entity, _)| app.world().get::<UiHitBox>(*entity).is_some())
    );
    let alpha = visible_items
        .iter()
        .find(|(_, item)| item.name == "alpha.txt")
        .unwrap()
        .clone();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: alpha.0,
        button: UiPointerButton::Primary,
        click_count: 1,
        position: [8.0, 54.0],
    }];
    app.update();
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .selected
            .contains(&directory.join("alpha.txt"))
    );
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .take()
            .iter()
            .any(|event| matches!(event, WidgetEvent::FileSelected { path, selected: true, .. } if path.ends_with("alpha.txt"))));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "alpha".into(),
    }];
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().filter(),
        "alpha"
    );
    assert_eq!(browser_items(app.world_mut(), owner).len(), 2); // Directories stay visible while filtering.

    let subfolder = browser_items(app.world_mut(), owner)
        .into_iter()
        .find(|(_, item)| item.name == "subfolder")
        .unwrap();
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Click {
        window,
        target: subfolder.0,
        button: UiPointerButton::Primary,
        click_count: 2,
        position: [10.0, 54.0],
    }];
    app.update();
    assert_eq!(
        app.world().get::<FileBrowser>(owner).unwrap().directory,
        std::fs::canonicalize(directory.join("subfolder")).unwrap()
    );
    assert!(
        app.world()
            .get::<FileBrowserItem>(subfolder.0)
            .is_none_or(|item| !item.selected)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
pub(super) fn list_view_virtualizes_stable_items_and_paints_clipped_rows() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let model = ListModel {
        items: (0..2_000)
            .map(|index| ListItem {
                id: index + 1,
                value: format!("Asset {index:04}"),
                enabled: index % 13 != 0,
            })
            .collect(),
        ..ListModel::default()
    };
    let owner = WidgetTree::new(WidgetNode::list("Assets", model))
        .spawn(app.world_mut(), None)
        .unwrap();
    let rect = UiRect::new(12.0, 16.0, 240.0, 130.0);
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect,
        clip: None,
        z: 4,
        content_size: [rect.width, rect.height],
        dpi_scale: 1.0,
    });
    app.update();

    let visible = collection_items(app.world_mut(), owner);
    assert!((5..=7).contains(&visible.len()));
    assert!(visible.iter().all(|(entity, item)| {
        item.kind == CollectionItemKind::List
            && app
                .world()
                .get::<UiParent>(*entity)
                .is_some_and(|parent| parent.0 == owner)
            && app.world().get::<UiHitBox>(*entity).is_some_and(|hit| {
                hit.window == Some(window)
                    && hit.clip.is_some_and(|clip| {
                        clip.y >= rect.y + 28.0 && clip.bottom() <= rect.bottom()
                    })
            })
    }));
    assert!(visible.len() < 2_000);
    let stable_id = visible.iter().find(|(_, item)| item.id == 5).unwrap().0;
    let first = visible.iter().map(|(_, item)| item.id).min().unwrap();
    assert_eq!(first, 1);
    assert!(app.world().resource::<DisplayList>().widget_items().iter().any(
            |draw| matches!(&draw.primitive, Primitive::TextWrapped { text, .. } if text.contains("Asset 0000"))
        ));

    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Wheel {
        window,
        target: owner,
        position: [30.0, 80.0],
        delta: [0.0, -1.0],
        unit: bbui_core::UiWheelUnit::Lines,
        modifiers: UiModifiers::default(),
    }];
    app.update();
    let scrolled = collection_items(app.world_mut(), owner);
    assert_eq!(scrolled.iter().map(|(_, item)| item.id).min(), Some(2));
    assert_eq!(
        app.world()
            .resource::<WidgetCollectionItems>()
            .items
            .get(&owner)
            .and_then(|items| items.get(&5)),
        Some(&stable_id)
    );
}

#[test]
pub(super) fn list_keyboard_routes_page_edges_select_all_activate_and_rename_cancel() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    let window = app.world_mut().spawn_empty().id();
    let model = ListModel {
        items: (0..10)
            .map(|index| ListItem {
                id: index + 1,
                value: format!("Asset {index}"),
                enabled: true,
            })
            .collect(),
        ..ListModel::default()
    };
    let owner = WidgetTree::new(WidgetNode::list("Assets", model))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut().entity_mut(owner).insert(UiLayoutResult {
        window,
        rect: UiRect::new(0.0, 0.0, 240.0, 150.0),
        clip: None,
        z: 0,
        content_size: [240.0, 150.0],
        dpi_scale: 1.0,
    });
    app.update();
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

    send_key(&mut app, UiKeyCode::ArrowDown, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::PageDown, UiModifiers::default());
    let page_row = app
        .world()
        .get::<ListModel<String>>(owner)
        .unwrap()
        .selection
        .active
        .unwrap();
    assert!(page_row > 1);
    send_key(&mut app, UiKeyCode::PageUp, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::End, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(10)
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
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .selected
            .len(),
        10
    );
    send_key(&mut app, UiKeyCode::Home, UiModifiers::default());
    assert_eq!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .active,
        Some(1)
    );
    send_key(&mut app, UiKeyCode::Enter, UiModifiers::default());
    assert!(app
            .world_mut()
            .resource_mut::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(event, WidgetEvent::CollectionItemActivated { target, id: 1 } if *target == owner)));

    send_key(&mut app, UiKeyCode::F2, UiModifiers::default());
    app.world_mut().resource_mut::<UiRoutedEvents>().events = vec![UiRoutedEvent::Text {
        window,
        target: owner,
        text: "Cancelled rename".to_owned(),
    }];
    app.update();
    send_key(&mut app, UiKeyCode::Escape, UiModifiers::default());
    assert_eq!(
        app.world().get::<ListModel<String>>(owner).unwrap().items[0].value,
        "Asset 0"
    );
    assert!(
        app.world()
            .get::<CollectionEditorState>(owner)
            .unwrap()
            .rename_id
            .is_none()
    );
}

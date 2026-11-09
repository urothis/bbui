use super::*;
use crate::FileView;
use crate::actions::CheckboxPaintChange;
use bui_core::{
    UiChildren, UiFocusable, UiKeyCode, UiModifiers, UiNode, UiRoutedEvents, set_parent,
};
use bui_text::{UiTextUndoFallback, process_text_events};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "bui-history-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    // Keep expectations aligned with FileBrowser's canonical directory.
    fs::canonicalize(path).unwrap()
}

fn setup() -> World {
    let mut world = World::new();
    world.init_resource::<UiHistory>();
    world.init_resource::<WidgetHistoryCursor>();
    world.init_resource::<WidgetEventQueue>();
    world.init_resource::<UiRoutedEvents>();
    world.init_resource::<UiFocus>();
    world.init_resource::<UiTextUndoFallback>();
    world.init_resource::<PanelExpansionStore>();
    world.init_resource::<crate::PanelOrderStore>();
    world
}

fn key_event(
    window: Entity,
    target: Entity,
    key: bui_core::UiKeyCode,
    shift: bool,
) -> bui_core::UiRoutedEvent {
    bui_core::UiRoutedEvent::Key {
        window,
        target,
        key,
        pressed: true,
        repeat: false,
        modifiers: UiModifiers {
            control: true,
            shift,
            ..UiModifiers::default()
        },
    }
}

#[test]
fn discrete_widget_edits_are_committed_and_undo_redo_applies_values() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let target = world
        .spawn((
            UiNode,
            UiFocusable,
            Checkbox::new("enabled", false),
            UiKey("enabled".into()),
        ))
        .id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
    world.resource_mut::<UiFocus>().set_active(window);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::ValueChanged {
            target,
            before: WidgetValue::Bool(false),
            after: WidgetValue::Bool(true),
        });

    record_widget_edits(&mut world);
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(
        world.get::<Checkbox>(target).unwrap().value,
        CheckboxValue::Off
    );
    assert_eq!(
        world.resource::<UiFocus>().state(window).unwrap().focused,
        Some(target)
    );

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(
        world.get::<Checkbox>(target).unwrap().value,
        CheckboxValue::On
    );
}

#[test]
fn exclusive_panel_expansion_undoes_as_one_atomic_group() {
    let mut world = setup();
    let group = world
        .spawn(PanelGroup::with_panels(["open", "closed"]))
        .id();
    let mut first = Panel::new("Open", true);
    first.exclusive = true;
    first.persistence_key = Some("open".into());
    let open = world.spawn((first, UiParent(group))).id();
    let mut second = Panel::new("Closed", false);
    second.exclusive = true;
    second.persistence_key = Some("closed".into());
    let closed = world.spawn((second, UiParent(group))).id();
    world
        .get_mut::<Panel>(open)
        .unwrap()
        .state
        .set_expanded(false);
    world
        .get_mut::<Panel>(closed)
        .unwrap()
        .state
        .set_expanded(true);
    world.get_mut::<PanelGroup>(group).unwrap().exclusive_open = Some("closed".into());
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::PanelGroupCommitted {
            changes: vec![(open, true, false), (closed, false, true)],
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    let undo = world.resource_mut::<UiHistory>().undo().unwrap();
    apply_restore(&mut world, undo);
    assert!(world.get::<Panel>(open).unwrap().state.target_expanded);
    assert!(!world.get::<Panel>(closed).unwrap().state.target_expanded);
    assert_eq!(
        world
            .get::<PanelGroup>(group)
            .unwrap()
            .exclusive_open
            .as_deref(),
        Some("open")
    );
    assert_eq!(
        world.resource::<PanelExpansionStore>().get("open"),
        Some(true)
    );
    assert_eq!(
        world.resource::<PanelExpansionStore>().get("closed"),
        Some(false)
    );

    let redo = world.resource_mut::<UiHistory>().redo().unwrap();
    apply_restore(&mut world, redo);
    assert!(!world.get::<Panel>(open).unwrap().state.target_expanded);
    assert!(world.get::<Panel>(closed).unwrap().state.target_expanded);
    assert_eq!(
        world
            .get::<PanelGroup>(group)
            .unwrap()
            .exclusive_open
            .as_deref(),
        Some("closed")
    );
}

#[test]
fn panel_order_history_restores_sibling_order_and_keeps_invalid_step() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let group = world
        .spawn((
            UiNode,
            UiKey("settings".into()),
            PanelGroup::with_panels(["alpha", "beta"]).with_reordering("settings"),
        ))
        .id();
    let mut alpha = Panel::new("Alpha", true);
    alpha.persistence_key = Some("alpha".into());
    let alpha = world.spawn((UiNode, alpha)).id();
    let mut beta = Panel::new("Beta", true);
    beta.persistence_key = Some("beta".into());
    let beta = world.spawn((UiNode, beta)).id();
    set_parent(&mut world, alpha, group).unwrap();
    set_parent(&mut world, beta, group).unwrap();
    let before = vec!["alpha".to_owned(), "beta".to_owned()];
    let after = vec!["beta".to_owned(), "alpha".to_owned()];
    assert!(crate::panels::restore_panel_order(
        &mut world, group, &after
    ));
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::PanelOrderCommitted {
            group,
            persistence_key: "settings".into(),
            before: before.clone(),
            after: after.clone(),
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, alpha, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<PanelGroup>(group).unwrap().panel_keys, before);
    assert_eq!(world.get::<UiChildren>(group).unwrap().0, [alpha, beta]);
    assert_eq!(
        world.resource::<crate::PanelOrderStore>().get("settings"),
        Some(before.as_slice())
    );

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, alpha, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<PanelGroup>(group).unwrap().panel_keys, after);
    assert_eq!(world.get::<UiChildren>(group).unwrap().0, [beta, alpha]);

    world.get_mut::<Panel>(beta).unwrap().persistence_key = Some("changed".into());
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, alpha, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
    assert_eq!(world.get::<UiChildren>(group).unwrap().0, [beta, alpha]);
    assert!(
        world
            .resource::<WidgetEventQueue>()
            .0
            .iter()
            .any(|event| matches!(
                event,
                WidgetEvent::PanelOrderRestoreFailed { group: target, .. } if *target == group
            ))
    );
}

#[test]
fn browser_navigation_bookmark_and_view_restore_in_one_transaction() {
    let root = temporary_directory("browser-roundtrip");
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    let mut browser = FileBrowser::new(&root).unwrap();
    let before = browser.snapshot_json().unwrap();
    browser.bookmark(&root).unwrap();
    browser.navigate(&child).unwrap();
    browser.view = FileView::Grid;
    let after = browser.snapshot_json().unwrap();

    let mut world = setup();
    let target = world.spawn(browser).id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::FileBrowserCommitted {
            target,
            before,
            after,
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    let undo = world.resource_mut::<UiHistory>().undo().unwrap();
    apply_restore(&mut world, undo);
    let browser = world.get::<FileBrowser>(target).unwrap();
    assert_eq!(browser.directory, root);
    assert!(browser.bookmarks.is_empty());
    assert_eq!(browser.view, FileView::List);

    let redo = world.resource_mut::<UiHistory>().redo().unwrap();
    apply_restore(&mut world, redo);
    let browser = world.get::<FileBrowser>(target).unwrap();
    assert_eq!(browser.directory, child);
    assert_eq!(browser.bookmarks, vec![root.clone()]);
    assert_eq!(browser.view, FileView::Grid);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn browser_restore_failure_preserves_current_state_and_reports_the_path() {
    let root = temporary_directory("browser-missing");
    let original = root.join("original");
    let current = root.join("current");
    fs::create_dir(&original).unwrap();
    fs::create_dir(&current).unwrap();
    let mut browser = FileBrowser::new(&original).unwrap();
    let before = browser.snapshot_json().unwrap();
    browser.navigate(&current).unwrap();
    let after = browser.snapshot_json().unwrap();

    let mut world = setup();
    let target = world.spawn(browser).id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::FileBrowserCommitted {
            target,
            before,
            after,
        });
    record_widget_edits(&mut world);
    fs::remove_dir(&original).unwrap();
    let window = world.spawn_empty().id();
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<FileBrowser>(target).unwrap().directory, current);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
    assert_eq!(world.resource::<UiHistory>().redo_len(), 0);
    assert!(world.resource::<WidgetEventQueue>().0.iter().any(|event| {
        matches!(event, WidgetEvent::FileBrowserRestoreFailed { target: reported, path, .. }
                if *reported == target && *path == original)
    }));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn undo_redo_preserves_backward_text_selection_direction() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let checkbox = world
        .spawn((Checkbox::new("enabled", true), UiKey("enabled".into())))
        .id();
    let text = world
        .spawn((
            UiNode,
            UiFocusable,
            bui_core::UiHitBox {
                window: Some(window),
                rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z_index: 0,
            },
            TextBuffer::new("abcdef"),
            UiKey("text".into()),
        ))
        .id();
    world
        .get_mut::<TextBuffer>(text)
        .unwrap()
        .set_selection(5, 2);
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(text);
    world.resource_mut::<UiFocus>().set_active(window);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::ValueChanged {
            target: checkbox,
            before: WidgetValue::Bool(false),
            after: WidgetValue::Bool(true),
        });
    record_widget_edits(&mut world);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, checkbox, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    let buffer = world.get::<TextBuffer>(text).unwrap();
    assert_eq!((buffer.anchor(), buffer.caret()), (5, 2));

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, checkbox, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    let buffer = world.get::<TextBuffer>(text).unwrap();
    assert_eq!((buffer.anchor(), buffer.caret()), (5, 2));
}

#[test]
fn committed_text_undoes_globally_after_focus_departure_and_restores_selection() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let text = world
        .spawn((
            UiNode,
            UiFocusable,
            bui_core::UiHitBox {
                window: Some(window),
                rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z_index: 0,
            },
            TextBuffer::new("hello!"),
            crate::TextField::new("hello!"),
            UiKey("name".into()),
        ))
        .id();
    let other = world
        .spawn((UiNode, UiFocusable, crate::Button::new("Apply")))
        .id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(other);
    world.resource_mut::<UiFocus>().set_active(window);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::TextCommitted {
            target: text,
            window,
            before: "hello".into(),
            after: "hello!".into(),
            before_selection: TextSelection {
                anchor: 5,
                caret: 2,
            },
            after_selection: TextSelection {
                anchor: 6,
                caret: 6,
            },
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, other, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    let buffer = world.get::<TextBuffer>(text).unwrap();
    assert_eq!(buffer.as_str(), "hello");
    assert_eq!((buffer.anchor(), buffer.caret()), (5, 2));
    assert_eq!(buffer.undo_len(), 0);
    assert_eq!(world.get::<crate::TextField>(text).unwrap().text, "hello");
    assert_eq!(
        world.resource::<UiFocus>().state(window).unwrap().focused,
        Some(text)
    );

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, text, UiKeyCode::Z, true)];
    process_text_events(&mut world);
    process_widget_undo_shortcuts(&mut world);
    let buffer = world.get::<TextBuffer>(text).unwrap();
    assert_eq!(buffer.as_str(), "hello!");
    assert_eq!((buffer.anchor(), buffer.caret()), (6, 6));
    assert_eq!(world.get::<crate::TextField>(text).unwrap().text, "hello!");
}

#[test]
fn undo_applies_surviving_values_without_restoring_hidden_focus() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let checkbox = world
        .spawn((
            UiNode,
            UiFocusable,
            Checkbox::new("enabled", true),
            UiKey("enabled".into()),
        ))
        .id();
    let text = world
        .spawn((
            UiNode,
            UiFocusable,
            bui_core::UiHitBox {
                window: Some(window),
                rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                clip: None,
                z_index: 0,
            },
            TextBuffer::new("selection"),
            UiKey("text".into()),
        ))
        .id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(text);
    world.resource_mut::<UiFocus>().set_active(window);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::ValueChanged {
            target: checkbox,
            before: WidgetValue::Bool(false),
            after: WidgetValue::Bool(true),
        });
    record_widget_edits(&mut world);
    world.entity_mut(text).insert(bui_core::UiVisible(false));

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, checkbox, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);

    assert_eq!(
        world.get::<Checkbox>(checkbox).unwrap().value,
        CheckboxValue::Off
    );
    let focus = world.resource::<UiFocus>().state(window).unwrap();
    assert_eq!(focus.focused, None);
    assert_eq!(focus.selection, None);
}

#[test]
fn undo_does_not_restore_focus_to_disabled_or_reassigned_controls() {
    for reassign_window in [false, true] {
        let mut world = setup();
        let window = world.spawn_empty().id();
        let other_window = world.spawn_empty().id();
        let checkbox = world
            .spawn((Checkbox::new("enabled", true), UiKey("enabled".into())))
            .id();
        let text = world
            .spawn((
                UiNode,
                UiFocusable,
                bui_core::UiHitBox {
                    window: Some(window),
                    rect: bui_core::UiRect::new(0.0, 0.0, 120.0, 24.0),
                    clip: None,
                    z_index: 0,
                },
                TextBuffer::new("focus"),
                UiKey("text".into()),
            ))
            .id();
        world.resource_mut::<UiFocus>().state_mut(window).focused = Some(text);
        world.resource_mut::<UiFocus>().set_active(window);
        world
            .resource_mut::<WidgetEventQueue>()
            .0
            .push(WidgetEvent::ValueChanged {
                target: checkbox,
                before: WidgetValue::Bool(false),
                after: WidgetValue::Bool(true),
            });
        record_widget_edits(&mut world);

        if reassign_window {
            world.get_mut::<bui_core::UiHitBox>(text).unwrap().window = Some(other_window);
        } else {
            world.entity_mut(text).insert(bui_core::UiEnabled(false));
        }
        let undo = world.resource_mut::<UiHistory>().undo().unwrap();
        assert!(apply_restore(&mut world, undo));
        assert_eq!(
            world.resource::<UiFocus>().state(window).unwrap().focused,
            None
        );
        assert_eq!(
            world.get::<Checkbox>(checkbox).unwrap().value,
            CheckboxValue::Off
        );
    }
}

#[test]
fn data_table_edits_restore_sort_selection_and_width_as_one_transaction() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let mut before = DataTable {
        columns: vec![crate::TableColumn::new("Name", 100.0)],
        rows: vec![
            crate::TableRow {
                id: 1,
                cells: vec!["zeta".into()],
            },
            crate::TableRow {
                id: 2,
                cells: vec!["alpha".into()],
            },
        ],
        ..DataTable::default()
    };
    let target = world
        .spawn((UiNode, UiFocusable, UiKey("table".into()), before.clone()))
        .id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
    world.resource_mut::<UiFocus>().set_active(window);
    let before_json = before.to_json().unwrap();
    before.resize_column(0, 180.0);
    before.sort_by(0, true);
    before.select_cell(2, 0, false, false);
    let after_json = before.to_json().unwrap();
    world.entity_mut(target).insert(before);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::DataTableCommitted {
            target,
            before: before_json,
            after: after_json,
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    let restored = world.get::<DataTable>(target).unwrap();
    assert_eq!(restored.columns[0].width, 100.0);
    assert_eq!(
        restored.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(restored.selection.active, None);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    let restored = world.get::<DataTable>(target).unwrap();
    assert_eq!(restored.columns[0].width, 180.0);
    assert_eq!(
        restored.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(restored.selection.active, Some(2));
}

#[test]
fn canvas_move_and_selection_restore_as_one_transaction() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let mut canvas = Canvas::default();
    let id = canvas
        .add(
            [10.0, 20.0],
            crate::CanvasShape::Rectangle { size: [30.0, 40.0] },
            [1.0; 4],
        )
        .unwrap();
    let target = world
        .spawn((UiNode, UiFocusable, UiKey("canvas".into()), canvas.clone()))
        .id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
    world.resource_mut::<UiFocus>().set_active(window);
    let before = canvas.to_json().unwrap();
    canvas.selected.insert(id);
    assert!(canvas.move_selected([7.0, -4.0]));
    let after = canvas.to_json().unwrap();
    world.entity_mut(target).insert(canvas);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::CanvasCommitted {
            target,
            before,
            after,
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    let restored = world.get::<Canvas>(target).unwrap();
    assert_eq!(restored.objects[0].origin, [10.0, 20.0]);
    assert!(restored.selected.is_empty());

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    let restored = world.get::<Canvas>(target).unwrap();
    assert_eq!(restored.objects[0].origin, [17.0, 16.0]);
    assert_eq!(restored.selected, std::collections::BTreeSet::from([id]));
}

#[test]
fn node_graph_move_restores_one_transaction() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let mut graph = NodeGraph::default();
    let id = graph.add_node("Source", [10.0, 20.0], vec![]);
    let target = world
        .spawn((UiNode, UiFocusable, UiKey("graph".into()), graph.clone()))
        .id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(target);
    world.resource_mut::<UiFocus>().set_active(window);
    let before = graph.to_json().unwrap();
    assert!(graph.select(id, false));
    assert!(graph.move_selected_by([15.0, -5.0]));
    let after = graph.to_json().unwrap();
    world.entity_mut(target).insert(graph);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::NodeGraphCommitted {
            target,
            before,
            after,
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    let graph = world.get::<NodeGraph>(target).unwrap();
    assert_eq!(graph.nodes[0].position, [10.0, 20.0]);
    assert!(graph.selected.is_empty());

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, target, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    let graph = world.get::<NodeGraph>(target).unwrap();
    assert_eq!(graph.nodes[0].position, [25.0, 15.0]);
    assert!(graph.selected.contains(&id));
}

#[test]
fn painted_checkboxes_and_numeric_scrubs_each_restore_as_one_gesture() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let first = world.spawn(Checkbox::new("a", true)).id();
    let second = world.spawn(Checkbox::new("b", true)).id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::CheckboxPaintCommitted {
            changes: vec![
                CheckboxPaintChange {
                    entity: first,
                    before: CheckboxValue::Off,
                    after: CheckboxValue::On,
                },
                CheckboxPaintChange {
                    entity: second,
                    before: CheckboxValue::Off,
                    after: CheckboxValue::On,
                },
            ],
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, first, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(
        world.get::<Checkbox>(first).unwrap().value,
        CheckboxValue::Off
    );
    assert_eq!(
        world.get::<Checkbox>(second).unwrap().value,
        CheckboxValue::Off
    );

    let numeric = world.spawn(NumericField::float(4.0).unwrap()).id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::NumericCommitted {
            target: numeric,
            before: vec![1.0, 2.0],
            after: vec![4.0, 5.0],
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, numeric, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<NumericField>(numeric).unwrap().value, 1.0);
}

#[test]
fn numeric_group_scrub_restores_all_fields_in_one_transaction() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let first = world.spawn(NumericField::float(4.0).unwrap()).id();
    let second = world.spawn(NumericField::float(12.0).unwrap()).id();
    world.resource_mut::<UiFocus>().state_mut(window).focused = Some(first);
    world.resource_mut::<UiFocus>().set_active(window);
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::NumericGroupCommitted {
            changes: vec![(first, 1.0, 4.0), (second, 10.0, 12.0)],
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, first, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<NumericField>(first).unwrap().value, 1.0);
    assert_eq!(world.get::<NumericField>(second).unwrap().value, 10.0);

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, first, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<NumericField>(first).unwrap().value, 4.0);
    assert_eq!(world.get::<NumericField>(second).unwrap().value, 12.0);
}

#[test]
fn slider_drag_undo_redo_restores_value_even_after_becoming_read_only() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let slider = world.spawn(Slider::new(7.0, 0.0, 10.0, 0.5).unwrap()).id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::NumericCommitted {
            target: slider,
            before: vec![4.0],
            after: vec![7.0],
        });
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 1);
    world.get_mut::<Slider>(slider).unwrap().read_only = true;
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, slider, UiKeyCode::Z, false)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<Slider>(slider).unwrap().value, 4.0);
    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, slider, UiKeyCode::Z, true)];
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<Slider>(slider).unwrap().value, 7.0);
}

#[test]
fn draining_the_widget_event_queue_does_not_lose_the_next_equal_sized_batch() {
    let mut world = setup();
    let target = world.spawn(Checkbox::new("enabled", false)).id();
    let event = WidgetEvent::ValueChanged {
        target,
        before: WidgetValue::Bool(false),
        after: WidgetValue::Bool(true),
    };
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(event.clone());
    record_widget_edits(&mut world);
    assert_eq!(world.resource_mut::<WidgetEventQueue>().take().len(), 1);
    world.resource_mut::<WidgetEventQueue>().0.push(event);
    record_widget_edits(&mut world);
    assert_eq!(world.resource::<UiHistory>().undo_len(), 2);
}

#[test]
fn a_text_buffer_undoes_locally_then_falls_back_to_widget_history() {
    let mut world = setup();
    let window = world.spawn_empty().id();
    let checkbox = world.spawn(Checkbox::new("enabled", true)).id();
    world
        .resource_mut::<WidgetEventQueue>()
        .0
        .push(WidgetEvent::ValueChanged {
            target: checkbox,
            before: WidgetValue::Bool(false),
            after: WidgetValue::Bool(true),
        });
    record_widget_edits(&mut world);
    let text = world.spawn(TextBuffer::new("a")).id();
    world.get_mut::<TextBuffer>(text).unwrap().insert_text("b");

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, text, UiKeyCode::Z, false)];
    process_text_events(&mut world);
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(world.get::<TextBuffer>(text).unwrap().as_str(), "a");
    assert_eq!(
        world.get::<Checkbox>(checkbox).unwrap().value,
        CheckboxValue::On
    );

    world.resource_mut::<UiRoutedEvents>().events =
        vec![key_event(window, text, UiKeyCode::Z, false)];
    process_text_events(&mut world);
    assert!(
        world
            .resource::<UiTextUndoFallback>()
            .needs_global_undo(text)
    );
    process_widget_undo_shortcuts(&mut world);
    assert_eq!(
        world.get::<Checkbox>(checkbox).unwrap().value,
        CheckboxValue::Off
    );
}

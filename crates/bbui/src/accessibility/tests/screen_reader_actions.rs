use super::*;

#[test]
pub(super) fn screen_reader_requests_cannot_activate_a_disabled_control() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .add_systems(Update, route_accessibility_actions);
    let mut button = Button::new("Unavailable");
    button.enabled = false;
    let target = app.world_mut().spawn((UiNode, UiFocusable, button)).id();
    sync_accessibility(app.world_mut());
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target.to_bits()),
            data: None,
        }));
    app.update();
    assert!(app.world().resource::<WidgetActionQueue>().is_empty());
    assert_ne!(app.world().resource::<UiFocus>().focused, Some(target));

    let mut field = TextField::new("locked");
    field.read_only = true;
    let text = app.world_mut().spawn((UiNode, UiFocusable, field)).id();
    sync_accessibility(app.world_mut());
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::ReplaceSelectedText,
            target_tree: TreeId::ROOT,
            target_node: NodeId(text.to_bits()),
            data: Some(ActionData::Value("changed".into())),
        }));
    app.update();
    assert!(app.world().resource::<WidgetActionQueue>().is_empty());
}

#[test]
pub(super) fn slider_screen_reader_step_and_set_value_use_widget_actions() {
    let mut app = App::new();
    app.add_plugins(bbui_widgets::WidgetPlugin)
        .add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .add_systems(
            Update,
            route_accessibility_actions.before(bbui_widgets::actions::apply_widget_actions),
        );
    let target = app
        .world_mut()
        .spawn((UiNode, Slider::new(4.0, 0.0, 10.0, 0.5).unwrap()))
        .id();
    for (action, data, expected) in [
        (Action::Increment, None, 4.5),
        (Action::SetValue, Some(ActionData::NumericValue(7.0)), 7.0),
    ] {
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data,
            }));
        app.update();
        assert_eq!(app.world().get::<Slider>(target).unwrap().value, expected);
    }
}

#[test]
pub(super) fn screen_reader_text_replacement_and_number_set_use_widget_actions() {
    let mut app = App::new();
    app.add_plugins(bbui_widgets::WidgetPlugin)
        .add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .add_systems(
            Update,
            route_accessibility_actions.before(bbui_widgets::actions::apply_widget_actions),
        );
    let mut field = TextField::new("old");
    field.select_all();
    let text = app.world_mut().spawn((UiNode, field)).id();
    let mut area = TextArea::new("note");
    area.field.select_all();
    let area = app.world_mut().spawn((UiNode, area)).id();
    let number = app
        .world_mut()
        .spawn((UiNode, NumericField::float(2.0).unwrap()))
        .id();
    for (target, action, data) in [
        (
            text,
            Action::ReplaceSelectedText,
            ActionData::Value("new".into()),
        ),
        (
            area,
            Action::ReplaceSelectedText,
            ActionData::Value("memo".into()),
        ),
        (number, Action::SetValue, ActionData::NumericValue(5.0)),
    ] {
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: Some(data),
            }));
        app.update();
    }
    assert_eq!(app.world().get::<TextField>(text).unwrap().text, "new");
    assert_eq!(
        app.world().get::<TextArea>(area).unwrap().field.text,
        "memo"
    );
    assert_eq!(app.world().get::<NumericField>(number).unwrap().value, 5.0);
}

#[test]
pub(super) fn file_browser_row_exposes_selection_and_click_action() {
    let mut world = World::new();
    let owner = world.spawn_empty().id();
    let row = world
        .spawn((
            UiNode,
            FileBrowserItem {
                owner,
                path: std::path::PathBuf::from("notes.txt"),
                name: "notes.txt".to_owned(),
                index: 2,
                directory: false,
                size: 12,
                selected: true,
                preview: bbui_widgets::PreviewKind::Text,
            },
        ))
        .id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(row).unwrap().0;
    assert_eq!(node.role(), Role::ListBoxOption);
    assert_eq!(node.label(), Some("notes.txt"));
    assert_eq!(node.is_selected(), Some(true));
    assert!(node.supports_action(Action::Click));
}

#[test]
pub(super) fn screen_reader_file_row_click_selects_the_browser_entry() {
    let root = std::env::temp_dir().join(format!(
        "bbui-accessible-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("notes.txt");
    std::fs::write(&path, b"notes").unwrap();

    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bbui_widgets::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bbui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let owner = app
        .world_mut()
        .spawn((UiNode, FileBrowser::new(&root).unwrap()))
        .id();
    // The browser canonicalizes its directory (notably /var on macOS), so use the
    // same entry path that the rendered row and selection state use.
    let path = app.world().get::<FileBrowser>(owner).unwrap().entries()[0]
        .path
        .clone();
    let row = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            FileBrowserItem {
                owner,
                path: path.clone(),
                name: "notes.txt".to_owned(),
                index: 0,
                directory: false,
                size: 5,
                selected: false,
                preview: bbui_widgets::PreviewKind::Text,
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(row.to_bits()),
            data: None,
        }));
    app.update();
    assert!(
        app.world()
            .get::<FileBrowser>(owner)
            .unwrap()
            .selected
            .contains(&path)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
pub(super) fn canvas_and_node_graph_expose_editor_summaries() {
    let mut world = World::new();
    let mut canvas = Canvas::default();
    let id = canvas
        .add(
            [0.0, 0.0],
            bbui_widgets::CanvasShape::Rectangle { size: [30.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    canvas.selected.insert(id);
    let canvas_entity = world
        .spawn((UiNode, Label::new("Layout canvas"), canvas))
        .id();
    let mut graph = NodeGraph::default();
    graph.add_node("Source", [0.0, 0.0], vec![]);
    let graph_entity = world.spawn((UiNode, graph)).id();
    sync_accessibility(&mut world);
    let canvas_node = &world.get::<AccessibilityNode>(canvas_entity).unwrap().0;
    assert_eq!(canvas_node.role(), Role::Canvas);
    assert_eq!(canvas_node.label(), Some("Layout canvas"));
    assert_eq!(canvas_node.value(), Some("1 objects, 1 selected"));
    let graph_node = &world.get::<AccessibilityNode>(graph_entity).unwrap().0;
    assert_eq!(graph_node.role(), Role::GraphicsDocument);
    assert_eq!(graph_node.value(), Some("1 nodes, 0 links, 0 selected"));
}

#[test]
pub(super) fn canvas_object_screen_reader_click_selects_visible_unlocked_shape() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bbui_widgets::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bbui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let mut canvas = Canvas::default();
    let id = canvas
        .add(
            [0.0, 0.0],
            bbui_widgets::CanvasShape::Rectangle { size: [20.0, 20.0] },
            [1.0; 4],
        )
        .unwrap();
    let owner = app.world_mut().spawn((UiNode, canvas)).id();
    let control = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            CanvasObjectControl {
                owner,
                object_id: id,
                label: "Rectangle 1".to_owned(),
                selected: false,
                locked: false,
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(control).unwrap().0;
    assert_eq!(node.role(), Role::GraphicsObject);
    assert_eq!(node.label(), Some("Rectangle 1"));
    assert!(node.supports_action(Action::Click));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(control.to_bits()),
            data: None,
        }));
    app.update();
    assert!(
        app.world()
            .get::<Canvas>(owner)
            .unwrap()
            .selected
            .contains(&id)
    );
    {
        let mut canvas = app.world_mut().get_mut::<Canvas>(owner).unwrap();
        canvas.selected.clear();
        canvas.objects[0].locked = true;
    }
    app.world_mut()
        .get_mut::<CanvasObjectControl>(control)
        .unwrap()
        .locked = true;
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(control).unwrap().0;
    assert!(node.is_disabled());
    assert!(!node.supports_action(Action::Click));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(control.to_bits()),
            data: None,
        }));
    app.update();
    assert!(
        app.world()
            .get::<Canvas>(owner)
            .unwrap()
            .selected
            .is_empty()
    );
}

#[test]
pub(super) fn color_ramp_exposes_selected_stop_and_routes_screen_reader_step() {
    let mut app = App::new();
    app.add_plugins(bbui_widgets::WidgetPlugin)
        .add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .add_systems(
            Update,
            route_accessibility_actions.before(bbui_widgets::actions::apply_widget_actions),
        );
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            Label::new("Gradient"),
            ColorRamp::default(),
        ))
        .id();
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(target).unwrap().0;
    assert_eq!(node.role(), Role::Slider);
    assert_eq!(node.label(), Some("Gradient"));
    assert_eq!(node.numeric_value(), Some(0.0));
    assert!(node.supports_action(Action::Increment));

    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Increment,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target.to_bits()),
            data: None,
        }));
    app.update();
    assert!(
        (app.world()
            .get::<ColorRamp>(target)
            .unwrap()
            .selected_stop()
            .unwrap()
            .position
            - 0.01)
            .abs()
            < 0.001
    );
}

#[test]
pub(super) fn hsv_editor_exposes_color_well_and_accepts_hex_action() {
    let mut app = App::new();
    app.add_plugins(bbui_widgets::WidgetPlugin)
        .add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .add_systems(
            Update,
            route_accessibility_actions.before(bbui_widgets::actions::apply_widget_actions),
        );
    let target = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            Label::new("Tint"),
            HsvColorEditor::default(),
        ))
        .id();
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(target).unwrap().0;
    assert_eq!(node.role(), Role::ColorWell);
    assert_eq!(node.label(), Some("Tint"));
    assert_eq!(
        node.color_value(),
        Some(AccessibleColor {
            red: 255,
            green: 0,
            blue: 0,
            alpha: 255,
        })
    );
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target.to_bits()),
            data: Some(ActionData::Value("#00FF80".into())),
        }));
    app.update();
    assert_eq!(
        app.world().get::<HsvColorEditor>(target).unwrap().hex(),
        "#00FF80FF"
    );
}

#[test]
pub(super) fn curve_editor_exposes_selected_point_and_routes_screen_reader_step() {
    let mut app = App::new();
    app.add_plugins(bbui_widgets::WidgetPlugin)
        .add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .add_systems(
            Update,
            route_accessibility_actions.before(bbui_widgets::actions::apply_widget_actions),
        );
    let mut editor = CurveEditor::default();
    assert!(editor.select_point(2));
    let target = app
        .world_mut()
        .spawn((UiNode, UiFocusable, Label::new("Ease"), editor))
        .id();
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(target).unwrap().0;
    assert_eq!(node.role(), Role::Slider);
    assert_eq!(node.label(), Some("Ease"));
    assert_eq!(node.numeric_value(), Some(1.0));
    assert!(node.supports_action(Action::Decrement));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Decrement,
            target_tree: TreeId::ROOT,
            target_node: NodeId(target.to_bits()),
            data: None,
        }));
    app.update();
    let point = app
        .world()
        .get::<CurveEditor>(target)
        .unwrap()
        .points
        .iter()
        .find(|point| point.id == 2)
        .unwrap();
    assert!((point.position[1] - 0.99).abs() < 0.001);
}

#[test]
pub(super) fn virtual_collection_items_expose_roles_selection_and_tree_expansion() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .add_systems(Update, route_accessibility_actions);
    let owner = app.world_mut().spawn(UiNode).id();
    let row = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            CollectionItemControl {
                owner,
                id: 7,
                kind: CollectionItemKind::Tree,
                index: 2,
                depth: 1,
                label: "Meshes".to_owned(),
                enabled: true,
                selected: true,
                expanded: false,
                has_children: true,
                asset_id: None,
                tint: [1.0; 4],
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    let node = &app.world().get::<AccessibilityNode>(row).unwrap().0;
    assert_eq!(node.role(), Role::TreeItem);
    assert_eq!(node.label(), Some("Meshes"));
    assert_eq!(node.level(), Some(2));
    assert_eq!(node.is_selected(), Some(true));
    assert_eq!(node.is_expanded(), Some(false));
    assert!(node.supports_action(Action::Expand));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Expand,
            target_tree: TreeId::ROOT,
            target_node: NodeId(row.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(app.world().resource::<WidgetActionQueue>().len(), 1);
}

#[test]
pub(super) fn screen_reader_click_selects_a_virtual_list_item() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bbui_widgets::actions::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bbui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let mut model = ListModel::<String>::default();
    model.items.push(bbui_widgets::ListItem {
        id: 7,
        value: "Mesh".to_owned(),
        enabled: true,
    });
    let owner = app.world_mut().spawn((UiNode, model)).id();
    let row = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            CollectionItemControl {
                owner,
                id: 7,
                kind: CollectionItemKind::List,
                index: 0,
                depth: 0,
                label: "Mesh".to_owned(),
                enabled: true,
                selected: false,
                expanded: false,
                has_children: false,
                asset_id: None,
                tint: [1.0; 4],
            },
        ))
        .id();
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(row.to_bits()),
            data: None,
        }));
    app.update();
    assert!(
        app.world()
            .get::<ListModel<String>>(owner)
            .unwrap()
            .selection
            .contains(7)
    );
}

#[test]
pub(super) fn screen_reader_expands_tree_and_selects_preview_cell() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bbui_widgets::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bbui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let tree = TreeModel::<String> {
        nodes: vec![
            bbui_widgets::TreeNode {
                id: 1,
                parent: None,
                children: vec![2],
                value: "Root".to_owned(),
                expanded: false,
                enabled: true,
            },
            bbui_widgets::TreeNode {
                id: 2,
                parent: Some(1),
                children: Vec::new(),
                value: "Child".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..Default::default()
    };
    let tree_owner = app.world_mut().spawn((UiNode, tree)).id();
    let tree_row = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(tree_owner),
            CollectionItemControl {
                owner: tree_owner,
                id: 1,
                kind: CollectionItemKind::Tree,
                index: 0,
                depth: 0,
                label: "Root".to_owned(),
                enabled: true,
                selected: false,
                expanded: false,
                has_children: true,
                asset_id: None,
                tint: [1.0; 4],
            },
        ))
        .id();
    let mut grid = PreviewGrid::default();
    grid.items
        .push(bbui_widgets::PreviewGridItem::new(7, "Material"));
    let grid_owner = app.world_mut().spawn((UiNode, grid)).id();
    let grid_cell = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(grid_owner),
            CollectionItemControl {
                owner: grid_owner,
                id: 7,
                kind: CollectionItemKind::PreviewGrid,
                index: 0,
                depth: 0,
                label: "Material".to_owned(),
                enabled: true,
                selected: false,
                expanded: false,
                has_children: false,
                asset_id: None,
                tint: [1.0; 4],
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(grid_cell)
            .unwrap()
            .0
            .role(),
        Role::GridCell
    );
    for (target, action) in [(tree_row, Action::Expand), (grid_cell, Action::Click)] {
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
    }
    assert!(
        app.world()
            .get::<TreeModel<String>>(tree_owner)
            .unwrap()
            .nodes[0]
            .expanded
    );
    assert!(
        app.world()
            .get::<PreviewGrid>(grid_owner)
            .unwrap()
            .selection
            .contains(7)
    );
}

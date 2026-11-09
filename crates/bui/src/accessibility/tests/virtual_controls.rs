use super::*;

#[test]
pub(super) fn tree_item_position_uses_sibling_order_after_reorder() {
    let mut world = World::new();
    let tree = TreeModel::<String> {
        nodes: vec![
            bui_widgets::TreeNode {
                id: 1,
                parent: None,
                children: vec![3, 2],
                value: "Root".to_owned(),
                expanded: true,
                enabled: true,
            },
            bui_widgets::TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![],
                value: "Second".to_owned(),
                expanded: false,
                enabled: true,
            },
            bui_widgets::TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
                value: "First".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..Default::default()
    };
    let owner = world.spawn((UiNode, tree)).id();
    let row = world
        .spawn((
            UiNode,
            CollectionItemControl {
                owner,
                id: 3,
                kind: CollectionItemKind::Tree,
                index: 2,
                depth: 1,
                label: "First".to_owned(),
                enabled: true,
                selected: false,
                expanded: false,
                has_children: false,
                asset_id: None,
                tint: [1.0; 4],
            },
        ))
        .id();
    sync_accessibility(&mut world);
    let node = &world.get::<AccessibilityNode>(row).unwrap().0;
    assert_eq!(node.position_in_set(), Some(1));
    assert_eq!(node.size_of_set(), Some(2));
}

#[test]
pub(super) fn table_grid_cells_and_headers_route_accessible_selection_and_resize() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bui_widgets::actions::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let table = DataTable {
        columns: vec![bui_widgets::TableColumn::new("Name", 120.0)],
        rows: vec![bui_widgets::TableRow {
            id: 42,
            cells: vec!["Cube".to_owned()],
        }],
        ..Default::default()
    };
    let owner = app
        .world_mut()
        .spawn((UiNode, Label::new("Objects"), table))
        .id();
    let header = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            DataTableHeaderControl {
                owner,
                column: 0,
                label: "Name".to_owned(),
                width: 120.0,
                sortable: true,
                sort_ascending: Some(true),
                active: true,
            },
        ))
        .id();
    let cell = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            DataTableCellControl {
                owner,
                row_id: 42,
                row_index: 0,
                column: 0,
                column_label: "Name".to_owned(),
                value: "Cube".to_owned(),
                selected: false,
                active: false,
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    let root = &app.world().get::<AccessibilityNode>(owner).unwrap().0;
    assert_eq!(root.role(), Role::Grid);
    assert_eq!(root.row_count(), Some(1));
    assert_eq!(root.column_count(), Some(1));
    let header_node = &app.world().get::<AccessibilityNode>(header).unwrap().0;
    assert_eq!(header_node.role(), Role::ColumnHeader);
    assert_eq!(header_node.sort_direction(), Some(SortDirection::Ascending));
    let cell_node = &app.world().get::<AccessibilityNode>(cell).unwrap().0;
    assert_eq!(cell_node.role(), Role::GridCell);
    assert_eq!(cell_node.label(), Some("Name, Cube"));
    assert_eq!(cell_node.row_index(), Some(0));
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(cell.to_bits()),
            data: None,
        }));
    app.update();
    assert!(
        app.world()
            .get::<DataTable>(owner)
            .unwrap()
            .selection
            .contains(42)
    );
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Increment,
            target_tree: TreeId::ROOT,
            target_node: NodeId(header.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(
        app.world().get::<DataTable>(owner).unwrap().columns[0].width,
        128.0
    );
    app.world_mut()
        .write_message(ActionRequest(AccessKitActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: NodeId(header.to_bits()),
            data: None,
        }));
    app.update();
    assert_eq!(
        app.world().get::<DataTable>(owner).unwrap().sort,
        Some((0, true))
    );
}

#[test]
pub(super) fn screen_reader_can_connect_node_sockets_in_either_focus_order() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bui_widgets::WidgetEventQueue>()
        .init_resource::<AccessibilityLinkSelection>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [0.0, 0.0],
        vec![bui_widgets::NodeSocket::output(
            "Value",
            bui_widgets::SocketType::Float,
        )],
    );
    let sink = graph.add_node(
        "Sink",
        [220.0, 0.0],
        vec![bui_widgets::NodeSocket::input(
            "Value",
            bui_widgets::SocketType::Float,
        )],
    );
    let owner = app.world_mut().spawn((UiNode, graph)).id();
    let output = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            NodeGraphSocketControl {
                owner,
                node_id: source,
                socket_index: 0,
                direction: SocketDirection::Output,
                label: "Value".to_owned(),
                socket_type: bui_widgets::SocketType::Float,
            },
        ))
        .id();
    let input = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            NodeGraphSocketControl {
                owner,
                node_id: sink,
                socket_index: 0,
                direction: SocketDirection::Input,
                label: "Value".to_owned(),
                socket_type: bui_widgets::SocketType::Float,
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    let output_node = &app.world().get::<AccessibilityNode>(output).unwrap().0;
    assert_eq!(output_node.role(), Role::Button);
    assert!(output_node.supports_action(Action::Click));
    assert!(output_node.description().unwrap().contains("create a link"));

    let click = |app: &mut App, target: Entity| {
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
    };
    click(&mut app, output);
    sync_accessibility(app.world_mut());
    let selected = &app.world().get::<AccessibilityNode>(output).unwrap().0;
    assert_eq!(selected.is_selected(), Some(true));
    assert_eq!(selected.live(), Some(Live::Polite));
    click(&mut app, output);
    click(&mut app, input);
    assert!(
        app.world()
            .get::<NodeGraph>(owner)
            .unwrap()
            .links
            .is_empty()
    );
    click(&mut app, output);
    let graph = app.world().get::<NodeGraph>(owner).unwrap();
    assert_eq!(graph.links.len(), 1);
    assert_eq!(graph.links[0].from_node, source);
    assert_eq!(graph.links[0].to_node, sink);
    let link_id = graph.links[0].id;
    app.world_mut()
        .get_mut::<NodeGraph>(owner)
        .unwrap()
        .remove_link(link_id);
    click(&mut app, output);
    click(&mut app, input);
    assert_eq!(app.world().get::<NodeGraph>(owner).unwrap().links.len(), 1);
    click(&mut app, output);
    app.world_mut().entity_mut(output).despawn();
    sync_accessibility(app.world_mut());
    assert!(
        app.world()
            .resource::<AccessibilityLinkSelection>()
            .0
            .is_none()
    );
}

#[test]
pub(super) fn node_editor_virtual_controls_route_screen_reader_selection_and_collapse() {
    let mut app = App::new();
    app.add_message::<ActionRequest>()
        .init_resource::<UiFocus>()
        .init_resource::<WidgetActionQueue>()
        .init_resource::<bui_widgets::WidgetEventQueue>()
        .add_systems(
            Update,
            (
                route_accessibility_actions,
                bui_widgets::actions::apply_widget_actions,
            )
                .chain(),
        );
    let mut graph = NodeGraph::default();
    let source = graph.add_node(
        "Source",
        [0.0, 0.0],
        vec![bui_widgets::NodeSocket::output(
            "Value",
            bui_widgets::SocketType::Float,
        )],
    );
    let sink = graph.add_node(
        "Sink",
        [220.0, 0.0],
        vec![bui_widgets::NodeSocket::input(
            "Value",
            bui_widgets::SocketType::Float,
        )],
    );
    let link_id = graph.create_link(source, 0, sink, 0).unwrap();
    let owner = app.world_mut().spawn((UiNode, graph)).id();
    let node = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            NodeGraphNodeControl {
                owner,
                node_id: source,
                label: "Source".to_owned(),
                selected: false,
                collapsed: false,
            },
        ))
        .id();
    let socket = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            NodeGraphSocketControl {
                owner,
                node_id: source,
                socket_index: 0,
                direction: bui_widgets::SocketDirection::Output,
                label: "Value".to_owned(),
                socket_type: bui_widgets::SocketType::Float,
            },
        ))
        .id();
    let link = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            NodeGraphLinkControl {
                owner,
                link_id,
                from_node: source,
                from_socket: 0,
                to_node: sink,
                to_socket: 0,
                selected: false,
            },
        ))
        .id();
    sync_accessibility(app.world_mut());
    assert_eq!(
        app.world().get::<AccessibilityNode>(node).unwrap().0.role(),
        Role::Group
    );
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(socket)
            .unwrap()
            .0
            .role(),
        Role::Button
    );
    assert_eq!(
        app.world().get::<AccessibilityNode>(link).unwrap().0.role(),
        Role::Button
    );
    for (action, target) in [
        (Action::Click, node),
        (Action::Collapse, node),
        (Action::Click, link),
    ] {
        app.world_mut()
            .write_message(ActionRequest(AccessKitActionRequest {
                action,
                target_tree: TreeId::ROOT,
                target_node: NodeId(target.to_bits()),
                data: None,
            }));
        app.update();
    }
    let graph = app.world().get::<NodeGraph>(owner).unwrap();
    assert!(graph.nodes[0].collapsed);
    assert!(graph.selected.is_empty());
    assert_eq!(graph.selected_link, Some(link_id));
}

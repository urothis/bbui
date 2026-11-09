use super::*;

#[test]
fn list_selection_supports_range_and_virtual_visible_rows() {
    let mut list = ListModel {
        items: (0..1000)
            .map(|id| ListItem {
                id,
                value: id,
                enabled: true,
            })
            .collect(),
        ..ListModel::default()
    };
    list.select(10, false, false);
    list.select(15, false, true);
    assert_eq!(list.selection.selected.len(), 6);
    list.scroll_offset = 100.0 * list.row_height;
    assert_eq!(list.visible_items(44.0).count(), 3);
    assert_eq!(list.visible_range(44.0), 100..103);
}

#[test]
fn tree_flattening_honors_expansion_and_keeps_identity() {
    let mut tree = TreeModel {
        nodes: vec![
            TreeNode {
                id: 1,
                parent: None,
                children: vec![2, 3],
                value: "root",
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![],
                value: "a",
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
                value: "b",
                expanded: false,
                enabled: true,
            },
        ],
        ..TreeModel::default()
    };
    assert_eq!(tree.visible_ids(), vec![1]);
    tree.toggle_expanded(1);
    assert_eq!(tree.visible_ids(), vec![1, 2, 3]);
    tree.select(3, false, false);
    assert_eq!(tree.selection.active, Some(3));
}

#[test]
fn collection_models_round_trip_and_reject_invalid_hierarchies() {
    let list = ListModel {
        items: vec![
            ListItem {
                id: 41,
                value: "Assets".to_owned(),
                enabled: true,
            },
            ListItem {
                id: 7,
                value: "Materials".to_owned(),
                enabled: false,
            },
        ],
        ..ListModel::default()
    };
    let encoded = list.to_json().unwrap();
    assert_eq!(ListModel::<String>::from_json(&encoded).unwrap(), list);

    let mut tree = TreeModel {
        nodes: vec![
            TreeNode {
                id: 1,
                parent: None,
                children: vec![2, 3],
                value: "Root".to_owned(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![],
                value: "Child A".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
                value: "Child B".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 4,
                parent: None,
                children: vec![],
                value: "Other root".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..TreeModel::default()
    };
    assert!(tree.move_node(3, Some(4), 0));
    assert_eq!(
        tree.nodes
            .iter()
            .find(|node| node.id == 4)
            .unwrap()
            .children,
        [3]
    );
    assert_eq!(
        tree.nodes.iter().find(|node| node.id == 3).unwrap().parent,
        Some(4)
    );
    assert!(!tree.move_node(1, Some(2), 0));
    tree.validate().unwrap();
    let encoded = tree.to_json().unwrap();
    assert_eq!(TreeModel::<String>::from_json(&encoded).unwrap(), tree);

    let grid = PreviewGrid {
        items: vec![
            PreviewGridItem::new(17, "Cube"),
            PreviewGridItem::new(5, "Sphere"),
        ],
        ..PreviewGrid::default()
    };
    let encoded = grid.to_json().unwrap();
    assert_eq!(PreviewGrid::from_json(&encoded).unwrap(), grid);
}

#[test]
fn tree_reorder_uses_sibling_order_and_preserves_parent_links() {
    let mut tree = TreeModel {
        nodes: vec![
            TreeNode {
                id: 1,
                parent: None,
                children: vec![2, 3, 4],
                value: "Root".to_owned(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 2,
                parent: Some(1),
                children: vec![],
                value: "A".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
                value: "B".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 4,
                parent: Some(1),
                children: vec![],
                value: "C".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..TreeModel::default()
    };
    assert!(tree.reorder_sibling(2, 2));
    assert_eq!(tree.nodes[0].children, [3, 4, 2]);
    tree.validate().unwrap();
}

#[test]
fn table_sort_resize_selection_and_virtualization_share_row_identity() {
    let mut table = DataTable {
        columns: vec![TableColumn::new("Name", 100.0)],
        rows: vec![
            TableRow {
                id: 1,
                cells: vec!["z".into()],
            },
            TableRow {
                id: 2,
                cells: vec!["a".into()],
            },
        ],
        ..DataTable::default()
    };
    assert!(table.columns[0].resize(140.0));
    assert!(table.sort_by(0, true));
    assert_eq!(table.rows[0].id, 2);
    table.select(2, false, false);
    assert_eq!(table.visible_rows(0, 1)[0].id, 2);
    assert_eq!(table.selection.active, Some(2));
}

#[test]
fn data_table_sorts_stably_resizes_clamps_and_round_trips() {
    let mut table = DataTable {
        columns: vec![TableColumn {
            label: "Name".into(),
            width: 100.0,
            min_width: 40.0,
            max_width: 220.0,
            sortable: true,
        }],
        rows: vec![
            TableRow {
                id: 8,
                cells: vec!["same".into()],
            },
            TableRow {
                id: 3,
                cells: vec!["same".into()],
            },
            TableRow {
                id: 5,
                cells: vec!["alpha".into()],
            },
        ],
        ..DataTable::default()
    };
    assert!(table.resize_column(0, 900.0));
    assert_eq!(table.columns[0].width, 220.0);
    assert!(table.sort_by(0, false));
    assert_eq!(
        table.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [3, 8, 5]
    );
    let json = table.to_json().unwrap();
    assert_eq!(DataTable::from_json(&json).unwrap(), table);
}

#[test]
fn data_table_sorts_numeric_columns_by_value() {
    let mut table = DataTable {
        columns: vec![TableColumn::new("Vertices", 100.0)],
        rows: vec![
            TableRow {
                id: 1,
                cells: vec!["100".into()],
            },
            TableRow {
                id: 2,
                cells: vec!["20".into()],
            },
            TableRow {
                id: 3,
                cells: vec!["20".into()],
            },
        ],
        ..DataTable::default()
    };

    assert!(table.sort_by(0, true));
    assert_eq!(
        table.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [2, 3, 1]
    );
}

#[test]
fn data_table_validation_rejects_duplicate_ids_and_bad_cell_shapes() {
    let mut table = DataTable {
        columns: vec![TableColumn::new("Name", 120.0)],
        rows: vec![
            TableRow {
                id: 1,
                cells: vec!["A".into()],
            },
            TableRow {
                id: 1,
                cells: vec!["B".into()],
            },
        ],
        ..DataTable::default()
    };
    assert!(table.validate().unwrap_err().contains("unique"));
    table.rows[1].id = 2;
    table.rows[1].cells.clear();
    assert!(table.validate().unwrap_err().contains("cell counts"));
}

#[test]
fn data_table_cell_selection_and_navigation_preserve_stable_row_identity() {
    let mut table = DataTable {
        columns: vec![TableColumn::new("A", 80.0), TableColumn::new("B", 80.0)],
        rows: (1..=6)
            .map(|id| TableRow {
                id,
                cells: vec![format!("{id}a"), format!("{id}b")],
            })
            .collect(),
        ..DataTable::default()
    };
    assert!(table.select_cell(2, 1, false, false));
    assert!(table.select_cell(4, 0, false, true));
    assert_eq!(table.selection.selected, [2, 3, 4].into());
    assert_eq!(table.active_column, 0);
    assert_eq!(table.move_active_column(1), Some(1));
    assert_eq!(table.move_active(1, false), Some(5));
    assert_eq!(table.selection.selected, [5].into());
}

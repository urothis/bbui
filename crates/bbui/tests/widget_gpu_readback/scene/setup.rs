use super::*;

#[allow(clippy::unwrap_used, clippy::expect_used)]
pub(super) fn build_scene() -> Scene {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    force_fallback_adapter: true,
                    ..Default::default()
                })),
                ..Default::default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::pbr::PbrPlugin>(),
    )
    .add_plugins(BbuiPlugin)
    .init_resource::<CapturedFrames>();
    app.finish();
    app.cleanup();

    let mut target = Image::new_target_texture(
        TARGET_WIDTH,
        TARGET_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(target.clone().into()),
            IsDefaultUiCamera,
        ))
        .id();

    let mut list_model = ListModel::<String> {
        row_height: 28.0,
        items: vec![
            ListItem {
                id: 1,
                value: "Files".to_owned(),
                enabled: true,
            },
            ListItem {
                id: 2,
                value: "Selected list row".to_owned(),
                enabled: true,
            },
            ListItem {
                id: 3,
                value: "Textures".to_owned(),
                enabled: true,
            },
        ],
        ..Default::default()
    };
    assert!(list_model.select(2, false, false));

    let mut tree_model = TreeModel::<String> {
        nodes: vec![
            TreeNode {
                id: 10,
                parent: None,
                children: vec![11],
                value: "Scene".to_owned(),
                expanded: true,
                enabled: true,
            },
            TreeNode {
                id: 11,
                parent: Some(10),
                children: Vec::new(),
                value: "Selected child".to_owned(),
                expanded: false,
                enabled: true,
            },
            TreeNode {
                id: 12,
                parent: None,
                children: Vec::new(),
                value: "Materials".to_owned(),
                expanded: false,
                enabled: true,
            },
        ],
        ..Default::default()
    };
    assert!(tree_model.select(11, false, false));

    let mut preview_grid = PreviewGrid {
        cell_width: 112.0,
        cell_height: 104.0,
        items: vec![
            PreviewGridItem {
                tint: [0.90, 0.20, 0.10, 1.0],
                ..PreviewGridItem::new(20, "Warm")
            },
            PreviewGridItem {
                tint: [0.10, 0.75, 0.25, 1.0],
                ..PreviewGridItem::new(21, "Selected tile")
            },
            PreviewGridItem {
                tint: [0.20, 0.35, 0.95, 1.0],
                ..PreviewGridItem::new(22, "Cool")
            },
        ],
        ..Default::default()
    };
    assert!(preview_grid.select(21, false, false));

    let tree = WidgetTree::new(WidgetNode::column([
        WidgetNode::row([
            WidgetNode::menu(
                "File",
                [
                    MenuEntry::command("open", "Open").with_shortcut("Ctrl+O"),
                    MenuEntry::command("save", "Save").with_shortcut("Ctrl+S"),
                    MenuEntry::command("quit", "Quit"),
                ],
            ),
            WidgetNode::hsv_color("Primary", Rgba(0.20, 0.40, 0.80, 1.0)),
        ]),
        WidgetNode::curve_editor_default("Ease"),
        WidgetNode::color_ramp_default("Gradient"),
        WidgetNode::row([
            WidgetNode::list("Assets", list_model),
            WidgetNode::tree("Hierarchy", tree_model),
            WidgetNode::preview_grid("Previews", preview_grid),
        ]),
    ]));
    let root = tree.spawn(app.world_mut(), None).unwrap();
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(root).unwrap();
        layout.size.width = UiLength::Px(TARGET_WIDTH as f32);
        layout.size.height = UiLength::Px(TARGET_HEIGHT as f32);
    }
    app.world_mut().entity_mut(root).insert((
        UiLayoutRoot::new(
            camera,
            Rect::new(0.0, 0.0, TARGET_WIDTH as f32, TARGET_HEIGHT as f32),
            1.0,
        ),
        UiPaint(Primitive::RoundedRect {
            rect: ClipRect {
                x: 0.0,
                y: 0.0,
                width: TARGET_WIDTH as f32,
                height: TARGET_HEIGHT as f32,
            },
            radius: 0.0,
            color: Theme::default().background,
        }),
    ));
    let mut data_table = DataTable {
        columns: [
            ("Name", 160.0),
            ("Type", 110.0),
            ("Status", 110.0),
            ("Owner", 110.0),
            ("Modified", 110.0),
            ("Size", 110.0),
        ]
        .into_iter()
        .map(|(label, width)| TableColumn::new(label, width))
        .collect(),
        rows: (1..=96)
            .map(|id| TableRow {
                id,
                cells: vec![
                    format!("Asset {id:04}"),
                    if id % 2 == 0 { "Mesh" } else { "Image" }.to_owned(),
                    match id % 3 {
                        0 => "Clean",
                        1 => "Modified",
                        _ => "Ready",
                    }
                    .to_owned(),
                    format!("Scene {}", id % 8),
                    format!("2026-11-{:02}", id % 28 + 1),
                    format!("{} KB", id * 17),
                ],
            })
            .collect(),
        row_height: 24,
        scroll_x: 170.0,
        active_column: 3,
        ..Default::default()
    };
    assert!(data_table.select_cell(40, 3, false, false));
    assert!(data_table.sort_by(2, true));
    let active_row_index = data_table.rows.iter().position(|row| row.id == 40).unwrap();
    data_table.scroll_rows = active_row_index.saturating_sub(6);
    let table_viewport = Rect::new(650.0, 20.0, 540.0, 390.0);
    let table_root = WidgetTree::new(WidgetNode::data_table("Virtual inventory", data_table))
        .spawn(app.world_mut(), None)
        .unwrap();
    app.world_mut()
        .entity_mut(table_root)
        .insert(UiLayoutRoot::new(camera, table_viewport, 1.0));
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(table_root).unwrap();
        layout.size.width = UiLength::Px(table_viewport.width);
        layout.size.height = UiLength::Px(table_viewport.height);
        layout.clip_contents = true;
        layout.z_index = 8;
    }
    // A second viewport shares the target camera but keeps this dropdown near the lower-right
    // edge. Its popup must flip above the trigger, stay clipped to this inset viewport, and
    // retain stable selected/highlighted option rows.
    let dropdown_window = app.world_mut().spawn_empty().id();
    let dropdown_viewport = Rect::new(850.0, 700.0, 330.0, 470.0);
    let dropdown_viewport_root = app
        .world_mut()
        .spawn(UiLayoutRoot::new(dropdown_window, dropdown_viewport, 1.0))
        .id();
    let dropdown = WidgetTree::new(WidgetNode::dropdown(
        "Quality",
        (0..10).map(|index| format!("Quality {index}")),
        Some(1),
    ))
    .spawn(app.world_mut(), None)
    .unwrap();
    {
        let mut model = app.world_mut().get_mut::<Dropdown>(dropdown).unwrap();
        model.open();
        model.highlighted = Some(4);
    }
    app.world_mut().entity_mut(dropdown).insert(UiLayoutResult {
        window: dropdown_window,
        rect: Rect::new(1060.0, 1120.0, 130.0, 28.0),
        clip: None,
        z: 20,
        content_size: [130.0, 28.0],
        dpi_scale: 1.0,
    });
    let root_children = app.world().get::<UiChildren>(root).unwrap().0.clone();
    let header = root_children[0];
    let curve = root_children[1];
    let ramp_widget = root_children[2];
    let collection_row = root_children[3];
    let collection_widgets = app
        .world()
        .get::<UiChildren>(collection_row)
        .unwrap()
        .0
        .clone();
    let list_widget = collection_widgets[0];
    let tree_widget = collection_widgets[1];
    let preview_grid_widget = collection_widgets[2];
    {
        let mut layout = app.world_mut().get_mut::<UiLayoutNode>(curve).unwrap();
        layout.size.width = UiLength::Px(680.0);
        layout.size.height = UiLength::Px(340.0);
    }
    let header_children = app.world().get::<UiChildren>(header).unwrap().0.clone();
    let menu = header_children[0];
    let editor = header_children[1];
    {
        let mut curve_editor = app.world_mut().get_mut::<CurveEditor>(curve).unwrap();
        curve_editor.points[0].position = [0.0, 0.16];
        curve_editor.points[1].position = [1.0, 0.26];
        let middle = curve_editor.add_point(0.5, 0.78).unwrap();
        assert!(curve_editor.set_handles(middle, [-0.13, -0.24], [0.13, 0.16]));
        curve_editor.selected = Some(middle);
    }
    {
        let mut ramp = app.world_mut().get_mut::<ColorRamp>(ramp_widget).unwrap();
        assert!(ramp.set_color(1, Rgba(0.90, 0.10, 0.10, 1.0)));
        assert!(ramp.set_color(2, Rgba(0.10, 0.20, 0.90, 1.0)));
        assert!(ramp.add_stop(0.5, Rgba(0.10, 0.80, 0.20, 1.0)).is_some());
        ramp.interpolation = Interpolation::Smooth;
    }
    app.world_mut()
        .get_mut::<MenuControl>(menu)
        .unwrap()
        .session
        .open(None);

    Scene {
        app,
        target,
        root,
        table_root,
        table_viewport,
        dropdown,
        dropdown_viewport,
        dropdown_viewport_root,
        list_widget,
        tree_widget,
        preview_grid_widget,
        curve,
        ramp_widget,
        menu,
        editor,
    }
}

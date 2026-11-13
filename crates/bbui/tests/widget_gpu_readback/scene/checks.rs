use super::*;

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub(super) fn verify_scene(scene: Scene) {
    let Scene {
        mut app,
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
    } = scene;
    for _ in 0..10 {
        app.update();
        thread::sleep(Duration::from_millis(5));
    }
    for (scenario, (kind, dpi_scale)) in [
        (ThemeKind::Dark, 1.0),
        (ThemeKind::Dark, 2.0),
        (ThemeKind::Light, 1.0),
        (ThemeKind::Light, 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        let theme = Theme::new(kind);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();
        app.world_mut()
            .get_mut::<UiLayoutRoot>(root)
            .unwrap()
            .dpi_scale = dpi_scale;
        app.world_mut()
            .get_mut::<UiLayoutRoot>(table_root)
            .unwrap()
            .dpi_scale = dpi_scale;
        app.world_mut()
            .get_mut::<UiLayoutRoot>(dropdown_viewport_root)
            .unwrap()
            .dpi_scale = dpi_scale;
        app.world_mut()
            .get_mut::<bbui_layout::UiLayoutResult>(dropdown)
            .unwrap()
            .dpi_scale = dpi_scale;
        if let Primitive::RoundedRect { color, .. } =
            &mut app.world_mut().get_mut::<UiPaint>(root).unwrap().0
        {
            *color = theme.background;
        }
        for _ in 0..3 {
            app.update();
            thread::sleep(Duration::from_millis(5));
        }

        for (mode_index, interpolation) in [
            Interpolation::Constant,
            Interpolation::Linear,
            Interpolation::Smooth,
        ]
        .into_iter()
        .enumerate()
        {
            app.world_mut()
                .get_mut::<MenuControl>(menu)
                .unwrap()
                .session
                .open(None);
            app.world_mut()
                .get_mut::<CurveEditor>(curve)
                .unwrap()
                .interpolation = interpolation;
            for _ in 0..3 {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }

            let (wheel_rect, popup_item_rect, curve_rect, curve_model, ramp_rect, ramp_model) = {
                let display_items = app.world().resource::<DisplayList>().items();
                let wheels = display_items
                    .iter()
                    .filter_map(|item| match &item.primitive {
                        Primitive::HsvWheel { rect, .. } => Some(*rect),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    wheels.len(),
                    1,
                    "the HSV widget must emit one wheel primitive"
                );

                let menu_control = app.world().get::<MenuControl>(menu).unwrap();
                assert!(menu_control.session.open);
                let popup_item = menu_control
                    .items
                    .iter()
                    .find(|entity| {
                        app.world()
                            .get::<bbui_widgets::MenuItemControl>(**entity)
                            .is_some_and(|item| item.id == "open")
                    })
                    .copied()
                    .expect("opened menu should expose its Open item");
                assert!(
                    app.world()
                        .get::<bbui_core::UiVisible>(popup_item)
                        .unwrap()
                        .0
                );
                let popup_rect = app
                    .world()
                    .get::<bbui_layout::UiLayoutResult>(popup_item)
                    .unwrap()
                    .rect;
                let curve_rect = app
                    .world()
                    .get::<bbui_layout::UiLayoutResult>(curve)
                    .unwrap()
                    .rect;
                let curve_model = app.world().get::<CurveEditor>(curve).unwrap().clone();
                let ramp_rect = app
                    .world()
                    .get::<bbui_layout::UiLayoutResult>(ramp_widget)
                    .unwrap()
                    .rect;
                let ramp_model = app.world().get::<ColorRamp>(ramp_widget).unwrap().clone();
                (
                    wheels[0],
                    popup_rect,
                    curve_rect,
                    curve_model,
                    ramp_rect,
                    ramp_model,
                )
            };

            let frame = scenario * 3 + mode_index;
            let (pixels, readback_latency) = capture_frame(&mut app, &target, frame);
            if mode_index == 0
                && let Some(directory) = std::env::var_os("BBUI_WIDGET_CAPTURE_DIR")
            {
                let directory = std::path::PathBuf::from(directory);
                fs::create_dir_all(&directory).expect("create widget capture directory");
                write_capture_ppm(
                    &directory.join(format!("{kind:?}-dpi{}.ppm", dpi_scale as u32)),
                    &pixels,
                );
            }

            let popup = *app
                .world()
                .get::<DropdownPopupState>(dropdown)
                .expect("open dropdown should expose viewport and popup geometry");
            assert!(popup.popup.open);
            assert_eq!(popup.popup.side, bbui_widgets::menus::PopupSide::Above);
            assert_eq!(popup.viewport, dropdown_viewport);
            assert_eq!(popup.visible_count, 8);
            assert!(popup.popup.bounds.x >= popup.viewport.x);
            assert!(popup.popup.bounds.y >= popup.viewport.y);
            assert!(
                popup.popup.bounds.x + popup.popup.bounds.width
                    <= popup.viewport.x + popup.viewport.width
            );
            assert!(
                popup.popup.bounds.y + popup.popup.bounds.height
                    <= popup.viewport.y + popup.viewport.height
            );
            let (normal_entity, normal_rect, normal_option) =
                dropdown_option(app.world(), dropdown, 0);
            let (_, selected_rect, selected_option) = dropdown_option(app.world(), dropdown, 1);
            let (_, highlighted_rect, highlighted_option) =
                dropdown_option(app.world(), dropdown, 4);
            assert!(!normal_option.selected && !normal_option.highlighted);
            assert!(selected_option.selected && !selected_option.highlighted);
            assert!(!highlighted_option.selected && highlighted_option.highlighted);
            assert_eq!(
                app.world()
                    .get::<bbui_layout::UiLayoutResult>(normal_entity)
                    .unwrap()
                    .clip,
                Some(popup.popup.bounds),
                "option text and fill must be clipped to popup bounds"
            );
            let selected_color = theme.color(ThemeColorToken::MenuSelectedSurface);
            for (rect, color, description) in [
                (normal_rect, theme.surface, "normal dropdown option"),
                (selected_rect, selected_color, "selected dropdown option"),
                (
                    highlighted_rect,
                    selected_color,
                    "highlighted dropdown option",
                ),
            ] {
                assert_color(
                    pixel_at(
                        &pixels,
                        (rect.x + rect.width - 12.0).round() as usize,
                        (rect.y + rect.height * 0.5).round() as usize,
                    ),
                    color,
                    description,
                );
            }
            assert_option_label_visible(
                &pixels,
                selected_rect,
                selected_color,
                &theme,
                "Quality 1",
            );
            assert_option_label_visible(
                &pixels,
                highlighted_rect,
                selected_color,
                &theme,
                "Quality 4",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (popup.viewport.x + popup.viewport.width + 5.0) as usize,
                    (selected_rect.y + selected_rect.height * 0.5).round() as usize,
                ),
                theme.background,
                "dropdown popup remains clipped at viewport edge",
            );

            let menu_x = (popup_item_rect.x + popup_item_rect.width - 12.0).floor() as usize;
            let menu_y = (popup_item_rect.y + popup_item_rect.height * 0.5).floor() as usize;
            assert_color(
                pixel_at(&pixels, menu_x, menu_y),
                theme.surface,
                "menu overlay row",
            );
            let menu_text_pixels = assert_menu_label_visible(&pixels, popup_item_rect, &theme);

            let center_x = wheel_rect.x + wheel_rect.width * 0.5;
            let outer_radius = wheel_rect.width.min(wheel_rect.height) * 0.5;
            let ring_x = center_x.floor() as usize;
            let ring_y = (wheel_rect.y + 5.0).floor() as usize;
            let disk_x = center_x.floor() as usize;
            let disk_y = (wheel_rect.y + outer_radius).floor() as usize;
            assert_near(
                pixel_at(&pixels, ring_x, ring_y),
                [128, 0, 255, 255],
                10,
                "actual HSV editor hue ring",
            );
            assert_near(
                pixel_at(&pixels, disk_x, disk_y),
                [204, 204, 204, 255],
                6,
                "actual HSV editor value center",
            );

            let curve_layout = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(curve)
                .unwrap();
            assert_eq!(curve_layout.dpi_scale, dpi_scale);
            assert_eq!(
                app.world()
                    .get::<bbui_layout::UiLayoutResult>(editor)
                    .unwrap()
                    .dpi_scale,
                dpi_scale
            );
            assert_eq!(wheel_rect.width, 136.0);
            assert_eq!(wheel_rect.height, 136.0);

            let plot = curve_plot(curve_rect, &theme);
            let curve_z = curve_layout.z;
            let (curve_line_count, bezier_count) = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .filter(|item| item.z == curve_z.saturating_add(3))
                .fold((0, 0), |(lines, beziers), item| match &item.primitive {
                    Primitive::Line { from, to, .. }
                        if [from, to].iter().all(|point| {
                            point[0] >= plot.x - 1.0
                                && point[0] <= plot.x + plot.width + 1.0
                                && point[1] >= plot.y - 1.0
                                && point[1] <= plot.y + plot.height + 1.0
                        }) =>
                    {
                        (lines + 1, beziers)
                    }
                    Primitive::Bezier { .. } => (lines, beziers + 1),
                    _ => (lines, beziers),
                });
            match interpolation {
                Interpolation::Constant => {
                    assert_eq!(curve_line_count, 4);
                    assert_eq!(bezier_count, 0);
                }
                Interpolation::Linear => {
                    assert_eq!(curve_line_count, 2);
                    assert_eq!(bezier_count, 0);
                }
                Interpolation::Smooth => {
                    assert_eq!(curve_line_count, 0);
                    assert_eq!(bezier_count, 2);
                }
            }

            for sample_x in [0.25, 0.75] {
                let sample =
                    curve_screen_position(plot, [sample_x, curve_model.evaluate(sample_x)]);
                assert_color_near_position(
                    &pixels,
                    sample,
                    theme.accent,
                    34,
                    3,
                    "actual curve path",
                );
            }
            let selected_point = curve_model
                .points
                .iter()
                .find(|point| Some(point.id) == curve_model.selected)
                .unwrap();
            assert_color_near_position(
                &pixels,
                curve_screen_position(plot, selected_point.position),
                theme.accent,
                20,
                2,
                "selected curve point",
            );
            if interpolation == Interpolation::Smooth {
                for handle in [selected_point.left_handle, selected_point.right_handle] {
                    assert_color_near_position(
                        &pixels,
                        curve_screen_position(
                            plot,
                            [
                                selected_point.position[0] + handle[0],
                                selected_point.position[1] + handle[1],
                            ],
                        ),
                        theme.text_muted,
                        28,
                        2,
                        "smooth curve handle",
                    );
                }
            }

            assert_eq!(ramp_model.interpolation, Interpolation::Smooth);
            let ramp_track = ramp_track(ramp_rect);
            let selected_stop = ramp_model.selected_stop().unwrap();
            assert_eq!(selected_stop.position, 0.5);
            let stop_marker_y = ramp_track.y + ramp_track.height + 10.0;
            for stop in &ramp_model.stops {
                let marker_color = if stop.id == selected_stop.id {
                    theme.accent
                } else {
                    stop.color
                };
                assert_color(
                    pixel_at(
                        &pixels,
                        (ramp_track.x + stop.position * ramp_track.width).round() as usize,
                        stop_marker_y.round() as usize,
                    ),
                    marker_color,
                    "color ramp stop marker",
                );
            }
            for segment in [8_usize, 24, 39] {
                let sample = (segment as f32 + 0.5) / 48.0;
                assert_color(
                    pixel_at(
                        &pixels,
                        (ramp_track.x + ramp_track.width * sample).floor() as usize,
                        (ramp_track.y + ramp_track.height * 0.5).floor() as usize,
                    ),
                    ramp_model.sample(sample),
                    "color ramp interpolation sample",
                );
            }

            let (_, selected_list_rect, selected_list_item) =
                collection_item(app.world(), list_widget, 2);
            let (_, unselected_list_rect, unselected_list_item) =
                collection_item(app.world(), list_widget, 1);
            assert_eq!(selected_list_item.kind, CollectionItemKind::List);
            assert!(selected_list_item.selected);
            assert!(!unselected_list_item.selected);
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_list_rect.x + selected_list_rect.width - 12.0).round() as usize,
                    (selected_list_rect.y + selected_list_rect.height * 0.5).round() as usize,
                ),
                blend(theme.surface, theme.accent, 0.28),
                "selected virtualized list row",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (unselected_list_rect.x + unselected_list_rect.width - 12.0).round() as usize,
                    (unselected_list_rect.y + unselected_list_rect.height * 0.5).round() as usize,
                ),
                theme.surface,
                "unselected virtualized list row",
            );

            let (_, selected_tree_rect, selected_tree_item) =
                collection_item(app.world(), tree_widget, 11);
            assert_eq!(selected_tree_item.kind, CollectionItemKind::Tree);
            assert!(selected_tree_item.selected);
            assert_eq!(selected_tree_item.depth, 1);
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_tree_rect.x + selected_tree_rect.width - 12.0).round() as usize,
                    (selected_tree_rect.y + selected_tree_rect.height * 0.5).round() as usize,
                ),
                blend(theme.surface, theme.accent, 0.28),
                "selected tree child row",
            );

            let (_, warm_tile_rect, warm_tile) =
                collection_item(app.world(), preview_grid_widget, 20);
            let (selected_tile_entity, selected_tile_rect, selected_tile) =
                collection_item(app.world(), preview_grid_widget, 21);
            assert_eq!(warm_tile.kind, CollectionItemKind::PreviewGrid);
            assert!(!warm_tile.selected);
            assert!(selected_tile.selected);
            let collection_spacing = theme.spacing.collection;
            let preview_padding = collection_spacing.preview_padding;
            let swatch_padding = collection_spacing.padding;
            let warm_swatch = Rect {
                x: warm_tile_rect.x + preview_padding + swatch_padding,
                y: warm_tile_rect.y + preview_padding + swatch_padding,
                width: warm_tile_rect.width - 2.0 * (preview_padding + swatch_padding),
                height: warm_tile_rect.height
                    - collection_spacing.preview_caption_height
                    - 2.0 * (preview_padding + swatch_padding),
            };
            assert_color(
                pixel_at(
                    &pixels,
                    (warm_swatch.x + warm_swatch.width * 0.5).round() as usize,
                    (warm_swatch.y + warm_swatch.height * 0.5).round() as usize,
                ),
                Rgba(warm_tile.tint[0], warm_tile.tint[1], warm_tile.tint[2], 1.0),
                "preview grid swatch",
            );
            let selected_overlay = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .any(|item| {
                    item.z
                        == app
                            .world()
                            .get::<bbui_layout::UiLayoutResult>(selected_tile_entity)
                            .unwrap()
                            .z
                            .saturating_add(3)
                        && matches!(
                            &item.primitive,
                            Primitive::RoundedRect { color, .. }
                                if *color == Rgba(theme.accent.0, theme.accent.1, theme.accent.2, 0.16)
                        )
                });
            assert!(
                selected_overlay,
                "selected preview tile should paint an accent overlay"
            );
            assert!(
                selected_tile_rect.x < TARGET_WIDTH as f32
                    && selected_tile_rect.y < TARGET_HEIGHT as f32
            );

            let table_model = app.world().get::<DataTable>(table_root).unwrap();
            let table_layout = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(table_root)
                .unwrap();
            assert_eq!(table_layout.rect, table_viewport);
            assert_eq!(table_layout.dpi_scale, dpi_scale);
            assert_eq!(table_model.sort, Some((2, true)));
            assert_eq!(table_model.scroll_x, 170.0);
            let table_children = app.world().get::<UiChildren>(table_root).unwrap_or_else(|| {
                let table_layout = app
                    .world()
                    .get::<bbui_layout::UiLayoutResult>(table_root)
                    .unwrap();
                let generated = app
                    .world()
                    .iter_entities()
                    .filter(|entity| {
                        entity.get::<DataTableHeaderControl>().is_some()
                            || entity.get::<DataTableCellControl>().is_some()
                    })
                    .map(|entity| entity.id())
                    .collect::<Vec<_>>();
                panic!(
                    "DataTable view did not mount children: layout={table_layout:?}, generated controls={generated:?}"
                );
            });
            let headers = table_children
                .0
                .iter()
                .filter_map(|entity| {
                    app.world()
                        .get::<DataTableHeaderControl>(*entity)
                        .cloned()
                        .map(|header| (*entity, header))
                })
                .collect::<Vec<_>>();
            let cells = table_children
                .0
                .iter()
                .filter_map(|entity| {
                    app.world()
                        .get::<DataTableCellControl>(*entity)
                        .cloned()
                        .map(|cell| (*entity, cell))
                })
                .collect::<Vec<_>>();
            let visible_columns = headers
                .iter()
                .map(|(_, header)| header.column)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(visible_columns, [1, 2, 3, 4, 5].into());
            let visible_rows = cells
                .iter()
                .map(|(_, cell)| cell.row_id)
                .collect::<std::collections::BTreeSet<_>>();
            assert!(visible_rows.len() < table_model.rows.len());
            assert!(
                visible_rows.contains(&40),
                "selected row 40 should be visible; scroll={}, selected={:?}, visible={visible_rows:?}",
                table_model.scroll_rows,
                table_model.selection.active,
            );
            assert!(!visible_rows.contains(&table_model.rows[0].id));
            let (sorted_header_entity, sorted_header_rect) =
                table_header(app.world(), table_root, 2);
            let sorted_header = app
                .world()
                .get::<DataTableHeaderControl>(sorted_header_entity)
                .unwrap();
            assert_eq!(sorted_header.sort_ascending, Some(true));
            let (active_header_entity, active_header_rect) =
                table_header(app.world(), table_root, table_model.active_column);
            let active_header = app
                .world()
                .get::<DataTableHeaderControl>(active_header_entity)
                .unwrap();
            assert!(active_header.active);
            assert!(headers.len() < table_model.columns.len());
            assert!(
                app.world()
                    .resource::<DisplayList>()
                    .items()
                    .iter()
                    .any(|item| {
                        matches!(
                            &item.primitive,
                            Primitive::TextWrapped { text, .. } if text == "Status  ↑"
                        )
                    })
            );
            // The open File popup covers this header and should win the z-order.
            // Verify the header pixels in a second capture after closing the popup.
            app.world_mut()
                .get_mut::<MenuControl>(menu)
                .unwrap()
                .session
                .close();
            for _ in 0..3 {
                app.update();
                thread::sleep(Duration::from_millis(5));
            }
            let (table_pixels, _) = capture_frame(&mut app, &target, frame + 100);
            assert_table_header_text_visible(&table_pixels, sorted_header_rect, &theme);
            let active_header_bg = blend(theme.surface, theme.selection, 0.34);
            assert_color(
                pixel_at(
                    &pixels,
                    (active_header_rect.x + active_header_rect.width - 14.0).round() as usize,
                    (active_header_rect.y + active_header_rect.height * 0.5).round() as usize,
                ),
                active_header_bg,
                "active virtualized table header",
            );
            let resize_divider = app
                .world()
                .resource::<DisplayList>()
                .items()
                .iter()
                .any(|item| {
                    matches!(
                        &item.primitive,
                        Primitive::Line { from, to, color, .. }
                            if (from[0] - (active_header_rect.x + active_header_rect.width - 1.0)).abs() < 0.1
                                && (to[0] - from[0]).abs() < 0.1
                                && *color == theme.focus
                    )
                });
            assert!(
                resize_divider,
                "active table column should emit its focus-colored resize divider"
            );

            let (selected_cell_entity, selected_cell_rect, selected_cell) =
                table_cell(app.world(), table_root, 40, 3);
            assert!(selected_cell.selected && selected_cell.active);
            assert_eq!(
                app.world()
                    .get::<bbui_style::UiComputedStyle>(selected_cell_entity)
                    .unwrap()
                    .color(bbui_style::UiStyleProperty::Background),
                Some(theme.selection)
            );
            let selected_cell_layout = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(selected_cell_entity)
                .unwrap();
            let cell_clip = selected_cell_layout.clip.unwrap();
            assert!(cell_clip.x >= table_viewport.x && cell_clip.y >= table_viewport.y);
            assert!(cell_clip.x + cell_clip.width <= table_viewport.x + table_viewport.width);
            let selected_cell_color = theme.selection;
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_cell_rect.x + selected_cell_rect.width - 14.0).round() as usize,
                    (selected_cell_rect.y + selected_cell_rect.height * 0.5).round() as usize,
                ),
                selected_cell_color,
                "selected virtualized table cell",
            );
            assert_color(
                pixel_at(
                    &pixels,
                    (selected_cell_rect.x + 1.0).round() as usize,
                    (selected_cell_rect.y + selected_cell_rect.height * 0.5).round() as usize,
                ),
                theme.focus,
                "active table cell focus marker",
            );
            let unselected_row_id = visible_rows
                .iter()
                .copied()
                .find(|row_id| *row_id != 40)
                .expect("a second table row should be visible for the unselected cell check");
            let (_, unselected_cell_rect, unselected_cell) =
                table_cell(app.world(), table_root, unselected_row_id, 4);
            assert!(!unselected_cell.selected);
            assert_color(
                pixel_at(
                    &pixels,
                    (unselected_cell_rect.x + unselected_cell_rect.width - 14.0).round() as usize,
                    (unselected_cell_rect.y + unselected_cell_rect.height * 0.5).round() as usize,
                ),
                theme.surface,
                "unselected virtualized table cell",
            );
            let (partial_header_entity, partial_header_rect) =
                table_header(app.world(), table_root, 1);
            let partial_header = app
                .world()
                .get::<DataTableHeaderControl>(partial_header_entity)
                .unwrap();
            assert!(partial_header_rect.x < table_viewport.x);
            let partial_header_layout = app
                .world()
                .get::<bbui_layout::UiLayoutResult>(partial_header_entity)
                .unwrap();
            assert!(partial_header_layout.clip.unwrap().x >= table_viewport.x);
            assert_eq!(partial_header.column, 1);

            println!(
                "widget scene {kind:?} dpi={dpi_scale} {interpolation:?}: menu={:?}, HSV ring={:?}, curve/ramp, selected collections and DataTable headers/cells visible, readback round-trip={:?} (label pixels={menu_text_pixels}, virtual rows={})",
                pixel_at(&pixels, menu_x, menu_y),
                pixel_at(&pixels, ring_x, ring_y),
                readback_latency,
                visible_rows.len(),
            );
        }
    }
}

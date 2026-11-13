use super::*;

#[test]
pub(super) fn tooltip_painter_uses_typed_bounds_padding_margin_and_anchor_gap() {
    let mut world = World::new();
    let window = world.spawn_empty().id();
    world.spawn(UiLayoutRoot::new(
        window,
        Rect::new(0.0, 0.0, 500.0, 300.0),
        1.0,
    ));
    let trigger = world
        .spawn(UiLayoutResult {
            window,
            rect: Rect::new(100.0, 40.0, 24.0, 20.0),
            clip: None,
            z: 4,
            content_size: [24.0, 20.0],
            dpi_scale: 1.0,
        })
        .id();
    let owner = world
        .spawn(Tooltip::new("A tooltip with a long caption"))
        .id();
    world.insert_resource(WidgetTooltips::default());
    world.resource_mut::<WidgetTooltips>().0.insert(
        window,
        TooltipHover {
            trigger,
            owner,
            elapsed_seconds: 1.0,
        },
    );
    let mut theme = Theme::default();
    theme.spacing.tooltip.padding_x = 10.0;
    theme.spacing.tooltip.padding_y = 6.0;
    theme.spacing.tooltip.maximum_width = 72.0;
    theme.spacing.tooltip.viewport_margin = 8.0;
    theme.spacing.tooltip.anchor_gap = 9.0;
    let tooltip = world.get::<Tooltip>(owner).unwrap().clone();
    let mut items = Vec::new();
    paint_tooltip_overlay(&world, owner, &tooltip, window, &theme, &mut items);

    assert!(items.iter().any(|item| matches!(
        &item.primitive,
        Primitive::TextWrapped { text, max_width, origin, .. }
            if text == &tooltip.text && *max_width == 72.0 && *origin == [110.0, 75.0]
    )));
    assert!(items.iter().any(|item| matches!(
        item.primitive,
        Primitive::RoundedRect { rect, .. }
            if rect.x == 100.0 && rect.y == 69.0
                && rect.width == 92.0 && rect.height > 0.0
    )));
}

#[test]
pub(super) fn popup_widget_painters_emit_theme_dependent_shadow_primitives() {
    let mut colors_by_theme = Vec::new();
    for kind in [bbui_style::ThemeKind::Dark, bbui_style::ThemeKind::Light] {
        let theme = Theme::new(kind);
        let expected_color = theme.color(ThemeColorToken::PopupShadow);
        let mut app = App::new();
        app.add_plugins(WidgetPlugin);
        *app.world_mut().resource_mut::<Theme>() = theme.clone();

        let window = app.world_mut().spawn_empty().id();
        let viewport = Rect::new(0.0, 0.0, 500.0, 360.0);
        app.world_mut()
            .spawn(UiLayoutRoot::new(window, viewport, 1.0));

        let mut menu = MenuControl::new(
            "File",
            crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
            MenuDisplay::Dropdown,
        );
        menu.open(None);
        menu.popup.bounds = Rect::new(40.0, 60.0, 120.0, 72.0);
        menu.popup.open = true;
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: Rect::new(40.0, 24.0, 120.0, 24.0),
                clip: None,
                z: 1,
                content_size: [120.0, 24.0],
                dpi_scale: 1.0,
            },
            menu,
        ));

        let mut pie_menu = MenuControl::new(
            "Radial",
            crate::MenuTree::new(vec![crate::MenuEntry::command("one", "One")]),
            MenuDisplay::Pie,
        );
        pie_menu.open(None);
        pie_menu.popup.bounds = Rect::new(220.0, 48.0, 112.0, 112.0);
        pie_menu.popup.open = true;
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: Rect::new(260.0, 8.0, 32.0, 24.0),
                clip: None,
                z: 2,
                content_size: [32.0, 24.0],
                dpi_scale: 1.0,
            },
            pie_menu,
        ));

        let dropdown_rect = Rect::new(40.0, 160.0, 160.0, 26.0);
        let dropdown_popup_rect = Rect::new(40.0, 186.0, 160.0, 56.0);
        let mut popup = crate::PopupState::new(dropdown_rect);
        popup.open = true;
        popup.bounds = dropdown_popup_rect;
        app.world_mut().spawn((
            UiNode,
            UiLayoutResult {
                window,
                rect: dropdown_rect,
                clip: None,
                z: 3,
                content_size: [dropdown_rect.width, dropdown_rect.height],
                dpi_scale: 1.0,
            },
            Dropdown {
                open: true,
                ..Dropdown::new(["First", "Second"], Some(0))
            },
            DropdownPopupState {
                popup,
                viewport,
                first_visible: 0,
                visible_count: 2,
            },
        ));

        let trigger = app
            .world_mut()
            .spawn(UiLayoutResult {
                window,
                rect: Rect::new(320.0, 120.0, 24.0, 24.0),
                clip: None,
                z: 4,
                content_size: [24.0, 24.0],
                dpi_scale: 1.0,
            })
            .id();
        let tooltip_owner = app
            .world_mut()
            .spawn((
                UiNode,
                Tooltip {
                    visible: true,
                    ..Tooltip::new("Themed popup tooltip")
                },
                UiLayoutResult {
                    window,
                    rect: Rect::new(320.0, 120.0, 24.0, 24.0),
                    clip: None,
                    z: 5,
                    content_size: [24.0, 24.0],
                    dpi_scale: 1.0,
                },
            ))
            .id();
        app.world_mut().resource_mut::<WidgetTooltips>().0.insert(
            window,
            TooltipHover {
                trigger,
                owner: tooltip_owner,
                elapsed_seconds: 1.0,
            },
        );

        collect_widget_paint(app.world_mut());
        let items = app
            .world()
            .resource::<DisplayList>()
            .widget_items()
            .to_vec();
        let shadows: Vec<_> = items
            .iter()
            .filter_map(|item| match &item.primitive {
                Primitive::Shadow {
                    rect,
                    shadow,
                    offset,
                    blur,
                    spread,
                    ..
                } => Some((*rect, *shadow, *offset, *blur, *spread, item.clip)),
                _ => None,
            })
            .collect();
        assert_eq!(
            shadows.len(),
            4,
            "menu, pie, dropdown, and tooltip each paint a shadow: {shadows:?}"
        );
        assert!(
            shadows
                .iter()
                .all(|(_, color, ..)| *color == expected_color)
        );
        assert!(shadows.iter().all(|(_, _, offset, blur, spread, _)| {
            let layer = theme.effects.popup_shadow.shadows[0];
            *offset == [layer.x_offset, layer.y_offset]
                && *blur == layer.blur_radius
                && *spread == layer.spread_radius
        }));
        assert!(
            shadows
                .iter()
                .all(|(_, _, _, _, _, clip)| *clip == ClipRect::from(viewport))
        );
        colors_by_theme.push(expected_color);
    }
    assert_ne!(colors_by_theme[0], colors_by_theme[1]);
}

#[test]
pub(super) fn menu_transition_uses_theme_motion_and_snaps_without_delaying_hitboxes() {
    let mut app = App::new();
    app.add_plugins(WidgetPlugin);
    app.world_mut()
        .init_resource::<bbui_style::UiReducedMotion>();
    app.world_mut().resource_mut::<Theme>().motion.menu_open =
        std::time::Duration::from_millis(100);
    let window = app.world_mut().spawn_empty().id();
    let viewport = Rect::new(0.0, 0.0, 500.0, 360.0);
    app.world_mut()
        .spawn(UiLayoutRoot::new(window, viewport, 1.0));

    let mut menu = MenuControl::new(
        "Menu",
        crate::MenuTree::new(vec![crate::MenuEntry::command("open", "Open")]),
        MenuDisplay::Dropdown,
    );
    menu.width = 220.0;
    let anchor = Rect::new(20.0, 10.0, 80.0, 20.0);
    let owner = app
        .world_mut()
        .spawn((
            UiNode,
            menu,
            UiLayoutResult {
                window,
                rect: anchor,
                clip: None,
                z: 1,
                content_size: [anchor.width, anchor.height],
                dpi_scale: 1.0,
            },
        ))
        .id();
    let row = app
        .world_mut()
        .spawn((
            UiNode,
            UiParent(owner),
            MenuItemControl {
                owner,
                id: "open".to_owned(),
                label: "Open".to_owned(),
                parent_path: Vec::new(),
                index: 0,
                enabled: true,
                separator: false,
                has_submenu: false,
                shortcut: None,
            },
            UiLayoutResult {
                window,
                rect: Rect::new(0.0, 0.0, 1.0, 1.0),
                clip: None,
                z: 0,
                content_size: [1.0, 1.0],
                dpi_scale: 1.0,
            },
        ))
        .id();
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .items
        .push(row);

    sync_menu_popups(app.world_mut());
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .open(None);
    sync_menu_popups(app.world_mut());
    let stable_menu_tick =
        entity_component_changed_tick::<MenuControl>(app.world(), owner).unwrap();
    let stable_owner_hitbox_tick =
        entity_component_changed_tick::<UiHitBox>(app.world(), owner).unwrap();
    let stable_row_control_tick =
        entity_component_changed_tick::<MenuItemControl>(app.world(), row).unwrap();
    let stable_row_layout_tick =
        entity_component_changed_tick::<UiLayoutResult>(app.world(), row).unwrap();
    app.world_mut().increment_change_tick();
    sync_menu_popups(app.world_mut());
    assert_eq!(
        entity_component_changed_tick::<MenuControl>(app.world(), owner),
        Some(stable_menu_tick),
        "idle menu sync must not touch its source component"
    );
    assert_eq!(
        entity_component_changed_tick::<UiHitBox>(app.world(), owner),
        Some(stable_owner_hitbox_tick),
        "idle menu sync must not rewrite the owner hitbox"
    );
    assert_eq!(
        entity_component_changed_tick::<MenuItemControl>(app.world(), row),
        Some(stable_row_control_tick)
    );
    assert_eq!(
        entity_component_changed_tick::<UiLayoutResult>(app.world(), row),
        Some(stable_row_layout_tick)
    );
    app.world_mut()
        .get_mut::<MenuItemControl>(row)
        .unwrap()
        .enabled = false;
    sync_menu_popups(app.world_mut());
    assert!(!app.world().get::<UiEnabled>(row).unwrap().0);
    advance_menu_motion_by(app.world_mut(), 0.05);

    let popup = app.world().get::<MenuControl>(owner).unwrap().popup.bounds;
    let owner_hit_rect = app.world().get::<UiHitBox>(owner).unwrap().rect;
    let row_rect = app.world().get::<UiHitBox>(row).unwrap().rect;
    assert_eq!(
        owner_hit_rect, popup,
        "open hit testing uses final popup bounds"
    );
    assert_eq!(
        row_rect,
        app.world().get::<UiLayoutResult>(row).unwrap().rect
    );
    let state = app
        .world()
        .resource::<WidgetMenuAnimations>()
        .0
        .get(&owner)
        .copied()
        .unwrap();
    assert!(
        (state.progress - 0.75).abs() < 0.001,
        "eased progress is stored in the shared track"
    );

    collect_widget_paint(app.world_mut());
    let progress =
        menu_animation_progress(app.world(), owner, app.world().resource::<Theme>()).unwrap();
    let animated_popup = interpolate_rect(anchor, popup, progress);
    let row_paint = app
        .world()
        .resource::<DisplayList>()
        .widget_items()
        .iter()
        .find(|item| {
            matches!(
                &item.primitive,
                Primitive::RoundedRect { rect, .. }
                    if rect.x == row_rect.x
                        && rect.y == row_rect.y
                        && rect.width == row_rect.width
                        && rect.height == row_rect.height
            )
        })
        .unwrap();
    assert!(row_paint.clip.y >= animated_popup.y);
    assert!(row_paint.clip.y + row_paint.clip.height <= animated_popup.y + animated_popup.height);
    assert!(
        row_paint.clip.height < row_rect.height,
        "rows are clipped during opening"
    );

    app.world_mut().increment_change_tick();
    app.world_mut()
        .get_mut::<MenuControl>(owner)
        .unwrap()
        .close();
    sync_menu_popups(app.world_mut());
    assert_eq!(app.world().get::<UiHitBox>(owner).unwrap().rect, anchor);
    assert!(!app.world().get::<UiVisible>(row).unwrap().0);
    advance_menu_motion_by(app.world_mut(), 0.025);
    let state = app
        .world()
        .resource::<WidgetMenuAnimations>()
        .0
        .get(&owner)
        .copied()
        .unwrap();
    assert!((state.progress - 0.421875).abs() < 0.001);
    collect_widget_paint(app.world_mut());
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::Text { text, .. } if text == "Open"))
    );

    app.world_mut().increment_change_tick();
    app.world_mut()
        .resource_mut::<bbui_style::UiReducedMotion>()
        .0 = true;
    advance_menu_motion_by(app.world_mut(), 0.0);
    assert!(
        !app.world()
            .resource::<WidgetMenuAnimations>()
            .0
            .contains_key(&owner)
    );
    collect_widget_paint(app.world_mut());
    assert!(
        !app.world()
            .resource::<DisplayList>()
            .widget_items()
            .iter()
            .any(|item| matches!(&item.primitive, Primitive::Shadow { .. }))
    );
}

use super::*;

#[test]
pub(super) fn theme_switch_updates_complete_palette() {
    let mut theme = Theme::default();
    theme.switch(ThemeKind::Light);
    assert_eq!(theme.kind, ThemeKind::Light);
    assert_ne!(theme.background, Theme::new(ThemeKind::Dark).background);
    assert_eq!(theme.spacing_unit, 4.0);
    assert_eq!(theme.control_height, 24.0);
}

#[test]
pub(super) fn typed_motion_preset_drives_editor_transition_and_reduced_motion() {
    let dark_theme = Theme::new(ThemeKind::Dark);
    assert_eq!(
        dark_theme.motion.duration(ThemeMotionToken::Fast),
        Duration::from_millis(80)
    );
    assert_eq!(
        dark_theme.motion.duration(ThemeMotionToken::MenuOpen),
        Duration::from_millis(120)
    );
    assert_eq!(
        dark_theme
            .motion
            .animation(ThemeMotionToken::PanelExpand)
            .curve,
        MotionCurve::EaseInOut
    );

    let mut app = style_app();
    let mut transitions = UiAutoAnimations::default();
    transitions.set_motion(
        UiStyleProperty::Background,
        &dark_theme,
        ThemeMotionToken::PanelExpand,
    );
    let editor = app
        .world_mut()
        .spawn((UiNode, UiRole::new("curve-editor"), transitions))
        .id();
    app.update();
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    advance(&mut app, 80);
    let halfway = app
        .world()
        .get::<UiComputedStyle>(editor)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    let light_surface = Theme::new(ThemeKind::Light).surface;
    assert_eq!(
        halfway,
        dark_theme.surface.mix(light_surface, 0.5),
        "editor style follows the typed panel-duration preset"
    );
    assert!(style_is_animating(
        app.world(),
        editor,
        UiStyleProperty::Background
    ));

    app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Dark);
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(editor)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(dark_theme.surface)
    );
    assert!(!style_is_animating(
        app.world(),
        editor,
        UiStyleProperty::Background
    ));
}

#[test]
pub(super) fn both_themes_provide_finite_semantic_tokens_and_editor_spacing() {
    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        let theme = Theme::new(kind);
        for color in ThemeColorToken::ALL {
            assert!(theme.color(color).is_finite(), "{kind:?} {color:?}");
        }
        assert_eq!(theme.spacing.control.checkbox_label_gap, 6.0);
        assert_eq!(theme.spacing.control.slider_height, 32.0);
        assert_eq!(theme.spacing.control.slider_track_height, 4.0);
        assert_eq!(theme.spacing.box_container.padding, 12.0);
        assert_eq!(theme.spacing.box_container.gap, 8.0);
        assert_eq!(theme.spacing.property_row.label_width, 112.0);
        assert_eq!(theme.spacing.property_row.gap, 8.0);
        assert_eq!(theme.spacing.property_row.row_height, 24.0);
        assert_eq!(theme.spacing.menu.item_height, 24.0);
        assert_eq!(theme.spacing.list.row_height, 22.0);
        assert_eq!(theme.spacing.table.header_height, 26.0);
        assert_eq!(theme.spacing.table.row_height, 24.0);
        assert!(theme.spacing.editor.gutter_width > 0.0);
        assert!(theme.spacing.editor.grid_spacing > 0.0);
        assert_eq!(theme.spacing.node_graph.header_height, 28.0);
        assert_eq!(theme.spacing.node_graph.header_hint_width, 210.0);
        assert_eq!(theme.spacing.node_graph.body_padding, 8.0);
        assert_eq!(theme.spacing.node_graph.socket_spacing, 22.0);
        assert_eq!(theme.spacing.node_graph.socket_radius, 5.0);
        assert_eq!(theme.spacing.node_graph.socket_hit_slop, 4.0);
        assert_eq!(theme.spacing.node_graph.connection_tangent_min, 36.0);
        assert_eq!(theme.spacing.node_graph.grid_spacing, 40.0);
        assert_eq!(theme.spacing.node_graph.grid_spacing_min, 12.0);
        assert_eq!(theme.spacing.node_graph.grid_spacing_max, 160.0);
        let graph_surface = theme.color(ThemeColorToken::NodeGraphSurface);
        assert_eq!(graph_surface, theme.background.mix(theme.surface, 0.22));
        assert!(graph_surface.0 < theme.surface.0);
        assert_eq!(theme.spacing.file_browser.header_height, 48.0);
        assert_eq!(theme.spacing.file_browser.list_row_height, 22.0);
        assert_eq!(theme.spacing.file_browser.grid_cell_width, 120.0);
        assert_eq!(theme.spacing.file_browser.grid_cell_height, 78.0);
        assert_eq!(theme.spacing.file_browser.grid_gap, 0.0);
        assert_eq!(theme.spacing.collection.header_height, 28.0);
        assert_eq!(theme.spacing.collection.preview_cell_width, 112.0);
        assert!(theme.disabled_opacity > 0.0 && theme.disabled_opacity < 1.0);
    }
    assert_ne!(
        Theme::new(ThemeKind::Dark).color(ThemeColorToken::PreviewPlaceholder),
        Theme::new(ThemeKind::Light).color(ThemeColorToken::PreviewPlaceholder),
    );
    let dark = Theme::new(ThemeKind::Dark);
    let light = Theme::new(ThemeKind::Light);
    for color in ThemeColorToken::ALL {
        assert_ne!(dark.color(color), light.color(color), "{color:?}");
    }
}

#[test]
pub(super) fn every_spacing_token_is_finite_nonnegative_and_theme_rhythm_is_shared() {
    let dark = all_spacing_values(Theme::new(ThemeKind::Dark).spacing);
    let light = all_spacing_values(Theme::new(ThemeKind::Light).spacing);
    assert_eq!(dark, light, "Dark and Light share logical layout rhythm");
    assert_eq!(
        dark.len(),
        142,
        "update this check when adding a spacing token"
    );
    for (index, value) in dark.into_iter().enumerate() {
        assert!(
            value.is_finite() && value >= 0.0,
            "spacing[{index}]={value}"
        );
    }
    let motion = Theme::new(ThemeKind::Dark).motion;
    for (name, duration) in [
        ("fast", motion.fast),
        ("standard", motion.standard),
        ("slow", motion.slow),
        ("panel_expand", motion.panel_expand),
        ("menu_open", motion.menu_open),
    ] {
        assert!(!duration.is_zero(), "{name} motion duration must be usable");
    }
}

#[test]
pub(super) fn theme_defaults_resolve_selected_checked_active_focused_disabled_and_error_states() {
    let theme = Theme::default();
    let resolve = |state| {
        resolve_style(
            &theme,
            &UiStyleSheet::default(),
            None,
            None,
            state,
            None,
            &UiInlineStyle::default(),
        )
    };

    let selected = resolve(UiStyleState::SELECTED);
    assert_eq!(
        selected.color(UiStyleProperty::Background),
        Some(theme.selection)
    );
    assert_eq!(
        selected.color(UiStyleProperty::SelectionColor),
        Some(theme.selection)
    );
    let hovered_selection = resolve(UiStyleState::SELECTED.union(UiStyleState::HOVERED));
    assert_eq!(
        hovered_selection.color(UiStyleProperty::Background),
        Some(theme.selection.mix(theme.surface_hover, 0.28))
    );
    let checked = resolve(UiStyleState::CHECKED);
    assert_eq!(
        checked.color(UiStyleProperty::AccentColor),
        Some(theme.accent)
    );
    for state in [UiStyleState::FOCUSED, UiStyleState::ACTIVE] {
        assert_eq!(
            resolve(state).color(UiStyleProperty::BorderColor),
            Some(theme.focus)
        );
    }
    let disabled = resolve(UiStyleState::DISABLED);
    assert_eq!(
        disabled.color(UiStyleProperty::Background),
        Some(theme.disabled_surface)
    );
    assert_eq!(
        disabled.color(UiStyleProperty::Foreground),
        Some(theme.disabled)
    );
    assert_eq!(
        disabled.color(UiStyleProperty::BorderColor),
        Some(theme.disabled_border)
    );
    assert_eq!(
        disabled.scalar(UiStyleProperty::Opacity),
        Some(theme.disabled_opacity)
    );
    assert_eq!(
        resolve(UiStyleState::ERROR).color(UiStyleProperty::BorderColor),
        Some(theme.error)
    );
}

#[test]
pub(super) fn dynamic_state_updates_preserve_other_flags_and_remove_empty_component() {
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    assert!(set_ui_style_state(
        &mut world,
        entity,
        UiStyleState::SELECTED,
        true
    ));
    assert!(set_ui_style_state(
        &mut world,
        entity,
        UiStyleState::ACTIVE,
        true
    ));
    assert_eq!(
        world.get::<UiStyleState>(entity).copied(),
        Some(UiStyleState::SELECTED.union(UiStyleState::ACTIVE))
    );
    assert!(set_ui_style_state(
        &mut world,
        entity,
        UiStyleState::SELECTED,
        false
    ));
    assert_eq!(
        world.get::<UiStyleState>(entity).copied(),
        Some(UiStyleState::ACTIVE)
    );
    assert!(!set_ui_style_state(
        &mut world,
        entity,
        UiStyleState::SELECTED,
        false
    ));
    assert!(set_ui_style_state(
        &mut world,
        entity,
        UiStyleState::ACTIVE,
        false
    ));
    assert!(world.get::<UiStyleState>(entity).is_none());
}

#[test]
pub(super) fn semantic_roles_apply_and_refresh_container_and_widget_spacing() {
    let mut world = World::new();
    world.init_resource::<Theme>();
    world.init_resource::<UiStyleSheet>();
    let box_container = world
        .spawn((UiNode, UiRole::new("box"), UiLayoutNode::default()))
        .id();
    let property_row = world
        .spawn((UiNode, UiRole::new("property-row"), UiLayoutNode::default()))
        .id();
    let property_label = world
        .spawn((
            UiNode,
            UiRole::new("property-label"),
            UiLayoutNode::default(),
        ))
        .id();
    let menu_item = world
        .spawn((UiNode, UiRole::new("menu-item"), UiLayoutNode::default()))
        .id();
    let slider = world
        .spawn((UiNode, UiRole::new("slider"), UiLayoutNode::default()))
        .id();
    let menu_triggers = [
        "menu-trigger",
        "popover-trigger",
        "context-menu-trigger",
        "pie-menu-trigger",
    ]
    .map(|role| {
        world
            .spawn((UiNode, UiRole::new(role), UiLayoutNode::default()))
            .id()
    });
    let editor_roots = ["node-graph", "canvas"].map(|role| {
        world
            .spawn((UiNode, UiRole::new(role), UiLayoutNode::default()))
            .id()
    });
    let list_row = world
        .spawn((UiNode, UiRole::new("list-row"), UiLayoutNode::default()))
        .id();
    let table_header = world
        .spawn((UiNode, UiRole::new("table-header"), UiLayoutNode::default()))
        .id();
    let table_cell = world
        .spawn((UiNode, UiRole::new("table-cell"), UiLayoutNode::default()))
        .id();
    let gutter = world
        .spawn((
            UiNode,
            UiRole::new("editor-gutter"),
            UiLayoutNode::default(),
        ))
        .id();
    let browser_header = world
        .spawn((
            UiNode,
            UiRole::new("file-browser-header"),
            UiLayoutNode::default(),
        ))
        .id();
    let browser_grid_item = world
        .spawn((
            UiNode,
            UiRole::new("file-browser-grid-item"),
            UiLayoutNode::default(),
        ))
        .id();
    let collection_header = world
        .spawn((
            UiNode,
            UiRole::new("collection-header"),
            UiLayoutNode::default(),
        ))
        .id();
    let preview_item = world
        .spawn((
            UiNode,
            UiRole::new("preview-grid-item"),
            UiLayoutNode::default(),
        ))
        .id();
    let control_gap_roles = [
        "layout-row",
        "layout-column",
        "layout-grid",
        "layout-flex",
        "layout-inline",
        "scroll",
    ]
    .map(|role| {
        world
            .spawn((UiNode, UiRole::new(role), UiLayoutNode::default()))
            .id()
    });
    let menu_bar = world
        .spawn((UiNode, UiRole::new("menu-bar"), UiLayoutNode::default()))
        .id();

    resolve_ui_styles(&mut world);
    assert_eq!(Theme::default().spacing.control.slider_track_height, 4.0);
    for entity in control_gap_roles {
        assert_eq!(world.get::<UiLayoutNode>(entity).unwrap().gap, 8.0);
    }
    assert_eq!(world.get::<UiLayoutNode>(menu_bar).unwrap().gap, 4.0);
    assert_eq!(
        world.get::<UiLayoutNode>(slider).unwrap().size.height,
        UiLength::Px(32.0)
    );
    for entity in menu_triggers {
        let node = world.get::<UiLayoutNode>(entity).unwrap();
        assert_eq!(node.size.height, UiLength::Px(24.0));
        assert_eq!(node.padding, UiEdges::horizontal_vertical(12.0, 4.0));
    }
    for entity in editor_roots {
        let node = world.get::<UiLayoutNode>(entity).unwrap();
        assert_eq!(node.padding, UiEdges::horizontal_vertical(6.0, 4.0));
        assert_eq!(node.gap, 4.0);
    }
    assert_eq!(
        world.get::<UiLayoutNode>(box_container).unwrap().padding,
        UiEdges::all(12.0)
    );
    assert_eq!(world.get::<UiLayoutNode>(box_container).unwrap().gap, 8.0);
    assert_eq!(
        world.get::<UiLayoutNode>(property_row).unwrap().size.height,
        UiLength::Px(24.0)
    );
    assert_eq!(world.get::<UiLayoutNode>(property_row).unwrap().gap, 8.0);
    assert_eq!(
        world
            .get::<UiLayoutNode>(property_label)
            .unwrap()
            .size
            .width,
        UiLength::Px(112.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(menu_item).unwrap().size.height,
        UiLength::Px(24.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(list_row).unwrap().size.height,
        UiLength::Px(22.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(table_header).unwrap().size.height,
        UiLength::Px(26.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(table_cell).unwrap().padding,
        UiEdges::horizontal_vertical(8.0, 4.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(gutter).unwrap().size.width,
        UiLength::Px(24.0)
    );
    assert_eq!(
        world
            .get::<UiLayoutNode>(browser_header)
            .unwrap()
            .size
            .height,
        UiLength::Px(48.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(browser_grid_item).unwrap().size,
        UiSize {
            width: UiLength::Px(120.0),
            height: UiLength::Px(78.0),
        }
    );
    assert_eq!(
        world
            .get::<UiLayoutNode>(collection_header)
            .unwrap()
            .size
            .height,
        UiLength::Px(28.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(preview_item).unwrap().size,
        UiSize {
            width: UiLength::Px(112.0),
            height: UiLength::Px(104.0),
        }
    );

    world.increment_change_tick();
    {
        let mut theme = world.resource_mut::<Theme>();
        theme.spacing.menu.item_height = 32.0;
        theme.spacing.list.row_height = 28.0;
        theme.spacing.table.header_height = 30.0;
        theme.spacing.editor.gutter_width = 36.0;
        theme.spacing.file_browser.header_height = 52.0;
        theme.spacing.file_browser.grid_cell_width = 90.0;
        theme.spacing.collection.header_height = 34.0;
        theme.spacing.collection.preview_cell_height = 120.0;
        theme.spacing.box_container.padding = 16.0;
        theme.spacing.box_container.gap = 10.0;
        theme.spacing.property_row.label_width = 140.0;
        theme.spacing.property_row.gap = 12.0;
        theme.spacing.property_row.row_height = 28.0;
        theme.spacing.control.gap = 10.0;
        theme.spacing.control.slider_height = 36.0;
        theme.spacing.control.slider_track_height = 6.0;
        theme.spacing.menu.section_gap = 6.0;
        theme.spacing.menu.padding_x = 14.0;
        theme.spacing.editor.padding_x = 10.0;
    }
    resolve_ui_styles(&mut world);
    for entity in control_gap_roles {
        assert_eq!(world.get::<UiLayoutNode>(entity).unwrap().gap, 10.0);
    }
    assert_eq!(world.get::<UiLayoutNode>(menu_bar).unwrap().gap, 6.0);
    assert_eq!(
        world.get::<UiLayoutNode>(slider).unwrap().size.height,
        UiLength::Px(36.0)
    );
    for entity in menu_triggers {
        assert_eq!(
            world.get::<UiLayoutNode>(entity).unwrap().padding,
            UiEdges::horizontal_vertical(14.0, 4.0)
        );
    }
    for entity in editor_roots {
        assert_eq!(
            world.get::<UiLayoutNode>(entity).unwrap().padding,
            UiEdges::horizontal_vertical(10.0, 4.0)
        );
    }
    assert_eq!(
        world
            .resource::<Theme>()
            .spacing
            .control
            .slider_track_height,
        6.0
    );
    assert_eq!(
        world.get::<UiLayoutNode>(box_container).unwrap().padding,
        UiEdges::all(16.0)
    );
    assert_eq!(world.get::<UiLayoutNode>(box_container).unwrap().gap, 10.0);
    assert_eq!(
        world.get::<UiLayoutNode>(property_row).unwrap().size.height,
        UiLength::Px(28.0)
    );
    assert_eq!(world.get::<UiLayoutNode>(property_row).unwrap().gap, 12.0);
    assert_eq!(
        world
            .get::<UiLayoutNode>(property_label)
            .unwrap()
            .size
            .width,
        UiLength::Px(140.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(menu_item).unwrap().size.height,
        UiLength::Px(32.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(list_row).unwrap().size.height,
        UiLength::Px(28.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(table_header).unwrap().size.height,
        UiLength::Px(30.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(gutter).unwrap().size.width,
        UiLength::Px(36.0)
    );
    assert_eq!(
        world
            .get::<UiLayoutNode>(browser_header)
            .unwrap()
            .size
            .height,
        UiLength::Px(52.0)
    );
    assert_eq!(
        world
            .get::<UiLayoutNode>(browser_grid_item)
            .unwrap()
            .size
            .width,
        UiLength::Px(90.0)
    );
    assert_eq!(
        world
            .get::<UiLayoutNode>(collection_header)
            .unwrap()
            .size
            .height,
        UiLength::Px(34.0)
    );
    assert_eq!(
        world.get::<UiLayoutNode>(preview_item).unwrap().size.height,
        UiLength::Px(120.0)
    );
}

#[test]
pub(super) fn box_role_has_theme_surface_border_and_corner_defaults() {
    let mut world = World::new();
    world.init_resource::<Theme>();
    world.init_resource::<UiStyleSheet>();
    let box_node = world
        .spawn((UiNode, UiRole::new("box"), UiLayoutNode::default()))
        .id();

    resolve_ui_styles(&mut world);
    let dark = world.get::<UiComputedStyle>(box_node).unwrap();
    assert_eq!(
        dark.color(UiStyleProperty::Background),
        Some(Theme::default().surface)
    );
    assert_eq!(
        dark.color(UiStyleProperty::BorderColor),
        Some(Theme::default().border)
    );
    assert_eq!(dark.scalar(UiStyleProperty::BorderWidth), Some(1.0));
    assert_eq!(dark.scalar(UiStyleProperty::CornerRadius), Some(3.0));

    world.increment_change_tick();
    world.resource_mut::<Theme>().switch(ThemeKind::Light);
    resolve_ui_styles(&mut world);
    let light = world.get::<UiComputedStyle>(box_node).unwrap();
    let theme = world.resource::<Theme>();
    assert_eq!(
        light.color(UiStyleProperty::Background),
        Some(theme.surface)
    );
    assert_eq!(
        light.color(UiStyleProperty::BorderColor),
        Some(theme.border)
    );
    assert_eq!(light.scalar(UiStyleProperty::BorderWidth), Some(1.0));
    assert_eq!(
        light.scalar(UiStyleProperty::CornerRadius),
        Some(theme.corner_radius)
    );
}

#[test]
pub(super) fn native_box_shadow_role_tracks_theme_tokens_and_restores_caller_value() {
    let mut app = style_app();
    let base_shadow = BoxShadow::new(
        Color::srgba(0.12, 0.24, 0.36, 0.48),
        Val::Px(1.0),
        Val::Px(2.0),
        Val::Px(3.0),
        Val::Px(4.0),
    );
    let node = app
        .world_mut()
        .spawn((
            UiNode,
            bevy_ui::Node::default(),
            UiRole::new("menu-popup"),
            UiLayoutNode::default(),
            base_shadow.clone(),
        ))
        .id();

    app.update();
    let dark = Theme::default();
    let dark_shadow = dark.effects.popup_shadow.resolve_theme_color(&dark);
    let computed = app.world().get::<UiComputedStyle>(node).unwrap();
    assert_eq!(computed.box_shadow(), Some(dark_shadow.clone()));
    assert_eq!(
        app.world().get::<BoxShadow>(node),
        Some(&dark_shadow.to_native(&dark))
    );

    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    let light = Theme::new(ThemeKind::Light);
    let light_shadow = light.effects.popup_shadow.resolve_theme_color(&light);
    let computed = app.world().get::<UiComputedStyle>(node).unwrap();
    assert_eq!(computed.box_shadow(), Some(light_shadow.clone()));
    assert_ne!(dark_shadow, light_shadow);
    assert_eq!(
        app.world().get::<BoxShadow>(node),
        Some(&light_shadow.to_native(&light))
    );

    let rule_shadow = UiBoxShadow::themed(ThemeColorToken::Error, 1.0, 3.0, 0.0, 6.0);
    let mut declaration = UiStyleDeclaration::default();
    declaration.set(
        UiStyleProperty::BoxShadow,
        UiStyleValue::BoxShadow(rule_shadow.clone()),
    );
    app.world_mut()
        .resource_mut::<UiStyleSheet>()
        .push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("raised".into()),
                required_state: UiStyleState::ACTIVE,
                ..UiStyleSelector::default()
            },
            declaration,
            priority: 0,
        });
    app.world_mut()
        .entity_mut(node)
        .insert((UiClasses(vec!["raised".into()]), UiStyleState::ACTIVE));
    app.update();
    let resolved_rule = rule_shadow.resolve_theme_color(&light);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        Some(resolved_rule.clone())
    );
    assert_eq!(
        app.world().get::<BoxShadow>(node),
        Some(&resolved_rule.to_native(&light))
    );

    let mut inline_shadow = UiBoxShadow::themed(ThemeColorToken::Accent, 2.0, 7.0, 1.0, 9.0);
    inline_shadow.push(UiShadow::new(Rgba(0.1, 0.2, 0.3, 0.4), -2.0, 1.0, 0.0, 5.0));
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::BoxShadow,
        UiStyleValue::BoxShadow(inline_shadow.clone()),
    );
    app.world_mut().entity_mut(node).insert(inline);
    app.update();
    let resolved_inline = inline_shadow.resolve_theme_color(&light);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        Some(resolved_inline.clone())
    );
    assert_eq!(
        app.world().get::<BoxShadow>(node),
        Some(&resolved_inline.to_native(&light))
    );
    assert_eq!(app.world().get::<BoxShadow>(node).unwrap().0.len(), 2);

    app.world_mut().entity_mut(node).remove::<UiInlineStyle>();
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        Some(resolved_rule.clone()),
        "stylesheet value overrides role defaults and survives inline removal"
    );
    app.world_mut()
        .entity_mut(node)
        .remove::<(UiClasses, UiStyleState)>();
    app.world_mut().entity_mut(node).remove::<UiRole>();
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        None
    );
    assert_eq!(app.world().get::<BoxShadow>(node), Some(&base_shadow));
}

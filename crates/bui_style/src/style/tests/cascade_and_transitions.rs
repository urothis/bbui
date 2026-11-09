use super::*;

#[test]
pub(super) fn native_box_shadow_interpolates_and_reduced_motion_snaps() {
    let mut app = style_app();
    let mut start = UiBoxShadow::new(Rgba(0.0, 0.0, 0.0, 0.1), 0.0, 2.0, 0.0, 4.0);
    start.push(UiShadow::new(Rgba(0.1, 0.2, 0.3, 0.2), -2.0, 0.0, 0.0, 2.0));
    let mut finish = UiBoxShadow::new(Rgba(1.0, 0.5, 0.25, 0.5), 4.0, 8.0, 2.0, 12.0);
    finish.push(UiShadow::new(Rgba(0.5, 0.8, 0.9, 0.6), 2.0, 4.0, 2.0, 6.0));
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::BoxShadow,
        UiStyleValue::BoxShadow(start.clone()),
    );
    let mut transitions = UiTransitions::default();
    set_transition(&mut transitions, UiStyleProperty::BoxShadow, 100);
    let node = app
        .world_mut()
        .spawn((
            UiNode,
            bevy_ui::Node::default(),
            UiLayoutNode::default(),
            inline,
            transitions,
        ))
        .id();
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        Some(start.clone())
    );

    app.world_mut().get_mut::<UiInlineStyle>(node).unwrap().set(
        UiStyleProperty::BoxShadow,
        UiStyleValue::BoxShadow(finish.clone()),
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        Some(start.clone()),
        "a shadow transition begins at the displayed value"
    );

    advance(&mut app, 50);
    let halfway = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .box_shadow()
        .unwrap();
    assert_eq!(halfway, start.interpolate(&finish, 0.5).unwrap());
    assert_eq!(halfway.shadows.len(), 2);
    assert_eq!(halfway.shadows[0].x_offset, 2.0);
    assert_eq!(halfway.shadows[0].blur_radius, 8.0);
    assert_eq!(halfway.shadows[1].y_offset, 2.0);
    let UiShadowColor::Color(second_color) = halfway.shadows[1].color else {
        panic!("resolved shadow color should be concrete");
    };
    assert!((second_color.3 - 0.4).abs() < 0.0001);
    assert_eq!(
        app.world().get::<BoxShadow>(node),
        Some(&halfway.to_native(&Theme::default())),
        "the Bevy component tracks the interpolated computed effect"
    );

    app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
    let target = UiBoxShadow::new(Rgba(0.2, 0.4, 0.6, 0.8), 3.0, 6.0, 1.0, 10.0);
    app.world_mut().get_mut::<UiInlineStyle>(node).unwrap().set(
        UiStyleProperty::BoxShadow,
        UiStyleValue::BoxShadow(target.clone()),
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .box_shadow(),
        Some(target.clone())
    );
    assert_eq!(
        app.world().get::<BoxShadow>(node),
        Some(&target.to_native(&Theme::default()))
    );
}

#[test]
pub(super) fn cascade_uses_selector_specificity_state_then_inline_values() {
    let theme = Theme::default();
    let mut sheet = UiStyleSheet::default();
    let mut broad = UiStyleDeclaration::default();
    broad.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(Rgba(0.1, 0.2, 0.3, 1.0)),
    );
    sheet.push(UiStyleRule {
        selector: UiStyleSelector::default(),
        declaration: broad,
        priority: 0,
    });
    let mut state = UiStyleDeclaration::default();
    state.set_color_token(UiStyleProperty::Foreground, ThemeColorToken::Accent);
    sheet.push(UiStyleRule {
        selector: UiStyleSelector {
            class: Some("button".into()),
            required_state: UiStyleState::HOVERED,
            ..UiStyleSelector::default()
        },
        declaration: state,
        priority: 0,
    });
    let mut inline = UiInlineStyle::default();
    inline.set_color_token(
        UiStyleProperty::Background,
        ThemeColorToken::CurvePlotSurface,
    );
    let resolved = resolve_style(
        &theme,
        &sheet,
        None,
        Some(&UiClasses(vec!["button".into()])),
        UiStyleState::HOVERED,
        None,
        &inline,
    );
    assert_eq!(
        resolved.color(UiStyleProperty::Background),
        Some(theme.color(ThemeColorToken::CurvePlotSurface))
    );
    assert_eq!(
        resolved.color(UiStyleProperty::Foreground),
        Some(theme.color(ThemeColorToken::Accent))
    );
    let light = Theme::new(ThemeKind::Light);
    let light_resolved = resolve_style(
        &light,
        &sheet,
        None,
        Some(&UiClasses(vec!["button".into()])),
        UiStyleState::HOVERED,
        None,
        &inline,
    );
    assert_eq!(
        light_resolved.color(UiStyleProperty::Background),
        Some(light.color(ThemeColorToken::CurvePlotSurface))
    );
    assert_eq!(
        light_resolved.color(UiStyleProperty::Foreground),
        Some(light.color(ThemeColorToken::Accent))
    );
    assert_ne!(
        resolved.color(UiStyleProperty::Background),
        light_resolved.color(UiStyleProperty::Background)
    );
}

#[test]
pub(super) fn inline_literal_override_is_stable_across_theme_and_state_changes() {
    let mut sheet = UiStyleSheet::default();
    for (required_state, token) in [
        (UiStyleState::HOVERED, ThemeColorToken::ButtonHoverSurface),
        (UiStyleState::PRESSED, ThemeColorToken::PressedSurface),
        (UiStyleState::SELECTED, ThemeColorToken::SelectedSurface),
        (UiStyleState::DISABLED, ThemeColorToken::DisabledSurface),
    ] {
        let mut declaration = UiStyleDeclaration::default();
        declaration.set_color_token(UiStyleProperty::Background, token);
        sheet.push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("control".into()),
                required_state,
                ..UiStyleSelector::default()
            },
            declaration,
            priority: 0,
        });
    }
    let override_color = Rgba(0.31, 0.27, 0.83, 1.0);
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(override_color),
    );
    let classes = UiClasses(vec!["control".into()]);
    let states = [
        UiStyleState::default(),
        UiStyleState::HOVERED,
        UiStyleState::PRESSED,
        UiStyleState::SELECTED,
        UiStyleState::DISABLED,
        UiStyleState(UiStyleState::HOVERED.0 | UiStyleState::SELECTED.0),
    ];

    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        let theme = Theme::new(kind);
        for state in states {
            let resolved =
                resolve_style(&theme, &sheet, None, Some(&classes), state, None, &inline);
            assert_eq!(
                resolved.color(UiStyleProperty::Background),
                Some(override_color),
                "inline literal must dominate {kind:?} state {state:?}"
            );
            assert_eq!(
                resolved.color(UiStyleProperty::Foreground),
                Some(if state.contains(UiStyleState::DISABLED) {
                    theme.disabled
                } else {
                    theme.text
                }),
                "non-overridden foreground remains theme/state-derived"
            );
        }
    }
}

#[test]
pub(super) fn text_values_inherit_but_layout_declarations_do_not() {
    let theme = Theme::default();
    let parent = UiComputedStyle {
        properties: HashMap::from([
            (
                UiStyleProperty::Foreground,
                UiStyleValue::Color(Rgba(0.2, 0.3, 0.4, 1.0)),
            ),
            (UiStyleProperty::Gap, UiStyleValue::Scalar(19.0)),
        ]),
    };
    let child = resolve_style(
        &theme,
        &UiStyleSheet::default(),
        None,
        None,
        UiStyleState::default(),
        Some(&parent),
        &UiInlineStyle::default(),
    );
    assert_eq!(
        child.color(UiStyleProperty::Foreground),
        Some(Rgba(0.2, 0.3, 0.4, 1.0))
    );
    assert_eq!(child.scalar(UiStyleProperty::Gap), None);
}

#[test]
pub(super) fn state_changes_apply_layout_before_layout_and_revert_to_base() {
    let mut world = World::new();
    world.init_resource::<UiStyleSheet>();
    let node = world
        .spawn((
            UiNode,
            UiLayoutNode {
                gap: 2.0,
                ..UiLayoutNode::default()
            },
            UiClasses(vec!["compact".into()]),
        ))
        .id();
    let mut declaration = UiStyleDeclaration::default();
    declaration.set(UiStyleProperty::Gap, UiStyleValue::Scalar(8.0));
    world
        .resource_mut::<UiStyleSheet>()
        .rules
        .push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("compact".into()),
                ..UiStyleSelector::default()
            },
            declaration,
            priority: 0,
        });
    resolve_ui_styles(&mut world);
    assert_eq!(world.get::<UiLayoutNode>(node).unwrap().gap, 8.0);
    world.increment_change_tick();
    world.resource_mut::<UiStyleSheet>().clear();
    resolve_ui_styles(&mut world);
    assert_eq!(world.get::<UiLayoutNode>(node).unwrap().gap, 2.0);
}

#[test]
pub(super) fn plugin_resolves_before_layout_phase() {
    let mut app = App::new();
    app.add_plugins(StylePlugin);
    let node = app
        .world_mut()
        .spawn((UiNode, UiInlineStyle::default()))
        .id();
    app.update();
    assert!(app.world().get::<UiComputedStyle>(node).is_some());
}

#[test]
pub(super) fn theme_switch_transitions_from_current_to_new_theme_with_time() {
    let mut app = style_app();
    let mut transitions = UiTransitions::default();
    set_transition(&mut transitions, UiStyleProperty::Background, 1000);
    let node = app.world_mut().spawn((UiNode, transitions)).id();
    app.update();
    let start = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();

    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(start),
        "a transition begins at the currently displayed theme value"
    );
    assert!(
        app.world()
            .get::<UiStyleTransitionState>(node)
            .unwrap()
            .is_transitioning(UiStyleProperty::Background)
    );

    advance(&mut app, 500);
    let current = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    let target = Theme::new(ThemeKind::Light).surface;
    assert!((current.0 - start.mix(target, 0.5).0).abs() < 0.001);
    assert!((current.1 - start.mix(target, 0.5).1).abs() < 0.001);

    advance(&mut app, 500);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(target)
    );
    assert!(
        app.world()
            .get::<UiStyleTransitionState>(node)
            .is_none_or(|state| !state.is_transitioning(UiStyleProperty::Background))
    );
}

#[test]
pub(super) fn live_theme_switch_invalidates_only_styles_with_changed_resolved_values() {
    let mut app = style_app();
    app.init_resource::<ChangedStyleEntities>()
        .configure_sets(Update, StyleChangeCapture.after(UiSystems::Style))
        .add_systems(Update, collect_changed_styles.in_set(StyleChangeCapture));
    let normal = app.world_mut().spawn(UiNode).id();
    let dark = Theme::default();
    let frozen_style = resolve_style(
        &dark,
        &UiStyleSheet::default(),
        None,
        None,
        UiStyleState::default(),
        None,
        &UiInlineStyle::default(),
    );
    let frozen = app
        .world_mut()
        .spawn((
            UiNode,
            UiInlineStyle(frozen_style.properties.into_iter().collect()),
        ))
        .id();
    let mut token_style = UiInlineStyle::default();
    token_style.set_color_token(UiStyleProperty::Background, ThemeColorToken::CurveGridLine);
    let tokenized = app.world_mut().spawn((UiNode, token_style)).id();

    app.update();
    assert!(
        app.world()
            .resource::<ChangedStyleEntities>()
            .0
            .contains(&normal)
    );
    assert!(
        app.world()
            .resource::<ChangedStyleEntities>()
            .0
            .contains(&frozen)
    );
    assert!(
        app.world()
            .resource::<ChangedStyleEntities>()
            .0
            .contains(&tokenized)
    );
    app.update();
    assert!(
        app.world().resource::<ChangedStyleEntities>().0.is_empty(),
        "unchanged resolved styles are not rewritten on idle frames"
    );

    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    let changed = &app.world().resource::<ChangedStyleEntities>().0;
    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&normal));
    assert!(changed.contains(&tokenized));
    assert!(!changed.contains(&frozen));
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(normal)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(Theme::new(ThemeKind::Light).surface)
    );
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(frozen)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(dark.surface)
    );
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(tokenized)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(Theme::new(ThemeKind::Light).color(ThemeColorToken::CurveGridLine))
    );
}

#[test]
pub(super) fn theme_color_token_transitions_and_reduced_motion_use_current_theme_values() {
    let mut app = style_app();
    let mut inline = UiInlineStyle::default();
    inline.set_color_token(
        UiStyleProperty::Background,
        ThemeColorToken::NodeGraphSelectedNodeSurface,
    );
    let mut transitions = UiTransitions::default();
    set_transition(&mut transitions, UiStyleProperty::Background, 1000);
    let node = app.world_mut().spawn((UiNode, inline, transitions)).id();
    app.update();
    let dark = Theme::new(ThemeKind::Dark).color(ThemeColorToken::NodeGraphSelectedNodeSurface);
    let light = Theme::new(ThemeKind::Light).color(ThemeColorToken::NodeGraphSelectedNodeSurface);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(dark)
    );

    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    advance(&mut app, 500);
    let middle = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    assert!((middle.0 - dark.mix(light, 0.5).0).abs() < 0.001);
    assert!((middle.1 - dark.mix(light, 0.5).1).abs() < 0.001);

    app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Dark);
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(dark)
    );
    assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
}

#[test]
pub(super) fn reduced_motion_snaps_active_style_transitions_to_their_targets() {
    let mut app = style_app();
    let mut transitions = UiTransitions::default();
    set_transition(&mut transitions, UiStyleProperty::Background, 1000);
    let node = app.world_mut().spawn((UiNode, transitions)).id();
    app.update();
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    advance(&mut app, 250);
    let partial = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    let target = Theme::new(ThemeKind::Light).surface;
    assert_ne!(partial, target);
    assert!(
        app.world()
            .get::<UiStyleTransitionState>(node)
            .is_some_and(|state| state.is_transitioning(UiStyleProperty::Background))
    );

    app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(target)
    );
    assert!(app.world().get::<UiStyleTransitionState>(node).is_none());

    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Dark);
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(Theme::default().surface)
    );
    assert!(app.world().get::<UiStyleTransitionState>(node).is_none());
}

#[test]
pub(super) fn hover_and_disabled_state_transitions_use_per_property_specs() {
    let mut app = style_app();
    let window = app.world_mut().spawn_empty().id();
    let mut transitions = UiTransitions::default();
    set_transition(&mut transitions, UiStyleProperty::Background, 400);
    set_transition(&mut transitions, UiStyleProperty::Foreground, 400);
    let node = app.world_mut().spawn((UiNode, transitions)).id();
    app.update();
    let base_background = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    let base_foreground = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Foreground)
        .unwrap();

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .hovered = Some(node);
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(base_background)
    );
    advance(&mut app, 400);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(Theme::default().surface_hover)
    );

    app.world_mut().entity_mut(node).insert(UiEnabled(false));
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Foreground),
        Some(base_foreground)
    );
    advance(&mut app, 200);
    let disabled_foreground = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Foreground)
        .unwrap();
    assert!(
        (disabled_foreground.0 - base_foreground.mix(Theme::default().disabled, 0.5).0).abs()
            < 0.001
    );
    advance(&mut app, 200);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Foreground),
        Some(Theme::default().disabled)
    );
}

#[test]
pub(super) fn interpolated_layout_properties_apply_before_layout() {
    let mut app = style_app();
    let window = app.world_mut().spawn_empty().id();
    let mut transitions = UiTransitions::default();
    set_transition(&mut transitions, UiStyleProperty::Gap, 400);
    let mut declaration = UiStyleDeclaration::default();
    declaration.set(UiStyleProperty::Gap, UiStyleValue::Scalar(10.0));
    app.world_mut()
        .resource_mut::<UiStyleSheet>()
        .push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("animated".into()),
                required_state: UiStyleState::HOVERED,
                ..UiStyleSelector::default()
            },
            declaration,
            priority: 0,
        });
    let node = app
        .world_mut()
        .spawn((
            UiNode,
            UiClasses(vec!["animated".into()]),
            UiLayoutNode {
                gap: 2.0,
                ..UiLayoutNode::default()
            },
            transitions,
        ))
        .id();
    app.update();
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .hovered = Some(node);
    app.update();
    assert_eq!(app.world().get::<UiLayoutNode>(node).unwrap().gap, 2.0);
    advance(&mut app, 200);
    assert!((app.world().get::<UiLayoutNode>(node).unwrap().gap - 6.0).abs() < 0.001);
    assert!(
        (app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .scalar(UiStyleProperty::Gap)
            .unwrap()
            - 6.0)
            .abs()
            < 0.001
    );
    advance(&mut app, 200);
    assert_eq!(app.world().get::<UiLayoutNode>(node).unwrap().gap, 10.0);
}

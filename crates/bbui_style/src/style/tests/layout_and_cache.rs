use super::*;

#[test]
pub(super) fn scroll_hover_style_reflows_before_text_and_keeps_stationary_hover_on_hitbox() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, LayoutPlugin, StylePlugin));
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, Rect::new(0.0, 0.0, 100.0, 60.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Overlay,
                align_items: UiAlign::Start,
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let scroll = app
        .world_mut()
        .spawn((
            UiNode,
            UiScroll::new(),
            UiLayoutNode {
                mode: UiLayoutMode::Column,
                size: UiSize {
                    width: UiLength::Px(80.0),
                    height: UiLength::Px(30.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let spacer = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
        ))
        .id();
    let first = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
        ))
        .id();
    let hovered = app
        .world_mut()
        .spawn((
            UiNode,
            UiClasses(vec!["hover-reflow".into()]),
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
        ))
        .id();
    set_parent(app.world_mut(), scroll, root).unwrap();
    set_parent(app.world_mut(), spacer, scroll).unwrap();
    set_parent(app.world_mut(), first, scroll).unwrap();
    set_parent(app.world_mut(), hovered, scroll).unwrap();

    let mut declaration = UiStyleDeclaration::default();
    declaration.set(
        UiStyleProperty::Margin,
        UiStyleValue::Edges(UiEdges {
            top: 5.0,
            ..UiEdges::ZERO
        }),
    );
    app.world_mut()
        .resource_mut::<UiStyleSheet>()
        .push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("hover-reflow".into()),
                required_state: UiStyleState::HOVERED,
                ..UiStyleSelector::default()
            },
            declaration,
            priority: 0,
        });

    app.update();
    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::PointerMove {
            window,
            position: [10.0, 25.0],
            time_seconds: 0.0,
        });
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .hovered,
        Some(first)
    );

    app.world_mut()
        .resource_mut::<UiInputQueue>()
        .push(UiInputEvent::Wheel {
            window,
            position: [10.0, 25.0],
            delta: [0.0, -1.0],
            unit: UiWheelUnit::Lines,
            modifiers: Default::default(),
        });
    app.update();

    assert_eq!(
        app.world().get::<UiScroll>(scroll).unwrap().offset,
        [0.0, 30.0]
    );
    assert_eq!(
        app.world()
            .resource::<UiFocus>()
            .state(window)
            .unwrap()
            .hovered,
        Some(hovered)
    );
    assert_eq!(
        app.world().get::<UiLayoutNode>(hovered).unwrap().margin.top,
        5.0
    );
    assert_eq!(
        app.world().get::<UiLayoutResult>(hovered).unwrap().rect.y,
        15.0
    );
    assert!(
        app.world()
            .get::<bbui_core::UiHitBox>(hovered)
            .unwrap()
            .contains([10.0, 25.0])
    );
}

#[test]
pub(super) fn animated_width_updates_shared_layout_results_before_paint() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, StylePlugin, bbui_layout::LayoutPlugin))
        .init_resource::<Time>();
    let window = app.world_mut().spawn_empty().id();
    let root = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutRoot::new(window, bbui_layout::Rect::new(0.0, 0.0, 200.0, 80.0), 1.0),
            UiLayoutNode {
                mode: UiLayoutMode::Row,
                size: bbui_layout::UiSize {
                    width: UiLength::Px(200.0),
                    height: UiLength::Px(80.0),
                },
                ..UiLayoutNode::default()
            },
        ))
        .id();
    let mut inline = UiInlineStyle::default();
    inline.set(
        UiStyleProperty::Width,
        UiStyleValue::Length(UiLength::Px(40.0)),
    );
    let mut transitions = UiAutoAnimations::default();
    set_transition(&mut transitions, UiStyleProperty::Width, 1000);
    let child = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(40.0), UiLength::Px(20.0)),
            inline,
            transitions,
        ))
        .id();
    bbui_core::set_parent(app.world_mut(), child, root)
        .expect("the test child belongs to the layout root");
    app.update();
    assert_eq!(
        app.world().get::<UiLayoutResult>(child).unwrap().rect.width,
        40.0
    );

    {
        let mut inline = app.world_mut().get_mut::<UiInlineStyle>(child).unwrap();
        inline.set(
            UiStyleProperty::Width,
            UiStyleValue::Length(UiLength::Px(120.0)),
        );
    }
    app.update();
    assert_eq!(
        app.world().get::<UiLayoutResult>(child).unwrap().rect.width,
        40.0
    );
    advance(&mut app, 500);
    assert!((app.world().get::<UiLayoutResult>(child).unwrap().rect.width - 80.0).abs() < 0.001);
}

#[test]
pub(super) fn transition_reversal_starts_at_the_displayed_value_and_reduced_motion_snaps() {
    let mut app = style_app();
    let window = app.world_mut().spawn_empty().id();
    let mut transitions = UiAutoAnimations::default();
    set_transition(&mut transitions, UiStyleProperty::Background, 1000);
    let node = app.world_mut().spawn((UiNode, transitions)).id();
    app.update();
    let base = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    let hover = Theme::default().surface_hover;
    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .hovered = Some(node);
    app.update();
    advance(&mut app, 250);
    let before_reverse = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .hovered = None;
    app.update();
    let after_reverse = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    assert_eq!(after_reverse, before_reverse);
    advance(&mut app, 500);
    let halfway_back = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    assert!((halfway_back.0 - before_reverse.mix(base, 0.5).0).abs() < 0.001);

    app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(Theme::new(ThemeKind::Light).surface)
    );
    assert!(!style_is_animating(
        app.world(),
        node,
        UiStyleProperty::Background
    ));
    assert_ne!(base, hover);

    app.world_mut().resource_mut::<UiReducedMotion>().0 = false;
    {
        let mut theme = app.world_mut().resource_mut::<Theme>();
        theme.switch(ThemeKind::Dark);
        theme.motion_scale = 0.0;
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(Theme::new(ThemeKind::Dark).surface)
    );
    assert!(!style_is_animating(
        app.world(),
        node,
        UiStyleProperty::Background
    ));
}

#[test]
pub(super) fn style_cache_tracks_focus_changes_and_active_transition_ticks_same_frame() {
    let mut app = style_app();
    let window = app.world_mut().spawn_empty().id();
    let base_color = Rgba(0.1, 0.2, 0.8, 1.0);
    let focus_color = Rgba(0.9, 0.2, 0.1, 1.0);
    let mut base_declaration = UiStyleDeclaration::default();
    base_declaration.set(UiStyleProperty::Background, UiStyleValue::Color(base_color));
    let mut focus_declaration = UiStyleDeclaration::default();
    focus_declaration.set(
        UiStyleProperty::Background,
        UiStyleValue::Color(focus_color),
    );
    {
        let mut sheet = app.world_mut().resource_mut::<UiStyleSheet>();
        sheet.push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("focus-cache".into()),
                ..UiStyleSelector::default()
            },
            declaration: base_declaration,
            priority: 0,
        });
        sheet.push(UiStyleRule {
            selector: UiStyleSelector {
                class: Some("focus-cache".into()),
                required_state: UiStyleState::FOCUSED,
                ..UiStyleSelector::default()
            },
            declaration: focus_declaration,
            priority: 0,
        });
    }
    let mut transitions = UiAutoAnimations::default();
    set_transition(&mut transitions, UiStyleProperty::Background, 100);
    let node = app
        .world_mut()
        .spawn((
            UiNode,
            UiFocusable,
            UiClasses(vec!["focus-cache".into()]),
            UiLayoutNode::default(),
            transitions,
        ))
        .id();

    app.update();
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(base_color)
    );
    let initial_passes = app
        .world()
        .resource::<UiStyleResolutionCache>()
        .resolved_passes;
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes,
        initial_passes,
        "an idle frame skips both cascade passes"
    );

    app.world_mut()
        .resource_mut::<UiFocus>()
        .state_mut(window)
        .focused = Some(node);
    app.update();
    let started_color = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    assert_eq!(started_color, base_color);
    assert!(style_is_animating(
        app.world(),
        node,
        UiStyleProperty::Background
    ));
    assert_eq!(
        app.world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes,
        initial_passes + 1,
        "focus changes are resolved before widgets in the same update"
    );

    advance(&mut app, 50);
    let halfway = app
        .world()
        .get::<UiComputedStyle>(node)
        .unwrap()
        .color(UiStyleProperty::Background)
        .unwrap();
    assert_ne!(halfway, base_color);
    assert_ne!(halfway, focus_color);
    assert_eq!(
        app.world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes,
        initial_passes + 1,
        "paint-only animation advances without rerunning the cascade"
    );

    advance(&mut app, 50);
    assert_eq!(
        app.world()
            .get::<UiComputedStyle>(node)
            .unwrap()
            .color(UiStyleProperty::Background),
        Some(focus_color)
    );
    assert!(!style_is_animating(
        app.world(),
        node,
        UiStyleProperty::Background
    ));
    let settled_passes = app
        .world()
        .resource::<UiStyleResolutionCache>()
        .resolved_passes;
    app.update();
    assert_eq!(
        app.world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes,
        settled_passes,
        "completed transitions return to the idle fast path"
    );
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub(super) struct ChangeCounts {
    styles: usize,
    layouts: usize,
}

pub(super) fn count_changed_components(
    styles: Query<Ref<UiComputedStyle>>,
    layouts: Query<Ref<UiLayoutNode>>,
    mut counts: ResMut<ChangeCounts>,
) {
    counts.styles += styles.iter().filter(|style| style.is_changed()).count();
    counts.layouts += layouts.iter().filter(|layout| layout.is_changed()).count();
}

#[test]
pub(super) fn unchanged_style_and_layout_values_do_not_retrigger_change_detection() {
    let mut app = style_app();
    app.init_resource::<ChangeCounts>().add_systems(
        Update,
        count_changed_components.after(UiSystems::StyleAfterInput),
    );
    let node = app
        .world_mut()
        .spawn((
            UiNode,
            UiLayoutNode::leaf(UiLength::Px(80.0), UiLength::Px(20.0)),
            UiInlineStyle::default(),
        ))
        .id();
    app.update();
    let first = *app.world().resource::<ChangeCounts>();
    let first_style_passes = app
        .world()
        .resource::<UiStyleResolutionCache>()
        .resolved_passes;
    assert_eq!(first.styles, 1);
    assert_eq!(first_style_passes, 1);
    app.update();
    let second = *app.world().resource::<ChangeCounts>();
    assert_eq!(second.styles, first.styles);
    assert_eq!(second.layouts, first.layouts);
    assert_eq!(
        app.world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes,
        first_style_passes,
        "both style phases skip a frame with unchanged inputs"
    );

    {
        let mut inline = app.world_mut().get_mut::<UiInlineStyle>(node).unwrap();
        inline.set(UiStyleProperty::Gap, UiStyleValue::Scalar(6.0));
    }
    app.update();
    let changed = *app.world().resource::<ChangeCounts>();
    assert!(changed.styles > second.styles);
    assert!(changed.layouts > second.layouts);
    assert_eq!(
        app.world()
            .resource::<UiStyleResolutionCache>()
            .resolved_passes,
        first_style_passes + 1,
        "an inline declaration change resolves in the same update"
    );
}

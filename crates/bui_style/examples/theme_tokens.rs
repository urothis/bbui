use std::time::Duration;

use bevy_app::App;
use bevy_time::Time;
use bui_core::{CorePlugin, UiNode};
use bui_layout::UiLayoutNode;
use bui_style::{
    StylePlugin, Theme, ThemeColorToken, ThemeKind, ThemeMotionToken, UiComputedStyle,
    UiInlineStyle, UiReducedMotion, UiRole, UiStyleProperty, UiStyleState, UiTransitions,
    set_ui_style_state,
};

fn main() {
    let mut app = App::new();
    app.add_plugins((CorePlugin, StylePlugin))
        .init_resource::<Time>();

    let mut transitions = UiTransitions::default();
    transitions.set_motion(
        UiStyleProperty::Background,
        &Theme::default(),
        ThemeMotionToken::MenuOpen,
    );
    let menu_item = app
        .world_mut()
        .spawn((
            UiNode,
            UiRole::new("menu-item"),
            UiLayoutNode::default(),
            transitions,
        ))
        .id();
    let mut plot_style = UiInlineStyle::default();
    plot_style.set_color_token(
        UiStyleProperty::Background,
        ThemeColorToken::CurvePlotSurface,
    );
    let mut curve_transitions = UiTransitions::default();
    curve_transitions.set_motion(
        UiStyleProperty::Background,
        &Theme::default(),
        ThemeMotionToken::PanelExpand,
    );
    let curve_plot = app
        .world_mut()
        .spawn((
            UiNode,
            UiRole::new("curve-editor"),
            UiLayoutNode::default(),
            plot_style,
            curve_transitions,
        ))
        .id();
    let browser_header = app
        .world_mut()
        .spawn((
            UiNode,
            UiRole::new("file-browser-header"),
            UiLayoutNode::default(),
        ))
        .id();
    let popup = app
        .world_mut()
        .spawn((
            UiNode,
            bevy_ui::Node::default(),
            UiRole::new("menu-popup"),
            UiLayoutNode::default(),
        ))
        .id();

    app.update();
    report(
        &app,
        menu_item,
        curve_plot,
        browser_header,
        popup,
        "dark default",
    );

    set_ui_style_state(app.world_mut(), menu_item, UiStyleState::SELECTED, true);
    app.update();
    report(
        &app,
        menu_item,
        curve_plot,
        browser_header,
        popup,
        "selection transition start",
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(300));
    app.update();
    report(
        &app,
        menu_item,
        curve_plot,
        browser_header,
        popup,
        "selected",
    );

    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Light);
    app.update();
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(150));
    app.update();
    report(
        &app,
        menu_item,
        curve_plot,
        browser_header,
        popup,
        "halfway to light",
    );

    app.world_mut().resource_mut::<UiReducedMotion>().0 = true;
    app.world_mut()
        .resource_mut::<Theme>()
        .switch(ThemeKind::Dark);
    app.update();
    report(
        &app,
        menu_item,
        curve_plot,
        browser_header,
        popup,
        "dark with reduced motion",
    );
}

fn report(
    app: &App,
    entity: bevy_ecs::entity::Entity,
    curve_plot: bevy_ecs::entity::Entity,
    browser_header: bevy_ecs::entity::Entity,
    popup: bevy_ecs::entity::Entity,
    label: &str,
) {
    let style = app
        .world()
        .get::<UiComputedStyle>(entity)
        .expect("StylePlugin resolves every UiNode");
    let height = app
        .world()
        .get::<UiLayoutNode>(entity)
        .map(|layout| layout.size.height);
    let plot_background = app
        .world()
        .get::<UiComputedStyle>(curve_plot)
        .and_then(|style| style.color(UiStyleProperty::Background));
    let browser_header_height = app
        .world()
        .get::<UiLayoutNode>(browser_header)
        .map(|layout| layout.size.height);
    let popup_shadow = app
        .world()
        .get::<UiComputedStyle>(popup)
        .and_then(UiComputedStyle::box_shadow);
    let native_popup_shadow_layers = app
        .world()
        .get::<bevy_ui::BoxShadow>(popup)
        .map(|shadow| shadow.0.len());
    println!(
        "{label}: background={:?}, selection={:?}, height={height:?}, curve plot token={plot_background:?}, file-browser header={browser_header_height:?}, popup shadow={popup_shadow:?}, native shadow layers={native_popup_shadow_layers:?}, node-graph spacing={:?}, slider track={:?}, float socket token={:?}, theme selection token={:?}",
        style.color(UiStyleProperty::Background),
        style.color(UiStyleProperty::SelectionColor),
        app.world().resource::<Theme>().spacing.node_graph,
        app.world()
            .resource::<Theme>()
            .spacing
            .control
            .slider_track_height,
        app.world()
            .resource::<Theme>()
            .color(ThemeColorToken::NodeGraphFloatSocket),
        app.world()
            .resource::<Theme>()
            .color(ThemeColorToken::SelectedSurface)
    );
}

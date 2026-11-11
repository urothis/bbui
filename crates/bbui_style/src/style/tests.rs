use super::*;
use bbui_core::{
    CorePlugin, UiEnabled, UiFocus, UiFocusable, UiInputEvent, UiInputQueue, UiNode, UiSystems,
    UiWheelUnit, set_parent,
};
use bbui_layout::{
    LayoutPlugin, Rect, UiAlign, UiLayoutMode, UiLayoutNode, UiLayoutResult, UiLayoutRoot,
    UiLength, UiScroll, UiSize,
};
use bevy_ecs::schedule::IntoScheduleConfigs;

fn style_app() -> App {
    let mut app = App::new();
    app.add_plugins((CorePlugin, StylePlugin))
        .init_resource::<Time>();
    app
}

fn set_transition(transitions: &mut UiAutoAnimations, property: UiStyleProperty, millis: u64) {
    transitions.set(
        property,
        UiAnimationSpec::new(Duration::from_millis(millis)),
    );
}

fn style_is_animating(world: &World, entity: Entity, property: UiStyleProperty) -> bool {
    world
        .resource::<bbui_motion::MotionPlayer>()
        .is_animating(entity, style_motion_binding(property).id())
}

#[derive(Resource, Default)]
struct ChangedStyleEntities(Vec<Entity>);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StyleChangeCapture;

fn collect_changed_styles(
    query: Query<Entity, Changed<UiComputedStyle>>,
    mut changes: ResMut<ChangedStyleEntities>,
) {
    changes.0.clear();
    changes.0.extend(query.iter());
}

fn advance(app: &mut App, millis: u64) {
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_millis(millis));
    app.update();
}

fn all_spacing_values(spacing: ThemeSpacing) -> Vec<f32> {
    let control = spacing.control;
    let panel = spacing.panel;
    let menu = spacing.menu;
    let list = spacing.list;
    let table = spacing.table;
    let editor = spacing.editor;
    let curve = spacing.curve;
    let hsv = spacing.hsv;
    let color_ramp = spacing.color_ramp;
    let tooltip = spacing.tooltip;
    let box_container = spacing.box_container;
    let property_row = spacing.property_row;
    let node_graph = spacing.node_graph;
    let file_browser = spacing.file_browser;
    let collection = spacing.collection;
    vec![
        control.padding_x,
        control.padding_y,
        control.gap,
        control.checkbox_label_gap,
        control.error_outline_width,
        control.slider_height,
        control.slider_track_height,
        panel.header_height,
        panel.content_padding,
        panel.section_gap,
        menu.item_height,
        menu.padding_x,
        menu.padding_y,
        menu.section_gap,
        menu.popup_padding,
        menu.pie_inner_inset,
        menu.pie_inner_radius,
        menu.pie_outer_radius,
        menu.pie_popup_diameter,
        menu.pie_outer_item_radius,
        menu.pie_nested_item_radius,
        menu.pie_item_max_width,
        list.row_height,
        list.padding_x,
        list.padding_y,
        list.indent,
        list.section_gap,
        table.header_height,
        table.row_height,
        table.cell_padding_x,
        table.cell_padding_y,
        editor.padding_x,
        editor.padding_y,
        editor.gutter_width,
        editor.toolbar_gap,
        editor.handle_radius,
        editor.grid_spacing,
        curve.selected_status_width,
        curve.interpolation_status_width,
        hsv.wheel_center_x,
        hsv.wheel_center_y,
        hsv.wheel_outer_radius,
        hsv.wheel_inner_radius,
        hsv.preview_right_inset,
        hsv.preview_top_inset,
        hsv.preview_width,
        hsv.preview_height,
        hsv.controls_x,
        hsv.value_label_y,
        hsv.value_track_y,
        hsv.value_track_width_inset,
        hsv.value_track_height,
        hsv.alpha_label_y,
        hsv.alpha_track_y,
        hsv.alpha_track_width_inset,
        hsv.alpha_field_y,
        hsv.alpha_field_right_inset,
        hsv.alpha_field_width,
        hsv.field_cell_width,
        hsv.field_gap,
        hsv.field_first_row_y,
        hsv.field_second_row_y,
        hsv.field_hex_y,
        hsv.field_hex_width_inset,
        hsv.field_height,
        hsv.field_text_inset_x,
        hsv.field_text_inset_y,
        hsv.help_y,
        color_ramp.title_x,
        color_ramp.title_y,
        color_ramp.button_width,
        color_ramp.button_step,
        color_ramp.button_right_inset,
        color_ramp.button_top_inset,
        color_ramp.button_height,
        color_ramp.header_hit_height,
        color_ramp.button_label_x,
        color_ramp.button_label_y,
        color_ramp.track_x,
        color_ramp.track_y,
        color_ramp.track_width_inset,
        color_ramp.track_height,
        color_ramp.marker_line_extension,
        color_ramp.marker_hit_extension,
        color_ramp.track_hit_top_inset,
        color_ramp.marker_handle_radius,
        color_ramp.marker_handle_top_inset,
        color_ramp.details_x,
        color_ramp.details_y,
        color_ramp.channel_x,
        color_ramp.channel_y,
        color_ramp.channel_step,
        color_ramp.channel_width_inset,
        color_ramp.channel_height,
        color_ramp.channel_selection_hit_slop,
        color_ramp.channel_click_hit_slop,
        color_ramp.channel_label_hit_slop,
        color_ramp.channel_label_x,
        color_ramp.channel_label_y,
        color_ramp.channel_value_right_inset,
        color_ramp.editing_right_inset,
        color_ramp.editing_bottom_inset,
        tooltip.padding_x,
        tooltip.padding_y,
        tooltip.maximum_width,
        tooltip.viewport_margin,
        tooltip.anchor_gap,
        box_container.padding,
        box_container.gap,
        property_row.label_width,
        property_row.gap,
        property_row.row_height,
        node_graph.header_height,
        node_graph.header_hint_width,
        node_graph.body_padding,
        node_graph.socket_spacing,
        node_graph.socket_radius,
        node_graph.socket_hit_slop,
        node_graph.connection_tangent_min,
        node_graph.grid_spacing,
        node_graph.grid_spacing_min,
        node_graph.grid_spacing_max,
        node_graph.node_radius,
        node_graph.title_padding_x,
        node_graph.title_padding_y,
        node_graph.collapse_toggle_inset_x,
        node_graph.socket_label_inset_x,
        node_graph.socket_label_inset_y,
        file_browser.header_height,
        file_browser.list_row_height,
        file_browser.grid_cell_width,
        file_browser.grid_cell_height,
        file_browser.grid_gap,
        file_browser.padding_x,
        collection.header_height,
        collection.item_gap,
        collection.padding,
        collection.preview_cell_width,
        collection.preview_cell_height,
        collection.preview_padding,
        collection.preview_caption_height,
        collection.minimum_preview_cell_size,
    ]
}

#[path = "tests/cascade_and_transitions.rs"]
mod cascade_and_transitions;
#[path = "tests/layout_and_cache.rs"]
mod layout_and_cache;
#[path = "tests/themes_and_defaults.rs"]
mod themes_and_defaults;

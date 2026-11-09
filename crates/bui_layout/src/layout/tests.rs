use super::*;
use bui_core::{
    CorePlugin, UiFocus, UiFocusable, UiInputEvent, UiInputQueue, UiPointerButton, UiRoutedEvent,
    UiRoutedEvents, UiSystems, UiVisible, set_parent,
};

#[derive(Resource, Default)]
struct ScrollPhaseObservation {
    target: Option<Entity>,
    text_rect: Option<Rect>,
    widget_hit_box: Option<UiHitBox>,
}

fn observe_text_phase(
    layouts: Query<&UiLayoutResult>,
    mut observation: ResMut<ScrollPhaseObservation>,
) {
    if let Some(target) = observation.target
        && let Ok(layout) = layouts.get(target)
    {
        observation.text_rect = Some(layout.rect);
    }
}

fn observe_widget_phase(
    hit_boxes: Query<&UiHitBox>,
    mut observation: ResMut<ScrollPhaseObservation>,
) {
    if let Some(target) = observation.target
        && let Ok(hit_box) = hit_boxes.get(target)
    {
        observation.widget_hit_box = Some(*hit_box);
    }
}

#[path = "tests/flow_and_scroll.rs"]
mod flow_and_scroll;
use flow_and_scroll::*;
#[path = "tests/layout_modes_and_validation.rs"]
mod layout_modes_and_validation;
use layout_modes_and_validation::*;
#[path = "tests/incremental_layout.rs"]
mod incremental_layout;

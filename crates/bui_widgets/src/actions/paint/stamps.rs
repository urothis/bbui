use super::*;

pub(in crate::actions) fn widget_paint_input_stamp(world: &World) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_resource_ticks::<Theme>(world, &mut hasher);
    hash_resource_ticks::<WidgetInteractionState>(world, &mut hasher);
    hash_resource_ticks::<WidgetMenuAnimations>(world, &mut hasher);
    hash_resource_ticks::<crate::canvas::CanvasImageAssets>(world, &mut hasher);

    if let Some(focus) = world.get_resource::<UiFocus>() {
        focus.active_window.hash(&mut hasher);
        for (window, state) in &focus.windows {
            window.hash(&mut hasher);
            state.focused.hash(&mut hasher);
            state.hovered.hash(&mut hasher);
            state.captured.hash(&mut hasher);
        }
    }
    if let Some(tooltips) = world.get_resource::<WidgetTooltips>() {
        tooltips.0.len().hash(&mut hasher);
        for (window, hover) in &tooltips.0 {
            window.hash(&mut hasher);
            hover.trigger.hash(&mut hasher);
            hover.owner.hash(&mut hasher);
        }
    }

    for entity in world.iter_entities() {
        if !entity.contains::<UiNode>()
            && !entity.contains::<UiParent>()
            && !entity.contains::<bui_core::UiChildren>()
            && !entity.contains::<UiLayoutResult>()
            && !entity.contains::<UiLayoutRoot>()
        {
            continue;
        }
        entity.id().hash(&mut hasher);
        entity.archetype().components().len().hash(&mut hasher);
        for component in entity.archetype().components() {
            component.index().hash(&mut hasher);
            if let Some(ticks) = entity.get_change_ticks_by_id(*component) {
                ticks.added.get().hash(&mut hasher);
                ticks.changed.get().hash(&mut hasher);
            }
        }
        if let Some(layout) = entity.get::<UiLayoutResult>() {
            let has_camera = world
                .get_entity(layout.window)
                .is_ok_and(|window| window.contains::<bevy_camera::Camera>());
            layout.window.hash(&mut hasher);
            has_camera.hash(&mut hasher);
        }
    }
    hasher.finish()
}

pub(in crate::actions) fn hash_resource_ticks<R: Resource>(
    world: &World,
    hasher: &mut impl Hasher,
) {
    if let Some(ticks) = world.get_resource_change_ticks::<R>() {
        true.hash(hasher);
        ticks.added.get().hash(hasher);
        ticks.changed.get().hash(hasher);
    } else {
        false.hash(hasher);
    }
}

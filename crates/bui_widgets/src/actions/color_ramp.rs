use super::*;

pub(super) fn color_ramp_spacing(world: &World) -> bui_style::ThemeColorRampSpacing {
    world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.color_ramp,
        |theme| theme.spacing.color_ramp,
    )
}

pub(super) fn color_ramp_track_rect(rect: Rect, spacing: bui_style::ThemeColorRampSpacing) -> Rect {
    Rect {
        x: rect.x + spacing.track_x,
        y: rect.y + spacing.track_y,
        width: (rect.width - spacing.track_width_inset).max(1.0),
        height: spacing.track_height,
    }
}

pub(super) fn color_ramp_channel_rect(
    rect: Rect,
    index: usize,
    spacing: bui_style::ThemeColorRampSpacing,
) -> Rect {
    Rect {
        x: rect.x + spacing.channel_x,
        y: rect.y + spacing.channel_y + index as f32 * spacing.channel_step,
        width: (rect.width - spacing.channel_width_inset).max(1.0),
        height: spacing.channel_height,
    }
}

pub(super) fn normalized_x(rect: Rect, x: f32) -> f32 {
    ((x - rect.x) / rect.width.max(1.0)).clamp(0.0, 1.0)
}

pub(super) fn begin_color_ramp_pointer(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let (Some(ramp), Some(layout)) = (
        world.get::<ColorRamp>(target).cloned(),
        world.get::<UiLayoutResult>(target).copied(),
    ) else {
        return;
    };
    let rect = layout.rect;
    let spacing = color_ramp_spacing(world);
    let track = color_ramp_track_rect(rect, spacing);
    let marker_y = track.y + track.height;
    if position[1] >= marker_y && position[1] <= marker_y + spacing.marker_hit_extension {
        let nearest = ramp.stops.iter().min_by(|a, b| {
            let ax = track.x + a.position * track.width;
            let bx = track.x + b.position * track.width;
            (ax - position[0])
                .abs()
                .total_cmp(&(bx - position[0]).abs())
        });
        if let Some(stop) = nearest
            && (track.x + stop.position * track.width - position[0]).abs()
                <= spacing.marker_handle_radius * 2.0
        {
            let stop_id = stop.id;
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_stop(stop_id);
            }
            let before = world
                .get::<ColorRamp>(target)
                .cloned()
                .unwrap_or_else(|| ramp.clone());
            world
                .resource_mut::<WidgetInteractionState>()
                .color_ramp_drags
                .insert(
                    window,
                    ColorRampDrag {
                        target,
                        before,
                        kind: ColorRampDragKind::Stop(stop_id),
                    },
                );
            update_color_ramp_drag(world, window, position);
            return;
        }
    }
    let Some(index) = (0..ColorChannel::ALL.len()).find(|index| {
        let row = color_ramp_channel_rect(rect, *index, spacing);
        position[1] >= row.y - spacing.channel_selection_hit_slop
            && position[1] <= row.y + row.height + spacing.channel_selection_hit_slop
    }) else {
        return;
    };
    let channel = ColorChannel::from_index(index);
    if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
        state.active_channel = channel;
        state.editing_color = true;
    }
    let Some(stop) = ramp.selected else {
        return;
    };
    let row = color_ramp_channel_rect(rect, index, spacing);
    if position[0] < row.x - spacing.channel_label_hit_slop {
        return;
    }
    world
        .resource_mut::<WidgetInteractionState>()
        .color_ramp_drags
        .insert(
            window,
            ColorRampDrag {
                target,
                before: ramp,
                kind: ColorRampDragKind::Channel { stop, channel },
            },
        );
    update_color_ramp_drag(world, window, position);
}

pub(super) fn update_color_ramp_drag(world: &mut World, window: Entity, position: [f32; 2]) {
    let Some((target, kind)) = world
        .resource::<WidgetInteractionState>()
        .color_ramp_drags
        .get(&window)
        .map(|drag| (drag.target, drag.kind))
    else {
        return;
    };
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = color_ramp_spacing(world);
    let changed = if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
        match kind {
            ColorRampDragKind::Stop(id) => ramp.move_stop(
                id,
                normalized_x(color_ramp_track_rect(rect, spacing), position[0]),
            ),
            ColorRampDragKind::Channel { stop, channel } => {
                let row = color_ramp_channel_rect(rect, channel.index(), spacing);
                ramp.set_color_channel(stop, channel, normalized_x(row, position[0]))
            }
        }
    } else {
        false
    };
    if changed
        && let Some(mut state) = world.get_mut::<ColorRampEditorState>(target)
        && let ColorRampDragKind::Channel { channel, .. } = kind
    {
        state.active_channel = channel;
        state.editing_color = true;
    }
}

pub(super) fn finish_color_ramp_drag(
    world: &mut World,
    drag: ColorRampDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Some(mut ramp) = world.get_mut::<ColorRamp>(drag.target) {
            *ramp = drag.before;
        }
        return;
    }
    let (Ok(before), Some(ramp)) = (drag.before.to_json(), world.get::<ColorRamp>(drag.target))
    else {
        return;
    };
    if let Ok(after) = ramp.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::ColorRampCommitted {
            target: drag.target,
            before,
            after,
        });
    }
}

pub(super) fn edit_color_ramp(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut ColorRamp) -> bool,
) {
    let Some(before_ramp) = world.get::<ColorRamp>(target).cloned() else {
        return;
    };
    let Ok(before) = before_ramp.to_json() else {
        return;
    };
    let changed = world
        .get_mut::<ColorRamp>(target)
        .is_some_and(|mut ramp| edit(&mut ramp));
    if !changed {
        return;
    }
    if let Some(ramp) = world.get::<ColorRamp>(target)
        && let Ok(after) = ramp.to_json()
        && before != after
    {
        emitted.push(WidgetEvent::ColorRampCommitted {
            target,
            before,
            after,
        });
    }
}

pub(super) fn click_color_ramp(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    click_count: u32,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = color_ramp_spacing(world);
    let add_x = rect.x + rect.width - spacing.button_right_inset;
    let remove_x = add_x + spacing.button_step;
    let interpolation_x = remove_x + spacing.button_step;
    if position[1] >= rect.y && position[1] <= rect.y + spacing.header_hit_height {
        if position[0] >= add_x && position[0] < add_x + spacing.button_width {
            edit_color_ramp(world, target, emitted, |ramp| {
                let at = suggested_stop_position(ramp);
                let color = ramp.sample(at);
                ramp.add_stop(at, color).is_some()
            });
        } else if position[0] >= remove_x && position[0] < interpolation_x {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected.is_some_and(|id| ramp.remove_stop(id))
            });
        } else if position[0] >= interpolation_x {
            edit_color_ramp(world, target, emitted, |ramp| {
                let next = match ramp.interpolation {
                    Interpolation::Constant => Interpolation::Linear,
                    Interpolation::Linear => Interpolation::Smooth,
                    Interpolation::Smooth => Interpolation::Constant,
                };
                ramp.set_interpolation(next)
            });
        } else {
            if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
                state.editing_color = !state.editing_color;
            }
        }
        return;
    }
    let track = color_ramp_track_rect(rect, spacing);
    if position[1] >= track.y - spacing.track_hit_top_inset
        && position[1] <= track.y + track.height + spacing.marker_hit_extension
    {
        let at = normalized_x(track, position[0]);
        if click_count >= 2 {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.add_stop(at, ramp.sample(at)).is_some()
            });
        } else if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
            let nearest = ramp
                .stops
                .iter()
                .min_by(|a, b| (a.position - at).abs().total_cmp(&(b.position - at).abs()))
                .map(|stop| stop.id);
            if let Some(nearest) = nearest {
                ramp.select_stop(nearest);
            }
        }
        return;
    }
    for index in 0..ColorChannel::ALL.len() {
        let row = color_ramp_channel_rect(rect, index, spacing);
        if position[1] >= row.y - spacing.channel_click_hit_slop
            && position[1] <= row.y + row.height + spacing.channel_click_hit_slop
        {
            let channel = ColorChannel::from_index(index);
            if let Some(mut state) = world.get_mut::<ColorRampEditorState>(target) {
                state.active_channel = channel;
                state.editing_color = true;
            }
            if position[0] < row.x - spacing.channel_label_hit_slop {
                return;
            }
            let value = normalized_x(row, position[0]);
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected
                    .is_some_and(|id| ramp.set_color_channel(id, channel, value))
            });
            break;
        }
    }
}

pub(super) fn suggested_stop_position(ramp: &ColorRamp) -> f32 {
    let Some(selected) = ramp.selected_stop() else {
        return 0.5;
    };
    let index = ramp
        .stops
        .iter()
        .position(|stop| stop.id == selected.id)
        .unwrap_or(0);
    if let Some(next) = ramp.stops.get(index + 1) {
        (selected.position + next.position) * 0.5
    } else if index > 0 {
        (ramp.stops[index - 1].position + selected.position) * 0.5
    } else {
        0.5
    }
}

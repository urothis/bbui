use super::*;

pub(super) fn click_radio_group(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let row_height = theme.control_height.max(1.0);
    let index = ((position[1] - layout.rect.y - row_height) / row_height).floor();
    let count = world
        .get::<RadioGroup>(target)
        .map_or(0, |group| group.options.len());
    if index < 0.0 || index >= count as f32 {
        return;
    }
    select_radio_option(world, target, index as usize, emitted);
}

pub(super) fn select_radio_option(
    world: &mut World,
    target: Entity,
    index: usize,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(mut group) = world.get_mut::<RadioGroup>(target) {
        let before = group.selected;
        if group.select(index) {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Option(before),
                after: WidgetValue::Option(group.selected),
            });
        }
    }
}

pub(super) fn select_dropdown_option(
    world: &mut World,
    owner: Entity,
    index: usize,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(mut dropdown) = world.get_mut::<Dropdown>(owner) {
        let before = dropdown.selected;
        if dropdown.choose(index) {
            emitted.push(WidgetEvent::ValueChanged {
                target: owner,
                before: WidgetValue::Option(before),
                after: WidgetValue::Option(dropdown.selected),
            });
        }
    }
}

pub(super) fn dropdown_owner(world: &World, target: Entity) -> Option<Entity> {
    world
        .get::<DropdownOptionControl>(target)
        .map(|option| option.owner)
        .or_else(|| world.get::<Dropdown>(target).map(|_| target))
}

pub(super) fn update_dropdown_pointer(world: &mut World, target: Entity) {
    let Some(option) = world.get::<DropdownOptionControl>(target).cloned() else {
        return;
    };
    if !option.enabled {
        return;
    }
    if let Some(mut dropdown) = world.get_mut::<Dropdown>(option.owner)
        && dropdown.open
        && dropdown.highlighted != Some(option.index)
    {
        dropdown.highlighted = Some(option.index);
    }
}

pub(super) fn close_dropdowns_outside(world: &mut World, window: Entity, target: Entity) {
    let keep_open = dropdown_owner(world, target);
    let mut query = world.query::<(Entity, &Dropdown, &UiLayoutResult)>();
    let owners: Vec<_> = query
        .iter(world)
        .filter(|(owner, dropdown, layout)| {
            dropdown.open && layout.window == window && Some(*owner) != keep_open
        })
        .map(|(owner, _, _)| owner)
        .collect();
    for owner in owners {
        if let Some(mut dropdown) = world.get_mut::<Dropdown>(owner) {
            dropdown.close();
        }
    }
}

pub(super) fn scroll_dropdown_options(world: &mut World, owner: Entity, delta_y: f32) {
    if !delta_y.is_finite() || delta_y == 0.0 {
        return;
    }
    let Some((open, option_count)) = world
        .get::<Dropdown>(owner)
        .map(|dropdown| (dropdown.open, dropdown.options.len()))
    else {
        return;
    };
    if !open || option_count == 0 {
        return;
    }
    let count = world
        .get::<DropdownPopupState>(owner)
        .map_or(1, |state| state.visible_count.max(1));
    if let Some(mut state) = world.get_mut::<DropdownPopupState>(owner) {
        let max_first = option_count.saturating_sub(count);
        state.first_visible = if delta_y > 0.0 {
            state.first_visible.saturating_sub(3)
        } else {
            state.first_visible.saturating_add(3).min(max_first)
        };
    }
}

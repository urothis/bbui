use super::*;

pub(super) fn collection_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<ListModel<String>>(target).is_some()
        || world.get::<TreeModel<String>>(target).is_some()
        || world.get::<PreviewGrid>(target).is_some()
    {
        Some(target)
    } else {
        world
            .get::<CollectionItemControl>(target)
            .map(|item| item.owner)
    }
}

pub(super) fn data_table_owner(world: &World, target: Entity) -> Option<Entity> {
    if world.get::<DataTable>(target).is_some() {
        Some(target)
    } else {
        world
            .get::<DataTableCellControl>(target)
            .map(|cell| cell.owner)
            .or_else(|| {
                world
                    .get::<DataTableHeaderControl>(target)
                    .map(|header| header.owner)
            })
    }
}

pub(super) fn data_table_json(world: &World, owner: Entity) -> Option<String> {
    world
        .get::<DataTable>(owner)
        .and_then(|table| table.to_json().ok())
}

pub(super) fn edit_data_table(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
    edit: impl FnOnce(&mut DataTable) -> bool,
) {
    let Some(owner) = data_table_owner(world, target) else {
        return;
    };
    let Some(before) = data_table_json(world, owner) else {
        return;
    };
    if world
        .get_mut::<DataTable>(owner)
        .is_none_or(|mut table| !edit(&mut table))
    {
        return;
    }
    if let Some(after) = data_table_json(world, owner)
        && before != after
    {
        emitted.push(WidgetEvent::DataTableCommitted {
            target: owner,
            before,
            after,
        });
    }
}

pub(super) fn select_data_table_cell(
    world: &mut World,
    cell: &DataTableCellControl,
    ctrl: bool,
    shift: bool,
    click_count: u32,
    emitted: &mut Vec<WidgetEvent>,
) {
    edit_data_table(world, cell.owner, emitted, |table| {
        table.select_cell(cell.row_id, cell.column, ctrl, shift)
    });
    if click_count >= 2 {
        emitted.push(WidgetEvent::DataTableCellActivated {
            target: cell.owner,
            row_id: cell.row_id,
            column: cell.column,
        });
    }
}

pub(super) fn begin_data_table_resize(
    world: &mut World,
    window: Entity,
    target: Entity,
    position: [f32; 2],
) {
    let Some(header) = world.get::<DataTableHeaderControl>(target).cloned() else {
        return;
    };
    let Some(hit_box) = world.get::<UiHitBox>(target) else {
        return;
    };
    if position[0] < hit_box.rect.x + hit_box.rect.width - 7.0 {
        return;
    }
    let Some(before) = data_table_json(world, header.owner) else {
        return;
    };
    world
        .resource_mut::<WidgetInteractionState>()
        .data_table_resize_drags
        .insert(
            window,
            DataTableResizeDrag {
                owner: header.owner,
                source: target,
                column: header.column,
                start_x: position[0],
                start_width: header.width,
                before,
                changed: false,
            },
        );
}

pub(super) fn update_data_table_resize(world: &mut World, window: Entity, position: [f32; 2]) {
    let Some((owner, column, start_x, start_width)) = world
        .resource::<WidgetInteractionState>()
        .data_table_resize_drags
        .get(&window)
        .map(|drag| (drag.owner, drag.column, drag.start_x, drag.start_width))
    else {
        return;
    };
    let changed = world
        .get_mut::<DataTable>(owner)
        .is_some_and(|mut table| table.resize_column(column, start_width + position[0] - start_x));
    if changed {
        let mut interaction = world.resource_mut::<WidgetInteractionState>();
        if let Some(drag) = interaction.data_table_resize_drags.get_mut(&window) {
            drag.changed = true;
        }
    }
}

pub(super) fn finish_data_table_resize(
    world: &mut World,
    drag: DataTableResizeDrag,
    cancelled: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    if cancelled {
        if let Ok(before) = DataTable::from_json(&drag.before)
            && let Some(mut table) = world.get_mut::<DataTable>(drag.owner)
        {
            *table = before;
        }
        return;
    }
    if !drag.changed {
        return;
    }
    if let Some(after) = data_table_json(world, drag.owner)
        && after != drag.before
    {
        emitted.push(WidgetEvent::DataTableCommitted {
            target: drag.owner,
            before: drag.before,
            after,
        });
    }
}

pub(super) fn scroll_data_table(
    world: &mut World,
    target: Entity,
    delta: [f32; 2],
    unit: bbui_core::UiWheelUnit,
) {
    let Some(owner) = data_table_owner(world, target) else {
        return;
    };
    let table_row_height = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.table.row_height,
        |theme| theme.spacing.table.row_height,
    );
    let row_height = world
        .get::<DataTable>(owner)
        .map_or(table_row_height, |table| {
            if table.row_height == 24 {
                table_row_height
            } else {
                table.row_height.max(1) as f32
            }
        });
    let rows = |dy: f32| match unit {
        bbui_core::UiWheelUnit::Lines => dy.abs().ceil() as usize,
        bbui_core::UiWheelUnit::Pixels => (dy.abs() / row_height).ceil().max(1.0) as usize,
    };
    let Some(table) = world.get::<DataTable>(owner) else {
        return;
    };
    let mut scroll_rows = table.scroll_rows;
    let mut scroll_x = table.scroll_x;
    {
        if delta[1].is_finite() && delta[1] != 0.0 {
            let amount = rows(delta[1]);
            scroll_rows = if delta[1] > 0.0 {
                scroll_rows.saturating_sub(amount)
            } else {
                scroll_rows.saturating_add(amount)
            };
        }
        if delta[0].is_finite() {
            scroll_x = (scroll_x - delta[0]).max(0.0);
        }
    }
    set_data_table_viewport(world, owner, scroll_rows, scroll_x);
}

pub(super) fn scroll_data_table_by_rows(world: &mut World, target: Entity, delta: i32) {
    if let Some(owner) = data_table_owner(world, target) {
        let Some(table) = world.get::<DataTable>(owner) else {
            return;
        };
        set_data_table_viewport(
            world,
            owner,
            table.scroll_rows.saturating_add_signed(delta as isize),
            table.scroll_x,
        );
    }
}

/// Change only the table viewport. An exact component tick lets the view sync
/// retain controls for cells that remain visible; arbitrary ECS edits still
/// take the full reconciliation path.
pub fn set_data_table_viewport(
    world: &mut World,
    owner: Entity,
    scroll_rows: usize,
    scroll_x: f32,
) {
    world.init_resource::<WidgetDataTableItems>();
    let previous_tick = world
        .resource::<WidgetDataTableItems>()
        .viewport_ticks
        .get(&owner)
        .copied()
        .or_else(|| {
            world
                .resource::<WidgetDataTableItems>()
                .stamps
                .get(&owner)
                .map(|stamp| stamp.model_changed)
        });
    let current_tick = entity_component_changed_tick::<DataTable>(world, owner);
    let trusted = current_tick.zip(previous_tick).is_some_and(|(a, b)| a == b);
    let before = world
        .get::<DataTable>(owner)
        .map(|table| (table.scroll_rows, table.scroll_x));
    let Some(before) = before else { return };
    if before == (scroll_rows, scroll_x) {
        return;
    }
    world.increment_change_tick();
    if let Some(mut table) = world.get_mut::<DataTable>(owner) {
        table.scroll_rows = scroll_rows;
        table.scroll_x = scroll_x;
    }
    if trusted {
        if let Some(tick) = entity_component_changed_tick::<DataTable>(world, owner) {
            world
                .resource_mut::<WidgetDataTableItems>()
                .viewport_ticks
                .insert(owner, tick);
        }
    } else {
        world
            .resource_mut::<WidgetDataTableItems>()
            .viewport_ticks
            .remove(&owner);
    }
    world.increment_change_tick();
}

pub(super) fn move_data_table_active_row(
    world: &mut World,
    owner: Entity,
    delta: i32,
    extend: bool,
    emitted: &mut Vec<WidgetEvent>,
) {
    edit_data_table(world, owner, emitted, |table| {
        let before = table.selection.clone();
        for _ in 0..delta.unsigned_abs() {
            table.move_active(delta.signum(), extend);
        }
        before != table.selection
    });
}

pub(super) fn data_table_page_size(world: &World, owner: Entity) -> usize {
    let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
    let header_heights = theme.control_height + theme.spacing.table.header_height;
    let height = world
        .get::<UiLayoutResult>(owner)
        .map_or(240.0, |layout| layout.rect.height - header_heights);
    let row_height =
        world
            .get::<DataTable>(owner)
            .map_or(theme.spacing.table.row_height, |table| {
                if table.row_height == 24 {
                    theme.spacing.table.row_height
                } else {
                    table.row_height.max(1) as f32
                }
            });
    (height.max(row_height) / row_height).floor() as usize
}

pub(super) fn activate_data_table_cell(world: &World, owner: Entity) -> Option<(u64, usize)> {
    let table = world.get::<DataTable>(owner)?;
    let row_id = table.selection.active?;
    table
        .rows
        .iter()
        .any(|row| row.id == row_id)
        .then_some((row_id, table.active_column))
}

pub(super) fn handle_data_table_key(
    world: &mut World,
    owner: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if (modifiers.control || modifiers.super_key) && key == UiKeyCode::A {
        edit_data_table(world, owner, emitted, DataTable::select_all);
        return;
    }
    match key {
        UiKeyCode::ArrowUp => {
            move_data_table_active_row(world, owner, -1, modifiers.shift, emitted)
        }
        UiKeyCode::ArrowDown => {
            move_data_table_active_row(world, owner, 1, modifiers.shift, emitted)
        }
        UiKeyCode::ArrowLeft => edit_data_table(world, owner, emitted, |table| {
            let before = table.active_column;
            table.move_active_column(-1);
            before != table.active_column
        }),
        UiKeyCode::ArrowRight => edit_data_table(world, owner, emitted, |table| {
            let before = table.active_column;
            table.move_active_column(1);
            before != table.active_column
        }),
        UiKeyCode::PageUp | UiKeyCode::PageDown => {
            let amount = data_table_page_size(world, owner).max(1) as i32;
            let direction = if key == UiKeyCode::PageUp {
                -amount
            } else {
                amount
            };
            move_data_table_active_row(world, owner, direction, modifiers.shift, emitted);
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let end = key == UiKeyCode::End;
            edit_data_table(world, owner, emitted, |table| {
                let row_id = if end {
                    table.rows.last().map(|row| row.id)
                } else {
                    table.rows.first().map(|row| row.id)
                };
                row_id.is_some_and(|row_id| table.set_active_row(row_id, modifiers.shift))
            });
        }
        UiKeyCode::Enter => {
            if let Some((row_id, column)) = activate_data_table_cell(world, owner) {
                emitted.push(WidgetEvent::DataTableCellActivated {
                    target: owner,
                    row_id,
                    column,
                });
            }
        }
        _ => {}
    }
}

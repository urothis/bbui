use super::*;

pub fn process_routed_widget_events(world: &mut World) {
    let events = world
        .get_resource::<UiRoutedEvents>()
        .map(|events| events.events.clone())
        .unwrap_or_default();
    let mut suppress_checkbox_clicks = HashSet::new();
    let mut suppress_menu_clicks = HashSet::new();
    let mut suppress_color_ramp_clicks = HashSet::new();
    let mut suppress_hsv_color_clicks = HashSet::new();
    let mut suppress_collection_clicks = HashSet::new();
    let mut suppress_data_table_clicks = HashSet::new();
    let mut suppress_panel_clicks = HashSet::new();
    let mut emitted = Vec::new();

    for event in events {
        if process_node_graph_event(world, &event, &mut emitted) {
            continue;
        }
        match event {
            UiRoutedEvent::PointerEnter { window, target } => {
                set_tooltip_hover(world, window, Some(target));
                let position = world
                    .get_resource::<UiFocus>()
                    .and_then(|focus| focus.state(window))
                    .and_then(|state| state.pointer_position)
                    .unwrap_or([0.0, 0.0]);
                update_menu_pointer(world, target, window, position, false);
                update_dropdown_pointer(world, target);
            }
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position,
                dragging: false,
                ..
            } => {
                set_tooltip_hover(world, window, Some(target));
                update_menu_pointer(world, target, window, position, false);
                update_dropdown_pointer(world, target);
            }
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
            } => {
                let modifiers = world
                    .resource::<WidgetInteractionState>()
                    .modifiers
                    .get(&window)
                    .copied()
                    .unwrap_or_default();
                place_text_caret(world, target, position, modifiers.shift);
                begin_color_ramp_pointer(world, window, target, position);
                begin_hsv_color_pointer(world, window, target, position);
                begin_curve_editor_pointer(world, window, target, position, modifiers);
                begin_collection_drag(world, window, target, position);
                begin_data_table_resize(world, window, target, position);
                begin_panel_reorder(world, window, target, position);
                if let Some(item) = world.get::<MenuItemControl>(target).cloned()
                    && let Some(mut menu) = world.get_mut::<MenuControl>(item.owner)
                {
                    menu.gesture.press(Some(item.index));
                    select_menu_item(&mut menu, &item);
                }
                if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                    && menu.display != MenuDisplay::Context
                {
                    menu.press_position = Some(position);
                }
                let paint = if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
                    PaintTransaction::begin(target, &mut checkbox)
                } else {
                    None
                };
                if let Some(transaction) = paint {
                    world
                        .resource_mut::<WidgetInteractionState>()
                        .checkbox_paints
                        .insert(window, transaction);
                }
                begin_numeric_scrub(world, window, target, position);
                begin_slider_drag(world, window, target, position);
            }
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Secondary,
                position,
            } => {
                close_menus_outside(world, window, target);
                let focus_restore_key = world.get::<UiKey>(target).map(|key| key.0.clone());
                let opened = if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                    && menu.display == MenuDisplay::Context
                {
                    menu.open_at(position, focus_restore_key);
                    menu.popup.open = true;
                    true
                } else {
                    false
                };
                if opened {
                    sync_menu_popups(world);
                }
            }
            UiRoutedEvent::PointerDown {
                window,
                target,
                button: UiPointerButton::Middle,
                position,
            } if world.get::<CurveEditor>(target).is_some() => {
                begin_curve_editor_pan(world, window, target, position);
            }
            UiRoutedEvent::PointerMoved {
                window,
                target,
                position,
                delta,
                dragging,
                ..
            } if dragging => {
                update_menu_pointer(world, target, window, position, true);
                let modifiers = world
                    .resource::<WidgetInteractionState>()
                    .modifiers
                    .get(&window)
                    .copied()
                    .unwrap_or_default();
                place_text_caret(world, target, position, modifiers.shift);
                update_color_ramp_drag(world, window, position);
                update_hsv_color_drag(world, window, position);
                update_curve_editor_drag(world, window, position, delta);
                update_collection_drag(world, window, position);
                update_data_table_resize(world, window, position);
                update_panel_reorder(world, window, position);
                apply_checkbox_paint_at(world, window, position);
                let modifiers = world
                    .resource::<WidgetInteractionState>()
                    .modifiers
                    .get(&window)
                    .copied()
                    .unwrap_or_default();
                update_numeric_scrub(world, window, position, delta[0], modifiers);
                update_slider_drag(world, window, position[0]);
            }
            UiRoutedEvent::PointerUp {
                window,
                target,
                button: UiPointerButton::Primary,
                position,
                cancelled,
                ..
            } => {
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .color_ramp_drags
                    .remove(&window)
                {
                    suppress_color_ramp_clicks.insert(drag.target);
                    finish_color_ramp_drag(world, drag, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .hsv_color_drags
                    .remove(&window)
                {
                    suppress_hsv_color_clicks.insert(drag.target);
                    finish_hsv_color_drag(world, drag, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .curve_editor_drags
                    .remove(&window)
                {
                    finish_curve_editor_drag(world, drag, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .collection_drags
                    .remove(&window)
                {
                    if drag.dragging {
                        suppress_collection_clicks.insert(drag.source);
                    }
                    finish_collection_drag(world, drag, position, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .data_table_resize_drags
                    .remove(&window)
                {
                    if drag.changed {
                        suppress_data_table_clicks.insert(drag.source);
                        suppress_data_table_clicks.insert(target);
                    }
                    finish_data_table_resize(world, drag, cancelled, &mut emitted);
                }
                if let Some(gesture) = world
                    .resource_mut::<WidgetInteractionState>()
                    .panel_reorders
                    .remove(&window)
                {
                    let moved = gesture.current != gesture.before;
                    if moved {
                        suppress_panel_clicks.insert(gesture.source);
                    }
                    finish_panel_reorder(world, gesture, cancelled, &mut emitted);
                }
                if let Some(item) = world.get::<MenuItemControl>(target).cloned() {
                    if cancelled {
                        if let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
                            menu.gesture.cancel();
                        }
                    } else {
                        let released = menu_item_at_position(world, item.owner, window, position);
                        if let Some(released) = released {
                            select_menu_item_for_release(world, &released, &mut emitted);
                            suppress_menu_clicks.insert(target);
                        }
                    }
                } else if world.get::<MenuControl>(target).is_some() {
                    let (opened_by_pointer, display) = {
                        let Some(mut menu) = world.get_mut::<MenuControl>(target) else {
                            continue;
                        };
                        let opened_by_pointer = menu.opened_by_pointer;
                        let display = menu.display;
                        menu.press_position = None;
                        menu.opened_by_pointer = false;
                        (opened_by_pointer, display)
                    };
                    if opened_by_pointer {
                        if cancelled {
                            close_menu_and_restore(world, target);
                        } else {
                            let released = if display == MenuDisplay::Pie {
                                update_pie_menu_pointer(world, target, position);
                                selected_menu_item(world, target)
                            } else {
                                menu_item_at_position(world, target, window, position)
                            };
                            if let Some(released) = released {
                                select_menu_item_for_release(world, &released, &mut emitted);
                            } else {
                                close_menu_and_restore(world, target);
                            }
                        }
                        suppress_menu_clicks.insert(target);
                    }
                }
                let paint = {
                    world
                        .resource_mut::<WidgetInteractionState>()
                        .checkbox_paints
                        .remove(&window)
                };
                if let Some(paint) = paint {
                    if cancelled {
                        restore_checkboxes(world, &paint.changes);
                        emitted.push(WidgetEvent::CheckboxPaintCancelled {
                            changes: paint.changes,
                        });
                    } else {
                        suppress_checkbox_clicks.insert(target);
                        if !paint.changes.is_empty() {
                            emitted.push(WidgetEvent::CheckboxPaintCommitted {
                                changes: paint.changes,
                            });
                        }
                    }
                }
                let scrub = {
                    world
                        .resource_mut::<WidgetInteractionState>()
                        .numeric_scrubs
                        .remove(&window)
                };
                if let Some(scrub) = scrub {
                    finish_numeric_scrub(world, window, scrub, cancelled, &mut emitted);
                }
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .slider_drags
                    .remove(&window)
                {
                    finish_slider_drag(world, drag, cancelled, &mut emitted);
                }
            }
            UiRoutedEvent::PointerUp {
                window,
                button: UiPointerButton::Middle,
                cancelled,
                ..
            } => {
                if let Some(drag) = world
                    .resource_mut::<WidgetInteractionState>()
                    .curve_editor_drags
                    .remove(&window)
                {
                    finish_curve_editor_drag(world, drag, cancelled, &mut emitted);
                }
            }
            UiRoutedEvent::PointerCancel { window, .. } => {
                cancel_gestures(world, window, &mut emitted)
            }
            UiRoutedEvent::PointerLeave { window, target } => {
                clear_tooltip_hover(world, window, target);
            }
            UiRoutedEvent::Click {
                window,
                target,
                button: UiPointerButton::Primary,
                click_count,
                position,
            } => {
                if suppress_checkbox_clicks.remove(&target) {
                    continue;
                }
                if suppress_menu_clicks.remove(&target) {
                    continue;
                }
                if suppress_color_ramp_clicks.remove(&target) {
                    continue;
                }
                if suppress_hsv_color_clicks.remove(&target) {
                    continue;
                }
                if suppress_collection_clicks.remove(&target) {
                    continue;
                }
                if suppress_data_table_clicks.remove(&target) {
                    continue;
                }
                if suppress_panel_clicks.remove(&target) {
                    continue;
                }
                close_dropdowns_outside(world, window, target);
                if let Some(header) = world.get::<DataTableHeaderControl>(target).cloned() {
                    edit_data_table(world, header.owner, &mut emitted, |table| {
                        let column_changed = table.set_active_column(header.column);
                        let sorted = table.toggle_sort(header.column);
                        column_changed || sorted
                    });
                } else if let Some(cell) = world.get::<DataTableCellControl>(target).cloned() {
                    let modifiers = world
                        .resource::<WidgetInteractionState>()
                        .modifiers
                        .get(&window)
                        .copied()
                        .unwrap_or_default();
                    select_data_table_cell(
                        world,
                        &cell,
                        modifiers.control || modifiers.super_key,
                        modifiers.shift,
                        click_count,
                        &mut emitted,
                    );
                } else if let Some(item) = world.get::<MenuItemControl>(target).cloned() {
                    select_menu_item_for_release(world, &item, &mut emitted);
                } else if let Some(option) = world.get::<DropdownOptionControl>(target).cloned() {
                    select_dropdown_option(world, option.owner, option.index, &mut emitted);
                } else if let Some(option) = world.get::<RadioOptionControl>(target).cloned() {
                    select_radio_option(world, option.owner, option.index, &mut emitted);
                } else if let Some(item) = world.get::<FileBrowserItem>(target).cloned() {
                    select_file_browser_item(world, &item, click_count, window, &mut emitted);
                } else if let Some(item) = world.get::<CollectionItemControl>(target).cloned() {
                    let modifiers = world
                        .resource::<WidgetInteractionState>()
                        .modifiers
                        .get(&window)
                        .copied()
                        .unwrap_or_default();
                    click_collection_item(
                        world,
                        &item,
                        position,
                        click_count,
                        modifiers,
                        &mut emitted,
                    );
                } else if world.get::<MenuControl>(target).is_some() {
                    close_menus_outside(world, window, target);
                    toggle_menu(world, target);
                } else if world.get::<FileBrowser>(target).is_some() {
                    close_menus_outside(world, window, target);
                    click_file_browser(world, target, position, click_count, window, &mut emitted);
                } else if world.get::<ColorRamp>(target).is_some() {
                    click_color_ramp(world, target, position, click_count, &mut emitted);
                } else if world.get::<HsvColorEditor>(target).is_some() {
                    click_hsv_color_editor(world, target, position);
                } else if world.get::<RadioGroup>(target).is_some() {
                    click_radio_group(world, target, position, &mut emitted);
                } else {
                    close_menus_outside(world, window, target);
                    if world.get::<Panel>(target).is_some() {
                        if panel_header_contains(world, target, position) {
                            let modifiers = world
                                .resource::<WidgetInteractionState>()
                                .modifiers
                                .get(&window)
                                .copied()
                                .unwrap_or_default();
                            if modifiers.control {
                                toggle_exclusive_panel(world, target, &mut emitted);
                            } else {
                                activate_target(world, target, click_count, &mut emitted);
                            }
                        }
                    } else {
                        activate_target(world, target, click_count, &mut emitted);
                    }
                }
            }
            UiRoutedEvent::Key {
                window,
                target,
                key,
                pressed,
                repeat: _,
                modifiers,
            } => {
                world
                    .resource_mut::<WidgetInteractionState>()
                    .modifiers
                    .insert(window, modifiers);
                if pressed
                    && key == UiKeyCode::Escape
                    && (cancel_panel_reorder(world, window, &mut emitted)
                        || cancel_numeric_scrub(world, window, &mut emitted)
                        || cancel_slider_drag(world, window, &mut emitted))
                {
                    continue;
                }
                if pressed && !handle_menu_key(world, target, key, modifiers, &mut emitted) {
                    handle_key(world, target, key, modifiers, &mut emitted);
                }
                if pressed
                    && key == UiKeyCode::Enter
                    && world.get::<TextField>(target).is_some()
                    && world.get::<TextArea>(target).is_none()
                {
                    finish_text_edit_on_focus_departure(world, target, &mut emitted);
                }
            }
            UiRoutedEvent::Wheel { target, delta, .. }
                if dropdown_owner(world, target).is_some() =>
            {
                let owner = dropdown_owner(world, target).unwrap_or(target);
                scroll_dropdown_options(world, owner, delta[1]);
            }
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                ..
            } if data_table_owner(world, target).is_some() => {
                scroll_data_table(world, target, delta, unit);
            }
            UiRoutedEvent::Wheel {
                target,
                position,
                delta,
                ..
            } if world.get::<CurveEditor>(target).is_some() => {
                zoom_curve_editor_at(world, target, position, delta[1], &mut emitted);
            }
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                ..
            } if collection_owner(world, target).is_some() => {
                scroll_collection(world, target, delta[1], unit);
            }
            UiRoutedEvent::Wheel {
                window,
                target,
                delta,
                modifiers,
                ..
            } if modifiers.control => {
                if let Some(mut slider) = world.get_mut::<Slider>(target) {
                    let before = slider.value;
                    let steps = if delta[1] > 0.0 {
                        1
                    } else if delta[1] < 0.0 {
                        -1
                    } else {
                        0
                    };
                    slider.step_by(steps);
                    if before != slider.value {
                        emitted.push(WidgetEvent::ValueChanged {
                            target,
                            before: WidgetValue::Number(before),
                            after: WidgetValue::Number(slider.value),
                        });
                    }
                } else if let Some(mut field) = world.get_mut::<NumericField>(target) {
                    let before = field.value;
                    let steps = if delta[1] > 0.0 {
                        1
                    } else if delta[1] < 0.0 {
                        -1
                    } else {
                        0
                    };
                    field.step_by(steps);
                    let after = field.value;
                    if before != after {
                        emitted.push(WidgetEvent::ValueChanged {
                            target,
                            before: WidgetValue::Number(before),
                            after: WidgetValue::Number(after),
                        });
                    }
                }
                world
                    .resource_mut::<WidgetInteractionState>()
                    .modifiers
                    .insert(window, modifiers);
            }
            UiRoutedEvent::Wheel { target, delta, .. }
                if world.get::<FileBrowser>(target).is_some() =>
            {
                scroll_file_browser(world, target, delta[1]);
            }
            UiRoutedEvent::Wheel {
                target,
                delta,
                unit,
                ..
            } if world.get::<TextArea>(target).is_some() => {
                scroll_text_area(world, target, delta[1], unit);
            }
            UiRoutedEvent::Text { target, text, .. }
            | UiRoutedEvent::ImeCommit { target, text, .. } => {
                if let Some(owner) = collection_owner(world, target)
                    && collection_is_renaming(world, owner)
                {
                    collection_rename_text(world, owner, &text);
                } else if world.get::<HsvColorEditor>(target).is_some() {
                    hsv_color_text_input(world, target, &text);
                } else if world.get::<MenuControl>(target).is_some() {
                    if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                        && menu.session.open
                    {
                        menu.query.push_str(&text);
                    }
                } else if world.get::<FileBrowser>(target).is_some() {
                    begin_file_browser_filter_edit(world, target);
                    if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                        let mut filter = browser.filter().to_owned();
                        filter.push_str(&text);
                        browser.set_filter(filter);
                    }
                    if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                        view.scroll_rows = 0;
                    }
                } else if let Some(buffer) = world.get::<TextBuffer>(target) {
                    emitted.push(WidgetEvent::TextChanged {
                        target,
                        text: buffer.as_str().to_owned(),
                    });
                } else {
                    insert_text(world, target, &text, &mut emitted);
                }
            }
            UiRoutedEvent::ImePreedit {
                target,
                text,
                cursor_range,
                ..
            } => emitted.push(WidgetEvent::ImePreedit {
                target,
                text,
                cursor_range,
            }),
            UiRoutedEvent::FocusChanged {
                window,
                previous,
                current: _,
            } => {
                if let Some(previous) = previous {
                    handle_widget_focus_departure(world, previous, &mut emitted);
                }
                world
                    .resource_mut::<WidgetInteractionState>()
                    .modifiers
                    .entry(window)
                    .or_default();
            }
            _ => {}
        }
    }
    {
        let mut requests = world.resource_mut::<ClipboardRequestQueue>();
        for event in &emitted {
            match event {
                WidgetEvent::ClipboardWrite { text, .. } => {
                    requests.push(ClipboardRequest::Write(text.clone()))
                }
                WidgetEvent::ClipboardRead { target } => {
                    requests.push(ClipboardRequest::Read { target: *target })
                }
                _ => {}
            }
        }
    }
    world.resource_mut::<WidgetEventQueue>().0.extend(emitted);
}

pub(super) fn tooltip_host(world: &World, target: Entity) -> Option<Entity> {
    let mut current = Some(target);
    let mut visited = HashSet::new();
    while let Some(entity) = current {
        if !visited.insert(entity) {
            return None;
        }
        if world.get::<Tooltip>(entity).is_some() {
            return Some(entity);
        }
        current = world.get::<UiParent>(entity).map(|parent| parent.0);
    }
    None
}

pub(super) fn set_tooltip_hover(world: &mut World, window: Entity, target: Option<Entity>) {
    let next = target.and_then(|trigger| {
        tooltip_host(world, trigger).map(|owner| TooltipHover {
            trigger,
            owner,
            elapsed_seconds: 0.0,
        })
    });
    let previous = world
        .get_resource::<WidgetTooltips>()
        .and_then(|tooltips| tooltips.0.get(&window).copied());
    if previous.is_some_and(|previous| {
        next.is_some_and(|next| previous.trigger == next.trigger && previous.owner == next.owner)
    }) {
        return;
    }
    if let Some(previous) = previous
        && let Some(mut tooltip) = world.get_mut::<Tooltip>(previous.owner)
    {
        tooltip.visible = false;
    }
    if let Some(next) = next {
        if let Some(mut tooltip) = world.get_mut::<Tooltip>(next.owner) {
            tooltip.visible = false;
        }
        world
            .resource_mut::<WidgetTooltips>()
            .0
            .insert(window, next);
    } else {
        world.resource_mut::<WidgetTooltips>().0.remove(&window);
    }
}

pub(super) fn clear_tooltip_hover(world: &mut World, window: Entity, target: Entity) {
    let hover = world
        .get_resource::<WidgetTooltips>()
        .and_then(|tooltips| tooltips.0.get(&window).copied());
    let Some(hover) = hover else {
        return;
    };
    if hover.trigger != target && hover.owner != target {
        return;
    }
    world.resource_mut::<WidgetTooltips>().0.remove(&window);
    if let Some(mut tooltip) = world.get_mut::<Tooltip>(hover.owner) {
        tooltip.visible = false;
    }
}

/// Advance tooltip dwell timers using Bevy's frame delta.
pub fn advance_tooltips(world: &mut World) {
    let delta_seconds = world
        .get_resource::<bevy_time::Time>()
        .map_or(1.0 / 60.0, bevy_time::Time::delta_secs);
    advance_tooltips_by(world, delta_seconds);
}

pub(super) fn advance_tooltips_by(world: &mut World, delta_seconds: f32) {
    if !delta_seconds.is_finite() || delta_seconds <= 0.0 {
        return;
    }
    let hovers = {
        let mut tooltips = world.resource_mut::<WidgetTooltips>();
        for hover in tooltips.0.values_mut() {
            hover.elapsed_seconds += delta_seconds;
        }
        tooltips.0.values().copied().collect::<Vec<_>>()
    };
    for hover in hovers {
        let should_show = world
            .get::<Tooltip>(hover.owner)
            .is_some_and(|tooltip| hover.elapsed_seconds * 1000.0 >= tooltip.delay_millis as f32);
        if should_show && let Some(mut tooltip) = world.get_mut::<Tooltip>(hover.owner) {
            tooltip.visible = true;
        }
    }
}

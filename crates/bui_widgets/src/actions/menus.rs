use super::*;

pub(super) fn click_hsv_color_editor(world: &mut World, target: Entity, position: [f32; 2]) {
    let Some(rect) = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.rect)
    else {
        return;
    };
    let spacing = hsv_spacing(world);
    let value_rect = hsv_value_rect(rect, spacing);
    let alpha_rect = hsv_alpha_rect(rect, spacing);
    if value_rect.contains(position[0], position[1])
        || alpha_rect.contains(position[0], position[1])
    {
        return;
    }
    for field in HsvColorField::ALL {
        if hsv_field_rect(rect, field, spacing).contains(position[0], position[1])
            && let Some(mut state) = world.get_mut::<HsvColorEditorState>(target)
        {
            state.active_field = field;
            state.editing = false;
            state.edit_buffer.clear();
            return;
        }
    }
}

pub(super) fn hsv_color_text_input(world: &mut World, target: Entity, text: &str) {
    let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) else {
        return;
    };
    if !state.editing {
        state.edit_buffer.clear();
        state.editing = true;
    }
    state.edit_buffer.push_str(text);
}

pub(super) fn toggle_menu(world: &mut World, target: Entity) {
    let was_open = world
        .get::<MenuControl>(target)
        .is_some_and(|menu| menu.session.open);
    let focus_restore_key = menu_focus_key(world, target);
    let closed = if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
        menu.toggle(focus_restore_key);
        menu.popup.open = menu.session.open;
        was_open && !menu.session.open
    } else {
        false
    };
    if closed {
        close_menu_and_restore(world, target);
    }
}

pub(super) fn menu_focus_key(world: &World, target: Entity) -> Option<String> {
    let window = world
        .get::<UiLayoutResult>(target)
        .map(|layout| layout.window);
    window
        .and_then(|window| {
            world
                .get_resource::<UiFocus>()
                .and_then(|focus| focus.state(window))
                .and_then(|state| state.focused)
        })
        .and_then(|focused| world.get::<UiKey>(focused).map(|key| key.0.clone()))
        .or_else(|| world.get::<UiKey>(target).map(|key| key.0.clone()))
}

pub(super) fn restore_menu_focus(world: &mut World, owner: Entity, key: Option<String>) {
    let Some(window) = world
        .get::<UiLayoutResult>(owner)
        .map(|layout| layout.window)
    else {
        return;
    };
    let keyed = key.and_then(|key| {
        let mut query = world.query::<(Entity, &UiKey)>();
        query
            .iter(world)
            .find(|(entity, candidate)| {
                candidate.0 == key
                    && world.get::<UiFocusable>(*entity).is_some()
                    && is_interactable_in_window(world, *entity, window)
            })
            .map(|(entity, _)| entity)
    });
    let focus_target = keyed.or_else(|| {
        (world.get::<UiFocusable>(owner).is_some()
            && is_interactable_in_window(world, owner, window))
        .then_some(owner)
    });
    if let Some(target) = focus_target
        && let Some(mut focus) = world.get_resource_mut::<UiFocus>()
    {
        focus.state_mut(window).focused = Some(target);
        focus.set_active(window);
    }
}

pub(super) fn close_menu_and_restore(world: &mut World, owner: Entity) {
    let restore_key = if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
        let restore_key = menu.session.focus_restore_key.clone();
        menu.close();
        menu.popup.open = false;
        restore_key
    } else {
        None
    };
    restore_menu_focus(world, owner, restore_key);
}

pub(super) fn handle_widget_focus_departure(
    world: &mut World,
    previous: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    finish_text_edit_on_focus_departure(world, previous, emitted);
    let menu_owner = world
        .get::<MenuControl>(previous)
        .is_some()
        .then_some(previous)
        .or_else(|| {
            world
                .get::<MenuItemControl>(previous)
                .map(|item| item.owner)
        });
    finish_file_browser_filter_edit(world, previous, emitted);
    if let Some(owner) = menu_owner
        && world
            .get::<MenuControl>(owner)
            .is_some_and(|menu| menu.session.open)
        && let Some(mut menu) = world.get_mut::<MenuControl>(owner)
    {
        // Tab has already moved focus. Close the popup without restoring focus
        // to its trigger; Escape and command activation use the restoring path.
        menu.close();
        menu.popup.open = false;
    }
    if let Some(mut dropdown) = world.get_mut::<Dropdown>(previous) {
        dropdown.close();
    }
    finish_numeric_edit_on_focus_departure(world, previous, emitted);
    finish_hsv_edit_on_focus_departure(world, previous, emitted);
}

pub(super) fn capture_focused_text_edit_drafts(world: &mut World) {
    let focused: Vec<_> = world
        .get_resource::<UiFocus>()
        .map(|focus| {
            let mut focused: Vec<_> = focus
                .windows
                .iter()
                .filter_map(|(window, state)| state.focused.map(|target| (*window, target)))
                .collect();
            if let (Some(window), Some(target)) = (focus.active_window, focus.focused)
                && !focused.contains(&(window, target))
            {
                focused.push((window, target));
            }
            focused
        })
        .unwrap_or_default();
    for (window, target) in focused {
        begin_text_edit_draft(world, window, target);
    }
}

pub(super) fn begin_text_edit_draft(world: &mut World, window: Entity, target: Entity) {
    if world.get::<TextEditDraft>(target).is_some() || !text_editable(world, target) {
        return;
    }
    let Some(snapshot) = text_snapshot(world, target) else {
        return;
    };
    world.entity_mut(target).insert(TextEditDraft {
        window,
        before: snapshot.text,
        before_selection: snapshot.selection,
    });
}

/// Rebase a live text-edit draft after application history restores its value.
/// The history adapter calls this after restoring focus and selection.
pub(crate) fn refresh_text_edit_draft_after_history_restore(world: &mut World, target: Entity) {
    let focused_window = world.get_resource::<UiFocus>().and_then(|focus| {
        focus
            .windows
            .iter()
            .find_map(|(window, state)| (state.focused == Some(target)).then_some(*window))
            .or_else(|| {
                (focus.focused == Some(target))
                    .then_some(focus.active_window)
                    .flatten()
            })
    });
    if let Some(window) = focused_window {
        world.entity_mut(target).remove::<TextEditDraft>();
        begin_text_edit_draft(world, window, target);
    } else if world.get_entity(target).is_ok() {
        world.entity_mut(target).remove::<TextEditDraft>();
    }
}

pub(super) fn text_editable(world: &World, target: Entity) -> bool {
    world.get::<UiReadOnly>(target).is_none()
        && world
            .get::<TextField>(target)
            .is_none_or(|field| !field.read_only)
        && world
            .get::<TextArea>(target)
            .is_none_or(|area| !area.field.read_only)
}

pub(super) fn text_snapshot(world: &World, target: Entity) -> Option<TextSnapshot> {
    if let Some(buffer) = world.get::<TextBuffer>(target) {
        return Some(TextSnapshot {
            text: buffer.as_str().to_owned(),
            selection: TextSelection {
                anchor: buffer.anchor(),
                caret: buffer.caret(),
            },
        });
    }
    if let Some(buffer) = world.get::<UiTextBuffer>(target) {
        return Some(TextSnapshot {
            text: buffer.0.as_str().to_owned(),
            selection: TextSelection {
                anchor: buffer.0.anchor(),
                caret: buffer.0.caret(),
            },
        });
    }
    if let Some(field) = world.get::<TextField>(target) {
        return Some(text_field_snapshot(
            &field.text,
            field.caret,
            field.selection.as_ref(),
        ));
    }
    world.get::<TextArea>(target).map(|area| {
        text_field_snapshot(
            &area.field.text,
            area.field.caret,
            area.field.selection.as_ref(),
        )
    })
}

pub(super) fn text_field_snapshot(
    text: &str,
    caret: usize,
    selection: Option<&std::ops::Range<usize>>,
) -> TextSnapshot {
    let byte_index = |char_index: usize| {
        text.char_indices()
            .nth(char_index)
            .map_or(text.len(), |(byte, _)| byte)
    };
    let (anchor, caret) = selection.map_or_else(
        || {
            let caret = byte_index(caret);
            (caret, caret)
        },
        |range| (byte_index(range.start), byte_index(range.end)),
    );
    TextSnapshot {
        text: text.to_owned(),
        selection: TextSelection { anchor, caret },
    }
}

pub(super) fn finish_text_edit_on_focus_departure(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(mut buffer) = world.get_mut::<TextBuffer>(target) {
        buffer.cancel_composition();
        buffer.break_undo_group();
    } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(target) {
        buffer.0.cancel_composition();
        buffer.0.break_undo_group();
    }
    let Some(draft) = world.get::<TextEditDraft>(target).cloned() else {
        return;
    };
    let Some(after) = text_snapshot(world, target) else {
        world.entity_mut(target).remove::<TextEditDraft>();
        return;
    };
    if draft.before != after.text {
        if let Some(mut buffer) = world.get_mut::<TextBuffer>(target) {
            buffer.clear_history();
        } else if let Some(mut buffer) = world.get_mut::<UiTextBuffer>(target) {
            buffer.0.clear_history();
        }
        emitted.push(WidgetEvent::TextCommitted {
            target,
            window: draft.window,
            before: draft.before,
            after: after.text,
            before_selection: draft.before_selection,
            after_selection: after.selection,
        });
    }
    world.entity_mut(target).remove::<TextEditDraft>();
}

pub(super) fn finish_numeric_edit_on_focus_departure(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let variables = world
        .get::<NumericExpressionVariables>(target)
        .map(|variables| variables.0.clone())
        .unwrap_or_default();
    let Some(mut field) = world.get_mut::<NumericField>(target) else {
        return;
    };
    if field.editing.is_none() {
        return;
    }
    if !field.enabled || field.read_only {
        field.cancel_edit();
        return;
    }
    let before = field.value;
    match field.commit_edit(&variables) {
        Ok(true) => emitted.push(WidgetEvent::ValueChanged {
            target,
            before: WidgetValue::Number(before),
            after: WidgetValue::Number(field.value),
        }),
        Ok(false) => {}
        Err(error) => {
            field.editing = None;
            field.replace_on_input = false;
            field.expression_error = Some(error.clone());
            emitted.push(WidgetEvent::NumericExpressionError {
                target,
                error: error.to_string(),
            });
        }
    }
}

pub(super) fn finish_hsv_edit_on_focus_departure(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(state) = world.get::<HsvColorEditorState>(target).cloned() else {
        return;
    };
    if !state.editing {
        return;
    }
    edit_hsv_color(world, target, emitted, |editor| {
        set_hsv_numeric_field(editor, state.active_field, &state.edit_buffer)
    });
    if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
        state.editing = false;
        state.edit_buffer.clear();
    }
}

pub(super) fn place_text_caret(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    extend: bool,
) {
    let Some(layout) = world.get::<UiTextLayout>(target).cloned() else {
        return;
    };
    if let Some(mut buffer) = world.get_mut::<TextBuffer>(target) {
        layout.place_caret(&mut buffer, position, extend);
    }
}

pub(super) fn handle_menu_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    _modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) -> bool {
    if world.get::<MenuControl>(target).is_none() {
        return false;
    }
    let focus_restore_key = world
        .get::<bui_core::UiKey>(target)
        .map(|key| key.0.clone());
    let mut command = None;
    let mut restore_focus = false;
    let mut restore_key = None;
    if matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
        && world.get::<MenuControl>(target).is_some_and(|menu| {
            menu.session.open
                && menu.session.path.len() <= 1
                && (key != UiKeyCode::ArrowRight
                    || !menu
                        .session
                        .selected(&menu.tree)
                        .is_some_and(|entry| !entry.children.is_empty()))
        })
        && let Some(sibling) = menu_bar_sibling(
            world,
            target,
            if key == UiKeyCode::ArrowRight { 1 } else { -1 },
        )
    {
        let next_key = world.get::<UiKey>(sibling).map(|key| key.0.clone());
        if let Some(mut current) = world.get_mut::<MenuControl>(target) {
            current.close();
            current.popup.open = false;
        }
        if let Some(mut next) = world.get_mut::<MenuControl>(sibling) {
            next.open(next_key);
            next.popup.open = true;
        }
        if let Some(window) = world
            .get::<UiLayoutResult>(sibling)
            .map(|layout| layout.window)
            && let Some(mut focus) = world.get_resource_mut::<UiFocus>()
        {
            focus.state_mut(window).focused = Some(sibling);
            focus.set_active(window);
        }
        return true;
    }
    if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
        let tree = menu.tree.clone();
        if !menu.session.open {
            if matches!(
                key,
                UiKeyCode::Enter | UiKeyCode::Space | UiKeyCode::ArrowDown
            ) {
                menu.open(focus_restore_key);
                menu.popup.open = true;
            }
            return true;
        }
        match key {
            UiKeyCode::Escape if !menu.query.is_empty() => menu.query.clear(),
            UiKeyCode::Escape => {
                if menu.session.path.len() > 1 {
                    menu.session.close_submenu();
                } else {
                    restore_focus = true;
                    restore_key = menu.session.focus_restore_key.clone();
                    menu.close();
                }
                menu.popup.open = menu.session.open;
            }
            UiKeyCode::Backspace if !menu.query.is_empty() => {
                menu.query.pop();
            }
            UiKeyCode::ArrowDown => {
                if menu.display == MenuDisplay::Pie {
                    let parent_path = if menu.session.path.len() > 1 {
                        menu.session.path[..menu.session.path.len() - 1].to_vec()
                    } else {
                        Vec::new()
                    };
                    let selectable: Vec<_> = tree
                        .entries_at(&parent_path)
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.enabled && !entry.separator)
                        .map(|(index, _)| index)
                        .collect();
                    let next = menu.pie.and_then(|pie| pie.selected).map_or(0, |at| at + 1);
                    let count = selectable.len();
                    if count > 0 {
                        let selected = next % count;
                        if let Some(pie) = &mut menu.pie {
                            pie.select_keyboard(selected, count);
                        }
                        menu.session.path.clone_from(&parent_path);
                        menu.session.path.push(selectable[selected]);
                    }
                } else {
                    menu.session.move_selection(&tree, 1);
                }
            }
            UiKeyCode::ArrowUp => {
                if menu.display == MenuDisplay::Pie {
                    let parent_path = if menu.session.path.len() > 1 {
                        menu.session.path[..menu.session.path.len() - 1].to_vec()
                    } else {
                        Vec::new()
                    };
                    let selectable: Vec<_> = tree
                        .entries_at(&parent_path)
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                        .filter(|(_, entry)| entry.enabled && !entry.separator)
                        .map(|(index, _)| index)
                        .collect();
                    let count = selectable.len();
                    if count > 0 {
                        let selected = menu
                            .pie
                            .and_then(|pie| pie.selected)
                            .map_or(count - 1, |at| (at + count - 1) % count);
                        if let Some(pie) = &mut menu.pie {
                            pie.select_keyboard(selected, count);
                        }
                        menu.session.path.clone_from(&parent_path);
                        menu.session.path.push(selectable[selected]);
                    }
                } else {
                    menu.session.move_selection(&tree, -1);
                }
            }
            UiKeyCode::ArrowRight => {
                menu.session.open_submenu(&tree);
            }
            UiKeyCode::ArrowLeft => {
                menu.session.close_submenu();
            }
            UiKeyCode::Enter | UiKeyCode::Space => {
                let selected = menu.session.selected(&tree).cloned();
                if selected
                    .as_ref()
                    .is_some_and(|entry| !entry.children.is_empty())
                {
                    menu.session.open_submenu(&tree);
                } else {
                    command = menu.session.activate(&tree);
                    if let Some(id) = &command {
                        menu.search.pick(id.clone());
                        restore_focus = true;
                        restore_key = menu.session.focus_restore_key.clone();
                        menu.close();
                    }
                    menu.popup.open = menu.session.open;
                }
            }
            _ => {}
        }
    }
    if let Some(command_id) = command {
        emitted.push(WidgetEvent::MenuCommand { target, command_id });
    }
    if restore_focus {
        restore_menu_focus(world, target, restore_key);
    }
    true
}

pub(super) fn menu_bar_sibling(
    world: &mut World,
    target: Entity,
    direction: i32,
) -> Option<Entity> {
    let parent = world.get::<UiParent>(target).map(|parent| parent.0)?;
    if !world
        .get::<UiRole>(parent)
        .is_some_and(|role| role.0 == "menu-bar")
    {
        return None;
    }
    let children = world.get::<bui_core::UiChildren>(parent)?.0.clone();
    let menus: Vec<_> = children
        .into_iter()
        .filter(|child| world.get::<MenuControl>(*child).is_some())
        .collect();
    if menus.len() < 2 {
        return None;
    }
    let index = menus.iter().position(|entity| *entity == target)? as i64;
    let next = (index + i64::from(direction)).rem_euclid(menus.len() as i64) as usize;
    menus.get(next).copied()
}

pub(super) fn select_menu_item(menu: &mut MenuControl, item: &MenuItemControl) {
    if !menu.session.open || !item.enabled || item.separator {
        return;
    }
    menu.session.path.clone_from(&item.parent_path);
    menu.session.path.push(item.index);
}

pub(super) fn select_menu_item_for_release(
    world: &mut World,
    item: &MenuItemControl,
    emitted: &mut Vec<WidgetEvent>,
) {
    if !item.enabled || item.separator {
        return;
    }
    let mut command = None;
    let mut restore_focus = None;
    if let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
        if !menu.session.open {
            return;
        }
        select_menu_item(&mut menu, item);
        if !item.has_submenu {
            command = Some(item.id.clone());
            menu.search.pick(item.id.clone());
            restore_focus = menu.session.focus_restore_key.clone();
            menu.close();
            menu.popup.open = false;
        }
        menu.gesture.pressed = false;
    }
    if command.is_some() {
        restore_menu_focus(world, item.owner, restore_focus);
    }
    if let Some(command_id) = command {
        emitted.push(WidgetEvent::MenuCommand {
            target: item.owner,
            command_id,
        });
    }
}

pub(super) fn menu_item_at_position(
    world: &mut World,
    owner: Entity,
    window: Entity,
    position: [f32; 2],
) -> Option<MenuItemControl> {
    let mut query = world.query::<(Entity, &MenuItemControl, &UiHitBox)>();
    query
        .iter(world)
        .filter(|(entity, item, hit)| {
            item.owner == owner
                && is_interactable_in_window(world, *entity, window)
                && hit.window.is_none_or(|hit_window| hit_window == window)
                && hit.contains(position)
        })
        .max_by_key(|(_, _, hit)| hit.z_index)
        .map(|(_, item, _)| item.clone())
}

pub(super) fn update_menu_pointer(
    world: &mut World,
    target: Entity,
    window: Entity,
    position: [f32; 2],
    dragging: bool,
) {
    if let Some(item) = world.get::<MenuItemControl>(target).cloned() {
        let display = world
            .get::<MenuControl>(item.owner)
            .map_or(MenuDisplay::Dropdown, |menu| menu.display);
        if display == MenuDisplay::Pie {
            update_pie_menu_pointer(world, item.owner, position);
        } else {
            let hovered = if dragging {
                menu_item_at_position(world, item.owner, window, position)
            } else {
                Some(item.clone())
            };
            if let Some(hovered) = hovered {
                if let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
                    select_menu_item(&mut menu, &hovered);
                    if dragging {
                        menu.gesture.drag_over(Some(hovered.index));
                    }
                }
            } else if dragging && let Some(mut menu) = world.get_mut::<MenuControl>(item.owner) {
                menu.gesture.drag_over(None);
                menu.session.path.clear();
            }
        }
        return;
    }

    let drag_open = world.get::<MenuControl>(target).is_some_and(|menu| {
        dragging
            && !menu.session.open
            && menu.display != MenuDisplay::Context
            && menu.press_position.is_some()
    });
    if drag_open {
        let start = world
            .get::<MenuControl>(target)
            .and_then(|menu| menu.press_position);
        let focus_restore_key = menu_focus_key(world, target);
        if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
            menu.open(focus_restore_key);
            menu.opened_by_pointer = true;
            menu.popup.open = true;
            if let (Some(pie), Some(start)) = (&mut menu.pie, start) {
                pie.center = start;
            }
        }
        sync_menu_popups(world);
    }

    let display = world.get::<MenuControl>(target).map(|menu| menu.display);
    if display == Some(MenuDisplay::Pie)
        && world
            .get::<MenuControl>(target)
            .is_some_and(|menu| menu.session.open)
    {
        update_pie_menu_pointer(world, target, position);
    } else if dragging
        && world
            .get::<MenuControl>(target)
            .is_some_and(|menu| menu.session.open)
        && let Some(item) = menu_item_at_position(world, target, window, position)
        && let Some(mut menu) = world.get_mut::<MenuControl>(target)
    {
        select_menu_item(&mut menu, &item);
        menu.gesture.drag_over(Some(item.index));
    }
}

pub(super) fn update_pie_menu_pointer(world: &mut World, owner: Entity, position: [f32; 2]) {
    let Some(menu) = world.get::<MenuControl>(owner) else {
        return;
    };
    if !menu.session.open || menu.display != MenuDisplay::Pie {
        return;
    }
    let path = if menu.session.path.len() > 1 {
        menu.session.path[..menu.session.path.len() - 1].to_vec()
    } else {
        Vec::new()
    };
    let Some(entries) = menu.tree.entries_at(&path) else {
        return;
    };
    let selectable: Vec<_> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.enabled && !entry.separator)
        .map(|(index, _)| index)
        .collect();
    let Some(mut pie) = menu.pie else {
        return;
    };
    let selected = pie.update_pointer(position, selectable.len());
    if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
        menu.pie = Some(pie);
        if let Some(index) = selected.and_then(|at| selectable.get(at).copied()) {
            menu.session.path.clone_from(&path);
            menu.session.path.push(index);
        } else {
            menu.session.path.clear();
        }
    }
}

pub(super) fn selected_menu_item(world: &World, owner: Entity) -> Option<MenuItemControl> {
    let menu = world.get::<MenuControl>(owner)?;
    let (&index, parent_path) = menu.session.path.split_last()?;
    menu.items.iter().find_map(|entity| {
        world
            .get::<MenuItemControl>(*entity)
            .filter(|item| item.index == index && item.parent_path == parent_path)
            .cloned()
    })
}

pub(super) fn close_menus_outside(world: &mut World, window: Entity, target: Entity) {
    if world.get::<MenuItemControl>(target).is_some() {
        return;
    }
    let mut query = world.query::<(Entity, &MenuControl, &UiLayoutResult)>();
    let open_menus: Vec<_> = query
        .iter(world)
        .filter(|(_, menu, layout)| menu.session.open && layout.window == window)
        .map(|(entity, _, _)| entity)
        .collect();
    for owner in open_menus {
        if owner == target {
            continue;
        }
        if let Some(mut menu) = world.get_mut::<MenuControl>(owner) {
            menu.close();
            menu.popup.open = false;
        }
    }
}

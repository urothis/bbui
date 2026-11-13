use super::*;

pub(super) fn activate_target(
    world: &mut World,
    target: Entity,
    click_count: u32,
    emitted: &mut Vec<WidgetEvent>,
) {
    if let Some(cell) = world.get::<DataTableCellControl>(target).cloned() {
        select_data_table_cell(world, &cell, false, false, 1, emitted);
    } else if let Some(owner) = data_table_owner(world, target) {
        if let Some((row_id, column)) = activate_data_table_cell(world, owner) {
            emitted.push(WidgetEvent::DataTableCellActivated {
                target: owner,
                row_id,
                column,
            });
        }
    } else if world.get::<HsvColorEditor>(target).is_some() {
        if let Some(mut state) = world.get_mut::<HsvColorEditorState>(target) {
            state.editing = true;
            state.edit_buffer.clear();
        }
        emitted.push(WidgetEvent::Activated { target });
    } else if world.get::<CurveEditor>(target).is_some() {
        emitted.push(WidgetEvent::Activated { target });
    } else if world.get::<MenuControl>(target).is_some() {
        toggle_menu(world, target);
    } else if let Some(menu_item) = world.get::<MenuItemControl>(target).cloned() {
        select_menu_item_for_release(world, &menu_item, emitted);
    } else if let Some(option) = world.get::<DropdownOptionControl>(target).cloned() {
        select_dropdown_option(world, option.owner, option.index, emitted);
    } else if let Some(option) = world.get::<RadioOptionControl>(target).cloned() {
        select_radio_option(world, option.owner, option.index, emitted);
    } else if let Some(file_item) = world.get::<FileBrowserItem>(target).cloned() {
        let window = world
            .get::<UiLayoutResult>(target)
            .map_or(Entity::PLACEHOLDER, |layout| layout.window);
        select_file_browser_item(world, &file_item, 1, window, emitted);
    } else if let Some(item) = world.get::<CollectionItemControl>(target).cloned() {
        edit_collection(world, item.owner, emitted, |world, owner| {
            if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                list.select(item.id, false, false)
            } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                tree.select(item.id, false, false)
            } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                grid.select(item.id, false, false)
            } else {
                false
            }
        });
    } else if let Some(mut button) = world.get_mut::<Button>(target) {
        if button.activate() {
            emitted.push(WidgetEvent::Activated { target });
        }
    } else if let Some(mut button) = world.get_mut::<IconButton>(target) {
        if button.activate() {
            emitted.push(WidgetEvent::Activated { target });
        }
    } else if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
        let before = checkbox.value == CheckboxValue::On;
        if checkbox.toggle() {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Bool(before),
                after: WidgetValue::Bool(checkbox.is_checked()),
            });
        }
    } else if let Some(mut panel) = world.get_mut::<Panel>(target) {
        let before = panel.state.target_expanded;
        if panel.toggle() {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Bool(before),
                after: WidgetValue::Bool(panel.state.target_expanded),
            });
        }
    } else if let Some(mut dropdown) = world.get_mut::<Dropdown>(target) {
        if dropdown.open {
            dropdown.close();
        } else {
            dropdown.open();
        }
    } else if let Some(mut field) = world.get_mut::<NumericField>(target)
        && click_count >= 2
    {
        field.begin_edit();
    }
}

pub(super) fn handle_slider_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(mut slider) = world.get_mut::<Slider>(target) else {
        return;
    };
    if !slider.enabled || slider.read_only {
        return;
    }
    let before = slider.value;
    let changed = match key {
        UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowDown
        | UiKeyCode::ArrowRight
        | UiKeyCode::ArrowUp
        | UiKeyCode::PageUp
        | UiKeyCode::PageDown => {
            let direction = match key {
                UiKeyCode::ArrowLeft | UiKeyCode::ArrowDown | UiKeyCode::PageDown => -1.0,
                _ => 1.0,
            };
            let multiplier =
                if matches!(key, UiKeyCode::PageUp | UiKeyCode::PageDown) || modifiers.control {
                    10.0
                } else if modifiers.shift && slider.kind == NumericFieldKind::Float {
                    0.1
                } else {
                    1.0
                };
            let step = slider.step;
            slider
                .set_value(before + direction * step * multiplier)
                .unwrap_or(false)
        }
        UiKeyCode::Home => slider
            .min
            .is_some_and(|min| slider.set_value(min).unwrap_or(false)),
        UiKeyCode::End => slider
            .max
            .is_some_and(|max| slider.set_value(max).unwrap_or(false)),
        UiKeyCode::Backspace => slider.reset(),
        _ => return,
    };
    if changed {
        emitted.push(WidgetEvent::ValueChanged {
            target,
            before: WidgetValue::Number(before),
            after: WidgetValue::Number(slider.value),
        });
    }
}

pub(super) fn handle_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    if world.get::<Panel>(target).is_some()
        && (modifiers.control || modifiers.super_key)
        && matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown)
    {
        reorder_panel_step(
            world,
            target,
            if key == UiKeyCode::ArrowUp { -1 } else { 1 },
            emitted,
        );
        return;
    }
    if let Some(owner) = data_table_owner(world, target) {
        handle_data_table_key(world, owner, key, modifiers, emitted);
        return;
    }
    if let Some(owner) = collection_owner(world, target) {
        handle_collection_key(world, owner, key, modifiers, emitted);
        return;
    }
    if world.get::<HsvColorEditor>(target).is_some() {
        handle_hsv_color_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<CurveEditor>(target).is_some() {
        handle_curve_editor_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<ColorRamp>(target).is_some() {
        handle_color_ramp_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<FileBrowser>(target).is_some() {
        handle_file_browser_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<Slider>(target).is_some() {
        handle_slider_key(world, target, key, modifiers, emitted);
        return;
    }
    if world.get::<TextBuffer>(target).is_some() {
        if modifiers.control || modifiers.super_key {
            match key {
                UiKeyCode::C => {
                    if let Some(buffer) = world.get::<TextBuffer>(target)
                        && let Some(text) = buffer.copy()
                    {
                        emitted.push(WidgetEvent::ClipboardWrite { target, text });
                    }
                }
                UiKeyCode::X if world.get::<UiReadOnly>(target).is_none() => {
                    if let Some(mut buffer) = world.get_mut::<TextBuffer>(target)
                        && let Some(text) = buffer.cut()
                    {
                        emitted.push(WidgetEvent::ClipboardWrite { target, text });
                        emitted.push(WidgetEvent::TextChanged {
                            target,
                            text: buffer.as_str().to_owned(),
                        });
                    }
                }
                UiKeyCode::V if world.get::<UiReadOnly>(target).is_none() => {
                    emitted.push(WidgetEvent::ClipboardRead { target });
                }
                _ => {}
            }
        } else if matches!(
            key,
            UiKeyCode::Backspace | UiKeyCode::Delete | UiKeyCode::Enter
        ) && let Some(buffer) = world.get::<TextBuffer>(target)
        {
            emitted.push(WidgetEvent::TextChanged {
                target,
                text: buffer.as_str().to_owned(),
            });
        }
        return;
    }
    if (key == UiKeyCode::Space || key == UiKeyCode::Enter)
        && world.get::<NumericField>(target).is_none()
        && world.get::<TextField>(target).is_none()
        && world.get::<TextArea>(target).is_none()
        && world.get::<Dropdown>(target).is_none()
    {
        if let Some(mut radio) = world.get_mut::<RadioGroup>(target) {
            let before = radio.selected;
            radio.move_selection(1);
            if before != radio.selected {
                emitted.push(WidgetEvent::ValueChanged {
                    target,
                    before: WidgetValue::Option(before),
                    after: WidgetValue::Option(radio.selected),
                });
            }
        } else {
            activate_target(world, target, 1, emitted);
        }
        return;
    }
    let expression_variables = if key == UiKeyCode::Enter {
        world
            .get::<NumericExpressionVariables>(target)
            .map(|variables| variables.0.clone())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    if let Some(mut field) = world.get_mut::<NumericField>(target) {
        match key {
            UiKeyCode::A if modifiers.control || modifiers.super_key => {
                field.edit_select_all();
            }
            UiKeyCode::ArrowUp | UiKeyCode::ArrowDown => {
                let sign = if key == UiKeyCode::ArrowUp { 1.0 } else { -1.0 };
                let factor = if modifiers.control {
                    10.0
                } else if modifiers.shift && field.kind == NumericFieldKind::Float {
                    0.1
                } else {
                    1.0
                };
                let before = field.value;
                let step = field.step;
                let _ = field.set_value(before + step * sign * factor);
                if before != field.value {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            }
            UiKeyCode::Backspace if field.editing.is_none() => {
                let before = field.value;
                field.reset();
                if before != field.value {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            }
            UiKeyCode::Backspace if field.editing.is_some() => field.edit_backspace(),
            UiKeyCode::Escape => field.cancel_edit(),
            UiKeyCode::Enter if field.editing.is_some() => {
                let before = field.value;
                match field.commit_edit(&expression_variables) {
                    Ok(true) => emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    }),
                    Ok(false) => {}
                    Err(error) => emitted.push(WidgetEvent::NumericExpressionError {
                        target,
                        error: error.to_string(),
                    }),
                }
            }
            UiKeyCode::C if modifiers.control => emitted.push(WidgetEvent::ClipboardWrite {
                target,
                text: field.clipboard_value(),
            }),
            UiKeyCode::V if modifiers.control => {
                emitted.push(WidgetEvent::ClipboardRead { target })
            }
            _ => {}
        }
        return;
    }
    if let Some(mut field) = world.get_mut::<TextField>(target) {
        if modifiers.control && key == UiKeyCode::C {
            emitted.push(WidgetEvent::ClipboardWrite {
                target,
                text: field.selected_text().unwrap_or_else(|| field.text.clone()),
            });
            return;
        }
        if modifiers.control && key == UiKeyCode::V {
            emitted.push(WidgetEvent::ClipboardRead { target });
            return;
        }
        handle_text_key(&mut field, key, modifiers, target, emitted);
        return;
    }
    if let Some(mut field) = world.get_mut::<TextArea>(target) {
        if modifiers.control && key == UiKeyCode::C {
            emitted.push(WidgetEvent::ClipboardWrite {
                target,
                text: field
                    .field
                    .selected_text()
                    .unwrap_or_else(|| field.field.text.clone()),
            });
            return;
        }
        if modifiers.control && key == UiKeyCode::V {
            emitted.push(WidgetEvent::ClipboardRead { target });
            return;
        }
        if matches!(key, UiKeyCode::ArrowUp | UiKeyCode::ArrowDown) {
            let before = field.field.text.clone();
            let current = field.field.caret;
            let prefix: String = field.field.text.chars().take(current).collect();
            let line_start = prefix
                .rfind('\n')
                .map_or(0, |byte| prefix[..byte].chars().count() + 1);
            let column = current.saturating_sub(line_start);
            field.move_vertical(if key == UiKeyCode::ArrowUp { -1 } else { 1 }, column);
            if before != field.field.text {
                emitted.push(WidgetEvent::TextChanged {
                    target,
                    text: field.field.text.clone(),
                });
            }
        } else {
            handle_text_key(&mut field.field, key, modifiers, target, emitted);
        }
        return;
    }
    if world.get::<Dropdown>(target).is_some() {
        let page = world
            .get::<DropdownPopupState>(target)
            .map_or(8, |state| state.visible_count.max(1))
            .min(i32::MAX as usize) as i32;
        let Some(mut dropdown) = world.get_mut::<Dropdown>(target) else {
            return;
        };
        match key {
            UiKeyCode::Space => {
                if dropdown.open {
                    dropdown.close();
                } else {
                    dropdown.open();
                }
            }
            UiKeyCode::ArrowDown => {
                if !dropdown.open {
                    dropdown.open();
                } else {
                    dropdown.move_highlight(1);
                }
            }
            UiKeyCode::ArrowUp => {
                if !dropdown.open {
                    dropdown.open();
                } else {
                    dropdown.move_highlight(-1);
                }
            }
            UiKeyCode::PageDown if dropdown.open => {
                dropdown.move_highlight(page);
            }
            UiKeyCode::PageUp if dropdown.open => {
                dropdown.move_highlight(-page);
            }
            UiKeyCode::Home if dropdown.open && !dropdown.options.is_empty() => {
                dropdown.highlighted = Some(0);
            }
            UiKeyCode::End if dropdown.open && !dropdown.options.is_empty() => {
                dropdown.highlighted = Some(dropdown.options.len() - 1);
            }
            UiKeyCode::Escape => dropdown.close(),
            UiKeyCode::Enter => {
                if !dropdown.open {
                    dropdown.open();
                } else if let Some(index) = dropdown.highlighted {
                    let before = dropdown.selected;
                    if dropdown.choose(index) {
                        emitted.push(WidgetEvent::ValueChanged {
                            target,
                            before: WidgetValue::Option(before),
                            after: WidgetValue::Option(dropdown.selected),
                        });
                    }
                }
            }
            _ => {}
        }
        return;
    }
    if let Some(mut radio) = world.get_mut::<RadioGroup>(target) {
        let before = radio.selected;
        match key {
            UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => {
                radio.move_selection(1);
            }
            UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => {
                radio.move_selection(-1);
            }
            _ => {}
        }
        if before != radio.selected {
            emitted.push(WidgetEvent::ValueChanged {
                target,
                before: WidgetValue::Option(before),
                after: WidgetValue::Option(radio.selected),
            });
        }
    }
}

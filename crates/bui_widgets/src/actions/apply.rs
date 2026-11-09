use super::*;

/// Apply actions requested by application code through `WidgetActionQueue`.
pub fn apply_widget_actions(world: &mut World) {
    let actions: Vec<_> = world
        .get_resource_mut::<WidgetActionQueue>()
        .map(|mut queue| queue.drain().collect())
        .unwrap_or_default();
    let mut emitted = Vec::new();
    for action in actions {
        apply_action(world, action, &mut emitted);
    }
    world.resource_mut::<WidgetEventQueue>().0.extend(emitted);
}

pub(super) fn apply_action(
    world: &mut World,
    action: WidgetAction,
    emitted: &mut Vec<WidgetEvent>,
) {
    let target = action.target;
    match action.action {
        WidgetActionKind::Activate => activate_target(world, target, 1, emitted),
        WidgetActionKind::Toggle => {
            if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
                let before = checkbox.is_checked();
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
            }
        }
        WidgetActionKind::SetBool(value) => {
            if let Some(mut checkbox) = world.get_mut::<Checkbox>(target) {
                let before = checkbox.is_checked();
                if checkbox.set(value) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Bool(before),
                        after: WidgetValue::Bool(value),
                    });
                }
            }
        }
        WidgetActionKind::SetNumber(value) => {
            if let Some(mut field) = world.get_mut::<NumericField>(target) {
                let before = field.value;
                if field.set_value(value).unwrap_or(false) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            } else if let Some(mut slider) = world.get_mut::<Slider>(target) {
                let before = slider.value;
                if slider.set_value(value).unwrap_or(false) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(slider.value),
                    });
                }
            }
        }
        WidgetActionKind::StepNumber(steps) => {
            if let Some(mut field) = world.get_mut::<NumericField>(target) {
                let before = field.value;
                if field.step_by(steps) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            } else if let Some(mut slider) = world.get_mut::<Slider>(target) {
                let before = slider.value;
                if slider.step_by(steps) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(slider.value),
                    });
                }
            }
        }
        WidgetActionKind::ResetNumber => {
            if let Some(mut field) = world.get_mut::<NumericField>(target) {
                let before = field.value;
                if field.reset() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(field.value),
                    });
                }
            } else if let Some(mut slider) = world.get_mut::<Slider>(target) {
                let before = slider.value;
                if slider.reset() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Number(before),
                        after: WidgetValue::Number(slider.value),
                    });
                }
            }
        }
        WidgetActionKind::InsertText(text) => insert_text(world, target, &text, emitted),
        WidgetActionKind::ChooseOption(index) => {
            if let Some(option) = world.get::<DropdownOptionControl>(target).cloned() {
                select_dropdown_option(world, option.owner, option.index, emitted);
            } else if let Some(option) = world.get::<RadioOptionControl>(target).cloned() {
                select_radio_option(world, option.owner, option.index, emitted);
            } else if world.get::<Dropdown>(target).is_some() {
                select_dropdown_option(world, target, index, emitted);
            } else if let Some(mut radio) = world.get_mut::<RadioGroup>(target) {
                let before = radio.selected;
                if radio.select(index) {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Option(before),
                        after: WidgetValue::Option(radio.selected),
                    });
                }
            }
        }
        WidgetActionKind::FileNavigateUp => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.up() {
                    Ok(true) => emitted.push(WidgetEvent::FileDirectoryChanged {
                        target,
                        path: browser.directory.clone(),
                    }),
                    Ok(false) => {}
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileNavigateTo(path) => {
            navigate_file_browser(world, target, path, emitted);
        }
        WidgetActionKind::FileRefresh => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let directory = browser.directory.clone();
                match browser.refresh() {
                    Ok(()) => emitted.push(WidgetEvent::FileDirectoryChanged {
                        target,
                        path: directory,
                    }),
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileSetFilter(filter) => {
            // This action mirrors edits from an external TextField. Its local TextBuffer owns
            // text undo, so browser snapshots deliberately do not create one transaction per key.
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.set_filter(filter);
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        WidgetActionKind::FileSetView(view) => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.view = view;
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileSetSort(sort, ascending) => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.set_sort(sort, ascending);
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileOpenIndex(index) => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target)
                && browser.select(index, false, false)
            {
                match browser.open_cursor() {
                    Ok(Some(path)) => emitted.push(WidgetEvent::FileActivated { target, path }),
                    Ok(None) => emitted.push(WidgetEvent::FileDirectoryChanged {
                        target,
                        path: browser.directory.clone(),
                    }),
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileBookmarkCurrent => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let directory = browser.directory.clone();
                match browser.bookmark(directory) {
                    Ok(true) => emitted.push(WidgetEvent::FileBookmarksChanged {
                        target,
                        bookmarks: browser.bookmarks.clone(),
                    }),
                    Ok(false) => {}
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileRemoveBookmarkCurrent => {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let directory = browser.directory.clone();
                if browser.remove_bookmark(directory) {
                    emitted.push(WidgetEvent::FileBookmarksChanged {
                        target,
                        bookmarks: browser.bookmarks.clone(),
                    });
                }
            }
            emit_file_browser_commit(world, target, before, emitted);
        }
        WidgetActionKind::FileCreateDirectory(name) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.create_directory(&name) {
                    Ok(path) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "create directory",
                            paths: vec![path],
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileRenameSelected(name) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target)
                && let Some(path) = browser.selected.iter().next().cloned()
            {
                match browser.rename(path, &name) {
                    Ok(destination) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "rename",
                            paths: vec![destination],
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileDeleteSelected => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let requested: Vec<_> = browser.selected.iter().cloned().collect();
                let present_before: HashSet<_> = requested
                    .iter()
                    .filter(|path| std::fs::symlink_metadata(path).is_ok())
                    .cloned()
                    .collect();
                match browser.delete_selected() {
                    Ok(_) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "delete",
                            paths: requested,
                            irreversible: true,
                        });
                    }
                    Err(error) => {
                        let partial_effects = present_before
                            .into_iter()
                            .filter(|path| {
                                std::fs::symlink_metadata(path).is_err_and(|error| {
                                    error.kind() == std::io::ErrorKind::NotFound
                                })
                            })
                            .collect();
                        emit_file_error_with_effects(target, error, partial_effects, emitted);
                    }
                }
            }
        }
        WidgetActionKind::FileCopySelectedTo(directory) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.copy_selected_to(directory) {
                    Ok(paths) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "copy",
                            paths,
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::FileMoveSelectedTo(directory) => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                match browser.move_selected_to(directory) {
                    Ok(paths) => {
                        emitted.push(WidgetEvent::FileDirectoryChanged {
                            target,
                            path: browser.directory.clone(),
                        });
                        emitted.push(WidgetEvent::FileOperationCommitted {
                            target,
                            operation: "move",
                            paths,
                            irreversible: true,
                        });
                    }
                    Err(error) => emit_file_error(target, error, emitted),
                }
            }
        }
        WidgetActionKind::TogglePanel => {
            if let Some(mut panel) = world.get_mut::<Panel>(target) {
                let before = panel.state.target_expanded;
                if panel.toggle() {
                    emitted.push(WidgetEvent::ValueChanged {
                        target,
                        before: WidgetValue::Bool(before),
                        after: WidgetValue::Bool(panel.state.target_expanded),
                    });
                }
            }
        }
        WidgetActionKind::PanelReorderStep(delta) => {
            reorder_panel_step(world, target, delta, emitted);
        }
        WidgetActionKind::ChooseMenuCommand(command_id) => {
            let restore_key = if let Some(mut menu) = world.get_mut::<MenuControl>(target)
                && menu.tree.flattened().iter().any(|(_, _, entry)| {
                    entry.id == command_id && entry.enabled && !entry.separator
                }) {
                menu.search.pick(command_id.clone());
                let restore_key = menu.session.focus_restore_key.clone();
                menu.close();
                menu.popup.open = false;
                Some(restore_key)
            } else {
                None
            };
            if let Some(restore_key) = restore_key {
                restore_menu_focus(world, target, restore_key);
                emitted.push(WidgetEvent::MenuCommand { target, command_id });
            }
        }
        WidgetActionKind::ClearMenuHistory => {
            if let Some(mut menu) = world.get_mut::<MenuControl>(target) {
                menu.clear_search_history();
            }
        }
        WidgetActionKind::ColorRampAddStop { position } => {
            edit_color_ramp(world, target, emitted, |ramp| {
                let position = position.clamp(0.0, 1.0);
                ramp.add_stop(position, ramp.sample(position)).is_some()
            });
        }
        WidgetActionKind::ColorRampRemoveSelectedStop => {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected.is_some_and(|id| ramp.remove_stop(id))
            });
        }
        WidgetActionKind::ColorRampSelectStop(id) => {
            if let Some(mut ramp) = world.get_mut::<ColorRamp>(target) {
                ramp.select_stop(id);
            }
        }
        WidgetActionKind::ColorRampMoveStop { id, position } => {
            edit_color_ramp(world, target, emitted, |ramp| ramp.move_stop(id, position));
        }
        WidgetActionKind::ColorRampSetChannel { channel, value } => {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.selected
                    .is_some_and(|id| ramp.set_color_channel(id, channel, value))
            });
        }
        WidgetActionKind::ColorRampSetInterpolation(interpolation) => {
            edit_color_ramp(world, target, emitted, |ramp| {
                ramp.set_interpolation(interpolation)
            });
        }
        WidgetActionKind::HsvColorSet(color) => {
            edit_hsv_color(world, target, emitted, |editor| editor.set_rgba(color));
        }
        WidgetActionKind::HsvColorSetHsv { field, value } => {
            edit_hsv_color(world, target, emitted, |editor| {
                editor.set_hsv(field, value)
            });
        }
        WidgetActionKind::HsvColorSetRgb { field, value } => {
            edit_hsv_color(world, target, emitted, |editor| {
                editor.set_rgb(field, value)
            });
        }
        WidgetActionKind::HsvColorSetHex(hex) => {
            edit_hsv_color(world, target, emitted, |editor| {
                editor.set_hex(&hex).unwrap_or(false)
            });
        }
        WidgetActionKind::CurveEditorAddPoint { position } => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.add_point(position[0], position[1]).is_some()
            });
        }
        WidgetActionKind::CurveEditorRemoveSelectedPoint => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.selected.is_some_and(|id| editor.remove_point(id))
            });
        }
        WidgetActionKind::CurveEditorSelectPoint(id) => {
            if let Some(mut editor) = world.get_mut::<CurveEditor>(target) {
                editor.select_point(id);
            }
        }
        WidgetActionKind::CurveEditorMovePoint { id, position } => {
            edit_curve_editor(world, target, emitted, |editor| {
                editor.move_point(id, position)
            });
        }
        WidgetActionKind::CurveEditorSetInterpolation(interpolation) => {
            edit_curve_editor(world, target, emitted, |editor| {
                if editor.interpolation == interpolation {
                    false
                } else {
                    editor.interpolation = interpolation;
                    true
                }
            });
        }
        WidgetActionKind::CurveEditorResetView => {
            edit_curve_editor(world, target, emitted, |editor| {
                if editor.pan == [0.0, 0.0] && editor.zoom == 1.0 {
                    false
                } else {
                    editor.reset_view();
                    true
                }
            });
        }
        WidgetActionKind::CollectionSelect { id, ctrl, shift } => {
            edit_collection(world, target, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    let before = list.selection.clone();
                    list.select(id, ctrl, shift);
                    before != list.selection
                } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                    let before = tree.selection.clone();
                    tree.select(id, ctrl, shift);
                    before != tree.selection
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    let before = grid.selection.clone();
                    grid.select(id, ctrl, shift);
                    before != grid.selection
                } else {
                    false
                }
            });
        }
        WidgetActionKind::CollectionSelectAll => {
            edit_collection(world, target, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    let before = list.selection.clone();
                    list.selection.selected = list
                        .items
                        .iter()
                        .filter(|item| item.enabled)
                        .map(|item| item.id)
                        .collect();
                    list.selection.active = list.selection.selected.iter().next_back().copied();
                    before != list.selection
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    let before = grid.selection.clone();
                    grid.selection.selected = grid
                        .items
                        .iter()
                        .filter(|item| item.enabled)
                        .map(|item| item.id)
                        .collect();
                    grid.selection.active = grid.selection.selected.iter().next_back().copied();
                    before != grid.selection
                } else {
                    false
                }
            });
        }
        WidgetActionKind::CollectionBeginRename(id) => begin_collection_rename(world, target, id),
        WidgetActionKind::CollectionCommitRename => {
            commit_collection_rename(world, target, emitted)
        }
        WidgetActionKind::CollectionCancelRename => cancel_collection_rename(world, target),
        WidgetActionKind::CollectionMove { id, index } => {
            edit_collection(world, target, emitted, |world, owner| {
                if let Some(mut list) = world.get_mut::<ListModel<String>>(owner) {
                    list.reorder(id, index)
                } else if let Some(mut tree) = world.get_mut::<TreeModel<String>>(owner) {
                    tree.reorder_sibling(id, index)
                } else if let Some(mut grid) = world.get_mut::<PreviewGrid>(owner) {
                    grid.reorder(id, index)
                } else {
                    false
                }
            });
        }
        WidgetActionKind::CollectionScroll(rows) => scroll_collection_by_rows(world, target, rows),
        WidgetActionKind::TreeToggleExpanded(id) => {
            edit_collection(world, target, emitted, |world, owner| {
                world
                    .get_mut::<TreeModel<String>>(owner)
                    .is_some_and(|mut tree| tree.toggle_expanded(id))
            });
        }
        WidgetActionKind::TreeMoveNode { id, parent, index } => {
            edit_collection(world, target, emitted, |world, owner| {
                world
                    .get_mut::<TreeModel<String>>(owner)
                    .is_some_and(|mut tree| tree.move_node(id, parent, index))
            });
        }
        WidgetActionKind::DataTableSelectCell {
            row_id,
            column,
            ctrl,
            shift,
        } => {
            edit_data_table(world, target, emitted, |table| {
                table.select_cell(row_id, column, ctrl, shift)
            });
        }
        WidgetActionKind::DataTableSelectAll => {
            edit_data_table(world, target, emitted, DataTable::select_all);
        }
        WidgetActionKind::DataTableSortColumn(column) => {
            edit_data_table(world, target, emitted, |table| {
                let changed = table.set_active_column(column);
                table.toggle_sort(column) || changed
            });
        }
        WidgetActionKind::DataTableResizeColumn { column, width } => {
            edit_data_table(world, target, emitted, |table| {
                table.resize_column(column, width)
            });
        }
        WidgetActionKind::DataTableMoveActiveRow { delta, extend } => {
            move_data_table_active_row(world, target, delta, extend, emitted);
        }
        WidgetActionKind::DataTableMoveActiveColumn(delta) => {
            edit_data_table(world, target, emitted, |table| {
                let before = table.active_column;
                table.move_active_column(delta);
                before != table.active_column
            });
        }
        WidgetActionKind::DataTableSetActiveColumn(column) => {
            edit_data_table(world, target, emitted, |table| {
                table.set_active_column(column)
            });
        }
        WidgetActionKind::DataTableScrollRows(rows) => {
            scroll_data_table_by_rows(world, target, rows)
        }
        WidgetActionKind::DataTableActivateCell => {
            if let Some(owner) = data_table_owner(world, target)
                && let Some((row_id, column)) = activate_data_table_cell(world, owner)
            {
                emitted.push(WidgetEvent::DataTableCellActivated {
                    target: owner,
                    row_id,
                    column,
                });
            }
        }
        WidgetActionKind::NodeGraphSelectNode { id, additive } => {
            edit_node_graph(world, target, emitted, |graph| graph.select(id, additive));
        }
        WidgetActionKind::NodeGraphSelectAll => {
            edit_node_graph(world, target, emitted, NodeGraph::select_all);
        }
        WidgetActionKind::NodeGraphSelectLink(link_id) => {
            edit_node_graph(world, target, emitted, |graph| graph.select_link(link_id));
        }
        WidgetActionKind::NodeGraphMoveSelected(delta) => {
            edit_node_graph(world, target, emitted, |graph| {
                graph.move_selected_by(delta)
            });
        }
        WidgetActionKind::NodeGraphAddNode {
            title,
            position,
            sockets,
        } => {
            edit_node_graph(world, target, emitted, |graph| {
                if !position.into_iter().all(f32::is_finite) {
                    return false;
                }
                graph.add_node(title, position, sockets);
                true
            });
        }
        WidgetActionKind::NodeGraphCreateLink {
            from_node,
            from_socket,
            to_node,
            to_socket,
        } => {
            edit_node_graph_result(world, target, emitted, |graph| {
                graph
                    .create_link(from_node, from_socket, to_node, to_socket)
                    .map(|_| true)
            });
        }
        WidgetActionKind::NodeGraphReconnectLink {
            link_id,
            endpoint,
            node_id,
            socket_index,
        } => {
            edit_node_graph_result(world, target, emitted, |graph| {
                graph
                    .reconnect_link(link_id, endpoint, node_id, socket_index)
                    .map(|()| true)
            });
        }
        WidgetActionKind::NodeGraphRemoveLink(link_id) => {
            edit_node_graph(world, target, emitted, |graph| graph.remove_link(link_id));
        }
        WidgetActionKind::NodeGraphDeleteSelection => {
            edit_node_graph(world, target, emitted, |graph| {
                if let Some(link) = graph.selected_link {
                    graph.remove_link(link)
                } else {
                    let selected: Vec<_> = graph.selected.iter().copied().collect();
                    let mut changed = false;
                    for id in selected {
                        changed |= graph.remove_node(id);
                    }
                    changed
                }
            });
        }
        WidgetActionKind::NodeGraphToggleCollapse => {
            edit_node_graph(world, target, emitted, NodeGraph::toggle_selected_collapsed);
        }
        WidgetActionKind::NodeGraphPanBy(delta) => {
            edit_node_graph(world, target, emitted, |graph| {
                let before = graph.pan;
                graph.pan_by(delta);
                graph.pan != before
            });
        }
        WidgetActionKind::NodeGraphZoomBy(factor) => {
            edit_node_graph(world, target, emitted, |graph| {
                let before = graph.zoom;
                graph.zoom_by(factor);
                graph.zoom != before
            });
        }
        WidgetActionKind::CanvasSelectObject { id, additive } => {
            select_canvas_object(world, target, id, additive, emitted);
        }
    }
}

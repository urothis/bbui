use super::*;

pub(super) fn file_browser_metrics(
    world: &World,
    target: Entity,
) -> (
    FileBrowserViewState,
    bbui_style::ThemeFileBrowserSpacing,
    f32,
    f32,
) {
    let view = world
        .get::<FileBrowserViewState>(target)
        .copied()
        .unwrap_or_default();
    let spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.file_browser,
        |theme| theme.spacing.file_browser,
    );
    let header_height = if view.header_height == 48.0 {
        spacing.header_height
    } else {
        view.header_height
    }
    .max(1.0);
    let row_height = if view.row_height == 22.0 {
        spacing.list_row_height
    } else {
        view.row_height
    }
    .max(1.0);
    (view, spacing, header_height, row_height)
}

pub(super) fn file_browser_snapshot(world: &World, target: Entity) -> Option<String> {
    world
        .get::<FileBrowser>(target)
        .and_then(|browser| browser.snapshot_json().ok())
}

pub(super) fn emit_file_browser_commit(
    world: &World,
    target: Entity,
    before: Option<String>,
    emitted: &mut Vec<WidgetEvent>,
) {
    let (Some(before), Some(after)) = (before, file_browser_snapshot(world, target)) else {
        return;
    };
    if before != after {
        emitted.push(WidgetEvent::FileBrowserCommitted {
            target,
            before,
            after,
        });
    }
}

pub(super) fn begin_file_browser_filter_edit(world: &mut World, target: Entity) {
    if let Some(snapshot) = file_browser_snapshot(world, target) {
        let mut edits = world.resource_mut::<WidgetFileBrowserFilterEdits>();
        edits.redo.remove(&target);
        edits.active.entry(target).or_insert(snapshot);
    }
}

pub(super) fn finish_file_browser_filter_edit(
    world: &mut World,
    target: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let before = {
        let mut edits = world.resource_mut::<WidgetFileBrowserFilterEdits>();
        edits.redo.remove(&target);
        edits.active.remove(&target)
    };
    emit_file_browser_commit(world, target, before, emitted);
}

pub(crate) fn undo_file_browser_filter_draft(world: &mut World, target: Entity) -> bool {
    let before = world
        .get_resource_mut::<WidgetFileBrowserFilterEdits>()
        .and_then(|mut edits| edits.active.remove(&target));
    let Some(before) = before else {
        return false;
    };
    let after = file_browser_snapshot(world, target);
    let result = world
        .get_mut::<FileBrowser>(target)
        .map(|mut browser| browser.restore_json(&before));
    match result {
        Some(Ok(())) => {
            if let Some(after) = after {
                world
                    .resource_mut::<WidgetFileBrowserFilterEdits>()
                    .redo
                    .insert(target, after);
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        Some(Err(error)) => {
            if let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>() {
                let message = error.to_string();
                events.0.push(WidgetEvent::FileBrowserRestoreFailed {
                    target,
                    path: error.path,
                    message,
                });
            }
        }
        None => return false,
    }
    true
}

pub(crate) fn redo_file_browser_filter_draft(world: &mut World, target: Entity) -> bool {
    let (active, after) = {
        let Some(mut edits) = world.get_resource_mut::<WidgetFileBrowserFilterEdits>() else {
            return false;
        };
        (
            edits.active.contains_key(&target),
            edits.redo.remove(&target),
        )
    };
    let Some(after) = after else {
        return active;
    };
    let before = file_browser_snapshot(world, target);
    let result = world
        .get_mut::<FileBrowser>(target)
        .map(|mut browser| browser.restore_json(&after));
    match result {
        Some(Ok(())) => {
            if let Some(before) = before {
                world
                    .resource_mut::<WidgetFileBrowserFilterEdits>()
                    .active
                    .insert(target, before);
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        Some(Err(error)) => {
            if let Some(mut events) = world.get_resource_mut::<WidgetEventQueue>() {
                let message = error.to_string();
                events.0.push(WidgetEvent::FileBrowserRestoreFailed {
                    target,
                    path: error.path,
                    message,
                });
            }
        }
        None => return false,
    }
    true
}

pub(super) fn navigate_file_browser(
    world: &mut World,
    target: Entity,
    path: PathBuf,
    emitted: &mut Vec<WidgetEvent>,
) {
    let before = file_browser_snapshot(world, target);
    if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
        match browser.navigate(&path) {
            Ok(()) => emitted.push(WidgetEvent::FileDirectoryChanged { target, path }),
            Err(error) => emit_file_error(target, error, emitted),
        }
    }
    if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
        view.scroll_rows = 0;
    }
    emit_file_browser_commit(world, target, before, emitted);
}

pub(super) fn scroll_file_browser(world: &mut World, target: Entity, delta_y: f32) {
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let (_view, spacing, header_height, row_height) = file_browser_metrics(world, target);
    let Some((browser_view, visible_count)) = world
        .get::<FileBrowser>(target)
        .map(|browser| (browser.view, browser.visible_count()))
    else {
        return;
    };
    let rows = match browser_view {
        FileView::List => ((layout.rect.height - header_height) / row_height)
            .floor()
            .max(1.0) as usize,
        FileView::Grid => ((layout.rect.height - header_height)
            / (spacing.grid_cell_height + spacing.grid_gap))
            .floor()
            .max(1.0) as usize,
    };
    let columns = if browser_view == FileView::Grid {
        (layout.rect.width / (spacing.grid_cell_width + spacing.grid_gap))
            .floor()
            .max(1.0) as usize
    } else {
        1
    };
    let total_rows = visible_count.div_ceil(columns);
    let max_scroll = total_rows.saturating_sub(rows);
    if let Some(mut state) = world.get_mut::<FileBrowserViewState>(target) {
        state.scroll_rows = if delta_y > 0.0 {
            state.scroll_rows.saturating_sub(3)
        } else if delta_y < 0.0 {
            state.scroll_rows.saturating_add(3).min(max_scroll)
        } else {
            state.scroll_rows.min(max_scroll)
        };
    }
}

pub(super) fn ensure_file_cursor_visible(
    world: &mut World,
    target: Entity,
    view: FileBrowserViewState,
    height: f32,
) {
    let Some(browser) = world.get::<FileBrowser>(target) else {
        return;
    };
    let Some(cursor) = browser.cursor.as_ref() else {
        return;
    };
    let Some(index) = browser
        .visible_entries()
        .iter()
        .position(|entry| &entry.path == cursor)
    else {
        return;
    };
    let spacing = world.get_resource::<Theme>().map_or_else(
        || Theme::default().spacing.file_browser,
        |theme| theme.spacing.file_browser,
    );
    let header_height = if view.header_height == 48.0 {
        spacing.header_height
    } else {
        view.header_height
    }
    .max(1.0);
    let row_height = if browser.view == FileView::List {
        if view.row_height == 22.0 {
            spacing.list_row_height.max(1.0)
        } else {
            view.row_height.max(1.0)
        }
    } else {
        (spacing.grid_cell_height + spacing.grid_gap).max(1.0)
    };
    let visible_rows = ((height - header_height) / row_height).floor().max(1.0) as usize;
    let columns = if browser.view == FileView::Grid {
        world.get::<UiLayoutResult>(target).map_or(1, |layout| {
            (layout.rect.width / (spacing.grid_cell_width + spacing.grid_gap))
                .floor()
                .max(1.0) as usize
        })
    } else {
        1
    };
    let row = index / columns;
    if let Some(mut state) = world.get_mut::<FileBrowserViewState>(target) {
        if row < state.scroll_rows {
            state.scroll_rows = row;
        } else if row >= state.scroll_rows + visible_rows {
            state.scroll_rows = row + 1 - visible_rows;
        }
    }
}

pub(super) fn emit_file_error(
    target: Entity,
    error: crate::FileBrowserError,
    emitted: &mut Vec<WidgetEvent>,
) {
    emit_file_error_with_effects(target, error, Vec::new(), emitted);
}

pub(super) fn emit_file_error_with_effects(
    target: Entity,
    error: crate::FileBrowserError,
    partial_effects: Vec<PathBuf>,
    emitted: &mut Vec<WidgetEvent>,
) {
    emitted.push(WidgetEvent::FileOperationFailed {
        target,
        operation: error.operation,
        path: error.path,
        message: error.source.to_string(),
        partial_effects,
    });
}

pub(super) fn select_file_browser_item(
    world: &mut World,
    item: &FileBrowserItem,
    click_count: u32,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let modifiers = world
        .get_resource::<WidgetInteractionState>()
        .and_then(|state| state.modifiers.get(&window).copied())
        .unwrap_or_default();
    let before = file_browser_snapshot(world, item.owner);
    let result = {
        let Some(mut browser) = world.get_mut::<FileBrowser>(item.owner) else {
            return;
        };
        if !browser.select(item.index, modifiers.control, modifiers.shift) {
            return;
        }
        let selected = browser.selected.contains(&item.path);
        let opened = if click_count >= 2 {
            let old_directory = browser.directory.clone();
            match browser.open_cursor() {
                Ok(Some(path)) => Some(Ok(Some(path))),
                Ok(None) if browser.directory != old_directory => Some(Ok(None)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            }
        } else {
            None
        };
        (selected, opened, browser.directory.clone())
    };
    emit_file_browser_commit(world, item.owner, before, emitted);
    emitted.push(WidgetEvent::FileSelected {
        target: item.owner,
        path: item.path.clone(),
        selected: result.0,
    });
    if let Some(opened) = result.1 {
        match opened {
            Ok(Some(path)) => emitted.push(WidgetEvent::FileActivated {
                target: item.owner,
                path,
            }),
            Ok(None) => {
                emitted.push(WidgetEvent::FileDirectoryChanged {
                    target: item.owner,
                    path: result.2,
                });
                if let Some(mut view) = world.get_mut::<FileBrowserViewState>(item.owner) {
                    view.scroll_rows = 0;
                }
            }
            Err(error) => emit_file_error(item.owner, error, emitted),
        }
    } else {
        let view = world
            .get::<FileBrowserViewState>(item.owner)
            .copied()
            .unwrap_or_default();
        let height = world
            .get::<UiLayoutResult>(item.owner)
            .map_or(0.0, |layout| layout.rect.height);
        ensure_file_cursor_visible(world, item.owner, view, height);
    }
}

pub(super) fn handle_file_browser_key(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let current_filter = world
        .get::<FileBrowser>(target)
        .map(|browser| browser.filter().to_owned())
        .unwrap_or_default();
    let filter_edit =
        !current_filter.is_empty() && matches!(key, UiKeyCode::Backspace | UiKeyCode::Escape);
    if filter_edit {
        begin_file_browser_filter_edit(world, target);
    }
    let has_filter_edit = world
        .resource::<WidgetFileBrowserFilterEdits>()
        .active
        .contains_key(&target);
    let before = (!filter_edit && !has_filter_edit)
        .then(|| file_browser_snapshot(world, target))
        .flatten();

    handle_file_browser_key_inner(world, target, key, modifiers, emitted);

    if (filter_edit && key == UiKeyCode::Escape) || (key == UiKeyCode::Enter && has_filter_edit) {
        finish_file_browser_filter_edit(world, target, emitted);
    } else if !filter_edit && !has_filter_edit {
        emit_file_browser_commit(world, target, before, emitted);
    }
}

pub(super) fn handle_file_browser_key_inner(
    world: &mut World,
    target: Entity,
    key: UiKeyCode,
    modifiers: UiModifiers,
    emitted: &mut Vec<WidgetEvent>,
) {
    let view = world
        .get::<FileBrowserViewState>(target)
        .copied()
        .unwrap_or_default();
    let height = world
        .get::<UiLayoutResult>(target)
        .map_or(0.0, |layout| layout.rect.height);
    if (modifiers.control || modifiers.super_key) && key == UiKeyCode::A {
        let changed = world
            .get_mut::<FileBrowser>(target)
            .is_some_and(|mut browser| browser.select_all_visible());
        if changed {
            let selected = world
                .get::<FileBrowser>(target)
                .map(|browser| browser.selected.iter().cloned().collect())
                .unwrap_or_default();
            emitted.push(WidgetEvent::FileSelectionChanged { target, selected });
            ensure_file_cursor_visible(world, target, view, height);
        }
        return;
    }
    let (_, spacing, header_height, list_row_height) = file_browser_metrics(world, target);
    let browser_view = world
        .get::<FileBrowser>(target)
        .map_or(FileView::List, |browser| browser.view);
    let layout_width = world
        .get::<UiLayoutResult>(target)
        .map_or(0.0, |layout| layout.rect.width);
    let columns = if browser_view == FileView::Grid {
        (layout_width / (spacing.grid_cell_width + spacing.grid_gap))
            .floor()
            .max(1.0) as usize
    } else {
        1
    };
    let row_height = if browser_view == FileView::Grid {
        spacing.grid_cell_height + spacing.grid_gap
    } else {
        list_row_height
    }
    .max(1.0);
    let visible_rows = ((height - header_height) / row_height).floor().max(1.0) as usize;
    match key {
        UiKeyCode::ArrowDown
        | UiKeyCode::ArrowUp
        | UiKeyCode::ArrowLeft
        | UiKeyCode::ArrowRight
        | UiKeyCode::PageDown
        | UiKeyCode::PageUp => {
            let delta = match key {
                UiKeyCode::ArrowDown if browser_view == FileView::Grid => columns as i32,
                UiKeyCode::ArrowUp if browser_view == FileView::Grid => -(columns as i32),
                UiKeyCode::ArrowDown | UiKeyCode::ArrowRight => 1,
                UiKeyCode::ArrowUp | UiKeyCode::ArrowLeft => -1,
                UiKeyCode::PageDown => (visible_rows * columns) as i32,
                UiKeyCode::PageUp => -((visible_rows * columns) as i32),
                _ => 0,
            };
            let delta = if browser_view == FileView::List
                && matches!(key, UiKeyCode::ArrowLeft | UiKeyCode::ArrowRight)
            {
                0
            } else {
                delta
            };
            if delta != 0
                && let Some(mut browser) = world.get_mut::<FileBrowser>(target)
            {
                browser.move_cursor(delta, modifiers.shift);
            }
            ensure_file_cursor_visible(world, target, view, height);
        }
        UiKeyCode::Enter => {
            let result = if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let old_directory = browser.directory.clone();
                browser.open_cursor().map(|opened| {
                    (
                        opened,
                        browser.directory != old_directory,
                        browser.directory.clone(),
                    )
                })
            } else {
                return;
            };
            match result {
                Ok((Some(path), _, _)) => emitted.push(WidgetEvent::FileActivated { target, path }),
                Ok((None, true, path)) => {
                    emitted.push(WidgetEvent::FileDirectoryChanged { target, path });
                    if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                        view.scroll_rows = 0;
                    }
                }
                Ok((None, false, _)) => {}
                Err(error) => emit_file_error(target, error, emitted),
            }
        }
        UiKeyCode::Backspace => {
            let filter = world
                .get::<FileBrowser>(target)
                .map(|browser| browser.filter().to_owned())
                .unwrap_or_default();
            if filter.is_empty() {
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
            } else if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let mut filter = filter;
                filter.pop();
                browser.set_filter(filter);
                if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                    view.scroll_rows = 0;
                }
            }
        }
        UiKeyCode::Escape => {
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.set_filter(String::new());
            }
            if let Some(mut view) = world.get_mut::<FileBrowserViewState>(target) {
                view.scroll_rows = 0;
            }
        }
        UiKeyCode::Home | UiKeyCode::End => {
            let count = world
                .get::<FileBrowser>(target)
                .map_or(0, FileBrowser::visible_count);
            if count > 0 {
                let index = if key == UiKeyCode::Home { 0 } else { count - 1 };
                if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                    browser.select(index, modifiers.control, modifiers.shift);
                }
                ensure_file_cursor_visible(world, target, view, height);
            }
        }
        _ => {}
    }
}

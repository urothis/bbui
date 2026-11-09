use super::*;

pub(super) fn click_file_browser(
    world: &mut World,
    target: Entity,
    position: [f32; 2],
    click_count: u32,
    window: Entity,
    emitted: &mut Vec<WidgetEvent>,
) {
    let Some(layout) = world.get::<UiLayoutResult>(target).copied() else {
        return;
    };
    let (view, spacing, header_height, row_height) = file_browser_metrics(world, target);
    if layout.window != window {
        return;
    }
    let local_y = position[1] - layout.rect.y;
    if local_y < header_height * 0.5 {
        let breadcrumbs = world
            .get::<FileBrowser>(target)
            .map(|browser| browser.breadcrumbs())
            .unwrap_or_default();
        let mut x = layout.rect.x + spacing.padding_x;
        let mut destination = None;
        for crumb in breadcrumbs {
            let width = (crumb.name.chars().count() as f32 * 8.0 + 18.0).max(32.0);
            if position[0] >= x && position[0] <= x + width {
                destination = Some(crumb.path);
                break;
            }
            x += width;
        }
        if let Some(destination) = destination {
            navigate_file_browser(world, target, destination, emitted);
        }
        return;
    }
    if local_y < header_height {
        let local_x = position[0] - layout.rect.x;
        let theme = world.get_resource::<Theme>().cloned().unwrap_or_default();
        let view_switch_width = theme.control_height * 3.0;
        let sort_control_width = theme.control_height * 3.25;
        if local_x < view_switch_width {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                browser.view = if browser.view == FileView::List {
                    FileView::Grid
                } else {
                    FileView::List
                };
            }
            emit_file_browser_commit(world, target, before, emitted);
            return;
        }
        if local_x < view_switch_width + sort_control_width {
            let before = file_browser_snapshot(world, target);
            if let Some(mut browser) = world.get_mut::<FileBrowser>(target) {
                let sort = browser.sort();
                let ascending = if browser.sort() == FileSort::Name {
                    !browser.ascending()
                } else {
                    true
                };
                browser.set_sort(sort, ascending);
            }
            emit_file_browser_commit(world, target, before, emitted);
            return;
        }
        let bookmarks = world
            .get::<FileBrowser>(target)
            .map(|browser| browser.bookmarks.clone())
            .unwrap_or_default();
        let mut x = view_switch_width + sort_control_width + theme.spacing.control.gap * 1.5;
        for bookmark in bookmarks {
            let width = (bookmark
                .file_name()
                .map_or(8, |name| name.to_string_lossy().chars().count())
                as f32
                * 8.0
                + 14.0)
                .max(36.0);
            if local_x >= x && local_x <= x + width {
                navigate_file_browser(world, target, bookmark, emitted);
                return;
            }
            x += width;
        }
        return;
    }
    let Some(browser_view) = world.get::<FileBrowser>(target).map(|browser| browser.view) else {
        return;
    };
    let row_index = match browser_view {
        FileView::List => {
            view.scroll_rows + ((local_y - header_height).max(0.0) / row_height).floor() as usize
        }
        FileView::Grid => {
            let cell_stride = spacing.grid_cell_width + spacing.grid_gap;
            let columns = (layout.rect.width / cell_stride).floor().max(1.0) as usize;
            let row_stride = spacing.grid_cell_height + spacing.grid_gap;
            let col = ((position[0] - layout.rect.x) / cell_stride)
                .floor()
                .max(0.0) as usize;
            let row = ((local_y - header_height).max(0.0) / row_stride).floor() as usize;
            (view.scroll_rows + row) * columns + col
        }
    };
    let Some(item) = world.get::<FileBrowser>(target).and_then(|browser| {
        browser
            .visible_entries()
            .get(row_index)
            .map(|entry| FileBrowserItem {
                owner: target,
                path: entry.path.clone(),
                name: entry.name.clone(),
                index: row_index,
                directory: entry.directory,
                size: entry.size,
                selected: browser.selected.contains(&entry.path),
                preview: entry.preview,
            })
    }) else {
        return;
    };
    select_file_browser_item(world, &item, click_count, window, emitted);
}

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "bbui-file-browser-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        // FileBrowser stores canonical paths, which can differ from the
        // temp_dir spelling on macOS (`/var` versus `/private/var`).
        Self(fs::canonicalize(path).unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn confined_browser_cannot_leave_root_or_transfer_into_another_directory() {
    let scratch = Scratch::new();
    let child = scratch.0.join("inside");
    fs::create_dir(&child).unwrap();
    fs::write(scratch.0.join("safe.txt"), b"safe").unwrap();
    let outside = Scratch::new();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    browser.confine_to(&scratch.0).unwrap();
    assert_eq!(browser.breadcrumbs().len(), 1);
    assert!(!browser.up().unwrap());
    assert!(browser.navigate(&outside.0).is_err());
    assert!(browser.bookmark(&outside.0).is_err());
    browser.select(1, false, false);
    assert!(browser.copy_selected_to(&outside.0).is_err());
    assert!(browser.move_selected_to(&outside.0).is_err());
    browser.navigate(&child).unwrap();
    assert!(browser.up().unwrap());
    assert_eq!(browser.directory, scratch.0);
}

#[test]
fn indexed_visible_ranges_scale_to_large_snapshots_and_follow_filter_sort_changes() {
    let scratch = Scratch::new();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    browser.entries = (0..10_000)
        .map(|index| FileEntry {
            path: scratch.0.join(format!("item-{index:05}.bin")),
            name: format!("item-{index:05}.bin"),
            directory: false,
            size: index as u64,
            modified: None,
            preview: PreviewKind::Other,
        })
        .collect();
    browser.entries[9_000].directory = true;
    browser.entries[9_000].name = "wanted-directory".to_owned();
    browser.rebuild_visible_index();

    assert_eq!(browser.visible_count(), 10_000);
    assert_eq!(
        browser
            .visible_range(5_000, 3)
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["item-05000.bin", "item-05001.bin", "item-05002.bin"]
    );

    browser.set_filter("item-0099");
    assert_eq!(browser.visible_count(), 11);
    assert_eq!(browser.visible_range(0, 2)[0].name, "item-00990.bin");
    browser.set_sort(FileSort::Size, false);
    assert_eq!(browser.visible_range(0, 1)[0].name, "wanted-directory");
    assert_eq!(browser.visible_range(1, 1)[0].name, "item-00999.bin");

    // Internal direct edits fall back to filtering until the private index is rebuilt.
    browser.filter = "wanted".to_owned();
    assert_eq!(browser.visible_count(), 1);
    browser.rebuild_visible_index();
    assert_eq!(browser.visible_range(0, 1)[0].name, "wanted-directory");
}

#[test]
fn refresh_reindexes_same_length_entry_replacements() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("match-old.txt"), b"old").unwrap();
    fs::write(scratch.0.join("other.txt"), b"other").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    browser.set_filter("match");
    assert_eq!(browser.visible_count(), 1);

    fs::remove_file(scratch.0.join("match-old.txt")).unwrap();
    fs::write(scratch.0.join("match-new.txt"), b"new").unwrap();
    browser.refresh().unwrap();

    assert_eq!(browser.entries.len(), 2);
    assert_eq!(browser.visible_count(), 1);
    assert_eq!(browser.visible_range(0, 1)[0].name, "match-new.txt");
}

#[test]
fn navigation_filter_sort_bookmarks_and_keyboard_selection_keep_path_identity() {
    let scratch = Scratch::new();
    fs::create_dir(scratch.0.join("Assets")).unwrap();
    fs::write(scratch.0.join("cube.txt"), b"cube").unwrap();
    fs::write(scratch.0.join("image.png"), b"image bytes").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    assert_eq!(browser.entries[0].name, "Assets");
    assert_eq!(browser.entries[2].preview, PreviewKind::Image);
    assert!(browser.bookmark(&scratch.0).unwrap());
    assert!(!browser.bookmark(&scratch.0).unwrap());
    assert_eq!(browser.breadcrumbs().last().unwrap().path, scratch.0);

    browser.set_filter("cube");
    assert_eq!(browser.visible_count(), 2); // Directories remain navigable while filtering.
    assert!(browser.select(1, false, false));
    assert!(browser.selected.contains(&scratch.0.join("cube.txt")));
    browser.set_sort(FileSort::Size, false);
    assert!(browser.selected.contains(&scratch.0.join("cube.txt")));
    assert_eq!(
        browser.open_cursor().unwrap(),
        Some(scratch.0.join("cube.txt"))
    );
    browser.set_filter("");
    browser.move_cursor(-1, true);
    assert_eq!(browser.selected.len(), 2);

    browser.navigate(scratch.0.join("Assets")).unwrap();
    assert!(browser.selected.is_empty());
    assert!(browser.up().unwrap());
    assert_eq!(browser.directory, scratch.0);
    let before = browser.entries.len();
    let missing = scratch.0.join("missing");
    let error = browser.navigate(&missing).unwrap_err();
    assert_eq!(error.operation, "open directory");
    assert_eq!(error.path, missing);
    assert_eq!(error.source.kind(), io::ErrorKind::NotFound);
    assert_eq!(browser.directory, scratch.0);
    assert_eq!(browser.entries.len(), before);
    assert!(browser.remove_bookmark(&scratch.0));
    assert!(!browser.remove_bookmark(&scratch.0));
}

#[test]
fn ui_state_snapshot_restores_selection_filter_view_and_reloads_entries() {
    let scratch = Scratch::new();
    let nested = scratch.0.join("Assets");
    fs::create_dir(&nested).unwrap();
    fs::write(scratch.0.join("cube.txt"), b"cube").unwrap();
    fs::write(scratch.0.join("notes.md"), b"notes").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    assert!(browser.bookmark(&scratch.0).unwrap());
    assert!(browser.select(1, false, false));
    browser.set_filter("cube");
    browser.set_sort(FileSort::Size, false);
    browser.view = FileView::Grid;
    let snapshot = browser.snapshot_json().unwrap();

    browser.navigate(&nested).unwrap();
    fs::write(nested.join("new.txt"), b"created after the snapshot").unwrap();
    browser.restore_json(&snapshot).unwrap();

    assert_eq!(browser.directory, scratch.0);
    assert_eq!(
        browser.bookmarks.as_slice(),
        std::slice::from_ref(&scratch.0)
    );
    assert_eq!(browser.filter, "cube");
    assert_eq!(browser.sort, FileSort::Size);
    assert!(!browser.ascending);
    assert_eq!(browser.view, FileView::Grid);
    assert_eq!(browser.selected, [scratch.0.join("cube.txt")].into());
    assert_eq!(
        browser.cursor.as_deref(),
        Some(scratch.0.join("cube.txt").as_path())
    );
    assert_eq!(browser.visible_count(), 2);
    assert!(browser.entries.iter().any(|entry| entry.name == "notes.md"));

    fs::remove_file(scratch.0.join("cube.txt")).unwrap();
    browser.restore_json(&snapshot).unwrap();
    assert!(browser.selected.is_empty());
    assert_eq!(browser.cursor, None);
    assert_eq!(browser.visible_count(), 1);
}

#[test]
fn invalid_or_missing_directory_snapshot_leaves_browser_unchanged() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("keep.txt"), b"keep").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    browser.set_filter("keep");
    let before = browser.snapshot_json().unwrap();
    let missing = scratch.0.join("removed");
    let invalid = serde_json::json!({
        "version": 1,
        "directory": missing,
        "bookmarks": [],
        "filter": "changed",
        "sort": "Name",
        "ascending": true,
        "view": "List",
        "selected": [],
        "cursor": null,
        "anchor": null
    });

    let error = browser.restore_json(&invalid.to_string()).unwrap_err();
    assert_eq!(error.operation, "open directory");
    assert_eq!(browser.snapshot_json().unwrap(), before);
    assert!(browser.restore_json("not json").is_err());
    assert_eq!(browser.snapshot_json().unwrap(), before);
}

#[test]
fn first_keyboard_move_starts_at_the_edge_in_both_directions() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("a.txt"), b"a").unwrap();
    fs::write(scratch.0.join("b.txt"), b"b").unwrap();
    fs::write(scratch.0.join("c.txt"), b"c").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    assert_eq!(browser.move_cursor(1, false).unwrap().name, "a.txt");
    assert_eq!(browser.move_cursor(1, false).unwrap().name, "b.txt");
    browser.cursor = None;
    assert_eq!(browser.move_cursor(-1, false).unwrap().name, "c.txt");
}

#[test]
fn keyboard_range_selection_keeps_its_starting_anchor() {
    let scratch = Scratch::new();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(scratch.0.join(name), name).unwrap();
    }
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    browser.move_cursor(1, false);
    browser.anchor = None;

    assert_eq!(browser.move_cursor(1, true).unwrap().name, "b.txt");
    assert_eq!(browser.selected.len(), 2);
    assert_eq!(browser.move_cursor(1, true).unwrap().name, "c.txt");
    assert_eq!(browser.selected.len(), 3);
    assert_eq!(
        browser.anchor.as_deref(),
        Some(scratch.0.join("a.txt").as_path())
    );

    assert_eq!(browser.move_cursor(-1, true).unwrap().name, "b.txt");
    assert_eq!(browser.selected.len(), 2);
    assert!(browser.selected.contains(&scratch.0.join("a.txt")));
    assert!(browser.selected.contains(&scratch.0.join("b.txt")));
    assert!(!browser.selected.contains(&scratch.0.join("c.txt")));
}

#[test]
fn select_all_uses_filtered_visible_entries_and_sets_a_keyboard_anchor() {
    let scratch = Scratch::new();
    fs::create_dir(scratch.0.join("Assets")).unwrap();
    fs::write(scratch.0.join("a.txt"), b"a").unwrap();
    fs::write(scratch.0.join("b.txt"), b"b").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    browser.set_filter("b.txt");
    assert!(browser.select_all_visible());
    assert_eq!(
        browser.selected,
        BTreeSet::from([scratch.0.join("Assets"), scratch.0.join("b.txt")])
    );
    assert_eq!(browser.cursor, Some(scratch.0.join("Assets")));
    assert_eq!(browser.anchor, browser.cursor);
    assert!(!browser.select_all_visible());
}

#[test]
fn file_operations_are_real_and_reject_overwrites_and_path_traversal() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("Destination");
    fs::create_dir(&destination).unwrap();
    fs::write(scratch.0.join("before.txt"), b"payload").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    assert!(browser.create_directory("../escape").is_err());
    let created = browser.create_directory("New Folder").unwrap();
    assert!(created.is_dir());
    let renamed = browser
        .rename(scratch.0.join("before.txt"), "after.txt")
        .unwrap();
    assert_eq!(fs::read(&renamed).unwrap(), b"payload");
    assert!(browser.rename(&renamed, "New Folder").is_err());
    let index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == renamed)
        .unwrap();
    browser.select(index, false, false);
    let copied = browser.copy_selected_to(&destination).unwrap();
    assert_eq!(fs::read(&copied[0]).unwrap(), b"payload");
    assert!(browser.copy_selected_to(&destination).is_err());
    assert_eq!(browser.delete_selected().unwrap(), 1);
    assert!(!renamed.exists());
    assert!(destination.join("after.txt").exists());
}

#[test]
fn delete_failure_refreshes_snapshot_after_partial_success() {
    let scratch = Scratch::new();
    let first = scratch.0.join("a-delete.txt");
    let second = scratch.0.join("b-removed-externally.txt");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    let first_index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == first)
        .unwrap();
    let second_index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == second)
        .unwrap();
    browser.select(first_index, false, false);
    browser.select(second_index, true, false);

    fs::remove_file(&second).unwrap();
    let error = browser.delete_selected().unwrap_err();
    assert_eq!(error.operation, "read metadata");
    assert_eq!(error.path, second);
    assert!(error.to_string().contains("1 earlier item(s) were deleted"));
    assert!(!first.exists());
    assert!(!browser.entries.iter().any(|entry| entry.path == first));
    assert!(!browser.entries.iter().any(|entry| entry.path == second));
    assert!(browser.selected.is_empty());
}

#[test]
fn failed_multi_move_rolls_back_prior_entries_and_refreshes_snapshot() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("Destination");
    let first = scratch.0.join("a-first.txt");
    let second = scratch.0.join("b-removed-externally.txt");
    fs::create_dir(&destination).unwrap();
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    let first_index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == first)
        .unwrap();
    let second_index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == second)
        .unwrap();
    browser.select(first_index, false, false);
    browser.select(second_index, true, false);

    fs::remove_file(&second).unwrap();
    let error = browser.move_selected_to(&destination).unwrap_err();
    assert_eq!(error.operation, "move");
    assert_eq!(error.path, second);
    assert!(first.exists());
    assert!(!destination.join("a-first.txt").exists());
    assert!(!browser.entries.iter().any(|entry| entry.path == second));
    assert!(browser.entries.iter().any(|entry| entry.path == first));
}

#[test]
fn sorting_keeps_directories_first_in_both_directions_and_path_selection_stable() {
    let scratch = Scratch::new();
    fs::create_dir(scratch.0.join("z-folder")).unwrap();
    fs::write(scratch.0.join("a.txt"), b"a").unwrap();
    fs::write(scratch.0.join("b.txt"), b"bb").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    let selected_index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.name == "a.txt")
        .unwrap();
    browser.select(selected_index, false, false);
    browser.set_sort(FileSort::Name, false);
    assert_eq!(
        browser
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["z-folder", "b.txt", "a.txt"]
    );
    assert!(browser.selected.contains(&scratch.0.join("a.txt")));
    browser.set_sort(FileSort::Name, true);
    assert_eq!(browser.entries[0].name, "z-folder");
    assert!(browser.selected.contains(&scratch.0.join("a.txt")));
}

#[test]
fn directory_copy_and_move_preserve_contents_without_recursive_self_copy() {
    let scratch = Scratch::new();
    let source = scratch.0.join("Source");
    let copy_target = scratch.0.join("Copies");
    let move_target = scratch.0.join("Moved");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&copy_target).unwrap();
    fs::create_dir(&move_target).unwrap();
    fs::write(source.join("nested.txt"), b"nested").unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    let index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == source)
        .unwrap();
    browser.select(index, false, false);
    assert!(browser.copy_selected_to(&source).is_err());
    browser.copy_selected_to(&copy_target).unwrap();
    assert_eq!(
        fs::read(copy_target.join("Source/nested.txt")).unwrap(),
        b"nested"
    );
    browser.move_selected_to(&move_target).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read(move_target.join("Source/nested.txt")).unwrap(),
        b"nested"
    );
    assert!(browser.selected.is_empty());
}

#[test]
fn preview_types_match_media_asset_families() {
    assert_eq!(
        preview_kind(Path::new("photo.PNG"), false),
        PreviewKind::Image
    );
    assert_eq!(
        preview_kind(Path::new("animated.gif"), false),
        PreviewKind::AnimatedImage
    );
    assert_eq!(preview_kind(Path::new("icon.svg"), false), PreviewKind::Svg);
    assert_eq!(
        preview_kind(Path::new("movie.webm"), false),
        PreviewKind::Video
    );
}

#[cfg(unix)]
#[test]
fn deleting_a_directory_symlink_removes_only_the_link() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new();
    let target = scratch.0.join("Target");
    let link = scratch.0.join("Link");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), b"safe").unwrap();
    symlink(&target, &link).unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    let index = browser
        .visible_entries()
        .iter()
        .position(|entry| entry.path == link)
        .unwrap();
    browser.select(index, false, false);
    assert!(browser.copy_selected_to(&target).is_err());
    assert_eq!(browser.delete_selected().unwrap(), 1);
    assert!(!link.exists());
    assert_eq!(fs::read(target.join("keep.txt")).unwrap(), b"safe");
}

#[cfg(unix)]
#[test]
fn nested_symlink_copy_fails_and_rolls_back_the_selection() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new();
    let destination = scratch.0.join("Destination");
    let directory = scratch.0.join("BFolder");
    fs::create_dir(&destination).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(scratch.0.join("A.txt"), b"first").unwrap();
    fs::write(scratch.0.join("outside.txt"), b"secret").unwrap();
    symlink(scratch.0.join("outside.txt"), directory.join("linked.txt")).unwrap();
    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    for name in ["A.txt", "BFolder"] {
        let index = browser
            .visible_entries()
            .iter()
            .position(|entry| entry.name == name)
            .unwrap();
        browser.select(index, true, false);
    }
    let error = browser.copy_selected_to(&destination).unwrap_err();
    assert_eq!(error.source.kind(), io::ErrorKind::Unsupported);
    assert!(!destination.join("A.txt").exists());
    assert!(!destination.join("BFolder").exists());
    assert_eq!(fs::read(scratch.0.join("outside.txt")).unwrap(), b"secret");
}

#[cfg(unix)]
#[test]
fn copy_rollback_removes_previously_copied_read_only_directories() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let scratch = Scratch::new();
    let destination = scratch.0.join("Destination");
    let locked = scratch.0.join("A-Locked");
    let failing = scratch.0.join("B-Failing");
    let outside = scratch.0.join("outside.txt");
    fs::create_dir(&destination).unwrap();
    fs::create_dir(&locked).unwrap();
    fs::create_dir(&failing).unwrap();
    fs::write(locked.join("keep.txt"), b"keep").unwrap();
    fs::write(&outside, b"outside").unwrap();
    symlink(&outside, failing.join("linked.txt")).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).unwrap();

    let mut browser = FileBrowser::new(&scratch.0).unwrap();
    for name in ["A-Locked", "B-Failing"] {
        let index = browser
            .visible_entries()
            .iter()
            .position(|entry| entry.name == name)
            .unwrap();
        browser.select(index, true, false);
    }
    let error = browser.copy_selected_to(&destination).unwrap_err();
    assert_eq!(error.source.kind(), io::ErrorKind::Unsupported);
    assert!(!destination.join("A-Locked").exists());
    assert!(!destination.join("B-Failing").exists());
    assert_eq!(fs::read(&outside).unwrap(), b"outside");

    // Restore permissions so the scratch directory cleanup can remove the test fixture.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn rollback_cleanup_handles_read_only_files_and_nested_directories() {
    let scratch = Scratch::new();
    let copied = scratch.0.join("copied");
    let nested = copied.join("nested");
    let file = nested.join("read-only.txt");
    fs::create_dir_all(&nested).unwrap();
    fs::write(&file, b"temporary copy").unwrap();

    let mut file_permissions = fs::metadata(&file).unwrap().permissions();
    file_permissions.set_readonly(true);
    fs::set_permissions(&file, file_permissions).unwrap();
    let mut directory_permissions = fs::metadata(&nested).unwrap().permissions();
    directory_permissions.set_readonly(true);
    fs::set_permissions(&nested, directory_permissions).unwrap();

    remove_copied_path(&copied).unwrap();
    assert!(!copied.exists());
}

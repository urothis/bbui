//! Filesystem-backed browser state shared by list and grid presentations.
//!
//! Directory reads and mutating operations return [`FileBrowserError`] with an operation, path,
//! and underlying [`io::Error`]. File copies reject symbolic links to avoid following a link out
//! of the selected tree; moving and deleting a selected symlink operate on the link itself.

use std::{
    cmp::Reverse,
    collections::{BTreeSet, HashMap},
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileView {
    List,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileSort {
    Name,
    Kind,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreviewKind {
    Directory,
    Image,
    Svg,
    AnimatedImage,
    Video,
    Text,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub directory: bool,
    pub size: u64,
    /// Modification time when the filesystem reports one.
    pub modified: Option<SystemTime>,
    pub preview: PreviewKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breadcrumb {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct FileBrowserError {
    /// Filesystem operation that failed.
    pub operation: &'static str,
    /// Path associated with the failure.
    pub path: PathBuf,
    /// Original filesystem error.
    pub source: io::Error,
}

impl std::fmt::Display for FileBrowserError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {}: {}",
            self.operation,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for FileBrowserError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

type BrowserResult<T> = Result<T, FileBrowserError>;

fn failure(operation: &'static str, path: &Path, source: io::Error) -> FileBrowserError {
    FileBrowserError {
        operation,
        path: path.to_owned(),
        source,
    }
}

/// Persistent browser state. `entries` are the current directory snapshot; call `refresh` after
/// external filesystem changes. Read the snapshot and active filter/sort through accessors, and
/// use `set_filter`, `set_sort`, navigation, and refresh methods to keep its virtual index current.
/// Selection and keyboard cursor use paths, so sorting does not change object identity.
#[derive(Component, Debug, Clone)]
#[component(summary_tick)]
pub struct FileBrowser {
    pub directory: PathBuf,
    entries: Vec<FileEntry>,
    pub bookmarks: Vec<PathBuf>,
    filter: String,
    sort: FileSort,
    ascending: bool,
    pub view: FileView,
    pub selected: BTreeSet<PathBuf>,
    pub cursor: Option<PathBuf>,
    anchor: Option<PathBuf>,
    visible_indices: Vec<usize>,
    visible_positions: HashMap<PathBuf, usize>,
    visible_filter: String,
    indexed_entries_len: usize,
}

impl FileBrowser {
    pub fn new(directory: impl AsRef<Path>) -> BrowserResult<Self> {
        let directory = canonical_directory(directory.as_ref())?;
        let entries = read_entries(&directory)?;
        let mut browser = Self {
            directory,
            entries,
            bookmarks: Vec::new(),
            filter: String::new(),
            sort: FileSort::Name,
            ascending: true,
            view: FileView::List,
            selected: BTreeSet::new(),
            cursor: None,
            anchor: None,
            visible_indices: Vec::new(),
            visible_positions: HashMap::new(),
            visible_filter: String::new(),
            indexed_entries_len: 0,
        };
        browser.sort_entries();
        Ok(browser)
    }

    /// Current filesystem snapshot, in the active sort order.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Current case-insensitive name filter.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Active entry sort key.
    pub fn sort(&self) -> FileSort {
        self.sort
    }

    /// Whether the active sort order is ascending.
    pub fn ascending(&self) -> bool {
        self.ascending
    }

    pub fn navigate(&mut self, directory: impl AsRef<Path>) -> BrowserResult<()> {
        let next = canonical_directory(directory.as_ref())?;
        let mut entries = read_entries(&next)?;
        sort_entries(&mut entries, self.sort, self.ascending);
        self.directory = next;
        self.entries = entries;
        self.selected.clear();
        self.cursor = None;
        self.anchor = None;
        self.rebuild_visible_index();
        Ok(())
    }

    pub fn up(&mut self) -> BrowserResult<bool> {
        let Some(parent) = self.directory.parent().map(Path::to_path_buf) else {
            return Ok(false);
        };
        if parent == self.directory {
            return Ok(false);
        }
        self.navigate(parent)?;
        Ok(true)
    }

    pub fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        let mut pieces = Vec::new();
        for path in self.directory.ancestors() {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            pieces.push(Breadcrumb {
                name,
                path: path.to_owned(),
            });
        }
        pieces.reverse();
        pieces
    }

    pub fn bookmark(&mut self, directory: impl AsRef<Path>) -> BrowserResult<bool> {
        let path = canonical_directory(directory.as_ref())?;
        if self.bookmarks.contains(&path) {
            return Ok(false);
        }
        self.bookmarks.push(path);
        Ok(true)
    }

    /// Remove a bookmark by path, even when its directory has since disappeared.
    pub fn remove_bookmark(&mut self, directory: impl AsRef<Path>) -> bool {
        let path =
            fs::canonicalize(directory.as_ref()).unwrap_or_else(|_| directory.as_ref().to_owned());
        let before = self.bookmarks.len();
        self.bookmarks.retain(|bookmark| bookmark != &path);
        self.bookmarks.len() != before
    }

    pub fn set_filter(&mut self, filter: impl Into<String>) {
        let filter = filter.into();
        if self.filter == filter && self.visible_index_is_current() {
            return;
        }
        self.filter = filter;
        self.rebuild_visible_index();
        self.retain_visible_selection();
    }

    pub fn set_sort(&mut self, sort: FileSort, ascending: bool) {
        if self.sort == sort && self.ascending == ascending {
            return;
        }
        self.sort = sort;
        self.ascending = ascending;
        self.sort_entries();
    }

    /// Return entries matching the current filter. Directories remain visible so they can still
    /// be opened while searching for files.
    pub fn visible_entries(&self) -> Vec<&FileEntry> {
        if self.visible_index_is_current() {
            self.visible_indices
                .iter()
                .filter_map(|index| self.entries.get(*index))
                .collect()
        } else {
            let query = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|entry| entry_matches_filter(entry, &query))
                .collect()
        }
    }

    pub fn visible_count(&self) -> usize {
        if self.visible_index_is_current() {
            self.visible_indices.len()
        } else {
            let query = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|entry| entry_matches_filter(entry, &query))
                .count()
        }
    }

    /// Return only the rows needed by a virtualized list/grid viewport.
    pub fn visible_range(&self, start: usize, count: usize) -> Vec<&FileEntry> {
        if self.visible_index_is_current() {
            let end = start.saturating_add(count).min(self.visible_indices.len());
            return self
                .visible_indices
                .get(start..end)
                .unwrap_or_default()
                .iter()
                .filter_map(|index| self.entries.get(*index))
                .collect();
        }
        let query = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry_matches_filter(entry, &query))
            .skip(start)
            .take(count)
            .collect()
    }

    /// Select a visible item. Shift extends from the last anchor; Ctrl toggles membership.
    pub fn select(&mut self, index: usize, control: bool, shift: bool) -> bool {
        let Some(path) = self
            .visible_range(index, 1)
            .first()
            .map(|entry| entry.path.clone())
        else {
            return false;
        };
        if shift {
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|anchor| self.visible_index_of(anchor))
                .or_else(|| {
                    self.cursor
                        .as_ref()
                        .and_then(|cursor| self.visible_index_of(cursor))
                })
                .unwrap_or(index);
            let anchor_path = self
                .visible_range(anchor, 1)
                .first()
                .map(|entry| entry.path.clone())
                .unwrap_or_else(|| path.clone());
            self.anchor = Some(anchor_path);
            if !control {
                self.selected.clear();
            }
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            let selected_paths: Vec<_> = self
                .visible_range(start, end - start + 1)
                .into_iter()
                .map(|entry| entry.path.clone())
                .collect();
            self.selected.extend(selected_paths);
        } else if control {
            if !self.selected.insert(path.clone()) {
                self.selected.remove(&path);
            }
            self.anchor = Some(path.clone());
        } else {
            self.selected.clear();
            self.selected.insert(path.clone());
            self.anchor = Some(path.clone());
        }
        self.cursor = Some(path);
        true
    }

    /// Select every currently visible entry and keep a valid keyboard cursor/anchor.
    pub fn select_all_visible(&mut self) -> bool {
        let query = self.filter.to_lowercase();
        let mut selected = BTreeSet::new();
        let mut first_visible = None;
        for entry in &self.entries {
            if entry_matches_filter(entry, &query) {
                first_visible.get_or_insert_with(|| entry.path.clone());
                selected.insert(entry.path.clone());
            }
        }
        let Some(first_visible) = first_visible else {
            let changed =
                !self.selected.is_empty() || self.cursor.is_some() || self.anchor.is_some();
            self.selected.clear();
            self.cursor = None;
            self.anchor = None;
            return changed;
        };
        let old_selected = self.selected.clone();
        let old_cursor = self.cursor.clone();
        let old_anchor = self.anchor.clone();
        let cursor = self
            .cursor
            .clone()
            .filter(|path| selected.contains(path))
            .unwrap_or(first_visible);
        self.selected = selected;
        self.cursor = Some(cursor.clone());
        self.anchor = Some(cursor);
        old_selected != self.selected || old_cursor != self.cursor || old_anchor != self.anchor
    }

    pub fn move_cursor(&mut self, delta: i32, extend_selection: bool) -> Option<&FileEntry> {
        let visible_count = self.visible_count();
        if visible_count == 0 {
            self.cursor = None;
            return None;
        }
        let next = self
            .cursor
            .as_ref()
            .and_then(|path| self.visible_index_of(path))
            .map_or_else(
                || if delta < 0 { visible_count - 1 } else { 0 },
                |current| {
                    (current as i64 + i64::from(delta)).clamp(0, visible_count as i64 - 1) as usize
                },
            );
        self.select(next, false, extend_selection);
        self.visible_range(next, 1).first().copied()
    }

    /// Enter the directory under the keyboard cursor, or return a chosen file path.
    pub fn open_cursor(&mut self) -> BrowserResult<Option<PathBuf>> {
        let Some(path) = self.cursor.clone() else {
            return Ok(None);
        };
        let Some(entry) = self.entries.iter().find(|entry| entry.path == path) else {
            return Ok(None);
        };
        if entry.directory {
            self.navigate(path)?;
            Ok(None)
        } else {
            Ok(Some(path))
        }
    }

    pub fn refresh(&mut self) -> BrowserResult<()> {
        let mut entries = read_entries(&self.directory)?;
        sort_entries(&mut entries, self.sort, self.ascending);
        self.entries = entries;
        self.rebuild_visible_index();
        self.retain_visible_selection();
        Ok(())
    }

    /// Serialize the reversible part of the browser state for an undo transaction.
    /// Filesystem entries are intentionally omitted and re-read when restored.
    pub fn snapshot_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&FileBrowserSnapshot {
            version: 1,
            directory: self.directory.clone(),
            bookmarks: self.bookmarks.clone(),
            filter: self.filter.clone(),
            sort: self.sort,
            ascending: self.ascending,
            view: self.view,
            selected: self.selected.clone(),
            cursor: self.cursor.clone(),
            anchor: self.anchor.clone(),
        })
    }

    /// Restore UI state and refresh directory entries from disk. The operation validates the
    /// target directory before changing any state; selections/cursor/anchor that no longer refer
    /// to visible entries are discarded, while bookmarks may point to missing directories.
    pub fn restore_json(&mut self, json: &str) -> BrowserResult<()> {
        let snapshot: FileBrowserSnapshot = serde_json::from_str(json).map_err(|error| {
            failure(
                "restore browser state",
                &self.directory,
                io::Error::new(io::ErrorKind::InvalidData, error),
            )
        })?;
        if snapshot.version != 1 {
            return Err(failure(
                "restore browser state",
                &snapshot.directory,
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported browser snapshot version {}", snapshot.version),
                ),
            ));
        }
        let directory = canonical_directory(&snapshot.directory)?;
        let mut entries = read_entries(&directory)?;
        sort_entries(&mut entries, snapshot.sort, snapshot.ascending);
        let visible_paths: BTreeSet<_> = entries
            .iter()
            .filter(|entry| entry_matches_filter(entry, &snapshot.filter.to_lowercase()))
            .map(|entry| entry.path.clone())
            .collect();
        let selected = snapshot
            .selected
            .into_iter()
            .filter(|path| visible_paths.contains(path))
            .collect();
        let cursor = snapshot.cursor.filter(|path| visible_paths.contains(path));
        let anchor = snapshot.anchor.filter(|path| visible_paths.contains(path));

        self.directory = directory;
        self.entries = entries;
        self.bookmarks = snapshot.bookmarks;
        self.filter = snapshot.filter;
        self.sort = snapshot.sort;
        self.ascending = snapshot.ascending;
        self.view = snapshot.view;
        self.selected = selected;
        self.cursor = cursor;
        self.anchor = anchor;
        self.rebuild_visible_index();
        Ok(())
    }

    pub fn create_directory(&mut self, name: &str) -> BrowserResult<PathBuf> {
        let destination = child_path(&self.directory, name)?;
        fs::create_dir(&destination)
            .map_err(|error| failure("create directory", &destination, error))?;
        self.refresh()?;
        Ok(destination)
    }

    pub fn rename(&mut self, path: impl AsRef<Path>, new_name: &str) -> BrowserResult<PathBuf> {
        let source = self.require_entry(path.as_ref())?;
        let destination = child_path(&self.directory, new_name)?;
        ensure_absent(&destination, "rename")?;
        fs::rename(&source, &destination).map_err(|error| failure("rename", &source, error))?;
        if self.selected.remove(&source) {
            self.selected.insert(destination.clone());
        }
        if self.cursor.as_ref() == Some(&source) {
            self.cursor = Some(destination.clone());
        }
        if self.anchor.as_ref() == Some(&source) {
            self.anchor = Some(destination.clone());
        }
        self.refresh()?;
        Ok(destination)
    }

    pub fn copy_selected_to(
        &mut self,
        destination_directory: impl AsRef<Path>,
    ) -> BrowserResult<Vec<PathBuf>> {
        let directory = canonical_directory(destination_directory.as_ref())?;
        let plan = self.plan_selected_transfer(&directory, "copy")?;
        let mut created: Vec<PathBuf> = Vec::new();
        for (source, destination) in plan {
            let result = if source.is_dir() {
                copy_directory(&source, &destination)
            } else {
                copy_file(&source, &destination)
            };
            if let Err(error) = result {
                let mut rollback_failure = None;
                for path in created.iter().rev() {
                    if let Err(rollback_error) = remove_copied_path(path) {
                        rollback_failure.get_or_insert_with(|| (path.clone(), rollback_error));
                    }
                }
                if let Some((path, rollback_error)) = rollback_failure {
                    return Err(failure(
                        "rollback copy",
                        &path,
                        io::Error::new(
                            rollback_error.kind(),
                            format!(
                                "{error}; cleanup of {} failed: {rollback_error}",
                                path.display()
                            ),
                        ),
                    ));
                }
                return Err(error);
            }
            created.push(destination);
        }
        Ok(created)
    }

    pub fn move_selected_to(
        &mut self,
        destination_directory: impl AsRef<Path>,
    ) -> BrowserResult<Vec<PathBuf>> {
        let directory = canonical_directory(destination_directory.as_ref())?;
        let plan = self.plan_selected_transfer(&directory, "move")?;
        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        for (source, destination) in plan {
            if let Err(error) = fs::rename(&source, &destination) {
                let mut rollback_failure = None;
                for (original, relocated) in moved.iter().rev() {
                    if let Err(rollback_error) = fs::rename(relocated, original) {
                        rollback_failure.get_or_insert_with(|| {
                            (original.clone(), relocated.clone(), rollback_error)
                        });
                    }
                }
                let _ = self.refresh();
                if let Some((original, relocated, rollback_error)) = rollback_failure {
                    return Err(failure(
                        "rollback move",
                        &relocated,
                        io::Error::new(
                            rollback_error.kind(),
                            format!(
                                "move of {} failed: {error}; rollback to {} failed: {rollback_error}",
                                source.display(),
                                original.display()
                            ),
                        ),
                    ));
                }
                return Err(failure("move", &source, error));
            }
            moved.push((source, destination));
        }
        self.refresh()?;
        Ok(moved
            .into_iter()
            .map(|(_, destination)| destination)
            .collect())
    }

    /// Delete selected entries and refresh the directory snapshot.
    ///
    /// Filesystem deletion is not atomic across multiple entries. If a later deletion fails,
    /// earlier deletions remain applied; the snapshot is refreshed before returning the error
    /// so stale selections do not remain actionable.
    pub fn delete_selected(&mut self) -> BrowserResult<usize> {
        let paths: Vec<_> = self.selected.iter().cloned().collect();
        let mut deleted = 0;
        for path in paths {
            let source = match self.require_entry(&path) {
                Ok(source) => source,
                Err(error) => return Err(self.refresh_after_delete_failure(error, deleted)),
            };
            let result = fs::symlink_metadata(&source)
                .map_err(|error| failure("read metadata", &source, error))
                .and_then(|metadata| {
                    if metadata.file_type().is_dir() {
                        fs::remove_dir_all(&source)
                            .map_err(|error| failure("delete directory", &source, error))
                    } else {
                        fs::remove_file(&source)
                            .map_err(|error| failure("delete file", &source, error))
                    }
                });
            if let Err(error) = result {
                return Err(self.refresh_after_delete_failure(error, deleted));
            }
            deleted += 1;
        }
        self.refresh()?;
        Ok(deleted)
    }

    fn refresh_after_delete_failure(
        &mut self,
        error: FileBrowserError,
        deleted: usize,
    ) -> FileBrowserError {
        match self.refresh() {
            Ok(()) if deleted == 0 => error,
            Ok(()) => failure(
                error.operation,
                &error.path,
                io::Error::new(
                    error.source.kind(),
                    format!(
                        "{}; {deleted} earlier item(s) were deleted and the directory snapshot was refreshed",
                        error.source
                    ),
                ),
            ),
            Err(refresh_error) => failure(
                "refresh after delete failure",
                &self.directory,
                io::Error::new(
                    refresh_error.source.kind(),
                    format!("{error}; refreshing the directory also failed: {refresh_error}"),
                ),
            ),
        }
    }

    fn require_entry(&self, path: &Path) -> BrowserResult<PathBuf> {
        self.entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.path.clone())
            .ok_or_else(|| {
                failure(
                    "access entry",
                    path,
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "path is not in the current directory snapshot",
                    ),
                )
            })
    }

    fn plan_selected_transfer(
        &self,
        destination_directory: &Path,
        operation: &'static str,
    ) -> BrowserResult<Vec<(PathBuf, PathBuf)>> {
        let mut plan = Vec::with_capacity(self.selected.len());
        for source in &self.selected {
            let source = self.require_entry(source)?;
            let name = source.file_name().ok_or_else(|| {
                failure(
                    operation,
                    &source,
                    io::Error::new(io::ErrorKind::InvalidInput, "missing filename"),
                )
            })?;
            let destination = destination_directory.join(name);
            ensure_absent(&destination, operation)?;
            if operation == "copy"
                && fs::symlink_metadata(&source)
                    .map_err(|error| failure("read metadata", &source, error))?
                    .file_type()
                    .is_symlink()
            {
                return Err(failure(
                    operation,
                    &source,
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "copying symbolic links is not supported",
                    ),
                ));
            }
            if source.is_dir() && destination_directory.starts_with(&source) {
                return Err(failure(
                    operation,
                    &source,
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "destination is inside source directory",
                    ),
                ));
            }
            plan.push((source, destination));
        }
        Ok(plan)
    }

    fn retain_visible_selection(&mut self) {
        let visible: BTreeSet<_> = self
            .visible_entries()
            .into_iter()
            .map(|entry| entry.path.clone())
            .collect();
        self.selected.retain(|path| visible.contains(path));
        if self
            .cursor
            .as_ref()
            .is_some_and(|path| !visible.contains(path))
        {
            self.cursor = None;
        }
        if self
            .anchor
            .as_ref()
            .is_some_and(|path| !visible.contains(path))
        {
            self.anchor = None;
        }
    }

    fn sort_entries(&mut self) {
        sort_entries(&mut self.entries, self.sort, self.ascending);
        self.rebuild_visible_index();
    }

    fn visible_index_of(&self, path: &Path) -> Option<usize> {
        if self.visible_index_is_current() {
            self.visible_positions.get(path).copied()
        } else {
            let query = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|entry| entry_matches_filter(entry, &query))
                .position(|entry| entry.path == path)
        }
    }

    fn visible_index_is_current(&self) -> bool {
        self.visible_filter == self.filter && self.indexed_entries_len == self.entries.len()
    }

    fn rebuild_visible_index(&mut self) {
        let query = self.filter.to_lowercase();
        self.visible_indices.clear();
        self.visible_positions.clear();
        for (index, entry) in self.entries.iter().enumerate() {
            if entry_matches_filter(entry, &query) {
                let visible_index = self.visible_indices.len();
                self.visible_indices.push(index);
                self.visible_positions
                    .insert(entry.path.clone(), visible_index);
            }
        }
        self.visible_filter.clone_from(&self.filter);
        self.indexed_entries_len = self.entries.len();
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileBrowserSnapshot {
    version: u32,
    directory: PathBuf,
    bookmarks: Vec<PathBuf>,
    filter: String,
    sort: FileSort,
    ascending: bool,
    view: FileView,
    selected: BTreeSet<PathBuf>,
    cursor: Option<PathBuf>,
    anchor: Option<PathBuf>,
}

mod filesystem;
use filesystem::*;

#[cfg(test)]
#[path = "file_browser/tests.rs"]
mod tests;

//! Filesystem-backed browser state shared by list and grid presentations.
//!
//! Directory reads and mutating operations return [`FileBrowserError`] with an operation, path,
//! and underlying [`io::Error`]. File copies reject symbolic links to avoid following a link out
//! of the selected tree; moving and deleting a selected symlink operate on the link itself.

use std::{
    cmp::Reverse,
    collections::BTreeSet,
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
/// external filesystem changes. Selection and keyboard cursor use paths, so sorting does not
/// change object identity.
#[derive(Component, Debug, Clone)]
pub struct FileBrowser {
    pub directory: PathBuf,
    pub entries: Vec<FileEntry>,
    pub bookmarks: Vec<PathBuf>,
    pub filter: String,
    pub sort: FileSort,
    pub ascending: bool,
    pub view: FileView,
    pub selected: BTreeSet<PathBuf>,
    pub cursor: Option<PathBuf>,
    anchor: Option<PathBuf>,
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
        };
        browser.sort_entries();
        Ok(browser)
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
        self.filter = filter.into();
        self.retain_visible_selection();
    }

    pub fn set_sort(&mut self, sort: FileSort, ascending: bool) {
        self.sort = sort;
        self.ascending = ascending;
        self.sort_entries();
    }

    /// Return entries matching the current filter. Directories remain visible so they can still
    /// be opened while searching for files.
    pub fn visible_entries(&self) -> Vec<&FileEntry> {
        let query = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry_matches_filter(entry, &query))
            .collect()
    }

    pub fn visible_count(&self) -> usize {
        let query = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry_matches_filter(entry, &query))
            .count()
    }

    /// Return only the rows needed by a virtualized list/grid viewport.
    pub fn visible_range(&self, start: usize, count: usize) -> Vec<&FileEntry> {
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
    }

    fn visible_index_of(&self, path: &Path) -> Option<usize> {
        let query = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| entry_matches_filter(entry, &query))
            .position(|entry| entry.path == path)
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

fn entry_matches_filter(entry: &FileEntry, query: &str) -> bool {
    entry.directory || query.is_empty() || entry.name.to_lowercase().contains(query)
}

fn canonical_directory(path: &Path) -> BrowserResult<PathBuf> {
    let canonical =
        fs::canonicalize(path).map_err(|error| failure("open directory", path, error))?;
    if !canonical.is_dir() {
        return Err(failure(
            "open directory",
            &canonical,
            io::Error::new(io::ErrorKind::NotADirectory, "path is not a directory"),
        ));
    }
    Ok(canonical)
}

fn child_path(parent: &Path, name: &str) -> BrowserResult<PathBuf> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || Path::new(name).components().count() != 1
        || name.contains('/')
        || name.contains('\\')
    {
        return Err(failure(
            "name child",
            parent,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "name must be one nonempty path component",
            ),
        ));
    }
    Ok(parent.join(name))
}

fn ensure_absent(path: &Path, operation: &'static str) -> BrowserResult<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(failure(
            operation,
            path,
            io::Error::new(io::ErrorKind::AlreadyExists, "destination already exists"),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure(operation, path, error)),
    }
}

fn read_entries(directory: &Path) -> BrowserResult<Vec<FileEntry>> {
    let reader =
        fs::read_dir(directory).map_err(|error| failure("list directory", directory, error))?;
    let mut entries = Vec::new();
    for result in reader {
        let entry = result.map_err(|error| failure("read directory entry", directory, error))?;
        let path = entry.path();
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| failure("read metadata", &path, error))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let directory = metadata.is_dir();
        entries.push(FileEntry {
            preview: preview_kind(&path, directory),
            path,
            name,
            directory,
            size: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }
    Ok(entries)
}

fn preview_kind(path: &Path, directory: bool) -> PreviewKind {
    if directory {
        return PreviewKind::Directory;
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tga" | "qoi" => PreviewKind::Image,
        "svg" => PreviewKind::Svg,
        "gif" => PreviewKind::AnimatedImage,
        "webm" | "mkv" | "ivf" | "mp4" | "mov" => PreviewKind::Video,
        "txt" | "md" | "rs" | "toml" | "json" | "csv" => PreviewKind::Text,
        _ => PreviewKind::Other,
    }
}

fn sort_entries(entries: &mut [FileEntry], sort: FileSort, ascending: bool) {
    entries.sort_by_cached_key(|entry| {
        let primary = match sort {
            FileSort::Name => EntrySortValue::Name(entry.name.to_lowercase()),
            FileSort::Kind => EntrySortValue::Kind(entry.preview),
            FileSort::Size => EntrySortValue::Size(entry.size),
            FileSort::Modified => EntrySortValue::Modified(entry.modified),
        };
        EntrySortKey {
            directory_rank: if entry.directory { 0 } else { 1 },
            primary: if ascending {
                DirectedSortValue::Ascending(primary)
            } else {
                DirectedSortValue::Descending(Reverse(primary))
            },
            name: entry.name.clone(),
        }
    });
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EntrySortValue {
    Name(String),
    Kind(PreviewKind),
    Size(u64),
    Modified(Option<SystemTime>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DirectedSortValue {
    Ascending(EntrySortValue),
    Descending(Reverse<EntrySortValue>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EntrySortKey {
    directory_rank: u8,
    primary: DirectedSortValue,
    name: String,
}

fn copy_directory(source: &Path, destination: &Path) -> BrowserResult<()> {
    fs::create_dir(destination).map_err(|error| failure("copy directory", destination, error))?;
    let result = (|| {
        for result in
            fs::read_dir(source).map_err(|error| failure("copy directory", source, error))?
        {
            let entry = result.map_err(|error| failure("copy directory entry", source, error))?;
            let source_child = entry.path();
            let destination_child = destination.join(entry.file_name());
            let file_type = entry
                .file_type()
                .map_err(|error| failure("read file type", &source_child, error))?;
            if file_type.is_symlink() {
                return Err(failure(
                    "copy symbolic link",
                    &source_child,
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "copying symbolic links is not supported",
                    ),
                ));
            }
            if file_type.is_dir() {
                copy_directory(&source_child, &destination_child)?;
            } else {
                copy_file(&source_child, &destination_child)?;
            }
        }
        let permissions = fs::metadata(source)
            .map_err(|error| failure("copy directory permissions", source, error))?
            .permissions();
        fs::set_permissions(destination, permissions)
            .map_err(|error| failure("copy directory permissions", destination, error))?;
        Ok(())
    })();
    if let Err(error) = result {
        return match remove_copied_path(destination) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(failure(
                "rollback copy directory",
                destination,
                io::Error::new(
                    rollback_error.kind(),
                    format!("{error}; cleanup failed: {rollback_error}"),
                ),
            )),
        };
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> BrowserResult<()> {
    let mut input = fs::File::open(source).map_err(|error| failure("copy file", source, error))?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| failure("copy file", destination, error))?;
    let result = io::copy(&mut input, &mut output)
        .and_then(|_| output.set_permissions(input.metadata()?.permissions()))
        .and_then(|_| output.sync_all())
        .map_err(|error| failure("copy file", destination, error));
    if let Err(error) = result {
        return match remove_copied_path(destination) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(failure(
                "rollback copy file",
                destination,
                io::Error::new(
                    rollback_error.kind(),
                    format!("{error}; cleanup failed: {rollback_error}"),
                ),
            )),
        };
    }
    Ok(())
}

fn remove_copied_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            make_directory_removable(path, &metadata)?;
            for result in fs::read_dir(path)? {
                remove_copied_path(&result?.path())?;
            }
            fs::remove_dir(path)
        }
        Ok(metadata) => {
            if metadata.file_type().is_file() {
                make_file_removable(path, &metadata)?;
            }
            fs::remove_file(path)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn make_directory_removable(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = permissions.mode();
        let removable_mode = mode | 0o700;
        if removable_mode != mode {
            permissions.set_mode(removable_mode);
            fs::set_permissions(path, permissions)?;
        }
    }
    #[cfg(not(unix))]
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_file_removable(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_file_removable(_path: &Path, _metadata: &fs::Metadata) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "bui-file-browser-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
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
}

use super::*;

pub(super) fn entry_matches_filter(entry: &FileEntry, query: &str) -> bool {
    entry.directory || query.is_empty() || entry.name.to_lowercase().contains(query)
}

pub(super) fn canonical_directory(path: &Path) -> BrowserResult<PathBuf> {
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

pub(super) fn child_path(parent: &Path, name: &str) -> BrowserResult<PathBuf> {
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

pub(super) fn ensure_absent(path: &Path, operation: &'static str) -> BrowserResult<()> {
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

pub(super) fn read_entries(directory: &Path) -> BrowserResult<Vec<FileEntry>> {
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

pub(super) fn preview_kind(path: &Path, directory: bool) -> PreviewKind {
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

pub(super) fn sort_entries(entries: &mut [FileEntry], sort: FileSort, ascending: bool) {
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
pub(super) enum EntrySortValue {
    Name(String),
    Kind(PreviewKind),
    Size(u64),
    Modified(Option<SystemTime>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DirectedSortValue {
    Ascending(EntrySortValue),
    Descending(Reverse<EntrySortValue>),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct EntrySortKey {
    directory_rank: u8,
    primary: DirectedSortValue,
    name: String,
}

pub(super) fn copy_directory(source: &Path, destination: &Path) -> BrowserResult<()> {
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

pub(super) fn copy_file(source: &Path, destination: &Path) -> BrowserResult<()> {
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

pub(super) fn remove_copied_path(path: &Path) -> io::Result<()> {
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

pub(super) fn make_directory_removable(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
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
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "on Windows this clears the read-only attribute without widening file ACLs"
        )]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn make_file_removable(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "on Windows this clears the read-only attribute without widening file ACLs"
        )]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn make_file_removable(_path: &Path, _metadata: &fs::Metadata) -> io::Result<()> {
    Ok(())
}

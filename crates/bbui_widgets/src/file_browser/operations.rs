use super::*;

impl FileBrowser {
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
        self.check_root(&directory, "copy")?;
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
        self.check_root(&directory, "move")?;
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
}

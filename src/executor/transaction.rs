use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

/// Filesystem operations used by an output transaction.
///
/// Keeping promotion and cleanup behind this boundary makes otherwise rare
/// destination races and paired failures directly testable.
pub trait TransactionFs {
    fn entry_kind(&self, path: &Path) -> io::Result<Option<EntryKind>>;
    fn create_dir(&self, path: &Path) -> io::Result<()>;
    fn set_private_directory(&self, path: &Path) -> io::Result<()>;
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn promote_no_replace(&self, source: &Path, destination: &Path) -> io::Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RealTransactionFs;

impl TransactionFs for RealTransactionFs {
    fn entry_kind(&self, path: &Path) -> io::Result<Option<EntryKind>> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                Ok(Some(if file_type.is_symlink() {
                    EntryKind::Symlink
                } else if file_type.is_dir() {
                    EntryKind::Directory
                } else if file_type.is_file() {
                    EntryKind::File
                } else {
                    EntryKind::Other
                }))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }

    fn set_private_directory(&self, path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    fn promote_no_replace(&self, source: &Path, destination: &Path) -> io::Result<()> {
        platform_promote_no_replace(source, destination)
    }
}

#[derive(Debug)]
pub enum TransactionError {
    UnsafeDestination(PathBuf),
    UnsafeStagingTarget(PathBuf),
    UnsafeRelativePath(PathBuf),
    DestinationExists(PathBuf),
    UnsafeFilesystemEntry {
        path: PathBuf,
        kind: EntryKind,
    },
    ManifestAlreadyWritten,
    ManifestNotWritten,
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    PrimaryAndCleanup {
        primary: Box<TransactionError>,
        cleanup: Box<TransactionError>,
    },
}

impl fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeDestination(path) => {
                write!(
                    formatter,
                    "unsafe or overly broad output destination: {}",
                    path.display()
                )
            }
            Self::UnsafeStagingTarget(path) => write!(
                formatter,
                "staging target is not an exact destination sibling: {}",
                path.display()
            ),
            Self::UnsafeRelativePath(path) => {
                write!(
                    formatter,
                    "unsafe transaction-relative path: {}",
                    path.display()
                )
            }
            Self::DestinationExists(path) => {
                write!(
                    formatter,
                    "output destination already exists: {}",
                    path.display()
                )
            }
            Self::UnsafeFilesystemEntry { path, kind } => write!(
                formatter,
                "transaction path contains an unsafe {kind:?} entry: {}",
                path.display()
            ),
            Self::ManifestAlreadyWritten => {
                formatter.write_str("the transaction manifest was already written")
            }
            Self::ManifestNotWritten => formatter
                .write_str("the transaction cannot be promoted before its manifest is written"),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {}: {source}",
                path.display()
            ),
            Self::PrimaryAndCleanup { primary, cleanup } => write!(
                formatter,
                "transaction failed ({primary}); staging cleanup also failed ({cleanup})"
            ),
        }
    }
}

impl std::error::Error for TransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::PrimaryAndCleanup { primary, .. } => Some(primary.as_ref()),
            _ => None,
        }
    }
}

pub struct OutputTransaction<F: TransactionFs = RealTransactionFs> {
    fs: F,
    destination: PathBuf,
    staging: CreatedStaging,
    cleanup_armed: bool,
    manifest_written: bool,
}

/// The exact sibling directory successfully created by this transaction.
/// Cleanup accepts this private capability rather than a caller-supplied path.
struct CreatedStaging {
    path: PathBuf,
}

impl CreatedStaging {
    fn sibling(path: PathBuf, parent: &Path) -> Result<Self, TransactionError> {
        if path.parent() != Some(parent)
            || !matches!(path.file_name(), Some(name) if !name.is_empty())
        {
            return Err(TransactionError::UnsafeStagingTarget(path));
        }
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl OutputTransaction<RealTransactionFs> {
    pub fn begin(destination: impl AsRef<Path>) -> Result<Self, TransactionError> {
        Self::begin_with_fs(destination, RealTransactionFs)
    }
}

impl<F: TransactionFs> OutputTransaction<F> {
    pub fn begin_with_fs(destination: impl AsRef<Path>, fs: F) -> Result<Self, TransactionError> {
        let destination = destination.as_ref().to_path_buf();
        validate_destination(&destination)?;
        if fs_entry(&fs, &destination, "inspect destination")?.is_some() {
            return Err(TransactionError::DestinationExists(destination));
        }

        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        validate_directory_chain(&fs, parent)?;

        let filename = destination.file_name().expect("validated filename");
        let mut staging = None;
        for _ in 0..1_024 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let mut name = OsString::from(".");
            name.push(filename);
            name.push(format!(
                ".dicom-test-suite-staging-{}-{sequence}",
                std::process::id()
            ));
            let candidate = CreatedStaging::sibling(parent.join(name), parent)?;
            match fs.create_dir(candidate.path()) {
                Ok(()) => {
                    staging = Some(candidate);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(io_error(
                        "create staging directory",
                        candidate.path(),
                        error,
                    ));
                }
            }
        }
        let staging = staging.ok_or_else(|| {
            io_error(
                "create unique staging directory",
                parent,
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "exhausted bounded staging-name attempts",
                ),
            )
        })?;

        if let Err(error) = fs.set_private_directory(staging.path()) {
            let primary = io_error("make staging directory private", staging.path(), error);
            return Err(cleanup_after_primary(&fs, &staging, primary));
        }

        Ok(Self {
            fs,
            destination,
            staging,
            cleanup_armed: true,
            manifest_written: false,
        })
    }

    pub fn staging_root(&self) -> &Path {
        self.staging.path()
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub fn write_output(
        &self,
        relative_path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> Result<(), TransactionError> {
        self.write_relative(relative_path.as_ref(), bytes, "write output file")
    }

    pub fn write_manifest(&mut self, bytes: &[u8]) -> Result<(), TransactionError> {
        if self.manifest_written {
            return Err(TransactionError::ManifestAlreadyWritten);
        }
        self.write_relative(
            Path::new("manifest.json"),
            bytes,
            "write manifest exclusively",
        )?;
        self.manifest_written = true;
        Ok(())
    }

    pub fn abort_with_error(mut self, primary: TransactionError) -> TransactionError {
        self.cleanup_armed = false;
        cleanup_after_primary(&self.fs, &self.staging, primary)
    }

    pub fn cleanup(mut self) -> Result<(), TransactionError> {
        self.cleanup_armed = false;
        self.fs
            .remove_dir_all(self.staging.path())
            .map_err(|error| io_error("remove staging directory", self.staging.path(), error))
    }

    pub fn promote(mut self) -> Result<PathBuf, TransactionError> {
        if !self.manifest_written {
            self.cleanup_armed = false;
            return Err(cleanup_after_primary(
                &self.fs,
                &self.staging,
                TransactionError::ManifestNotWritten,
            ));
        }
        match self
            .fs
            .promote_no_replace(self.staging.path(), &self.destination)
        {
            Ok(()) => {
                self.cleanup_armed = false;
                Ok(self.destination.clone())
            }
            Err(error) => {
                let primary = io_error("promote staging directory", &self.destination, error);
                self.cleanup_armed = false;
                Err(cleanup_after_primary(&self.fs, &self.staging, primary))
            }
        }
    }

    fn write_relative(
        &self,
        relative_path: &Path,
        bytes: &[u8],
        operation: &'static str,
    ) -> Result<(), TransactionError> {
        validate_relative(relative_path)?;
        require_directory(&self.fs, self.staging.path())?;
        if let Some(parent) = relative_path.parent() {
            let mut current = self.staging.path().to_path_buf();
            for component in parent.components() {
                let Component::Normal(segment) = component else {
                    return Err(TransactionError::UnsafeRelativePath(
                        relative_path.to_path_buf(),
                    ));
                };
                current.push(segment);
                match fs_entry(&self.fs, &current, "inspect output directory")? {
                    Some(EntryKind::Directory) => {}
                    Some(kind) => {
                        return Err(TransactionError::UnsafeFilesystemEntry {
                            path: current,
                            kind,
                        });
                    }
                    None => match self.fs.create_dir(&current) {
                        Ok(()) => self.fs.set_private_directory(&current).map_err(|error| {
                            io_error("make output directory private", &current, error)
                        })?,
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                            require_directory(&self.fs, &current)?;
                        }
                        Err(error) => {
                            return Err(io_error("create output directory", &current, error));
                        }
                    },
                }
            }
        }
        let path = self.staging.path().join(relative_path);
        self.fs
            .write_new(&path, bytes)
            .map_err(|error| io_error(operation, path, error))
    }
}

impl<F: TransactionFs> Drop for OutputTransaction<F> {
    fn drop(&mut self) {
        if self.cleanup_armed {
            let _ = self.fs.remove_dir_all(self.staging.path());
        }
    }
}

fn validate_destination(path: &Path) -> Result<(), TransactionError> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(TransactionError::UnsafeDestination(path.to_path_buf()));
    }
    let mut normal = 0_usize;
    for component in path.components() {
        match component {
            Component::Normal(_) => normal += 1,
            Component::RootDir | Component::Prefix(_) => {}
            Component::CurDir | Component::ParentDir => {
                return Err(TransactionError::UnsafeDestination(path.to_path_buf()));
            }
        }
    }
    if normal == 0 || (path.is_absolute() && normal < 2) {
        return Err(TransactionError::UnsafeDestination(path.to_path_buf()));
    }
    Ok(())
}

fn validate_relative(path: &Path) -> Result<(), TransactionError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(TransactionError::UnsafeRelativePath(path.to_path_buf()));
    }
    Ok(())
}

fn validate_directory_chain<F: TransactionFs>(
    fs: &F,
    parent: &Path,
) -> Result<(), TransactionError> {
    let mut current = if parent.is_absolute() {
        PathBuf::new()
    } else {
        PathBuf::from(".")
    };
    if !parent.is_absolute() {
        require_existing_ancestor(fs, &current)?;
    }
    for component in parent.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::Normal(segment) => current.push(segment),
            Component::CurDir => continue,
            Component::ParentDir => {
                return Err(TransactionError::UnsafeDestination(parent.to_path_buf()));
            }
        }
        require_existing_ancestor(fs, &current)?;
    }
    Ok(())
}

fn require_existing_ancestor<F: TransactionFs>(
    fs: &F,
    path: &Path,
) -> Result<(), TransactionError> {
    match fs_entry(fs, path, "inspect destination ancestor")? {
        Some(EntryKind::Directory) => Ok(()),
        Some(kind) => Err(TransactionError::UnsafeFilesystemEntry {
            path: path.to_path_buf(),
            kind,
        }),
        None => Err(io_error(
            "find destination ancestor",
            path,
            io::Error::new(
                io::ErrorKind::NotFound,
                "destination ancestor does not exist",
            ),
        )),
    }
}

fn require_directory<F: TransactionFs>(fs: &F, path: &Path) -> Result<(), TransactionError> {
    match fs_entry(fs, path, "inspect transaction directory")? {
        Some(EntryKind::Directory) => Ok(()),
        Some(kind) => Err(TransactionError::UnsafeFilesystemEntry {
            path: path.to_path_buf(),
            kind,
        }),
        None => Err(io_error(
            "find transaction directory",
            path,
            io::Error::new(io::ErrorKind::NotFound, "transaction directory is missing"),
        )),
    }
}

fn fs_entry<F: TransactionFs>(
    fs: &F,
    path: &Path,
    operation: &'static str,
) -> Result<Option<EntryKind>, TransactionError> {
    fs.entry_kind(path)
        .map_err(|error| io_error(operation, path, error))
}

fn cleanup_after_primary<F: TransactionFs>(
    fs: &F,
    staging: &CreatedStaging,
    primary: TransactionError,
) -> TransactionError {
    match fs.remove_dir_all(staging.path()) {
        Ok(()) => primary,
        Err(error) => TransactionError::PrimaryAndCleanup {
            primary: Box::new(primary),
            cleanup: Box::new(io_error("remove staging directory", staging.path(), error)),
        },
    }
}

fn io_error(
    operation: &'static str,
    path: impl AsRef<Path>,
    source: io::Error,
) -> TransactionError {
    TransactionError::Io {
        operation,
        path: path.as_ref().to_path_buf(),
        source,
    }
}

fn platform_promote_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(source.as_os_str().as_bytes())?;
        let destination = CString::new(destination.as_os_str().as_bytes())?;
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            return Ok(());
        }
        Err(io::Error::last_os_error())
    }
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let source = CString::new(source.as_os_str().as_bytes())?;
        let destination = CString::new(destination.as_os_str().as_bytes())?;
        let result = unsafe {
            libc::renameatx_np(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        if result == 0 {
            return Ok(());
        }
        Err(io::Error::last_os_error())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        if fs::symlink_metadata(destination).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "output destination already exists",
            ));
        }
        fs::rename(source, destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn test_root(label: &str) -> PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "dicom-test-suite-transaction-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    fn fresh_parent(label: &str) -> PathBuf {
        let root = test_root(label);
        fs::create_dir(&root).unwrap();
        fs::canonicalize(root).unwrap()
    }

    #[derive(Clone, Default)]
    struct FaultFs {
        fail_write: Arc<AtomicBool>,
        fail_promote: Arc<AtomicBool>,
        fail_cleanup: Arc<AtomicBool>,
    }

    impl TransactionFs for FaultFs {
        fn entry_kind(&self, path: &Path) -> io::Result<Option<EntryKind>> {
            RealTransactionFs.entry_kind(path)
        }
        fn create_dir(&self, path: &Path) -> io::Result<()> {
            RealTransactionFs.create_dir(path)
        }
        fn set_private_directory(&self, path: &Path) -> io::Result<()> {
            RealTransactionFs.set_private_directory(path)
        }
        fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            if self.fail_write.load(Ordering::SeqCst) {
                return Err(io::Error::other("injected write failure"));
            }
            RealTransactionFs.write_new(path, bytes)
        }
        fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
            if self.fail_cleanup.load(Ordering::SeqCst) {
                return Err(io::Error::other("injected cleanup failure"));
            }
            RealTransactionFs.remove_dir_all(path)
        }
        fn promote_no_replace(&self, source: &Path, destination: &Path) -> io::Result<()> {
            if self.fail_promote.load(Ordering::SeqCst) {
                return Err(io::Error::other("injected promotion failure"));
            }
            RealTransactionFs.promote_no_replace(source, destination)
        }
    }

    struct SyntheticAncestorFs {
        destination: PathBuf,
        symlink: PathBuf,
        inspected: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl TransactionFs for SyntheticAncestorFs {
        fn entry_kind(&self, path: &Path) -> io::Result<Option<EntryKind>> {
            self.inspected.lock().unwrap().push(path.to_path_buf());
            if path == self.destination {
                Ok(None)
            } else if path == self.symlink {
                Ok(Some(EntryKind::Symlink))
            } else {
                Ok(Some(EntryKind::Directory))
            }
        }

        fn create_dir(&self, _: &Path) -> io::Result<()> {
            panic!("staging must not be created beneath a rejected ancestor")
        }

        fn set_private_directory(&self, _: &Path) -> io::Result<()> {
            unreachable!()
        }

        fn write_new(&self, _: &Path, _: &[u8]) -> io::Result<()> {
            unreachable!()
        }

        fn remove_dir_all(&self, _: &Path) -> io::Result<()> {
            unreachable!()
        }

        fn promote_no_replace(&self, _: &Path, _: &Path) -> io::Result<()> {
            unreachable!()
        }
    }

    #[test]
    fn rejects_broad_existing_and_symlink_destinations() {
        assert!(matches!(
            OutputTransaction::begin(Path::new("/")),
            Err(TransactionError::UnsafeDestination(_))
        ));
        assert!(matches!(
            OutputTransaction::begin(Path::new("../escape")),
            Err(TransactionError::UnsafeDestination(_))
        ));
        assert!(matches!(
            OutputTransaction::begin(Path::new("/unsafe-top-level-output")),
            Err(TransactionError::UnsafeDestination(_))
        ));

        let parent = fresh_parent("reject");
        let existing = parent.join("existing");
        fs::create_dir(&existing).unwrap();
        assert!(matches!(
            OutputTransaction::begin(&existing),
            Err(TransactionError::DestinationExists(_))
        ));

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&existing, parent.join("link")).unwrap();
            assert!(matches!(
                OutputTransaction::begin(parent.join("link")),
                Err(TransactionError::DestinationExists(_))
            ));
        }
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn injected_filesystem_checks_each_ancestor_without_following_links() {
        let destination = PathBuf::from("/synthetic-safe/linked/nested/corpus");
        let symlink = PathBuf::from("/synthetic-safe/linked");
        let inspected = Arc::new(Mutex::new(Vec::new()));
        let filesystem = SyntheticAncestorFs {
            destination: destination.clone(),
            symlink: symlink.clone(),
            inspected: Arc::clone(&inspected),
        };
        let error = match OutputTransaction::begin_with_fs(&destination, filesystem) {
            Ok(_) => panic!("injected symlinked ancestor was accepted"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TransactionError::UnsafeFilesystemEntry {
                path,
                kind: EntryKind::Symlink,
            } if path == symlink
        ));
        let inspected = inspected.lock().unwrap();
        assert!(inspected.contains(&PathBuf::from("/")));
        assert!(inspected.contains(&PathBuf::from("/synthetic-safe")));
        assert!(inspected.contains(&symlink));
        assert!(!inspected.contains(&PathBuf::from("/synthetic-safe/linked/nested")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_destination_beneath_symlinked_ancestor() {
        let parent = fresh_parent("ancestor-symlink");
        let actual = parent.join("actual");
        fs::create_dir(&actual).unwrap();
        fs::create_dir(actual.join("nested")).unwrap();
        let linked = parent.join("linked");
        std::os::unix::fs::symlink(&actual, &linked).unwrap();

        let error = match OutputTransaction::begin(linked.join("nested/corpus")) {
            Ok(_) => panic!("symlinked ancestor was accepted"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TransactionError::UnsafeFilesystemEntry {
                path,
                kind: EntryKind::Symlink,
            } if path == linked
        ));
        assert_eq!(fs::read_dir(&actual).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn writes_exclusively_and_promotes_private_sibling() {
        let parent = fresh_parent("promote");
        let destination = parent.join("corpus");
        let mut transaction = OutputTransaction::begin(&destination).unwrap();
        assert_eq!(transaction.staging_root().parent(), Some(parent.as_path()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(transaction.staging_root())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        transaction
            .write_output("instances/a.dcm", b"dicom")
            .unwrap();
        assert!(
            transaction
                .write_output("instances/a.dcm", b"replace")
                .is_err()
        );
        transaction.write_manifest(b"{}").unwrap();
        assert!(matches!(
            transaction.write_manifest(b"{}"),
            Err(TransactionError::ManifestAlreadyWritten)
        ));
        assert_eq!(transaction.promote().unwrap(), destination);
        assert_eq!(
            fs::read(destination.join("instances/a.dcm")).unwrap(),
            b"dicom"
        );
        assert_eq!(fs::read(destination.join("manifest.json")).unwrap(), b"{}");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn destination_race_preserves_winner_and_cleans_staging() {
        let parent = fresh_parent("race");
        let destination = parent.join("corpus");
        let mut transaction = OutputTransaction::begin(&destination).unwrap();
        let staging = transaction.staging_root().to_path_buf();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("winner"), b"winner").unwrap();
        transaction.write_manifest(b"{}").unwrap();
        assert!(transaction.promote().is_err());
        assert!(!staging.exists());
        assert_eq!(fs::read(destination.join("winner")).unwrap(), b"winner");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn rejects_symlinked_output_parent_and_unsafe_relative_paths() {
        let parent = fresh_parent("output-symlink");
        let outside = parent.join("outside");
        fs::create_dir(&outside).unwrap();
        let transaction = OutputTransaction::begin(parent.join("corpus")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, transaction.staging_root().join("linked")).unwrap();
        #[cfg(unix)]
        assert!(matches!(
            transaction.write_output("linked/file", b"bad"),
            Err(TransactionError::UnsafeFilesystemEntry {
                kind: EntryKind::Symlink,
                ..
            })
        ));
        assert!(matches!(
            transaction.write_output("../escape", b"bad"),
            Err(TransactionError::UnsafeRelativePath(_))
        ));
        transaction.cleanup().unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn preserves_primary_and_cleanup_failures() {
        let parent = fresh_parent("paired");
        let destination = parent.join("corpus");
        let fs = FaultFs::default();
        fs.fail_write.store(true, Ordering::SeqCst);
        fs.fail_cleanup.store(true, Ordering::SeqCst);
        let transaction = OutputTransaction::begin_with_fs(&destination, fs.clone()).unwrap();
        let staging = transaction.staging_root().to_path_buf();
        let primary = transaction.write_output("file", b"data").unwrap_err();
        let error = transaction.abort_with_error(primary);
        assert!(matches!(error, TransactionError::PrimaryAndCleanup { .. }));
        fs.fail_cleanup.store(false, Ordering::SeqCst);
        fs::remove_dir_all(staging).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn preserves_promotion_and_cleanup_failures() {
        let parent = fresh_parent("promote-paired");
        let destination = parent.join("corpus");
        let fs = FaultFs::default();
        fs.fail_promote.store(true, Ordering::SeqCst);
        fs.fail_cleanup.store(true, Ordering::SeqCst);
        let mut transaction = OutputTransaction::begin_with_fs(&destination, fs.clone()).unwrap();
        let staging = transaction.staging_root().to_path_buf();
        transaction.write_manifest(b"{}").unwrap();
        let error = transaction.promote().unwrap_err();
        assert!(matches!(error, TransactionError::PrimaryAndCleanup { .. }));
        fs.fail_cleanup.store(false, Ordering::SeqCst);
        fs::remove_dir_all(staging).unwrap();
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn cleanup_removes_only_the_exact_staging_target() {
        let parent = fresh_parent("exact-cleanup");
        let destination = parent.join("corpus");
        let transaction = OutputTransaction::begin(&destination).unwrap();
        let staging = transaction.staging_root().to_path_buf();
        let sibling = parent.join(format!(
            "{}-keep",
            staging.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir(&sibling).unwrap();
        transaction.cleanup().unwrap();
        assert!(!staging.exists());
        assert!(sibling.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn promotion_requires_an_exclusive_manifest() {
        let parent = fresh_parent("manifest-required");
        let destination = parent.join("corpus");
        let transaction = OutputTransaction::begin(&destination).unwrap();
        let staging = transaction.staging_root().to_path_buf();
        assert!(matches!(
            transaction.promote(),
            Err(TransactionError::ManifestNotWritten)
        ));
        assert!(!staging.exists());
        assert!(!destination.exists());
        fs::remove_dir_all(parent).unwrap();
    }
}

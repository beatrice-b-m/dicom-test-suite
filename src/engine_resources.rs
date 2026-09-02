use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/embedded_engine_resources.rs"));

pub const ENGINE_RESOURCE_SET_VERSION: &str = "2.0.0";
pub const ENGINE_RESOURCE_COUNT_V2: usize = 74;
pub const ENGINE_RESOURCE_TOTAL_BYTES_V2: u64 = 1_251_116;
pub const ENGINE_RESOURCE_SHA256_V2: &str =
    "a54f1c1e897162dfaca6c3bc9264b45d2e2ddc77258fe3c6263f7a285a675c17";
pub const TRANSITIONAL_ENGINE_RESOURCE_COUNT_V1: usize = 240;
pub const TRANSITIONAL_ENGINE_RESOURCE_SHA256_V1: &str =
    "dc61cc012f983297fef864f68e6cd172a9d33ac9ad4faab4cc66d3526b688410";
/// R4.4 makes the reduced immutable set authoritative while retaining the
/// full physical table solely for internal compatibility and explicit-root
/// integrity until R5 rewires every consumer.
pub const ENGINE_RESOURCE_SET_MEMBERSHIP: EngineResourceSetMembership =
    EngineResourceSetMembership::SeparatedWithLegacyPhysicalClosure;
pub const TEMPLATE_CATALOG_RESOURCE: &str = "templates/catalog.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineResourceSetMembership {
    SeparatedWithLegacyPhysicalClosure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineResourceOrigin {
    Embedded,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineResourceRecord {
    pub logical_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineResourceIdentity {
    pub resource_set_version: String,
    pub origin: EngineResourceOrigin,
    pub resource_count: usize,
    pub resource_set_sha256: String,
    pub resources: Vec<EngineResourceRecord>,
}

#[derive(Debug, Clone)]
enum EngineResourceSource {
    Embedded,
    Explicit(Arc<BTreeMap<&'static str, Vec<u8>>>),
}

#[derive(Debug, Clone)]
pub struct EngineResources {
    source: EngineResourceSource,
    snapshot_cache: Arc<SnapshotCache>,
}

#[derive(Debug, Clone)]
pub struct EngineResourceSnapshot {
    materialized: Arc<MaterializedSnapshot>,
}

#[derive(Debug)]
struct SnapshotCache {
    state: Mutex<SnapshotCacheState>,
    ready: Condvar,
}

#[derive(Debug, Default)]
enum SnapshotCacheState {
    #[default]
    Empty,
    Building,
    Ready(Arc<MaterializedSnapshot>),
}

impl Default for SnapshotCache {
    fn default() -> Self {
        Self {
            state: Mutex::new(SnapshotCacheState::Empty),
            ready: Condvar::new(),
        }
    }
}

#[derive(Debug)]
struct MaterializedSnapshot {
    root: PathBuf,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EngineResourceError {
    UnsafeLogicalPath(String),
    UnknownResource(String),
    Read {
        logical_path: String,
        path: PathBuf,
        source: std::io::Error,
    },
    Symlink {
        logical_path: String,
        path: PathBuf,
    },
    NotRegular {
        logical_path: String,
        path: PathBuf,
    },
    SizeMismatch {
        logical_path: String,
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    Unstable {
        logical_path: String,
        path: PathBuf,
    },
    NonUtf8(String),
    Integrity {
        expected_resource_set_sha256: String,
        actual_resource_set_sha256: String,
    },
    CreateSnapshot {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteSnapshot {
        logical_path: String,
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for EngineResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeLogicalPath(path) => {
                write!(formatter, "unsafe product resource path: {path}")
            }
            Self::UnknownResource(path) => write!(formatter, "unknown product resource: {path}"),
            Self::Read {
                logical_path,
                path,
                source,
            } => write!(
                formatter,
                "read product resource {logical_path} at {}: {source}",
                path.display()
            ),
            Self::Symlink { logical_path, path } => write!(
                formatter,
                "engine resource {logical_path} resolves through a symbolic link at {}",
                path.display()
            ),
            Self::NotRegular { logical_path, path } => write!(
                formatter,
                "engine resource {logical_path} is not a regular file at {}",
                path.display()
            ),
            Self::SizeMismatch {
                logical_path,
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "product resource integrity failed: engine resource {logical_path} at {} has size {actual}, expected {expected}",
                path.display()
            ),
            Self::Unstable { logical_path, path } => write!(
                formatter,
                "engine resource {logical_path} changed while being captured at {}",
                path.display()
            ),
            Self::NonUtf8(path) => write!(formatter, "product resource is not UTF-8: {path}"),
            Self::Integrity {
                expected_resource_set_sha256,
                actual_resource_set_sha256,
            } => write!(
                formatter,
                "product resource integrity failed: expected set {expected_resource_set_sha256}, got {actual_resource_set_sha256}"
            ),
            Self::CreateSnapshot { path, source } => {
                write!(
                    formatter,
                    "create product resource snapshot {}: {source}",
                    path.display()
                )
            }
            Self::WriteSnapshot {
                logical_path,
                path,
                source,
            } => write!(
                formatter,
                "write product resource {logical_path} to snapshot {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for EngineResourceError {}

impl EngineResources {
    pub fn embedded() -> Self {
        Self {
            source: EngineResourceSource::Embedded,
            snapshot_cache: Arc::default(),
        }
    }

    pub fn explicit(root: impl Into<PathBuf>) -> Result<Self, EngineResourceError> {
        let root = root.into();
        let captured = capture_explicit_resources(&root)?;
        let resources = Self {
            source: EngineResourceSource::Explicit(Arc::new(captured)),
            snapshot_cache: Arc::default(),
        };
        resources.verify_integrity()?;
        Ok(resources)
    }

    pub fn origin(&self) -> EngineResourceOrigin {
        match self.source {
            EngineResourceSource::Embedded => EngineResourceOrigin::Embedded,
            EngineResourceSource::Explicit(_) => EngineResourceOrigin::Explicit,
        }
    }

    pub fn logical_paths(&self) -> Vec<&'static str> {
        EMBEDDED_ENGINE_RESOURCES
            .iter()
            .map(|(path, _)| *path)
            .collect()
    }

    pub fn contains(&self, logical_path: &str) -> bool {
        validate_logical_path(logical_path).is_ok()
            && EMBEDDED_ENGINE_RESOURCES
                .binary_search_by_key(&logical_path, |(path, _)| *path)
                .is_ok()
    }

    pub fn bytes(&self, logical_path: &str) -> Result<Cow<'static, [u8]>, EngineResourceError> {
        validate_logical_path(logical_path)?;
        let index = EMBEDDED_ENGINE_RESOURCES
            .binary_search_by_key(&logical_path, |(path, _)| *path)
            .map_err(|_| EngineResourceError::UnknownResource(logical_path.to_string()))?;
        match &self.source {
            EngineResourceSource::Embedded => Ok(Cow::Borrowed(EMBEDDED_ENGINE_RESOURCES[index].1)),
            EngineResourceSource::Explicit(captured) => Ok(Cow::Owned(
                captured
                    .get(logical_path)
                    .expect("validated engine resource was captured")
                    .clone(),
            )),
        }
    }

    pub fn text(&self, logical_path: &str) -> Result<Cow<'static, str>, EngineResourceError> {
        match self.bytes(logical_path)? {
            Cow::Borrowed(bytes) => std::str::from_utf8(bytes)
                .map(Cow::Borrowed)
                .map_err(|_| EngineResourceError::NonUtf8(logical_path.to_string())),
            Cow::Owned(bytes) => String::from_utf8(bytes)
                .map(Cow::Owned)
                .map_err(|_| EngineResourceError::NonUtf8(logical_path.to_string())),
        }
    }

    pub fn identity(&self) -> Result<EngineResourceIdentity, EngineResourceError> {
        self.identity_for_paths(
            self.logical_paths()
                .into_iter()
                .filter(|path| *path != "Cargo.lock" && !path.starts_with("cases/")),
            ENGINE_RESOURCE_SET_VERSION,
        )
    }

    pub(crate) fn legacy_identity_v1(&self) -> Result<EngineResourceIdentity, EngineResourceError> {
        self.identity_for_paths(LEGACY_ENGINE_RESOURCE_PATHS_V1.iter().copied(), "1.0.0")
    }

    fn identity_for_paths<'a>(
        &self,
        paths: impl IntoIterator<Item = &'a str>,
        version: &str,
    ) -> Result<EngineResourceIdentity, EngineResourceError> {
        let mut records = Vec::new();
        let mut identity_bytes = Vec::new();
        for logical_path in paths {
            let bytes = self.bytes(logical_path)?;
            let sha256 = crate::sha256_hex(&bytes);
            identity_bytes.extend_from_slice(logical_path.as_bytes());
            identity_bytes.push(0);
            identity_bytes.extend_from_slice(sha256.as_bytes());
            identity_bytes.push(0);
            identity_bytes.extend_from_slice(bytes.len().to_string().as_bytes());
            identity_bytes.push(b'\n');
            records.push(EngineResourceRecord {
                logical_path: logical_path.to_string(),
                size_bytes: bytes.len() as u64,
                sha256,
            });
        }
        Ok(EngineResourceIdentity {
            resource_set_version: version.to_string(),
            origin: self.origin(),
            resource_count: records.len(),
            resource_set_sha256: crate::sha256_hex(&identity_bytes),
            resources: records,
        })
    }

    pub fn verify_integrity(&self) -> Result<EngineResourceIdentity, EngineResourceError> {
        let legacy = self.legacy_identity_v1()?;
        verify_transitional_oracle(&legacy)?;
        let actual = self.identity()?;
        if self.origin() == EngineResourceOrigin::Embedded {
            verify_current_oracle(&actual)?;
            return Ok(actual);
        }
        let expected = Self::embedded().verify_integrity()?;
        if actual.resource_set_sha256 != expected.resource_set_sha256 {
            return Err(EngineResourceError::Integrity {
                expected_resource_set_sha256: expected.resource_set_sha256,
                actual_resource_set_sha256: actual.resource_set_sha256,
            });
        }
        Ok(actual)
    }

    pub fn snapshot(&self) -> Result<EngineResourceSnapshot, EngineResourceError> {
        self.verify_integrity()?;
        let materialized =
            Arc::new(self.materialize_snapshot(&mut |path, bytes| fs::write(path, bytes))?);
        self.validate_snapshot_root(&materialized.root)?;
        Ok(EngineResourceSnapshot { materialized })
    }

    pub(crate) fn shared_snapshot(&self) -> Result<EngineResourceSnapshot, EngineResourceError> {
        self.shared_snapshot_with_writer(|path, bytes| fs::write(path, bytes))
    }

    fn shared_snapshot_with_writer(
        &self,
        mut write: impl FnMut(&Path, &[u8]) -> std::io::Result<()>,
    ) -> Result<EngineResourceSnapshot, EngineResourceError> {
        loop {
            let mut state = lock_snapshot_state(&self.snapshot_cache);
            match &*state {
                SnapshotCacheState::Ready(materialized) => {
                    let materialized = Arc::clone(materialized);
                    drop(state);
                    self.validate_snapshot_root(&materialized.root)?;
                    return Ok(EngineResourceSnapshot { materialized });
                }
                SnapshotCacheState::Building => {
                    state = self
                        .snapshot_cache
                        .ready
                        .wait(state)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    drop(state);
                }
                SnapshotCacheState::Empty => {
                    *state = SnapshotCacheState::Building;
                    drop(state);
                    break;
                }
            }
        }

        let mut publication = SnapshotPublication::new(&self.snapshot_cache);
        self.verify_integrity()?;
        let materialized = Arc::new(self.materialize_snapshot(&mut write)?);
        self.validate_snapshot_root(&materialized.root)?;
        publication.publish(Arc::clone(&materialized));
        Ok(EngineResourceSnapshot { materialized })
    }

    fn materialize_snapshot(
        &self,
        write: &mut impl FnMut(&Path, &[u8]) -> std::io::Result<()>,
    ) -> Result<MaterializedSnapshot, EngineResourceError> {
        static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir();
        let root = (0..128)
            .find_map(|_| {
                let sequence = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
                let candidate = parent.join(format!(
                    "synth-dicom-gen-resources-{}-{sequence}",
                    std::process::id()
                ));
                match create_private_directory(&candidate) {
                    Ok(()) => Some(Ok(candidate)),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(source) => Some(Err(EngineResourceError::CreateSnapshot {
                        path: candidate,
                        source,
                    })),
                }
            })
            .transpose()?
            .ok_or_else(|| EngineResourceError::CreateSnapshot {
                path: parent,
                source: std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "could not allocate a unique resource snapshot",
                ),
            })?;
        let mut pending = PendingSnapshot::new(root);

        for logical_path in self.logical_paths() {
            let path = pending.root().join(logical_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|source| {
                    EngineResourceError::WriteSnapshot {
                        logical_path: logical_path.to_string(),
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
            let bytes = self.bytes(logical_path)?;
            write(&path, &bytes).map_err(|source| EngineResourceError::WriteSnapshot {
                logical_path: logical_path.to_string(),
                path,
                source,
            })?;
        }
        Ok(MaterializedSnapshot {
            root: pending.publish(),
        })
    }

    fn validate_snapshot_root(&self, root: &Path) -> Result<(), EngineResourceError> {
        let captured = capture_explicit_resources(root)?;
        for logical_path in self.logical_paths() {
            let actual = captured
                .get(logical_path)
                .expect("complete snapshot capture contains every resource");
            if actual.as_slice() != self.bytes(logical_path)?.as_ref() {
                return Err(EngineResourceError::Integrity {
                    expected_resource_set_sha256: format!(
                        "{}:{}",
                        logical_path,
                        crate::sha256_hex(self.bytes(logical_path)?.as_ref())
                    ),
                    actual_resource_set_sha256: format!(
                        "{}:{}",
                        logical_path,
                        crate::sha256_hex(actual)
                    ),
                });
            }
        }
        Ok(())
    }
}

fn lock_snapshot_state(cache: &SnapshotCache) -> MutexGuard<'_, SnapshotCacheState> {
    cache
        .state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct SnapshotPublication<'a> {
    cache: &'a SnapshotCache,
    published: bool,
}

impl<'a> SnapshotPublication<'a> {
    fn new(cache: &'a SnapshotCache) -> Self {
        Self {
            cache,
            published: false,
        }
    }

    fn publish(&mut self, materialized: Arc<MaterializedSnapshot>) {
        *lock_snapshot_state(self.cache) = SnapshotCacheState::Ready(materialized);
        self.published = true;
        self.cache.ready.notify_all();
    }
}

impl Drop for SnapshotPublication<'_> {
    fn drop(&mut self) {
        if !self.published {
            *lock_snapshot_state(self.cache) = SnapshotCacheState::Empty;
            self.cache.ready.notify_all();
        }
    }
}

#[derive(Debug)]
struct PendingSnapshot {
    root: Option<PathBuf>,
}

impl PendingSnapshot {
    fn new(root: PathBuf) -> Self {
        Self { root: Some(root) }
    }

    fn root(&self) -> &Path {
        self.root.as_deref().expect("pending snapshot has a root")
    }

    fn publish(&mut self) -> PathBuf {
        self.root.take().expect("pending snapshot has a root")
    }
}

impl Drop for PendingSnapshot {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            let _ = fs::remove_dir_all(root);
        }
    }
}

fn verify_current_oracle(identity: &EngineResourceIdentity) -> Result<(), EngineResourceError> {
    let total = identity
        .resources
        .iter()
        .map(|record| record.size_bytes)
        .sum::<u64>();
    if identity.resource_set_version == ENGINE_RESOURCE_SET_VERSION
        && identity.resource_count == ENGINE_RESOURCE_COUNT_V2
        && total == ENGINE_RESOURCE_TOTAL_BYTES_V2
        && identity.resource_set_sha256 == ENGINE_RESOURCE_SHA256_V2
    {
        return Ok(());
    }
    Err(EngineResourceError::Integrity {
        expected_resource_set_sha256: format!(
            "version={ENGINE_RESOURCE_SET_VERSION};count={ENGINE_RESOURCE_COUNT_V2};bytes={ENGINE_RESOURCE_TOTAL_BYTES_V2};sha256={ENGINE_RESOURCE_SHA256_V2}"
        ),
        actual_resource_set_sha256: format!(
            "version={};count={};bytes={total};sha256={}",
            identity.resource_set_version, identity.resource_count, identity.resource_set_sha256
        ),
    })
}

impl EngineResourceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsafeLogicalPath(_)
            | Self::UnknownResource(_)
            | Self::NonUtf8(_)
            | Self::Symlink { .. }
            | Self::NotRegular { .. }
            | Self::Unstable { .. } => "resource.document.invalid",
            Self::SizeMismatch { .. } | Self::Integrity { .. } => "evidence.integrity.failed",
            Self::Read { .. } => "io.read.failed",
            Self::CreateSnapshot { .. } | Self::WriteSnapshot { .. } => "io.write.failed",
        }
    }
}

impl EngineResourceSnapshot {
    pub fn root(&self) -> &Path {
        &self.materialized.root
    }

    pub fn path(&self, logical_path: &str) -> Result<PathBuf, EngineResourceError> {
        validate_logical_path(logical_path)?;
        Ok(self.materialized.root.join(logical_path))
    }
}

impl Drop for MaterializedSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

fn validate_logical_path(path: &str) -> Result<(), EngineResourceError> {
    let parsed = Path::new(path);
    let safe = !path.is_empty()
        && !path.contains('\\')
        && !parsed.is_absolute()
        && parsed
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if safe {
        Ok(())
    } else {
        Err(EngineResourceError::UnsafeLogicalPath(path.to_string()))
    }
}

fn verify_transitional_oracle(
    identity: &EngineResourceIdentity,
) -> Result<(), EngineResourceError> {
    if identity.resource_set_version == "1.0.0"
        && identity.resource_count == TRANSITIONAL_ENGINE_RESOURCE_COUNT_V1
        && identity.resource_set_sha256 == TRANSITIONAL_ENGINE_RESOURCE_SHA256_V1
    {
        return Ok(());
    }
    Err(EngineResourceError::Integrity {
        expected_resource_set_sha256: format!(
            "version=1.0.0;count={TRANSITIONAL_ENGINE_RESOURCE_COUNT_V1};sha256={TRANSITIONAL_ENGINE_RESOURCE_SHA256_V1}"
        ),
        actual_resource_set_sha256: format!(
            "version={};count={};sha256={}",
            identity.resource_set_version, identity.resource_count, identity.resource_set_sha256
        ),
    })
}

#[cfg(test)]
mod snapshot_cache_tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::{Arc, Barrier};
    use std::thread;

    fn file_inventory(root: &Path) -> (usize, u64) {
        let mut pending = vec![root.to_path_buf()];
        let mut files = 0;
        let mut bytes = 0;
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                let metadata = entry.metadata().unwrap();
                if metadata.is_dir() {
                    pending.push(entry.path());
                } else {
                    files += 1;
                    bytes += metadata.len();
                }
            }
        }
        (files, bytes)
    }

    #[test]
    fn snapshot_cache_is_lazy_and_reuses_one_complete_copy_across_clones() {
        let resources = EngineResources::embedded();
        resources.verify_integrity().unwrap();
        resources.bytes("templates/catalog.json").unwrap();
        assert!(
            matches!(
                *lock_snapshot_state(&resources.snapshot_cache),
                SnapshotCacheState::Empty
            ),
            "identity and direct byte reads must not materialize a snapshot"
        );

        let clone = resources.clone();
        let started = std::time::Instant::now();
        let first = resources.shared_snapshot().unwrap();
        let second = resources.shared_snapshot().unwrap();
        let third = clone.shared_snapshot().unwrap();
        let elapsed = started.elapsed();
        assert_eq!(first.root(), second.root());
        assert_eq!(first.root(), third.root());
        assert_eq!(file_inventory(first.root()), (254, 2_664_374));
        eprintln!(
            "r4_5_post operations=3 roots=1 files_written=254 bytes_written=2664374 elapsed_us={}",
            elapsed.as_micros()
        );
    }

    #[test]
    fn concurrent_snapshot_acquisition_materializes_once_per_handle() {
        let resources = EngineResources::embedded();
        let barrier = Arc::new(Barrier::new(9));
        let workers = (0..8)
            .map(|_| {
                let resources = resources.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    resources.shared_snapshot().unwrap().root().to_path_buf()
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        let roots = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn separately_constructed_handles_remain_isolated() {
        let first_resources = EngineResources::embedded();
        let second_resources = EngineResources::embedded();
        let first = first_resources.shared_snapshot().unwrap();
        let second = second_resources.shared_snapshot().unwrap();
        assert_ne!(first.root(), second.root());
        assert_eq!(file_inventory(first.root()), file_inventory(second.root()));
    }

    #[test]
    fn materialized_root_lives_until_the_last_resource_or_snapshot_handle() {
        let resources = EngineResources::embedded();
        let resource_clone = resources.clone();
        let snapshot = resources.shared_snapshot().unwrap();
        let snapshot_clone = snapshot.clone();
        let root = snapshot.root().to_path_buf();
        drop(resources);
        drop(snapshot);
        assert!(root.exists());
        drop(resource_clone);
        assert!(root.exists());
        drop(snapshot_clone);
        assert!(!root.exists());
    }

    #[test]
    fn failed_or_corrupt_materialization_is_not_published_and_can_retry() {
        let resources = EngineResources::embedded();
        let mut failed_root = None;
        let error = resources
            .shared_snapshot_with_writer(|path, _| {
                failed_root = path.ancestors().find_map(|ancestor| {
                    ancestor
                        .file_name()
                        .and_then(|name| name.to_str())
                        .filter(|name| name.starts_with("synth-dicom-gen-resources-"))
                        .map(|_| ancestor.to_path_buf())
                });
                Err(std::io::Error::other("injected write failure"))
            })
            .unwrap_err();
        assert!(matches!(error, EngineResourceError::WriteSnapshot { .. }));
        assert!(
            matches!(
                *lock_snapshot_state(&resources.snapshot_cache),
                SnapshotCacheState::Empty
            ),
            "failed materialization must reset the cache to retryable empty state"
        );
        assert!(!failed_root.expect("writer observed snapshot root").exists());
        let snapshot = resources.shared_snapshot().unwrap();
        assert_eq!(file_inventory(snapshot.root()), (254, 2_664_374));
    }

    #[test]
    fn mutation_of_an_internal_shared_root_fails_closed_on_reuse() {
        let resources = EngineResources::embedded();
        let snapshot = resources.shared_snapshot().unwrap();
        let catalog = snapshot.path("templates/catalog.json").unwrap();
        let mut bytes = fs::read(&catalog).unwrap();
        bytes[0] ^= 1;
        fs::write(catalog, bytes).unwrap();
        let error = resources.shared_snapshot().unwrap_err();
        assert!(matches!(error, EngineResourceError::Integrity { .. }));
        assert_eq!(error.code(), "evidence.integrity.failed");
    }

    #[test]
    fn one_handle_reuses_one_tree_across_batch_generate_validate_report_and_compose() {
        let resources = EngineResources::embedded();
        let workspace = std::env::temp_dir().join(format!(
            "synth-dicom-gen-r4-5-batch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let generated = workspace.join("generated");
        let run = crate::prepare_generation_run(crate::GenerateOptions {
            profile: "smoke".to_string(),
            out_dir: generated.clone(),
            seed: 1,
            include_stress: false,
        })
        .unwrap();
        let summary = crate::write_generation_run_with_resources(&run, &resources).unwrap();
        assert_eq!(summary.files_written, 3);
        let shared_root = resources.shared_snapshot().unwrap().root().to_path_buf();

        let validation = crate::validate_generated_root_with_resources(&generated, &resources)
            .expect("shared-resource validation must complete");
        assert!(validation.failures.is_empty());
        let report = crate::build_coverage_report_with_resources(&generated, &resources)
            .expect("shared-resource report must complete");
        assert_eq!(report["coverage_report_schema_version"], "1.0.0");

        for (path, expected) in [
            (
                "classic/sc/mono1_u8_explicit_le/instance.dcm",
                "76dc5208b139899fcb87bbf7ec9edf1a323000a91c4015de9ef8bde7bd344ecc",
            ),
            (
                "classic/sc/mono2_u8_explicit_le/instance.dcm",
                "fce766bcbb4b4aa79cfb3fa0c3b5e4ef888b11c0708fad713b9cde8d41ec6a15",
            ),
            (
                "classic/sc/rgb_planar0_explicit_le/instance.dcm",
                "33de9448509431fda27005cbf83c79977f1c3ebadb669ae1dedf1a225742f3c5",
            ),
        ] {
            assert_eq!(
                crate::sha256_hex(&fs::read(generated.join(path)).unwrap()),
                expected
            );
        }

        let composition_out = workspace.join("composition");
        let composition_options = crate::composition::ComposeBytesOptions {
            spec_root: PathBuf::from("tests/fixtures/composition/valid"),
            out_dir: composition_out.clone(),
            seed: 1,
            catalog_path: PathBuf::from(TEMPLATE_CATALOG_RESOURCE),
            dry_run: false,
        };
        let (composition_summary, _) = crate::composition::compose_from_bytes_with_resources(
            include_bytes!("../tests/fixtures/composition/valid/template-only.json"),
            &composition_options,
            &resources,
        )
        .unwrap();
        assert_eq!(composition_summary.instances_written, 1);
        assert_eq!(
            resources.shared_snapshot().unwrap().root(),
            shared_root.as_path()
        );
        assert_eq!(file_inventory(&shared_root), (254, 2_664_374));

        fs::remove_dir_all(workspace).unwrap();
    }
}

#[cfg(unix)]
fn capture_explicit_resources(
    root: &Path,
) -> Result<BTreeMap<&'static str, Vec<u8>>, EngineResourceError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let root_before = fs::symlink_metadata(root).map_err(|source| EngineResourceError::Read {
        logical_path: ".".to_string(),
        path: root.to_path_buf(),
        source,
    })?;
    if root_before.file_type().is_symlink() {
        return Err(EngineResourceError::Symlink {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }
    if !root_before.is_dir() {
        return Err(EngineResourceError::NotRegular {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }
    let root_handle = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)
        .map_err(|source| map_open_error(".", root.to_path_buf(), source))?;
    let root_open = root_handle
        .metadata()
        .map_err(|source| EngineResourceError::Read {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
            source,
        })?;
    if root_before.dev() != root_open.dev() || root_before.ino() != root_open.ino() {
        return Err(EngineResourceError::Unstable {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }

    let expected_total = EMBEDDED_ENGINE_RESOURCES
        .iter()
        .try_fold(0_u64, |total, (_, bytes)| {
            total.checked_add(bytes.len() as u64)
        })
        .expect("embedded engine resource total fits u64");
    let mut inspected_total = 0_u64;
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let file = open_resource_at(&root_handle, root, logical_path)?;
        let metadata = validate_open_resource(&file, root, logical_path, expected.len() as u64)?;
        inspected_total = inspected_total.checked_add(metadata.len()).ok_or_else(|| {
            EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                expected: expected_total,
                actual: u64::MAX,
            }
        })?;
    }
    if inspected_total != expected_total {
        return Err(EngineResourceError::SizeMismatch {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
            expected: expected_total,
            actual: inspected_total,
        });
    }

    let mut captured = BTreeMap::new();
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let mut file = open_resource_at(&root_handle, root, logical_path)?;
        let before = validate_open_resource(&file, root, logical_path, expected.len() as u64)?;
        let mut bytes = Vec::with_capacity(expected.len());
        file.by_ref()
            .take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| EngineResourceError::Read {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                source,
            })?;
        if bytes.len() != expected.len() {
            return Err(EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                expected: expected.len() as u64,
                actual: bytes.len() as u64,
            });
        }
        let after = file
            .metadata()
            .map_err(|source| EngineResourceError::Read {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
                source,
            })?;
        if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len()
        {
            return Err(EngineResourceError::Unstable {
                logical_path: (*logical_path).to_string(),
                path: root.join(logical_path),
            });
        }
        captured.insert(*logical_path, bytes);
    }

    let root_after = fs::symlink_metadata(root).map_err(|source| EngineResourceError::Read {
        logical_path: ".".to_string(),
        path: root.to_path_buf(),
        source,
    })?;
    if root_after.file_type().is_symlink()
        || !root_after.is_dir()
        || root_after.dev() != root_open.dev()
        || root_after.ino() != root_open.ino()
    {
        return Err(EngineResourceError::Unstable {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
        });
    }
    Ok(captured)
}

#[cfg(unix)]
fn open_resource_at(
    root_handle: &fs::File,
    root: &Path,
    logical_path: &'static str,
) -> Result<fs::File, EngineResourceError> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    let components = Path::new(logical_path).components().collect::<Vec<_>>();
    let mut directory = root_handle
        .try_clone()
        .map_err(|source| EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path: root.to_path_buf(),
            source,
        })?;
    let mut traversed = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(EngineResourceError::UnsafeLogicalPath(
                logical_path.to_string(),
            ));
        };
        traversed.push(component);
        let name = CString::new(component.as_bytes())
            .map_err(|_| EngineResourceError::UnsafeLogicalPath(logical_path.to_string()))?;
        let is_last = index + 1 == components.len();
        let flags = libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | if is_last {
                libc::O_RDONLY | libc::O_NONBLOCK
            } else {
                libc::O_RDONLY | libc::O_DIRECTORY
            };
        let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(map_open_error(
                logical_path,
                traversed,
                std::io::Error::last_os_error(),
            ));
        }
        let opened = unsafe { fs::File::from_raw_fd(descriptor) };
        if is_last {
            return Ok(opened);
        }
        directory = opened;
    }
    Err(EngineResourceError::UnsafeLogicalPath(
        logical_path.to_string(),
    ))
}

#[cfg(unix)]
fn validate_open_resource(
    file: &fs::File,
    root: &Path,
    logical_path: &'static str,
    expected: u64,
) -> Result<fs::Metadata, EngineResourceError> {
    let path = root.join(logical_path);
    let metadata = file
        .metadata()
        .map_err(|source| EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path: path.clone(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(EngineResourceError::NotRegular {
            logical_path: logical_path.to_string(),
            path,
        });
    }
    if metadata.len() != expected {
        return Err(EngineResourceError::SizeMismatch {
            logical_path: logical_path.to_string(),
            path,
            expected,
            actual: metadata.len(),
        });
    }
    Ok(metadata)
}

#[cfg(unix)]
fn map_open_error(
    logical_path: &str,
    path: PathBuf,
    source: std::io::Error,
) -> EngineResourceError {
    match source.raw_os_error() {
        Some(libc::ELOOP) => EngineResourceError::Symlink {
            logical_path: logical_path.to_string(),
            path,
        },
        Some(libc::ENOTDIR) | Some(libc::EISDIR) | Some(libc::ENXIO) => {
            EngineResourceError::NotRegular {
                logical_path: logical_path.to_string(),
                path,
            }
        }
        _ => EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path,
            source,
        },
    }
}

#[cfg(not(unix))]
fn capture_explicit_resources(
    root: &Path,
) -> Result<BTreeMap<&'static str, Vec<u8>>, EngineResourceError> {
    let expected_total = EMBEDDED_ENGINE_RESOURCES
        .iter()
        .map(|(_, bytes)| bytes.len() as u64)
        .sum::<u64>();
    let mut inspected_total = 0_u64;
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let path = explicit_resource_path(root, logical_path)?;
        let metadata = fs::metadata(&path).map_err(|source| EngineResourceError::Read {
            logical_path: (*logical_path).to_string(),
            path: path.clone(),
            source,
        })?;
        if metadata.len() != expected.len() as u64 {
            return Err(EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path,
                expected: expected.len() as u64,
                actual: metadata.len(),
            });
        }
        inspected_total += metadata.len();
    }
    if inspected_total != expected_total {
        return Err(EngineResourceError::SizeMismatch {
            logical_path: ".".to_string(),
            path: root.to_path_buf(),
            expected: expected_total,
            actual: inspected_total,
        });
    }
    let mut captured = BTreeMap::new();
    for (logical_path, expected) in EMBEDDED_ENGINE_RESOURCES {
        let path = explicit_resource_path(root, logical_path)?;
        let file = fs::File::open(&path).map_err(|source| EngineResourceError::Read {
            logical_path: (*logical_path).to_string(),
            path: path.clone(),
            source,
        })?;
        let mut bytes = Vec::with_capacity(expected.len());
        file.take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| EngineResourceError::Read {
                logical_path: (*logical_path).to_string(),
                path: path.clone(),
                source,
            })?;
        if bytes.len() != expected.len() {
            return Err(EngineResourceError::SizeMismatch {
                logical_path: (*logical_path).to_string(),
                path,
                expected: expected.len() as u64,
                actual: bytes.len() as u64,
            });
        }
        captured.insert(*logical_path, bytes);
    }
    Ok(captured)
}

#[cfg(not(unix))]
fn explicit_resource_path(root: &Path, logical_path: &str) -> Result<PathBuf, EngineResourceError> {
    let mut path = root.to_path_buf();
    let root_metadata = fs::symlink_metadata(root).map_err(|source| EngineResourceError::Read {
        logical_path: logical_path.to_string(),
        path: root.to_path_buf(),
        source,
    })?;
    if root_metadata.file_type().is_symlink() {
        return Err(EngineResourceError::Symlink {
            logical_path: logical_path.to_string(),
            path,
        });
    }
    if !root_metadata.is_dir() {
        return Err(EngineResourceError::NotRegular {
            logical_path: logical_path.to_string(),
            path,
        });
    }
    let components = Path::new(logical_path).components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(EngineResourceError::UnsafeLogicalPath(
                logical_path.to_string(),
            ));
        };
        path.push(component);
        let metadata = fs::symlink_metadata(&path).map_err(|source| EngineResourceError::Read {
            logical_path: logical_path.to_string(),
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EngineResourceError::Symlink {
                logical_path: logical_path.to_string(),
                path,
            });
        }
        let is_last = index + 1 == components.len();
        if (is_last && !metadata.is_file()) || (!is_last && !metadata.is_dir()) {
            return Err(EngineResourceError::NotRegular {
                logical_path: logical_path.to_string(),
                path,
            });
        }
    }
    Ok(path)
}

// TEST SUPPORT ONLY — P6.06 (edge identity/enrollment tests).
//
// A throwaway-directory wrapper around the
// [`crate::enrollment::FilesystemCredentialStore`] for tests: every instance
// gets a UNIQUE temporary directory (process-unique counter + label), the
// directory is removed on drop, and the store file receives the same
// owner-only permission treatment the production store enforces on Unix.
//
// This module ships NO cryptographic material and NO production behaviour —
// it exists so both in-module (`#[cfg(test)]`) and integration
// (`crates/edge-agent/tests/`) tests share one honest harness instead of
// inventing per-file temp logic. Never composed by the binary
// (`main.rs` does not reference this module's types).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::enrollment::{DeviceCredentialRecord, FilesystemCredentialStore};
use crate::ports::EdgeError;

static COUNTER: AtomicU64 = AtomicU64::new(1);

/// A unique temporary directory per instance (tests may run in parallel).
fn unique_dir(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    std::env::temp_dir().join(format!(
        "flexcore-edge-test-{label}-{pid}-{unique}"
    ))
}

/// A [`FilesystemCredentialStore`] rooted in a throwaway temp directory.
pub struct TempFileCredentialStore {
    inner: FilesystemCredentialStore,
    dir: PathBuf,
}

impl TempFileCredentialStore {
    /// Builds the store with a unique temp directory named after `label`.
    /// Panics only if the platform cannot create a temp directory, which is
    /// an environment failure, not a test-logic condition.
    pub fn new(label: &str) -> Self {
        let dir = unique_dir(label);
        std::fs::create_dir_all(&dir).expect("temp directory is creatable");
        let path = dir.join("device-identity.conf");
        Self {
            inner: FilesystemCredentialStore::new(path),
            dir,
        }
    }

    /// The wrapped store (pass to the flow under test).
    pub fn inner(&mut self) -> &mut FilesystemCredentialStore {
        &mut self.inner
    }

    /// Convenience: stores a record through the wrapped store.
    pub fn put(&mut self, record: &DeviceCredentialRecord) -> Result<(), EdgeError> {
        self.inner.store(record)
    }

    /// The directory backing this instance (assertions may name the path;
    /// contents are never logged).
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }
}

impl Drop for TempFileCredentialStore {
    fn drop(&mut self) {
        // Best-effort cleanup; a leaked temp dir must never fail a test
        // silently — but neither may cleanup mask the test outcome.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#![forbid(unsafe_code)]

//! Pinned offline workspace lock schema (`workspace-lock.toml`) and manifest output (`workspace-manifest.json`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors returned while verifying or assembling an offline developer workspace.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    /// Lockfile failed parsing or schema validation.
    #[error("invalid workspace lockfile: {0}")]
    InvalidLock(String),
    /// Pinned component checkout or file is missing, dirty, or has a mismatched SHA-256 digest.
    #[error("offline pin verification failed: {0}")]
    PinMismatch(String),
    /// Destination path points inside a game installation or already exists.
    #[error("workspace output refused: {0}")]
    OutputRefused(String),
    /// Filesystem I/O failure.
    #[error("I/O error: {0}")]
    Io(String),
}

/// One pinned file inside a component of `workspace-lock.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedComponentFile {
    /// Relative path inside the component checkout.
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    /// Target workspace subdirectory category (`sdk`, `base`, `authoring`, `mcp`).
    #[serde(rename = "targetCategory")]
    pub target_category: String,
    /// Optional explicit SHA-256 digest pin; verified when present and always recorded in `workspace-manifest.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// One pinned component repository in `workspace-lock.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedComponent {
    /// Logical component identifier (`simoder-sdk`, `simoder-base`, `simoder-authoring`, `simoder-mcp`).
    pub id: String,
    /// Canonical repository slug.
    pub repository: String,
    /// Relative folder name under the prepared workspace parent directory.
    #[serde(rename = "relativeSourceDir")]
    pub relative_source_dir: String,
    /// Pinned semantic version string.
    pub version: String,
    /// Optional explicit Git commit revision pin (`40` hex chars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Pinned files belonging to this component.
    #[serde(rename = "pinnedFiles")]
    pub pinned_files: Vec<PinnedComponentFile>,
}

/// Top-level `workspace-lock.toml` configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLock {
    /// Lockfile schema version (must be `1`).
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Workspace profile version (`0.2.0-beta`).
    #[serde(rename = "workspaceVersion")]
    pub workspace_version: String,
    /// Pinned component declarations.
    pub components: Vec<PinnedComponent>,
}

/// Emitted component record inside `workspace-manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssembledComponentRecord {
    /// Component identifier.
    pub id: String,
    /// Upstream repository slug.
    pub repository: String,
    /// Component version.
    pub version: String,
    /// Observed or pinned Git commit revision (`40` hex chars or `"pinned-tree"`).
    pub revision: String,
    /// Map of relative workspace output path -> SHA-256 hex digest.
    pub files: BTreeMap<String, String>,
}

/// Deterministic `workspace-manifest.json` written to the root of an assembled offline workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssembledWorkspaceManifest {
    /// Schema version (`1`).
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Workspace version from lock.
    #[serde(rename = "workspaceVersion")]
    pub workspace_version: String,
    /// SHA-256 hex digest of the input `workspace-lock.toml`.
    #[serde(rename = "lockSha256")]
    pub lock_sha256: String,
    /// Explicitly confirms that assembly ran 100% offline with zero network fetch/upgrade.
    #[serde(rename = "offlineOnly")]
    pub offline_only: bool,
    /// Explicitly confirms that the host workspace tool is excluded from game installations.
    #[serde(rename = "hostToolOnly")]
    pub host_tool_only: bool,
    /// Verified components with their revisions and file SHA-256 maps.
    pub components: Vec<AssembledComponentRecord>,
}

/// Load and validate a `workspace-lock.toml` file from disk.
pub fn load_workspace_lock(lock_path: &Path) -> Result<(WorkspaceLock, String), WorkspaceError> {
    let bytes = fs::read(lock_path).map_err(|e| WorkspaceError::Io(e.to_string()))?;
    let lock_sha = hex::encode(Sha256::digest(&bytes));
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| WorkspaceError::InvalidLock("lockfile is not valid UTF-8".to_string()))?;
    let lock: WorkspaceLock =
        toml::from_str(text).map_err(|e| WorkspaceError::InvalidLock(e.to_string()))?;
    if lock.schema_version != 1 {
        return Err(WorkspaceError::InvalidLock(format!(
            "unsupported schemaVersion: {}",
            lock.schema_version
        )));
    }
    if lock.components.is_empty() {
        return Err(WorkspaceError::InvalidLock(
            "workspace lockfile must pin at least one component".to_string(),
        ));
    }
    for comp in &lock.components {
        if comp.id.is_empty()
            || comp.relative_source_dir.is_empty()
            || comp.relative_source_dir.contains("..")
        {
            return Err(WorkspaceError::InvalidLock(format!(
                "invalid component definition: {}",
                comp.id
            )));
        }
        if comp.pinned_files.is_empty() {
            return Err(WorkspaceError::InvalidLock(format!(
                "component '{}' has no pinnedFiles",
                comp.id
            )));
        }
    }
    Ok((lock, lock_sha))
}

#![forbid(unsafe_code)]

//! Closed packaging and USTAR verification limits for `Simoder-Base`.

/// Minimum number of regular entries required in a valid `Simoder-Base` USTAR candidate (`LICENSE`, `mod.toml`, `scripts/init.luau`).
pub const MIN_ENTRIES: usize = 3;

/// Maximum number of regular entries permitted in a `Simoder-Base` USTAR candidate.
pub const MAX_ENTRIES: usize = 65;

/// Minimum byte length of a valid USTAR archive (at least two 512-byte blocks).
pub const MIN_ARTIFACT_BYTES: usize = 1024;

/// Maximum byte length of a valid `Simoder-Base` USTAR archive (`8 MiB`).
pub const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;

/// Maximum aggregate executable source bytes (`mod.toml` + `scripts/**/*.luau`, `4 MiB`).
pub const MAX_EXECUTABLE_BYTES: usize = 4 * 1024 * 1024;

/// Maximum byte length of the `LICENSE` notice member (`64 KiB`).
pub const MAX_NOTICE_BYTES: usize = 64 * 1024;

/// Maximum relative path byte length inside the archive.
pub const MAX_RELATIVE_PATH_LEN: usize = 240;

/// USTAR block size in bytes.
pub const USTAR_BLOCK_SIZE: usize = 512;

/// Expected manifest identifier for `Simoder-Base`.
pub const EXPECTED_BASE_MOD_ID: &str = "simoder.base";

/// Expected authoring format string in `mod.toml`.
pub const EXPECTED_AUTHORING_FORMAT: &str = "simoder.authoring";

/// Provenance format identifier emitted in `base.provenance.json`.
pub const PROVENANCE_FORMAT: &str = "simoder.base.artifact.candidate";

/// Provenance trust marker explicitly stating that build candidates are not runtime trust.
pub const PROVENANCE_TRUST_MARKER: &str = "unapproved-build-candidate";

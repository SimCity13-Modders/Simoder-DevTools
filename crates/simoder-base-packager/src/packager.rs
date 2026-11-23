#![forbid(unsafe_code)]

//! Deterministic offline `Simoder-Base` USTAR packager and provenance generator.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::config::{
    EXPECTED_AUTHORING_FORMAT, EXPECTED_BASE_MOD_ID, MAX_ARTIFACT_BYTES, MAX_ENTRIES,
    MAX_EXECUTABLE_BYTES, MAX_NOTICE_BYTES, MAX_RELATIVE_PATH_LEN, MIN_ARTIFACT_BYTES, MIN_ENTRIES,
    PROVENANCE_FORMAT, PROVENANCE_TRUST_MARKER, USTAR_BLOCK_SIZE,
};

/// Errors returned during offline `Simoder-Base` packaging or verification.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BasePackageError {
    /// Selector JSON or file path violated the closed grammar.
    #[error("selector validation failed: {0}")]
    InvalidSelector(String),
    /// Anchored script directory inventory differed from explicit `package-files.json` selection.
    #[error("script inventory mismatch: {0}")]
    InventoryMismatch(String),
    /// Selected `mod.toml` failed `Simoder-Base` schema validation.
    #[error("invalid Base manifest: {0}")]
    InvalidManifest(String),
    /// Output directory already exists or is not a fresh immediate child.
    #[error("output path refused: {0}")]
    OutputRefused(String),
    /// Emitted or verified USTAR archive violated strict Core reader bounds.
    #[error("USTAR archive verification refused: {0}")]
    ArchiveVerificationFailed(String),
    /// Underlying filesystem error.
    #[error("I/O error: {0}")]
    Io(String),
}

/// One entry in `package-files.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSelectorEntry {
    /// Canonical relative path (`LICENSE`, `mod.toml`, or `scripts/**/*.luau`).
    pub path: String,
    /// True only for the single required `LICENSE` notice member.
    pub notice: bool,
}

/// Top-level `package-files.json` selector schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSelector {
    /// Selector schema version (must be `1`).
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Explicit list of source members.
    pub files: Vec<PackageSelectorEntry>,
}

/// Provenance record for one captured file inside `base.provenance.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedFileProvenance {
    /// Canonical relative path inside `base.tar`.
    pub path: String,
    /// Exact byte length of the captured member.
    pub size: usize,
    /// Lowercase SHA-256 hex digest of the member bytes.
    pub sha256: String,
    /// Whether this member is the license notice.
    pub notice: bool,
}

/// Artifact summary recorded in `base.provenance.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDescriptor {
    /// Archive filename (`base.tar`).
    pub name: String,
    /// Exact byte size of `base.tar` (multiple of 512).
    pub size: usize,
    /// Lowercase SHA-256 hex digest of `base.tar`.
    pub sha256: String,
}

/// Provenance sidecar (`base.provenance.json`) emitted beside `base.tar`.
///
/// Explicitly marked `unapproved-build-candidate` with `requires_independent_selection = true`
/// so that build output is never mistaken for runtime trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseCandidateProvenance {
    /// Schema version (`1`).
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Candidate format (`simoder.base.artifact.candidate`).
    pub format: String,
    /// Trust marker (`unapproved-build-candidate`).
    pub trust: String,
    /// Always `true`: runtime promotion requires independent native pin selection.
    #[serde(rename = "requiresIndependentSelection")]
    pub requires_independent_selection: bool,
    /// Version extracted from `mod.toml`.
    pub version: String,
    /// SHA-256 digest of the selector bytes (`package-files.json`).
    #[serde(rename = "selectionSha256")]
    pub selection_sha256: String,
    /// Summary of the emitted `base.tar`.
    pub artifact: ArtifactDescriptor,
    /// Captured file digests in canonical order.
    pub files: Vec<CapturedFileProvenance>,
}

/// Validate a canonical relative path under the `Simoder-Base` finite path grammar.
pub fn is_valid_relative_name(path: &str) -> bool {
    if path.is_empty() || path.len() > MAX_RELATIVE_PATH_LEN {
        return false;
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 16 {
        return false;
    }
    for part in parts {
        if part.is_empty()
            || part.len() > 100
            || part == "."
            || part == ".."
            || part.ends_with('.')
            || part.ends_with(' ')
        {
            return false;
        }
        if part
            .chars()
            .any(|c| (c as u32) < 33 || (c as u32) > 126 || "\\:*?\"<>|".contains(c))
        {
            return false;
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_lowercase();
        if matches!(stem.as_str(), "con" | "prn" | "aux" | "nul") {
            return false;
        }
        if stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0'
        {
            return false;
        }
    }
    true
}

/// Parse and validate `package-files.json`, returning sorted `(path, notice)` pairs.
pub fn parse_and_validate_selector(
    selector_bytes: &[u8],
) -> Result<Vec<(String, bool)>, BasePackageError> {
    let text = std::str::from_utf8(selector_bytes)
        .map_err(|_| BasePackageError::InvalidSelector("invalid UTF-8".to_string()))?;
    reject_duplicate_json_keys(text)?;
    let selector: PackageSelector = serde_json::from_str(text)
        .map_err(|e| BasePackageError::InvalidSelector(e.to_string()))?;
    if selector.schema_version != 1 {
        return Err(BasePackageError::InvalidSelector(
            "unsupported schemaVersion".to_string(),
        ));
    }
    if !(MIN_ENTRIES..=MAX_ENTRIES).contains(&selector.files.len()) {
        return Err(BasePackageError::InvalidSelector(
            "selector entry count out of bounds".to_string(),
        ));
    }
    let mut folded = BTreeSet::new();
    let mut result = Vec::with_capacity(selector.files.len());
    let mut notices = 0usize;
    for entry in selector.files {
        if !is_valid_relative_name(&entry.path) {
            return Err(BasePackageError::InvalidSelector(format!(
                "invalid relative path: {}",
                entry.path
            )));
        }
        let lower = entry.path.to_ascii_lowercase();
        if !folded.insert(lower) {
            return Err(BasePackageError::InvalidSelector(
                "duplicate folded selector member".to_string(),
            ));
        }
        if entry.notice {
            if entry.path != "LICENSE" {
                return Err(BasePackageError::InvalidSelector(
                    "unsupported notice member".to_string(),
                ));
            }
            notices += 1;
        } else {
            let valid_exec = entry.path == "mod.toml"
                || (entry.path.starts_with("scripts/") && entry.path.ends_with(".luau"))
                || (entry.path.starts_with("ui/") && (entry.path.ends_with(".js") || entry.path.ends_with(".ts") || entry.path.ends_with(".d.ts")));
            if !valid_exec {
                return Err(BasePackageError::InvalidSelector(
                    "unsupported executable selector".to_string(),
                ));
            }
            if !entry.path.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '/' | '.' | '-' | '_')
            }) {
                return Err(BasePackageError::InvalidSelector(
                    "noncanonical executable member".to_string(),
                ));
            }
        }
        result.push((entry.path, entry.notice));
    }
    if notices != 1 || !folded.contains("mod.toml") || !folded.contains("scripts/init.luau") {
        return Err(BasePackageError::InvalidSelector(
            "required Base members absent".to_string(),
        ));
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

/// Build a deterministic `Simoder-Base` USTAR archive and provenance sidecar into a fresh output directory.
pub fn build_base_candidate(
    source_root: &Path,
    selector_name: &str,
    output_dir: &Path,
) -> Result<BaseCandidateProvenance, BasePackageError> {
    if !is_valid_relative_name(selector_name)
        || selector_name.contains('/')
        || !selector_name.ends_with(".json")
    {
        return Err(BasePackageError::InvalidSelector(
            "unknown selector filename".to_string(),
        ));
    }
    validate_no_symlink_or_traversal(source_root)?;
    if output_dir.exists() {
        return Err(BasePackageError::OutputRefused(
            "output directory must not already exist".to_string(),
        ));
    }
    let output_leaf = output_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| BasePackageError::OutputRefused("invalid output leaf".to_string()))?;
    if !is_valid_relative_name(output_leaf)
        || output_leaf.contains('/')
        || output_leaf.contains('\\')
    {
        return Err(BasePackageError::OutputRefused(
            "invalid fresh output directory name".to_string(),
        ));
    }
    let output_parent = output_dir
        .parent()
        .ok_or_else(|| BasePackageError::OutputRefused("missing output parent".to_string()))?;
    validate_no_symlink_or_traversal(output_parent)?;

    let selector_path = source_root.join(selector_name);
    ensure_regular_non_symlink(&selector_path)?;
    let selector_bytes =
        fs::read(&selector_path).map_err(|e| BasePackageError::Io(e.to_string()))?;
    if selector_bytes.len() > MAX_NOTICE_BYTES {
        return Err(BasePackageError::InvalidSelector(
            "selector exceeds 64 KiB".to_string(),
        ));
    }
    let selected = parse_and_validate_selector(&selector_bytes)?;

    let discovered_scripts = inventory_scripts_tree(source_root)?;
    let expected_scripts: BTreeSet<String> = selected
        .iter()
        .filter(|(p, _)| p.starts_with("scripts/"))
        .map(|(p, _)| p.clone())
        .collect();
    if discovered_scripts != expected_scripts {
        return Err(BasePackageError::InventoryMismatch(
            "anchored script tree differs from explicit selection".to_string(),
        ));
    }

    let mut captured: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut executable_bytes = 0usize;
    let mut records = Vec::with_capacity(selected.len());

    for (path, notice) in &selected {
        let full_path = source_root.join(path);
        ensure_regular_non_symlink(&full_path)?;
        let data = fs::read(&full_path).map_err(|e| BasePackageError::Io(e.to_string()))?;
        let max_len = if *notice {
            MAX_NOTICE_BYTES
        } else {
            MAX_EXECUTABLE_BYTES
        };
        if data.len() > max_len {
            return Err(BasePackageError::InvalidSelector(format!(
                "member {path} exceeds size limit"
            )));
        }
        if *notice && data.is_empty() {
            return Err(BasePackageError::InvalidSelector(
                "required LICENSE notice is empty".to_string(),
            ));
        }
        if !*notice {
            executable_bytes += data.len();
            if executable_bytes > MAX_EXECUTABLE_BYTES {
                return Err(BasePackageError::InvalidSelector(
                    "aggregate executable bytes exceed 4 MiB".to_string(),
                ));
            }
        }
        let digest = hex::encode(Sha256::digest(&data));
        records.push(CapturedFileProvenance {
            path: path.clone(),
            size: data.len(),
            sha256: digest,
            notice: *notice,
        });
        captured.insert(path.clone(), data);
    }

    let mod_toml_bytes = captured
        .get("mod.toml")
        .ok_or_else(|| BasePackageError::InvalidManifest("missing mod.toml".to_string()))?;
    let version = validate_base_manifest(mod_toml_bytes)?;

    let archive_bytes = encode_deterministic_ustar(&selected, &captured)?;
    if !(MIN_ARTIFACT_BYTES..=MAX_ARTIFACT_BYTES).contains(&archive_bytes.len())
        || !archive_bytes.len().is_multiple_of(USTAR_BLOCK_SIZE)
    {
        return Err(BasePackageError::ArchiveVerificationFailed(
            "archive byte length out of bounds".to_string(),
        ));
    }

    let provenance = BaseCandidateProvenance {
        schema_version: 1,
        format: PROVENANCE_FORMAT.to_string(),
        trust: PROVENANCE_TRUST_MARKER.to_string(),
        requires_independent_selection: true,
        version,
        selection_sha256: hex::encode(Sha256::digest(&selector_bytes)),
        artifact: ArtifactDescriptor {
            name: "base.tar".to_string(),
            size: archive_bytes.len(),
            sha256: hex::encode(Sha256::digest(&archive_bytes)),
        },
        files: records,
    };

    let provenance_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&provenance)
            .map_err(|e| BasePackageError::Io(e.to_string()))?
    );

    fs::create_dir(output_dir).map_err(|e| BasePackageError::OutputRefused(e.to_string()))?;
    fs::write(output_dir.join("base.tar"), &archive_bytes)
        .map_err(|e| BasePackageError::Io(e.to_string()))?;
    fs::write(
        output_dir.join("base.provenance.json"),
        provenance_json.as_bytes(),
    )
    .map_err(|e| BasePackageError::Io(e.to_string()))?;

    Ok(provenance)
}

fn validate_base_manifest(mod_toml_bytes: &[u8]) -> Result<String, BasePackageError> {
    let text = std::str::from_utf8(mod_toml_bytes)
        .map_err(|_| BasePackageError::InvalidManifest("invalid UTF-8 in mod.toml".to_string()))?;
    let doc: toml::Value =
        toml::from_str(text).map_err(|e| BasePackageError::InvalidManifest(e.to_string()))?;
    let schema_version = doc.get("schemaVersion").and_then(toml::Value::as_integer);
    let format = doc.get("format").and_then(toml::Value::as_str);
    let id = doc
        .get("metadata")
        .and_then(|m| m.get("id"))
        .and_then(toml::Value::as_str);
    let version = doc
        .get("metadata")
        .and_then(|m| m.get("version"))
        .and_then(toml::Value::as_str);
    if schema_version != Some(2)
        || format != Some(EXPECTED_AUTHORING_FORMAT)
        || id != Some(EXPECTED_BASE_MOD_ID)
    {
        return Err(BasePackageError::InvalidManifest(
            "unexpected schemaVersion, format, or metadata.id".to_string(),
        ));
    }
    let version = version
        .ok_or_else(|| BasePackageError::InvalidManifest("missing metadata.version".to_string()))?;
    if version.is_empty()
        || version.len() > 64
        || version
            .chars()
            .any(|c| (c as u32) < 33 || (c as u32) > 126)
    {
        return Err(BasePackageError::InvalidManifest(
            "invalid metadata.version".to_string(),
        ));
    }
    Ok(version.to_string())
}

fn encode_deterministic_ustar(
    selected: &[(String, bool)],
    captured: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, BasePackageError> {
    let mut out = Vec::new();
    for (path, _) in selected {
        let data = captured
            .get(path)
            .ok_or_else(|| BasePackageError::InvalidSelector("missing captured data".to_string()))?;
        let mut header = [0u8; USTAR_BLOCK_SIZE];
        let path_bytes = path.as_bytes();
        if path_bytes.len() > 100 {
            return Err(BasePackageError::InvalidSelector(
                "USTAR name exceeds 100 bytes".to_string(),
            ));
        }
        header[..path_bytes.len()].copy_from_slice(path_bytes);
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        let size_octal = format!("{:011o}\0", data.len());
        header[124..136].copy_from_slice(size_octal.as_bytes());
        header[136..148].copy_from_slice(b"00000000000\0");
        header[148..156].fill(b' ');
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        let checksum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        let chk_octal = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(chk_octal.as_bytes());

        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        let rem = data.len() % USTAR_BLOCK_SIZE;
        if rem != 0 {
            out.resize(out.len() + (USTAR_BLOCK_SIZE - rem), 0);
        }
    }
    out.resize(out.len() + USTAR_BLOCK_SIZE * 2, 0);
    Ok(out)
}

fn inventory_scripts_tree(source_root: &Path) -> Result<BTreeSet<String>, BasePackageError> {
    for required in ["LICENSE", "mod.toml", "scripts"] {
        if !source_root.join(required).exists() {
            return Err(BasePackageError::InventoryMismatch(format!(
                "missing canonical root entry: {required}"
            )));
        }
    }
    let scripts_dir = source_root.join("scripts");
    let meta = fs::symlink_metadata(&scripts_dir)
        .map_err(|e| BasePackageError::InventoryMismatch(e.to_string()))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(BasePackageError::InventoryMismatch(
            "scripts must be an ordinary directory".to_string(),
        ));
    }
    let mut pending = vec![(scripts_dir, "scripts".to_string())];
    let mut found = BTreeSet::new();
    while let Some((dir, prefix)) = pending.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| BasePackageError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| BasePackageError::Io(e.to_string()))?;
            let file_type = entry
                .file_type()
                .map_err(|e| BasePackageError::Io(e.to_string()))?;
            if file_type.is_symlink() {
                return Err(BasePackageError::InventoryMismatch(
                    "symlink/reparse inside scripts tree refused".to_string(),
                ));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| BasePackageError::InventoryMismatch("non-UTF8 filename".to_string()))?;
            let rel = format!("{prefix}/{name}");
            if !is_valid_relative_name(&rel) {
                return Err(BasePackageError::InventoryMismatch(format!(
                    "noncanonical script tree entry: {rel}"
                )));
            }
            if file_type.is_dir() {
                pending.push((entry.path(), rel));
            } else if file_type.is_file() {
                if !rel.ends_with(".luau") || found.len() >= MAX_ENTRIES {
                    return Err(BasePackageError::InventoryMismatch(format!(
                        "unselected or excessive script member: {rel}"
                    )));
                }
                found.insert(rel);
            } else {
                return Err(BasePackageError::InventoryMismatch(
                    "non-ordinary entry in scripts tree".to_string(),
                ));
            }
        }
    }
    Ok(found)
}

fn validate_no_symlink_or_traversal(path: &Path) -> Result<(), BasePackageError> {
    let raw = path.to_string_lossy();
    if raw.split(['/', '\\']).any(|part| part == "." || part == "..") {
        return Err(BasePackageError::InvalidSelector(
            "noncanonical path with '.' or '..' refused".to_string(),
        ));
    }
    let mut cur = std::path::PathBuf::new();
    for comp in path.components() {
        cur.push(comp);
        if let Ok(meta) = fs::symlink_metadata(&cur)
            && meta.file_type().is_symlink()
        {
            return Err(BasePackageError::InvalidSelector(
                "symlink/junction in path ancestry refused".to_string(),
            ));
        }
    }
    Ok(())
}

fn ensure_regular_non_symlink(path: &Path) -> Result<(), BasePackageError> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| BasePackageError::InvalidSelector(e.to_string()))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(BasePackageError::InvalidSelector(
            "non-regular or symlink file refused".to_string(),
        ));
    }
    Ok(())
}

fn reject_duplicate_json_keys(text: &str) -> Result<(), BasePackageError> {
    let schema_matches = text.matches("\"schemaVersion\"").count();
    let files_matches = text.matches("\"files\"").count();
    if schema_matches > 1 || files_matches > 1 {
        return Err(BasePackageError::InvalidSelector(
            "duplicate selector JSON key".to_string(),
        ));
    }
    Ok(())
}

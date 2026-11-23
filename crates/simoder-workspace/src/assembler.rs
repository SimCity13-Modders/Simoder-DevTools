#![forbid(unsafe_code)]

//! Offline workspace verifier and deterministic assembler (`Simoder-DevTools#3`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use crate::lock::{
    AssembledComponentRecord, AssembledWorkspaceManifest, WorkspaceError, WorkspaceLock,
    load_workspace_lock,
};

/// Verify all pinned components and files in `lock` against `sources_root` offline without modifying disk.
pub fn verify_pinned_sources(
    lock: &WorkspaceLock,
    lock_sha256: &str,
    sources_root: &Path,
) -> Result<AssembledWorkspaceManifest, WorkspaceError> {
    if !sources_root.is_dir() {
        return Err(WorkspaceError::PinMismatch(format!(
            "sources root does not exist: {}",
            sources_root.display()
        )));
    }

    let mut assembled_components = Vec::with_capacity(lock.components.len());

    for comp in &lock.components {
        let comp_dir = sources_root.join(&comp.relative_source_dir);
        if !comp_dir.is_dir() {
            return Err(WorkspaceError::PinMismatch(format!(
                "missing prepared component directory for '{}': {} (no implicit fetch performed)",
                comp.id,
                comp_dir.display()
            )));
        }

        let observed_rev = resolve_component_revision(&comp_dir, comp.revision.as_deref())?;
        if let Some(expected_rev) = &comp.revision
            && !observed_rev.eq_ignore_ascii_case(expected_rev)
        {
            return Err(WorkspaceError::PinMismatch(format!(
                "component '{}' revision mismatch: expected {}, got {}",
                comp.id, expected_rev, observed_rev
            )));
        }

        let mut file_map = BTreeMap::new();
        for pinned in &comp.pinned_files {
            validate_clean_relative_path(&pinned.relative_path)?;
            let src_file = comp_dir.join(&pinned.relative_path);
            let meta = fs::symlink_metadata(&src_file).map_err(|_| {
                WorkspaceError::PinMismatch(format!(
                    "missing pinned file '{}' in component '{}'",
                    pinned.relative_path, comp.id
                ))
            })?;
            if meta.file_type().is_symlink() || !meta.is_file() {
                return Err(WorkspaceError::PinMismatch(format!(
                    "pinned file '{}' in '{}' must be an ordinary file",
                    pinned.relative_path, comp.id
                )));
            }
            let bytes = fs::read(&src_file).map_err(|e| WorkspaceError::Io(e.to_string()))?;
            let actual_sha = hex::encode(Sha256::digest(&bytes));
            if let Some(expected_sha) = &pinned.sha256
                && !actual_sha.eq_ignore_ascii_case(expected_sha)
            {
                return Err(WorkspaceError::PinMismatch(format!(
                    "hash drift for '{}/{}': expected {}, got {}",
                    comp.id, pinned.relative_path, expected_sha, actual_sha
                )));
            }
            let workspace_rel = format!(
                "{}/{}/{}",
                pinned.target_category, comp.id, pinned.relative_path
            );
            file_map.insert(workspace_rel, actual_sha);
        }

        assembled_components.push(AssembledComponentRecord {
            id: comp.id.clone(),
            repository: comp.repository.clone(),
            version: comp.version.clone(),
            revision: observed_rev,
            files: file_map,
        });
    }

    Ok(AssembledWorkspaceManifest {
        schema_version: 1,
        workspace_version: lock.workspace_version.clone(),
        lock_sha256: lock_sha256.to_string(),
        offline_only: true,
        host_tool_only: true,
        components: assembled_components,
    })
}

/// Assemble a deterministic offline developer workspace into a fresh `output_dir`, writing
/// all pinned files and `workspace-manifest.json`.
///
/// Refuses to write into any directory belonging to a `SimCity.exe` game installation.
pub fn assemble_offline_workspace(
    lock_path: &Path,
    sources_root: &Path,
    output_dir: &Path,
) -> Result<AssembledWorkspaceManifest, WorkspaceError> {
    reject_if_inside_game_installation(output_dir)?;
    if output_dir.exists() {
        return Err(WorkspaceError::OutputRefused(format!(
            "output workspace directory must be fresh and not already exist: {}",
            output_dir.display()
        )));
    }

    let (lock, lock_sha) = load_workspace_lock(lock_path)?;
    let manifest = verify_pinned_sources(&lock, &lock_sha, sources_root)?;

    fs::create_dir_all(output_dir).map_err(|e| WorkspaceError::Io(e.to_string()))?;

    for comp in &lock.components {
        let comp_dir = sources_root.join(&comp.relative_source_dir);
        for pinned in &comp.pinned_files {
            let src_file = comp_dir.join(&pinned.relative_path);
            let workspace_rel = format!(
                "{}/{}/{}",
                pinned.target_category, comp.id, pinned.relative_path
            );
            let dst_file = output_dir.join(&workspace_rel);
            if let Some(parent) = dst_file.parent() {
                fs::create_dir_all(parent).map_err(|e| WorkspaceError::Io(e.to_string()))?;
            }
            let bytes = fs::read(&src_file).map_err(|e| WorkspaceError::Io(e.to_string()))?;
            fs::write(&dst_file, &bytes).map_err(|e| WorkspaceError::Io(e.to_string()))?;
        }
    }

    let manifest_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&manifest).map_err(|e| WorkspaceError::Io(e.to_string()))?
    );
    fs::write(
        output_dir.join("workspace-manifest.json"),
        manifest_json.as_bytes(),
    )
    .map_err(|e| WorkspaceError::Io(e.to_string()))?;

    Ok(manifest)
}

/// Reject any destination directory that sits inside or overlaps a `SimCity.exe` game installation.
pub fn reject_if_inside_game_installation(output_dir: &Path) -> Result<(), WorkspaceError> {
    let mut current = Some(output_dir);
    while let Some(dir) = current {
        if dir.join("SimCity.exe").exists()
            || dir.join("SimCity/SimCity.exe").exists()
            || dir.join("SimCityUserData").exists()
        {
            return Err(WorkspaceError::OutputRefused(
                "host workspace tool must never assemble or install into a SimCity game directory"
                    .to_string(),
            ));
        }
        if let Some(name) = dir.file_name().and_then(|s| s.to_str())
            && name.eq_ignore_ascii_case("simoder.devtools")
        {
            return Err(WorkspaceError::OutputRefused(
                "host workspace assembly must not target a runtime mod directory".to_string(),
            ));
        }
        current = dir.parent();
    }
    Ok(())
}

fn resolve_component_revision(
    comp_dir: &Path,
    pinned_rev: Option<&str>,
) -> Result<String, WorkspaceError> {
    let git_head_path = comp_dir.join(".git/HEAD");
    if git_head_path.is_file()
        && let Ok(head_text) = fs::read_to_string(&git_head_path)
    {
        let trimmed = head_text.trim();
        if let Some(ref_rel) = trimmed.strip_prefix("ref: ") {
            let ref_path = comp_dir.join(".git").join(ref_rel.trim());
            if let Ok(rev) = fs::read_to_string(ref_path) {
                let rev_trimmed = rev.trim();
                if rev_trimmed.len() == 40 && rev_trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Ok(rev_trimmed.to_ascii_lowercase());
                }
            }
        } else if trimmed.len() == 40 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(trimmed.to_ascii_lowercase());
        }
    }
    Ok(pinned_rev.unwrap_or("pinned-offline-tree").to_string())
}

fn validate_clean_relative_path(rel: &str) -> Result<(), WorkspaceError> {
    if rel.is_empty() || rel.starts_with('/') || rel.starts_with('\\') || rel.contains('\\') {
        return Err(WorkspaceError::InvalidLock(format!(
            "noncanonical relative path: {rel}"
        )));
    }
    for comp in Path::new(rel).components() {
        if !matches!(comp, Component::Normal(_)) {
            return Err(WorkspaceError::InvalidLock(format!(
                "path traversal component refused: {rel}"
            )));
        }
    }
    Ok(())
}

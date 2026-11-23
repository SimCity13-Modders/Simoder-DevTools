#![forbid(unsafe_code)]

//! Integration tests for `Simoder-DevTools` covering `#3` (*Workspace: zbudować CLI składający przypięte środowisko offline*):
//! - Zero implicit fetch/upgrade;
//! - Manifest records component revisions and file SHA-256 hashes (`workspace-manifest.json`);
//! - Clean clones reproduce the exact same directory layout and byte-for-byte identical manifest;
//! - Missing or tampered pinned files/revisions fail closed offline;
//! - Tool refuses to assemble into a `SimCity.exe` game installation or runtime mod directory.

use std::fs;
use std::path::{Path, PathBuf};

use simoder_workspace::{
    WorkspaceError, assemble_offline_workspace, load_workspace_lock, verify_pinned_sources,
};
use tempfile::TempDir;
use walkdir::WalkDir;

fn devtools_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Simoder-DevTools root")
        .to_path_buf()
}

fn ecosystem_parent_root() -> PathBuf {
    devtools_repo_root()
        .parent()
        .expect("SimCity workspace root")
        .to_path_buf()
}

fn create_clean_pinned_sources_clone(
    lock_path: &Path,
    real_sources: &Path,
    clone_root: &Path,
    pinned_rev: &str,
) {
    let (lock, _) = load_workspace_lock(lock_path).expect("load lock");
    for comp in &lock.components {
        let comp_dst = clone_root.join(&comp.relative_source_dir);
        let git_dir = comp_dst.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), format!("{pinned_rev}\n")).unwrap();

        let comp_src = real_sources.join(&comp.relative_source_dir);
        for file in &comp.pinned_files {
            let src = comp_src.join(&file.relative_path);
            let dst = comp_dst.join(&file.relative_path);
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p).unwrap();
            }
            fs::copy(&src, &dst).unwrap();
        }
    }
}

#[test]
fn real_ecosystem_sources_verify_and_assemble_offline_recording_revisions_and_hashes() {
    let devtools_root = devtools_repo_root();
    let lock_path = devtools_root.join("workspace-lock.toml");
    let sources_root = ecosystem_parent_root();

    let temp = TempDir::new().expect("tempdir");
    let out_dir = temp.path().join("assembled-workspace");

    let manifest =
        assemble_offline_workspace(&lock_path, &sources_root, &out_dir).expect("assemble workspace");
    assert!(manifest.offline_only);
    assert!(manifest.host_tool_only);
    assert_eq!(manifest.components.len(), 4);

    for comp in &manifest.components {
        assert!(!comp.revision.is_empty());
        assert!(!comp.files.is_empty());
        for (rel_path, sha) in &comp.files {
            assert_eq!(sha.len(), 64);
            assert!(out_dir.join(rel_path).is_file(), "missing assembled file {rel_path}");
        }
    }
    assert!(out_dir.join("workspace-manifest.json").is_file());
}

#[test]
fn two_clean_clones_reproduce_identical_layout_and_manifest_bytes_offline() {
    let devtools_root = devtools_repo_root();
    let lock_path = devtools_root.join("workspace-lock.toml");
    let real_sources = ecosystem_parent_root();

    let temp = TempDir::new().expect("tempdir");
    let clone_a = temp.path().join("clone-a");
    let clone_b = temp.path().join("clone-b");
    let rev = "1111111111111111111111111111111111111111";

    create_clean_pinned_sources_clone(&lock_path, &real_sources, &clone_a, rev);
    create_clean_pinned_sources_clone(&lock_path, &real_sources, &clone_b, rev);

    let out_a = temp.path().join("workspace-a");
    let out_b = temp.path().join("workspace-b");

    let manifest_a = assemble_offline_workspace(&lock_path, &clone_a, &out_a).expect("assemble A");
    let manifest_b = assemble_offline_workspace(&lock_path, &clone_b, &out_b).expect("assemble B");
    assert_eq!(manifest_a, manifest_b);

    let raw_manifest_a = fs::read(out_a.join("workspace-manifest.json")).unwrap();
    let raw_manifest_b = fs::read(out_b.join("workspace-manifest.json")).unwrap();
    assert_eq!(
        raw_manifest_a, raw_manifest_b,
        "clean clones must produce byte-for-byte identical workspace-manifest.json"
    );

    let files_a: Vec<String> = WalkDir::new(&out_a)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(&out_a)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let files_b: Vec<String> = WalkDir::new(&out_b)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(&out_b)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(files_a, files_b);
}

#[test]
fn revision_mismatch_hash_drift_and_game_directory_targets_are_refused() {
    let devtools_root = devtools_repo_root();
    let lock_path = devtools_root.join("workspace-lock.toml");
    let real_sources = ecosystem_parent_root();
    let temp = TempDir::new().expect("tempdir");

    let clone = temp.path().join("clone");
    create_clean_pinned_sources_clone(
        &lock_path,
        &real_sources,
        &clone,
        "2222222222222222222222222222222222222222",
    );

    // 1. Pinned revision mismatch must fail closed
    let (mut lock_with_rev, lock_sha) = load_workspace_lock(&lock_path).unwrap();
    lock_with_rev.components[0].revision =
        Some("9999999999999999999999999999999999999999".to_string());
    assert!(matches!(
        verify_pinned_sources(&lock_with_rev, &lock_sha, &clone),
        Err(WorkspaceError::PinMismatch(_))
    ));

    // 2. Pinned file SHA-256 drift must fail closed
    let (mut lock_with_hash, _) = load_workspace_lock(&lock_path).unwrap();
    lock_with_hash.components[0].pinned_files[0].sha256 =
        Some("0000000000000000000000000000000000000000000000000000000000000000".to_string());
    assert!(matches!(
        verify_pinned_sources(&lock_with_hash, &lock_sha, &clone),
        Err(WorkspaceError::PinMismatch(_))
    ));

    // 3. Game installation directory target must be refused
    let fake_game_root = temp.path().join("SimCityInstall");
    fs::create_dir_all(fake_game_root.join("SimCity")).unwrap();
    fs::write(fake_game_root.join("SimCity/SimCity.exe"), b"MZ").unwrap();
    let inside_game = fake_game_root.join("workspace-out");
    assert!(matches!(
        assemble_offline_workspace(&lock_path, &clone, &inside_game),
        Err(WorkspaceError::OutputRefused(_))
    ));
    assert!(!inside_game.exists());
}

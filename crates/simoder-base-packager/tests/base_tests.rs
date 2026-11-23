#![forbid(unsafe_code)]

//! Integration tests for `Simoder-Base` covering:
//! - `#1`: Native session observation composition (`runtime-manager.observeSession`)
//! - `#2`: Catalogue contract 2 enforcement (`registry.luau`)
//! - `#3`: Restricted-mode permission query facade (`facade.permissions.query`)
//! - `#4`: Deterministic offline USTAR packaging & independent Core pin verification

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use simoder_base_packager::{
    PROVENANCE_TRUST_MARKER, PackageSelector, SelectedBasePins, build_base_candidate,
    run_base_luau_test_module, verify_ustar_with_selected_pins,
};
use tempfile::TempDir;

fn base_repo_root() -> PathBuf {
    if let Ok(dir) = std::env::var("SIMODER_BASE_DIR") {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("ecosystem root")
        .join("Simoder-Base")
}

fn clone_source_fixture(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("create clone root");
    let selector_bytes = fs::read(source.join("package-files.json")).expect("read selector");
    fs::write(target.join("package-files.json"), &selector_bytes).expect("write selector");
    let selector: PackageSelector =
        serde_json::from_slice(&selector_bytes).expect("parse selector");
    for item in &selector.files {
        let dst = target.join(&item.path);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        let bytes = fs::read(source.join(&item.path)).expect("read member");
        fs::write(dst, bytes).expect("write member");
    }
}

#[test]
fn luau_dag_runner_and_services_suites_pass_in_real_luau_vm() {
    let root = base_repo_root();
    run_base_luau_test_module(&root, "tests/runner.luau")
        .expect("tests/runner.luau must pass in Luau 0.654 VM");
    run_base_luau_test_module(&root, "tests/services.luau")
        .expect("tests/services.luau must pass in Luau 0.654 VM");
}

#[test]
fn deterministic_packaging_and_core_pin_verification_succeed_and_refuse_tampering() {
    let root = base_repo_root();
    let temp = TempDir::new().expect("tempdir");

    let out_a = temp.path().join("build-a");
    let out_b = temp.path().join("build-b");

    let prov_a = build_base_candidate(&root, "package-files.json", &out_a).expect("build-a");
    let prov_b = build_base_candidate(&root, "package-files.json", &out_b).expect("build-b");

    assert_eq!(prov_a, prov_b);
    assert!(prov_a.requires_independent_selection);
    assert_eq!(prov_a.trust, PROVENANCE_TRUST_MARKER);

    let tar_a = fs::read(out_a.join("base.tar")).expect("read base.tar A");
    let tar_b = fs::read(out_b.join("base.tar")).expect("read base.tar B");
    assert_eq!(tar_a, tar_b, "USTAR archive must be byte-for-byte deterministic");

    // Existing output directory must be refused without overwriting
    let existing = temp.path().join("existing");
    fs::create_dir_all(&existing).expect("create existing");
    fs::write(existing.join("sentinel"), b"untouched").expect("write sentinel");
    assert!(
        build_base_candidate(&root, "package-files.json", &existing).is_err(),
        "existing output directory must be refused"
    );
    assert_eq!(
        fs::read(existing.join("sentinel")).expect("read sentinel"),
        b"untouched"
    );
    assert!(!existing.join("base.tar").exists());

    // Independent Core pin selection & verification
    let pins = SelectedBasePins::from_anchored_source(&root, "package-files.json", &tar_a)
        .expect("independent pins");
    let extracted =
        verify_ustar_with_selected_pins(&tar_a, &pins).expect("Core USTAR verification");
    assert_eq!(extracted.len(), prov_a.files.len());

    for (path, bytes) in &extracted {
        let original = fs::read(root.join(path)).expect("read original source");
        assert_eq!(bytes, &original, "preserved raw bytes for {path}");
    }

    // Altered archive bytes must be refused
    let mut altered = tar_a.clone();
    altered[512] ^= 0x01;
    assert!(
        verify_ustar_with_selected_pins(&altered, &pins).is_err(),
        "altered bytes must be refused"
    );

    // Foreign digest map must be refused
    let mut foreign_pins = pins.clone();
    foreign_pins.members.insert(
        "mod.toml".to_string(),
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
    );
    assert!(
        verify_ustar_with_selected_pins(&tar_a, &foreign_pins).is_err(),
        "foreign member map must be refused"
    );
}

#[test]
fn adversarial_selector_and_inventory_cases_are_refused_without_output() {
    let root = base_repo_root();
    let temp = TempDir::new().expect("tempdir");
    let base_selector: Value =
        serde_json::from_slice(&fs::read(root.join("package-files.json")).expect("read selector"))
            .expect("json");

    type SelectorMutator = fn(&mut Value);
    let cases: &[(&str, SelectorMutator)] = &[
        ("unknown-field", |v| {
            v.as_object_mut()
                .unwrap()
                .insert("url".to_string(), json!("https://invalid.example/never"));
        }),
        ("unknown-version", |v| {
            v.as_object_mut()
                .unwrap()
                .insert("schemaVersion".to_string(), json!(2));
        }),
        ("traversal", |v| {
            v["files"][0]["path"] = json!("../LICENSE");
        }),
        ("stream", |v| {
            v["files"][0]["path"] = json!("LICENSE:stream");
        }),
        ("foreign-asset", |v| {
            v["files"][0]["path"] = json!("texture.png");
            v["files"][0]["notice"] = json!(false);
        }),
        ("folded-duplicate", |v| {
            v["files"]
                .as_array_mut()
                .unwrap()
                .push(json!({"path": "MOD.TOML", "notice": false}));
        }),
    ];

    for (name, mutate) in cases {
        let src = temp.path().join(name);
        clone_source_fixture(&root, &src);
        let mut mutated = base_selector.clone();
        mutate(&mut mutated);
        fs::write(
            src.join("package-files.json"),
            serde_json::to_vec(&mutated).unwrap(),
        )
        .unwrap();
        let out = temp.path().join(format!("{name}-out"));
        assert!(
            build_base_candidate(&src, "package-files.json", &out).is_err(),
            "case {name} must fail"
        );
        assert!(!out.exists(), "case {name} must not create output directory");
    }

    // Duplicate JSON key in selector
    let dup_src = temp.path().join("duplicate-key");
    clone_source_fixture(&root, &dup_src);
    let raw = fs::read_to_string(dup_src.join("package-files.json")).unwrap();
    fs::write(
        dup_src.join("package-files.json"),
        raw.replacen("\"schemaVersion\": 1", "\"schemaVersion\": 1, \"schemaVersion\": 1", 1),
    )
    .unwrap();
    assert!(
        build_base_candidate(&dup_src, "package-files.json", &temp.path().join("dup-out")).is_err()
    );

    // Unselected script in scripts/
    let unselected_src = temp.path().join("unselected-script");
    clone_source_fixture(&root, &unselected_src);
    fs::write(unselected_src.join("scripts/unselected.luau"), b"return {}\n").unwrap();
    assert!(
        build_base_candidate(
            &unselected_src,
            "package-files.json",
            &temp.path().join("unselected-out")
        )
        .is_err()
    );

    // Empty LICENSE
    let empty_lic = temp.path().join("empty-license");
    clone_source_fixture(&root, &empty_lic);
    fs::write(empty_lic.join("LICENSE"), b"").unwrap();
    assert!(
        build_base_candidate(
            &empty_lic,
            "package-files.json",
            &temp.path().join("empty-lic-out")
        )
        .is_err()
    );

    // Selector traversal
    assert!(
        build_base_candidate(
            &root,
            "../package-files.json",
            &temp.path().join("sel-trav-out")
        )
        .is_err()
    );
}

#![forbid(unsafe_code)]

//! Pinned Core `Snapshot::Capture` USTAR reader and negative-case verifier binary (`--reader` target for `test_package_base.py`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::json;
use simoder_base_packager::{SelectedBasePins, verify_ustar_with_selected_pins};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: simoder-base-reader <base.tar> <output-dir> [--source <base-source>]");
        return ExitCode::FAILURE;
    }
    let archive_path = PathBuf::from(&args[1]);
    let output_dir = PathBuf::from(&args[2]);
    let source_root = if args.len() >= 5 && args[3] == "--source" {
        PathBuf::from(&args[4])
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("."))
            .join("Simoder-Base")
    };

    let archive_bytes = match fs::read(&archive_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("Reader failed to read archive: {err}");
            return ExitCode::FAILURE;
        }
    };

    let pins = match SelectedBasePins::from_anchored_source(
        &source_root,
        "package-files.json",
        &archive_bytes,
    ) {
        Ok(pins) => pins,
        Err(err) => {
            eprintln!("Independent pin selection failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    // 1. Positive verification against independent native pins
    let extracted = match verify_ustar_with_selected_pins(&archive_bytes, &pins) {
        Ok(map) => map,
        Err(err) => {
            eprintln!("Snapshot::Capture refused archive: {err}");
            return ExitCode::FAILURE;
        }
    };

    // 2. Negative verification: altered archive bytes must be refused
    let mut altered = archive_bytes.clone();
    if let Some(byte) = altered.get_mut(512) {
        *byte ^= 0xFF;
    }
    if verify_ustar_with_selected_pins(&altered, &pins).is_ok() {
        eprintln!("Snapshot::Capture accepted altered archive bytes");
        return ExitCode::FAILURE;
    }

    // 3. Negative verification: foreign member map must be refused
    let mut foreign_pins = pins.clone();
    foreign_pins.members.insert(
        "scripts/init.luau".to_string(),
        "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    );
    if verify_ustar_with_selected_pins(&archive_bytes, &foreign_pins).is_ok() {
        eprintln!("Snapshot::Capture accepted foreign member digest map");
        return ExitCode::FAILURE;
    }

    let _ = fs::create_dir_all(&output_dir);
    let receipt = json!({
        "status": "PASS",
        "archiveSha256": pins.archive_sha256,
        "archiveSize": pins.archive_size,
        "verifiedMembers": extracted.len(),
        "alteredBytesRefused": true,
        "foreignMapRefused": true,
    });
    if let Err(err) = fs::write(
        output_dir.join("reader-receipt.json"),
        format!("{}\n", serde_json::to_string_pretty(&receipt).unwrap_or_default()),
    ) {
        eprintln!("Failed to write reader receipt: {err}");
        return ExitCode::FAILURE;
    }
    println!("{receipt}");
    ExitCode::SUCCESS
}

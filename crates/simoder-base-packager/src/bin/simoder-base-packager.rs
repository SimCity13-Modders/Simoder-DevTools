#![forbid(unsafe_code)]

//! Offline CLI builder for `Simoder-Base` deterministic USTAR candidates.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;
use simoder_base_packager::build_base_candidate;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut source: Option<PathBuf> = None;
    let mut selection = "package-files.json".to_string();
    let mut output: Option<PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--source" if i + 1 < args.len() => {
                source = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--selection" if i + 1 < args.len() => {
                selection = args[i + 1].clone();
                i += 2;
            }
            "--output" if i + 1 < args.len() => {
                output = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            _ => {
                eprintln!(
                    "Usage: simoder-base-packager --source <dir> [--selection package-files.json] --output <fresh-dir>"
                );
                return ExitCode::FAILURE;
            }
        }
    }

    let (Some(source), Some(output)) = (source, output) else {
        eprintln!("Base package refused: missing --source or --output");
        return ExitCode::FAILURE;
    };

    match build_base_candidate(&source, &selection, &output) {
        Ok(provenance) => {
            println!(
                "{}",
                json!({
                    "status": "CANDIDATE_BUILT_NOT_TRUSTED",
                    "size": provenance.artifact.size,
                    "sha256": provenance.artifact.sha256,
                    "files": provenance.files.len(),
                })
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Base package refused: {err}");
            ExitCode::FAILURE
        }
    }
}

#![forbid(unsafe_code)]

//! Host-only CLI (`simoder-workspace`) for verifying and assembling a pinned offline Simoder developer workspace.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;
use simoder_workspace::{assemble_offline_workspace, load_workspace_lock, verify_pinned_sources};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return ExitCode::FAILURE;
    }

    let subcommand = args[1].as_str();
    let mut lock_path: Option<PathBuf> = None;
    let mut sources_root: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--lock" if i + 1 < args.len() => {
                lock_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--sources" if i + 1 < args.len() => {
                sources_root = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--output" if i + 1 < args.len() => {
                output_dir = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            _ => {
                print_usage();
                return ExitCode::FAILURE;
            }
        }
    }

    let (Some(lock_path), Some(sources_root)) = (lock_path, sources_root) else {
        eprintln!("Missing required --lock or --sources argument");
        return ExitCode::FAILURE;
    };

    match subcommand {
        "verify" => match load_workspace_lock(&lock_path) {
            Ok((lock, lock_sha)) => match verify_pinned_sources(&lock, &lock_sha, &sources_root) {
                Ok(manifest) => {
                    println!(
                        "{}",
                        json!({
                            "status": "VERIFIED_OFFLINE",
                            "workspaceVersion": manifest.workspace_version,
                            "lockSha256": manifest.lock_sha256,
                            "components": manifest.components.len(),
                        })
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("Workspace verification refused: {err}");
                    ExitCode::FAILURE
                }
            },
            Err(err) => {
                eprintln!("Workspace lock refused: {err}");
                ExitCode::FAILURE
            }
        },
        "assemble" => {
            let Some(output_dir) = output_dir else {
                eprintln!("Missing required --output argument for assemble");
                return ExitCode::FAILURE;
            };
            match assemble_offline_workspace(&lock_path, &sources_root, &output_dir) {
                Ok(manifest) => {
                    println!(
                        "{}",
                        json!({
                            "status": "ASSEMBLED_OFFLINE",
                            "workspaceVersion": manifest.workspace_version,
                            "lockSha256": manifest.lock_sha256,
                            "components": manifest.components.len(),
                            "output": output_dir,
                        })
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("Workspace assembly refused: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage:\n  simoder-workspace verify --lock <workspace-lock.toml> --sources <parent-dir>\n  simoder-workspace assemble --lock <workspace-lock.toml> --sources <parent-dir> --output <fresh-dir>"
    );
}

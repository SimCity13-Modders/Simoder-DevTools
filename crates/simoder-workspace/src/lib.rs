#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Host-only CLI and library for assembling a pinned offline Simoder developer workspace (`Simoder-DevTools#3`).
//!
//! Enforces strict offline pinning (`workspace-lock.toml`), records exact revisions and SHA-256 digests
//! in `workspace-manifest.json`, reproduces byte-identical layouts across clean clones, and refuses
//! to stage or install the host workspace tool inside a `SimCity.exe` game installation.

pub mod assembler;
pub mod lock;

pub use {assembler::*, lock::*};

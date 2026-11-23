#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Deterministic offline USTAR packager, pinned Core `Snapshot::Capture` verifier,
//! and Luau semantic service harness for `Simoder-Base`.
//!
//! Enforces explicit source membership (`package-files.json`), fresh deterministic USTAR
//! output (`base.tar` + `base.provenance.json`), and independent native pin verification
//! without inferring runtime trust or installation authority from provenance.

pub mod config;
pub mod luau_harness;
pub mod packager;
pub mod reader;

pub use {config::*, luau_harness::*, packager::*, reader::*};

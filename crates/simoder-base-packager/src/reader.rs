#![forbid(unsafe_code)]

//! Independent Core `Snapshot::Capture` USTAR reader and native pin verifier for `Simoder-Base`.
//!
//! Validates a candidate `base.tar` archive against strict USTAR invariants and an
//! independently selected content map (`SelectedBasePins`), refusing altered bytes,
//! foreign file maps, unzeroed USTAR fields, or duplicate members.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{
    MAX_ARTIFACT_BYTES, MAX_ENTRIES, MAX_EXECUTABLE_BYTES, MAX_NOTICE_BYTES, MIN_ARTIFACT_BYTES,
    MIN_ENTRIES, USTAR_BLOCK_SIZE,
};
use crate::packager::{BasePackageError, is_valid_relative_name, parse_and_validate_selector};

/// Independently selected native pin set used by Core `Snapshot::Capture` verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedBasePins {
    /// Expected SHA-256 hex digest of the complete `base.tar` archive.
    pub archive_sha256: String,
    /// Expected exact size in bytes of `base.tar`.
    pub archive_size: usize,
    /// Map of canonical relative path -> expected SHA-256 hex digest.
    pub members: BTreeMap<String, String>,
}

impl SelectedBasePins {
    /// Compute independent native pins directly from an anchored `Simoder-Base` source checkout.
    ///
    /// Does not read or trust `base.provenance.json`.
    pub fn from_anchored_source(
        source_root: &Path,
        selector_name: &str,
        expected_archive_bytes: &[u8],
    ) -> Result<Self, BasePackageError> {
        let selector_bytes = fs::read(source_root.join(selector_name))
            .map_err(|e| BasePackageError::Io(e.to_string()))?;
        let selected = parse_and_validate_selector(&selector_bytes)?;
        let mut members = BTreeMap::new();
        for (path, _) in selected {
            let bytes =
                fs::read(source_root.join(&path)).map_err(|e| BasePackageError::Io(e.to_string()))?;
            members.insert(path, hex::encode(Sha256::digest(&bytes)));
        }
        Ok(Self {
            archive_sha256: hex::encode(Sha256::digest(expected_archive_bytes)),
            archive_size: expected_archive_bytes.len(),
            members,
        })
    }
}

/// Verify a `base.tar` USTAR payload against strict Core `Snapshot::Capture` rules and [`SelectedBasePins`].
pub fn verify_ustar_with_selected_pins(
    archive_bytes: &[u8],
    pins: &SelectedBasePins,
) -> Result<BTreeMap<String, Vec<u8>>, BasePackageError> {
    if !(MIN_ARTIFACT_BYTES..=MAX_ARTIFACT_BYTES).contains(&archive_bytes.len())
        || !archive_bytes.len().is_multiple_of(USTAR_BLOCK_SIZE)
    {
        return Err(BasePackageError::ArchiveVerificationFailed(
            "archive size out of bounds or not block-aligned".to_string(),
        ));
    }
    if archive_bytes.len() != pins.archive_size {
        return Err(BasePackageError::ArchiveVerificationFailed(
            "archive byte length differs from selected pin".to_string(),
        ));
    }
    let actual_archive_sha = hex::encode(Sha256::digest(archive_bytes));
    if !actual_archive_sha.eq_ignore_ascii_case(&pins.archive_sha256) {
        return Err(BasePackageError::ArchiveVerificationFailed(
            "archive SHA-256 differs from selected pin".to_string(),
        ));
    }

    let mut offset = 0usize;
    let mut extracted = BTreeMap::new();
    let mut prev_name: Option<String> = None;
    let mut exec_total = 0usize;

    while offset + USTAR_BLOCK_SIZE <= archive_bytes.len() {
        let block = &archive_bytes[offset..offset + USTAR_BLOCK_SIZE];
        if block.iter().all(|&b| b == 0) {
            if offset + USTAR_BLOCK_SIZE * 2 > archive_bytes.len()
                || !archive_bytes[offset..].iter().all(|&b| b == 0)
            {
                return Err(BasePackageError::ArchiveVerificationFailed(
                    "invalid or non-zero trailing blocks after EOF".to_string(),
                ));
            }
            break;
        }
        verify_ustar_header_checksum(block)?;
        if &block[257..263] != b"ustar\0" || &block[263..265] != b"00" {
            return Err(BasePackageError::ArchiveVerificationFailed(
                "invalid USTAR magic or version".to_string(),
            ));
        }
        if block[156] != b'0' && block[156] != 0 {
            return Err(BasePackageError::ArchiveVerificationFailed(
                "non-regular USTAR entry type refused".to_string(),
            ));
        }
        if &block[100..107] != b"0000644"
            || &block[108..115] != b"0000000"
            || &block[116..123] != b"0000000"
            || &block[136..147] != b"00000000000"
            || block[265] != 0
            || block[297] != 0
            || block[345] != 0
        {
            return Err(BasePackageError::ArchiveVerificationFailed(
                "non-deterministic USTAR mode/uid/gid/mtime/uname/gname/prefix refused".to_string(),
            ));
        }
        let name_end = block[..100]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(100);
        let name = std::str::from_utf8(&block[..name_end])
            .map_err(|_| {
                BasePackageError::ArchiveVerificationFailed("non-UTF8 USTAR member name".to_string())
            })?
            .to_string();
        if !is_valid_relative_name(&name) {
            return Err(BasePackageError::ArchiveVerificationFailed(format!(
                "invalid USTAR member name: {name}"
            )));
        }
        if let Some(prev) = &prev_name
            && name <= *prev
        {
            return Err(BasePackageError::ArchiveVerificationFailed(
                "USTAR members are not strictly ascending".to_string(),
            ));
        }
        prev_name = Some(name.clone());

        let size_str = std::str::from_utf8(&block[124..135]).map_err(|_| {
            BasePackageError::ArchiveVerificationFailed("invalid octal size".to_string())
        })?;
        let size = usize::from_str_radix(size_str.trim(), 8).map_err(|_| {
            BasePackageError::ArchiveVerificationFailed("malformed octal size".to_string())
        })?;

        offset += USTAR_BLOCK_SIZE;
        let padded = size.div_ceil(USTAR_BLOCK_SIZE) * USTAR_BLOCK_SIZE;
        if offset + padded > archive_bytes.len() {
            return Err(BasePackageError::ArchiveVerificationFailed(
                "truncated USTAR member payload".to_string(),
            ));
        }
        let payload = &archive_bytes[offset..offset + size];
        if !archive_bytes[offset + size..offset + padded]
            .iter()
            .all(|&b| b == 0)
        {
            return Err(BasePackageError::ArchiveVerificationFailed(
                "non-zero USTAR padding bytes".to_string(),
            ));
        }
        offset += padded;

        if name == "LICENSE" {
            if payload.is_empty() || payload.len() > MAX_NOTICE_BYTES {
                return Err(BasePackageError::ArchiveVerificationFailed(
                    "LICENSE size out of bounds".to_string(),
                ));
            }
        } else {
            exec_total += payload.len();
            if exec_total > MAX_EXECUTABLE_BYTES {
                return Err(BasePackageError::ArchiveVerificationFailed(
                    "executable payload exceeds 4 MiB".to_string(),
                ));
            }
        }

        let expected_member_sha = pins.members.get(&name).ok_or_else(|| {
            BasePackageError::ArchiveVerificationFailed(format!(
                "unselected/foreign member in archive: {name}"
            ))
        })?;
        let actual_member_sha = hex::encode(Sha256::digest(payload));
        if !actual_member_sha.eq_ignore_ascii_case(expected_member_sha) {
            return Err(BasePackageError::ArchiveVerificationFailed(format!(
                "member digest mismatch for {name}"
            )));
        }
        extracted.insert(name, payload.to_vec());
    }

    if !(MIN_ENTRIES..=MAX_ENTRIES).contains(&extracted.len())
        || extracted.len() != pins.members.len()
    {
        return Err(BasePackageError::ArchiveVerificationFailed(
            "extracted member set differs from selected native pins".to_string(),
        ));
    }

    Ok(extracted)
}

fn verify_ustar_header_checksum(block: &[u8]) -> Result<(), BasePackageError> {
    let stored_str = std::str::from_utf8(&block[148..154]).map_err(|_| {
        BasePackageError::ArchiveVerificationFailed("invalid header checksum encoding".to_string())
    })?;
    let stored = u32::from_str_radix(stored_str.trim(), 8).map_err(|_| {
        BasePackageError::ArchiveVerificationFailed("malformed header checksum".to_string())
    })?;
    let mut sum = 0u32;
    for (idx, &b) in block.iter().enumerate() {
        if (148..156).contains(&idx) {
            sum += u32::from(b' ');
        } else {
            sum += u32::from(b);
        }
    }
    if sum != stored {
        return Err(BasePackageError::ArchiveVerificationFailed(
            "USTAR header checksum mismatch".to_string(),
        ));
    }
    Ok(())
}

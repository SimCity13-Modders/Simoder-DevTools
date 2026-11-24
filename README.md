# Simoder-DevTools

[![Download Latest (Nightly)](https://img.shields.io/github/actions/workflow/status/SimCity13-Modders/Simoder-DevTools/ci.yml?branch=main&label=Download%20latest%20(nightly)&logo=githubactions&logoColor=white)](https://nightly.link/SimCity13-Modders/Simoder-DevTools/workflows/ci/main)
[![Download Latest Release](https://img.shields.io/github/v/release/SimCity13-Modders/Simoder-DevTools?label=Download%20latest%20release&logo=github)](https://github.com/SimCity13-Modders/Simoder-DevTools/releases/latest)
[![Star](https://img.shields.io/github/stars/SimCity13-Modders/Simoder-DevTools?style=social)](https://github.com/SimCity13-Modders/Simoder-DevTools/stargazers)

**Simoder-DevTools** provides the host-only offline developer CLI tools for the Simoder ecosystem — specifically deterministic workspace assembly (`simoder-workspace`) and offline `Simoder-Base` USTAR packaging and round-trip verification (`simoder-base-packager` / `simoder-base-reader`). It never modifies or writes directly into a live `SimCity.exe` game installation.

## Dependencies & Requirements

- **Rust toolchain**: Rust `edition = "2024"` (`x86_64-pc-windows-msvc` or host platform)
- **Pinned ecosystem checkouts**: Local sibling checkouts matching [`workspace-lock.toml`](workspace-lock.toml) when running offline workspace verification or assembly

## Installation

1. Clone this repository alongside your Simoder ecosystem checkouts (or download a prebuilt CLI from the **Release** or **Nightly** badge above).
2. Build the workspace CLI tools locally:

```powershell
cargo build --release --workspace
```

---

## Workspace Crates & Tools

| Crate / Binary | Responsibility |
| --- | --- |
| `simoder-workspace` | Verifies pinned local checkouts against [`workspace-lock.toml`](workspace-lock.toml), records exact revisions and SHA-256 digests into `workspace-manifest.json`, and assembles reproducible staging layouts. |
| `simoder-base-packager` (`simoder-base-reader`) | Builds and verifies deterministic offline USTAR candidate archives from `Simoder-Base`'s `package-files.json` without implicit network fetches. |

## Verification & Usage

```powershell
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
cargo run --release --bin simoder-workspace -- verify --lock workspace-lock.toml --sources ..
cargo run --release --bin simoder-workspace -- assemble --lock workspace-lock.toml --sources .. --output C:\stage\simoder-workspace
```

## Related Repositories

- [Simoder](https://github.com/SimCity13-Modders/Simoder) — Integrated 32-bit `d3d9.dll` runtime proxy
- [Simoder-Base](https://github.com/SimCity13-Modders/Simoder-Base) — Pure-Luau base semantic layer & UI
- [Simoder-SDK](https://github.com/SimCity13-Modders/Simoder-SDK) — Canonical Luau, TypeScript/JS, and Native `ABI v2` declarations
- [Simoder-MCP](https://github.com/SimCity13-Modders/Simoder-MCP) — Optional native `ABI v2` MCP DevTool mod (`simoder_mcp.dll`)

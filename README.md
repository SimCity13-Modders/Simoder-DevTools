# Simoder-DevTools

Host-only offline workspace composition CLI (`simoder-workspace`, `crates/simoder-workspace`) and developer documentation for the Simoder ecosystem.

- Runtime proxy DLL (`d3d9.dll`) belongs to `Simoder`.
- Pure-Luau base semantic abstraction mod (`base`) belongs to `Simoder-Base` (usable across Luau, TypeScript, and JavaScript mods).
- Canonical API/ABI declarations and contract matrix belong to `Simoder-SDK`.
- Source-mod compiler (`simoder-build`) and `TS -> JS` toolchain belong to `Simoder-DevTools-Authoring`.
- Native `ABI v2` MCP DevTool mod (`simoder_mcp.dll`) belongs to `Simoder-DevTools-MCP`.
- Developer workspace CLI (`simoder-workspace`) and deterministic `Simoder-Base` offline packager/verifier (`simoder-base-packager` / `simoder-base-reader`) belong to `Simoder-DevTools`.

## Offline Workspace CLI (`simoder-workspace`)

`simoder-workspace` verifies and assembles a developer workspace from explicitly pinned local checkouts described in [`workspace-lock.toml`](workspace-lock.toml). It never performs implicit network fetches or upgrades, records exact component revisions and file SHA-256 digests in `workspace-manifest.json`, reproduces byte-identical layouts from clean clones, and refuses to write into any `SimCity.exe` game installation.

```powershell
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
cargo run --release --bin simoder-workspace -- verify --lock workspace-lock.toml --sources ..
cargo run --release --bin simoder-workspace -- assemble --lock workspace-lock.toml --sources .. --output C:\stage\simoder-workspace
```

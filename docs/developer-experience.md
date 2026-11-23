# Developer workflow and integrations

## Ownership

Simoder owns the integrated game runtime. OpenSCP owns mod editing, package
conversion and package inspection. This repository retains the existing
composition of the separately pinned Simoder-DevTools-Authoring and
Simoder-DevTools-MCP packages.
SDK declarations and example mods are separate pinned repositories.

The obsolete developer CLI, standalone inspectors/probes and migration/import
adapters have been removed. No alpha/prototype backwards-compatibility promise
is made; the commitment begins at the real Beta release. Runtime parser behavior
is unchanged by this cleanup. Removed probe capabilities are not assumed to have
complete equivalents in the current MCP implementation.

## Build a source mod

Build `simoder-build.exe` in the Authoring repository or through its explicit
pinned dependency in this root CMake project, then use a source project
whose parent directory also contains its `cache` directory:

```powershell
C:/Tools/Simoder-DevTools/simoder-build.exe C:/work/mods/MyMod C:/work/mods/cache
C:/Tools/Simoder-DevTools/simoder-build.exe --check C:/work/mods/MyScriptMod
```

The compiler publishes immutable content-addressed generations and cancels
publication when source changes during compilation. `--check` supports only
script-only mods, executes lifecycle callbacks outside the game and does not
activate native game resources. Success does not establish gameplay correctness.

The existing TypeScript authoring path uses TypeScriptToLua and externally
available Node.js. It is the current implementation, not the planned Beta JS
runtime. Resource builds require the existing external converter executable;
removal of the obsolete package CLI does not provide or replace that dependency.
No dependency is installed by the commands above.

## Editor support

The Authoring repository owns `devtools/vscode/README.md` and the declarative
TOML snippets. Configure Luau LSP using its `devtools/luau/README.md` and pinned
SDK definitions. Installed paths remain `VSCode/` and `Luau/` in this assembly.
Executable validation, on-save diagnostics and alias-completion integration
were retired with their CLI. No IDE build/debugger feature is introduced here.

## MCP and diagnostics

See [current MCP tools and controls](https://github.com/SimCity13-Modders/Simoder-MCP). Runtime evidence
retains freshness, process identity and bounded-read checks. Current standalone
MCP/DevBridge applications remain pending migration to a native DevTool mod.
Do not infer a replacement diagnostic capability from removal of an old probe.

REA remains an optional separately prepared reverse-engineering integration:
see [its separate source package](https://github.com/SimCity13-Modders).
The explicit clean pin is optional through `SIMODER_BUILD_REA`; installed
`Reverse/` files are host helpers, not a native Diagnostics mod. No REA process,
dependency fetch or client registration is started by composition. Raw experiments and audits
belong in ignored `.analysis/`.

## Verification boundary

Use fresh CMake build and install directories after retirement; old installed
files are not uninstalled by CMake. Remaining authoring/DLL-host tests exercise
existing generation behavior. Current MCP and DevBridge tests validate their
own existing contracts. Compiler success, SDK/editor declarations and synthetic
protocol tests do not prove game-resource selection or visible simulation effects.
The runtime and its deployment procedures are maintained in the Simoder repository.

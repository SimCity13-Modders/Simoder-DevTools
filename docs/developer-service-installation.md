# Game-owned developer tools

Build the DevTools composition targets: `simoder-build` comes from the explicit
clean pinned `SIMODER_AUTHORING_SOURCE_DIR` checkout, while `sc13-ai-devtools`
comes from the explicit clean pinned `SIMODER_MCP_SOURCE_DIR` checkout. This
repository assembles their outputs; each component owns its own sources.
Authoring regression tests require separately prepared pinned npm
dependencies in that checkout, as documented by Authoring. Staging does not
require or copy those dependencies. Install DevTools into a fresh staging
directory.
Also supply the exact clean `SIMODER_MCP_SOURCE_DIR` pinned by
`cmake/simoder-mcp.lock.cmake`, with its existing .NET dependencies explicitly
restored as described in that repository. The build never restores implicitly.
MCP owns only its service components; this repository retains the mixed manifest,
Authoring capabilities and complete package assembly.
Use `cmake --install <build-directory> --config Release --component developer-services
--prefix <staging-directory>` to stage only the optional service mod.
The developer-service package is `mods/simoder.devtools/`. Copy that complete
directory into the game's `mods/` directory, then enable **Simoder Developer Tools**
in ModLoader. No broker or watcher needs to be started separately.

The runtime owns DevBridge and MCP through the enabled mod's Windows Job Object.
Disabling the mod or ending the game terminates their process tree. The manifest
starts the broker before the MCP game host; the client handles delayed pipe
availability. Services are not automatically restarted after failure.

Run `Register-Simoder-MCP.ps1` from the installed developer mod to configure the
external stdio client. The registered executable and the game-owned MCP host share
the same installed component; `--game-host` selects the owned server mode.
Registration changes client configuration only. It does not enable the mod or
launch an injector.

Installation ownership:

- Simoder: one `d3d9.dll` beside `SimCity.exe`.
- Developer services: optional `mods/simoder.devtools/` directory.
- Authoring compiler/toolchain capabilities: `Tools/` inside the enabled service mod.
- Compiler, editor definitions and optional examples: the separate DevTools installation.

The service mod registers its authoring worker and TypeScript adapter only while
enabled. The runtime DLL does not accept a sibling compiler fallback. Install
Node.js separately and make it available on PATH. TypeScript and TypeScriptToLua
are separate pinned npm prerequisites for the current TS-to-Luau adapter;
neither this package nor the runtime DLL embeds Node.js or those compiler packages.
The packaged adapter, declarations and dependency lock describe how to use them,
without installing or downloading them automatically.

Before using TypeScript, separately prepare the pinned npm dependencies beside
the installed adapter in `Tools/toolchains/typescript`, using the approved
external preparation procedure. Run `npm ci --ignore-scripts` from that directory
when network access is approved. Do this after package installation; the
distributable stage remains free of `node_modules`. A globally installed npm
compiler is not sufficient for the adapter's local imports. The enabled mod's
registered toolchain path takes precedence over `SIMODER_TYPESCRIPT_TOOLCHAIN`;
that variable is used only when there is no registered or adjacent adapter.
No toolchain-discovery behavior is changed by this packaging cleanup.

Native-resource conversion requires a separately declared and
installed resource-converter capability.

After changing a service manifest or executable, disable and re-enable the mod.
If the game is not running, its game-owned services are unavailable.

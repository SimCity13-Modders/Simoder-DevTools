# Repository extraction verification Ă˘â‚¬â€ť 2026-10-11

- Simoder Release build and C++ CTest passed using the standard vs2026 presets.
- A clean runtime staging install contained loader/launcher/config/mod/license files
  and no developer executables, MCP, editor or importer payload.
- DevTools Release build, self-contained win-x64 publication and all four CTest
  groups passed from C:/Projects/Simoder-DevTools: importers, editor helpers and
  DevBridge/MCP (including Windows Graphics Capture and configured game-root test).
- Native-only DevTools built with no local source override, using a local Git mirror
  and locked core revision 0391f81e8317b82469b99e303748573f25099874. No ImGui/MinHook
  dependencies were fetched by either DevTools build.
- Final standalone installation is build/install-isolated. Its CLI validated the packaged
  BetterSanitizer against installed game packages with checkedGame=true, no errors
  or conflicts and sanitizerOperatingCost 300 -> set 345 -> 345.
- Project MCP registration now points to build/install-isolated/AI/MCP with SIMODER_GAME_ROOT.
  The client must restart before using that registration.

This validates builds, tools and package evaluation. A fresh in-game session or
visual VS Code Extension Host acceptance was not run during extraction. REA
provider setup and its Node prerequisite remain separate from these tests.

Repositories and their original/extracted commit map are local; no remote push
or GitHub repository creation was performed. Use SIMODER_SOURCE_DIR or a local
SIMODER_CORE_REPOSITORY mirror until the locked Simoder commit is published.

A published-executable handshake additionally exposed a dependency collision in the former shared AI output. Publication now uses isolated MCP and Broker subdirectories, with companion path tests and a separate installed stdio handshake check.

The final four DevTools CTest groups and loader CTest passed. The published and installed isolated MCP both completed stdio initialize/tools/list and exposed the exact ten tools. The new published-executable regression test is tracked under tests/test_published_mcp.py.

Current ownership note (2026-10-15): the preserved published-executable test
now belongs to tests/test_published_mcp.py in Simoder-DevTools-MCP. Tool counts
and paths above describe historical extraction; use the current deployment guide.

# Change: Remove npx distribution chain

## Why
Maintaining the npm/npx distribution path adds ongoing release and infrastructure overhead for this fork. The project will focus on local builds or direct binary distribution instead.

## What Changes
- Remove the npm/npx wrapper, build scripts, and related artifacts
- Remove npm publish and npx packaging steps from CI workflows
- Limit release artifacts to Linux x86_64 builds only
- Remove in-repo documentation to reduce maintenance burden
- Update docs and defaults to reflect source build + optional standalone MCP binary
- **BREAKING**: Installation and MCP server instructions will no longer use `npx vibe-kanban`

## Impact
- Affected specs: install-app (new)
- Affected code: `npx-cli/`, `local-build.sh`, `package.json`, `Dockerfile`, `.github/workflows/pre-release.yml`, `.github/workflows/publish.yml`, `docs/` (removed), `ARCH.md`, `AGENTS.md`, `crates/executors/default_mcp.json`

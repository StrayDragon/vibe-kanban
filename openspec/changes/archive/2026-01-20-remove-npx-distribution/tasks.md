## 1. Implementation
- [x] 1.1 Remove npm/npx distribution assets (`npx-cli/`, `local-build.sh`) and npm bin scripts
- [x] 1.2 Remove npm publish and npx packaging steps from CI workflows (`.github/workflows/pre-release.yml`, `.github/workflows/publish.yml`)
- [x] 1.3 Limit release workflow builds to Linux x86_64 only
- [x] 1.4 Update build and container configuration (root `package.json`, `Dockerfile`)
- [x] 1.5 Remove in-repo documentation and related references (`ARCH.md`, `AGENTS.md`)
- [x] 1.6 Update default MCP config to use `mcp_task_server` instead of `npx vibe-kanban`
- [x] 1.7 Audit for leftover `npx vibe-kanban` references and clean up

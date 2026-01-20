## Context
Executors currently embed fixed `npx` command strings for each agent. The requested behavior is to prefer user-installed global versions (pnpm/npm) and only fall back to `npx @latest` when nothing is installed, without blocking startup.

## Goals / Non-Goals
- Goals:
  - Resolve a command source and version for every `BaseCodingAgent`.
  - Prefer pnpm global installs over npm global installs, with `npx @latest` fallback.
  - Initialize resolution asynchronously and reuse cached results.
  - Surface source/version in Agent Settings, including a fallback warning.
- Non-Goals:
  - Changing executor configuration schema semantics beyond adding read-only metadata.
  - Auto-installing or upgrading agent CLIs.

## Decisions
- Decision: Introduce an async `AgentCommandResolver` in `executors` with a cached map keyed by `BaseCodingAgent`.
  - Each entry contains: source (`pnpm_global`, `npm_global`, `npx_latest`, `system_binary`, `override`, `unknown`), version (optional), resolved base command string, and a `status` (`checking`/`ready`).
- Decision: Use a static mapping from `BaseCodingAgent` to package + binary metadata.
  - Node-distributed executors map to `{package_name, binary_name, npx_base}`.
  - Non-node executors (e.g., `CURSOR_AGENT`, `DROID`, `FAKE_AGENT`) map to `{binary_name}` and use system binary resolution.
  - Claude Code router uses a distinct package/binary mapping and includes `code` as a base argument.
- Decision: Resolve pnpm/npm global package versions via `pnpm list -g --json` and `npm list -g --json`.
  - Locate global bin dirs via `pnpm bin -g` / `npm bin -g` and build absolute binary paths when possible.
  - If a global package is detected but the binary cannot be resolved, treat it as not installed and continue to the next tier.
- Decision: If no global install is found, build the base command as `npx -y <pkg>@latest`.
  - Do not use `pnpm dlx`.
- Decision: Respect `CmdOverrides.base_command_override` by short-circuiting resolution and marking the source as `override`.

## Risks / Trade-offs
- Running package manager commands at startup may be slow; async initialization avoids blocking.
- Some CLIs may not expose a reliable `--version`; for system binaries, version will be reported as unknown.

## Migration Plan
- Add resolver and API metadata first.
- Update executor command builders to use resolver output.
- Update frontend to display resolved source/version and fallback notice.

## Open Questions
- None.

# Change: Smart agent version selection

## Why
Executors currently launch with pinned `npx` versions. Users who already installed agent CLIs globally want the system to prefer their installed version (pnpm/npm) and see what version is in use.

## What Changes
- Resolve the command/version for each `BaseCodingAgent` using priority: pnpm global install, npm global install, then `npx` with `@latest`.
- Run resolution asynchronously during startup and cache results for reuse.
- Use the resolved command when spawning executors unless a `base_command_override` is set.
- Expose resolved source/version to the frontend and show it in Agent Settings (including a notice when falling back to `latest`).

## Impact
- Affected specs: `agent-version-selection` (new)
- Affected code: `crates/executors`, `crates/local-deployment`, `crates/server`, `frontend/src/pages/settings/AgentSettings.tsx`, `shared/types.ts` (generated)

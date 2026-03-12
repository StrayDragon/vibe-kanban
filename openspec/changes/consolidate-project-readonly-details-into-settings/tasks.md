## 1. Settings-surface parity

- [x] 1.1 Add a compact readonly project metadata section to `Settings > Projects` for the selected project. Verification: `pnpm run frontend:check && pnpm run frontend:lint`
- [x] 1.2 Move the latest lifecycle-hook outcome summary into the existing lifecycle-hooks settings section with compact loading and empty states. The expensive "scan tasks + fetch attempts" query SHOULD be on-demand (expander/button) rather than running on initial settings render. Verification: `pnpm run frontend:check && pnpm run frontend:lint`

## 2. Deprecated surface cleanup

- [x] 2.1 Remove the obsolete standalone project-detail page wiring once settings parity is complete. Verification: `pnpm run frontend:check`
- [x] 2.2 Delete or consolidate frontend components/imports that only served the deprecated project-detail workflow. Verification: `rg -n "ProjectDetail|Projects" frontend/src && pnpm run frontend:check`

## 3. Browser validation

- [x] 3.1 Smoke-test the selected-project settings page for a project with hook activity and confirm the readonly metadata + hook summary are visible without leaving settings. Confirm the hook outcome query is only triggered after expanding/requesting the summary. Verification: manual browser smoke check on `/settings/projects?projectId=<id>`

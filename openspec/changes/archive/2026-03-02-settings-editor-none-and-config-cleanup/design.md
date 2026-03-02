## Context

Vibe Kanban currently treats “Editor” as a required preference and surfaces multiple “Open in …” affordances across the UI (e.g., the navbar IDE icon, task attempt actions, diff cards). This is useful when the user has a local editor CLI available, but it becomes distracting when:

- The user does not want the app to open anything in an external editor.
- The editor executable is not available on `PATH`, producing repeated warnings/indicators.
- The app is used in contexts where local process spawning is undesirable.

Additionally, git `--no-verify` is configured globally via `Config.git_no_verify`, but this is often project-dependent (some repos rely heavily on hooks, others don’t). A single global value forces an all-or-nothing trade-off.

Two existing Settings areas may be niche and worth simplifying:

- “Pull Requests” settings (auto-generate PR description + prompt customization)
- “Remote SSH Host” in Editor settings (remote URL generation for VS Code/Cursor/Windsurf)

## Goals / Non-Goals

**Goals:**
- Add an explicit “None / disabled” editor type.
- When disabled, hide all “Open in …” UI affordances and do not show availability warnings.
- Add a project-scoped override for git hook skipping (`--no-verify`) with precedence: project overrides global.
- Update UI helper text to clearly explain precedence.
- Produce a recommendation on whether PR settings and remote SSH host should be kept/moved/removed.

**Non-Goals:**
- Broad Settings UI reorganization.
- Changing default behaviors for existing users (defaults remain unchanged unless the user opts in).
- Expanding git settings beyond `--no-verify` in this change.

## Decisions

### 1) Model “disabled editor” as a new `EditorType` variant

**Decision:** Add `EditorType::None` (serialized as `NONE`) to the existing `EditorType` enum.

**Why:**
- Matches the user’s mental model (“I am not using an editor integration”).
- Avoids introducing a separate boolean flag and split-brain configuration.
- Minimal changes to config shape; only expands allowed values.

**Alternatives considered:**
- Add `editor_enabled: bool` to config: adds another knob and requires more UI/logic branching.
- Hide editor integration when the executable is missing: conflates “unavailable” with “disabled”.

### 2) Centralize editor affordance gating in the frontend

**Decision:** Introduce a single “is editor integration enabled?” helper (e.g., `isEditorIntegrationEnabled(config)` or a hook) and use it everywhere that renders or triggers “Open in …”.

**Why:**
- “Open in …” appears in multiple components; ad-hoc checks are easy to miss.
- Central logic makes it easier to audit and prevents UI drift.

**Expected touch points (non-exhaustive):**
- Navbar: `frontend/src/components/layout/Navbar.tsx`
- IDE icon/button: `frontend/src/components/ide/OpenInIdeButton.tsx` and `IdeIcon.tsx`
- Task attempt actions: `frontend/src/components/ui/actions-dropdown.tsx`
- Diff UI: `frontend/src/components/DiffCard.tsx`
- Conversation/next actions: `frontend/src/components/NormalizedConversation/NextActionCard.tsx`
- Any dialog that offers editor fallback selection

### 3) Backend behavior when editor integration is disabled

**Decision:** If an “open editor” endpoint is called while the effective editor type is `NONE`, return a clear validation-style error (HTTP 400) rather than attempting to spawn a process or constructing a URL.

**Why:**
- Avoids silent no-ops that confuse users and logs.
- Provides a deterministic response even if a UI affordance is missed.

**Implementation approach:**
- Add a new `EditorOpenError` variant (e.g., `EditorDisabled`) or validate in the route handler before calling `open_file`.

### 4) Project-scoped `--no-verify` override as a nullable boolean

**Decision:** Add `projects.git_no_verify_override: Option<bool>` (nullable) with semantics:

- `null`: inherit from global `Config.git_no_verify`
- `true`: always pass `--no-verify` for this project
- `false`: never pass `--no-verify` for this project

**Why:**
- Captures all required states with a single column and clear precedence.
- Avoids storing “effective” values redundantly.

**Alternatives considered:**
- A separate “inherit” boolean + value boolean: more fields and more UI complexity.
- Per-repo override: more granular than needed for the described use case.

### 5) UI/UX for global vs project precedence

**Decision:**
- Keep the global checkbox in General Settings, but update helper text to clarify it is the default.
- Add a project-level control in Project Settings to override the global value.

**UI control suggestion:**
- A 3-state select: “Use global”, “Enabled”, “Disabled”.
  - This communicates precedence explicitly and supports the “global enabled + project disabled” example.

### 6) Research-first for PR settings and remote SSH host

**Decision:** Treat these as an audit with a recommendation output first; implement keep/move/remove only after confirming the desired direction.

**Research inputs:**
- Code references and coupling (frontend + backend)
- UX discoverability and user impact
- Maintenance cost (config schema, migrations, docs)
- Whether the feature has an obvious “advanced” home vs removal

## Risks / Trade-offs

- **Risk:** Missing one “Open in …” affordance → **Mitigation:** central helper + grep-based audit + UI smoke checklist.
- **Risk:** Adding a DB column requires migration correctness → **Mitigation:** add a forward-only migration with NULL default; include a quick migration verification step.
- **Risk:** Downgrading after selecting `EditorType.NONE` could break old binaries that can’t deserialize the new enum value → **Mitigation:** document rollback caveat; optionally consider tolerant enum parsing if rollbacks are common.

## Migration Plan

1. Add DB migration to introduce `projects.git_no_verify_override` (nullable, default NULL).
2. Update Rust models/entities and project API payloads to include the new field.
3. Update server-side git operations to compute `effective_no_verify` using project override precedence.
4. Add `EditorType::None` and update editor open behavior + error handling.
5. Regenerate shared TS types (`pnpm run generate-types`) and update frontend:
   - Add “None” to editor selection UI (localized label).
   - Gate all “Open in …” affordances on editor being enabled.
   - Add project override UI and update helper copy explaining precedence.
6. Ship with a short internal note/recommendation for PR settings and remote SSH host (keep/move/remove).

Rollback strategy:
- Safe DB rollback: the new nullable column can be ignored by older binaries.
- Editor rollback caveat: configs written with `editor_type: NONE` may fail to deserialize on older binaries (will fall back to defaults today).

## Open Questions

- Should the backend return `400 Bad Request` vs `204 No Content` when editor integration is disabled?
- Do we want editor type labels fully localized (recommended) vs `toPrettyCase`?
- Should the project override UI show the effective value (“Effective: Enabled/Disabled”) to reduce confusion?
- If PR settings and/or remote SSH host are retained, should they move under an “Advanced” section instead of General Settings?

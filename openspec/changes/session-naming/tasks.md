## 1. DB + Models

- [ ] 1.1 Add SeaORM migration to add nullable `sessions.name` column
- [ ] 1.2 Update `crates/db/src/entities/session.rs` and
      `crates/db/src/models/session.rs` to include `name`
- [ ] 1.3 Update key session creation call sites to pass an auto-generated name
      when context is available

## 2. API

- [ ] 2.1 Extend `/api/sessions` responses to include `name`
- [ ] 2.2 Add `PATCH /api/sessions/:session_id` rename endpoint with validation
      (trim, empty → null, max length)
- [ ] 2.3 Add `sessionsApi.rename(sessionId, { name })` to frontend API client

## 3. Frontend (Processes Dialog)

- [ ] 3.1 Fetch sessions for the current attempt/workspace and display a session
      selector showing `name` (or fallback label)
- [ ] 3.2 Add a rename UI for the selected session (dialog or inline edit)
- [ ] 3.3 (Optional) Filter the execution process list by selected session id

## 4. Types + Verification

- [ ] 4.1 Run `pnpm run generate-types` and ensure `shared/types.ts` updates are
      committed
- [ ] 4.2 Run `pnpm run check` and `pnpm run lint`
- [ ] 4.3 Run `cargo test --workspace`


## 1. Specification
- [ ] 1.1 Update translate-conversation spec delta for server-side OpenAI-compatible proxy.
- [ ] 1.2 Validate with `openspec validate update-chat-translation-llm --strict`.

## 2. Backend
- [ ] 2.1 Add `POST /api/translation` route and wire into router.
- [ ] 2.2 Implement OpenAI-compatible client using `reqwest` against `/v1/chat/completions`.
- [ ] 2.3 Read `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, `KANBAN_OPENAI_DEFAULT_MODEL` with fallbacks to `OPENAI_API_BASE`, `OPENAI_API_KEY`, `OPENAI_DEFAULT_MODEL`; missing config returns a non-200 error with message.
- [ ] 2.4 Add basic unit test for request/response mapping.

## 3. Frontend
- [ ] 3.1 Add API wrapper for translation and update translation callsites.
- [ ] 3.2 Keep existing translation UI states (loading/failed/stale/retry).

## 4. Docs / QA
- [ ] 4.1 Document `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, `KANBAN_OPENAI_DEFAULT_MODEL` (and fallback envs) in developer setup notes.
- [ ] 4.2 Manual test: translate a streaming assistant message and confirm no history mutation.

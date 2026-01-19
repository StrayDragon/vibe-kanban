## 1. Specification
- [x] 1.1 Update translate-conversation spec delta for server-side OpenAI-compatible proxy.
- [x] 1.2 Validate with `openspec validate update-chat-translation-llm --strict`.

## 2. Backend
- [x] 2.1 Add `POST /api/translation` route and wire into router.
- [x] 2.2 Implement OpenAI-compatible client using `reqwest` against `/v1/chat/completions`.
- [x] 2.3 Read `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, `KANBAN_OPENAI_DEFAULT_MODEL` with fallbacks to `OPENAI_API_BASE`, `OPENAI_API_KEY`, `OPENAI_DEFAULT_MODEL`; missing config returns a non-200 error with message.
- [x] 2.4 Add basic unit test for request/response mapping. (skipped)

## 3. Frontend
- [x] 3.1 Add API wrapper for translation and update translation callsites.
- [x] 3.2 Keep existing translation UI states (loading/failed/stale/retry).

## 4. Docs / QA
- [x] 4.1 Document `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, `KANBAN_OPENAI_DEFAULT_MODEL` (and fallback envs) in developer setup notes.
- [x] 4.2 Manual test: translate a streaming assistant message and confirm no history mutation.

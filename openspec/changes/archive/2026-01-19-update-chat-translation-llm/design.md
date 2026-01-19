## Context
Users want assistant replies in a target language, but streaming outputs can include English. We already have on-demand translation in the UI; we now need to switch to an OpenAI-compatible LLM endpoint and keep API keys off the client.

## Goals / Non-Goals
- Goals:
  - Route translation through a backend endpoint that calls an OpenAI-compatible LLM.
  - Preserve the existing translate button and ephemeral client-side display.
  - Provide clear loading/error/retry states, including missing config errors.
- Non-Goals:
  - Real-time streaming translation.
  - Automatic translation of all content.
  - Persisting translated text to the database or log stream.
  - Language selection UI (keep `en` → `zh-CN` default).

## Decisions
- **Endpoint**: Add `POST /api/translation` to proxy translation requests.
- **Provider**: Use an OpenAI-compatible LLM endpoint configured via `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, and `KANBAN_OPENAI_DEFAULT_MODEL` with fallback to `OPENAI_API_BASE`, `OPENAI_API_KEY`, and `OPENAI_DEFAULT_MODEL`.
- **Request shape**: Call `POST {base_url}/v1/chat/completions` with a system prompt that instructs pure translation and a user message containing the source text.
- **Response shape**: Return `{ translated_text }` wrapped in `ApiResponse` without storing anything server-side.
- **Error handling**: Missing LLM config (including fallback envs) returns a clear error so the UI can show a non-blocking failure and retry.

## Interaction Flow
1. User clicks Translate next to a conversation entry.
2. Frontend sends `{ text, source_lang, target_lang }` to `POST /api/translation`.
3. Backend calls the OpenAI-compatible endpoint and returns translated text.
4. UI renders translation below the original with the existing bilingual toggle.
5. If config is missing or provider fails, UI shows an error and allows retry.

## Risks / Trade-offs
- **Provider availability**: LLM outages will surface to users; keep retries and clear errors.
- **API key exposure**: Avoided by keeping LLM credentials server-side only.

## Migration Plan
No data migrations. Deploy with `KANBAN_` prefixed LLM credentials (or fallback envs) configured in the backend environment.

## Open Questions
- None.

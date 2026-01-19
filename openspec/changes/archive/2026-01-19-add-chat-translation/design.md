## Context
Users frequently want agent replies in a specific language (often Chinese), but streaming responses can include English. The existing conversation UI renders read-only Markdown with a copy action but has no translation affordance. We need a low-risk, on-demand translation flow that does not modify stored logs.

## Goals / Non-Goals
- Goals:
  - Provide an explicit translate action per conversation entry.
  - Translate on demand and show results inline without persisting them.
  - Keep the action fast and non-blocking with clear loading/error states.
- Non-Goals:
  - Real-time streaming translation.
  - Automatic translation of all content.
  - Persisting translated text to the database or log stream.
  - Translating tool output or code blocks in v1.
  - Adding new backend endpoints or services for translation.

## Decisions
- **UI entry point**: Add a translate icon button next to the existing copy action in read-only WYSIWYG content when a caller opts in. This keeps UX consistent and avoids a new toolbar.
- **Eligible entries**: Enable translate for text-based entries rendered by WYSIWYG (assistant/system/thinking/user). Tool cards and file diffs stay unchanged. For v1, hide/disable translate when code fences are detected to avoid translating code blocks.
- **Target language**: Use a fixed translation direction of English → Simplified Chinese (`en` → `zh-CN`) with no language detection or user selection; keep it ephemeral (no persistence).
- **Ephemeral storage**: Keep translation state in the frontend keyed by `patchKey` + `target_language` + `source_hash`. Do not mutate `NormalizedEntry` or server logs. Refreshing clears all translations.
- **Translation transport**: Call the translation provider directly from the frontend (no backend changes). Use MyMemory public API for v1.
- **Bilingual layout**: Render the translated content directly below the original entry with a compact label (e.g., "Translated to zh-Hans"). This is a UI-only change; virtualization can handle dynamic height updates.

## Provider Survey (Free Sources)
### MyMemory (hosted, free tier with limits)
- API: `GET https://api.mymemory.translated.net/get?q=...&langpair=en|zh`.
- CORS: Allows `Access-Control-Allow-Origin: *` (verified).
- Usage limits: free anonymous usage limited to 5000 chars/day; providing a valid email (`de` param) raises limit to 50000 chars/day.
- Usage limits page: `https://mymemory.translated.net/doc/usagelimits.php`.

### Recommendation
Use MyMemory as the default provider for a pure-frontend MVP. Make limits explicit in the UI, and keep translation ephemeral.

## Interaction Flow
1. User hovers a conversation entry and sees Copy + Translate actions.
2. User clicks Translate.
3. UI sends API request with current entry content using the fixed `en` → `zh-CN` direction.
4. UI shows a spinner on the translate button and/or a placeholder row.
5. On success, UI renders the translated text beneath the original with a small "Translated to <lang>" label.
6. If the entry content changes (streaming update), mark translation as stale and show a "Re-translate" affordance.
7. On error, show a non-blocking error state and allow retry.

## Risks / Trade-offs
- **Latency/cost**: Translation adds API latency and potential model cost. Mitigate by keeping it opt-in and client-side cached per entry.
- **Provider limits**: Free tiers can be rate-limited. Mitigate with clear error messaging and retry UI.
- **Data exposure**: Translations send text to an external service. Mitigate with explicit user action.
- **Staleness**: Streaming updates can invalidate translations. Mitigate with `source_hash` checks and re-translate UI.

## Migration Plan
No data migrations. No backend changes.

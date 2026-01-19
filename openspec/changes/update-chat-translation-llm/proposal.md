# Change: Update chat translation to OpenAI-compatible LLM

## Why
The current on-demand translation uses a public browser-side provider. We need a backend proxy so translations remain ephemeral while using an OpenAI-compatible LLM endpoint and protecting the API key.

## What Changes
- Replace frontend MyMemory calls with a backend translation endpoint.
- Backend calls an OpenAI-compatible LLM endpoint using `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, and `KANBAN_OPENAI_DEFAULT_MODEL` (fallback to `OPENAI_API_BASE`, `OPENAI_API_KEY`, `OPENAI_DEFAULT_MODEL`) and returns translated text.
- Keep translation UI/ephemeral behavior unchanged; missing config reports an error on click.

## Impact
- Affected specs: translate-conversation
- Affected code: `crates/server/src/routes`, `frontend/src/utils/translation.ts`, `frontend/src/components/NormalizedConversation/TranslatableContent.tsx`, `frontend/src/lib/api.ts`

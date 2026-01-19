# Change: Add on-demand chat translation

## Why
Users expect agent replies in a target language (e.g., Chinese), but streaming outputs can include other languages. Provide a fast, on-demand translation flow that does not mutate stored chat history.

## What Changes
- Add a translate action in the conversation UI (next to copy) for eligible entries.
- Translate directly from the frontend using a public translation API (MyMemory) with no backend changes.
- Keep translations ephemeral on the client; do not persist or rewrite server-side logs.

## Impact
- Affected specs: translate-conversation (new)
- Affected code: frontend conversation rendering + WYSIWYG actions

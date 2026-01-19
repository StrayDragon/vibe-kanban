## 1. Specification
- [x] 1.1 Add translate-conversation spec delta with UI and API requirements.
- [x] 1.2 Validate the change with `openspec validate add-chat-translation --strict`.

## 2. Frontend
- [x] 2.1 Add translation state store keyed by `patchKey` + target language.
- [x] 2.2 Add optional translate action to read-only WYSIWYG toolbar (skip tool output + code fence entries).
- [x] 2.3 Call MyMemory API directly from the browser with source/target language mapping.
- [x] 2.4 Render bilingual stacked content (original + translation) with loading/error/retry states.
- [x] 2.5 Wire i18n labels for translate UI.

## 3. QA
- [x] 3.1 Manual test: translate a streaming assistant message and confirm no history mutation.
- [x] 3.2 Manual test: reload attempt view and confirm translations are cleared.
- [x] 3.3 Manual test: MyMemory quota/limit failure shows non-blocking error.

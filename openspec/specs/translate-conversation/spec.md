# translate-conversation Specification

## Purpose
TBD - created by archiving change add-chat-translation. Update Purpose after archive.
## Requirements
### Requirement: On-demand translation action
The UI SHALL provide a translate action for eligible text conversation entries (assistant/system/thinking/user) that requests translation via the backend endpoint for the current entry content.

#### Scenario: Translate assistant entry
- **WHEN** a user activates Translate on a text-based assistant entry
- **THEN** the client requests a translation for the entry content from the backend
- **AND** the translated text is rendered below the original content

#### Scenario: Ineligible entry
- **WHEN** a conversation entry is tool output or contains code fences
- **THEN** the Translate action is not shown

### Requirement: Fixed translation direction
The system SHALL translate from English to Simplified Chinese (`zh-CN`) without language detection or user selection, and the setting SHALL remain frontend-only.

#### Scenario: Default translation direction
- **WHEN** a user triggers translation
- **THEN** the request uses `en` as the source language and `zh-CN` as the target language

#### Scenario: No language persistence
- **WHEN** a user refreshes the page
- **THEN** any translated content is cleared and the default `en` → `zh-CN` direction is used again

### Requirement: Ephemeral translation state
The system SHALL NOT persist translated content in stored conversation history.

#### Scenario: Reload attempt history
- **WHEN** the user reloads an attempt view
- **THEN** only original conversation entries are loaded with no translated content attached

### Requirement: Translation error handling
The UI SHALL surface translation failures without altering the original entry and provide a retry action.

#### Scenario: Provider failure
- **WHEN** the translation endpoint returns an error
- **THEN** the UI displays a non-blocking error state and allows retry

### Requirement: Server-side translation provider
The system SHALL route translation through a backend endpoint and SHALL call an OpenAI-compatible LLM endpoint without exposing credentials to clients.

#### Scenario: Successful translation
- **WHEN** a user triggers translation
- **THEN** the client sends a request to the backend translation endpoint
- **AND** the backend calls the OpenAI-compatible endpoint and returns translated text

#### Scenario: Missing configuration
- **WHEN** `KANBAN_OPENAI_API_BASE`, `KANBAN_OPENAI_API_KEY`, or `KANBAN_OPENAI_DEFAULT_MODEL` is not set and fallback envs are also missing
- **THEN** the backend returns an error response
- **AND** the UI shows a non-blocking error state with retry

### Requirement: OpenAI-compatible provider usage
The system SHALL use an OpenAI-compatible endpoint specified by `KANBAN_OPENAI_API_BASE` (fallback to `OPENAI_API_BASE`) as the default translation provider for the backend proxy.

#### Scenario: OpenAI-compatible request
- **WHEN** a user translates an entry from English to Chinese
- **THEN** the backend POSTs JSON to `{KANBAN_OPENAI_API_BASE}/v1/chat/completions`
- **AND** the request includes `model=KANBAN_OPENAI_DEFAULT_MODEL` (fallback to `OPENAI_DEFAULT_MODEL`), a system prompt that instructs translation only, and the user content
- **AND** the request authenticates using `KANBAN_OPENAI_API_KEY` (fallback to `OPENAI_API_KEY`)
- **AND** the request uses `en` and `zh-CN` for the source and target languages in the prompt


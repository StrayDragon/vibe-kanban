## ADDED Requirements
### Requirement: Client-side translation provider
The client SHALL call a public translation API directly from the browser without adding new backend routes or persisting any server-side state.

#### Scenario: Successful translation
- **WHEN** a user triggers translation
- **THEN** the client sends a request to the translation provider and renders the translated text on success

#### Scenario: No backend changes
- **WHEN** translation is used
- **THEN** no server-side logs or execution processes are created

### Requirement: On-demand translation action
The UI SHALL provide a translate action for eligible text conversation entries (assistant/system/thinking/user) that requests translation for the current entry content.

#### Scenario: Translate assistant entry
- **WHEN** a user activates Translate on a text-based assistant entry
- **THEN** the client requests a translation for the entry content
- **AND** the translated text is rendered below the original content

#### Scenario: Ineligible entry
- **WHEN** a conversation entry is tool output or contains code fences
- **THEN** the Translate action is not shown

### Requirement: MyMemory provider usage
The system SHALL use the MyMemory public API as the default translation provider for the frontend-only MVP.

#### Scenario: MyMemory request
- **WHEN** a user translates an entry from English to Chinese
- **THEN** the client calls `https://api.mymemory.translated.net/get` with `langpair=en|zh-CN`

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

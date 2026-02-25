## ADDED Requirements

### Requirement: Clear conversation loading with no processes
The UI SHALL clear conversation-history loading when there are no execution processes to load.

#### Scenario: Empty process list clears loading
- **WHEN** the execution-process list is empty and loading has completed
- **THEN** the conversation history loading state is `false`

#### Scenario: Still loading does not clear early
- **WHEN** the execution-process list is empty but is still loading
- **THEN** the conversation history loading state remains `true`

### Requirement: Log normalization resilience
The system MUST avoid panics when log sequences are anomalous or out-of-order, and SHALL emit a normalization error entry before continuing streaming.

#### Scenario: Tool result anomaly
- **WHEN** a tool result arrives without a matching pending tool call (or required index state is missing)
- **THEN** the stream emits an error entry describing the anomaly and continues processing subsequent events

### Requirement: Stable log item identity in the UI
The UI MUST use stable identifiers (entry index or patch key) when rendering raw and normalized log entries to avoid incorrect associations during history prepends/truncation.

#### Scenario: Prepending older history preserves identity
- **WHEN** a user loads older history that is prepended before existing items
- **THEN** existing rendered log items preserve identity and scroll position remains stable


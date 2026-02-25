## ADDED Requirements
### Requirement: Deterministic debounce testing
Frontend debounce tests SHALL use deterministic fake timers rather than wall-clock sleeps.

#### Scenario: Debounce update assertion
- **WHEN** a test validates a debounced update path
- **THEN** it advances fake timers explicitly and asserts expected calls without fixed real-time delays

### Requirement: No sleep-based retry masking
The test suite SHALL avoid fixed sleep-based retries that mask scheduling variance.

#### Scenario: Async UI expectation
- **WHEN** UI state settles asynchronously after timer advancement
- **THEN** tests use explicit async assertions (for example `waitFor`) instead of additional fixed sleeps

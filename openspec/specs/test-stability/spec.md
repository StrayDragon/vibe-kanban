# test-stability Specification

## Purpose
TBD - created by archiving change stabilize-workflow-debounce-tests. Update Purpose after archive.
## Requirements
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

### Requirement: Tests avoid repo-local temporary artifacts
The test and development workflows SHALL avoid writing temporary databases or run-state directories into the repository working tree by default.

#### Scenario: prepare-db uses OS temp paths
- **WHEN** a developer runs the DB preparation script
- **THEN** any temporary SQLite database is created under an OS temp directory (or a unique run directory)
- **AND** the repository working tree is not polluted by leftover DB files

### Requirement: E2E runs use unique run directories and clean up
The E2E runner SHALL create a unique run directory per invocation and SHALL clean it up during teardown.

#### Scenario: Aborted run does not poison the next run
- **WHEN** an E2E run is interrupted or fails
- **THEN** the next E2E run starts from a fresh run directory and does not reuse stale state

### Requirement: Global environment mutations are RAII-guarded and serialized
Tests that mutate process-global environment variables SHALL use RAII guards and SHALL serialize such tests to prevent cross-test interference.

#### Scenario: Env is restored after a test
- **WHEN** a test sets process-global environment variables
- **THEN** those variables are restored to their prior values when the test completes (including on panic)


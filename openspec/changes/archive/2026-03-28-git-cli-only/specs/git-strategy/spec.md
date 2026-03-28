# git-strategy Specification (Delta)

## ADDED Requirements

### Requirement: Git operations are executed via the Git CLI
The system SHALL execute Git repository operations by invoking the `git` CLI and SHALL NOT require libgit2-based runtime Git APIs for core workflows.

#### Scenario: Repository operation succeeds with Git CLI available
- **WHEN** the system performs a local repository operation (for example worktree ensure, diff, commit, merge, or rebase)
- **THEN** the operation is executed via the `git` executable and produces the expected result

#### Scenario: Missing git binary produces a clear diagnostic
- **WHEN** the system attempts a Git operation and the `git` executable is not available or not runnable
- **THEN** the operation fails with a clear error indicating that `git` is required
- **AND** the error does not include sensitive configuration or credential material

### Requirement: Git CLI invocations are safe for async runtimes
The system SHALL ensure that blocking Git CLI invocations do not block the async runtime event loop.

#### Scenario: Git operations do not block async runtime scheduling
- **WHEN** the system runs a Git operation from an async context
- **THEN** the implementation executes the Git CLI call in an async-safe manner (for example via a blocking threadpool)

### Requirement: Git credential material is not leaked in logs or errors
The system SHALL NOT include embedded credentials (for example tokens embedded in remote URLs) in logs, error strings, or API responses.

#### Scenario: Remote URL token is redacted on failure
- **WHEN** a Git operation fails and the remote URL contains credential material
- **THEN** the reported error message redacts the credential material


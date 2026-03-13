## ADDED Requirements

### Requirement: Repo provides a CI-equivalent QA command
The repo SHALL provide a single command that runs the same high-signal checks as CI so contributors can validate changes locally before opening a PR.

#### Scenario: QA command passes on a clean checkout
- **WHEN** a contributor runs `pnpm run qa` from the repo root on a clean checkout
- **THEN** the command exits successfully

### Requirement: CI fails if disallowed external domains reappear
The repo SHALL provide a deterministic check that fails CI when disallowed external domains are introduced in product surfaces, while allowing explicit upstream GitHub links.

#### Scenario: Disallowed domain causes failure
- **WHEN** a product surface file contains a disallowed external domain
- **THEN** the external-link guardrail command exits with a non-zero status
- **AND** the output identifies at least one matching file path and line number

#### Scenario: Upstream GitHub links remain allowed
- **WHEN** a product surface file contains `https://github.com/BloopAI/vibe-kanban` links
- **THEN** the external-link guardrail command exits successfully

### Requirement: Docs references do not contain dead internal links
The repo SHALL NOT contain documentation references to missing internal files (for example a README entry pointing to a non-existent `docs/*.md`).

#### Scenario: Docs link check catches missing referenced files
- **WHEN** a docs file references a missing repo file path
- **THEN** the docs-link guardrail command exits with a non-zero status
- **AND** the output identifies the referencing file and the missing target


## ADDED Requirements

### Requirement: Operator can add a repository to a configured project by filesystem path
The system SHALL provide a UI flow to add a repository to an existing configured project by entering a local filesystem path.

#### Scenario: Add repository succeeds
- **WHEN** an operator selects a project and submits a valid repository path
- **THEN** the repository is appended to that project's repository list
- **AND** the repository becomes selectable/visible in the project's repositories UI after reload

### Requirement: Add-by-path validates and normalizes repository paths
The system SHALL validate the provided path and SHALL normalize it to an absolute path suitable for projects configuration.

#### Scenario: Non-existent path is rejected
- **WHEN** an operator submits a path that does not exist on disk
- **THEN** the system rejects the request with a validation error that identifies the path issue

#### Scenario: Relative path is normalized or rejected deterministically
- **WHEN** an operator submits a relative path
- **THEN** the system either resolves it to an absolute path deterministically or rejects it with a clear validation error

### Requirement: Git repository roots are detected when possible
When the provided path is inside a Git worktree, the system SHALL prefer the Git repository root as the configured repository path.

#### Scenario: Path inside a Git repo resolves to repo root
- **WHEN** an operator submits a path that is inside a Git repository
- **THEN** the configured repository path is the Git repository root directory

#### Scenario: Non-Git directory falls back to directory path
- **WHEN** an operator submits a path that is not inside a Git repository
- **THEN** the configured repository path is the validated directory path

### Requirement: Repository additions are persisted via a VK-managed overlay YAML
Repository additions performed via UI SHALL be persisted in a VK-managed overlay YAML file under the config directory and applied as an additive overlay on top of `projects.yaml` / `projects.d/*.yaml`.

#### Scenario: Additions survive restart
- **WHEN** the server restarts
- **AND** configuration is reloaded
- **THEN** repositories previously added via UI still appear under the same project

### Requirement: Add-by-path is idempotent for duplicate repository paths
The system SHALL NOT create duplicate repository entries within a project.

#### Scenario: Adding the same path twice does not duplicate
- **WHEN** an operator adds a repository whose normalized path already exists in the target project
- **THEN** the system returns success as a no-op or a clear duplicate error
- **AND** the project repository list contains only one entry for that path

### Requirement: UI add-by-path does not modify operator-authored config files
The system SHALL NOT write to or overwrite `config.yaml`, `projects.yaml`, or any operator-authored `projects.d/*.yaml` file as part of the add-by-path flow.

#### Scenario: Only overlay file is updated
- **WHEN** an operator adds a repository by path
- **THEN** the only persisted config change is within the VK-managed overlay YAML file

### Requirement: Overlay persistence is safe and script-free
The overlay YAML used for repository additions SHALL NOT persist executable script bodies (for example `setup_script` / `cleanup_script`).

#### Scenario: Overlay contains only safe repo metadata
- **WHEN** the overlay YAML is written
- **THEN** it contains only safe repository metadata needed for selection (for example path and display name)
- **AND** it does not include script body fields


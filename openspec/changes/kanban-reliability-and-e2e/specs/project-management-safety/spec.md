## ADDED Requirements

### Requirement: Project creation is explicit and side-effect free until confirmation
The UI SHALL NOT create a project as a side effect of selecting a repository. Project creation SHALL require an explicit confirmation action.

#### Scenario: Selecting a repo does not create a project
- **WHEN** a user selects or highlights a repository during the project creation flow
- **THEN** no project is created until the user confirms creation explicitly

#### Scenario: Confirm creates the project using the chosen repo
- **WHEN** a user completes the project creation flow by confirming with a selected repository and project name
- **THEN** the system creates a project that is associated with the chosen repository path/ID

#### Scenario: Cancel creates no project
- **WHEN** a user cancels the project creation flow at any step
- **THEN** the system SHALL NOT create a project

### Requirement: Same-name projects are disambiguated in UI and destructive confirmations
The UI SHALL disambiguate same-name projects by displaying stable identifiers (repo path and/or IDs) anywhere the user chooses or deletes a project.

#### Scenario: Delete confirmation includes disambiguating identifiers
- **WHEN** a user attempts to delete a project
- **THEN** the confirmation UI includes the project name AND at least one disambiguator (repo path and/or project ID)

#### Scenario: Same-name projects remain distinguishable in selection UI
- **WHEN** two projects share the same display name
- **THEN** the project selection UI displays enough additional information (repo path and/or IDs) for a user to reliably choose the intended project

### Requirement: Unsafe repo paths require explicit acknowledgement
The system SHALL prevent or require explicit user acknowledgement for repository selections that are likely to be temporary or unsafe (for example worktree or temporary directories).

#### Scenario: Worktree-like repo path is blocked or requires explicit acknowledgement
- **WHEN** a user selects a repository path that matches an unsafe-path heuristic (e.g., worktree or temporary directory patterns)
- **THEN** the UI blocks creation OR requires an explicit acknowledgement before allowing project creation

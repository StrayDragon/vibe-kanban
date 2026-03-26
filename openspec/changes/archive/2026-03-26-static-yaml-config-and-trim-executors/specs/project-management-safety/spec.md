## MODIFIED Requirements

### Requirement: Project creation is explicit and side-effect free until confirmation
The UI SHALL NOT mutate the canonical YAML config (`config.yaml`) as a side effect of selecting a repository. Project creation SHALL require an explicit confirmation action and SHALL present an operator-visible YAML snippet representing the project definition to be applied to `config.yaml`.

#### Scenario: Selecting a repo does not persist a project
- **WHEN** a user selects or highlights a repository during the project creation flow
- **THEN** no change is applied to `config.yaml`

#### Scenario: Confirm produces a YAML snippet using the chosen repo
- **WHEN** a user completes the project creation flow by confirming with a selected repository and project name
- **THEN** the system presents a YAML snippet that is associated with the chosen repository path/ID
- **AND** it instructs the operator to apply the snippet to `config.yaml` and trigger a reload

#### Scenario: Cancel writes no project
- **WHEN** a user cancels the project creation flow at any step
- **THEN** the system SHALL NOT apply any change to `config.yaml`

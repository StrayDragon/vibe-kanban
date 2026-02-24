## ADDED Requirements

### Requirement: Persist Task Group node instructions
The system SHALL persist optional `TaskGroupNode.instructions` when creating or updating a Task Group graph.

#### Scenario: Instructions persist after update
- **WHEN** a Task Group graph update includes node instructions
- **THEN** subsequent reads return the same instruction content

#### Scenario: Missing instructions remain empty
- **WHEN** a node does not include `instructions`
- **THEN** subsequent reads still return empty/missing instructions for that node

### Requirement: Prompt augmentation for Task Groups
The system SHALL append non-empty node instructions to the task prompt when starting an attempt from a Task Group node.

#### Scenario: Instructions are appended to the prompt
- **WHEN** an attempt is started from a node with non-empty instructions
- **THEN** the initial prompt includes the instruction content

#### Scenario: No instructions does not change the prompt
- **WHEN** an attempt is started from a node with no instructions
- **THEN** the initial prompt matches the base task prompt

#### Scenario: Blank instructions are not appended
- **WHEN** node instructions are empty or whitespace-only
- **THEN** the initial prompt matches the base task prompt

### Requirement: Node instructions editing UI
The UI SHALL provide a way to edit node instructions in the Task Group workflow view.

#### Scenario: Edit instructions in the workflow UI
- **WHEN** a user edits node instructions in the workflow view
- **THEN** the updated instructions are saved to the Task Group graph

#### Scenario: Clear instructions in the workflow UI
- **WHEN** a user clears node instructions in the workflow view
- **THEN** the node is saved with empty/missing instructions


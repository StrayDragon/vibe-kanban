## MODIFIED Requirements

### Requirement: Prompt augmentation for Task Groups
The system SHALL append applicable task-group (milestone) context and non-empty node instructions to the task prompt when starting an attempt from a Task Group node.

Milestone context includes:
- objective (if present and non-empty)
- definition of done (if present and non-empty)

#### Scenario: Milestone context is appended to the prompt
- **WHEN** an attempt is started from a node whose task group has a non-empty objective or definition of done
- **THEN** the initial prompt includes that milestone context

#### Scenario: Node instructions are appended to the prompt
- **WHEN** an attempt is started from a node with non-empty instructions
- **THEN** the initial prompt includes the instruction content

#### Scenario: No milestone context and no instructions does not change the prompt
- **WHEN** an attempt is started from a node whose task group has no milestone context
- **AND** the node has no instructions
- **THEN** the initial prompt matches the base task prompt

#### Scenario: Blank milestone fields are not appended
- **WHEN** milestone objective or definition of done are empty or whitespace-only
- **THEN** they are not appended to the prompt

#### Scenario: Blank instructions are not appended
- **WHEN** node instructions are empty or whitespace-only
- **THEN** the initial prompt matches the base task prompt (aside from any applicable milestone context)

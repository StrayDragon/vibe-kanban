## ADDED Requirements
### Requirement: Command source selection
The system SHALL resolve the executable command for each `BaseCodingAgent` using the following priority when no base command override is provided: pnpm global install, npm global install, then `npx` with `@latest`. The system SHALL NOT use `pnpm dlx` as a fallback.

#### Scenario: pnpm global install available
- **WHEN** the agent package is installed globally via pnpm
- **THEN** the resolved command uses the pnpm global version and records that version

#### Scenario: npm global install available
- **WHEN** pnpm has no global installation but npm does
- **THEN** the resolved command uses the npm global version and records that version

#### Scenario: no global install found
- **WHEN** neither pnpm nor npm lists the package
- **THEN** the resolved command uses `npx` with `@latest` and marks the source as a fallback

#### Scenario: base command override provided
- **WHEN** a profile provides `base_command_override`
- **THEN** the resolved command uses the override and the source is marked as `override`

#### Scenario: package listed but binary not resolved
- **WHEN** pnpm or npm lists the package but the executable path cannot be resolved
- **THEN** the system treats that source as unavailable and continues to the next priority

### Requirement: Async initialization and caching
The system SHALL resolve agent command metadata asynchronously at startup and cache results for subsequent spawns.

#### Scenario: startup does not block
- **WHEN** the server starts
- **THEN** command resolution runs in the background and does not delay startup

#### Scenario: spawn while resolution pending
- **WHEN** an executor is spawned before its resolution completes
- **THEN** the system resolves the command before spawning and updates the cache

#### Scenario: cached resolution used
- **WHEN** an executor is spawned after resolution completes
- **THEN** the cached command is used without re-running detection

### Requirement: Version visibility in settings
The system SHALL expose the resolved command source and version for every agent in user system info and surface it in Agent Settings.

#### Scenario: show installed version
- **WHEN** an agent uses a pnpm or npm global version
- **THEN** the settings UI displays the version and source

#### Scenario: show fallback notice
- **WHEN** an agent falls back to `npx @latest`
- **THEN** the settings UI shows a notice that latest will be used

#### Scenario: non-node executor version is unknown
- **WHEN** an agent does not map to a Node package
- **THEN** the system reports the version as `unknown` and the UI shows it as such

#### Scenario: settings refresh behavior
- **WHEN** the user refreshes system data in existing flows
- **THEN** the UI updates the displayed command source and version without a dedicated refresh control

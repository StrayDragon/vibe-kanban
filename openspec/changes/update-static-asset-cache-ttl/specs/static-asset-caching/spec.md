## ADDED Requirements
### Requirement: Hashed asset cache control
The system SHALL serve hashed static assets with `Cache-Control: public, max-age=31536000, immutable`.

#### Scenario: Hashed JS bundle request
- **WHEN** a request is made for a filename containing a content hash (for example, `/assets/index-<hash>.js`)
- **THEN** the response includes `Cache-Control: public, max-age=31536000, immutable`

### Requirement: Non-hashed asset cache control
The system SHALL serve non-hashed static assets with a shorter cache TTL.

#### Scenario: Non-hashed static request
- **WHEN** a request is made for a static asset without a content hash
- **THEN** the response uses a shorter cache TTL than hashed assets

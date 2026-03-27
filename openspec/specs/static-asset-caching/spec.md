# static-asset-caching Specification

## Purpose
TBD - created by archiving change update-static-asset-cache-ttl. Update Purpose after archive.
## Requirements
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

### Requirement: API-served user content is not publicly cacheable
The system SHALL serve user/task-scoped assets under `/api/**` (for example uploaded images) with cache headers that are not publicly cacheable.

Responses MUST NOT include `Cache-Control: public`.

#### Scenario: Uploaded image response is private
- **WHEN** a client requests an uploaded image file via an `/api/**` endpoint
- **THEN** the response does not include `Cache-Control: public`
- **AND** the response includes a `Cache-Control` directive that prevents shared caching (for example `private` or `no-store`)


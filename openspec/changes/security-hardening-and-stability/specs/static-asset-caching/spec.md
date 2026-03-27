# static-asset-caching Specification

## ADDED Requirements

### Requirement: API-served user content is not publicly cacheable
The system SHALL serve user/task-scoped assets under `/api/**` (for example uploaded images) with cache headers that are not publicly cacheable.

Responses MUST NOT include `Cache-Control: public`.

#### Scenario: Uploaded image response is private
- **WHEN** a client requests an uploaded image file via an `/api/**` endpoint
- **THEN** the response does not include `Cache-Control: public`
- **AND** the response includes a `Cache-Control` directive that prevents shared caching (for example `private` or `no-store`)


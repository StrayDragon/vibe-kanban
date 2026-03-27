# frontend-security-cleanup Specification

## ADDED Requirements

### Requirement: User-uploaded images must not allow SVG execution
The system SHALL reject SVG uploads (for example `.svg` or `image/svg+xml`) for user-uploaded images that are later served from the same origin.

#### Scenario: SVG upload is rejected
- **WHEN** a client uploads an image with SVG format
- **THEN** the system rejects the upload with a 4xx error


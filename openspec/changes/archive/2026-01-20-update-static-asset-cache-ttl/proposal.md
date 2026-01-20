# Change: long-lived cache headers for hashed static assets

## Why
DevTools Cache insight shows zero TTL for hashed frontend bundles, wasting repeat-visit bandwidth.

## What Changes
- Serve hashed static assets with `Cache-Control: public, max-age=31536000, immutable`.
- Keep shorter caching for non-hashed/static URLs.

## Impact
- Affected specs: static-asset-caching
- Affected code: static asset serving (backend and/or frontend hosting layer)

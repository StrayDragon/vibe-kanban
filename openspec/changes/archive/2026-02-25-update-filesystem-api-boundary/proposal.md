# Change: Update filesystem API boundary

## Why
`/api/filesystem/directory` currently accepts arbitrary paths and can enumerate host directories outside project workspaces. This is a security risk even in internal networks.

## What Changes
- Restrict filesystem listing and git-repo discovery APIs to configured workspace roots.
- Canonicalize and validate requested paths before filesystem access.
- Return structured `403` errors for out-of-bound paths.
- Keep current authentication model unchanged in this change.

## Impact
- Affected specs: `filesystem-api-boundary`
- Affected code: `crates/server/src/routes/filesystem.rs`, `crates/services/src/services/filesystem.rs`
- Out of scope: changing default access-control mode or token policy.

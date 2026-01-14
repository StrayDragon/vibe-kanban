# Operations

This document covers practical guidance for running Vibe Kanban, with a focus on SQLite-backed deployments.

## Asset directory

Vibe Kanban stores runtime state in an “asset directory” (config, credentials, SQLite DB, logs).

- **Dev builds** default to `dev_assets/` (repo root).
- **Release builds** default to the OS app data directory (see `utils::assets::asset_dir()`).
- **Override** with `VIBE_ASSET_DIR=/absolute/path`.

Key files:
- `db.sqlite` (SQLite database)
- `config.json`, `profiles.json`, `credentials.json`

## SQLite lock handling

If you see errors like “database is locked”, start with these checks:

1. **Single writer**: ensure only one backend instance is pointing at the same `VIBE_ASSET_DIR`.
2. **Local disk**: avoid placing the asset dir on NFS/SMB or other networked filesystems (SQLite locking can be unreliable).
3. **Clean shutdown**: stop the backend before backing up or moving the DB.
4. **Identify lock holders** (Linux/macOS): `lsof dev_assets/db.sqlite` (or your overridden path).

If the DB remains locked after a crash, a full backend restart usually clears it. If it doesn’t, restore from a known-good backup.

## Backups

Preferred approach (consistent snapshot):
1. Stop the backend.
2. Copy `db.sqlite` to your backup location.
3. Restart the backend.

If SQLite is running in WAL mode on your platform, you may also see `db.sqlite-wal` and `db.sqlite-shm`. Back up those files together with `db.sqlite`.

## Routine ops checklist

Daily:
- Confirm the backend is running and `/api/health` responds.
- Watch logs for repeated “database is locked” or unexpected process failures.
- Check disk usage of the asset directory.

Weekly:
- Verify backups restore cleanly (copy DB into a scratch asset dir and start the backend).
- Review recent failed execution processes for systemic issues (missing tools, auth expiry).

Monthly:
- Review retention/cleanup knobs (e.g. legacy JSONL cleanup) and adjust to your deployment size.
- Audit config changes and ensure the latest config migration version is applied.


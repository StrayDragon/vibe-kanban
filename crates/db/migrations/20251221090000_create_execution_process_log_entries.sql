PRAGMA foreign_keys = ON;

CREATE TABLE execution_process_log_entries (
    execution_id      BLOB NOT NULL,
    channel           TEXT NOT NULL
                     CHECK (channel IN ('raw', 'normalized')),
    entry_index       INTEGER NOT NULL,
    entry_json        TEXT NOT NULL,
    created_at        TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at        TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (execution_id) REFERENCES execution_processes(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_execution_process_log_entries_unique
    ON execution_process_log_entries (execution_id, channel, entry_index);

CREATE INDEX idx_execution_process_log_entries_exec_channel_index
    ON execution_process_log_entries (execution_id, channel, entry_index DESC);

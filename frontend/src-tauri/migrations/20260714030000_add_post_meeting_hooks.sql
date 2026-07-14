CREATE TABLE IF NOT EXISTS post_meeting_hooks (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('artifact_export', 'obsidian_export')),
    destination TEXT NOT NULL,
    folder TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS post_meeting_hook_runs (
    id TEXT PRIMARY KEY NOT NULL,
    hook_id TEXT NOT NULL,
    meeting_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('success', 'failed')),
    detail TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (hook_id) REFERENCES post_meeting_hooks(id) ON DELETE CASCADE,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_post_meeting_hook_runs_meeting
    ON post_meeting_hook_runs(meeting_id, created_at);

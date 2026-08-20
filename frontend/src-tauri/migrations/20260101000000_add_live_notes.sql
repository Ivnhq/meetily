-- Store structured Live Notes state for a saved meeting.
CREATE TABLE IF NOT EXISTS live_notes (
    meeting_id TEXT PRIMARY KEY NOT NULL,
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_live_notes_meeting_id ON live_notes(meeting_id);

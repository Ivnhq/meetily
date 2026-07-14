CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    starts_at TEXT NOT NULL,
    ends_at TEXT NOT NULL,
    location TEXT,
    join_url TEXT,
    notes TEXT,
    source TEXT NOT NULL DEFAULT 'ics',
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_time
    ON calendar_events(starts_at, ends_at);

CREATE TABLE IF NOT EXISTS meeting_context (
    meeting_id TEXT PRIMARY KEY NOT NULL,
    calendar_event_id TEXT,
    context_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    FOREIGN KEY (calendar_event_id) REFERENCES calendar_events(id) ON DELETE SET NULL
);

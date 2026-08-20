CREATE TABLE IF NOT EXISTS speaker_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    source TEXT,
    fingerprint_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE transcripts ADD COLUMN speaker_fingerprint_json TEXT;

CREATE TABLE IF NOT EXISTS meeting_speaker_labels (
    meeting_id TEXT NOT NULL,
    speaker_id TEXT NOT NULL,
    profile_id TEXT,
    display_name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (meeting_id, speaker_id),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id) REFERENCES speaker_profiles(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_meeting_speaker_labels_profile
    ON meeting_speaker_labels(profile_id);

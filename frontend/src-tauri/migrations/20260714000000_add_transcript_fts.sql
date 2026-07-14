-- Full-text transcript index used by local search, CLI, and cited meeting Q&A.
CREATE VIRTUAL TABLE IF NOT EXISTS transcript_fts USING fts5(
    transcript_id UNINDEXED,
    meeting_id UNINDEXED,
    transcript,
    speaker,
    timestamp UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO transcript_fts (transcript_id, meeting_id, transcript, speaker, timestamp)
SELECT t.id, t.meeting_id, t.transcript, COALESCE(t.speaker, ''), t.timestamp
FROM transcripts t
WHERE NOT EXISTS (
    SELECT 1 FROM transcript_fts f WHERE f.transcript_id = t.id
);

CREATE TRIGGER IF NOT EXISTS transcripts_fts_insert
AFTER INSERT ON transcripts BEGIN
    INSERT INTO transcript_fts (transcript_id, meeting_id, transcript, speaker, timestamp)
    VALUES (new.id, new.meeting_id, new.transcript, COALESCE(new.speaker, ''), new.timestamp);
END;

CREATE TRIGGER IF NOT EXISTS transcripts_fts_delete
AFTER DELETE ON transcripts BEGIN
    DELETE FROM transcript_fts WHERE transcript_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS transcripts_fts_update
AFTER UPDATE OF transcript, speaker, timestamp, meeting_id ON transcripts BEGIN
    DELETE FROM transcript_fts WHERE transcript_id = old.id;
    INSERT INTO transcript_fts (transcript_id, meeting_id, transcript, speaker, timestamp)
    VALUES (new.id, new.meeting_id, new.transcript, COALESCE(new.speaker, ''), new.timestamp);
END;

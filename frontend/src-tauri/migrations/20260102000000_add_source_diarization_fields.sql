-- Migration: Add source diarization metadata to transcripts
-- `speaker` already exists from the earlier speaker field migration.

ALTER TABLE transcripts ADD COLUMN source TEXT;
ALTER TABLE transcripts ADD COLUMN speaker_id TEXT;
ALTER TABLE transcripts ADD COLUMN source_overlap BOOLEAN DEFAULT 0;

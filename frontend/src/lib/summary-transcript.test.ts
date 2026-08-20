import { describe, expect, it } from 'vitest';
import type { Transcript } from '@/types';
import { buildSummaryTranscriptPayload } from './summary-transcript';

describe('buildSummaryTranscriptPayload', () => {
  it('preserves speaker labels for the summary while keeping raw text for language detection', () => {
    const transcripts: Transcript[] = [
      {
        id: 'segment-1',
        text: 'We approved the pilot.',
        timestamp: '10:00:01',
        audio_start_time: 61.8,
        speaker: 'Remote speaker 1',
      },
      {
        id: 'segment-2',
        text: 'I will send the scope.',
        timestamp: '10:00:05',
        audio_start_time: 65.2,
        speaker: 'Local speaker',
      },
    ];

    expect(buildSummaryTranscriptPayload(transcripts)).toEqual({
      transcriptText:
        '[01:01] Remote speaker 1: We approved the pilot.\n[01:05] Local speaker: I will send the scope.',
      transcriptTexts: ['We approved the pilot.', 'I will send the scope.'],
    });
  });
});

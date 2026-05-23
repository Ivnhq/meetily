import { describe, expect, it, vi } from 'vitest';
import type { Transcript } from '@/types';
import {
  createEmptyLiveNotesState,
  createLiveNotesRecap,
  formatLiveNotesAsMarkdown,
  formatTranscriptChunk,
  mergeLiveNotesUpdate,
} from './liveNotes';

function transcript(overrides: Partial<Transcript>): Transcript {
  return {
    id: overrides.id ?? 'segment-1',
    text: overrides.text ?? 'Hello',
    timestamp: overrides.timestamp ?? '10:00:00',
    ...overrides,
  };
}

describe('live notes helpers', () => {
  it('formats transcript chunks with recording-relative timestamps', () => {
    expect(
      formatTranscriptChunk([
        transcript({ text: 'First point', audio_start_time: 65.2, audio_end_time: 68.5 }),
        transcript({ text: 'Fallback point', chunk_start_time: 7 }),
      ])
    ).toBe('[01:05-01:08 | 65200-68500ms] First point\n[00:07 | 7000ms] Fallback point');
  });

  it('merges updates deterministically and deduplicates by text', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-05-23T12:00:00.000Z'));

    const state = {
      ...createEmptyLiveNotesState('meeting-1'),
      currentTopic: 'Launch plan',
      rollingSummary: ['Existing summary'],
      keyPoints: ['Existing key point'],
      decisions: [{ text: 'Ship the first slice' }],
      actionItems: [{ text: 'Ivan to test recording', owner: 'Ivan' }],
      openQuestions: ['Who reviews notes?'],
      risks: ['Model may be slow'],
      highlights: [
        {
          id: 'manual-1',
          text: 'Important manual moment',
          userMarked: true,
          transcriptStartMs: 1_000,
          transcriptEndMs: 2_000,
          createdAt: '2026-05-23T11:00:00.000Z',
        },
      ],
    };

    const merged = mergeLiveNotesUpdate(state, {
      currentTopic: '  Pilot workflow  ',
      rollingSummary: ['New summary', 'existing summary', '  '],
      keyPoints: ['New key point'],
      decisions: [{ text: 'Ship the first slice' }, { text: 'Use local-only recap' }],
      actionItems: [{ text: 'Ivan to test recording' }, { text: 'Add persistence later' }],
      openQuestions: ['Who reviews notes?', 'Where should notes persist?'],
      risks: ['Model may be slow', 'Recap could overwrite notes'],
      highlights: [{ text: 'Generated moment', transcriptStartMs: 3_000, transcriptEndMs: 4_000 }],
    });

    expect(merged.currentTopic).toBe('Pilot workflow');
    expect(merged.rollingSummary).toEqual(['New summary', 'Existing summary']);
    expect(merged.keyPoints).toEqual(['New key point', 'Existing key point']);
    expect(merged.decisions.map((item) => item.text)).toEqual([
      'Use local-only recap',
      'Ship the first slice',
    ]);
    expect(merged.actionItems).toEqual([
      { text: 'Add persistence later' },
      { text: 'Ivan to test recording', owner: 'Ivan' },
    ]);
    expect(merged.openQuestions).toEqual(['Where should notes persist?', 'Who reviews notes?']);
    expect(merged.risks).toEqual(['Recap could overwrite notes', 'Model may be slow']);
    expect(merged.highlights).toEqual([
      state.highlights[0],
      {
        id: 'generated-1779537600000-0',
        text: 'Generated moment',
        transcriptStartMs: 3_000,
        transcriptEndMs: 4_000,
        userMarked: false,
        createdAt: '2026-05-23T12:00:00.000Z',
      },
    ]);

    vi.useRealTimers();
  });

  it('normalizes recaps without creating canonical live note ids', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-05-23T12:30:00.000Z'));

    const recap = createLiveNotesRecap({
      currentTopic: '  Decisions so far  ',
      rollingSummary: [' The team picked the MVP path. ', ''],
      keyPoints: ['Tests come first'],
      decisions: [{ text: 'Keep recap separate' }],
      actionItems: [{ text: 'Add persistence in the next pass' }],
      openQuestions: [''],
      risks: [''],
      highlights: [{ text: 'User asked what they missed' }],
    });

    expect(recap).toEqual({
      generatedAt: '2026-05-23T12:30:00.000Z',
      currentTopic: 'Decisions so far',
      rollingSummary: ['The team picked the MVP path.'],
      keyPoints: ['Tests come first'],
      decisions: [{ text: 'Keep recap separate' }],
      actionItems: [{ text: 'Add persistence in the next pass' }],
      openQuestions: [],
      risks: [],
      highlights: [{ text: 'User asked what they missed' }],
    });

    vi.useRealTimers();
  });

  it('exports live notes as markdown with evidence timestamps', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-05-23T13:00:00.000Z'));

    const markdown = formatLiveNotesAsMarkdown({
      ...createEmptyLiveNotesState('meeting-1'),
      updatedAt: '2026-05-23T12:55:00.000Z',
      currentTopic: 'Pilot workflow',
      rollingSummary: ['The team picked the MVP path.'],
      keyPoints: ['Tests come first'],
      decisions: [
        {
          text: 'Use SQLite persistence',
          transcriptStartMs: 29_000,
          transcriptEndMs: 38_000,
        },
      ],
      actionItems: [
        {
          text: 'Maya to test a five minute recording',
          owner: 'Maya',
          transcriptStartMs: 15_000,
          transcriptEndMs: 28_000,
        },
      ],
      openQuestions: ['Will Gemma return clean JSON?'],
      risks: ['CPU-only models may be slow'],
      highlights: [
        {
          id: 'manual-1',
          text: 'Important MVP decision',
          userMarked: true,
          transcriptStartMs: 5_000,
          transcriptEndMs: 14_000,
          createdAt: '2026-05-23T12:50:00.000Z',
        },
      ],
    });

    expect(markdown).toContain('# Live Notes');
    expect(markdown).toContain('- Use SQLite persistence [00:29-00:38]');
    expect(markdown).toContain('- Maya to test a five minute recording (owner: Maya) [00:15-00:28]');
    expect(markdown).toContain('- User-marked: Important MVP decision [00:05-00:14]');

    vi.useRealTimers();
  });
});

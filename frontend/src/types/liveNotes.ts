import { Transcript } from '@/types';

export interface LiveActionItem {
  text: string;
  owner?: string;
  dueDate?: string;
  transcriptStartMs?: number;
  transcriptEndMs?: number;
  confidence?: number;
}

export interface LiveDecision {
  text: string;
  transcriptStartMs?: number;
  transcriptEndMs?: number;
  confidence?: number;
}

export interface LiveHighlight {
  id: string;
  text?: string;
  userMarked: boolean;
  transcriptStartMs: number;
  transcriptEndMs: number;
  note?: string;
  createdAt: string;
}

export interface LiveNotesState {
  meetingId: string;
  updatedAt: string | null;
  currentTopic?: string;
  rollingSummary: string[];
  keyPoints: string[];
  decisions: LiveDecision[];
  actionItems: LiveActionItem[];
  openQuestions: string[];
  risks: string[];
  highlights: LiveHighlight[];
}

export interface LiveNotesUpdate {
  currentTopic?: string;
  rollingSummary?: string[];
  keyPoints?: string[];
  decisions?: LiveDecision[];
  actionItems?: LiveActionItem[];
  openQuestions?: string[];
  risks?: string[];
  highlights?: Omit<LiveHighlight, 'id' | 'createdAt' | 'userMarked'>[];
}

export interface LiveNotesModelConfig {
  provider: string;
  model: string;
  ollamaEndpoint?: string | null;
  customOpenAIEndpoint?: string | null;
  customOpenAIApiKey?: string | null;
  maxTokens?: number | null;
  temperature?: number | null;
  topP?: number | null;
}

export function createEmptyLiveNotesState(meetingId: string): LiveNotesState {
  return {
    meetingId,
    updatedAt: null,
    rollingSummary: [],
    keyPoints: [],
    decisions: [],
    actionItems: [],
    openQuestions: [],
    risks: [],
    highlights: [],
  };
}

export function formatTranscriptChunk(transcripts: Transcript[]): string {
  return transcripts
    .map((transcript) => {
      const start = transcript.audio_start_time ?? transcript.chunk_start_time ?? 0;
      const minutes = Math.floor(start / 60);
      const seconds = Math.floor(start % 60);
      const stamp = `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
      return `[${stamp}] ${transcript.text}`;
    })
    .join('\n');
}

function mergeUniqueStrings(existing: string[], incoming: string[] | undefined, maxItems = 8): string[] {
  if (!incoming?.length) return existing;

  const seen = new Set<string>();
  return [...incoming, ...existing]
    .map((item) => item.trim())
    .filter((item) => {
      if (!item || seen.has(item.toLowerCase())) return false;
      seen.add(item.toLowerCase());
      return true;
    })
    .slice(0, maxItems);
}

function mergeObjectsByText<T extends { text: string }>(
  existing: T[],
  incoming: T[] | undefined,
  maxItems = 8
): T[] {
  if (!incoming?.length) return existing;

  const seen = new Set<string>();
  return [...incoming, ...existing]
    .filter((item) => {
      const key = item.text?.trim().toLowerCase();
      if (!key || seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .slice(0, maxItems);
}

export function mergeLiveNotesUpdate(
  state: LiveNotesState,
  update: LiveNotesUpdate
): LiveNotesState {
  const generatedHighlights: LiveHighlight[] = (update.highlights ?? []).map((highlight, index) => ({
    ...highlight,
    id: `generated-${Date.now()}-${index}`,
    userMarked: false,
    createdAt: new Date().toISOString(),
  }));

  return {
    ...state,
    updatedAt: new Date().toISOString(),
    currentTopic: update.currentTopic?.trim() || state.currentTopic,
    rollingSummary: mergeUniqueStrings(state.rollingSummary, update.rollingSummary, 6),
    keyPoints: mergeUniqueStrings(state.keyPoints, update.keyPoints, 8),
    decisions: mergeObjectsByText(state.decisions, update.decisions, 8),
    actionItems: mergeObjectsByText(state.actionItems, update.actionItems, 10),
    openQuestions: mergeUniqueStrings(state.openQuestions, update.openQuestions, 8),
    risks: mergeUniqueStrings(state.risks, update.risks, 6),
    highlights: [...state.highlights, ...generatedHighlights].slice(-12),
  };
}

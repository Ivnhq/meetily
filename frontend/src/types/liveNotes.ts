import type { Transcript } from '@/types';

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

export interface LiveGeneratedHighlight {
  text?: string;
  transcriptStartMs?: number;
  transcriptEndMs?: number;
  note?: string;
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
  highlights?: LiveGeneratedHighlight[];
}

export interface LiveNotesRecap {
  generatedAt: string;
  currentTopic?: string;
  rollingSummary: string[];
  keyPoints: string[];
  decisions: LiveDecision[];
  actionItems: LiveActionItem[];
  openQuestions: string[];
  risks: string[];
  highlights: LiveGeneratedHighlight[];
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

const LIVE_NOTES_STORAGE_PREFIX = 'meetily_live_notes_state:';

function getLiveNotesStorageKey(meetingId: string): string {
  return `${LIVE_NOTES_STORAGE_PREFIX}${meetingId}`;
}

function isLiveNotesState(value: unknown): value is LiveNotesState {
  if (!value || typeof value !== 'object') return false;

  const candidate = value as Partial<LiveNotesState>;
  return (
    typeof candidate.meetingId === 'string' &&
    Array.isArray(candidate.rollingSummary) &&
    Array.isArray(candidate.keyPoints) &&
    Array.isArray(candidate.decisions) &&
    Array.isArray(candidate.actionItems) &&
    Array.isArray(candidate.openQuestions) &&
    Array.isArray(candidate.risks) &&
    Array.isArray(candidate.highlights)
  );
}

export function loadLiveNotesFromLocalStorage(meetingId: string): LiveNotesState | null {
  if (typeof window === 'undefined') return null;

  try {
    const raw = window.localStorage.getItem(getLiveNotesStorageKey(meetingId));
    if (!raw) return null;

    const parsed = JSON.parse(raw);
    return isLiveNotesState(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function saveLiveNotesToLocalStorage(state: LiveNotesState): void {
  if (typeof window === 'undefined') return;

  try {
    window.localStorage.setItem(getLiveNotesStorageKey(state.meetingId), JSON.stringify(state));
  } catch {
    // Local cache is best-effort; SQLite is the durable store after save.
  }
}

export function consumeLiveNotesFromLocalStorage(meetingId: string): LiveNotesState | null {
  if (typeof window === 'undefined') return null;

  const liveNotes = loadLiveNotesFromLocalStorage(meetingId);
  try {
    window.localStorage.removeItem(getLiveNotesStorageKey(meetingId));
  } catch {
    // Nothing to do if local storage is unavailable.
  }

  return liveNotes;
}

export function formatTranscriptChunk(transcripts: Transcript[]): string {
  return transcripts
    .map((transcript) => {
      const start = transcript.audio_start_time ?? transcript.chunk_start_time ?? 0;
      const end = transcript.audio_end_time ?? (transcript.duration ? start + transcript.duration : start);
      const minutes = Math.floor(start / 60);
      const seconds = Math.floor(start % 60);
      const startStamp = `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
      const endMinutes = Math.floor(end / 60);
      const endSeconds = Math.floor(end % 60);
      const endStamp = `${endMinutes.toString().padStart(2, '0')}:${endSeconds.toString().padStart(2, '0')}`;
      const startMs = Math.round(start * 1000);
      const endMs = Math.round(end * 1000);
      const range = endMs > startMs
        ? `${startStamp}-${endStamp} | ${startMs}-${endMs}ms`
        : `${startStamp} | ${startMs}ms`;

      const label = transcript.speaker || transcript.source;
      return `[${range}] ${label ? `${label}: ` : ''}${transcript.text}`;
    })
    .join('\n');
}

function mergeUniqueStrings(existing: string[], incoming: string[] | undefined, maxItems = 8): string[] {
  const seen = new Set<string>();
  const existingItems = existing
    .map((item) => item.trim())
    .filter((item) => {
      if (!item || seen.has(item.toLowerCase())) return false;
      seen.add(item.toLowerCase());
      return true;
    });

  if (!incoming?.length) return existingItems.slice(0, maxItems);

  const newItems = incoming
    .map((item) => item.trim())
    .filter((item) => {
      if (!item || seen.has(item.toLowerCase())) return false;
      seen.add(item.toLowerCase());
      return true;
    });

  return [...newItems, ...existingItems].slice(0, maxItems);
}

function mergeObjectsByText<T extends { text: string }>(
  existing: T[],
  incoming: T[] | undefined,
  maxItems = 8
): T[] {
  const seen = new Set<string>();
  const existingItems = existing
    .filter((item) => {
      const key = item.text?.trim().toLowerCase();
      if (!key || seen.has(key)) return false;
      seen.add(key);
      return true;
    });

  if (!incoming?.length) return existingItems.slice(0, maxItems);

  const newItems = incoming
    .filter((item) => {
      const key = item.text?.trim().toLowerCase();
      if (!key || seen.has(key)) return false;
      seen.add(key);
      return true;
    });

  return [...newItems, ...existingItems].slice(0, maxItems);
}

export function mergeLiveNotesUpdate(
  state: LiveNotesState,
  update: LiveNotesUpdate
): LiveNotesState {
  const generatedHighlights: LiveHighlight[] = (update.highlights ?? [])
    .filter(
      (highlight): highlight is LiveGeneratedHighlight & { transcriptStartMs: number; transcriptEndMs: number } =>
        typeof highlight.transcriptStartMs === 'number' &&
        typeof highlight.transcriptEndMs === 'number'
    )
    .map((highlight, index) => ({
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

export function createLiveNotesRecap(update: LiveNotesUpdate): LiveNotesRecap {
  return {
    generatedAt: new Date().toISOString(),
    currentTopic: update.currentTopic?.trim() || undefined,
    rollingSummary: update.rollingSummary?.map((item) => item.trim()).filter(Boolean) ?? [],
    keyPoints: update.keyPoints?.map((item) => item.trim()).filter(Boolean) ?? [],
    decisions: update.decisions?.filter((item) => item.text?.trim()) ?? [],
    actionItems: update.actionItems?.filter((item) => item.text?.trim()) ?? [],
    openQuestions: update.openQuestions?.map((item) => item.trim()).filter(Boolean) ?? [],
    risks: update.risks?.map((item) => item.trim()).filter(Boolean) ?? [],
    highlights: update.highlights?.filter((item) => item.text?.trim()) ?? [],
  };
}

function formatExportTime(ms: number): string {
  const totalSeconds = Math.max(Math.floor(ms / 1000), 0);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  return `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
}

function formatExportRange(startMs?: number, endMs?: number): string {
  if (typeof startMs !== 'number' && typeof endMs !== 'number') return '';

  const label = typeof startMs === 'number' && typeof endMs === 'number'
    ? `${formatExportTime(startMs)}-${formatExportTime(endMs)}`
    : formatExportTime(startMs ?? endMs ?? 0);

  return ` [${label}]`;
}

function addStringSection(lines: string[], title: string, items: string[]): void {
  lines.push(`## ${title}`, '');

  if (items.length === 0) {
    lines.push('None noted.', '');
    return;
  }

  items.forEach((item) => lines.push(`- ${item}`));
  lines.push('');
}

export function formatLiveNotesAsMarkdown(state: LiveNotesState): string {
  const lines: string[] = ['# Live Notes', ''];

  if (state.updatedAt) {
    lines.push(`Updated: ${new Date(state.updatedAt).toLocaleString()}`, '');
  }

  lines.push('## Current Topic', '');
  lines.push(state.currentTopic?.trim() || 'None noted.', '');

  addStringSection(lines, 'Rolling Summary', state.rollingSummary);
  addStringSection(lines, 'Key Points', state.keyPoints);

  lines.push('## Decisions', '');
  if (state.decisions.length === 0) {
    lines.push('None noted.', '');
  } else {
    state.decisions.forEach((decision) => {
      lines.push(`- ${decision.text}${formatExportRange(decision.transcriptStartMs, decision.transcriptEndMs)}`);
    });
    lines.push('');
  }

  lines.push('## Action Items', '');
  if (state.actionItems.length === 0) {
    lines.push('None noted.', '');
  } else {
    state.actionItems.forEach((item) => {
      const metadata = [
        item.owner ? `owner: ${item.owner}` : null,
        item.dueDate ? `due: ${item.dueDate}` : null,
      ].filter(Boolean);
      const suffix = metadata.length > 0 ? ` (${metadata.join(', ')})` : '';
      lines.push(`- ${item.text}${suffix}${formatExportRange(item.transcriptStartMs, item.transcriptEndMs)}`);
    });
    lines.push('');
  }

  addStringSection(lines, 'Open Questions', state.openQuestions);
  addStringSection(lines, 'Risks', state.risks);

  lines.push('## Highlights', '');
  if (state.highlights.length === 0) {
    lines.push('None noted.', '');
  } else {
    state.highlights.forEach((highlight) => {
      const marker = highlight.userMarked ? 'User-marked: ' : '';
      const note = highlight.note ? ` - ${highlight.note}` : '';
      lines.push(`- ${marker}${highlight.text || 'Marked highlight'}${note}${formatExportRange(highlight.transcriptStartMs, highlight.transcriptEndMs)}`);
    });
    lines.push('');
  }

  return lines.join('\n').trimEnd();
}

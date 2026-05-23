import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Transcript } from '@/types';
import { ModelConfig } from '@/services/configService';
import {
  createEmptyLiveNotesState,
  formatTranscriptChunk,
  LiveHighlight,
  LiveNotesState,
  LiveNotesUpdate,
  mergeLiveNotesUpdate,
} from '@/types/liveNotes';

const MIN_AUTO_TRANSCRIPTS = 4;
const LIVE_NOTES_INTERVAL_MS = 90_000;
const HIGHLIGHT_WINDOW_SECONDS = 90;

interface UseLiveNotesProps {
  meetingId: string | null;
  transcripts: Transcript[];
  isRecording: boolean;
  modelConfig: ModelConfig;
}

export function useLiveNotes({
  meetingId,
  transcripts,
  isRecording,
  modelConfig,
}: UseLiveNotesProps) {
  const effectiveMeetingId = meetingId || 'active-meeting';
  const [liveNotes, setLiveNotes] = useState<LiveNotesState>(() =>
    createEmptyLiveNotesState(effectiveMeetingId)
  );
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lastProcessedIndexRef = useRef(0);
  const latestTranscriptsRef = useRef(transcripts);
  const latestLiveNotesRef = useRef(liveNotes);

  useEffect(() => {
    latestTranscriptsRef.current = transcripts;
  }, [transcripts]);

  useEffect(() => {
    latestLiveNotesRef.current = liveNotes;
  }, [liveNotes]);

  useEffect(() => {
    setLiveNotes(createEmptyLiveNotesState(effectiveMeetingId));
    lastProcessedIndexRef.current = 0;
    setError(null);
  }, [effectiveMeetingId]);

  const transcriptText = useMemo(() => formatTranscriptChunk(transcripts), [transcripts]);

  const generateUpdate = useCallback(async (mode: 'auto' | 'manual' | 'recap' = 'manual') => {
    const currentTranscripts = latestTranscriptsRef.current;
    const startIndex = mode === 'recap'
      ? Math.max(currentTranscripts.length - 8, 0)
      : lastProcessedIndexRef.current;
    const transcriptChunk = currentTranscripts.slice(startIndex);

    if (transcriptChunk.length === 0) {
      toast.info('No new transcript to summarize yet.');
      return;
    }

    setIsUpdating(true);
    setError(null);

    try {
      const response = await invoke<LiveNotesUpdate>('api_generate_live_notes_update', {
        transcriptChunk: formatTranscriptChunk(transcriptChunk),
        currentState: latestLiveNotesRef.current,
        model: modelConfig.provider,
        modelName: modelConfig.model,
        ollamaEndpoint: modelConfig.ollamaEndpoint || null,
        customOpenaiEndpoint: modelConfig.customOpenAIEndpoint || null,
        customOpenaiApiKey: modelConfig.customOpenAIApiKey || null,
        customOpenaiMaxTokens: modelConfig.maxTokens || null,
        customOpenaiTemperature: modelConfig.temperature || null,
        customOpenaiTopP: modelConfig.topP || null,
      });

      setLiveNotes((current) => mergeLiveNotesUpdate(current, response));

      if (mode !== 'recap') {
        lastProcessedIndexRef.current = currentTranscripts.length;
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      if (mode !== 'auto') {
        toast.error('Live notes update failed', { description: message });
      }
    } finally {
      setIsUpdating(false);
    }
  }, [
    modelConfig.provider,
    modelConfig.model,
    modelConfig.ollamaEndpoint,
    modelConfig.customOpenAIEndpoint,
    modelConfig.customOpenAIApiKey,
    modelConfig.maxTokens,
    modelConfig.temperature,
    modelConfig.topP,
  ]);

  useEffect(() => {
    if (!isRecording) return;

    const interval = setInterval(() => {
      const newTranscriptCount = latestTranscriptsRef.current.length - lastProcessedIndexRef.current;
      if (newTranscriptCount >= MIN_AUTO_TRANSCRIPTS) {
        generateUpdate('auto');
      }
    }, LIVE_NOTES_INTERVAL_MS);

    return () => clearInterval(interval);
  }, [isRecording, generateUpdate]);

  const markHighlight = useCallback(() => {
    const currentTranscripts = latestTranscriptsRef.current;
    if (currentTranscripts.length === 0) {
      toast.info('No transcript available to highlight yet.');
      return;
    }

    const latestEnd = currentTranscripts
      .map((transcript) => transcript.audio_end_time ?? transcript.audio_start_time ?? 0)
      .reduce((max, value) => Math.max(max, value), 0);
    const windowStart = Math.max(latestEnd - HIGHLIGHT_WINDOW_SECONDS, 0);
    const highlightedTranscripts = currentTranscripts.filter((transcript) => {
      const start = transcript.audio_start_time ?? transcript.chunk_start_time ?? 0;
      return start >= windowStart;
    });

    const text = highlightedTranscripts.map((transcript) => transcript.text).join(' ').trim();
    const highlight: LiveHighlight = {
      id: `manual-${Date.now()}`,
      text: text || 'Marked recent moment',
      userMarked: true,
      transcriptStartMs: windowStart * 1000,
      transcriptEndMs: latestEnd * 1000,
      createdAt: new Date().toISOString(),
    };

    setLiveNotes((current) => ({
      ...current,
      updatedAt: new Date().toISOString(),
      highlights: [...current.highlights, highlight].slice(-12),
    }));
    toast.success('Highlight marked');
  }, []);

  return {
    liveNotes,
    transcriptText,
    isUpdating,
    error,
    generateUpdate,
    markHighlight,
  };
}

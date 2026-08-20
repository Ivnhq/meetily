import type { Transcript } from '@/types';

function formatTranscriptTime(seconds: number | undefined, fallbackTimestamp: string): string {
  if (seconds === undefined) return fallbackTimestamp;

  const totalSeconds = Math.floor(seconds);
  const minutes = Math.floor(totalSeconds / 60);
  const remainingSeconds = totalSeconds % 60;
  return `[${minutes.toString().padStart(2, '0')}:${remainingSeconds
    .toString()
    .padStart(2, '0')}]`;
}

export function buildSummaryTranscriptPayload(allTranscripts: Transcript[]) {
  return {
    transcriptText: allTranscripts
      .map(
        transcript =>
          `${formatTranscriptTime(transcript.audio_start_time, transcript.timestamp)} ${
            transcript.speaker ? `${transcript.speaker}: ` : ''
          }${transcript.text}`
      )
      .join('\n'),
    transcriptTexts: allTranscripts.map(transcript => transcript.text),
  };
}

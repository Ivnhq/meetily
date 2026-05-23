'use client';

import type React from 'react';
import { AlertCircle, CheckCircle2, CircleHelp, Clipboard, Clock3, Highlighter, ListChecks, Loader2, RefreshCw, Sparkles } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { formatLiveNotesAsMarkdown, LiveNotesRecap, LiveNotesState } from '@/types/liveNotes';

interface LiveNotesPanelProps {
  liveNotes: LiveNotesState;
  latestRecap: LiveNotesRecap | null;
  isRecording: boolean;
  isUpdating: boolean;
  error: string | null;
  onGenerateUpdate: () => void;
  onRecap: () => void;
  onMarkHighlight: () => void;
}

function EmptyList({ text }: { text: string }) {
  return <p className="text-sm text-gray-400">{text}</p>;
}

function formatEvidenceTime(ms: number): string {
  const totalSeconds = Math.max(Math.floor(ms / 1000), 0);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  return `${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
}

function EvidenceRange({
  startMs,
  endMs,
}: {
  startMs?: number;
  endMs?: number;
}) {
  if (typeof startMs !== 'number' && typeof endMs !== 'number') return null;

  const label = typeof startMs === 'number' && typeof endMs === 'number'
    ? `${formatEvidenceTime(startMs)}-${formatEvidenceTime(endMs)}`
    : formatEvidenceTime(startMs ?? endMs ?? 0);

  return (
    <span className="mt-1 inline-flex items-center gap-1 text-xs text-gray-400">
      <Clock3 className="h-3 w-3" />
      {label}
    </span>
  );
}

function StringList({ items, empty }: { items: string[]; empty: string }) {
  if (items.length === 0) return <EmptyList text={empty} />;

  return (
    <ul className="space-y-2">
      {items.map((item, index) => (
        <li key={`${item}-${index}`} className="text-sm leading-relaxed text-gray-700">
          {item}
        </li>
      ))}
    </ul>
  );
}

function Section({
  title,
  icon,
  children,
}: {
  title: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-normal text-gray-500">
        {icon}
        <span>{title}</span>
      </div>
      {children}
    </section>
  );
}

export function LiveNotesPanel({
  liveNotes,
  latestRecap,
  isRecording,
  isUpdating,
  error,
  onGenerateUpdate,
  onRecap,
  onMarkHighlight,
}: LiveNotesPanelProps) {
  const copyMarkdown = async () => {
    try {
      await navigator.clipboard.writeText(formatLiveNotesAsMarkdown(liveNotes));
      toast.success('Live notes copied as Markdown');
    } catch {
      toast.error('Could not copy Live Notes Markdown');
    }
  };

  return (
    <aside className="w-full xl:w-[380px] flex-shrink-0 border-t xl:border-t-0 xl:border-l border-gray-200 bg-gray-50">
      <div className="sticky top-0 max-h-screen overflow-y-auto p-4 pb-28">
        <div className="mb-4 flex items-center justify-between gap-3">
          <div>
            <div className="flex items-center gap-2">
              <Sparkles className="h-4 w-4 text-blue-600" />
              <h2 className="text-sm font-semibold text-gray-900">Live Notes</h2>
            </div>
            <p className="mt-1 text-xs text-gray-500">
              {isUpdating
                ? 'Updating with local AI...'
                : liveNotes.updatedAt
                  ? `Updated ${new Date(liveNotes.updatedAt).toLocaleTimeString()}`
                  : 'Ready once the transcript has enough context'}
            </p>
          </div>

          {isUpdating && <Loader2 className="h-4 w-4 animate-spin text-blue-600" />}
        </div>

        <div className="mb-4 flex flex-wrap gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={onGenerateUpdate}
                disabled={isUpdating}
              >
                <RefreshCw className="h-4 w-4" />
                <span className="hidden 2xl:inline">Update</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>Summarize new transcript now</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={onRecap}
                disabled={isUpdating}
              >
                <CircleHelp className="h-4 w-4" />
                <span className="hidden 2xl:inline">Recap</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>Summarize the latest few transcript segments</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={onMarkHighlight}
                disabled={!isRecording}
              >
                <Highlighter className="h-4 w-4" />
                <span className="hidden 2xl:inline">Mark</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>Mark the last 90 seconds as a highlight</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={copyMarkdown}
              >
                <Clipboard className="h-4 w-4" />
                <span className="hidden 2xl:inline">Copy</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>Copy Live Notes as Markdown</TooltipContent>
          </Tooltip>
        </div>

        {error && (
          <div className="mb-4 flex gap-2 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700">
            <AlertCircle className="mt-0.5 h-4 w-4 flex-shrink-0" />
            <p>{error}</p>
          </div>
        )}

        <div className="space-y-6">
          {latestRecap && (
            <Section title="Latest Recap" icon={<CircleHelp className="h-3.5 w-3.5" />}>
              <div className="space-y-2 border-l-2 border-blue-200 pl-3">
                <p className="text-xs text-gray-500">
                  Generated {new Date(latestRecap.generatedAt).toLocaleTimeString()}
                </p>
                {latestRecap.currentTopic && (
                  <p className="text-sm font-medium leading-relaxed text-gray-800">
                    {latestRecap.currentTopic}
                  </p>
                )}
                <StringList
                  items={[
                    ...latestRecap.rollingSummary,
                    ...latestRecap.keyPoints,
                    ...latestRecap.openQuestions.map((question) => `Question: ${question}`),
                  ]}
                  empty="No recap details returned."
                />
              </div>
            </Section>
          )}

          <Section title="Current Topic" icon={<Sparkles className="h-3.5 w-3.5" />}>
            <p className="text-sm leading-relaxed text-gray-800">
              {liveNotes.currentTopic || 'Waiting for the meeting shape to emerge.'}
            </p>
          </Section>

          <Section title="Summary" icon={<ListChecks className="h-3.5 w-3.5" />}>
            <StringList items={liveNotes.rollingSummary} empty="No summary yet." />
          </Section>

          <Section title="Action Items" icon={<CheckCircle2 className="h-3.5 w-3.5" />}>
            {liveNotes.actionItems.length === 0 ? (
              <EmptyList text="No action items yet." />
            ) : (
              <ul className="space-y-2">
                {liveNotes.actionItems.map((item, index) => (
                  <li key={`${item.text}-${index}`} className="text-sm leading-relaxed text-gray-700">
                    <div>
                      {item.text}
                      {item.owner && <span className="ml-1 text-gray-500">({item.owner})</span>}
                    </div>
                    <EvidenceRange startMs={item.transcriptStartMs} endMs={item.transcriptEndMs} />
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title="Decisions" icon={<CheckCircle2 className="h-3.5 w-3.5" />}>
            {liveNotes.decisions.length === 0 ? (
              <EmptyList text="No decisions captured yet." />
            ) : (
              <ul className="space-y-2">
                {liveNotes.decisions.map((decision, index) => (
                  <li key={`${decision.text}-${index}`} className="text-sm leading-relaxed text-gray-700">
                    <div>{decision.text}</div>
                    <EvidenceRange startMs={decision.transcriptStartMs} endMs={decision.transcriptEndMs} />
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title="Open Questions" icon={<CircleHelp className="h-3.5 w-3.5" />}>
            <StringList items={liveNotes.openQuestions} empty="No open questions yet." />
          </Section>

          <Section title="Highlights" icon={<Highlighter className="h-3.5 w-3.5" />}>
            {liveNotes.highlights.length === 0 ? (
              <EmptyList text="Use the marker during an important moment." />
            ) : (
              <ul className="space-y-2">
                {liveNotes.highlights.slice().reverse().map((highlight) => (
                  <li key={highlight.id} className="rounded-md border border-gray-200 bg-white p-2 text-sm leading-relaxed text-gray-700">
                    <div>{highlight.text || 'Marked highlight'}</div>
                    <EvidenceRange startMs={highlight.transcriptStartMs} endMs={highlight.transcriptEndMs} />
                  </li>
                ))}
              </ul>
            )}
          </Section>
        </div>
      </div>
    </aside>
  );
}

'use client';

import type React from 'react';
import { AlertCircle, CheckCircle2, CircleHelp, Highlighter, ListChecks, Loader2, RefreshCw, Sparkles } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { LiveNotesState } from '@/types/liveNotes';

interface LiveNotesPanelProps {
  liveNotes: LiveNotesState;
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
  isRecording,
  isUpdating,
  error,
  onGenerateUpdate,
  onRecap,
  onMarkHighlight,
}: LiveNotesPanelProps) {
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
        </div>

        {error && (
          <div className="mb-4 flex gap-2 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-700">
            <AlertCircle className="mt-0.5 h-4 w-4 flex-shrink-0" />
            <p>{error}</p>
          </div>
        )}

        <div className="space-y-6">
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
                    {item.text}
                    {item.owner && <span className="ml-1 text-gray-500">({item.owner})</span>}
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
                    {decision.text}
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
                    {highlight.text || 'Marked highlight'}
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

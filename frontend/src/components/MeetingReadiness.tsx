'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { CalendarDays, CheckCircle2, CircleAlert, Upload } from 'lucide-react';
import { toast } from 'sonner';

interface CalendarEvent {
  id: string;
  title: string;
  starts_at: string;
  ends_at: string;
}

interface Detection {
  detected: boolean;
  event?: CalendarEvent;
  meeting_app_running: boolean;
  reason: string;
}

interface ReadinessCheck {
  id: string;
  label: string;
  ready: boolean;
  detail: string;
}

interface MeetingAnswer {
  answer: string;
  grounded: boolean;
  citations: Array<{
    meeting_id: string;
    meeting_title: string;
    timestamp: string;
    audio_start_time?: number;
    speaker?: string;
    quote: string;
  }>;
}

interface PostMeetingHook {
  id: string;
  name: string;
  kind: 'artifact_export' | 'obsidian_export';
  destination: string;
  folder?: string;
  enabled: boolean;
}

export function MeetingReadiness({
  isRecording,
  onUseMeetingTitle,
}: {
  isRecording: boolean;
  onUseMeetingTitle: (title: string) => void;
}) {
  const [detection, setDetection] = useState<Detection | null>(null);
  const [checks, setChecks] = useState<ReadinessCheck[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [question, setQuestion] = useState('');
  const [answer, setAnswer] = useState<MeetingAnswer | null>(null);
  const [isAsking, setIsAsking] = useState(false);
  const [hooks, setHooks] = useState<PostMeetingHook[]>([]);

  const refresh = useCallback(async () => {
    if (isRecording) return;
    const [nextDetection, nextChecks, nextHooks] = await Promise.all([
      invoke<Detection>('api_detect_current_meeting'),
      invoke<ReadinessCheck[]>('api_get_meeting_readiness'),
      invoke<PostMeetingHook[]>('api_list_post_meeting_hooks'),
    ]);
    setDetection(nextDetection);
    setChecks(nextChecks);
    setHooks(nextHooks);
  }, [isRecording]);

  useEffect(() => {
    void refresh().catch((error) => console.warn('Readiness check failed', error));
    const timer = window.setInterval(() => {
      void refresh().catch((error) => console.warn('Meeting detection failed', error));
    }, 60_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const importCalendar = async () => {
    const path = await open({ multiple: false, filters: [{ name: 'Calendar', extensions: ['ics'] }] });
    if (!path) return;
    const result = await invoke<{ imported: number }>('api_import_calendar_ics', { path });
    toast.success(`Imported ${result.imported} calendar events`);
    await refresh();
  };

  const askMeetings = async () => {
    if (!question.trim()) return;
    setIsAsking(true);
    try {
      setAnswer(await invoke<MeetingAnswer>('api_ask_meetings', { question, limit: 6 }));
      setExpanded(true);
    } catch (error) {
      toast.error('Meeting search failed', { description: String(error) });
    } finally {
      setIsAsking(false);
    }
  };

  const toggleObsidianHook = async () => {
    const existing = hooks.find((hook) => hook.kind === 'obsidian_export');
    let destination = existing?.destination;
    if (!destination) {
      const selected = await open({ directory: true, multiple: false, title: 'Choose your Obsidian vault' });
      if (!selected) return;
      destination = selected;
    }
    const enabled = !existing?.enabled;
    await invoke('api_save_post_meeting_hook', {
      id: existing?.id || null,
      name: 'Obsidian auto-export',
      kind: 'obsidian_export',
      destination,
      folder: existing?.folder || 'Meetily',
      enabled,
    });
    toast.success(enabled ? 'Obsidian auto-export enabled' : 'Obsidian auto-export paused');
    await refresh();
  };

  if (isRecording) return null;
  const failed = checks.filter((check) => !check.ready && check.id !== 'calendar');

  return (
    <div className="mx-auto mt-4 w-2/3 max-w-[750px] rounded-xl border border-gray-200 bg-white px-4 py-3 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          {failed.length === 0 ? <CheckCircle2 className="h-4 w-4 text-emerald-600" /> : <CircleAlert className="h-4 w-4 text-amber-600" />}
          <div>
            <p className="text-sm font-medium text-gray-800">
              {detection?.event ? detection.event.title : failed.length === 0 ? 'Ready for your next meeting' : `${failed.length} readiness item${failed.length === 1 ? '' : 's'} need attention`}
            </p>
            <p className="text-xs text-gray-500">{detection?.reason || 'Checking local meeting signals…'}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {detection?.event && (
            <button className="rounded-md bg-gray-900 px-3 py-1.5 text-xs font-medium text-white" onClick={() => onUseMeetingTitle(detection.event!.title)}>
              Use meeting title
            </button>
          )}
          <button className="flex items-center gap-1 rounded-md border px-3 py-1.5 text-xs text-gray-600" onClick={() => void importCalendar()}>
            <Upload className="h-3.5 w-3.5" /> Import .ics
          </button>
          <button className="flex items-center gap-1 text-xs text-gray-500" onClick={() => setExpanded((value) => !value)}>
            <CalendarDays className="h-3.5 w-3.5" /> {expanded ? 'Hide' : 'Details'}
          </button>
        </div>
      </div>
      {expanded && (
        <div className="mt-3 border-t pt-3">
          <div className="grid gap-2 sm:grid-cols-2">
            {checks.map((check) => (
              <div key={check.id} className="flex items-start gap-2 text-xs">
                {check.ready ? <CheckCircle2 className="mt-0.5 h-3.5 w-3.5 text-emerald-600" /> : <CircleAlert className="mt-0.5 h-3.5 w-3.5 text-amber-600" />}
                <div><p className="font-medium text-gray-700">{check.label}</p><p className="text-gray-500">{check.detail}</p></div>
              </div>
            ))}
          </div>
          <div className="mt-3 flex gap-2 border-t pt-3">
            <input
              className="min-w-0 flex-1 rounded-md border px-3 py-2 text-xs"
              placeholder="Ask across saved meetings…"
              value={question}
              onChange={(event) => setQuestion(event.target.value)}
              onKeyDown={(event) => { if (event.key === 'Enter') void askMeetings(); }}
            />
            <button disabled={isAsking || !question.trim()} className="rounded-md bg-gray-900 px-3 py-2 text-xs font-medium text-white disabled:opacity-50" onClick={() => void askMeetings()}>
              {isAsking ? 'Searching…' : 'Ask'}
            </button>
          </div>
          <div className="mt-2 flex justify-end">
            <button className="text-xs font-medium text-gray-600 hover:text-gray-900" onClick={() => void toggleObsidianHook()}>
              {hooks.some((hook) => hook.kind === 'obsidian_export' && hook.enabled)
                ? 'Pause Obsidian auto-export'
                : 'Enable Obsidian auto-export'}
            </button>
          </div>
          {answer && (
            <div className="mt-3 rounded-md bg-gray-50 p-3 text-xs text-gray-700">
              <p className="whitespace-pre-wrap">{answer.answer}</p>
              {answer.citations.length > 0 && (
                <div className="mt-3 space-y-2 border-t pt-2">
                  {answer.citations.map((citation, index) => (
                    <button
                      type="button"
                      key={`${citation.meeting_id}-${index}`}
                      className="block w-full rounded border bg-white p-2 text-left hover:border-gray-400"
                      onClick={() => { window.location.href = `/meeting-details?id=${encodeURIComponent(citation.meeting_id)}`; }}
                    >
                      <span className="font-medium">[{index + 1}] {citation.meeting_title}</span>
                      <span className="ml-2 text-gray-400">{citation.speaker || citation.timestamp}</span>
                      <p className="mt-1 line-clamp-2 text-gray-600">{citation.quote}</p>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

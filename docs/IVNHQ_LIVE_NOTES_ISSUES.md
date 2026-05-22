# Ivnhq Live Notes Issue Backlog

Use these as the initial GitHub issues for the `Ivnhq/meetily` fork.

## 1. Milestone: Live Notes MVP

Create a milestone named `Live Notes MVP`.

Goal: deliver a local-first live notes workspace that updates during an active meeting using the existing transcript stream and local summary provider.

## 2. Issues

### P0: Map Existing Transcript And Summary Flow

Labels: `enhancement`, `ivnhq`, `live-notes`, `P0`

Identify the current path for live transcript segments and final summary generation so Live Notes Mode can reuse existing app architecture.

Acceptance criteria:

- Document the frontend components that render live transcript.
- Document the Tauri/Rust commands or events that deliver transcript segments.
- Document the current summary provider abstraction and where Ollama/Gemma is called.
- Recommend the smallest insertion point for rolling live summaries.

### P0: Add LiveNotesState Model And Merge Logic

Labels: `enhancement`, `ivnhq`, `live-notes`, `P0`

Add a typed LiveNotesState model for rolling meeting notes, actions, decisions, open questions, risks, and highlights.

Acceptance criteria:

- LiveNotesState has meeting id, updated timestamp, rolling summary, key points, action items, decisions, questions, risks, and highlights.
- Generated updates can be validated before merging.
- Merge behavior is deterministic and unit tested.
- Transcript timestamp references are preserved where available.

### P0: Build Live Notes Panel UI

Labels: `enhancement`, `ivnhq`, `live-notes`, `P0`

Add a meeting-time UI panel for structured notes beside the live transcript.

Acceptance criteria:

- Live notes panel can show current topic, key points, decisions, action items, open questions, risks, and highlights.
- Empty states are calm and useful.
- Updates do not shift layout aggressively during a meeting.
- The panel works while recording is active and remains visible after recording stops.

### P0: Add Rolling Local Summary Loop

Labels: `enhancement`, `ivnhq`, `live-notes`, `P0`

Call the local summary provider periodically with only the newest transcript chunk plus current LiveNotesState.

Acceptance criteria:

- Rolling update interval is configurable or safely hardcoded between 60 and 120 seconds for the MVP.
- The prompt asks for strict JSON output.
- The app validates the JSON before updating state.
- The loop can be stopped cleanly when recording ends.
- The loop works with Ollama and Gemma 3 4B.

### P0: Add Manual Highlight Button And Hotkey

Labels: `enhancement`, `ivnhq`, `live-notes`, `P0`

Allow the user to mark the last 30-90 seconds as important during a meeting.

Acceptance criteria:

- User can click a button to mark a highlight.
- User can trigger the same action with a keyboard shortcut.
- Highlight stores transcript start/end timestamps.
- Optional user note can be added after marking.
- Highlights appear in Live Notes and final summary context.

### P1: Add "What Did I Miss?" Recap

Labels: `enhancement`, `ivnhq`, `live-notes`, `P1`

Add a one-click recap of the last few minutes of transcript.

Acceptance criteria:

- User can request a recap of the last 5 minutes during an active meeting.
- Recap uses local model provider.
- Recap is displayed separately from canonical rolling notes.
- Recap does not mutate LiveNotesState unless explicitly saved.

### P1: Generate Final Summary From Transcript Plus Live Notes

Labels: `enhancement`, `ivnhq`, `live-notes`, `P1`

Use LiveNotesState as additional context when generating the final post-meeting summary.

Acceptance criteria:

- Final summary prompt includes the full transcript and LiveNotesState.
- Final output includes executive summary, decisions, action items, open questions, and follow-up draft.
- User-marked highlights are preserved.
- Existing final summary behavior still works if live notes are disabled.

### P2: Export Live Notes To Markdown / Obsidian

Labels: `enhancement`, `ivnhq`, `live-notes`, `P2`

Export transcript, summary, action items, decisions, questions, and highlights to a durable Markdown note.

Acceptance criteria:

- Export writes a clean Markdown file.
- Frontmatter includes meeting title, date, attendees when available, model provider, and source app.
- Action items and decisions link to transcript timestamps where possible.
- Output can be dropped into Obsidian without cleanup.


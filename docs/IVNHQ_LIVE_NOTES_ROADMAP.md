# Ivnhq Live Notes Roadmap

This document tracks Ivnhq's planned enhancements to Meetily Community Edition for a more Fathom-like, local-first live meeting workflow.

## Product Direction

Meetily already has the right local primitives:

- Parakeet/Whisper transcription.
- Local summaries through Ollama.
- Local SQLite storage.
- Native desktop capture through Tauri.

The next product gap is meeting-time usefulness: while a call is still happening, the user should see structured notes, decisions, action items, and quick recaps without waiting for the final post-meeting summary.

## Source And Speaker Labeling Direction

Treat speaker identity as a layered problem:

1. **Realtime source labels:** preserve whether transcript text came from the microphone or system audio path.
2. **Realtime overlap policy:** suppress or mark microphone audio while system audio is active to reduce duplicate remote-speaker bleed.
3. **Local speaker clustering:** split each source into stable speaker clusters such as `Remote speaker 1` and `Remote speaker 2`.
4. **Speaker naming:** assign human-readable names from user correction, meeting context, or attendee metadata after diarization.

The MVP should not claim true participant identity from ScreenCaptureKit alone. The first reliable labels are source-aware speaker clusters such as `Local speaker`, `Remote speaker 1`, and `Remote speaker 2`.

## Recommendation

Build **Live Notes Mode** first.

Live Notes Mode should add a meeting workspace with three core panes:

1. **Transcript**
   - Raw live transcript from the existing transcription engine.
   - Timestamped segments.
   - Speaker labels when available.

2. **Live Notes**
   - Rolling summary updated during the meeting.
   - Current topic.
   - Key points.
   - Decisions.
   - Concerns or risks.
   - Open questions.

3. **Action Bar**
   - Mark highlight.
   - Add manual note.
   - Capture action item.
   - Summarize last 5 minutes.
   - "What did I miss?"

## Prioritized Backlog

| Priority | Feature | Value |
|---|---|---|
| P0 | Live structured notes panel | Gives value during the call, not only after it ends. |
| P0 | Live decisions/action items/questions extraction | Turns transcript into an operational meeting surface. |
| P0 | Manual highlight hotkey | Lets the user mark important moments without breaking conversation flow. |
| P1 | Rolling summary loop | Keeps summaries fast by summarizing new transcript chunks plus prior state. |
| P1 | "What did I miss?" recap | One-click catch-up for the last few minutes. |
| P1 | Final executive summary template | Produces a concise post-meeting artifact with actions and follow-up draft. |
| P2 | Markdown/Obsidian export | Makes meeting notes durable outside the app. |
| P2 | Meeting memory search | Enables questions across prior meetings. |
| P2 | Calendar-aware meeting titles | Uses calendar context for meeting metadata. |
| P3 | ClickUp/CRM push | Pushes action items and follow-ups into workflow systems. |

## First Milestone

**Milestone 1: Local Live Notes MVP**

Success criteria:

- A meeting can be recorded with Parakeet transcription.
- The UI shows a live notes panel alongside transcript.
- Every 60-120 seconds, the app updates structured notes from the newest transcript chunk.
- The notes model includes summary, action items, decisions, open questions, and highlights.
- A manual highlight button captures the last configurable time window.
- The final meeting summary can incorporate the live notes state.
- The feature works with a local Ollama model, including Gemma 3 4B.

Non-goals for milestone 1:

- Auto-joining calls as a meeting bot.
- CRM sync.
- Team collaboration.
- Cloud storage.
- Replacing the existing final summary flow.

## Suggested Architecture

Use an incremental summarization model rather than resummarizing the entire meeting repeatedly.

```text
live transcript segment stream
  -> chunk buffer, approximately 60-120 seconds
  -> local LLM prompt with current LiveNotesState
  -> JSON patch/update
  -> persist LiveNotesState
  -> render live notes UI
```

Core state shape:

```ts
type LiveNotesState = {
  meetingId: string;
  updatedAt: string;
  currentTopic?: string;
  rollingSummary: string[];
  keyPoints: string[];
  decisions: LiveDecision[];
  actionItems: LiveActionItem[];
  openQuestions: string[];
  risks: string[];
  highlights: LiveHighlight[];
};

type LiveActionItem = {
  text: string;
  owner?: string;
  dueDate?: string;
  transcriptStartMs?: number;
  transcriptEndMs?: number;
  confidence?: number;
};

type LiveDecision = {
  text: string;
  transcriptStartMs?: number;
  transcriptEndMs?: number;
  confidence?: number;
};

type LiveHighlight = {
  text?: string;
  userMarked: boolean;
  transcriptStartMs: number;
  transcriptEndMs: number;
  note?: string;
};
```

The LLM should return structured JSON. The app should validate and merge the update instead of trusting free-form prose.

## UX Principles

- Keep the meeting UI calm. The user is in a live conversation.
- Prefer small, continuously useful updates over large AI-generated blocks.
- Preserve raw transcript timestamps so every generated item can link back to evidence.
- Make local/private behavior obvious.
- Make manual capture fast: one click or one hotkey.

## Implementation Notes To Validate In Code

- Find the existing transcript segment event flow from Rust/Tauri to the Next.js frontend.
- Find the current final summary command and provider abstraction.
- Reuse the summary provider path for live note updates where possible.
- Add throttling/debouncing so the local model is not called too often.
- Persist live notes either alongside meeting summaries or in a new table keyed by meeting id.
- Add a feature flag or settings toggle if live summarization has meaningful CPU/GPU cost.

## Test Plan

- Unit-test LiveNotesState merge behavior.
- Prompt-test structured JSON outputs with Gemma 3 4B.
- Record a 5-minute local test meeting and verify at least two rolling updates.
- Confirm the UI remains responsive during local summarization.
- Confirm generated actions/decisions link back to transcript timestamps.
- Confirm the final summary can be generated from transcript plus LiveNotesState.

## Local Smoke Notes

- 2026-05-23: `gemma3:4b` was installed in Ollama and tested on a CPU-only Linux environment.
- The first JSON prompt attempt timed out after the model loaded on CPU with one thread.
- A shorter prompt with a smaller output cap and `num_thread=4` still produced no response inside four minutes.
- Treat Gemma 3 4B CPU-only Live Notes as a performance risk until tested with GPU/BLAS acceleration or a smaller local model.

## Custom Build Update Safety

- Build and install the Ivnhq macOS app through `scripts/install-macos-live-notes.sh`.
- The custom build disables in-app update checks and points its native updater away from the official Meetily release feed.
- Do not enable the official release feed for the custom app: an official updater payload replaces the forked bundle and removes Live Notes features.
- Update the source branch and rebuild with the installer instead.

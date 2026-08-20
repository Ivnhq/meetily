# Ivnhq Local Meeting Intelligence

This fork extends Meetily's Live Notes workstream with a local, inspectable
meeting-intelligence layer. The installed application remains a desktop-first,
single-user tool: recordings, transcripts, search indexes, speaker names,
calendar context, and hook configuration live in the same local SQLite data
directory as existing meetings.

## Capabilities

### Durable meeting artifacts

Every saved meeting can be exported as a versioned bundle containing:

- `meeting.json` manifest
- `transcript.json` and `transcript.md`
- `live-notes.md` when Live Notes exist
- `summary.md` when a summary exists
- the original audio file when available

Writes use a temporary file followed by an atomic rename. The artifact schema
is currently `1.0.0`.

### Read-only CLI and MCP

The app bundle includes `meetily-cli` in `Contents/MacOS`. It opens the SQLite
database in read-only mode.

```bash
"/Applications/Meetily Live Notes.app/Contents/MacOS/meetily-cli" info
"/Applications/Meetily Live Notes.app/Contents/MacOS/meetily-cli" meetings list
"/Applications/Meetily Live Notes.app/Contents/MacOS/meetily-cli" search "launch budget"
```

The same executable exposes an MCP server over standard input/output. Transcript
access is denied by default and must be granted for each launched process:

```json
{
  "command": "/Applications/Meetily Live Notes.app/Contents/MacOS/meetily-cli",
  "args": ["mcp", "--allow-read"]
}
```

MCP tools are read-only: list meetings, get one meeting, full-text search, and
cited cross-meeting Q&A. There is no network listener and no write tool.

### Search and cited Q&A

SQLite FTS5 indexes existing and new transcript text. Search results and Q&A
responses carry meeting, transcript, timestamp, speaker, and quote citations.
The current Q&A path is deliberately extractive: it returns relevant meeting
evidence without inventing an uncited synthesis. A future local-model synthesis
can be added above the same citation contract.

### Speaker correction and remembered profiles

Click a speaker label in a saved meeting to rename every segment assigned to
that diarized speaker. `Remember` averages the speaker's six-feature acoustic
fingerprints locally and reapplies the label when a later cluster is sufficiently
close. This lightweight matcher is useful personalization, not biometric-grade
identity verification. Cross-source duplicate suppression removes
only highly similar, time-overlapping microphone/system transcript pairs and
keeps distinct overlapping speech.

### Calendar context, detection, and readiness

Meetily imports standard `.ics` files chosen by the user. It does not request a
cloud calendar token or silently scrape a calendar database. The home screen
polls the imported schedule for a due meeting, reports whether a known meeting
app is running, and lets the user apply the calendar title before pressing
record. Detection never starts recording automatically.

The readiness panel checks local meeting storage, microphone and system-audio
sources, available transcription models, and imported calendar context.

### Obsidian and controlled post-meeting hooks

Saved meetings can be exported to an Obsidian vault chosen by the user. Notes
contain YAML metadata, transcript, Live Notes, and summary data.

Post-meeting hooks are stored locally and are restricted to two built-in kinds:

- `artifact_export`
- `obsidian_export`

Hooks require absolute destination paths. Arbitrary shell commands, network
webhooks, and external sends are intentionally unsupported. Every run is logged
as success or failure, and a failed hook never discards the saved meeting.

## Privacy and operating boundaries

- Import, search, Q&A, speaker profiles, exports, and hooks run locally.
- Cloud summary providers remain optional existing Meetily behavior and are not
  invoked by the new search/Q&A paths.
- Calendar import and Obsidian export require an explicit file or folder choice.
- MCP transcript access requires `--allow-read` for every process.
- The Ivnhq installer disables in-app updates and points updater metadata at the
  fork so an official Meetily update cannot overwrite the custom app.

## Database migrations

The feature set adds additive migrations for FTS5, speaker profiles, calendar
context, and post-meeting hooks. Existing meetings are backfilled into FTS5.
No existing transcript, Live Notes, summary, or audio columns are removed.

use crate::database::models::{MeetingModel, Transcript};
use crate::state::AppState;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const ARTIFACT_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingArtifactBundle {
    pub schema_version: String,
    pub generated_at: String,
    pub meeting: MeetingModel,
    pub transcripts: Vec<Transcript>,
    pub summary: Option<Value>,
    pub live_notes: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactExportResult {
    pub schema_version: String,
    pub meeting_id: String,
    pub output_directory: String,
    pub files: Vec<String>,
}

pub async fn load_meeting_artifact_bundle(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<MeetingArtifactBundle> {
    if meeting_id.trim().is_empty() {
        return Err(anyhow!("meeting id cannot be empty"));
    }

    let meeting = sqlx::query_as::<_, MeetingModel>(
        "SELECT id, title, created_at, updated_at, folder_path FROM meetings WHERE id = ?",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("meeting not found: {meeting_id}"))?;

    let transcripts = sqlx::query_as::<_, Transcript>(
        "SELECT * FROM transcripts WHERE meeting_id = ? ORDER BY COALESCE(audio_start_time, 0), timestamp, id",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    let summary_json: Option<String> =
        sqlx::query_scalar("SELECT result FROM summary_processes WHERE meeting_id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await?
            .flatten();
    let summary = parse_optional_json(summary_json, "summary")?;

    let live_notes_json: Option<String> =
        sqlx::query_scalar("SELECT state_json FROM live_notes WHERE meeting_id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await?;
    let live_notes = parse_optional_json(live_notes_json, "live notes")?;

    Ok(MeetingArtifactBundle {
        schema_version: ARTIFACT_SCHEMA_VERSION.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        meeting,
        transcripts,
        summary,
        live_notes,
    })
}

pub async fn export_meeting_artifacts(
    pool: &SqlitePool,
    meeting_id: &str,
    destination: Option<&Path>,
) -> Result<ArtifactExportResult> {
    let bundle = load_meeting_artifact_bundle(pool, meeting_id).await?;
    let output_directory = destination
        .map(Path::to_path_buf)
        .or_else(|| bundle.meeting.folder_path.as_ref().map(PathBuf::from))
        .ok_or_else(|| anyhow!("meeting has no recording folder; choose an export destination"))?;

    fs::create_dir_all(&output_directory)
        .with_context(|| format!("failed to create {}", output_directory.display()))?;

    let mut files = Vec::new();
    write_json_atomic(
        &output_directory.join("transcript.json"),
        &bundle.transcripts,
    )?;
    files.push("transcript.json".to_string());

    write_text_atomic(
        &output_directory.join("transcript.md"),
        &render_transcript_markdown(&bundle),
    )?;
    files.push("transcript.md".to_string());

    if let Some(live_notes) = &bundle.live_notes {
        write_text_atomic(
            &output_directory.join("live-notes.md"),
            &render_live_notes_markdown(live_notes),
        )?;
        files.push("live-notes.md".to_string());
    }

    if let Some(summary) = &bundle.summary {
        write_text_atomic(
            &output_directory.join("summary.md"),
            &render_summary_markdown(summary),
        )?;
        files.push("summary.md".to_string());
    }

    if let Some(audio_file) = find_audio_file(bundle.meeting.folder_path.as_deref()) {
        let file_name = audio_file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("recording has an invalid audio filename"))?
            .to_string();
        let target = output_directory.join(&file_name);
        if audio_file != target && !target.exists() {
            match fs::hard_link(&audio_file, &target) {
                Ok(_) => {}
                Err(_) => {
                    fs::copy(&audio_file, &target).with_context(|| {
                        format!("failed to copy recording to {}", target.display())
                    })?;
                }
            }
        }
        files.push(file_name);
    }

    let manifest = json!({
        "schemaVersion": ARTIFACT_SCHEMA_VERSION,
        "generatedAt": bundle.generated_at,
        "meeting": bundle.meeting,
        "transcriptCount": bundle.transcripts.len(),
        "hasSummary": bundle.summary.is_some(),
        "hasLiveNotes": bundle.live_notes.is_some(),
        "files": files,
    });
    write_json_atomic(&output_directory.join("meeting.json"), &manifest)?;
    files.insert(0, "meeting.json".to_string());

    Ok(ArtifactExportResult {
        schema_version: ARTIFACT_SCHEMA_VERSION.to_string(),
        meeting_id: meeting_id.to_string(),
        output_directory: output_directory.to_string_lossy().to_string(),
        files,
    })
}

pub async fn export_meeting_to_obsidian(
    pool: &SqlitePool,
    meeting_id: &str,
    vault_path: &Path,
    folder: Option<&str>,
) -> Result<PathBuf> {
    if !vault_path.is_absolute() {
        return Err(anyhow!("Obsidian vault path must be absolute"));
    }
    let bundle = load_meeting_artifact_bundle(pool, meeting_id).await?;
    let output_directory = folder
        .filter(|value| !value.trim().is_empty())
        .map(|value| vault_path.join(value.trim()))
        .unwrap_or_else(|| vault_path.to_path_buf());
    fs::create_dir_all(&output_directory)?;
    let file_name = format!(
        "{}--{}.md",
        sanitize_file_name(&bundle.meeting.title),
        bundle.meeting.id
    );
    let output = output_directory.join(file_name);
    let mut body = vec![
        "---".to_string(),
        format!("meetily_id: \"{}\"", bundle.meeting.id.replace('"', "\\\"")),
        format!("title: \"{}\"", bundle.meeting.title.replace('"', "\\\"")),
        format!("created: {}", bundle.meeting.created_at.0.to_rfc3339()),
        "tags: [meetily, meeting]".to_string(),
        "---".to_string(),
        String::new(),
        render_transcript_markdown(&bundle),
    ];
    if let Some(live_notes) = &bundle.live_notes {
        body.push(render_live_notes_markdown(live_notes));
    }
    if let Some(summary) = &bundle.summary {
        body.push(render_summary_markdown(summary));
    }
    write_text_atomic(&output, &body.join("\n"))?;
    Ok(output)
}

#[tauri::command]
pub async fn api_export_meeting_to_obsidian(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    vault_path: String,
    folder: Option<String>,
) -> std::result::Result<String, String> {
    export_meeting_to_obsidian(
        state.db_manager.pool(),
        &meeting_id,
        Path::new(&vault_path),
        folder.as_deref(),
    )
    .await
    .map(|path| path.to_string_lossy().to_string())
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn api_export_meeting_artifacts(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    destination: Option<String>,
) -> std::result::Result<ArtifactExportResult, String> {
    let destination = destination.as_deref().map(Path::new);
    export_meeting_artifacts(state.db_manager.pool(), &meeting_id, destination)
        .await
        .map_err(|error| error.to_string())
}

fn parse_optional_json(raw: Option<String>, label: &str) -> Result<Option<Value>> {
    raw.map(|value| {
        serde_json::from_str(&value).with_context(|| format!("stored {label} is not valid JSON"))
    })
    .transpose()
}

fn write_json_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let body = serde_json::to_string_pretty(value)?;
    write_text_atomic(path, &format!("{body}\n"))
}

fn write_text_atomic(path: &Path, body: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("export path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("export path has an invalid filename"))?;
    let temp = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    fs::write(&temp, body).with_context(|| format!("failed to write {}", temp.display()))?;

    if let Err(error) = fs::rename(&temp, path) {
        if path.exists() {
            fs::remove_file(path)?;
            fs::rename(&temp, path)?;
        } else {
            return Err(error).with_context(|| format!("failed to publish {}", path.display()));
        }
    }
    Ok(())
}

fn find_audio_file(folder_path: Option<&str>) -> Option<PathBuf> {
    let folder = Path::new(folder_path?);
    ["audio.mp4", "audio.wav", "audio.m4a", "recording.wav"]
        .into_iter()
        .map(|name| folder.join(name))
        .find(|path| path.is_file())
}

fn render_transcript_markdown(bundle: &MeetingArtifactBundle) -> String {
    let mut lines = vec![
        format!("# {}", bundle.meeting.title),
        String::new(),
        format!("Meeting ID: `{}`", bundle.meeting.id),
        format!("Created: {}", bundle.meeting.created_at.0.to_rfc3339()),
        String::new(),
        "## Transcript".to_string(),
        String::new(),
    ];

    for segment in &bundle.transcripts {
        let time = segment
            .audio_start_time
            .map(format_seconds)
            .unwrap_or_else(|| segment.timestamp.clone());
        let speaker = segment
            .speaker
            .as_deref()
            .or(segment.source.as_deref())
            .unwrap_or("Speaker");
        lines.push(format!(
            "- **[{time}] {speaker}:** {}",
            segment.transcript.trim()
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn render_live_notes_markdown(value: &Value) -> String {
    let mut lines = vec!["# Live Notes".to_string(), String::new()];
    push_scalar_section(&mut lines, "Current Topic", value.get("currentTopic"));
    push_string_list_section(&mut lines, "Rolling Summary", value.get("rollingSummary"));
    push_string_list_section(&mut lines, "Key Points", value.get("keyPoints"));
    push_object_list_section(&mut lines, "Decisions", value.get("decisions"));
    push_object_list_section(&mut lines, "Action Items", value.get("actionItems"));
    push_string_list_section(&mut lines, "Open Questions", value.get("openQuestions"));
    push_string_list_section(&mut lines, "Risks", value.get("risks"));
    push_object_list_section(&mut lines, "Highlights", value.get("highlights"));
    lines.join("\n").trim_end().to_string() + "\n"
}

fn render_summary_markdown(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return format!("# Meeting Summary\n\n{}\n", text.trim());
    }

    let mut lines = vec!["# Meeting Summary".to_string(), String::new()];
    render_json_value(&mut lines, value, 2);
    lines.join("\n").trim_end().to_string() + "\n"
}

fn render_json_value(lines: &mut Vec<String>, value: &Value, heading_level: usize) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if matches!(key.as_str(), "sectionOrder" | "metadata") {
                    continue;
                }
                lines.push(format!(
                    "{} {}",
                    "#".repeat(heading_level.min(6)),
                    humanize(key)
                ));
                lines.push(String::new());
                render_json_value(lines, child, heading_level + 1);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                lines.push("None noted.".to_string());
            } else {
                for item in items {
                    if let Some(text) = item.as_str() {
                        lines.push(format!("- {}", text.trim()));
                    } else if let Some(text) = item.get("text").and_then(Value::as_str) {
                        lines.push(format!("- {}", text.trim()));
                    } else {
                        lines.push(format!("- `{}`", item));
                    }
                }
            }
            lines.push(String::new());
        }
        Value::String(text) => {
            lines.push(text.trim().to_string());
            lines.push(String::new());
        }
        Value::Null => {
            lines.push("None noted.".to_string());
            lines.push(String::new());
        }
        other => {
            lines.push(other.to_string());
            lines.push(String::new());
        }
    }
}

fn push_scalar_section(lines: &mut Vec<String>, title: &str, value: Option<&Value>) {
    lines.push(format!("## {title}"));
    lines.push(String::new());
    lines.push(
        value
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .unwrap_or("None noted.")
            .to_string(),
    );
    lines.push(String::new());
}

fn push_string_list_section(lines: &mut Vec<String>, title: &str, value: Option<&Value>) {
    lines.push(format!("## {title}"));
    lines.push(String::new());
    let items = value.and_then(Value::as_array);
    if items.map(|items| items.is_empty()).unwrap_or(true) {
        lines.push("None noted.".to_string());
    } else if let Some(items) = items {
        for item in items.iter().filter_map(Value::as_str) {
            lines.push(format!("- {}", item.trim()));
        }
    }
    lines.push(String::new());
}

fn push_object_list_section(lines: &mut Vec<String>, title: &str, value: Option<&Value>) {
    lines.push(format!("## {title}"));
    lines.push(String::new());
    let items = value.and_then(Value::as_array);
    if items.map(|items| items.is_empty()).unwrap_or(true) {
        lines.push("None noted.".to_string());
    } else if let Some(items) = items {
        for item in items {
            let text = item
                .get("text")
                .and_then(Value::as_str)
                .or_else(|| item.as_str())
                .unwrap_or("Untitled item");
            let owner = item.get("owner").and_then(Value::as_str);
            let suffix = owner
                .map(|owner| format!(" (owner: {owner})"))
                .unwrap_or_default();
            lines.push(format!("- {}{}", text.trim(), suffix));
        }
    }
    lines.push(String::new());
}

fn format_seconds(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

fn humanize(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if index > 0 && character.is_uppercase() {
            output.push(' ');
        }
        if index == 0 {
            output.extend(character.to_uppercase());
        } else {
            output.push(character);
        }
    }
    output.replace('_', " ")
}

fn sanitize_file_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        "Meeting".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fixture_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, folder_path TEXT)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, transcript TEXT NOT NULL, timestamp TEXT NOT NULL, summary TEXT, action_items TEXT, key_points TEXT, source TEXT, speaker TEXT, speaker_id TEXT, speaker_fingerprint_json TEXT, source_overlap BOOLEAN, audio_start_time REAL, audio_end_time REAL, duration REAL)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE summary_processes (meeting_id TEXT PRIMARY KEY, result TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE live_notes (meeting_id TEXT PRIMARY KEY, state_json TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO meetings VALUES ('meeting-1', 'Weekly Review', '2026-07-14T10:00:00Z', '2026-07-14T11:00:00Z', NULL)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts VALUES ('t1', 'meeting-1', 'Ship the durable export', '10:05', NULL, NULL, NULL, 'System', 'Remote speaker 1', 'remote-speaker-1', '[0.1,0.2,0.3,0.4,0.5,0.6]', 0, 65.0, 68.0, 3.0)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO live_notes VALUES ('meeting-1', '{\"currentTopic\":\"Exports\",\"rollingSummary\":[\"Ship it\"],\"keyPoints\":[],\"decisions\":[{\"text\":\"Use JSON\"}],\"actionItems\":[],\"openQuestions\":[],\"risks\":[],\"highlights\":[]}')")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn exports_versioned_json_and_markdown() {
        let pool = fixture_pool().await;
        let directory = tempfile::tempdir().unwrap();
        let result = export_meeting_artifacts(&pool, "meeting-1", Some(directory.path()))
            .await
            .unwrap();

        assert_eq!(result.schema_version, ARTIFACT_SCHEMA_VERSION);
        assert!(directory.path().join("meeting.json").exists());
        assert!(directory.path().join("transcript.json").exists());
        let transcript = fs::read_to_string(directory.path().join("transcript.md")).unwrap();
        assert!(transcript.contains("**[01:05] Remote speaker 1:**"));
        let notes = fs::read_to_string(directory.path().join("live-notes.md")).unwrap();
        assert!(notes.contains("Use JSON"));
    }

    #[tokio::test]
    async fn exports_obsidian_note_with_stable_metadata() {
        let pool = fixture_pool().await;
        let vault = tempfile::tempdir().unwrap();
        let path = export_meeting_to_obsidian(&pool, "meeting-1", vault.path(), Some("Meetily"))
            .await
            .unwrap();
        let note = fs::read_to_string(path).unwrap();
        assert!(note.contains("meetily_id: \"meeting-1\""));
        assert!(note.contains("# Weekly Review"));
        assert!(note.contains("# Live Notes"));
    }

    #[test]
    fn renders_nested_summary_without_losing_items() {
        let summary =
            json!({"executiveSummary": "A concise result", "actionItems": [{"text": "Follow up"}]});
        let markdown = render_summary_markdown(&summary);
        assert!(markdown.contains("## Executive Summary"));
        assert!(markdown.contains("- Follow up"));
    }
}
